//! Unified diff reader. A hunk header states how many old and new lines the hunk holds, and the
//! reader consumes exactly that many, so a line whose content starts with `++`, `--` or `@@` is
//! never taken for a header. Context and removed lines are counted, never returned as code to
//! review.

pub mod review;

use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddedLine {
    /// Line number in the new file, 1-based.
    pub line: u32,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    /// Path after the change; for a deleted file, the path it had.
    pub path: String,
    pub old_path: Option<String>,
    pub kind: ChangeKind,
    /// Binary content has no lines to read.
    pub binary: bool,
    pub added: Vec<AddedLine>,
    /// New-file line each removal sits before: a removal changes the statement now at that line.
    pub removed_before: Vec<u32>,
}

impl FileDiff {
    /// Lines of the new file the change touches: added lines, and the line that follows a removal.
    #[must_use]
    pub fn touched_lines(&self) -> BTreeSet<u32> {
        self.added
            .iter()
            .map(|a| a.line)
            .chain(self.removed_before.iter().copied())
            .collect()
    }
}

struct Hunk {
    old_left: u32,
    new_left: u32,
    new_line: u32,
}

/// Parses `diff --git` and plain unified diffs. Combined (merge) diffs are skipped: their lines
/// carry one marker per parent and mean something else.
#[must_use]
pub fn parse_unified_diff(diff: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut hunk: Option<Hunk> = None;
    let mut pending_old: Option<String> = None;
    let mut in_header = false;

    for raw in diff.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if let Some(h) = hunk.as_mut() {
            if h.old_left == 0 && h.new_left == 0 {
                hunk = None;
            } else if let Some(file) = files.last_mut() {
                read_hunk_line(h, file, raw);
                continue;
            }
        }
        if let Some(rest) = line.strip_prefix("diff --git ") {
            files.push(FileDiff {
                path: git_header_path(rest),
                old_path: None,
                kind: ChangeKind::Modified,
                binary: false,
                added: Vec::new(),
                removed_before: Vec::new(),
            });
            pending_old = None;
            in_header = true;
        } else if line.starts_with("@@@") {
            in_header = false;
        } else if let Some(h) = hunk_header(line) {
            if files.is_empty() {
                files.push(blank_file());
            }
            hunk = Some(h);
            in_header = false;
        } else if let Some(rest) = line.strip_prefix("--- ") {
            if !in_header {
                files.push(blank_file());
                in_header = true;
            }
            pending_old = Some(header_path(rest));
        } else if let Some(rest) = line.strip_prefix("+++ ") {
            if let Some(file) = files.last_mut() {
                apply_paths(file, pending_old.take(), &header_path(rest));
            }
        } else if in_header && let Some(file) = files.last_mut() {
            header_line(file, line);
        }
    }
    files.retain(|f| !f.path.is_empty() || !f.added.is_empty());
    files
}

fn blank_file() -> FileDiff {
    FileDiff {
        path: String::new(),
        old_path: None,
        kind: ChangeKind::Modified,
        binary: false,
        added: Vec::new(),
        removed_before: Vec::new(),
    }
}

fn read_hunk_line(h: &mut Hunk, file: &mut FileDiff, raw: &str) {
    match raw.as_bytes().first() {
        Some(b'+') => {
            file.added.push(AddedLine {
                line: h.new_line,
                text: raw[1..].strip_suffix('\r').unwrap_or(&raw[1..]).to_owned(),
            });
            h.new_line += 1;
            h.new_left = h.new_left.saturating_sub(1);
        }
        Some(b'-') => {
            file.removed_before.push(h.new_line.max(1));
            h.old_left = h.old_left.saturating_sub(1);
        }
        Some(b'\\') => {}
        _ => {
            h.new_line += 1;
            h.old_left = h.old_left.saturating_sub(1);
            h.new_left = h.new_left.saturating_sub(1);
        }
    }
}

fn hunk_header(line: &str) -> Option<Hunk> {
    let rest = line.strip_prefix("@@ -")?;
    let (old, rest) = rest.split_once(" +")?;
    let (new, _) = rest.split_once(" @@")?;
    let (_, old_len) = range(old)?;
    let (new_start, new_len) = range(new)?;
    Some(Hunk {
        old_left: old_len,
        new_left: new_len,
        new_line: new_start,
    })
}

/// `start` or `start,len`; a bare `start` is a one-line range.
fn range(text: &str) -> Option<(u32, u32)> {
    match text.split_once(',') {
        Some((start, len)) => Some((start.parse().ok()?, len.parse().ok()?)),
        None => Some((text.parse().ok()?, 1)),
    }
}

fn header_line(file: &mut FileDiff, line: &str) {
    if let Some(from) = line.strip_prefix("rename from ") {
        file.old_path = Some(unquote(from));
        file.kind = ChangeKind::Renamed;
    } else if let Some(to) = line.strip_prefix("rename to ") {
        file.path = unquote(to);
        file.kind = ChangeKind::Renamed;
    } else if line.starts_with("new file mode") {
        file.kind = ChangeKind::Added;
    } else if line.starts_with("deleted file mode") {
        file.kind = ChangeKind::Deleted;
    } else if line.starts_with("Binary files ") || line.starts_with("GIT binary patch") {
        file.binary = true;
    }
}

fn apply_paths(file: &mut FileDiff, old: Option<String>, new: &str) {
    let old = old.unwrap_or_default();
    match (old.as_str(), new) {
        ("/dev/null", "/dev/null") => {}
        ("/dev/null", _) => {
            file.kind = ChangeKind::Added;
            file.path = strip_prefix(new, "b/");
        }
        (_, "/dev/null") => {
            file.kind = ChangeKind::Deleted;
            file.path = strip_prefix(&old, "a/");
        }
        _ => file.path = strip_prefix(new, "b/"),
    }
}

/// The `b/` path of `a/x b/x`. When both paths are the same the header splits in the middle, which
/// is also right for names with spaces; a rename is corrected later by `rename to`.
fn git_header_path(rest: &str) -> String {
    if let Some(unquoted) = rest.strip_prefix('"') {
        let end = closing_quote(unquoted);
        let after = unquoted[end..].trim_start_matches('"').trim_start();
        return strip_prefix(&unquote(after), "b/");
    }
    let mid = rest.len() / 2;
    if rest.len() % 2 == 1 && rest.is_char_boundary(mid) {
        let (a, b) = (&rest[..mid], &rest[mid + 1..]);
        if let (Some(old), Some(new)) = (a.strip_prefix("a/"), b.strip_prefix("b/"))
            && old == new
        {
            return new.to_owned();
        }
    }
    rest.rsplit_once(" b/")
        .map(|(_, p)| p.to_owned())
        .unwrap_or_default()
}

fn closing_quote(text: &str) -> usize {
    let mut escaped = false;
    for (i, c) in text.char_indices() {
        match c {
            '\\' if !escaped => escaped = true,
            '"' if !escaped => return i,
            _ => escaped = false,
        }
    }
    text.len()
}

fn strip_prefix(path: &str, prefix: &str) -> String {
    path.strip_prefix(prefix).unwrap_or(path).to_owned()
}

/// A `---`/`+++` path: quoted by git when it holds odd bytes, followed by a tab and a timestamp in
/// plain `diff -u`.
fn header_path(rest: &str) -> String {
    let rest = rest.split('\t').next().unwrap_or(rest);
    unquote(rest.trim_end())
}

/// Git's C-style quoting: `\"`, `\\`, `\n`, `\t` and octal bytes for non-ASCII names.
fn unquote(text: &str) -> String {
    let Some(inner) = text.strip_prefix('"').and_then(|t| t.strip_suffix('"')) else {
        return text.to_owned();
    };
    let mut bytes: Vec<u8> = Vec::with_capacity(inner.len());
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            let mut buf = [0; 4];
            bytes.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            continue;
        }
        match chars.next() {
            Some('n') => bytes.push(b'\n'),
            Some('t') => bytes.push(b'\t'),
            Some(d @ '0'..='3') => {
                let mut value = d.to_digit(8).unwrap_or(0);
                for _ in 0..2 {
                    if let Some(o) = chars.next_if(|c| c.is_digit(8)) {
                        value = value * 8 + o.to_digit(8).unwrap_or(0);
                    }
                }
                bytes.push(u8::try_from(value).unwrap_or(b'?'));
            }
            Some(other) => {
                let mut buf = [0; 4];
                bytes.extend_from_slice(other.encode_utf8(&mut buf).as_bytes());
            }
            None => bytes.push(b'\\'),
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "diff --git a/src/foo.js b/src/foo.js
index 111..222 100644
--- a/src/foo.js
+++ b/src/foo.js
@@ -1,3 +1,4 @@
 function foo() {
-  return 1;
+  var x = eval(input);
+  return x;
 }
";

    fn texts(f: &FileDiff) -> Vec<(u32, &str)> {
        f.added.iter().map(|a| (a.line, a.text.as_str())).collect()
    }

    #[test]
    fn reads_added_lines_with_their_line_numbers() {
        let files = parse_unified_diff(SAMPLE);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "src/foo.js");
        assert_eq!(
            texts(&files[0]),
            [(2, "  var x = eval(input);"), (3, "  return x;")]
        );
        assert_eq!(files[0].removed_before, [2]);
    }

    #[test]
    fn a_line_that_looks_like_a_header_is_content() {
        let diff = "diff --git a/a.txt b/a.txt
--- a/a.txt
+++ b/a.txt
@@ -1,2 +1,3 @@
 keep
+++ not a header
+@@ -9 +9 @@ nor this
";
        let files = parse_unified_diff(diff);
        assert_eq!(files.len(), 1);
        assert_eq!(
            texts(&files[0]),
            [(2, "++ not a header"), (3, "@@ -9 +9 @@ nor this")]
        );
    }

    #[test]
    fn several_hunks_and_files() {
        let diff = "diff --git a/a.rs b/a.rs
--- a/a.rs
+++ b/a.rs
@@ -1 +1,2 @@
 x
+y
@@ -10,2 +11,2 @@
-old
+new
 tail
diff --git a/b.rs b/b.rs
--- a/b.rs
+++ b/b.rs
@@ -5 +5 @@
-p
+q
";
        let files = parse_unified_diff(diff);
        assert_eq!(files.len(), 2);
        assert_eq!(texts(&files[0]), [(2, "y"), (11, "new")]);
        assert_eq!(texts(&files[1]), [(5, "q")]);
        assert_eq!(files[0].touched_lines(), BTreeSet::from([2, 11]));
    }

    #[test]
    fn new_deleted_renamed_and_binary_files() {
        let diff = "diff --git a/new.py b/new.py
new file mode 100644
--- /dev/null
+++ b/new.py
@@ -0,0 +1 @@
+print(1)
diff --git a/gone.py b/gone.py
deleted file mode 100644
--- a/gone.py
+++ /dev/null
@@ -1 +0,0 @@
-print(2)
diff --git a/old.go b/moved.go
similarity index 90%
rename from old.go
rename to moved.go
diff --git a/img.png b/img.png
Binary files a/img.png and b/img.png differ
";
        let files = parse_unified_diff(diff);
        let kinds: Vec<_> = files.iter().map(|f| (f.path.as_str(), f.kind)).collect();
        assert_eq!(
            kinds,
            [
                ("new.py", ChangeKind::Added),
                ("gone.py", ChangeKind::Deleted),
                ("moved.go", ChangeKind::Renamed),
                ("img.png", ChangeKind::Modified),
            ]
        );
        assert_eq!(files[2].old_path.as_deref(), Some("old.go"));
        assert!(files[3].binary);
        assert_eq!(files[1].removed_before, [1]);
    }

    #[test]
    fn paths_with_spaces_and_quotes() {
        let diff = "diff --git a/my file.rs b/my file.rs
--- a/my file.rs
+++ b/my file.rs
@@ -1 +1,2 @@
 a
+b
diff --git \"a/caf\\303\\251.rs\" \"b/caf\\303\\251.rs\"
--- \"a/caf\\303\\251.rs\"
+++ \"b/caf\\303\\251.rs\"
@@ -1 +1,2 @@
 a
+b
";
        let files = parse_unified_diff(diff);
        assert_eq!(files[0].path, "my file.rs");
        assert_eq!(files[1].path, "café.rs");
    }

    #[test]
    fn plain_diff_without_git_header_and_no_newline_marker() {
        let diff = "--- a/x.txt\t2020-01-01
+++ b/x.txt\t2020-01-02
@@ -1 +1 @@
-a
\\ No newline at end of file
+b
\\ No newline at end of file
";
        let files = parse_unified_diff(diff);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "x.txt");
        assert_eq!(texts(&files[0]), [(1, "b")]);
    }

    #[test]
    fn crlf_is_not_part_of_the_line() {
        let files = parse_unified_diff(
            "diff --git a/a b/a\r\n--- a/a\r\n+++ b/a\r\n@@ -1 +1 @@\r\n-x\r\n+y\r\n",
        );
        assert_eq!(texts(&files[0]), [(1, "y")]);
    }

    #[test]
    fn empty_and_garbage_input_give_nothing() {
        assert_eq!(parse_unified_diff("").len(), 0);
        assert_eq!(
            parse_unified_diff("not a diff\n@@ nonsense @@\n+x\n").len(),
            0
        );
    }

    #[test]
    fn combined_diffs_are_skipped() {
        let diff = "diff --cc a.rs
--- a/a.rs
+++ b/a.rs
@@@ -1,2 -1,2 +1,3 @@@
  x
++y
";
        assert!(parse_unified_diff(diff).iter().all(|f| f.added.is_empty()));
    }

    #[test]
    fn truncating_and_corrupting_a_diff_never_panics() {
        let mut seed = 0x9e37_79b9_u32;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        let sample =
            format!("{SAMPLE}{SAMPLE}diff --git \"a/é\" \"b/é\"\n@@ -1,99999999999 +1 @@\n+x\n");
        for cut in 0..sample.len() {
            if sample.is_char_boundary(cut) {
                std::hint::black_box(parse_unified_diff(&sample[..cut]));
            }
        }
        for _ in 0..2_000 {
            let mut bytes = sample.clone().into_bytes();
            for _ in 0..4 {
                let at = next() as usize % bytes.len();
                bytes[at] = b"+- @\n\\\"\r,0"[next() as usize % 10];
            }
            std::hint::black_box(parse_unified_diff(&String::from_utf8_lossy(&bytes)));
        }
    }
}

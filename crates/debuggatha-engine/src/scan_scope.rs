//! What Debuggatha is allowed to look at. A review of code the project does not own
//! (dependencies, build output, generated or minified files) is noise the person cannot act on,
//! so it is excluded once, here, for every review and every analysis. Port of `scan-scope`.
//!
//! The predicates are pure. Listing the files of a repository (git or a directory walk) comes with
//! the corpus gate, where it is exercised end to end.

use std::path::Path;
use std::sync::LazyLock;

use regress::Regex;

/// Directories that hold dependencies or build output, never the project's own source.
pub(crate) const IGNORED_DIRS: &[&str] = &[
    "node_modules",
    "vendor",
    "bower_components",
    "jspm_packages",
    "Pods",
    "Carthage",
    "target",
    "build",
    "dist",
    "out",
    "coverage",
    "__pycache__",
    "site-packages",
    "venv",
    "obj",
    ".gradle",
    ".dart_tool",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".turbo",
    ".venv",
];

/// `bin` and `obj` are build output only beside a build tool; a Node project's `bin/` holds real
/// scripts.
const BUILD_OUTPUT_DIRS: &[&str] = &["bin", "obj"];
const BUILD_MARKERS: &[&str] = &[
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "settings.gradle.kts",
    "pom.xml",
    "Makefile",
    "CMakeLists.txt",
];

#[allow(clippy::expect_used)] // constant patterns, proven by the tests
fn regex(pattern: &str, flags: &str) -> Regex {
    Regex::with_flags(pattern, flags).expect("a valid pattern")
}

static GENERATED_FILE_NAME: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"(\.min\.[a-z]+$|\.bundle\.[a-z]+$|\.generated\.[a-z]+$|\.g\.dart$|\.freezed\.dart$|\.pb\.go$|_pb2\.py$|\.d\.ts$|\.map$)",
        "i",
    )
});
static TEST_PATH: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"(^|/)(__tests__|tests?|specs?|__mocks__|mocks?|fixtures?|e2e|testdata|androidTest|(jvm|common|ios|native|js|desktop|wasmJs)Test)(/|$)|\.(test|spec)\.[a-z]+$|_test\.(go|py|dart|exs|rb|cc|cpp)$|_spec\.rb$|(^|/)(test_[^/]*|conftest)\.py$",
        "i",
    )
});
/// `FooTest.java`, `FooTests.cs`, `FooIT.kt`: the capital letters matter, `Commit.java` is not a test.
static TEST_CLASS: LazyLock<Regex> =
    LazyLock::new(|| regex(r"[a-z0-9](Test|Tests|IT)\.(kt|java|cs|php|swift)$", ""));
static PROJECT_FILE: LazyLock<Regex> =
    LazyLock::new(|| regex(r"\.(csproj|fsproj|vbproj|sln)$", "i"));
static GENERATED_HEAD: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"@generated|DO NOT EDIT|auto-?generated|this file (?:was|is) generated",
        "i",
    )
});

fn to_posix(path: &str) -> String {
    if std::path::MAIN_SEPARATOR == '/' {
        path.to_owned()
    } else {
        path.replace(std::path::MAIN_SEPARATOR, "/")
    }
}

fn has_build_marker(dir: &Path) -> bool {
    if BUILD_MARKERS.iter().any(|m| dir.join(m).exists()) {
        return true;
    }
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries.flatten().any(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| PROJECT_FILE.find(n).is_some())
        })
    })
}

/// True when a repository-relative path lives in a dependency, build output, or hidden directory,
/// or is named like a generated file. Without a `root_dir` (a diff has none) `bin/` and `obj/`
/// cannot be told apart from real source and are kept.
#[must_use]
pub fn is_ignored_path(root_dir: Option<&Path>, relative_path: &str) -> bool {
    ignored(root_dir, relative_path, true)
}

/// `is_ignored_path`, with the generated-file-name rule optional: the analysis needs to know that
/// `index.d.ts` exists even though a review does not read it.
fn ignored(root_dir: Option<&Path>, relative_path: &str, by_name: bool) -> bool {
    let posix = to_posix(relative_path);
    let segments: Vec<&str> = posix.split('/').collect();
    let file_name = segments.last().copied().unwrap_or("");
    if by_name && GENERATED_FILE_NAME.find(file_name).is_some() {
        return true;
    }

    let mut parent = root_dir.map(Path::to_path_buf);
    for segment in &segments[..segments.len() - 1] {
        if segment.starts_with('.') && *segment != "." && *segment != ".." {
            return true;
        }
        if IGNORED_DIRS.contains(segment) {
            return true;
        }
        if let Some(dir) = &parent {
            if BUILD_OUTPUT_DIRS.contains(segment) && has_build_marker(dir) {
                return true;
            }
            parent = Some(dir.join(segment));
        }
    }
    false
}

/// UTF-16 length, the unit the TypeScript thresholds below were written in.
fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Content a person did not write: a generated-file header, or minified code.
#[must_use]
pub fn is_generated_content(content: &str) -> bool {
    // `content.split('\n', 8)` in JavaScript keeps the first eight pieces and drops the rest.
    let head: Vec<&str> = content.split('\n').take(8).collect();
    if GENERATED_HEAD.find(&head.join("\n")).is_some() {
        return true;
    }
    let length = utf16_len(content);
    if length < 2_000 {
        return false;
    }
    let lines: Vec<&str> = content.split('\n').collect();
    let longest = lines.iter().map(|l| utf16_len(l)).max().unwrap_or(0);
    #[allow(clippy::cast_precision_loss)] // lengths of a text file, far below 2^52
    let average = length as f64 / lines.len() as f64;
    longest > 1_000 || average > 300.0
}

/// Tests carry fake credentials and deliberately bad code by design.
#[must_use]
pub fn is_test_path(relative_path: &str) -> bool {
    let path = to_posix(relative_path);
    TEST_PATH.find(&path).is_some() || TEST_CLASS.find(&path).is_some()
}

/// The extensions the TypeScript engine scans (`SOURCE_EXTENSIONS` in `read-files.ts`). The parity
/// gates build their units with this list, so that Rust and TypeScript see the same files.
pub const PARITY_EXTENSIONS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "mjs", "cjs", "dart", "rs", "go", "py", "kt", "kts", "php", "cs",
    "java", "vue", "svelte",
];

/// The extensions a review of a workspace scans: the parity list, and the languages the engine
/// parses that the TypeScript engine never read (Ruby, Elixir, Swift, C++, `GDScript` and Astro).
/// Not `html` (no rule reads it) and not `.h` (no parser can tell C from C++ or Objective-C).
pub const SCAN_EXTENSIONS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "mjs", "cjs", "dart", "rs", "go", "py", "kt", "kts", "php", "cs",
    "java", "vue", "svelte", "rb", "ex", "exs", "swift", "cpp", "cc", "cxx", "hpp", "hh", "hxx",
    "gd", "astro",
];

/// Files git tracks or would track, so the repository's own `.gitignore` decides; `None` outside a
/// git repository. The arguments go to git as they are, never through a shell.
fn git_listed_files(root: &Path) -> Option<Vec<String>> {
    let out = std::process::Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .current_dir(root)
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    out.status.success().then(|| {
        String::from_utf8_lossy(&out.stdout)
            .split('\0')
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
            .collect()
    })
}

fn walk_files(root: &Path, dir: &Path, into: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || IGNORED_DIRS.contains(&name.as_str()) {
            continue;
        }
        let path = entry.path();
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            walk_files(root, &path, into);
        } else if let Ok(rel) = path.strip_prefix(root) {
            into.push(to_posix(&rel.to_string_lossy()));
        }
    }
}

/// Like `walk_files`, but it enters hidden directories (`.github`, `.cargo`): the files that say
/// how a project is configured live there.
fn walk_repo_files(root: &Path, dir: &Path, into: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == ".git" || IGNORED_DIRS.contains(&name.as_str()) {
            continue;
        }
        let path = entry.path();
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            walk_repo_files(root, &path, into);
        } else if let Ok(rel) = path.strip_prefix(root) {
            into.push(to_posix(&rel.to_string_lossy()));
        }
    }
}

/// The largest configuration or documentation file that is read (1 MiB): a bigger one is a data
/// file, a lockfile or a generated manual.
pub const MAX_REPO_FILE_BYTES: u64 = 1_048_576;

/// The files of the project that are read for what they say about its configuration rather than
/// reviewed as code (`detect::env_docs::doc_kind`): example dotenv files, READMEs, Dockerfiles,
/// compose files, CI workflows and package manager manifests, as repository-relative POSIX paths
/// in a stable order. Hidden directories count, dependency and build directories do not.
#[must_use]
pub fn list_repo_files(root: &Path) -> Vec<String> {
    let mut all = git_listed_files(root).unwrap_or_else(|| {
        let mut v = Vec::new();
        walk_repo_files(root, root, &mut v);
        v
    });
    all.sort_by(|a, b| debuggatha_core::js::utf16_cmp(a, b));
    all.dedup();
    all.retain(|p| {
        crate::detect::env_docs::doc_kind(p).is_some()
            && !to_posix(p)
                .split('/')
                .any(|segment| IGNORED_DIRS.contains(&segment))
            && std::fs::metadata(root.join(p))
                .is_ok_and(|m| m.is_file() && m.len() <= MAX_REPO_FILE_BYTES)
    });
    all
}

/// Every file in the project's own directories, including declaration, minified and map files that
/// a review skips by name: what exists, for resolving an import.
#[must_use]
pub fn list_tree_files(root: &Path) -> Vec<String> {
    let mut all = git_listed_files(root).unwrap_or_else(|| {
        let mut v = Vec::new();
        walk_files(root, root, &mut v);
        v
    });
    all.sort_by(|a, b| debuggatha_core::js::utf16_cmp(a, b));
    all.dedup();
    all.retain(|p| !ignored(Some(root), p, false) && root.join(p).is_file());
    all
}

/// Every file the project owns (not ignored, not hidden, not build output), whatever its
/// extension, as repository-relative POSIX paths in a stable order (by UTF-16 code unit). The
/// analysis needs them all to tell whether an import points at a file that exists.
#[must_use]
pub fn list_owned_files(root: &Path) -> Vec<String> {
    let mut all = git_listed_files(root).unwrap_or_else(|| {
        let mut v = Vec::new();
        walk_files(root, root, &mut v);
        v
    });
    all.sort_by(|a, b| debuggatha_core::js::utf16_cmp(a, b));
    all.dedup();
    all.retain(|p| !is_ignored_path(Some(root), p) && root.join(p).is_file());
    all
}

/// Every file the project owns whose extension is in `extensions`, as repository-relative POSIX
/// paths in a stable order (JavaScript's default sort: by UTF-16 code unit).
#[must_use]
pub fn list_scan_files(root: &Path, extensions: &[&str]) -> Vec<String> {
    let mut all = list_owned_files(root);
    all.retain(|p| {
        Path::new(p)
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| extensions.contains(&e))
    });
    all
}

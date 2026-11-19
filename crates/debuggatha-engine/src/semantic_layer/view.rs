//! What a model is allowed to read of a file, and how it is shown to it.

use std::collections::BTreeSet;
use std::path::Path;

/// One line of code shown to a model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeLine {
    /// The line number, 1-based.
    pub n: u32,
    /// The line's text.
    pub text: String,
}

/// The code a model is allowed to read of one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeView {
    /// The file.
    pub file: String,
    /// The lines.
    pub lines: Vec<CodeLine>,
}

/// Files that hold credentials by their name: never sent to a model, even a local one.
#[must_use]
pub fn is_secret_file(file: &str) -> bool {
    let name = file.rsplit('/').next().unwrap_or(file).to_lowercase();
    let dotenv = name.strip_prefix(".env").is_some_and(|rest| {
        rest.chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
    });
    let keyfile = name.rsplit_once('.').is_some_and(|(stem, ext)| {
        !stem.is_empty() && matches!(ext, "pem" | "key" | "p12" | "pfx" | "jks" | "keystore")
    });
    dotenv || keyfile || name == "config.env" || name.starts_with("id_rsa")
}

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '/' | '+' | '=' | '.')
}

/// Nothing that looks like a credential is sent to a model: a quoted run of 24 or more token
/// characters is replaced.
#[must_use]
pub fn scrub(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let quote = chars[i];
        if matches!(quote, '"' | '\'' | '`') {
            let run = chars[i + 1..]
                .iter()
                .take_while(|c| is_token_char(**c))
                .count();
            if run >= 24 && chars.get(i + 1 + run) == Some(&quote) {
                out.push(quote);
                out.push_str("[REDACTED]");
                out.push(quote);
                i += run + 2;
                continue;
            }
        }
        out.push(quote);
        i += 1;
    }
    out
}

/// The view as text for a prompt, one numbered line each: lines in `withheld` are hidden and `marked` is flagged.
#[must_use]
pub fn render_view(view: &CodeView, withheld: &BTreeSet<u32>, marked: Option<u32>) -> String {
    view.lines
        .iter()
        .map(|line| {
            let body = if withheld.contains(&line.n) {
                "[line withheld: may hold a secret]".to_owned()
            } else {
                scrub(&line.text)
            };
            let mark = if marked == Some(line.n) { '>' } else { ' ' };
            format!("{mark} {:>4} | {body}", line.n)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `radius` lines either side of a line; `None` when the file cannot be read inside the root.
#[must_use]
pub fn window_around(root: &Path, file: &str, line: u32, radius: u32) -> Option<CodeView> {
    let text = crate::diff::review::read_text(root, file).ok()?;
    let all: Vec<&str> = text.split('\n').collect();
    let from = line.saturating_sub(radius).max(1);
    let to = line
        .saturating_add(radius)
        .min(u32::try_from(all.len()).unwrap_or(u32::MAX));
    Some(CodeView {
        file: file.to_owned(),
        lines: (from..=to)
            .map(|n| CodeLine {
                n,
                text: all.get(n as usize - 1).copied().unwrap_or("").to_owned(),
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_files_are_recognised_by_name() {
        for secret in [
            ".env",
            ".env.local",
            "a/b/.env.production",
            "x/server.pem",
            "k.KEY",
            "config.env",
            "id_rsa.pub",
            "a.keystore",
        ] {
            assert!(is_secret_file(secret), "{secret}");
        }
        for fine in [
            "env.ts",
            "src/environment.rs",
            "key.rs",
            ".envrc-notes.md",
            "monkey.txt",
            "pem",
        ] {
            assert!(!is_secret_file(fine), "{fine}");
        }
    }

    #[test]
    fn a_quoted_long_token_is_replaced_and_the_rest_is_kept() {
        assert_eq!(
            scrub(r#"const k = "mock_secret_abcdefghijklmnopqrst"; const n = "short";"#),
            r#"const k = "[REDACTED]"; const n = "short";"#
        );
        assert_eq!(
            scrub("a 'abcdefghijklmnopqrstuvwxy"),
            "a 'abcdefghijklmnopqrstuvwxy"
        );
        assert_eq!(scrub("`aaaaaaaaaaaaaaaaaaaaaaaa`"), "`[REDACTED]`");
        assert_eq!(scrub("no quotes at all é"), "no quotes at all é");
    }

    #[test]
    fn withheld_lines_are_not_shown_and_the_flagged_line_is_marked() {
        let view = CodeView {
            file: "a.rs".into(),
            lines: vec![
                CodeLine {
                    n: 9,
                    text: "let a = 1;".into(),
                },
                CodeLine {
                    n: 10,
                    text: "let key = AKIA;".into(),
                },
            ],
        };
        let shown = render_view(&view, &BTreeSet::from([10]), Some(9));
        assert_eq!(
            shown,
            ">    9 | let a = 1;\n    10 | [line withheld: may hold a secret]"
        );
    }
}

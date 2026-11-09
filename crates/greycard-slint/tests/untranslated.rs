//! Every string the window shows is translatable: a scan of the Slint
//! files for a string literal bound to a shown property ([`SHOWN`])
//! outside `@tr(...)`. The few that are data, the same in every
//! language (glyphs, units, the program's name), are listed in
//! `ui/untranslated.txt`.
//!
//! A `Segmented`'s `options` are shown and also compared (its
//! `current`, its `selected`), so they are not simply wrapped: they
//! count as bare until the element also binds the shown words
//! separately (`labels:`), when the compared values may stay English.

use std::path::{Path, PathBuf};

/// The properties whose strings are shown.
const SHOWN: [&str; 15] = [
    "text",
    "words",
    "placeholder-text",
    "placeholder",
    "accessible-label",
    "accessible-description",
    "title",
    "label",
    "note",
    "hint",
    "tooltip",
    "tips",
    "empty",
    "options",
    "labels",
];

/// One bare string: where it is, what it says, and whether it is a
/// `Segmented`'s compared option.
#[derive(Debug, PartialEq)]
struct Bare {
    line: usize,
    text: String,
    option: bool,
}

/// The bare strings of one Slint file's `source`, not counting
/// `allowed` ones or the empty string.
fn bare_strings(source: &str, allowed: &[String]) -> Vec<Bare> {
    let code = without_comments(source);
    let bytes = code.as_bytes();
    let mut found = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'"' {
            // A string outside a binding, skipped whole.
            at = string_end(bytes, at);
            continue;
        }
        let Some(name) = SHOWN.iter().find(|name| binding_at(bytes, at, name)) else {
            at += 1;
            continue;
        };
        let option = *name == "options";
        if option && element_binds(&code, at, "labels") {
            at += name.len();
            continue;
        }
        at += name.len();
        // The binding's expression: to its `;`, to the `,` after a
        // struct literal's field, or to the `}` that closes the element,
        // at its own depth.
        let mut depth = 0i32;
        let mut tr_depths: Vec<i32> = Vec::new();
        while at < bytes.len() {
            match bytes[at] {
                b'"' => {
                    let end = string_end(bytes, at);
                    let text = &code[at + 1..end.saturating_sub(1).max(at + 1)];
                    if tr_depths.is_empty()
                        && !text.is_empty()
                        && !allowed.iter().any(|a| a == text)
                    {
                        found.push(Bare {
                            line: line_of(&code, at),
                            text: text.to_string(),
                            option,
                        });
                    }
                    at = end;
                    continue;
                }
                b'@' if code[at..].starts_with("@tr(") => {
                    depth += 1;
                    tr_depths.push(depth);
                    at += 4;
                    continue;
                }
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => {
                    if depth == 0 {
                        break;
                    }
                    if tr_depths.last() == Some(&depth) {
                        tr_depths.pop();
                    }
                    depth -= 1;
                }
                b';' | b',' if depth == 0 => break,
                _ => {}
            }
            at += 1;
        }
    }
    found
}

/// Whether the element around `at` (from the `{` that opens it to the
/// `}` that closes it) binds `name` anywhere in it.
fn element_binds(code: &str, at: usize, name: &str) -> bool {
    let bytes = code.as_bytes();
    let mut depth = 0i32;
    let mut start = at;
    while start > 0 {
        start -= 1;
        match bytes[start] {
            b'}' => depth += 1,
            b'{' if depth == 0 => break,
            b'{' => depth -= 1,
            _ => {}
        }
    }
    let mut end = at;
    depth = 0;
    while end < bytes.len() {
        match bytes[end] {
            b'{' => depth += 1,
            b'}' if depth == 0 => break,
            b'}' => depth -= 1,
            _ => {}
        }
        end += 1;
    }
    (start..end).any(|i| binding_at(bytes, i, name))
}

/// Whether `name:` (a binding, not `name <=>` or part of a longer
/// name) starts at `at`.
fn binding_at(bytes: &[u8], at: usize, name: &str) -> bool {
    let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'-' || b == b'_';
    if !bytes[at..].starts_with(name.as_bytes()) || (at > 0 && ident(bytes[at - 1])) {
        return false;
    }
    let after = &bytes[at + name.len()..];
    let skip = after
        .iter()
        .take_while(|b| **b == b' ' || **b == b'\t')
        .count();
    after.get(skip) == Some(&b':')
}

/// Where the string opening at `at` ends, one past its closing quote.
/// An interpolation (`\{...}`) may hold strings of its own.
fn string_end(bytes: &[u8], at: usize) -> usize {
    let mut i = at + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if bytes.get(i + 1) == Some(&b'{') => {
                let mut depth = 0;
                i += 1;
                while i < bytes.len() {
                    match bytes[i] {
                        b'{' => depth += 1,
                        b'}' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        b'"' => {
                            i = string_end(bytes, i);
                            continue;
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
            b'\\' => i += 1,
            b'"' => return i + 1,
            _ => {}
        }
        i += 1;
    }
    bytes.len()
}

/// `source` with its comments blanked, lines kept where they were.
fn without_comments(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = bytes.to_vec();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            i = string_end(bytes, i);
        } else if bytes[i..].starts_with(b"//") {
            while i < bytes.len() && bytes[i] != b'\n' {
                out[i] = b' ';
                i += 1;
            }
        } else if bytes[i..].starts_with(b"/*") {
            while i < bytes.len() && !bytes[i..].starts_with(b"*/") {
                if bytes[i] != b'\n' {
                    out[i] = b' ';
                }
                i += 1;
            }
            for b in out.iter_mut().skip(i).take(2) {
                *b = b' ';
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    // Only ASCII bytes were blanked, and whole comments, so the text is
    // still UTF-8.
    String::from_utf8(out).expect("blanking comments keeps UTF-8")
}

fn line_of(code: &str, at: usize) -> usize {
    code[..at].matches('\n').count() + 1
}

fn slint_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("the ui folder reads") {
        let path = entry.expect("an entry reads").path();
        if path.is_dir() {
            slint_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "slint") {
            out.push(path);
        }
    }
}

/// The allow-list: one string a line in double quotes, `#` lines and
/// blank ones aside.
fn allowed(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            l.strip_prefix('"')
                .and_then(|l| l.strip_suffix('"'))
                .unwrap_or_else(|| panic!("{l:?} in ui/untranslated.txt is not in double quotes"))
                .to_string()
        })
        .collect()
}

/// Every Slint file of the window: a string it shows is in `@tr`, or on
/// the allow-list. Ignored until the wrapping of the window's strings
/// lands, which un-ignores it.
#[test]
#[ignore = "the window's strings are wrapped in @tr next; that change un-ignores this"]
fn every_shown_string_is_translatable() {
    let ui = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui");
    let allowed = allowed(
        &std::fs::read_to_string(ui.join("untranslated.txt")).expect("the allow-list reads"),
    );
    let mut files = Vec::new();
    slint_files(&ui, &mut files);
    files.sort();
    let mut offenders = Vec::new();
    for file in &files {
        let source = std::fs::read_to_string(file).expect("a Slint file reads");
        let shown = file
            .strip_prefix(env!("CARGO_MANIFEST_DIR"))
            .unwrap_or(file);
        for bare in bare_strings(&source, &allowed) {
            let what = if bare.option {
                " (an option: shown and compared; bind the shown words as `labels`)"
            } else {
                ""
            };
            offenders.push(format!(
                "{}:{}: {:?}{what}",
                shown.display(),
                bare.line,
                bare.text
            ));
        }
    }
    assert!(
        offenders.is_empty(),
        "{} shown strings outside @tr (wrap them, or list a data string in ui/untranslated.txt):\n{}",
        offenders.len(),
        offenders.join("\n")
    );
}

/// The scan finds a bare string in each shown binding, by its line, and
/// lets through `@tr`, the allow-list, the empty string, comments, and
/// bindings that are not shown.
#[test]
fn the_scan_finds_bare_strings_by_line() {
    let source = r#"component A {
    Text { text: "Bare"; }
    Text { text: @tr("Wrapped"); }
    Text { text: @tr("{} of {}", a, b); }
    Text { text: root.on ? "Yes" : @tr("No"); }
    Text {
        text: root.on
            ? @tr("On")
            : "Off";
    }
    Text { text: ""; }
    Text { text: "greycard"; }
    // text: "In a comment";
    /* words: "In a block" */
    Rectangle { accessible-label: "Label"; }
    Rectangle { placeholder-text: @tr("Type here"); title: "Title"; }
    Rectangle { context-text: "Not shown"; font-family: "Noto Sans"; }
    property <string> subtitle: "Not this";
    Text { text: "\{root.n} frames"; }
    Text { text: @tr("Hello \{"x"}"); }
    in property <string> text: "Default";
    property <Row> row: { text: "Field", value: 1, key: "k" };
    Segmented { options: ["One", "Two"]; current <=> root.c; }
    Segmented { options: ["Kept"]; labels: [@tr("Kept")]; }
    Toggle { label: "Toggle"; note: @tr("Wrapped note"); }
}"#;
    let found = bare_strings(source, &["greycard".to_string()]);
    let got: Vec<(usize, &str, bool)> = found
        .iter()
        .map(|b| (b.line, b.text.as_str(), b.option))
        .collect();
    assert_eq!(
        got,
        [
            (2, "Bare", false),
            (5, "Yes", false),
            (9, "Off", false),
            (15, "Label", false),
            (16, "Title", false),
            (19, "\\{root.n} frames", false),
            (21, "Default", false),
            (22, "Field", false),
            (23, "One", true),
            (23, "Two", true),
            (25, "Toggle", false),
        ]
    );
}

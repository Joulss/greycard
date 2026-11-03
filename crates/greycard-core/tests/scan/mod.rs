//! The scan the guard tests share: which code under `crates/` is test
//! code. Test code is a crate's `tests/` folder, a file under an inner
//! test `cfg`, a module file declared under one (and the files below
//! it), and the body of any item under such a `cfg` or a `#[test]`. A
//! test `cfg` is one that is off whenever `test` is, on every platform.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Every `.rs` file under the crates, with each file's blanked code and
/// its test spans, and the set of files that are test code whole.
pub fn test_code(crates: &Path) -> TestCode {
    let mut files = Vec::new();
    rust_files(crates, &mut files);
    assert!(files.len() > 100, "found only {} files", files.len());

    let mut test_files: BTreeSet<PathBuf> = files
        .iter()
        .filter(|f| {
            f.strip_prefix(crates)
                .unwrap()
                .components()
                .nth(1)
                .is_some_and(|c| c.as_os_str() == "tests")
        })
        .cloned()
        .collect();
    let mut spans = std::collections::HashMap::new();
    for f in &files {
        let raw = std::fs::read_to_string(f).unwrap();
        let code = blank(&raw);
        let (s, modules, whole) =
            test_spans(&raw, &code).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        for (inline, name) in modules {
            test_files.insert(module_file(f, &inline, &name));
        }
        if whole {
            test_files.insert(f.clone());
        }
        spans.insert(f.clone(), (code, s));
    }
    // A module below a test module is test code too.
    loop {
        let mut more = Vec::new();
        for f in &test_files {
            for name in declared_modules(&spans[f].0) {
                let m = module_file(f, &[], &name);
                if !test_files.contains(&m) {
                    more.push(m);
                }
            }
        }
        if more.is_empty() {
            break;
        }
        test_files.extend(more);
    }
    TestCode {
        files,
        spans,
        test_files,
    }
}

pub struct TestCode {
    pub files: Vec<PathBuf>,
    pub spans: std::collections::HashMap<PathBuf, (String, Vec<std::ops::Range<usize>>)>,
    pub test_files: BTreeSet<PathBuf>,
}

/// Every `.rs` file under `dir`, past build output.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n != "target") {
                rust_files(&path, out);
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The source with its comments, strings, and character literals
/// blanked to spaces, its line breaks and byte offsets kept: what is
/// left is the code alone.
pub fn blank(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = b.to_vec();
    let mut i = 0;
    let clear = |out: &mut Vec<u8>, from: usize, to: usize| {
        for c in &mut out[from..to] {
            if *c != b'\n' {
                *c = b' ';
            }
        }
    };
    while i < b.len() {
        if b[i..].starts_with(b"//") {
            let end = b[i..]
                .iter()
                .position(|&c| c == b'\n')
                .map_or(b.len(), |e| i + e);
            clear(&mut out, i, end);
            i = end;
        } else if b[i..].starts_with(b"/*") {
            let (mut depth, mut j) = (1, i + 2);
            while j < b.len() && depth > 0 {
                if b[j..].starts_with(b"/*") {
                    depth += 1;
                    j += 2;
                } else if b[j..].starts_with(b"*/") {
                    depth -= 1;
                    j += 2;
                } else {
                    j += 1;
                }
            }
            clear(&mut out, i, j);
            i = j;
        } else if let Some(hashes) = raw_string_at(b, i) {
            let mut close = vec![b'"'];
            close.extend(std::iter::repeat_n(b'#', hashes));
            let from = i + 2 + hashes;
            let end = b[from..]
                .windows(close.len())
                .position(|w| w == close.as_slice())
                .map_or(b.len(), |e| from + e + close.len());
            clear(&mut out, i, end);
            i = end;
        } else if b[i] == b'"' {
            let mut j = i + 1;
            while j < b.len() && b[j] != b'"' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            clear(&mut out, i, (j + 1).min(b.len()));
            i = j + 1;
        } else if b[i] == b'\'' {
            // A character literal, or a lifetime or label: only the
            // literal closes within a few bytes.
            let end = if b.get(i + 1) == Some(&b'\\') {
                b[i + 2..]
                    .iter()
                    .position(|&c| c == b'\'')
                    .map(|e| i + 2 + e)
            } else {
                let len = std::str::from_utf8(&b[i + 1..(i + 5).min(b.len())])
                    .ok()
                    .or_else(|| std::str::from_utf8(&b[i + 1..(i + 2).min(b.len())]).ok())
                    .and_then(|s| s.chars().next())
                    .map_or(1, char::len_utf8);
                (b.get(i + 1 + len) == Some(&b'\'')).then_some(i + 1 + len)
            };
            match end {
                Some(e) => {
                    clear(&mut out, i, e + 1);
                    i = e + 1;
                }
                None => i += 1,
            }
        } else {
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// The number of `#` of a raw string that starts at `i` (its `r`,
/// after any `b` or `c`), or `None` when none starts there.
fn raw_string_at(b: &[u8], i: usize) -> Option<usize> {
    let before = i.checked_sub(1).map(|p| b[p]);
    let prefixed =
        matches!(before, Some(b'b' | b'c')) && i.checked_sub(2).is_none_or(|p| !is_ident(b[p]));
    if b[i] != b'r' || (before.is_some_and(is_ident) && !prefixed) {
        return None;
    }
    let hashes = b[i + 1..].iter().take_while(|&&c| c == b'#').count();
    (b.get(i + 1 + hashes) == Some(&b'"')).then_some(hashes)
}

/// What [`test_spans`] finds in a file: the byte ranges of the items
/// under a test attribute, the module files declared under one (each
/// with the inline modules it sits in), and whether the whole file is
/// test code by an inner attribute.
pub type Spans = (
    Vec<std::ops::Range<usize>>,
    Vec<(Vec<String>, String)>,
    bool,
);

/// The test code of a file, from its `raw` source and its blanked
/// `code`; an attribute the cfg reading cannot parse is the error.
pub fn test_spans(raw: &str, code: &str) -> Result<Spans, String> {
    let b = code.as_bytes();
    let (mut spans, mut modules, mut whole) = (Vec::new(), Vec::new(), false);
    let mut from = 0;
    while let Some(at) = code[from..].find('#').map(|a| from + a) {
        from = at + 1;
        let inner = b.get(at + 1) == Some(&b'!');
        let open = at + 1 + usize::from(inner);
        if b.get(open) != Some(&b'[') {
            continue;
        }
        let close = matching(b, open);
        from = close + 1;
        if !is_test_attribute(&raw[open + 1..close])? {
            continue;
        }
        if inner {
            whole = true;
            continue;
        }
        // The item the attribute stands on: its body, or a module file.
        let mut j = close + 1;
        while j < b.len() {
            match b[j] {
                b'[' | b'(' => j = matching(b, j) + 1,
                b'{' => {
                    let end = matching(b, j);
                    spans.push(at..end + 1);
                    // An inline module's own module files.
                    let item: Vec<&str> = words(&code[close + 1..j]).collect();
                    if let [.., "mod", name] = item.as_slice() {
                        for child in declared_modules(&code[j + 1..end]) {
                            modules.push((vec![name.to_string()], child));
                        }
                    }
                    break;
                }
                b';' => {
                    let item: Vec<&str> = words(&code[close + 1..j]).collect();
                    if let Some(m) = item.iter().position(|w| *w == "mod") {
                        modules.push((Vec::new(), item[m + 1].to_string()));
                    }
                    break;
                }
                // A field or a variant ends at its comma or at the
                // closing brace of what holds it: no body of its own.
                b',' | b'}' => break,
                _ => j += 1,
            }
        }
    }
    Ok((spans, modules, whole))
}

/// Whether an attribute's text (between its brackets) makes the item
/// test code: `test`, or a `cfg` that is false whenever `test` is,
/// whatever the platform. The test helpers' features, `testing` and
/// `fixtures`, are off with `test`; every other key may be either way.
pub fn is_test_attribute(attr: &str) -> Result<bool, String> {
    let attr = attr.trim();
    if attr == "test" {
        return Ok(true);
    }
    let Some(pred) = attr.strip_prefix("cfg").map(str::trim_start) else {
        return Ok(false);
    };
    if !pred.starts_with('(') {
        return Ok(false);
    }
    let tokens = cfg_tokens(&pred[1..pred.len() - 1])?;
    let without = Cfg::parse(&tokens, &mut 0)?.eval(false);
    let with = Cfg::parse(&tokens, &mut 0)?.eval(true);
    Ok(without == Some(false) && with != Some(false))
}

/// A cfg predicate.
enum Cfg {
    Key(String, Option<String>),
    All(Vec<Cfg>),
    Any(Vec<Cfg>),
    Not(Box<Cfg>),
}

impl Cfg {
    /// One predicate from `tokens` at `at`, which it moves past.
    fn parse(tokens: &[String], at: &mut usize) -> Result<Cfg, String> {
        let name = tokens.get(*at).ok_or("a cfg ends early")?.clone();
        *at += 1;
        match tokens.get(*at).map(String::as_str) {
            Some("=") => {
                let value = tokens.get(*at + 1).ok_or("a cfg value is missing")?;
                *at += 2;
                Ok(Cfg::Key(name, Some(value.trim_matches('"').to_string())))
            }
            Some("(") => {
                *at += 1;
                let mut args = Vec::new();
                while tokens.get(*at).map(String::as_str) != Some(")") {
                    args.push(Cfg::parse(tokens, at)?);
                    if tokens.get(*at).map(String::as_str) == Some(",") {
                        *at += 1;
                    }
                }
                *at += 1;
                match name.as_str() {
                    "all" => Ok(Cfg::All(args)),
                    "any" => Ok(Cfg::Any(args)),
                    "not" if args.len() == 1 => Ok(Cfg::Not(Box::new(args.remove(0)))),
                    _ => Err(format!("a cfg the scan cannot read: {name}(..)")),
                }
            }
            _ => Ok(Cfg::Key(name, None)),
        }
    }

    /// The predicate's value with `test` as given: `None` when it
    /// turns on a key that may be either way.
    fn eval(&self, test: bool) -> Option<bool> {
        match self {
            Cfg::Key(k, None) if k == "test" => Some(test),
            Cfg::Key(k, Some(v)) if k == "feature" && (v == "testing" || v == "fixtures") => {
                Some(test)
            }
            Cfg::Key(..) => None,
            Cfg::All(args) => args
                .iter()
                .try_fold(Some(true), |acc, a| match a.eval(test) {
                    Some(false) => Err(()),
                    v => Ok(if v.is_none() { None } else { acc }),
                })
                .unwrap_or(Some(false)),
            Cfg::Any(args) => args
                .iter()
                .try_fold(Some(false), |acc, a| match a.eval(test) {
                    Some(true) => Err(()),
                    v => Ok(if v.is_none() { None } else { acc }),
                })
                .unwrap_or(Some(true)),
            Cfg::Not(a) => a.eval(test).map(|v| !v),
        }
    }
}

/// A cfg's words, strings, and punctuation, one token each.
fn cfg_tokens(s: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c.is_alphanumeric() || c == '_' {
            let mut w = String::new();
            while let Some(&c) = chars.peek().filter(|c| c.is_alphanumeric() || **c == '_') {
                w.push(c);
                chars.next();
            }
            tokens.push(w);
        } else if c == '"' {
            let mut w = String::from(chars.next().unwrap());
            for c in chars.by_ref() {
                w.push(c);
                if c == '"' {
                    break;
                }
            }
            tokens.push(w);
        } else if "(),=".contains(c) {
            tokens.push(chars.next().unwrap().to_string());
        } else {
            return Err(format!("a cfg the scan cannot read: {s}"));
        }
    }
    Ok(tokens)
}

/// The names of the module files `code` declares, test or not.
fn declared_modules(code: &str) -> Vec<String> {
    code.split(';')
        .filter_map(|stmt| {
            let w: Vec<&str> = words(stmt).collect();
            let m = w.iter().rposition(|w| *w == "mod")?;
            (m + 2 == w.len()).then(|| w[m + 1].to_string())
        })
        .collect()
}

fn words(s: &str) -> impl Iterator<Item = &str> {
    s.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
}

/// The index of the bracket that closes the one at `open`.
pub fn matching(b: &[u8], open: usize) -> usize {
    let (o, c) = match b[open] {
        b'[' => (b'[', b']'),
        b'(' => (b'(', b')'),
        _ => (b'{', b'}'),
    };
    let mut depth = 0;
    for (k, &ch) in b.iter().enumerate().skip(open) {
        if ch == o {
            depth += 1;
        } else if ch == c {
            depth -= 1;
            if depth == 0 {
                return k;
            }
        }
    }
    b.len() - 1
}

/// The file of module `name` declared in `parent`, inside the inline
/// modules `inline`. A crate root (a target of its own) keeps its
/// modules beside it.
fn module_file(parent: &Path, inline: &[String], name: &str) -> PathBuf {
    let stem = parent.file_stem().unwrap();
    let up = parent.parent().unwrap();
    let target_dir = ["tests", "examples", "benches"]
        .iter()
        .any(|d| up.file_name().is_some_and(|n| n == *d) && up.join("../Cargo.toml").exists())
        || up.ends_with("src/bin");
    let dir = if target_dir || ["lib", "main", "mod", "build"].iter().any(|s| stem == *s) {
        parent.parent().unwrap().to_path_buf()
    } else {
        parent.with_extension("")
    };
    let dir = inline.iter().fold(dir, |d, m| d.join(m));
    let flat = dir.join(format!("{name}.rs"));
    let nested = dir.join(name).join("mod.rs");
    if flat.exists() {
        flat
    } else if nested.exists() {
        nested
    } else {
        panic!("module {name} of {} has no file", parent.display())
    }
}

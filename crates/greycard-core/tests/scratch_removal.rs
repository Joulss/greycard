//! Every test in the workspace removes its folders through
//! `greycard_core::testing::remove_dir_retry`, or the window's
//! `remove_scratch` over it, never with a bare `remove_dir_all`. Linux
//! removes a folder with the index's SQLite file still open under it,
//! Windows refuses, and a test that does so passes here and fails on
//! the Windows runner. The helper waits for what Windows would refuse
//! and names it; a bare removal skips that check.
//!
//! The scan (`scan/`) reads every `.rs` file under `crates/` and finds
//! its test code. A removal that must stay bare says why on its line or
//! the line above, in a comment that starts `remove_dir_all allowed:`.

mod scan;

use std::path::Path;

use scan::{TestCode, blank, is_test_attribute, test_code, test_spans};

const ALLOW: &str = "remove_dir_all allowed:";
/// A bare `fs::rename` in the window crate's test code is refused the
/// same way: Windows refuses to rename a file or a folder while the
/// thumbnail pool or the worker holds a file under it open, so a test
/// renames through `greycard_core::testing::rename_retry`, or says
/// why on the line above with this.
const RENAME_ALLOW: &str = "rename allowed:";

/// The `file:line` of every `needle` in test code under `crates`,
/// among the files `take` admits, not allowed by `allow` on its line
/// or the line above.
fn bare_in_tests(
    crates: &Path,
    code: &TestCode,
    needle: &str,
    allow: &str,
    take: impl Fn(&Path) -> bool,
) -> Vec<String> {
    let mut bare = Vec::new();
    for f in &code.files {
        if !take(f) {
            continue;
        }
        let raw = std::fs::read_to_string(f).unwrap();
        let raw: Vec<&str> = raw.lines().collect();
        let (blanked, s) = &code.spans[f];
        let whole = code.test_files.contains(f);
        let mut offset = 0;
        for (i, line) in blanked.split('\n').enumerate() {
            let at = line.find(needle).map(|c| offset + c);
            offset += line.len() + 1;
            let Some(at) = at else { continue };
            if !whole && !s.iter().any(|r| r.contains(&at)) {
                continue;
            }
            let allowed = raw[i].contains(allow) || (i > 0 && raw[i - 1].contains(allow));
            if !allowed {
                bare.push(format!(
                    "{}:{}",
                    f.strip_prefix(crates).unwrap().display(),
                    i + 1
                ));
            }
        }
    }
    bare
}

#[test]
fn no_test_removes_a_folder_bare() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let crates = crates.canonicalize().unwrap();
    let code = test_code(&crates);
    let bare = bare_in_tests(&crates, &code, "remove_dir_all", ALLOW, |_| true);
    assert!(
        bare.is_empty(),
        "a bare remove_dir_all in test code; use greycard_core::testing::remove_dir_retry \
         (or the window's testing::remove_scratch), or say why on the line above with \
         `// {ALLOW} <why>`:\n{}",
        bare.join("\n")
    );
}

#[test]
fn no_window_test_renames_bare() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let crates = crates.canonicalize().unwrap();
    let code = test_code(&crates);
    let bare = bare_in_tests(&crates, &code, "fs::rename(", RENAME_ALLOW, |f| {
        f.strip_prefix(&crates)
            .unwrap()
            .components()
            .next()
            .is_some_and(|c| c.as_os_str() == "greycard-ui")
    });
    assert!(
        bare.is_empty(),
        "a bare fs::rename in the window's test code, which Windows refuses while the pool \
         or the worker holds a file under it; use greycard_core::testing::rename_retry, or say why on \
         line above with `// {RENAME_ALLOW} <why>`:\n{}",
        bare.join("\n")
    );
}

#[test]
fn a_cfg_is_test_code_only_when_off_outside_tests() {
    let test = |a: &str| is_test_attribute(a).unwrap();
    assert!(test("test"));
    assert!(test("cfg(test)"));
    assert!(test("cfg(all(test, unix))"));
    assert!(test(r#"cfg(any(test, feature = "testing"))"#));
    assert!(test(r#"cfg(all(test, not(target_os = "windows")))"#));
    assert!(!test("cfg(not(test))"));
    assert!(!test(r#"cfg(not(any(test, feature = "testing")))"#));
    assert!(!test(r#"cfg(any(target_os = "linux", test))"#));
    assert!(!test("cfg(unix)"));
    assert!(!test("cfg_attr(test, allow(dead_code))"));
    assert!(!test("derive(Debug)"));
    assert!(is_test_attribute("cfg(foo(test))").is_err());
}

#[test]
fn an_inner_attribute_and_an_inline_module_s_files_are_read() {
    let src = "#![cfg(test)]\nfn a() {}\n#[cfg(test)]\nmod tests {\n    mod child;\n}\n";
    let (spans, modules, whole) = test_spans(src, &blank(src)).unwrap();
    assert!(whole);
    assert_eq!(spans.len(), 1);
    assert_eq!(
        modules,
        vec![(vec!["tests".to_string()], "child".to_string())]
    );
}

#[test]
fn a_test_attribute_on_a_field_or_a_variant_ends_at_its_comma() {
    let src = "struct S {\n    #[cfg(test)]\n    tamper: Option<u8>,\n}\nfn f() { let x = 1; }\n";
    let (spans, _, _) = test_spans(src, &blank(src)).unwrap();
    assert!(spans.is_empty(), "{spans:?}");
    let src = "enum E {\n    A,\n    #[cfg(test)]\n    B\n}\nfn f() { let x = 1; }\n";
    let (spans, _, _) = test_spans(src, &blank(src)).unwrap();
    assert!(spans.is_empty(), "{spans:?}");
}

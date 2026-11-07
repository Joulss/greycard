//! The layer rule, held: our crates' dependencies on one another, as
//! `cargo metadata` reads the manifests, against the list on the
//! architecture page, and the window naming none of core's develop ops
//! outside its tests.
//!
//! `EDGES` and `DEV_ONLY` are docs/architecture.md section 1, "The
//! crates and the layer rule", the table's "Depends on" column. A crate
//! that gains a dependency on another of ours fails here until the page
//! and this list both have it; an edge the list has and a manifest
//! dropped fails too, so the page stays the manifests' own.

mod scan;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Each crate's normal (and build) dependencies among ours, by the
/// page's short names: `core` is `greycard-core`.
const EDGES: &[(&str, &[&str])] = &[
    ("greycard-core", &[]),
    ("greycard-edit", &["core"]),
    ("greycard-edit-io", &["core", "edit"]),
    ("greycard-lens", &["core"]),
    ("greycard-gpu", &["core"]),
    ("greycard-ai", &["core"]),
    ("greycard-match", &["core"]),
    ("greycard-render", &["core", "edit"]),
    ("greycard-library", &["core", "edit", "edit-io"]),
    ("greycard-viewport", &["core", "edit", "gpu", "render"]),
    (
        "greycard-engine",
        &[
            "core", "edit", "edit-io", "lens", "gpu", "render", "ai", "match", "library",
        ],
    ),
    ("greycard-slint", &[]),
    (
        "greycard-ui",
        &[
            "core", "edit", "edit-io", "lens", "gpu", "render", "ai", "library", "match", "engine",
            "viewport", "slint",
        ],
    ),
    (
        "greycard-cli",
        &[
            "core", "edit", "edit-io", "lens", "gpu", "ai", "library", "engine",
        ],
    ),
    ("greycard-bench", &["core", "ai"]),
    ("ort-dylib", &[]),
    ("greycard-fuzz", &["core", "edit", "edit-io", "library"]),
];

/// The looser set: a dev-dependency may name any crate the crate's
/// normal dependencies name (most do, to switch on a `testing`
/// feature), and these besides, which only its tests use.
const DEV_ONLY: &[(&str, &str)] = &[("ort-dylib", "core")];

/// The window's sites that name core's develop outside a test, each to
/// move behind the engine or the edit; the list only shrinks. A file
/// under `crates/`, the path as it is written, and how many times. The
/// viewport's defringe dropper reads `MIN_RADIUS` and `MAX_RADIUS` and
/// calls `deviation_hue` and `nearer_window` through its one import.
const DEVELOP_IN_THE_WINDOW: &[(&str, &str, usize)] = &[(
    "greycard-ui/src/panel/viewport.rs",
    "greycard_core::develop::defringe",
    1,
)];

fn full(short: &str) -> String {
    if short == "ort-dylib" {
        short.to_string()
    } else {
        format!("greycard-{short}")
    }
}

/// One crate's dependencies on ours, by kind.
#[derive(Default)]
struct Deps {
    normal: BTreeSet<String>,
    dev: BTreeSet<String>,
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every crate of ours and its dependencies on ours: the workspace's
/// members and the fuzz workspace's one, read once.
fn crates() -> &'static BTreeMap<String, Deps> {
    static CRATES: OnceLock<BTreeMap<String, Deps>> = OnceLock::new();
    CRATES.get_or_init(|| {
        let root = root();
        let packages: Vec<serde_json::Value> =
            [root.join("Cargo.toml"), root.join("fuzz/Cargo.toml")]
                .iter()
                .flat_map(|m| packages(m))
                .collect();
        let ours: BTreeSet<&str> = packages
            .iter()
            .map(|p| p["name"].as_str().unwrap())
            .collect();
        let mut crates = BTreeMap::new();
        for package in &packages {
            let name = package["name"].as_str().unwrap().to_string();
            let deps: &mut Deps = crates.entry(name).or_default();
            // An edge is one to a package of ours by name, whatever its
            // source: a path, a registry, or a git dependency.
            for dep in package["dependencies"].as_array().unwrap() {
                let on = dep["name"].as_str().unwrap();
                if !ours.contains(on) {
                    continue;
                }
                match dep["kind"].as_str() {
                    Some("dev") => deps.dev.insert(on.to_string()),
                    _ => deps.normal.insert(on.to_string()),
                };
            }
        }
        crates
    })
}

/// The packages `cargo metadata` lists for `manifest`'s workspace.
fn packages(manifest: &Path) -> Vec<serde_json::Value> {
    let out = std::process::Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .arg("--manifest-path")
        .arg(manifest)
        .output()
        .expect("run cargo metadata");
    assert!(
        out.status.success(),
        "cargo metadata on {}: {}",
        manifest.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    let mut meta: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    match meta["packages"].take() {
        serde_json::Value::Array(packages) => packages,
        _ => panic!("cargo metadata on {}: no packages", manifest.display()),
    }
}

#[test]
fn the_crates_depend_on_what_the_architecture_page_lists() {
    let crates = crates();
    let listed: BTreeMap<String, BTreeSet<String>> = EDGES
        .iter()
        .map(|(c, on)| (c.to_string(), on.iter().map(|s| full(s)).collect()))
        .collect();
    let mut wrong = Vec::new();
    for name in crates.keys().filter(|c| !listed.contains_key(*c)) {
        wrong.push(format!("{name} is a crate the page does not list"));
    }
    for (name, on) in &listed {
        let Some(deps) = crates.get(name) else {
            wrong.push(format!("the page lists {name}, which is no crate"));
            continue;
        };
        for new in deps.normal.difference(on) {
            wrong.push(format!(
                "{name} depends on {new}, which the page does not list"
            ));
        }
        for gone in on.difference(&deps.normal) {
            wrong.push(format!(
                "the page lists {name} on {gone}, which its Cargo.toml no longer names"
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "docs/architecture.md section 1 and the manifests disagree; change the one that is \
         wrong, and EDGES here with the page:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn the_crates_tests_depend_on_what_the_list_allows() {
    let crates = crates();
    let mut wrong = Vec::new();
    for (name, deps) in crates {
        let extra: BTreeSet<String> = DEV_ONLY
            .iter()
            .filter(|(c, _)| c == name)
            .map(|(_, on)| full(on))
            .collect();
        for new in deps.dev.difference(&deps.normal) {
            if !extra.contains(new) {
                wrong.push(format!(
                    "{name}'s tests depend on {new}, which DEV_ONLY does not list"
                ));
            }
        }
        for gone in extra.difference(&deps.dev) {
            wrong.push(format!(
                "DEV_ONLY lists {name}'s tests on {gone}, which its Cargo.toml no longer names"
            ));
        }
    }
    for (name, _) in DEV_ONLY {
        if !crates.contains_key(*name) {
            wrong.push(format!("DEV_ONLY lists {name}, which is no crate"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The names a path through core's root reaches develop by: `develop`
/// itself, and every name `lib` (core's lib.rs) re-exports from it.
fn develop_names(lib: &str) -> BTreeSet<String> {
    let code = scan::blank(lib);
    let mut names = BTreeSet::from(["develop".to_string()]);
    for stmt in code.split(';') {
        let Some(at) = stmt.find("pub use develop::") else {
            continue;
        };
        let items = stmt[at + "pub use develop::".len()..].trim();
        let items = items
            .strip_prefix('{')
            .and_then(|i| i.strip_suffix('}'))
            .unwrap_or(items);
        for item in items.split(',').map(str::trim).filter(|i| !i.is_empty()) {
            assert!(
                item != "*" && !item.contains('{'),
                "core re-exports `develop::{item}`, which the scan cannot follow"
            );
            let name = match item.split_once(" as ") {
                Some((_, alias)) => alias.trim(),
                None => item.rsplit("::").next().unwrap(),
            };
            names.insert(name.to_string());
        }
    }
    names
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// The offsets of `word` in `code` as a word of its own.
fn word_at<'a>(code: &'a str, word: &'a str) -> impl Iterator<Item = usize> + 'a {
    let b = code.as_bytes();
    code.match_indices(word)
        .map(|(at, _)| at)
        .filter(move |&at| {
            let end = at + word.len();
            (at == 0 || !is_ident(b[at - 1])) && b.get(end).is_none_or(|&c| !is_ident(c))
        })
}

fn skip_space(code: &str, at: usize) -> usize {
    at + code[at..].len() - code[at..].trim_start().len()
}

/// Every place `code` (blanked) reaches core's develop: a path through
/// `develop` or a name in `names`, a `use greycard_core::{..}` that
/// takes one at its top level, a glob of core's root, and the same
/// through an alias (`use greycard_core as gc`). Each with its byte
/// offset and the text as written, on one line.
fn develop_paths(code: &str, names: &BTreeSet<String>) -> Vec<(usize, String)> {
    let b = code.as_bytes();
    let mut roots = vec!["greycard_core".to_string()];
    for at in word_at(code, "greycard_core") {
        let after = skip_space(code, at + "greycard_core".len());
        if let Some(rest) = code[after..].strip_prefix("as")
            && rest.starts_with(char::is_whitespace)
        {
            let alias = skip_space(code, after + 2);
            let end = code[alias..]
                .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                .map_or(code.len(), |e| alias + e);
            if end > alias && &code[alias..end] != "_" {
                roots.push(code[alias..end].to_string());
            }
        }
    }
    roots.sort();
    roots.dedup();
    let compact = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut found = Vec::new();
    for root in &roots {
        for at in word_at(code, root) {
            let mut j = skip_space(code, at + root.len());
            if !code[j..].starts_with("::") {
                continue;
            }
            j = skip_space(code, j + 2);
            match b.get(j) {
                Some(b'*') => found.push((at, format!("{root}::*"))),
                Some(b'{') => {
                    let close = scan::matching(b, j);
                    let group = &code[j + 1..close];
                    let mut depth = 0;
                    let mut from = 0;
                    let mut items = Vec::new();
                    for (k, c) in group.bytes().enumerate() {
                        match c {
                            b'{' => depth += 1,
                            b'}' => depth -= 1,
                            b',' if depth == 0 => {
                                items.push(&group[from..k]);
                                from = k + 1;
                            }
                            _ => {}
                        }
                    }
                    items.push(&group[from..]);
                    for item in items.into_iter().map(str::trim) {
                        let first = item
                            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                            .next()
                            .unwrap_or("");
                        if item == "*" || names.contains(first) {
                            found.push((at, format!("{root}::{{{}}}", compact(item))));
                        }
                    }
                }
                _ => {
                    let end = code[j..]
                        .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == ':'))
                        .map_or(code.len(), |e| j + e);
                    let path = code[j..end].trim_end_matches(':');
                    let first = path.split("::").next().unwrap_or("");
                    if names.contains(first) {
                        found.push((at, format!("{root}::{path}")));
                    }
                }
            }
        }
    }
    found.sort();
    found
}

#[test]
fn the_window_names_no_develop_op_outside_its_tests() {
    let lib = std::fs::read_to_string(root().join("crates/greycard-core/src/lib.rs")).unwrap();
    let names = develop_names(&lib);
    assert!(
        names.len() > 1,
        "found nothing core's lib.rs re-exports from develop"
    );
    let crates_dir = root().join("crates");
    let code = scan::test_code(&crates_dir);
    let in_window = |f: &Path| {
        let rel = f.strip_prefix(&crates_dir).unwrap();
        rel.starts_with("greycard-ui/src") || rel.starts_with("greycard-slint")
    };
    let mut found: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
    let mut in_tests = 0;
    for f in code.files.iter().filter(|f| in_window(f)) {
        let (blanked, spans) = &code.spans[f];
        let whole = code.test_files.contains(f);
        let rel = f.strip_prefix(&crates_dir).unwrap();
        let rel = rel.to_string_lossy().replace('\\', "/");
        for (at, path) in develop_paths(blanked, &names) {
            if whole || spans.iter().any(|r| r.contains(&at)) {
                in_tests += 1;
                continue;
            }
            let line = blanked[..at].matches('\n').count() + 1;
            found.entry((rel.clone(), path)).or_default().push(line);
        }
    }
    // The window's tests do name ops (crop's and the browser's
    // orientation tests, a develop in sync's and the viewport's); a scan
    // that saw none would be blind to them.
    assert!(
        in_tests > 0,
        "the scan found no develop op in the window's tests"
    );
    let known: BTreeMap<(String, String), usize> = DEVELOP_IN_THE_WINDOW
        .iter()
        .map(|(f, p, n)| ((f.to_string(), p.to_string()), *n))
        .collect();
    let mut wrong = Vec::new();
    for ((f, p), lines) in &found {
        let listed = known.get(&(f.clone(), p.clone())).copied().unwrap_or(0);
        if lines.len() > listed {
            let at: Vec<String> = lines.iter().map(|l| format!("{f}:{l}")).collect();
            wrong.push(format!(
                "{p} outside a test at {} sites where DEVELOP_IN_THE_WINDOW allows {listed} \
                 ({}); call the engine (or the edit) instead",
                lines.len(),
                at.join(", ")
            ));
        }
    }
    for ((f, p), n) in &known {
        let has = found.get(&(f.clone(), p.clone())).map_or(0, Vec::len);
        if has < *n {
            wrong.push(format!(
                "DEVELOP_IN_THE_WINDOW allows {f}: {p} {n} times, the window has it {has}; \
                 lower the count"
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "the window and core's develop:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn core_s_re_exports_from_develop_are_read() {
    let lib = "pub mod develop;\n\
               // pub use develop::Commented;\n\
               pub use develop::{\n    Alpha, beta as gamma,\n    sub::Delta,\n};\n\
               pub use develop::Epsilon;\n\
               pub use output::Other;\n";
    let names: Vec<String> = develop_names(lib).into_iter().collect();
    assert_eq!(
        names,
        ["Alpha", "Delta", "Epsilon", "develop", "gamma"].map(String::from)
    );
}

#[test]
fn a_develop_path_is_found_in_code_and_not_in_a_comment_or_a_string() {
    let names: BTreeSet<String> = ["develop", "DemosaicMethod", "DevelopSettings", "demosaic"]
        .map(String::from)
        .into();
    let cases: &[(&str, &[&str])] = &[
        (
            "use greycard_core::develop::orient;",
            &["greycard_core::develop::orient"],
        ),
        ("// greycard_core::develop::x", &[]),
        ("let s = \"greycard_core::develop::y\";", &[]),
        ("use greycard_core::developer;", &[]),
        ("use greycard_core::{decode::Decoder};", &[]),
        (
            "use greycard_core::{decode, develop};",
            &["greycard_core::{develop}"],
        ),
        (
            "use greycard_core::{decode::Decoder, develop::orient};",
            &["greycard_core::{develop::orient}"],
        ),
        (
            "use greycard_core::{decode::{Decoder, Source}, develop::orient};",
            &["greycard_core::{develop::orient}"],
        ),
        (
            "use greycard_core::{\n    develop::{\n        orient,\n    },\n};",
            &["greycard_core::{develop::{ orient, }}"],
        ),
        (
            "let m = greycard_core::develop::DemosaicMethod::Bilinear;",
            &["greycard_core::develop::DemosaicMethod::Bilinear"],
        ),
        (
            "let m = greycard_core::DemosaicMethod::Bilinear;",
            &["greycard_core::DemosaicMethod::Bilinear"],
        ),
        (
            "let d = greycard_core::demosaic(&raw, m);",
            &["greycard_core::demosaic"],
        ),
        (
            "use greycard_core::{Profile, DevelopSettings};",
            &["greycard_core::{DevelopSettings}"],
        ),
        ("use greycard_core::*;", &["greycard_core::*"]),
        ("use greycard_core::{decode, *};", &["greycard_core::{*}"]),
        (
            "use greycard_core as gc; fn f() { gc::develop::orient(); }",
            &["gc::develop::orient"],
        ),
        (
            "use ::greycard_core::develop::orient;",
            &["greycard_core::develop::orient"],
        ),
        (
            "macro_rules! m { () => { greycard_core::develop::orient() } }",
            &["greycard_core::develop::orient"],
        ),
        (
            "let my_greycard_core = 1; my_greycard_core::develop();",
            &[],
        ),
    ];
    for (src, want) in cases {
        let found: Vec<String> = develop_paths(&scan::blank(src), &names)
            .into_iter()
            .map(|(_, p)| p)
            .collect();
        assert_eq!(found, want.to_vec(), "{src}");
    }
}

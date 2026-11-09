//! The window's words in the user's language, on the Rust side: the
//! strings set from Rust (status words, menus, sheets, errors shown)
//! go through [`tr!`](crate::tr). The Slint side's `@tr` reads the same
//! catalogs, `translations/<lang>/LC_MESSAGES/greycard.po`, bundled by
//! greycard-slint's build script; this side's build script reads them
//! into static tables, so no catalog is a file at runtime and no
//! gettext library is linked.
//!
//! English is the source language: a string with no catalog, or none
//! in the chosen one, is shown as written. Logs stay English, and so
//! does anything compared or stored; only what is shown is translated.
//! A test sets no language, so the tests read English.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicUsize, Ordering};

mod plural;
// Read by the build script, which includes it by its path, and by the
// tests' own catalogs.
#[cfg(test)]
mod po;

/// A language's catalog as the build script writes it: its tag (the
/// folder's name), its `Plural-Forms` header, and its translated
/// messages, sorted by their English.
struct Table {
    tag: &'static str,
    plural_forms: &'static str,
    entries: &'static [(&'static str, &'static [&'static str])],
}

// `static TABLES: &[Table]`, one a catalog, sorted by tag.
include!(concat!(env!("OUT_DIR"), "/catalogs.rs"));

/// One language's translations, read for lookup.
struct Catalog {
    tag: &'static str,
    forms: usize,
    rule: plural::Expr,
    messages: HashMap<String, Vec<String>>,
}

impl Catalog {
    fn from_table(table: &Table) -> Self {
        // The build script read the rule already; a rule it took and
        // this does not is the plural English's.
        let (forms, rule) = plural::forms(table.plural_forms)
            .unwrap_or_else(|_| (2, plural::parse("n != 1").expect("English's rule reads")));
        Self {
            tag: table.tag,
            forms,
            rule,
            messages: table
                .entries
                .iter()
                .map(|(id, strs)| (id.to_string(), strs.iter().map(|s| s.to_string()).collect()))
                .collect(),
        }
    }

    /// A catalog from a `.po` file's text, as the build script reads one.
    #[cfg(test)]
    fn from_po(tag: &'static str, text: &str) -> Self {
        let po = po::parse(text).expect("the test's catalog reads");
        let (forms, rule) = plural::forms(
            po.field("Plural-Forms")
                .unwrap_or("nplurals=2; plural=(n != 1);"),
        )
        .expect("the test's rule reads");
        Self {
            tag,
            forms,
            rule,
            messages: po
                .entries
                .iter()
                .filter(|e| e.translated())
                .map(|e| (e.key(), e.strs.clone()))
                .collect(),
        }
    }

    /// `msg` in this language, or as written when it has no translation.
    fn text<'a>(&'a self, msg: &'a str) -> &'a str {
        self.messages
            .get(msg)
            .and_then(|s| s.first())
            .map_or(msg, String::as_str)
    }

    /// The form for a count of `n`, by the catalog's rule, or English's
    /// two forms when the message has no translation.
    fn plural<'a>(&'a self, one: &'a str, many: &'a str, n: u64) -> &'a str {
        let Some(strs) = self.messages.get(one) else {
            return english_plural(one, many, n);
        };
        let index = usize::try_from(self.rule.eval(n)).unwrap_or(0);
        strs.get(index.min(self.forms.saturating_sub(1)))
            .or_else(|| strs.last())
            .map_or_else(|| english_plural(one, many, n), String::as_str)
    }
}

fn english_plural<'a>(one: &'a str, many: &'a str, n: u64) -> &'a str {
    if n == 1 { one } else { many }
}

static CATALOGS: LazyLock<Vec<Catalog>> =
    LazyLock::new(|| TABLES.iter().map(Catalog::from_table).collect());

/// The language in force: 0 for English, else one past its catalog's
/// place in [`CATALOGS`]. Set once, at the start.
static CURRENT: AtomicUsize = AtomicUsize::new(0);

fn current() -> Option<&'static Catalog> {
    CURRENT
        .load(Ordering::Relaxed)
        .checked_sub(1)
        .and_then(|i| CATALOGS.get(i))
}

/// The languages the build carries a catalog of: each tag, as its
/// folder is named, and the language's name in itself.
pub fn languages() -> Vec<(&'static str, &'static str)> {
    TABLES.iter().map(|t| (t.tag, own_name(t.tag))).collect()
}

/// A language's name in itself, for the Settings sheet's list; the tag
/// for one not in the table.
pub fn own_name(tag: &'static str) -> &'static str {
    const NAMES: [(&str, &str); 26] = [
        ("ca", "Català"),
        ("cs", "Čeština"),
        ("da", "Dansk"),
        ("de", "Deutsch"),
        ("el", "Ελληνικά"),
        ("es", "Español"),
        ("fi", "Suomi"),
        ("fr", "Français"),
        ("hu", "Magyar"),
        ("it", "Italiano"),
        ("ja", "日本語"),
        ("ko", "한국어"),
        ("nb", "Norsk bokmål"),
        ("nl", "Nederlands"),
        ("pl", "Polski"),
        ("pt", "Português"),
        ("pt_BR", "Português (Brasil)"),
        ("ro", "Română"),
        ("ru", "Русский"),
        ("sk", "Slovenčina"),
        ("sv", "Svenska"),
        ("tr", "Türkçe"),
        ("uk", "Українська"),
        ("vi", "Tiếng Việt"),
        ("zh_CN", "简体中文"),
        ("zh_TW", "繁體中文"),
    ];
    NAMES
        .iter()
        .find(|(t, _)| t.eq_ignore_ascii_case(&tag.replace('-', "_")))
        .map_or(tag, |(_, name)| name)
}

/// The catalog's tag for a language asked for (a setting, or the
/// system's locale such as `fr-CA` or `fr_FR.UTF-8`): the catalog of
/// the whole tag, else of its language part; empty for English, which
/// is also what a language with no catalog gets.
pub fn pick(wanted: &str) -> &'static str {
    pick_among(&TABLES.iter().map(|t| t.tag).collect::<Vec<_>>(), wanted).unwrap_or("")
}

fn pick_among<'a>(tags: &[&'a str], wanted: &str) -> Option<&'a str> {
    // The encoding and the modifier are no part of the language.
    let wanted = wanted
        .split(['.', '@'])
        .next()
        .unwrap_or("")
        .replace('-', "_");
    let mut parts: Vec<&str> = wanted.split('_').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return None;
    }
    // A script (`zh_Hant`, `sr_Latn_RS`) is no catalog's: dropped, and
    // Chinese's two written forms taken as their usual regions when no
    // region is given.
    if let Some(at) = parts
        .iter()
        .skip(1)
        .position(|p| p.len() == 4 && p.chars().all(|c| c.is_ascii_alphabetic()))
    {
        let script = parts.remove(at + 1).to_ascii_lowercase();
        if parts.len() == 1 && parts[0].eq_ignore_ascii_case("zh") {
            match script.as_str() {
                "hant" => parts.push("TW"),
                "hans" => parts.push("CN"),
                _ => {}
            }
        }
    }
    let wanted = parts.join("_");
    let base = |t: &str| t.split('_').next().unwrap_or("").to_ascii_lowercase();
    tags.iter()
        .find(|t| t.replace('-', "_").eq_ignore_ascii_case(&wanted))
        .or_else(|| tags.iter().find(|t| base(t) == base(&wanted)))
        .copied()
}

/// The catalog for the system's languages, most preferred first: the
/// first that has one, else English.
pub fn system() -> &'static str {
    sys_locale::get_locales()
        .map(|l| pick(&l))
        .find(|t| !t.is_empty())
        .unwrap_or("")
}

/// Put the language of `tag` (from [`pick`]) in force; empty, or a tag
/// with no catalog, is English.
pub fn select(tag: &str) {
    let index = CATALOGS
        .iter()
        .position(|c| c.tag == tag)
        .map_or(0, |i| i + 1);
    CURRENT.store(index, Ordering::Relaxed);
}

/// The tag in force, empty for English.
pub fn selected() -> &'static str {
    current().map_or("", |c| c.tag)
}

/// One value a [`tr!`](crate::tr) string is filled with: by its place
/// (`{}`, `{0}`) or by its name (`{name}`).
pub struct Arg<'a> {
    pub name: Option<&'static str>,
    pub value: &'a dyn Display,
}

/// [`tr!`](crate::tr) with a plain string.
pub fn text(msg: &str) -> String {
    // Through the fill, so `{{` is a brace with values or without.
    fill(current().map_or(msg, |c| c.text(msg)), &[])
}

/// [`tr!`](crate::tr) with values.
pub fn text_with(msg: &str, args: &[Arg]) -> String {
    fill(current().map_or(msg, |c| c.text(msg)), args)
}

/// [`tr!`](crate::tr) with a count.
pub fn plural(one: &str, many: &str, n: u64, args: &[Arg]) -> String {
    let form = current().map_or_else(|| english_plural(one, many, n), |c| c.plural(one, many, n));
    fill(form, args)
}

/// A count for [`plural`]: any integer, a negative one as zero.
pub fn count<T: TryInto<u64>>(n: T) -> u64 {
    n.try_into().unwrap_or(0)
}

/// `template` with its placeholders filled: `{}` takes the next value
/// by place, `{0}` the first, `{name}` the one so named; `{{` and `}}`
/// are braces. A placeholder with no value is left as written, so a
/// translation's slip shows rather than panics.
pub fn fill(template: &str, args: &[Arg]) -> String {
    let positional: Vec<&Arg> = args.iter().filter(|a| a.name.is_none()).collect();
    let mut next = 0;
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(at) = rest.find(['{', '}']) {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        if tail.starts_with("{{") || tail.starts_with("}}") {
            out.push_str(&tail[..1]);
            rest = &tail[2..];
            continue;
        }
        let close = match tail.strip_prefix('{').and_then(|t| t.find('}')) {
            Some(close) => close + 1,
            None => {
                out.push_str(&tail[..1]);
                rest = &tail[1..];
                continue;
            }
        };
        let name = &tail[1..close];
        let arg = if name.is_empty() {
            next += 1;
            positional.get(next - 1).copied()
        } else if let Ok(i) = name.parse::<usize>() {
            positional.get(i).copied()
        } else {
            args.iter().find(|a| a.name == Some(name))
        };
        match arg {
            Some(arg) => out.push_str(&arg.value.to_string()),
            None => out.push_str(&tail[..=close]),
        }
        rest = &tail[close + 1..];
    }
    out.push_str(rest);
    out
}

/// A string shown in the window, in the user's language: the `tr`
/// crate's shape, which `xtr` reads for the catalog.
///
/// - `tr!("Done")`
/// - `tr!("Exporting {} of {}", done, total)`, or with names a
///   translator may put in another order:
///   `tr!("Exporting {done} of {total}", done = d, total = t)`
/// - `tr!("{n} frame" | "{n} frames" % count)`, the form chosen by the
///   catalog's plural rule, `{n}` the count; values may follow.
#[macro_export]
macro_rules! tr {
    ($one:literal | $many:literal % $n:expr $(,)?) => {{
        let n = $n;
        $crate::i18n::plural(
            $one,
            $many,
            $crate::i18n::count(n),
            &[$crate::i18n::Arg { name: Some("n"), value: &n }],
        )
    }};
    ($one:literal | $many:literal % $n:expr, $($rest:tt)+) => {{
        let n = $n;
        $crate::i18n::plural(
            $one,
            $many,
            $crate::i18n::count(n),
            &$crate::__tr_args!([$crate::i18n::Arg { name: Some("n"), value: &n },] $($rest)+),
        )
    }};
    ($msg:literal $(,)?) => {
        $crate::i18n::text($msg)
    };
    ($msg:literal, $($rest:tt)+) => {
        $crate::i18n::text_with($msg, &$crate::__tr_args!([] $($rest)+))
    };
}

/// [`tr!`]'s values, named or by place, gathered into an array.
#[doc(hidden)]
#[macro_export]
macro_rules! __tr_args {
    ([$($done:tt)*]) => { [$($done)*] };
    ([$($done:tt)*] ,) => { [$($done)*] };
    ([$($done:tt)*] $name:ident = $value:expr $(, $($rest:tt)*)?) => {
        $crate::__tr_args!(
            [$($done)* $crate::i18n::Arg { name: Some(stringify!($name)), value: &$value },]
            $($($rest)*)?
        )
    };
    ([$($done:tt)*] $value:expr $(, $($rest:tt)*)?) => {
        $crate::__tr_args!(
            [$($done)* $crate::i18n::Arg { name: None, value: &$value },]
            $($($rest)*)?
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRENCH: &str = r#"
# A test's catalog.
msgid ""
msgstr ""
"Language: fr\n"
"Plural-Forms: nplurals=2; plural=(n > 1);\n"

msgid "Done"
msgstr "Terminé"

#, fuzzy
msgid "Cancel"
msgstr "Annuler"

msgid "Untranslated"
msgstr ""

msgid "Exporting {done} of {total}"
msgstr "Sur {total}, {done} exportées"

msgid "{} of {}"
msgstr "{1} sur {0}"

msgid "{n} frame"
msgid_plural "{n} frames"
msgstr[0] "{n} vue"
msgstr[1] "{n} vues"

msgctxt "menu"
msgid "Done"
msgstr "Fini"

msgid "A long "
"line"
msgstr "Une longue "
"ligne \"citée\""

msgid "Escapes"
msgstr "\a\b\f\v\101\x42\t"
"#;

    /// A message the catalog translates is its translation; one it does
    /// not, one left empty, and one marked fuzzy are the English.
    #[test]
    fn a_catalog_finds_its_translations_and_falls_back_to_english() {
        let fr = Catalog::from_po("fr", FRENCH);
        assert_eq!(fr.text("Done"), "Terminé");
        assert_eq!(fr.text("Cancel"), "Cancel");
        assert_eq!(fr.text("Untranslated"), "Untranslated");
        assert_eq!(fr.text("Not in it"), "Not in it");
        assert_eq!(fr.text("A long line"), "Une longue ligne \"citée\"");
        assert_eq!(fr.text("menu\u{4}Done"), "Fini");
        assert_eq!(fr.text("Escapes"), "\u{7}\u{8}\u{c}\u{b}AB\t");
        // A byte-order mark before the header is no part of it.
        let marked = Catalog::from_po("fr", &format!("\u{feff}{FRENCH}"));
        assert_eq!(marked.text("Done"), "Terminé");
        assert_eq!(marked.forms, 2);
    }

    /// A translation may put the values in another order, by name or by
    /// place, and the English fills them as written.
    #[test]
    fn placeholders_follow_the_translation() {
        let fr = Catalog::from_po("fr", FRENCH);
        let (done, total) = (3, 12);
        let named = [
            Arg {
                name: Some("done"),
                value: &done,
            },
            Arg {
                name: Some("total"),
                value: &total,
            },
        ];
        assert_eq!(
            fill(fr.text("Exporting {done} of {total}"), &named),
            "Sur 12, 3 exportées"
        );
        assert_eq!(
            fill("Exporting {done} of {total}", &named),
            "Exporting 3 of 12"
        );
        let placed = [
            Arg {
                name: None,
                value: &done,
            },
            Arg {
                name: None,
                value: &total,
            },
        ];
        assert_eq!(fill(fr.text("{} of {}"), &placed), "12 sur 3");
        assert_eq!(fill("{} of {}", &placed), "3 of 12");
        // Braces doubled are braces; a placeholder with no value is
        // left as written.
        assert_eq!(fill("{{{}}} {missing} {", &placed), "{3} {missing} {");
    }

    /// French's rule takes 0 and 1 as singular, English's 1 alone; a
    /// message with no translation uses English's two forms.
    #[test]
    fn the_plural_form_is_the_catalogs_rule() {
        let fr = Catalog::from_po("fr", FRENCH);
        let form = |n| {
            let n_arg = [Arg {
                name: Some("n"),
                value: &n,
            }];
            (
                fill(fr.plural("{n} frame", "{n} frames", n), &n_arg),
                fill(english_plural("{n} frame", "{n} frames", n), &n_arg),
            )
        };
        assert_eq!(form(0), ("0 vue".into(), "0 frames".into()));
        assert_eq!(form(1), ("1 vue".into(), "1 frame".into()));
        assert_eq!(form(2), ("2 vues".into(), "2 frames".into()));
        assert_eq!(fr.plural("{n} file", "{n} files", 0), "{n} files");
    }

    /// The rules of the common European languages read and give the
    /// forms gettext's manual gives them.
    #[test]
    fn plural_rules_evaluate_as_gettext_does() {
        let forms = |rule: &str, counts: &[u64]| {
            let (_, e) = plural::forms(rule).unwrap();
            counts.iter().map(|n| e.eval(*n)).collect::<Vec<_>>()
        };
        let counts = [0, 1, 2, 3, 5, 11, 21, 22, 25, 101, 111, 112];
        assert_eq!(
            forms("nplurals=2; plural=(n != 1);", &counts),
            [1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1]
        );
        assert_eq!(
            forms("nplurals=2; plural=(n > 1);", &counts),
            [0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1]
        );
        // Russian, Ukrainian, and the like.
        let slavic = "nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);";
        assert_eq!(forms(slavic, &counts), [2, 0, 1, 1, 2, 2, 0, 1, 2, 0, 2, 2]);
        // Polish.
        let polish = "nplurals=3; plural=(n==1 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);";
        assert_eq!(forms(polish, &counts), [2, 0, 1, 1, 2, 2, 2, 1, 2, 2, 2, 2]);
        // Czech and Slovak.
        assert_eq!(
            forms(
                "nplurals=3; plural=(n==1) ? 0 : (n>=2 && n<=4) ? 1 : 2;",
                &[0, 1, 2, 4, 5]
            ),
            [2, 0, 1, 1, 2]
        );
        // Japanese and the other one-form languages.
        assert_eq!(forms("nplurals=1; plural=0;", &[0, 1, 2]), [0, 0, 0]);
        for bad in [
            "nplurals=2; plural=(n >;",
            "plural=n != 1;",
            "nplurals=2; plural=n = 1;",
        ] {
            assert!(plural::forms(bad).is_err(), "{bad}");
        }
    }

    /// A tag picks its catalog, a regional or encoded locale its
    /// language's, and a language with none English.
    #[test]
    fn a_language_picks_its_catalog_or_english() {
        let tags = ["de", "fr", "pt_BR"];
        assert_eq!(pick_among(&tags, "fr"), Some("fr"));
        assert_eq!(pick_among(&tags, "fr-CA"), Some("fr"));
        assert_eq!(pick_among(&tags, "fr_FR.UTF-8"), Some("fr"));
        assert_eq!(pick_among(&tags, "pt-BR"), Some("pt_BR"));
        assert_eq!(pick_among(&tags, "pt_PT"), Some("pt_BR"));
        assert_eq!(pick_among(&tags, "it"), None);
        assert_eq!(pick_among(&tags, "en-US"), None);
        assert_eq!(pick_among(&tags, ""), None);
        assert_eq!(pick_among(&tags, "C"), None);
        // A script is dropped; Chinese's written forms take their regions.
        let chinese = ["zh_CN", "zh_TW", "sr"];
        assert_eq!(pick_among(&chinese, "zh-Hant"), Some("zh_TW"));
        assert_eq!(pick_among(&chinese, "zh-Hans"), Some("zh_CN"));
        assert_eq!(pick_among(&chinese, "zh-Hant-TW"), Some("zh_TW"));
        assert_eq!(pick_among(&chinese, "zh_TW"), Some("zh_TW"));
        assert_eq!(pick_among(&chinese, "sr-Latn-RS"), Some("sr"));
        // The build carries French, and nothing selected is English.
        assert_eq!(pick("fr-BE"), "fr");
        assert_eq!(pick("xx"), "");
        assert!(languages().contains(&("fr", "Français")));
        assert_eq!(own_name("zz"), "zz");
    }

    /// The macro's arms, in English, as every test reads it.
    #[test]
    fn the_macro_reads_english_with_no_language() {
        // By another name, so the catalog's extractor (`xtr`, which
        // looks for `tr!`) does not take the test's strings for the
        // window's.
        use crate::tr as shown;
        assert_eq!(selected(), "");
        let (a, b) = (2, 5);
        assert_eq!(shown!("Done"), "Done");
        assert_eq!(shown!("{{braces}}"), "{braces}");
        assert_eq!(shown!("{} of {}", a, b), "2 of 5");
        assert_eq!(shown!("{b} then {a}", a = a, b = b + 1), "6 then 2");
        assert_eq!(shown!("{n} frame" | "{n} frames" % 1), "1 frame");
        assert_eq!(shown!("{n} frame" | "{n} frames" % 3usize), "3 frames");
        assert_eq!(
            shown!("{n} frames in {}" | "{n} frames in {}" % 4, "Here"),
            "4 frames in Here"
        );
        assert_eq!(
            shown!("{n} in {place}" | "{n} in {place}" % -1i32, place = "x"),
            "-1 in x"
        );
    }
}

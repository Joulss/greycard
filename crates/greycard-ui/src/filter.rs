//! The folder's filter: which frames the browser lists, out of what
//! their sidecars say about them.
//!
//! This is the three-way Show grown up. That one asked one question
//! of one field — All, Picks, No rejects — and a culling pass asks
//! more than that: the four-star frames, the reds and the greens,
//! everything unflagged, the ones whose name or keywords carry a
//! word. So the filter is four tests over a frame's [`Meta`] and its
//! file name, and a frame is shown when it passes all four. The
//! three old names still parse, for the command line and for a
//! folder opened the way it was last time.
//!
//! Everything here is pure: it decides, and it never reads a file or
//! touches the window. The counts the chips carry are decided here
//! too, in one pass over the sidecars the browser has already
//! loaded, because a count that disagrees with what a click does is
//! worse than no count at all.
//!
//! The EXIF is not in a sidecar, so the facets — camera, lens, ISO,
//! focal length, day, style — are the library index's (notes §160);
//! the keyword beside them is the sidecar's and the file's kind its
//! path's, both answered in hand ([`in_hand`]). What is asked of the
//! index is decided here ([`Filter::index_filter`]) and answered in
//! `library`, and comes back as one bit a frame, [`Frame::index`]:
//! whether the index's tests pass it, or the index has no row for it
//! yet. A frame the index has not reached is shown until it has;
//! hiding it for want of a row would be the filter guessing. The
//! facets' own counts are one `GROUP BY` a facet over the frames the
//! other tests leave, which is this module's rule for a count, asked
//! of SQL.
//!
//! The text field reads the §160 grammar as well as words: a token
//! shaped like a term (`camera:R6`, `iso>=3200`, `rating>=3`) is a
//! term, anything else the word it always was. A term the sidecar
//! can answer — rating, flag, label, keyword, name — is answered
//! from the sidecar, which is fresher than the row an `index_file`
//! is still writing, and the kind from the path; the EXIF ones go to
//! the index.

use std::path::Path;

use greycard_edit::meta::{Flag, Label, Meta, STARS};
pub use greycard_library::filter::Facet;
use greycard_library::filter::{Cmp, NumberField, ParseError, Term};

use crate::i18n::text;
use crate::tr;

/// How the rating chips read: at least so many stars, or exactly so
/// many.
///
/// `AtLeast(0)` is every frame, which is why there is no third arm
/// for "any": the chip that says Any and the chip that says nothing
/// are the same chip, and one enum with one meaning is easier to be
/// right about than two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stars {
    AtLeast(u8),
    Exactly(u8),
}

impl Default for Stars {
    fn default() -> Self {
        Stars::AtLeast(0)
    }
}

impl Stars {
    /// The stars the chip stands for, whichever way it reads.
    pub fn count(self) -> u8 {
        match self {
            Stars::AtLeast(n) | Stars::Exactly(n) => n.min(STARS),
        }
    }

    pub fn is_exact(self) -> bool {
        matches!(self, Stars::Exactly(_))
    }

    /// The same count, read the other way.
    pub fn read_as(self, exact: bool) -> Stars {
        let n = self.count();
        if exact {
            Stars::Exactly(n)
        } else {
            Stars::AtLeast(n)
        }
    }

    pub fn shows(self, rating: u8) -> bool {
        match self {
            Stars::AtLeast(n) => rating >= n,
            Stars::Exactly(n) => rating == n,
        }
    }

    /// What the `n`th rating chip is called, under the reading the
    /// row is in. Under "at least" the first chip lets every frame
    /// through and says so; under "exactly" it is the unrated ones,
    /// and there is nothing else to call it but zero. The last chip
    /// is five either way, since no frame carries six.
    pub fn chip_name(n: u8, exact: bool) -> String {
        if exact {
            n.to_string()
        } else if n == 0 {
            // Filter: the rating choice that passes every frame
            tr!("Any")
        } else if n >= STARS {
            STARS.to_string()
        } else {
            format!("{n}+")
        }
    }
}

/// The filter as the settings keep it between sessions, and as a smart
/// collection's file does: the library's, since the command line
/// writes it too.
pub use greycard_library::filter::Saved;

/// The bar kept and given back: [`Saved`] is the shape on disk, and
/// the filter it stands for is this module's.
pub trait SavedBar {
    fn of(filter: &Filter) -> Saved;
    /// The filter kept, a facet this build does not know left out. ISO
    /// and focal length chips, kept by a build that had them, come back
    /// as the range's terms in the text ([`ranged_terms`]). Quiet: it is
    /// asked at every rebuild, of every smart collection.
    fn filter(&self) -> Filter;
    /// [`SavedBar::filter`] for a filter put back on the bar (the last
    /// session's, a smart collection opened), the log told once of any
    /// held-out ISO or focal value it drops.
    fn opened(&self) -> Filter;
}

/// The terms that stand for a ranged facet's chips as an older build
/// kept them: the values chosen as one range, from the lowest to the
/// highest (`iso:800` for one value, `iso>=400 iso<=1600` for more,
/// which also takes the values between them). A value that is not a
/// number is dropped. Values held out are dropped too ([`SavedBar::opened`]
/// logs them): a held-out chip let a frame with no value through, and
/// `iso!=v` would hide it, so the filter widens in plain view rather
/// than hiding frames no one asked to hide.
fn ranged_terms(facet: Facet, on: &[String]) -> Vec<String> {
    let name = facet.field();
    fn number(v: &str) -> Option<(f64, &str)> {
        v.trim().parse::<f64>().ok().map(|n| (n, v.trim()))
    }
    let mut on: Vec<(f64, &str)> = on.iter().map(String::as_str).filter_map(number).collect();
    on.sort_by(|a, b| a.0.total_cmp(&b.0));
    match (on.first(), on.last()) {
        (Some(lo), Some(hi)) if lo.0 == hi.0 => vec![format!("{name}:{}", lo.1)],
        (Some(lo), Some(hi)) => vec![format!("{name}>={}", lo.1), format!("{name}<={}", hi.1)],
        _ => Vec::new(),
    }
}

impl SavedBar for Saved {
    fn of(filter: &Filter) -> Saved {
        Saved {
            stars: filter.stars.count(),
            exact: filter.stars.is_exact(),
            flags: filter.flags.clone(),
            labels: filter.labels.clone(),
            text: filter.text.clone(),
            facets: Facet::ALL
                .iter()
                .filter(|f| !filter.chosen(**f).is_empty())
                .map(|f| (f.field().to_string(), filter.chosen(*f).to_vec()))
                .collect(),
            not_flags: filter.not_flags.clone(),
            not_labels: filter.not_labels.clone(),
            not_facets: Facet::ALL
                .iter()
                .filter(|f| !filter.held_out(**f).is_empty())
                .map(|f| (f.field().to_string(), filter.held_out(*f).to_vec()))
                .collect(),
        }
    }

    fn filter(&self) -> Filter {
        let mut filter = Filter {
            stars: Stars::AtLeast(self.stars.min(STARS)).read_as(self.exact),
            flags: self.flags.clone(),
            labels: self.labels.clone(),
            text: self.text.clone(),
            not_flags: self.not_flags.clone(),
            not_labels: self.not_labels.clone(),
            ..Filter::default()
        };
        for (field, values) in &self.facets {
            if let Some(facet) = Facet::from_field(field).filter(|f| ranged(*f).is_none()) {
                filter.facets[facet_slot(facet)] = values.clone();
            }
        }
        for (field, values) in &self.not_facets {
            if let Some(facet) = Facet::from_field(field).filter(|f| ranged(*f).is_none()) {
                filter.not_facets[facet_slot(facet)] = values.clone();
            }
        }
        // ISO and the focal length have no chips: an older build's come
        // back as the range's terms.
        let terms: Vec<String> = RANGED
            .iter()
            .flat_map(|(facet, _)| ranged_terms(*facet, &kept(&self.facets, *facet)))
            .collect();
        filter.text = append(std::mem::take(&mut filter.text), &terms);
        filter
    }

    fn opened(&self) -> Filter {
        for (facet, _) in RANGED {
            let out = kept(&self.not_facets, facet);
            if !out.is_empty() {
                tracing::info!(
                    "filter: {} values held out by an older build ({}) dropped; \
                     a range cannot leave a value out",
                    facet.field(),
                    out.join(", ")
                );
            }
        }
        self.filter()
    }
}

/// A saved filter's values for `facet`, under any of its field's names.
fn kept(map: &std::collections::BTreeMap<String, Vec<String>>, facet: Facet) -> Vec<String> {
    (map.iter())
        .filter(|(field, _)| Facet::from_field(field) == Some(facet))
        .flat_map(|(_, v)| v.iter().cloned())
        .collect()
}

/// What the browser says when the filter has hidden the whole
/// folder. A blank grid and a zero is not an explanation, and the
/// four places that can arrive at one say the same thing.
///
/// It used to read "show All to see them", which named a control:
/// All was the three-way Show's first option. It is now the first
/// rating chip and means any rating, so the way back is Clear.
pub fn nothing_shown() -> String {
    tr!("no frames pass the filter; Clear shows the folder again")
}

/// One frame as the filter reads it: where the file is, which is
/// where its name is, what its sidecar says about it, and what the
/// library index made of it.
#[derive(Debug, Clone, Copy)]
pub struct Frame<'a> {
    pub path: &'a Path,
    pub meta: &'a Meta,
    /// Whether the index's tests — the EXIF facets and the EXIF
    /// terms typed — pass this frame. True when nothing is asked of
    /// the index, and when the index has no row for the frame yet:
    /// it shows until the pass says otherwise.
    pub index: bool,
}

/// The text field, read: the words looked for as they always were,
/// the terms the sidecar answers, the terms only the index can, and
/// what did not parse.
#[derive(Debug, Clone, Default)]
pub struct Typed {
    pub words: Vec<String>,
    pub meta: Vec<Term>,
    pub index: Vec<Term>,
    pub errors: Vec<ParseError>,
}

impl Typed {
    /// Whether the words and the sidecar's terms pass this frame.
    /// Every word has to be somewhere, in the name or in one
    /// keyword, which is the search box's rule unchanged.
    fn shows(&self, path: &Path, meta: &Meta) -> bool {
        if !self.words.is_empty() {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let words: Vec<String> = meta.keywords.iter().map(|k| k.to_lowercase()).collect();
            let all = self
                .words
                .iter()
                .all(|w| name.contains(w.as_str()) || words.iter().any(|k| k.contains(w.as_str())));
            if !all {
                return false;
            }
        }
        self.meta
            .iter()
            .all(|t| t.on_meta(path, meta).unwrap_or(true))
    }
}

/// Whether a facet's chips are answered by the frames in hand rather
/// than by the index, and its row counted from them: the keyword from
/// the sidecar, which is fresher than a row, and the kind from the
/// path, which every frame has before the index has reached it.
pub fn in_hand(facet: Facet) -> bool {
    matches!(facet, Facet::Keyword | Facet::Kind)
}

/// A facet's place in [`Facet::ALL`], which is its slot in
/// [`Filter::facets`] and its code on the window.
pub fn facet_slot(facet: Facet) -> usize {
    Facet::ALL
        .iter()
        .position(|f| *f == facet)
        .expect("every facet is in ALL")
}

/// What a facet's row is captioned on the bar: in English, marked for
/// the catalog, and translated where it is shown.
pub fn facet_caption(facet: Facet) -> &'static str {
    use crate::marked;
    match facet {
        // Filter bar: a facet's chip
        Facet::Camera => marked::tr!("Camera"),
        // Filter bar: a facet's chip
        Facet::Lens => marked::tr!("Lens"),
        // Filter bar: a facet's chip
        Facet::Kind => marked::tr!("Kind"),
        // Filter bar: a facet's chip
        Facet::Style => marked::tr!("Style"),
        Facet::Iso => "ISO",
        // Filter bar: a facet's chip, the focal length
        Facet::Focal => marked::tr!("Focal"),
        // Filter bar: a facet's chip, the capture day
        Facet::Date => marked::tr!("Day"),
        // Filter bar: a facet's chip
        Facet::Keyword => marked::tr!("Keyword"),
    }
}

/// The flags' and the labels' names (`Flag::name`, `Label::name`, in
/// greycard-edit), marked here for the catalog: [`Filter::describe`]
/// translates them where it says them.
const _MARK_NAMES: [&str; 9] = [
    crate::marked::tr!("Unflagged"),
    crate::marked::tr!("Pick"),
    crate::marked::tr!("Reject"),
    crate::marked::tr!("None"),
    crate::marked::tr!("Red"),
    crate::marked::tr!("Yellow"),
    crate::marked::tr!("Green"),
    crate::marked::tr!("Blue"),
    crate::marked::tr!("Purple"),
];

/// The flag and label names as a sentence says them, in lower case,
/// their own entries in the catalog (a language need not lower its
/// words as English does).
const _MARK_LOWER_NAMES: [&str; 9] = [
    crate::marked::tr!("unflagged"),
    crate::marked::tr!("pick"),
    crate::marked::tr!("reject"),
    crate::marked::tr!("none"),
    crate::marked::tr!("red"),
    crate::marked::tr!("yellow"),
    crate::marked::tr!("green"),
    crate::marked::tr!("blue"),
    crate::marked::tr!("purple"),
];

/// A facet's value as its chip says it: a focal length in
/// millimeters, a kind as a word (Raw) or a format's capitals (JPEG),
/// the rest as the index has it.
pub fn facet_chip_text(facet: Facet, label: &str) -> String {
    match facet {
        Facet::Focal => format!("{label} mm"),
        // Filter bar: the kind chip for raws
        Facet::Kind if label == greycard_library::filter::RAW => tr!("Raw"),
        Facet::Kind => label.to_uppercase(),
        _ => label.to_string(),
    }
}

/// The facets the CULLING section filters by a range rather than by a
/// chip a value, each with the number field its terms test. The range
/// reads and writes those terms in the text, so a range set is a
/// filter typed, and one typed is a range set.
pub const RANGED: [(Facet, NumberField); 2] = [
    (Facet::Iso, NumberField::Iso),
    (Facet::Focal, NumberField::Focal),
];

/// The number field a ranged facet's terms test.
pub fn ranged(facet: Facet) -> Option<NumberField> {
    RANGED.iter().find(|(f, _)| *f == facet).map(|(_, n)| *n)
}

/// The tokens of a filter's text by their byte spans, split as
/// [`greycard_library::filter::tokens`] splits them: on whitespace
/// outside double quotes.
fn spans(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    let mut quoted = false;
    for (i, c) in text.char_indices() {
        if c == '"' {
            quoted = !quoted;
            start.get_or_insert(i);
        } else if c.is_whitespace() && !quoted {
            if let Some(s) = start.take() {
                out.push((s, i));
            }
        } else {
            start.get_or_insert(i);
        }
    }
    if let Some(s) = start {
        out.push((s, text.len()));
    }
    out
}

/// A token of the text as a bound on `field`: its test and value. A
/// `!=` is no bound and is left alone, as is anything else.
fn bound_on(token: &str, field: NumberField) -> Option<(Cmp, f64)> {
    if !greycard_library::filter::is_term(token) {
        return None;
    }
    let parsed = greycard_library::Filter::from_terms(&[token]).ok()?;
    match parsed.terms.as_slice() {
        [
            Term::Number {
                field: f,
                op,
                value,
            },
        ] if *f == field && *op != Cmp::Ne => Some((*op, *value)),
        _ => None,
    }
}

/// The text with the tokens `goes` picks spliced out, every other byte
/// as typed: a token goes with the gap before it, or, with no token
/// kept before it, with the gap after it.
fn cut(text: &str, goes: impl Fn(&str) -> bool) -> String {
    let spans = spans(text);
    let Some((first, last)) = spans.first().zip(spans.last()) else {
        return text.to_string();
    };
    let mut out = text[..first.0].to_string();
    let mut kept_any = false;
    let mut prev_end = first.0;
    for &(s, e) in &spans {
        if !goes(&text[s..e]) {
            if kept_any {
                out.push_str(&text[prev_end..s]);
            }
            out.push_str(&text[s..e]);
            kept_any = true;
        }
        prev_end = e;
    }
    out.push_str(&text[last.1..]);
    out
}

/// Terms written at the end of the text: after a closing quote when the
/// text ends inside an open one, so a term is never taken into a
/// phrase, and after a space unless the text ends in one.
fn append(mut text: String, terms: &[String]) -> String {
    if terms.is_empty() {
        return text;
    }
    if text.matches('"').count() % 2 == 1 {
        text.push('"');
    }
    if !text.is_empty() && !text.ends_with(char::is_whitespace) {
        text.push(' ');
    }
    text.push_str(&terms.join(" "));
    text
}

/// Whether a value passes a bound, as the index's SQL has it: a whole
/// number exactly, a focal length through its tenth of a millimeter.
fn passes(field: NumberField, op: Cmp, bound: f64, v: f64) -> bool {
    let (lo, hi) = if field.is_whole() {
        (bound, bound)
    } else {
        (bound - 0.05, bound + 0.05)
    };
    match op {
        Cmp::Eq => v >= lo && v <= hi,
        Cmp::Ne => true,
        Cmp::Ge => v >= lo,
        Cmp::Gt => v > hi,
        Cmp::Le => v <= hi,
        Cmp::Lt => v < lo,
    }
}

fn is_low(op: Cmp) -> bool {
    matches!(op, Cmp::Ge | Cmp::Gt | Cmp::Eq)
}

fn is_high(op: Cmp) -> bool {
    matches!(op, Cmp::Le | Cmp::Lt | Cmp::Eq)
}

/// Where a range's ends sit among the folder's `values` in order, for
/// the text's bounds on `field`: the low end at the lowest value every
/// low bound (`>=`, `>`, `:`) lets through, the high at the highest
/// every high bound (`<=`, `<`, `:`) does, so the handles span the
/// values the filter passes; and whether anything bounds it. A bound on
/// a value the folder lacks moves its end to the next value in, the
/// text left as typed. Bounds that pass no value put both ends on the
/// value nearest the one asked: an exact one, or else the tightest low,
/// or else the tightest high.
pub fn range_ends(text: &str, field: NumberField, values: &[f64]) -> (usize, usize, bool) {
    let top = values.len().saturating_sub(1);
    let bounds: Vec<(Cmp, f64)> = greycard_library::filter::tokens(text)
        .iter()
        .filter_map(|t| bound_on(t, field))
        .collect();
    if bounds.is_empty() || values.is_empty() {
        return (0, top, !bounds.is_empty());
    }
    let all = |want: fn(Cmp) -> bool, v: f64| {
        bounds
            .iter()
            .filter(|(op, _)| want(*op))
            .all(|(op, b)| passes(field, *op, *b, v))
    };
    let low = values.iter().position(|v| all(is_low, *v));
    let high = values.iter().rposition(|v| all(is_high, *v));
    if let (Some(l), Some(h)) = (low, high)
        && l <= h
    {
        return (l, h, true);
    }
    let of = |want: fn(Cmp) -> bool| bounds.iter().filter(move |(op, _)| want(*op)).map(|b| b.1);
    let asked = bounds
        .iter()
        .find(|(op, _)| *op == Cmp::Eq)
        .map(|b| b.1)
        .or_else(|| of(is_low).reduce(f64::max))
        .or_else(|| of(is_high).reduce(f64::min))
        .unwrap_or_default();
    let near = (0..values.len())
        .min_by(|a, b| {
            (values[*a] - asked)
                .abs()
                .total_cmp(&(values[*b] - asked).abs())
        })
        .unwrap_or(0);
    (near, near, true)
}

/// The text after a range's end was moved: the moved end's bounds on
/// the facet's field spliced out and `field>=low` or `field<=high`
/// written at the end, none when that end is at the folder's own
/// (`low` or `high` empty). The other end's bounds stay as typed;
/// an exact `field:v` bounds both ends, so it goes, with every bound on
/// the field, and both ends are written as shown. A `!=` and every
/// other byte of the text stay as typed.
pub fn with_range(text: &str, facet: Facet, moved_low: bool, low: &str, high: &str) -> String {
    let Some(field) = ranged(facet) else {
        return text.to_string();
    };
    let both = spans(text)
        .iter()
        .any(|(s, e)| bound_on(&text[*s..*e], field).is_some_and(|(op, _)| op == Cmp::Eq));
    let goes = |op: Cmp| both || if moved_low { is_low(op) } else { is_high(op) };
    let kept = cut(text, |t| bound_on(t, field).is_some_and(|(op, _)| goes(op)));
    let name = facet.field();
    let mut terms = Vec::new();
    if (moved_low || both) && !low.is_empty() {
        terms.push(format!("{name}>={low}"));
    }
    if (!moved_low || both) && !high.is_empty() {
        terms.push(format!("{name}<={high}"));
    }
    append(kept, &terms)
}

/// The text with the facet's range bounds spliced out, every other byte
/// as typed: the folder's whole range, which asks nothing.
pub fn without_range(text: &str, facet: Facet) -> String {
    let Some(field) = ranged(facet) else {
        return text.to_string();
    };
    cut(text, |t| bound_on(t, field).is_some())
}

/// What the browser shows of the folder.
///
/// Every field is a test, and a frame is shown when it passes all of
/// them. An empty set of flags or labels is every flag or every
/// label rather than none: a chip row with nothing picked is a
/// question nobody asked, and the alternative — an empty folder
/// until something is ticked — is a filter that starts by hiding
/// everything.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Filter {
    pub stars: Stars,
    /// Any-of. Empty is every flag.
    pub flags: Vec<Flag>,
    /// Any-of. Empty is every label, [`Label::None`] among them.
    pub labels: Vec<Label>,
    /// Matched against the file name and the keywords, a word at a
    /// time, with the §160 grammar's terms among the words. Empty is
    /// every frame.
    pub text: String,
    /// The facets' chips, by the facet's slot in [`Facet::ALL`]:
    /// any-of within a facet, as the flags and labels are, and an
    /// empty facet asks nothing. The values are the index's own
    /// ([`greycard_library::FacetCount::value`]).
    pub facets: [Vec<String>; Facet::ALL.len()],
    /// The chips held out, a Ctrl+click each: none-of, within each
    /// group, beside the any-of above, so "everything but the
    /// rejects" is one chip. A chip is on, held out, or neither.
    pub not_flags: Vec<Flag>,
    pub not_labels: Vec<Label>,
    pub not_facets: [Vec<String>; Facet::ALL.len()],
}

impl Filter {
    /// Whether this filter shows the whole folder, which is what
    /// the panel asks before it bothers rebuilding anything.
    pub fn is_empty(&self) -> bool {
        *self == Filter::default()
    }

    /// The three names the old three-way Show went by, still spoken
    /// on the command line and in the log. Nothing else parses.
    pub const NAMES: [&'static str; 3] = ["All", "Picks", "No rejects"];

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match Self::NAMES.iter().position(|n| *n == name)? {
            1 => Filter {
                flags: vec![Flag::Pick],
                ..Filter::default()
            },
            2 => Filter {
                flags: vec![Flag::None, Flag::Pick],
                ..Filter::default()
            },
            _ => Filter::default(),
        })
    }

    /// Whether the flag chips are ticked; a frame's flag passes when
    /// none of them is.
    pub fn shows_flag(&self, flag: Flag) -> bool {
        any_of(&self.flags, flag) && !self.not_flags.contains(&flag)
    }

    pub fn shows_label(&self, label: Label) -> bool {
        any_of(&self.labels, label) && !self.not_labels.contains(&label)
    }

    /// The text field read: words, the sidecar's terms, the index's
    /// terms, and the terms that did not parse. A token shaped like a
    /// term that does not parse (`rating>=9`) is kept as the word it
    /// would have been before the field took terms, and its error is
    /// said on the status line; a filter that silently asked nothing
    /// would look like the folder and not like a mistake.
    ///
    /// Words are split exactly as the search box always split them —
    /// on whitespace, quotes kept as characters — so a field with no
    /// term in it means what it meant.
    pub fn typed(&self) -> Typed {
        use greycard_library::filter::{is_term, tokens};
        let mut typed = Typed::default();
        let text = self.text.trim();
        if text.is_empty() {
            return typed;
        }
        for token in tokens(text) {
            if is_term(&token) {
                match greycard_library::Filter::from_terms(&[&token]) {
                    Ok(parsed) => {
                        for term in parsed.terms {
                            if term.is_meta() {
                                typed.meta.push(term);
                            } else {
                                typed.index.push(term);
                            }
                        }
                        continue;
                    }
                    Err(e) => typed.errors.push(e),
                }
            }
            typed
                .words
                .extend(token.to_lowercase().split_whitespace().map(str::to_string));
        }
        typed
    }

    /// The chips of one facet.
    pub fn chosen(&self, facet: Facet) -> &[String] {
        &self.facets[facet_slot(facet)]
    }

    /// The chips of one facet held out.
    pub fn held_out(&self, facet: Facet) -> &[String] {
        &self.not_facets[facet_slot(facet)]
    }

    /// Press a facet's chip: on if it was off, off if it was on or
    /// held out.
    pub fn toggle_facet(&mut self, facet: Facet, value: &str) {
        let at = facet_slot(facet);
        press(
            &mut self.facets[at],
            &mut self.not_facets[at],
            value.to_string(),
            false,
        );
    }

    /// Ctrl+click a facet's chip: held out if it was not, off if it
    /// was.
    pub fn exclude_facet(&mut self, facet: Facet, value: &str) {
        let at = facet_slot(facet);
        press(
            &mut self.facets[at],
            &mut self.not_facets[at],
            value.to_string(),
            true,
        );
    }

    /// Whether the chips answered in hand ([`in_hand`]) pass this
    /// frame, `except`'s own set aside for that facet's count: the
    /// keyword asked of the sidecar, as the text field's keyword
    /// terms are, and the kind of the path, as `kind:` is.
    pub fn shows_in_hand(&self, frame: Frame, except: Option<Facet>) -> bool {
        Facet::ALL
            .into_iter()
            .filter(|f| in_hand(*f) && Some(*f) != except)
            .all(|facet| {
                let (on, out) = (self.chosen(facet), self.held_out(facet));
                (on.is_empty()
                    || Term::Facet {
                        facet,
                        values: on.to_vec(),
                    }
                    .on_meta(frame.path, frame.meta)
                    .unwrap_or(true))
                    && Term::NotFacet {
                        facet,
                        values: out.to_vec(),
                    }
                    .on_meta(frame.path, frame.meta)
                    .unwrap_or(true)
            })
    }

    /// What the index is asked: the EXIF terms typed and every facet
    /// not answered in hand with a chip on, less `except`'s own group
    /// — which is how a facet's count sets its own row aside.
    pub fn index_filter(&self, typed: &Typed, except: Option<Facet>) -> greycard_library::Filter {
        let mut terms = typed.index.clone();
        for facet in Facet::ALL {
            if in_hand(facet) || Some(facet) == except {
                continue;
            }
            let (on, out) = (self.chosen(facet), self.held_out(facet));
            if !on.is_empty() {
                terms.push(Term::Facet {
                    facet,
                    values: on.to_vec(),
                });
            }
            if !out.is_empty() {
                terms.push(Term::NotFacet {
                    facet,
                    values: out.to_vec(),
                });
            }
        }
        greycard_library::Filter { terms }
    }

    /// Whether anything is asked of the index: without that, every
    /// frame passes it and the index need not be asked.
    pub fn asks_index(&self, typed: &Typed) -> bool {
        !typed.index.is_empty()
            || Facet::ALL.iter().any(|f| {
                !in_hand(*f) && !(self.chosen(*f).is_empty() && self.held_out(*f).is_empty())
            })
    }

    /// Whether the tests the frames in hand answer pass this frame:
    /// every group but the index's, with `except`'s chips set aside,
    /// for that facet's own count.
    pub fn shows_meta(&self, typed: &Typed, frame: Frame, except: Option<Facet>) -> bool {
        self.stars.shows(frame.meta.rating)
            && self.shows_flag(frame.meta.flag)
            && self.shows_label(frame.meta.label)
            && typed.shows(frame.path, frame.meta)
            && self.shows_in_hand(frame, except)
    }

    /// Whether the whole filter shows this frame.
    pub fn shows(&self, frame: Frame) -> bool {
        frame.index && self.shows_meta(&self.typed(), frame, None)
    }

    /// Turn on the chips `--filter` asked for that no frame has to
    /// name — a kind, which [`from_cli`] has already read as the
    /// term reads it — and give back the rest, to be chosen once the
    /// frames and the index can name theirs.
    pub fn choose_at_once(&mut self, wanted: Vec<Wanted>) -> Vec<Wanted> {
        let mut rest = Vec::new();
        for want in wanted {
            if want.facet == Facet::Kind {
                if !self.chosen(Facet::Kind).contains(&want.value) {
                    self.toggle_facet(Facet::Kind, &want.value);
                }
            } else {
                rest.push(want);
            }
        }
        rest
    }

    /// The file indices the browser lists, in file order.
    pub fn apply(&self, frames: &[Frame]) -> Vec<usize> {
        let typed = self.typed();
        frames
            .iter()
            .enumerate()
            .filter(|(_, f)| f.index && self.shows_meta(&typed, **f, None))
            .map(|(i, _)| i)
            .collect()
    }

    /// Tick a flag chip on, or off if it was on or held out.
    pub fn toggle_flag(&mut self, flag: Flag) {
        press(&mut self.flags, &mut self.not_flags, flag, false);
    }

    /// Hold a flag chip out, or let it back if it was.
    pub fn exclude_flag(&mut self, flag: Flag) {
        press(&mut self.flags, &mut self.not_flags, flag, true);
    }

    pub fn toggle_label(&mut self, label: Label) {
        press(&mut self.labels, &mut self.not_labels, label, false);
    }

    pub fn exclude_label(&mut self, label: Label) {
        press(&mut self.labels, &mut self.not_labels, label, true);
    }

    /// The filter in words, for the status line and the log, in the
    /// window's language: what is asked of a frame, in the order the
    /// chips sit in, or "all frames" when nothing is asked.
    pub fn describe(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        match self.stars {
            Stars::AtLeast(0) => {}
            Stars::AtLeast(n) => parts.push(tr!("{n} star or more" | "{n} stars or more" % n)),
            Stars::Exactly(0) => parts.push(tr!("unrated")),
            Stars::Exactly(n) => parts.push(tr!("exactly {n} star" | "exactly {n} stars" % n)),
        }
        let label_name = |l: &Label| match l {
            Label::None => tr!("no label"),
            _ => text(&l.name().to_lowercase()),
        };
        let flag_name = |f: &Flag| text(&f.name().to_lowercase());
        if !self.flags.is_empty() {
            parts.push(join(self.flags.iter().map(flag_name)));
        }
        if !self.not_flags.is_empty() {
            parts.push(tr!("not {}", join(self.not_flags.iter().map(flag_name))));
        }
        if !self.labels.is_empty() {
            parts.push(join(self.labels.iter().map(label_name)));
        }
        // "No label" held out is every label, said so.
        if self.not_labels.contains(&Label::None) {
            parts.push(tr!("labeled"));
        }
        let colors: Vec<&Label> = self
            .not_labels
            .iter()
            .filter(|l| **l != Label::None)
            .collect();
        if !colors.is_empty() {
            parts.push(tr!("not {}", join(colors.into_iter().map(label_name))));
        }
        let text = self.text.trim();
        if !text.is_empty() {
            parts.push(format!("\"{text}\""));
        }
        for (facet, values, not) in Facet::ALL
            .into_iter()
            .flat_map(|f| [(f, self.chosen(f), false), (f, self.held_out(f), true)])
        {
            if values.is_empty() {
                continue;
            }
            let said = join(values.iter().cloned());
            let asked = match facet {
                Facet::Camera => tr!("camera {}", said),
                Facet::Lens => tr!("lens {}", said),
                Facet::Kind => tr!(
                    "kind {}",
                    join(values.iter().map(|v| facet_chip_text(Facet::Kind, v)))
                ),
                Facet::Style => tr!("style {}", said),
                Facet::Iso => format!("ISO {said}"),
                Facet::Focal => format!("{said} mm"),
                Facet::Date => tr!("taken {}", said),
                Facet::Keyword => tr!("keyword {}", said),
            };
            parts.push(if not { tr!("not {}", asked) } else { asked });
        }
        if parts.is_empty() {
            tr!("all frames")
        } else {
            parts.join(", ")
        }
    }
}

/// A facet's chips `--filter` asked for, to be turned on once the
/// index can name them: with `:` every value containing `value`,
/// with `=` the value that is it, case aside, as the language reads
/// the two on a text field; a number is equal either way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
    pub facet: Facet,
    pub value: String,
    pub exact: bool,
}

/// `--filter` in the filter language, split: a camera, lens, kind,
/// style, date, or keyword term with `:` or `=` is a facet's chips to
/// turn on once they can be named ([`Wanted`]); every other token,
/// ISO and focal length terms among them, is left for the text field,
/// in its order. A
/// kind is read as the term reads it (`jpg` is `jpeg`) and wanted
/// exactly, which names its chip at once
/// ([`Filter::choose_at_once`]), and one that is no kind is left for
/// the field, to be refused there with the kinds there are.
pub fn from_cli(text: &str) -> (String, Vec<Wanted>) {
    let mut rest = Vec::new();
    let mut wanted = Vec::new();
    for token in greycard_library::filter::tokens(text) {
        let name_len = token
            .find(|c: char| !c.is_ascii_alphabetic())
            .unwrap_or(token.len());
        let (name, after) = token.split_at(name_len);
        let (value, exact) = match (after.strip_prefix(':'), after.strip_prefix('=')) {
            (Some(v), _) => (Some(v), false),
            (None, Some(v)) => (Some(v), true),
            (None, None) => (None, false),
        };
        match (Facet::from_field(name), value.map(|v| v.trim_matches('"'))) {
            // ISO and the focal length have no chips: their terms go to
            // the text, where the CULLING section's range reads them,
            // under the field's own name (`mm:50` as `focal:50`). A
            // token that is no term (the word `mm`) stays as typed.
            (Some(facet), _) if ranged(facet).is_some() => {
                let named = format!("{}{after}", facet.field());
                rest.push(if greycard_library::filter::is_term(&named) {
                    named
                } else {
                    token.clone()
                });
            }
            (Some(Facet::Kind), Some(v)) => match greycard_library::filter::kind_named(v) {
                Some(kind) => wanted.push(Wanted {
                    facet: Facet::Kind,
                    value: kind.to_string(),
                    exact: true,
                }),
                None => rest.push(token),
            },
            (Some(facet), Some(v)) if !v.is_empty() && !v.starts_with('=') => {
                wanted.push(Wanted {
                    facet,
                    value: v.to_string(),
                    exact,
                });
            }
            _ => rest.push(token),
        }
    }
    (rest.join(" "), wanted)
}

/// Whether an any-of set lets a value through: an empty set is every
/// value, not none, so a group with no chip ticked asks nothing.
fn any_of<T: PartialEq>(set: &[T], value: T) -> bool {
    set.is_empty() || set.contains(&value)
}

/// A group's chips after `chip` is pressed, `on` the set ticked and
/// `out` the set held out, the order of the rest kept. A chip held
/// out comes back to neither, whichever the press; otherwise a plain
/// press (`exclude` false) ticks it on or off, and a Ctrl+press holds
/// it out, off the ticked set.
///
/// This is what a click does. It is not what a chip's count counts;
/// see [`Counts`] for why those are two different questions.
fn press<T: PartialEq>(on: &mut Vec<T>, out: &mut Vec<T>, chip: T, exclude: bool) {
    if let Some(at) = out.iter().position(|v| *v == chip) {
        out.remove(at);
    } else if let Some(at) = on.iter().position(|v| *v == chip) {
        on.remove(at);
        if exclude {
            out.push(chip);
        }
    } else if exclude {
        out.push(chip);
    } else {
        on.push(chip);
    }
}

/// "a", "a or b", "a, b or c".
fn join(names: impl IntoIterator<Item = String>) -> String {
    let names: Vec<String> = names.into_iter().collect();
    match names.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => tr!("{} or {last}", rest.join(", "), last = last),
    }
}

/// What each chip would show, and what the whole filter shows now.
///
/// A chip's count is how many frames carry that chip's value among
/// the frames the *other* groups leave: its own group's chips are
/// ignored, and every other group is applied as it stands. So the
/// flag row under a three-star filter reads how the three-star
/// frames are flagged, and the label row reads how they are
/// labeled — the state of the cull, narrowed by everything else
/// being asked.
///
/// It is worth being plain about what this is not, because the first
/// cut of it was the other thing. A count is not "press this and N
/// frames will be listed". For the rating row the two happen to
/// agree, since a rating chip replaces. For the flag and label rows
/// they do not, because those chips *join* their group: with Picks
/// on, this rule has Reject read the number of rejects, while
/// pressing Reject adds them to the picks and lists both. The
/// press-and-N reading was tried and is worse — with Picks on it
/// made the row read Unflagged 28, Pick 33, Reject 18 on a shoot of
/// 33, which is arithmetic about the filter rather than anything
/// about the pictures, and a culler learns nothing from it. What is
/// wanted from a row of chips is the shape of the shoot; how many
/// frames a press will leave is one number, and the `N of M` beside
/// the field is where it belongs.
///
/// Two consequences worth leaning on. Within a group the counts sum
/// to the frames the other groups leave, since a frame has one flag
/// and one label or none — a row that adds up is a row that can be
/// read at a glance. And a chip that says 0 leads nowhere when
/// pressed *alone*, which is what dims it.
///
/// Stated exactly: `counts.flags[n]` is
/// `filter.only_flag(Flag::ALL[n]).apply(frames).len()`, and the
/// same for the label and rating rows. The tests hold it to that.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Counts {
    /// By the chip, 0 to [`STARS`], read the way the row is read.
    pub stars: [usize; STARS as usize + 1],
    /// By the chip, in [`Flag::ALL`]'s order: unflagged, pick, reject.
    pub flags: [usize; 3],
    /// By the chip, in [`Label::ALL`]'s order: no label, then the
    /// five colors.
    pub labels: [usize; 6],
    /// What the filter as it stands shows, and the folder's size.
    pub shown: usize,
    pub total: usize,
}

impl Counts {
    /// One pass over the loaded sidecars. Nothing is indexed and no
    /// file is read: the folder is open, its sidecars are in hand,
    /// and a thousand frames is a thousand cheap comparisons.
    ///
    /// Each frame answers the four groups once. It then counts
    /// towards a group's chips exactly when the other three said
    /// yes, which is that group being ignored for its own row; and
    /// within the row it lands on the chip it carries — for the
    /// rating, on every chip its stars satisfy, since those chips
    /// overlap by construction.
    pub fn of(filter: &Filter, frames: &[Frame]) -> Counts {
        let mut counts = Counts {
            total: frames.len(),
            ..Counts::default()
        };
        let exact = filter.stars.is_exact();
        let typed = filter.typed();
        for frame in frames {
            let meta = frame.meta;
            let stars = filter.stars.shows(meta.rating);
            let flag = filter.shows_flag(meta.flag);
            let label = filter.shows_label(meta.label);
            // The text, the chips answered in hand and the index's
            // tests are groups of their own whose rows are counted
            // elsewhere, so here they are one more thing the other
            // rows apply.
            let text =
                typed.shows(frame.path, meta) && filter.shows_in_hand(*frame, None) && frame.index;
            if stars && flag && label && text {
                counts.shown += 1;
            }
            if flag && label && text {
                for (n, seen) in counts.stars.iter_mut().enumerate() {
                    if Stars::AtLeast(n as u8).read_as(exact).shows(meta.rating) {
                        *seen += 1;
                    }
                }
            }
            if stars
                && label
                && text
                && let Some(n) = Flag::ALL.iter().position(|f| *f == meta.flag)
            {
                counts.flags[n] += 1;
            }
            if stars
                && flag
                && text
                && let Some(n) = Label::ALL.iter().position(|l| *l == meta.label)
            {
                counts.labels[n] += 1;
            }
        }
        counts
    }
}

/// The three filters a row of chips is counted against, which is the
/// rule [`Counts`] keeps written down in one place and the tests
/// hold it to. Nothing in the window needs them — the count is made
/// in one pass rather than by filtering the folder once a chip — so
/// they exist to say what that pass is supposed to come to.
#[cfg(test)]
impl Filter {
    /// This filter with the flag group set to just this one chip.
    ///
    /// The question a flag chip's count answers, and deliberately not
    /// the question pressing it asks: see [`Counts`].
    pub fn only_flag(&self, flag: Flag) -> Filter {
        Filter {
            flags: vec![flag],
            not_flags: Vec::new(),
            ..self.clone()
        }
    }

    /// This filter with the label group set to just this one chip.
    pub fn only_label(&self, label: Label) -> Filter {
        Filter {
            labels: vec![label],
            not_labels: Vec::new(),
            ..self.clone()
        }
    }

    /// This filter with the rating set to this chip, read the way the
    /// row is read. A rating chip replaces rather than joining, so
    /// here the count's question and the press's are the same one.
    pub fn only_stars(&self, n: u8) -> Filter {
        Filter {
            stars: Stars::AtLeast(n).read_as(self.stars.is_exact()),
            ..self.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// The names marked for the catalog are greycard-edit's own.
    #[test]
    fn the_marked_names_are_the_flags_and_the_labels() {
        let names: Vec<&str> = Flag::ALL
            .iter()
            .map(|f| f.name())
            .chain(Label::ALL.iter().map(|l| l.name()))
            .collect();
        assert_eq!(names, _MARK_NAMES);
        let lower: Vec<String> = _MARK_NAMES.iter().map(|n| n.to_lowercase()).collect();
        assert_eq!(lower, _MARK_LOWER_NAMES);
    }

    /// A folder to filter: a name, a rating, a flag, a label and
    /// some keywords each.
    fn folder() -> Vec<(PathBuf, Meta)> {
        let frame = |name: &str, rating, flag, label, words: &[&str]| {
            (
                PathBuf::from("/shoot").join(name),
                Meta {
                    rating,
                    flag,
                    label,
                    keywords: words.iter().map(|w| w.to_string()).collect(),
                    ..Meta::default()
                },
            )
        };
        vec![
            frame("IMG_0001.CR3", 0, Flag::None, Label::None, &[]),
            frame("IMG_0002.CR3", 3, Flag::Pick, Label::Red, &["harbor"]),
            frame("IMG_0003.CR3", 5, Flag::Pick, Label::Green, &["Sunset"]),
            frame("IMG_0004.CR3", 1, Flag::Reject, Label::None, &["sunset"]),
            frame(
                "DSC_0005.NEF",
                3,
                Flag::None,
                Label::Red,
                &["harbor", "dusk"],
            ),
        ]
    }

    /// Thirty-three frames flagged as the sample shoot is: fifteen
    /// nobody has said anything about, thirteen picks, five rejects.
    /// Nothing else is set, so the flag row is the only one with
    /// anything to say.
    fn shoot() -> Vec<(PathBuf, Meta)> {
        let flags = std::iter::repeat_n(Flag::None, 15)
            .chain(std::iter::repeat_n(Flag::Pick, 13))
            .chain(std::iter::repeat_n(Flag::Reject, 5));
        flags
            .enumerate()
            .map(|(i, flag)| {
                (
                    PathBuf::from("/shoot").join(format!("IMG_{i:04}.CR3")),
                    Meta {
                        flag,
                        ..Meta::default()
                    },
                )
            })
            .collect()
    }

    fn view(folder: &[(PathBuf, Meta)]) -> Vec<Frame<'_>> {
        folder
            .iter()
            .map(|(path, meta)| Frame {
                path,
                meta,
                index: true,
            })
            .collect()
    }

    #[test]
    fn each_chip_asks_its_own_question_and_a_frame_passes_them_all() {
        let folder = folder();
        let frames = view(&folder);
        // Nothing asked is the whole folder.
        assert!(Filter::default().is_empty());
        assert_eq!(Filter::default().apply(&frames), vec![0, 1, 2, 3, 4]);

        // At least three stars, and exactly three.
        let three = Filter {
            stars: Stars::AtLeast(3),
            ..Filter::default()
        };
        assert_eq!(three.apply(&frames), vec![1, 2, 4]);
        let exactly = Filter {
            stars: Stars::Exactly(3),
            ..Filter::default()
        };
        assert_eq!(exactly.apply(&frames), vec![1, 4]);
        let unrated = Filter {
            stars: Stars::Exactly(0),
            ..Filter::default()
        };
        assert_eq!(unrated.apply(&frames), vec![0]);

        // Flags, any-of: one chip, then two.
        let picks = Filter {
            flags: vec![Flag::Pick],
            ..Filter::default()
        };
        assert_eq!(picks.apply(&frames), vec![1, 2]);
        let no_rejects = Filter {
            flags: vec![Flag::None, Flag::Pick],
            ..Filter::default()
        };
        assert_eq!(no_rejects.apply(&frames), vec![0, 1, 2, 4]);

        // Labels, any-of, and "no label" is a chip like the rest.
        let warm = Filter {
            labels: vec![Label::Red, Label::Green],
            ..Filter::default()
        };
        assert_eq!(warm.apply(&frames), vec![1, 2, 4]);
        let bare = Filter {
            labels: vec![Label::None],
            ..Filter::default()
        };
        assert_eq!(bare.apply(&frames), vec![0, 3]);

        // All four at once, which is an and.
        let both = Filter {
            stars: Stars::AtLeast(3),
            flags: vec![Flag::Pick],
            labels: vec![Label::Red],
            text: "harbor".into(),
            ..Filter::default()
        };
        assert_eq!(both.apply(&frames), vec![1]);
    }

    #[test]
    fn the_text_looks_at_the_name_and_the_keywords_and_wants_every_word() {
        let folder = folder();
        let frames = view(&folder);
        let find = |text: &str| {
            Filter {
                text: text.into(),
                ..Filter::default()
            }
            .apply(&frames)
        };
        // The name, whole or in part, and case is nobody's business.
        assert_eq!(find("DSC"), vec![4]);
        assert_eq!(find("dsc_0005"), vec![4]);
        assert_eq!(find("img"), vec![0, 1, 2, 3]);
        // A keyword, whatever case it was put on in.
        assert_eq!(find("sunset"), vec![2, 3]);
        assert_eq!(find("SUNSET"), vec![2, 3]);
        // Two words narrow: both have to be somewhere, and they may
        // be in different places.
        assert_eq!(find("harbor dusk"), vec![4]);
        assert_eq!(find("harbor 0002"), vec![1]);
        assert_eq!(find("harbor sunset"), Vec::<usize>::new());
        // Space is not a query.
        assert_eq!(find("   "), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn a_chip_says_how_many_frames_carry_it_among_the_rest() {
        let folder = folder();
        let frames = view(&folder);

        // The rule, held against every chip of every group: a chip's
        // count is what the filter would show with that chip's own
        // group set to just that chip — the other groups as they
        // stand, its own set aside.
        let holds = |filter: &Filter| {
            let counts = Counts::of(filter, &frames);
            let what = filter.describe();
            assert_eq!(counts.total, frames.len(), "{what}");
            assert_eq!(counts.shown, filter.apply(&frames).len(), "{what}");
            for (n, &flag) in Flag::ALL.iter().enumerate() {
                let want = filter.only_flag(flag).apply(&frames).len();
                assert_eq!(counts.flags[n], want, "flag chip {n} of {what}");
            }
            for (n, &label) in Label::ALL.iter().enumerate() {
                let want = filter.only_label(label).apply(&frames).len();
                assert_eq!(counts.labels[n], want, "label chip {n} of {what}");
            }
            for n in 0..=STARS {
                let want = filter.only_stars(n).apply(&frames).len();
                assert_eq!(counts.stars[n as usize], want, "star chip {n} of {what}");
            }
            // And so a row adds up to what the other groups leave,
            // since a frame carries one flag and one label or none.
            // A row that adds up is a row that reads at a glance.
            let loose = Filter {
                flags: Vec::new(),
                ..filter.clone()
            };
            let sum: usize = counts.flags.iter().sum();
            assert_eq!(sum, loose.apply(&frames).len(), "the flags add up, {what}");
            let loose = Filter {
                labels: Vec::new(),
                ..filter.clone()
            };
            let sum: usize = counts.labels.iter().sum();
            assert_eq!(sum, loose.apply(&frames).len(), "the labels add up, {what}");
        };
        holds(&Filter::default());
        for flags in [
            vec![Flag::Pick],
            vec![Flag::Pick, Flag::Reject],
            vec![Flag::None, Flag::Pick],
        ] {
            holds(&Filter {
                flags,
                ..Filter::default()
            });
        }
        for labels in [
            vec![Label::Red],
            vec![Label::Red, Label::Green],
            vec![Label::None],
        ] {
            holds(&Filter {
                labels,
                ..Filter::default()
            });
        }
        holds(&Filter {
            stars: Stars::AtLeast(5),
            ..Filter::default()
        });
        holds(&Filter {
            stars: Stars::Exactly(3),
            ..Filter::default()
        });
        holds(&Filter {
            stars: Stars::AtLeast(3),
            flags: vec![Flag::Pick],
            labels: vec![Label::Red, Label::Green],
            text: "sunset".into(),
            ..Filter::default()
        });

        // And the numbers themselves, so the rule above is anchored
        // to something a reader can count on their fingers.
        let counts = Counts::of(&Filter::default(), &frames);
        assert_eq!(counts.total, 5);
        assert_eq!(counts.shown, 5);
        assert_eq!(counts.stars, [5, 4, 3, 3, 1, 1]);
        assert_eq!(counts.flags, [2, 2, 1]);
        assert_eq!(counts.labels, [2, 2, 0, 1, 0, 0]);

        // Picks only: the flag row is unmoved, because a flag chip's
        // count sets its own group aside, while the star and label
        // rows narrow to the two picks.
        let picks = Filter {
            flags: vec![Flag::Pick],
            ..Filter::default()
        };
        let counts = Counts::of(&picks, &frames);
        assert_eq!(counts.shown, 2);
        assert_eq!(counts.flags, [2, 2, 1]);
        assert_eq!(counts.stars, [2, 2, 2, 2, 1, 1]);
        assert_eq!(counts.labels, [0, 1, 0, 1, 0, 0]);

        // A chip that leads nowhere says nothing rather than a
        // number that turns out to be somebody else's.
        let five = Filter {
            stars: Stars::AtLeast(5),
            ..Filter::default()
        };
        let counts = Counts::of(&five, &frames);
        assert_eq!(counts.shown, 1);
        assert_eq!(counts.flags, [0, 1, 0]);
        assert_eq!(counts.labels, [0, 0, 0, 1, 0, 0]);

        // Under "exactly" the star chips are the ratings themselves.
        let exact = Filter {
            stars: Stars::Exactly(3),
            ..Filter::default()
        };
        let counts = Counts::of(&exact, &frames);
        assert_eq!(counts.shown, 2);
        assert_eq!(counts.stars, [1, 1, 0, 2, 0, 1]);
    }

    /// The case the rule exists for, on a folder shaped like the
    /// sample shoot: with the picks showing, the flag row still says
    /// how the shoot is flagged. The other reading — how many frames
    /// each press would leave — would say 28, 33 and 18 here, which
    /// is arithmetic about the filter and tells a culler nothing
    /// about the pictures. How many a press leaves is one number and
    /// the figure beside the text field is where it goes.
    #[test]
    fn the_flag_row_reads_the_shoot_and_the_press_is_read_beside_the_field() {
        let shoot = shoot();
        let frames = view(&shoot);
        let picks = Filter {
            flags: vec![Flag::Pick],
            ..Filter::default()
        };
        let counts = Counts::of(&picks, &frames);
        assert_eq!((counts.shown, counts.total), (13, 33));
        assert_eq!(counts.flags, [15, 13, 5], "the state of the cull");

        // Press Reject and it joins the picks: eighteen frames, and
        // the `N of M` beside the field is what says eighteen.
        let mut after = picks.clone();
        after.toggle_flag(Flag::Reject);
        assert_eq!(after.flags, vec![Flag::Pick, Flag::Reject]);
        assert_eq!(after.apply(&frames).len(), 18);
        assert_eq!(Counts::of(&after, &frames).shown, 18);
        // And the row still reads the shoot, unmoved by either chip.
        assert_eq!(Counts::of(&after, &frames).flags, [15, 13, 5]);
    }

    #[test]
    fn the_old_three_way_show_still_parses_and_still_means_what_it_did() {
        let folder = folder();
        let frames = view(&folder);
        let flags: Vec<Flag> = folder.iter().map(|(_, m)| m.flag).collect();
        for name in Filter::NAMES {
            let filter = Filter::from_name(name).expect("one of the three");
            let want: Vec<usize> = flags
                .iter()
                .enumerate()
                .filter(|(_, f)| match name {
                    "Picks" => **f == Flag::Pick,
                    "No rejects" => **f != Flag::Reject,
                    _ => true,
                })
                .map(|(i, _)| i)
                .collect();
            assert_eq!(filter.apply(&frames), want, "{name}");
        }
        assert_eq!(Filter::from_name("Rejects"), None);
        assert!(Filter::from_name("All").expect("all").is_empty());
    }

    /// `--filter` in the filter language: a facet term with `:` or
    /// `=` waits for the index to name its chips, and the rest is
    /// the text field's, ISO and the focal length among it, which have
    /// ranges and no chips.
    #[test]
    fn the_command_line_filter_splits_into_chips_and_text() {
        let want = |facet, value: &str, exact| Wanted {
            facet,
            value: value.to_string(),
            exact,
        };
        let (text, wanted) = from_cli("camera:R6 iso>=3200 rating>=3 focal=50mm harbor");
        assert_eq!(text, "iso>=3200 rating>=3 focal=50mm harbor");
        assert_eq!(wanted, vec![want(Facet::Camera, "R6", false)]);
        let (text, wanted) = from_cli("lens:\"RF 50mm\" date:2026-09 kw:dusk camera=\"EOS R5\"");
        assert_eq!(text, "");
        assert_eq!(
            wanted,
            vec![
                want(Facet::Lens, "RF 50mm", false),
                want(Facet::Date, "2026-09", false),
                want(Facet::Keyword, "dusk", false),
                want(Facet::Camera, "EOS R5", true)
            ]
        );
        // Not a facet, or not a chip's operator: text.
        let (text, wanted) = from_cli("aperture:2.8 iso!=100");
        assert_eq!(text, "aperture:2.8 iso!=100");
        assert!(wanted.is_empty());
        // A kind as the term reads it, wanted exactly; `!=` and a
        // kind there is not are the field's, which refuses the second.
        let (text, wanted) = from_cli("kind:raws kind=JPG kind!=raw kind:gif");
        assert_eq!(text, "kind!=raw kind:gif");
        assert_eq!(
            wanted,
            vec![
                want(Facet::Kind, "raw", true),
                want(Facet::Kind, "jpeg", true)
            ]
        );
        let field = Filter {
            text,
            ..Filter::default()
        };
        let typed = field.typed();
        assert_eq!(typed.meta.len(), 1);
        assert_eq!(typed.errors.len(), 1);
        assert!(
            typed.errors[0]
                .to_string()
                .contains("raw, jpeg, png or tiff")
        );
    }

    /// The kind is answered from the path, which every frame has,
    /// whether typed or a chip: a frame the index has no row for is
    /// held to it all the same, and nothing is asked of the index.
    /// Its chips read as a word or a format's capitals.
    #[test]
    fn a_kind_is_answered_from_the_path_typed_or_chipped() {
        let folder: Vec<(PathBuf, Meta)> = ["a.CR3", "b.jpg", "c.NEF", "d.tif", "e.JPEG"]
            .iter()
            .map(|n| (PathBuf::from("/shoot").join(n), Meta::default()))
            .collect();
        let typed = |text: &str| Filter {
            text: text.into(),
            ..Filter::default()
        };
        let raws = typed("kind:raw");
        assert_eq!(raws.typed().meta.len(), 1);
        assert!(!raws.asks_index(&raws.typed()), "the path is in hand");
        assert_eq!(raws.apply(&view(&folder)), vec![0, 2]);
        assert_eq!(typed("kind!=raw").apply(&view(&folder)), vec![1, 3, 4]);
        assert_eq!(typed("kind:jpg").apply(&view(&folder)), vec![1, 4]);

        let mut chips = Filter::default();
        chips.toggle_facet(Facet::Kind, "raw");
        assert!(!chips.asks_index(&chips.typed()), "the path is in hand");
        assert!(chips.index_filter(&chips.typed(), None).is_empty());
        // Every frame passes the index here, as one with no row does.
        assert_eq!(chips.apply(&view(&folder)), vec![0, 2]);
        chips.toggle_facet(Facet::Kind, "jpeg");
        assert_eq!(chips.apply(&view(&folder)), vec![0, 1, 2, 4]);
        // Its own row's count sets it aside; the others' keep it.
        let frames = view(&folder);
        assert!(chips.shows_in_hand(frames[3], Some(Facet::Kind)));
        assert!(!chips.shows_in_hand(frames[3], Some(Facet::Keyword)));
        assert_eq!(chips.describe(), "kind Raw or JPEG");
        assert_eq!(facet_caption(Facet::Kind), "Kind");
        let said: Vec<String> = greycard_library::filter::kinds()
            .into_iter()
            .map(|v| facet_chip_text(Facet::Kind, v))
            .collect();
        assert_eq!(said, ["Raw", "JPEG", "PNG", "TIFF"]);

        // `--filter kind:` needs no frame to name its chip: it is on
        // from the start, and the rest wait.
        let (_, wanted) = from_cli("kind:jpg camera:R6");
        let mut filter = Filter::default();
        let rest = filter.choose_at_once(wanted);
        assert_eq!(filter.chosen(Facet::Kind), ["jpeg".to_string()]);
        assert_eq!(rest.len(), 1);
        assert_eq!(rest[0].facet, Facet::Camera);
    }

    /// The field reads terms among its words, and a field with no
    /// term in it means exactly what the search box meant.
    #[test]
    fn the_text_field_reads_terms_and_leaves_words_as_they_were() {
        let typed = |text: &str| {
            Filter {
                text: text.into(),
                ..Filter::default()
            }
            .typed()
        };
        let t = typed("harbor Dusk \"low tide\" 12:30 foo:bar");
        assert_eq!(
            t.words,
            ["harbor", "dusk", "\"low", "tide\"", "12:30", "foo:bar"]
        );
        assert!(t.meta.is_empty() && t.index.is_empty() && t.errors.is_empty());
        let t = typed("camera:R6 rating>=3 iso>=3200 keyword:dusk lens:\"RF 50\"");
        assert!(t.words.is_empty());
        assert_eq!(t.meta.len(), 2, "rating and keyword are the sidecar's");
        assert_eq!(t.index.len(), 3, "camera, iso and lens are the index's");
        // One that does not parse is looked for as the word it was.
        let t = typed("rating>=9");
        assert_eq!(t.words, ["rating>=9"]);
        assert_eq!(t.errors.len(), 1);

        // A sidecar term narrows here; an index term is the index's.
        let folder = folder();
        let frames = view(&folder);
        let find = |text: &str| {
            Filter {
                text: text.into(),
                ..Filter::default()
            }
            .apply(&frames)
        };
        assert_eq!(find("rating>=3"), vec![1, 2, 4]);
        assert_eq!(find("rating>=3 flag:pick"), vec![1, 2]);
        assert_eq!(find("keyword=sunset"), vec![2, 3]);
        assert_eq!(find("label:red dusk"), vec![4]);
        assert_eq!(
            find("camera:R6"),
            vec![0, 1, 2, 3, 4],
            "no index, no answer"
        );
    }

    /// A facet's chips join their group as the flags do, and the
    /// index's answer arrives as each frame's `index` bit.
    #[test]
    fn a_facet_chip_joins_its_group_and_the_index_decides_the_rest() {
        let folder = folder();
        let mut filter = Filter::default();
        filter.toggle_facet(Facet::Camera, "Canon EOS R6");
        filter.toggle_facet(Facet::Camera, "Canon EOS R5");
        assert_eq!(
            filter.chosen(Facet::Camera),
            ["Canon EOS R6", "Canon EOS R5"]
        );
        assert!(!filter.is_empty());
        assert!(filter.asks_index(&filter.typed()));
        assert_eq!(filter.describe(), "camera Canon EOS R6 or Canon EOS R5");
        // The index's tests less the camera's own, for its count.
        let typed = filter.typed();
        assert_eq!(filter.index_filter(&typed, None).terms.len(), 1);
        assert!(filter.index_filter(&typed, Some(Facet::Camera)).is_empty());
        // Frames 1 and 3 fail the index; the others pass or have no
        // row yet, and show.
        let frames: Vec<Frame> = folder
            .iter()
            .enumerate()
            .map(|(i, (path, meta))| Frame {
                path,
                meta,
                index: i != 1 && i != 3,
            })
            .collect();
        assert_eq!(filter.apply(&frames), vec![0, 2, 4]);
        let counts = Counts::of(&filter, &frames);
        assert_eq!(counts.shown, 3);
        assert_eq!(counts.flags, [2, 1, 0], "the rows read what the index left");

        // The keyword chips are the sidecar's, any-of, case folded.
        let mut words = Filter::default();
        words.toggle_facet(Facet::Keyword, "harbor");
        assert!(!words.asks_index(&words.typed()), "the keyword is in hand");
        assert_eq!(words.apply(&view(&folder)), vec![1, 4]);
        words.toggle_facet(Facet::Keyword, "sunset");
        assert_eq!(words.apply(&view(&folder)), vec![1, 2, 3, 4]);
        words.toggle_facet(Facet::Keyword, "harbor");
        words.toggle_facet(Facet::Keyword, "sunset");
        assert!(words.is_empty(), "pressed twice is not pressed");
    }

    #[test]
    fn a_chip_pressed_twice_is_a_chip_not_pressed() {
        let mut filter = Filter::default();
        filter.toggle_flag(Flag::Pick);
        filter.toggle_flag(Flag::None);
        assert_eq!(filter.flags, vec![Flag::Pick, Flag::None]);
        filter.toggle_flag(Flag::Pick);
        assert_eq!(filter.flags, vec![Flag::None]);
        filter.toggle_flag(Flag::None);
        assert!(filter.is_empty(), "back to the whole folder");

        filter.toggle_label(Label::Red);
        filter.toggle_label(Label::Red);
        assert!(filter.is_empty());
    }

    /// A Ctrl+click holds a chip out: "everything but the rejects" is
    /// one chip, and a held-out chip beside ticked ones narrows them.
    /// Any press on a held-out chip lets it back to neither.
    #[test]
    fn a_chip_held_out_hides_what_it_names() {
        let folder = folder();
        let frames = view(&folder);
        let mut filter = Filter::default();
        filter.exclude_flag(Flag::Reject);
        assert_eq!(filter.not_flags, [Flag::Reject]);
        assert_eq!(filter.apply(&frames), vec![0, 1, 2, 4]);
        assert_eq!(filter.describe(), "not reject");
        // The row still reads the shoot: a count sets its own group
        // aside, held-out chips with the rest.
        let counts = Counts::of(&filter, &frames);
        assert_eq!(counts.flags, [2, 2, 1]);
        assert_eq!(counts.shown, 4);
        assert_eq!(
            counts.flags[2],
            filter.only_flag(Flag::Reject).apply(&frames).len()
        );
        // A plain click on it clears it; so does a second Ctrl+click.
        filter.toggle_flag(Flag::Reject);
        assert!(filter.is_empty());
        filter.exclude_flag(Flag::Reject);
        filter.exclude_flag(Flag::Reject);
        assert!(filter.is_empty());
        // A Ctrl+click on a ticked chip moves it across.
        filter.toggle_label(Label::Red);
        filter.exclude_label(Label::Red);
        assert!(filter.labels.is_empty());
        assert_eq!(filter.apply(&frames), vec![0, 2, 3]);
        // Ticked and held out in one group: Red or None, but not
        // what else is held out.
        filter.toggle_label(Label::Red);
        filter.toggle_label(Label::None);
        filter.exclude_label(Label::Red);
        assert_eq!(filter.labels, [Label::None]);
        assert_eq!(filter.not_labels, [Label::Red]);
        assert_eq!(filter.apply(&frames), vec![0, 3]);
        assert_eq!(filter.describe(), "no label, not red");
        filter.exclude_label(Label::None);
        assert_eq!(filter.describe(), "labeled, not red");
        filter.exclude_label(Label::None);

        // A keyword held out is answered in hand, case folded.
        let mut words = Filter::default();
        words.exclude_facet(Facet::Keyword, "sunset");
        assert!(!words.asks_index(&words.typed()));
        assert_eq!(words.apply(&frames), vec![0, 1, 4]);
        assert_eq!(words.describe(), "not keyword sunset");
        // An ISO held out goes to the index, as none of its values,
        // and its own count sets it aside.
        let mut iso = Filter::default();
        iso.exclude_facet(Facet::Iso, "6400");
        let typed = iso.typed();
        assert!(iso.asks_index(&typed));
        assert_eq!(
            iso.index_filter(&typed, None).terms,
            [Term::NotFacet {
                facet: Facet::Iso,
                values: vec!["6400".into()],
            }]
        );
        assert!(iso.index_filter(&typed, Some(Facet::Iso)).is_empty());
        assert_eq!(iso.describe(), "not ISO 6400");
        iso.toggle_facet(Facet::Iso, "6400");
        assert!(iso.is_empty(), "a plain click clears it");
    }

    #[test]
    fn the_rating_chips_are_named_for_the_reading_they_are_in() {
        let at_least: Vec<String> = (0..=STARS).map(|n| Stars::chip_name(n, false)).collect();
        assert_eq!(at_least, ["Any", "1+", "2+", "3+", "4+", "5"]);
        let exactly: Vec<String> = (0..=STARS).map(|n| Stars::chip_name(n, true)).collect();
        assert_eq!(exactly, ["0", "1", "2", "3", "4", "5"]);
        // The count survives a change of reading; the meaning does not.
        assert_eq!(Stars::AtLeast(3).read_as(true), Stars::Exactly(3));
        assert!(Stars::AtLeast(3).shows(5));
        assert!(!Stars::Exactly(3).shows(5));
        assert!(Stars::default().shows(0), "the default hides nothing");
    }

    #[test]
    fn the_filter_says_what_it_is_in_words() {
        assert_eq!(Filter::default().describe(), "all frames");
        assert_eq!(
            Filter {
                stars: Stars::AtLeast(4),
                flags: vec![Flag::Pick],
                labels: vec![Label::Red, Label::Green],
                text: " harbor ".into(),
                ..Filter::default()
            }
            .describe(),
            "4 stars or more, pick, red or green, \"harbor\""
        );
        assert_eq!(
            Filter {
                stars: Stars::Exactly(0),
                labels: vec![Label::None],
                ..Filter::default()
            }
            .describe(),
            "unrated, no label"
        );
        assert_eq!(
            Filter::from_name("No rejects")
                .expect("it parses")
                .describe(),
            "unflagged or pick"
        );
    }

    /// The filter kept between sessions comes back as it was left:
    /// every chip, the rows' reading, the text and the facets, by
    /// name. A facet a later build dropped is left out, not a panic.
    #[test]
    fn the_filter_kept_comes_back_as_it_was_left() {
        let mut filter = Filter {
            stars: Stars::Exactly(3),
            flags: vec![Flag::Pick, Flag::None],
            labels: vec![Label::Red],
            text: "harbor iso>=3200".into(),
            ..Filter::default()
        };
        filter.toggle_facet(Facet::Camera, "Canon EOS R6m2");
        filter.toggle_facet(Facet::Lens, "RF 35mm");
        filter.toggle_facet(Facet::Keyword, "dusk");
        filter.toggle_facet(Facet::Kind, "raw");
        filter.exclude_flag(Flag::Reject);
        filter.exclude_label(Label::Purple);
        filter.exclude_facet(Facet::Style, "Canon Faithful");
        let saved = Saved::of(&filter);
        let text = serde_json::to_string(&saved).unwrap();
        let back: Saved = serde_json::from_str(&text).unwrap();
        assert_eq!(back.filter(), filter);
        // None held out writes the shape it always did.
        let plain = serde_json::to_string(&Saved::of(&Filter::default())).unwrap();
        assert!(!plain.contains("not_"), "{plain}");
        // Cleared, it is kept empty, and comes back empty.
        assert_eq!(Saved::of(&Filter::default()).filter(), Filter::default());
        assert!(Saved::of(&Filter::default()).facets.is_empty());
        // A facet this build does not know, and stars past five.
        let odd: Saved = serde_json::from_str(
            r#"{"stars": 9, "facets": {"colour": ["red"], "lens": ["RF 50"]}}"#,
        )
        .unwrap();
        let read = odd.filter();
        assert_eq!(read.stars, Stars::AtLeast(STARS));
        assert_eq!(read.chosen(Facet::Lens), ["RF 50".to_string()]);
        assert_eq!(read.facets.iter().filter(|f| !f.is_empty()).count(), 1);
        // One kept by a build before the kind, whose facets sat in
        // other places on the bar: each comes back to its own.
        let before: Saved = serde_json::from_str(
            r#"{"stars": 2, "exact": false, "flags": [], "labels": [],
                "text": "harbor", "facets": {"camera": ["Canon EOS R5"],
                "style": ["Canon Faithful"], "keyword": ["dusk"]}}"#,
        )
        .unwrap();
        let read = before.filter();
        assert_eq!(read.chosen(Facet::Camera), ["Canon EOS R5".to_string()]);
        assert_eq!(read.chosen(Facet::Style), ["Canon Faithful".to_string()]);
        assert_eq!(read.chosen(Facet::Keyword), ["dusk".to_string()]);
        assert!(read.chosen(Facet::Kind).is_empty());
        assert_eq!(Saved::of(&read), before);
    }
}

#[cfg(test)]
mod range_tests {
    use super::*;

    const ISOS: [f64; 6] = [100.0, 200.0, 400.0, 800.0, 3200.0, 102400.0];

    fn ends(text: &str) -> (usize, usize, bool) {
        range_ends(text, NumberField::Iso, &ISOS)
    }

    /// No term is the whole range, off; each end alone moves its own.
    #[test]
    fn a_range_reads_its_ends_from_the_text() {
        assert_eq!(ends(""), (0, 5, false));
        assert_eq!(ends("harbor rating>=3"), (0, 5, false));
        assert_eq!(ends("iso>=400"), (2, 5, true));
        assert_eq!(ends("ISO<=800"), (0, 3, true));
        assert_eq!(ends("iso>=200 harbor iso<=3200"), (1, 4, true));
        // The tighter of two bounds on one end, in either order.
        assert_eq!(ends("iso>=200 iso>=800"), (3, 5, true));
        assert_eq!(ends("iso>=800 iso>=200"), (3, 5, true));
        assert_eq!(ends("iso<=800 iso<=200"), (0, 1, true));
        // Strict bounds pass the next value in.
        assert_eq!(ends("iso>400 iso<3200"), (3, 3, true));
        // An exact value bounds both ends.
        assert_eq!(ends("iso:800"), (3, 3, true));
        // A `!=` is no bound.
        assert_eq!(ends("iso!=800"), (0, 5, false));
        // The focal length's are the focal field's, and the reverse.
        assert_eq!(ends("focal>=50"), (0, 5, false));
    }

    /// A bound on a value the folder lacks puts its end on the next
    /// value the term lets through; bounds no value passes, on the
    /// value nearest the one asked.
    #[test]
    fn a_value_the_folder_lacks_snaps_to_one_it_has() {
        assert_eq!(ends("iso>=1000"), (4, 5, true));
        assert_eq!(ends("iso<=1000"), (0, 3, true));
        assert_eq!(ends("iso>=50 iso<=200000"), (0, 5, true));
        assert_eq!(ends("iso:1000"), (3, 3, true));
        assert_eq!(ends("iso>=200000"), (5, 5, true));
        assert_eq!(ends("iso<=50"), (0, 0, true));
        // A focal length through its tenth of a millimeter, as the
        // index compares it.
        let mm = [23.9, 35.0, 70.0];
        assert_eq!(
            range_ends("focal>=24", NumberField::Focal, &mm),
            (1, 2, true)
        );
        assert_eq!(
            range_ends("focal<=23.9 focal>=23.9", NumberField::Focal, &mm),
            (0, 0, true)
        );
    }

    /// Bounds that cross put both ends at the tightest low asked,
    /// whatever their order in the text; with no low, the tightest
    /// high.
    #[test]
    fn crossed_bounds_take_the_tightest() {
        assert_eq!(ends("iso>=200 iso>=3200 iso<=100"), (4, 4, true));
        assert_eq!(ends("iso<=100 iso>=3200 iso>=200"), (4, 4, true));
        assert_eq!(ends("iso<=800 iso<=50 iso<=3200"), (0, 0, true));
    }

    /// An older build's ISO and focal length chips, kept in the settings
    /// or a smart collection, load as the range's terms in the text: one
    /// value exactly, more from the lowest to the highest; a held-out
    /// value is dropped (see `ranged_terms`). The other facets' chips load as chips.
    #[test]
    fn kept_iso_and_focal_chips_load_as_the_ranges_terms() {
        let kept: Saved = serde_json::from_str(
            r#"{"stars": 0, "exact": false, "flags": [], "labels": [], "text": "\"harbor",
                "facets": {"iso": ["3200", "200", "800"], "focal": ["35"], "camera": ["R6"]},
                "not_facets": {"iso": ["6400"], "mm": ["50"]}}"#,
        )
        .unwrap();
        let read = kept.filter();
        assert_eq!(read.text, "\"harbor\" iso>=200 iso<=3200 focal:35");
        assert!(read.chosen(Facet::Iso).is_empty() && read.held_out(Facet::Iso).is_empty());
        assert!(read.chosen(Facet::Focal).is_empty() && read.held_out(Facet::Focal).is_empty());
        assert_eq!(read.chosen(Facet::Camera), ["R6".to_string()]);
        let typed = read.typed();
        assert!(typed.errors.is_empty(), "{:?}", typed.errors);
        assert_eq!(ends(&read.text), (1, 4, true));
        assert_eq!(
            range_ends(&read.text, NumberField::Focal, &[24.0, 35.0, 50.0]),
            (1, 1, true)
        );
        // Kept again, the terms are in the text and no chip is.
        let again = Saved::of(&read);
        assert!(!again.facets.contains_key("iso") && !again.not_facets.contains_key("mm"));
        assert_eq!(again.filter(), read);
        // `--filter` at launch: the same terms go to the text, under
        // the field's own name.
        let (rest, wanted) = from_cli("camera:R6 iso:800 mm>=24 harbor");
        assert_eq!(rest, "iso:800 focal>=24 harbor");
        // A word that names the field and asks nothing stays a word.
        assert_eq!(from_cli("mm iso focal: harbor").0, "mm iso focal: harbor");
        assert_eq!(wanted.len(), 1);
        assert_eq!(wanted[0].facet, Facet::Camera);
    }

    /// The held-out values dropped are logged when the filter is opened,
    /// once, and not each time it is read, as every rebuild reads every
    /// smart collection's.
    #[test]
    fn held_out_values_are_logged_once_when_opened() {
        use std::sync::{Arc, Mutex};
        #[derive(Clone, Default)]
        struct Lines(Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for Lines {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let kept: Saved = serde_json::from_str(
            r#"{"facets": {"iso": ["200"]}, "not_facets": {"iso": ["6400"], "focal": ["50"]}}"#,
        )
        .unwrap();
        let lines = Lines::default();
        let writer = lines.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(move || writer.clone())
            .with_ansi(false)
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            for _ in 0..5 {
                assert_eq!(kept.filter().text, "iso:200");
            }
            assert_eq!(kept.opened(), kept.filter());
        });
        let said = String::from_utf8(lines.0.lock().unwrap().clone()).unwrap();
        assert_eq!(
            said.matches("held out by an older build").count(),
            2,
            "{said}"
        );
        assert!(said.contains("iso values held out by an older build (6400)"));
        assert!(said.contains("focal values held out by an older build (50)"));
    }

    /// A moved end writes its own term only, none at the folder's own
    /// end, and leaves the other end's and every other byte as typed.
    #[test]
    fn a_moved_end_writes_its_term_and_leaves_the_rest() {
        let moved = |text, low_moved, low, high| with_range(text, Facet::Iso, low_moved, low, high);
        assert_eq!(moved("", true, "400", ""), "iso>=400");
        assert_eq!(moved("", false, "", "800"), "iso<=800");
        assert_eq!(
            moved("harbor  iso>=200 \"low tide\"", true, "400", ""),
            "harbor \"low tide\" iso>=400"
        );
        // A space typed at the end is the separator.
        assert_eq!(moved("harbor ", true, "400", ""), "harbor iso>=400");
        // Back to the folder's end: the term goes, with its gap.
        assert_eq!(moved("iso>=400   iso<=800 ", true, "", "800"), "iso<=800 ");
        // The other end as typed, a value the folder lacks included.
        assert_eq!(
            moved("iso<=1000\tiso!=200", true, "400", "800"),
            "iso<=1000\tiso!=200 iso>=400"
        );
        // An exact value bounds both, so both are written as shown.
        assert_eq!(moved("iso:800", true, "400", "800"), "iso>=400 iso<=800");
        assert_eq!(moved("iso=800", false, "800", ""), "iso>=800");
        // A focal length's own field.
        assert_eq!(
            with_range("focal>=24 iso>=400", Facet::Focal, true, "35", ""),
            "iso>=400 focal>=35"
        );
        // A facet with no range leaves the text alone.
        assert_eq!(with_range("a  b", Facet::Lens, true, "x", ""), "a  b");
    }

    /// A quote left open is closed before a term is written, so the
    /// term is not taken into the phrase.
    #[test]
    fn a_term_is_written_outside_an_open_quote() {
        let text = with_range("\"harbor", Facet::Iso, true, "400", "");
        assert_eq!(text, "\"harbor\" iso>=400");
        assert_eq!(ends(&text), (2, 5, true));
        let text = with_range("\"low tide ", Facet::Iso, false, "", "800");
        assert_eq!(text, "\"low tide \" iso<=800");
        assert_eq!(ends(&text), (0, 3, true));
    }

    /// The whole range asked for takes every bound on the field out,
    /// and nothing else.
    #[test]
    fn the_whole_range_takes_the_fields_bounds_out() {
        assert_eq!(
            without_range(
                "harbor iso>=400 iso<=800  iso:3200 iso!=100 focal>=24",
                Facet::Iso
            ),
            "harbor iso!=100 focal>=24"
        );
        assert_eq!(without_range(" iso>=400  harbor ", Facet::Iso), " harbor ");
        // With no bound to take out, the text as it was.
        assert_eq!(without_range("a  b ", Facet::Iso), "a  b ");
    }

    /// What a range writes, the filter reads back to the same ends.
    #[test]
    fn a_range_written_reads_back_where_it_was_set() {
        let text = with_range("", Facet::Iso, true, "200", "");
        let text = with_range(&text, Facet::Iso, false, "200", "3200");
        assert_eq!(ends(&text), (1, 4, true));
        let typed = Filter {
            text,
            ..Filter::default()
        }
        .typed();
        assert!(typed.errors.is_empty(), "{:?}", typed.errors);
        assert_eq!(typed.index.len(), 2);
    }

    /// Filters made up at random: the spans split as the parser does,
    /// and a range's splice leaves every byte of the tokens it keeps
    /// and of the gaps between them as typed, its terms at the end.
    #[test]
    fn a_splice_leaves_every_other_byte_as_typed() {
        const PIECES: [&str; 12] = [
            "harbor",
            "\"low tide\"",
            "iso>=400",
            "iso<=3200",
            "iso:800",
            "iso!=100",
            "focal>=24",
            "ISO>200",
            "iso<1600",
            "rating>=3",
            "é",
            "\"open",
        ];
        const GAPS: [&str; 5] = ["", " ", "  ", "\t", " \t "];
        let mut seed: u64 = 0x5eed;
        let mut next = |n: usize| {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) as usize % n
        };
        for _ in 0..2000 {
            let mut text = GAPS[next(GAPS.len())].to_string();
            for i in 0..next(6) {
                if i > 0 {
                    text.push_str(GAPS[1 + next(GAPS.len() - 1)]);
                }
                text.push_str(PIECES[next(PIECES.len())]);
            }
            text.push_str(GAPS[next(GAPS.len())]);
            let tokens: Vec<String> = spans(&text)
                .iter()
                .map(|(s, e)| text[*s..*e].to_string())
                .collect();
            assert_eq!(tokens, greycard_library::filter::tokens(&text), "{text:?}");

            let gone = |t: &str| bound_on(t, NumberField::Iso).is_some();
            let out = without_range(&text, Facet::Iso);
            kept_as_typed(&text, &out, &gone);

            let low_moved = next(2) == 0;
            let exact = tokens
                .iter()
                .any(|t| bound_on(t, NumberField::Iso).is_some_and(|(op, _)| op == Cmp::Eq));
            let gone = |t: &str| {
                bound_on(t, NumberField::Iso).is_some_and(|(op, _)| {
                    exact || if low_moved { is_low(op) } else { is_high(op) }
                })
            };
            let out = with_range(&text, Facet::Iso, low_moved, "400", "3200");
            let base = cut(&text, gone);
            kept_as_typed(&text, &base, &gone);
            assert!(out.starts_with(&base), "{text:?} -> {out:?}");
            let terms = match (low_moved || exact, !low_moved || exact) {
                (true, true) => "iso>=400 iso<=3200",
                (true, false) => "iso>=400",
                _ => "iso<=3200",
            };
            assert!(out.ends_with(terms), "{text:?} -> {out:?}");
            let read = greycard_library::filter::tokens(&out);
            let n = terms.split(' ').count();
            assert_eq!(
                read[read.len() - n..].join(" "),
                terms,
                "{text:?} -> {out:?}"
            );
        }
    }

    /// `out` is `text` with the tokens `gone` picks taken out: the
    /// tokens left in order, and each gap between two tokens left
    /// that were neighbors, the lead before a first token kept, and
    /// the tail after a last one, byte for byte.
    fn kept_as_typed(text: &str, out: &str, gone: &dyn Fn(&str) -> bool) {
        let before = spans(text);
        let after = spans(out);
        let kept: Vec<usize> = (0..before.len())
            .filter(|i| !gone(&text[before[*i].0..before[*i].1]))
            .collect();
        assert_eq!(kept.len(), after.len(), "{text:?} -> {out:?}");
        if kept.len() == before.len() {
            assert_eq!(out, text);
        }
        for (k, i) in kept.iter().enumerate() {
            let (s, e) = before[*i];
            assert_eq!(
                &out[after[k].0..after[k].1],
                &text[s..e],
                "{text:?} -> {out:?}"
            );
            if k > 0 && kept[k - 1] + 1 == *i {
                assert_eq!(
                    &out[after[k - 1].1..after[k].0],
                    &text[before[i - 1].1..s],
                    "{text:?} -> {out:?}"
                );
            }
        }
        if kept.first() == Some(&0) {
            assert_eq!(
                &out[..after[0].0],
                &text[..before[0].0],
                "{text:?} -> {out:?}"
            );
        }
        if let (Some(&l), Some(a)) = (kept.last(), after.last())
            && l + 1 == before.len()
        {
            assert_eq!(&out[a.1..], &text[before[l].1..], "{text:?} -> {out:?}");
        }
    }
}

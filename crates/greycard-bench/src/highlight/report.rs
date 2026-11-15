//! The tables: every case's rows as CSV, the pooled summary as CSV and as
//! Markdown, every score against both truths (see [`super::run`]).
//!
//! Pooling: the pixels of every case are pooled, each case's bucket (a
//! regime and size class of one frame at one overexposure) weighing at
//! most [`POOL_CAP`] pixels, so no one large frame decides a row.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use greycard_core::develop::segments::SegmentStats;

use super::run::{Domain, Record, Truth};
use super::score::{Bucket, POOL_CAP, RingSummary, Summary, summarize, summarize_ring};

/// What is known of a case besides its scores.
#[derive(Debug, Clone)]
pub struct CaseInfo {
    /// The frame's id in the frame list, never its file name.
    pub frame: String,
    pub camera: String,
    pub maker: String,
    pub sensor: &'static str,
    /// What the frame shows, from the frame list, or empty.
    pub kind: String,
    /// The overexposure, "1" to "3" stops, or "bracket N" for a real one.
    pub stops: String,
    /// Share of photosites the input clipped.
    pub clipped_share: f64,
    /// Scored pixels per regime and size class.
    pub counts: [[usize; 3]; 3],
    /// What the segment pass did.
    pub segments: Option<SegmentStats>,
}

/// A case and its records.
#[derive(Debug, Clone)]
pub struct CaseResult {
    pub info: CaseInfo,
    pub records: Vec<Record>,
}

impl CaseResult {
    fn is_bracket(&self) -> bool {
        self.info.stops.starts_with("bracket")
    }
}

/// A case as a log names it: "f01 at +2", or "b05 at bracket 1.0".
pub fn case_label(info: &CaseInfo) -> String {
    if info.stops.starts_with("bracket") {
        format!("{} at {}", info.frame, info.stops)
    } else {
        format!("{} at +{}", info.frame, info.stops)
    }
}

/// The synthetic cases missing a method some other synthetic case has
/// (an outside tool that failed on them), by [`case_label`]. A table
/// pooled over them would pool each method over different cases, so the
/// run leaves them out of the summary and fails at its end. Brackets are
/// not held to it: the outside tools never run on them.
pub fn incomplete(cases: &[CaseResult]) -> Vec<String> {
    let synthetic: Vec<&CaseResult> = cases.iter().filter(|c| !c.is_bracket()).collect();
    let mut all: Vec<&str> = synthetic
        .iter()
        .flat_map(|c| c.records.iter().map(|r| r.method.as_str()))
        .collect();
    all.sort_unstable();
    all.dedup();
    synthetic
        .iter()
        .filter(|c| {
            all.iter()
                .any(|m| !c.records.iter().any(|r| r.method == *m))
        })
        .map(|c| case_label(&c.info))
        .collect()
}

/// Which buckets a row pools: a regime (1 to 3) or all, a size class or
/// all.
pub type Slice = (Option<usize>, Option<usize>);

pub fn slices() -> Vec<Slice> {
    let mut v = vec![(None, None)];
    v.extend((0..3).map(|r| (Some(r), None)));
    v.extend((0..3).map(|s| (None, Some(s))));
    for r in 0..3 {
        for s in 0..3 {
            v.push((Some(r), Some(s)));
        }
    }
    v
}

fn picked(buckets: &[[Bucket; 3]; 3], (r, s): Slice) -> impl Iterator<Item = &Bucket> {
    buckets.iter().enumerate().flat_map(move |(ri, row)| {
        row.iter()
            .enumerate()
            .filter(move |(si, _)| r.is_none_or(|r| r == ri) && s.is_none_or(|s| s == *si))
            .map(|(_, b)| b)
    })
}

fn regime_name(r: Option<usize>) -> String {
    r.map_or("all".into(), |r| (r + 1).to_string())
}

fn size_name(s: Option<usize>) -> &'static str {
    match s {
        None => "all",
        Some(0) => "small",
        Some(1) => "medium",
        _ => "large",
    }
}

const HEADER: &str = "truth,ceiling,regime,size,n,med_abs_stops,p90_abs_stops,med_stops,mean_stops,med_dab,p90_dab,mean_da,mean_db,med_de,p90_de,structure,contrast,ring_n,ring_med_abs_stops,ring_p90_abs_stops,ring_med_dab,ring_step";

fn f(v: f64) -> String {
    if v.is_finite() {
        format!("{v:.4}")
    } else {
        String::new()
    }
}

fn summary_cells(s: &Summary) -> String {
    [
        s.n.to_string(),
        f(s.med_abs_stops),
        f(s.p90_abs_stops),
        f(s.med_stops),
        f(s.mean_stops),
        f(s.med_dab),
        f(s.p90_dab),
        f(s.mean_da),
        f(s.mean_db),
        f(s.med_de),
        f(s.p90_de),
        f(s.structure),
        f(s.contrast),
    ]
    .join(",")
}

fn ring_cells(r: Option<&RingSummary>) -> String {
    match r {
        Some(r) => [
            r.n.to_string(),
            f(r.med_abs_stops),
            f(r.p90_abs_stops),
            f(r.med_dab),
            f(r.step),
        ]
        .join(","),
        None => ",,,,".into(),
    }
}

/// Every case's rows: per method, domain and truth, every slice. The
/// ceiling is in units of the develop's (the largest gain).
pub fn rows_csv(cases: &[CaseResult]) -> String {
    let mut out = format!("frame,camera,sensor,kind,stops,clipped_share,method,domain,{HEADER}\n");
    for c in cases {
        let i = &c.info;
        for r in &c.records {
            let ring = r.ring.as_ref().map(|g| summarize_ring([g]));
            for slice in slices() {
                let s = summarize(picked(&r.buckets, slice), f64::INFINITY);
                if s.n == 0 {
                    continue;
                }
                let whole = slice == (None, None);
                let _ = writeln!(
                    out,
                    "{},{},{},{},{},{:.5},{},{},{},{},{},{},{},{}",
                    i.frame,
                    csv(&i.camera),
                    i.sensor,
                    csv(&i.kind),
                    i.stops,
                    i.clipped_share,
                    r.method,
                    r.domain.name(),
                    r.truth.name(),
                    f(f64::from(r.ceiling)),
                    regime_name(slice.0),
                    size_name(slice.1),
                    summary_cells(&s),
                    ring_cells(if whole { ring.as_ref() } else { None }),
                );
            }
        }
    }
    out
}

fn csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Groups the summary pools over: everything, then by overexposure, by
/// sensor, by maker, and by kind of scene; the brackets apart.
fn groups(cases: &[CaseResult]) -> Vec<(String, Vec<&CaseResult>)> {
    let mut out: Vec<(String, Vec<&CaseResult>)> = Vec::new();
    let synthetic: Vec<&CaseResult> = cases.iter().filter(|c| !c.is_bracket()).collect();
    out.push(("all".into(), synthetic.clone()));
    let mut by: BTreeMap<String, Vec<&CaseResult>> = BTreeMap::new();
    for c in &synthetic {
        by.entry(format!("stops={}", c.info.stops))
            .or_default()
            .push(c);
        by.entry(format!("sensor={}", c.info.sensor))
            .or_default()
            .push(c);
        by.entry(format!("maker={}", c.info.maker))
            .or_default()
            .push(c);
        if !c.info.kind.is_empty() {
            by.entry(format!("kind={}", c.info.kind))
                .or_default()
                .push(c);
        }
    }
    out.extend(by);
    let brackets: Vec<&CaseResult> = cases.iter().filter(|c| c.is_bracket()).collect();
    if !brackets.is_empty() {
        out.push(("brackets".into(), brackets));
    }
    out
}

/// Methods in the order they ran, with their domain.
fn methods(cases: &[CaseResult]) -> Vec<(String, Domain)> {
    let mut seen: Vec<(String, Domain)> = Vec::new();
    for c in cases {
        for r in &c.records {
            let key = (r.method.clone(), r.domain);
            if !seen.contains(&key) {
                seen.push(key);
            }
        }
    }
    seen
}

/// A method's ceiling as the tables state it.
fn ceiling(cases: &[&CaseResult], method: &str) -> String {
    let mut v: Vec<f32> = cases
        .iter()
        .flat_map(|c| c.records.iter())
        .filter(|r| r.method == method)
        .map(|r| r.ceiling)
        .collect();
    if v.is_empty() || v.iter().all(|c| c.is_nan()) {
        return "–".into();
    }
    v.retain(|c| !c.is_nan());
    let (lo, hi) = v
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), c| (a.min(*c), b.max(*c)));
    if (hi - lo).abs() < 1e-3 {
        format!("×{lo:.2}")
    } else {
        format!("×{lo:.2}–{hi:.1}")
    }
}

/// Pool one method's records over `cases` for a truth and a slice.
fn pooled(
    cases: &[&CaseResult],
    method: &str,
    domain: Domain,
    truth: Truth,
    slice: Slice,
) -> Summary {
    summarize(
        cases
            .iter()
            .flat_map(|c| c.records.iter())
            .filter(|r| r.method == method && r.domain == domain && r.truth == truth)
            .flat_map(|r| picked(&r.buckets, slice)),
        POOL_CAP,
    )
}

fn pooled_ring(cases: &[&CaseResult], method: &str) -> RingSummary {
    summarize_ring(
        cases
            .iter()
            .flat_map(|c| c.records.iter())
            .filter(|r| r.method == method && r.truth == Truth::Raw)
            .filter_map(|r| r.ring.as_ref()),
    )
}

/// The pooled summary, every group, method, domain, truth and slice.
pub fn summary_csv(cases: &[CaseResult]) -> String {
    let mut out = format!("group,cases,method,domain,{HEADER}\n");
    for (group, members) in groups(cases) {
        for (method, domain) in methods(cases) {
            let ring = (domain == Domain::Image).then(|| pooled_ring(&members, &method));
            for truth in Truth::ALL {
                let c = members
                    .iter()
                    .flat_map(|c| c.records.iter())
                    .find(|r| r.method == method)
                    .map_or(f32::NAN, |r| r.ceiling);
                for slice in slices() {
                    let s = pooled(&members, &method, domain, truth, slice);
                    if s.n == 0 {
                        continue;
                    }
                    let whole = slice == (None, None) && truth == Truth::Raw;
                    let _ = writeln!(
                        out,
                        "{},{},{},{},{},{},{},{},{},{}",
                        csv(&group),
                        members.len(),
                        method,
                        domain.name(),
                        truth.name(),
                        f(f64::from(c)),
                        regime_name(slice.0),
                        size_name(slice.1),
                        summary_cells(&s),
                        ring_cells(if whole { ring.as_ref() } else { None }),
                    );
                }
            }
        }
    }
    out
}

fn m(v: f64, digits: usize) -> String {
    if v.is_finite() {
        format!("{v:.digits$}")
    } else {
        "–".into()
    }
}

fn pair(a: f64, b: f64, digits: usize) -> String {
    format!("{} / {}", m(a, digits), m(b, digits))
}

const BOTH_HEAD: &str = "| | ceiling | pixels | med abs stops | bias (med stops) | med dab | med dE | contrast |\n|---|---|---:|---:|---:|---:|---:|---:|\n";

/// A row against both truths: raw / capped in every cell.
fn both_row(label: &str, ceiling: &str, raw: &Summary, capped: &Summary) -> String {
    format!(
        "| {label} | {ceiling} | {} | {} | {} | {} | {} | {} |\n",
        raw.n,
        pair(raw.med_abs_stops, capped.med_abs_stops, 2),
        pair(raw.med_stops, capped.med_stops, 2),
        pair(raw.med_dab, capped.med_dab, 3),
        pair(raw.med_de, capped.med_de, 3),
        pair(raw.contrast, capped.contrast, 2),
    )
}

const GROUP_HEAD: &str = "| | pixels | med abs stops | bias | med dab | med dE raw / capped | contrast |\n|---|---:|---:|---:|---:|---:|---:|\n";

fn group_row(label: &str, raw: &Summary, capped: &Summary) -> String {
    format!(
        "| {label} | {} | {} | {} | {} | {} | {} |\n",
        raw.n,
        m(raw.med_abs_stops, 2),
        m(raw.med_stops, 2),
        m(raw.med_dab, 3),
        pair(raw.med_de, capped.med_de, 3),
        m(raw.contrast, 2),
    )
}

fn both_table(
    out: &mut String,
    cases: &[&CaseResult],
    image: &[&String],
    rows: &[(Slice, String)],
) {
    out.push_str(BOTH_HEAD);
    for (slice, name) in rows {
        for method in image {
            let raw = pooled(cases, method, Domain::Image, Truth::Raw, *slice);
            let capped = pooled(cases, method, Domain::Image, Truth::Capped, *slice);
            if raw.n > 0 {
                out.push_str(&both_row(
                    &format!("{method}, {name}"),
                    &ceiling(cases, method),
                    &raw,
                    &capped,
                ));
            }
        }
    }
}

/// The readable summary.
pub fn summary_md(cases: &[CaseResult], preamble: &str) -> String {
    let mut out = String::new();
    out.push_str(preamble);
    let all: Vec<&CaseResult> = cases.iter().filter(|c| !c.is_bracket()).collect();
    let methods = methods(cases);
    let image: Vec<&String> = methods
        .iter()
        .filter(|(_, d)| *d == Domain::Image)
        .map(|(m, _)| m)
        .collect();
    let regimes: Vec<(Slice, String)> = [None, Some(0), Some(1), Some(2)]
        .into_iter()
        .map(|r| ((r, None), format!("regime {}", regime_name(r))))
        .collect();
    let sizes: Vec<(Slice, String)> = [Some(0), Some(1), Some(2)]
        .into_iter()
        .map(|s| ((None, s), size_name(s).to_string()))
        .collect();

    let _ = writeln!(
        out,
        "\n## By regime, against both truths (working space, {} synthetic cases)\n\nEach cell: against the raw truth (what is lost) / against the truth cut at the develop's ceiling (what a method can do within today's engine). Ceiling in units of the develop's, the largest gain.\n",
        all.len()
    );
    both_table(&mut out, &all, &image, &regimes);

    let _ = writeln!(
        out,
        "\n## By region size, against both truths (all regimes)\n"
    );
    both_table(&mut out, &all, &image, &sizes);

    let _ = writeln!(out, "\n## Regime by size, med dE raw / capped (pixels)\n");
    let _ = write!(out, "| method | regime |");
    for sz in [0, 1, 2] {
        let _ = write!(out, " {} |", size_name(Some(sz)));
    }
    let _ = writeln!(out, "\n|---|---|---:|---:|---:|");
    for method in &image {
        for r in 0..3 {
            let _ = write!(out, "| {method} | {} |", r + 1);
            for sz in 0..3 {
                let s = (Some(r), Some(sz));
                let raw = pooled(&all, method, Domain::Image, Truth::Raw, s);
                let capped = pooled(&all, method, Domain::Image, Truth::Capped, s);
                let _ = write!(out, " {} ({}) |", pair(raw.med_de, capped.med_de, 3), raw.n);
            }
            out.push('\n');
        }
    }

    for (title, prefix) in [
        ("By overexposure", "stops="),
        ("By sensor", "sensor="),
        ("By maker", "maker="),
        ("By kind of scene", "kind="),
    ] {
        let gs: Vec<(String, Vec<&CaseResult>)> = groups(cases)
            .into_iter()
            .filter(|(g, _)| g.starts_with(prefix))
            .collect();
        if gs.is_empty() {
            continue;
        }
        let _ = writeln!(
            out,
            "\n## {title} (all regimes; against the raw truth, dE against both)\n"
        );
        out.push_str(GROUP_HEAD);
        for (g, members) in &gs {
            for method in &image {
                let raw = pooled(members, method, Domain::Image, Truth::Raw, (None, None));
                let capped = pooled(members, method, Domain::Image, Truth::Capped, (None, None));
                if raw.n > 0 {
                    out.push_str(&group_row(
                        &format!("{} ({} cases), {method}", &g[prefix.len()..], members.len()),
                        &raw,
                        &capped,
                    ));
                }
            }
        }
    }

    let _ = writeln!(
        out,
        "\n## The ring at the clip boundary\n\nUnclipped pixels within 3 px of a clipped cell; step = median signed stops just inside less just outside.\n\n| method | pixels | med abs stops | p90 abs stops | med dab | step |\n|---|---:|---:|---:|---:|---:|"
    );
    for method in &image {
        let r = pooled_ring(&all, method);
        let _ = writeln!(
            out,
            "| {method} | {} | {} | {} | {} | {} |",
            r.n,
            m(r.med_abs_stops, 3),
            m(r.p90_abs_stops, 3),
            m(r.med_dab, 4),
            m(r.step, 3)
        );
    }

    let _ = writeln!(
        out,
        "\n## Mosaic domain (clipped photosites, white balanced), raw / capped truth\n\n| method, regime | ceiling | photosites | med abs stops | p90 abs stops | bias (med stops) |\n|---|---|---:|---:|---:|---:|"
    );
    for (method, domain) in &methods {
        if *domain != Domain::Mosaic {
            continue;
        }
        for r in [None, Some(0), Some(1), Some(2)] {
            let raw = pooled(&all, method, Domain::Mosaic, Truth::Raw, (r, None));
            let capped = pooled(&all, method, Domain::Mosaic, Truth::Capped, (r, None));
            if raw.n > 0 {
                let _ = writeln!(
                    out,
                    "| {method}, {} | {} | {} | {} | {} | {} |",
                    regime_name(r),
                    ceiling(&all, method),
                    raw.n,
                    pair(raw.med_abs_stops, capped.med_abs_stops, 2),
                    pair(raw.p90_abs_stops, capped.p90_abs_stops, 2),
                    pair(raw.med_stops, capped.med_stops, 2)
                );
            }
        }
    }

    let runs: Vec<&SegmentStats> = all
        .iter()
        .filter_map(|c| c.info.segments.as_ref())
        .collect();
    if !runs.is_empty() {
        let changed = runs.iter().filter(|s| s.replaced > 0).count();
        let segs: usize = runs.iter().map(|s| s.segments.iter().sum::<usize>()).sum();
        let cands: usize = runs
            .iter()
            .map(|s| s.candidates.iter().sum::<usize>())
            .sum();
        let replaced: usize = runs.iter().map(|s| s.replaced).sum();
        let _ = writeln!(
            out,
            "\n## The segment pass\n\nIt changed a photosite in {changed} of {} cases; over all cases it formed {segs} segments, found a candidate spot for {cands}, and replaced {replaced} photosites.\n",
            runs.len()
        );
    }

    let brackets: Vec<&CaseResult> = cases.iter().filter(|c| c.is_bracket()).collect();
    if !brackets.is_empty() {
        let _ = writeln!(
            out,
            "\n## Real brackets ({} pairs), against both truths\n",
            brackets.len()
        );
        both_table(&mut out, &brackets, &image, &regimes);
    }

    let _ = writeln!(
        out,
        "\n## Per case (working space, all regimes: med dE raw / capped)\n"
    );
    let _ = write!(
        out,
        "| case | camera | sensor | kind | stops | clipped % | r1 / r2 / r3 px | segments: replaced, candidates |"
    );
    for method in &image {
        let _ = write!(out, " {method} |");
    }
    let _ = write!(out, "\n|---|---|---|---|---|---:|---|---|");
    for _ in &image {
        let _ = write!(out, "---:|");
    }
    out.push('\n');
    for c in cases {
        let i = &c.info;
        let per: [usize; 3] = std::array::from_fn(|r| i.counts[r].iter().sum());
        let seg = i.segments.map_or("–".into(), |s| {
            format!("{}, {}", s.replaced, s.candidates.iter().sum::<usize>())
        });
        let _ = write!(
            out,
            "| {} | {} | {} | {} | {} | {:.2} | {} / {} / {} | {seg} |",
            i.frame,
            i.camera,
            i.sensor,
            i.kind,
            i.stops,
            100.0 * i.clipped_share,
            per[0],
            per[1],
            per[2]
        );
        for method in &image {
            let de = |t: Truth| {
                c.records
                    .iter()
                    .find(|r| &&r.method == method && r.domain == Domain::Image && r.truth == t)
                    .map(|r| summarize(r.buckets.iter().flatten(), f64::INFINITY).med_de)
                    .unwrap_or(f64::NAN)
            };
            let _ = write!(out, " {} |", pair(de(Truth::Raw), de(Truth::Capped), 3));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slices_cover_every_bucket_once_at_the_finest_level() {
        let s = slices();
        assert_eq!(s.len(), 16);
        let finest = s.iter().filter(|(r, z)| r.is_some() && z.is_some()).count();
        assert_eq!(finest, 9);
        let mut b: [[Bucket; 3]; 3] = Default::default();
        b[1][2].n = 5;
        assert_eq!(picked(&b, (Some(1), None)).map(|b| b.n).sum::<usize>(), 5);
        assert_eq!(picked(&b, (None, Some(2))).map(|b| b.n).sum::<usize>(), 5);
        assert_eq!(picked(&b, (Some(0), None)).map(|b| b.n).sum::<usize>(), 0);
    }

    /// A case an outside tool failed on lacks its methods and is named;
    /// a bracket, which the tools never see, is not.
    #[test]
    fn a_case_missing_a_method_is_incomplete() {
        let case = |frame: &str, stops: &str, methods: &[&str]| CaseResult {
            info: CaseInfo {
                frame: frame.into(),
                camera: String::new(),
                maker: String::new(),
                sensor: "bayer",
                kind: String::new(),
                stops: stops.into(),
                clipped_share: 0.0,
                counts: Default::default(),
                segments: None,
            },
            records: methods
                .iter()
                .map(|m| Record {
                    method: (*m).into(),
                    domain: Domain::Image,
                    truth: Truth::Raw,
                    ceiling: 1.0,
                    buckets: Default::default(),
                    ring: None,
                })
                .collect(),
        };
        let cases = [
            case("f01", "1", &["opposed", "dt-opposed"]),
            case("f01", "2", &["opposed"]),
            case("f02", "2", &["opposed", "dt-opposed"]),
            case("b01", "bracket 1.0", &["opposed"]),
        ];
        assert_eq!(incomplete(&cases), ["f01 at +2"]);
        assert!(incomplete(&cases[2..]).is_empty());
    }
}

//! One case: a clipped input, its truths, where it was clipped; every
//! method developed and scored against both truths.
//!
//! The two truths: the frame scaled, nothing cut ("raw": what is lost),
//! and the same mosaic cut at the develop's ceiling, the largest gain,
//! before the demosaic ("capped": what a method can do within today's
//! engine, which cuts every reconstruction there). A fully clipped
//! region's truth lies almost entirely above that ceiling, so against
//! the raw truth regime 3 measures the ceiling, and only against the
//! capped one does it measure the method.

use anyhow::Result;
use greycard_core::WhitePoint;
use greycard_core::develop::highlights::{CLIP_MAGIC, inpaint_opposed};
use greycard_core::develop::segments::SegmentStats;
use greycard_core::develop::{DemosaicMethod, HighlightMode};
use greycard_core::raw::{CfaPattern, RawFrame};

use super::clipmap::ClipMap;
use super::oracle;
use super::score::{Bucket, ImageScore, Ring, score_image, score_mosaic, smooth_log_luma, strides};
use super::sheet::SheetWriter;
use super::sim::{
    Developed, Scene, develop_mode, develop_mosaic, develop_scaled, develop_truths, lowered,
    prepare_scaled,
};

/// Where a record's scores were taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Domain {
    /// The working space, after the shared demosaic and matrix.
    Image,
    /// The white-balanced mosaic, before the demosaic.
    Mosaic,
}

impl Domain {
    pub fn name(self) -> &'static str {
        match self {
            Domain::Image => "image",
            Domain::Mosaic => "mosaic",
        }
    }
}

/// Which truth a record was scored against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Truth {
    /// Nothing cut: what is lost.
    Raw,
    /// Cut at the develop's ceiling in the mosaic: what a method can do
    /// within today's engine.
    Capped,
}

impl Truth {
    pub const BOTH: [Truth; 2] = [Truth::Raw, Truth::Capped];

    pub fn name(self) -> &'static str {
        match self {
            Truth::Raw => "raw",
            Truth::Capped => "capped",
        }
    }
}

/// One method's scores on one case against one truth.
#[derive(Debug, Clone)]
pub struct Record {
    pub method: String,
    pub domain: Domain,
    pub truth: Truth,
    /// The ceiling the method's mosaic was cut at, in units of the
    /// develop's (the largest gain): 1 for the engine's methods, below
    /// for clip (it cuts at 1.0), above for a raised one; NaN for a
    /// picture made in the working space.
    pub ceiling: f32,
    pub buckets: [[Bucket; 3]; 3],
    pub ring: Option<Ring>,
}

/// The engine's methods, by the name the tables give them.
pub const METHODS: [(&str, HighlightMode); 3] = [
    ("clip", HighlightMode::Clip),
    ("opposed", HighlightMode::InpaintOpposed),
    ("segments", HighlightMode::Segments),
];

/// The method the oracles start from: the engine's default.
pub const ORACLE_BASE: &str = "opposed";

/// A case ready to score.
pub struct Case {
    /// What the methods develop: the frame with its white lowered, or
    /// the brighter frame of a bracket.
    pub input: RawFrame,
    pub pattern: CfaPattern,
    /// The truth, nothing cut.
    pub truth: Developed,
    /// The truth cut at [`Case::ceiling`] in the mosaic.
    pub capped: Developed,
    /// The develop's ceiling, white-balanced mosaic units.
    pub ceiling: f32,
    pub map: ClipMap,
    pub demosaic: DemosaicMethod,
}

/// What the segment pass did on a case.
pub type SegmentRun = SegmentStats;

/// The demosaic a mosaic's cases share: RCD for Bayer, near enough
/// scale-free that a truth several stops above the clip takes the
/// same path through it as a reconstruction at the clip; Markesteijn's
/// three passes for X-Trans.
pub fn demosaic_for(pattern: &CfaPattern) -> DemosaicMethod {
    if pattern.is_xtrans() {
        DemosaicMethod::Markesteijn3
    } else {
        DemosaicMethod::Rcd
    }
}

/// A photosite the original frame read at or above this share of its
/// white is not truth.
pub const UNRELIABLE: f32 = 0.95;

/// The synthetic case: `scene` overexposed by `stops`. A photosite is
/// clipped where the overexposure puts it at or above the engine's own
/// threshold, [`CLIP_MAGIC`] of the white, as the bracket path reads it.
pub fn synthetic_case(scene: &Scene, stops: f32) -> Result<Case> {
    let factor = 2f32.powf(stops);
    let demosaic = demosaic_for(&scene.pattern);
    let input = lowered(&scene.frame, stops)?;
    let (truth, capped, ceiling) =
        develop_truths(&scene.frame, factor, demosaic, WhitePoint::AsShot)?;
    let n = &scene.normalized;
    let map = ClipMap::build(
        scene.width,
        scene.height,
        &scene.pattern,
        |i| n[i] * factor >= CLIP_MAGIC,
        |i| n[i] >= UNRELIABLE,
    );
    Ok(Case {
        input,
        pattern: scene.pattern.clone(),
        truth,
        capped,
        ceiling,
        map,
        demosaic,
    })
}

/// Scores against both truths, built once a case.
struct Scorer<'a> {
    case: &'a Case,
    stride: [[u64; 3]; 3],
    logs: [Vec<f32>; 2],
}

impl<'a> Scorer<'a> {
    fn new(case: &'a Case) -> Self {
        let (w, h) = (case.map.width, case.map.height);
        Self {
            case,
            stride: strides(&case.map.counts()),
            logs: [
                smooth_log_luma(&case.truth.rgb, w, h),
                smooth_log_luma(&case.capped.rgb, w, h),
            ],
        }
    }

    fn truth(&self, t: Truth) -> (&Developed, &[f32]) {
        match t {
            Truth::Raw => (&self.case.truth, &self.logs[0]),
            Truth::Capped => (&self.case.capped, &self.logs[1]),
        }
    }

    /// `rgb` against both truths, and `mosaic` too when there is one.
    fn score(&self, name: &str, ceiling: f32, rgb: &[f32], mosaic: Option<&[f32]>) -> Vec<Record> {
        let map = &self.case.map;
        let log = smooth_log_luma(rgb, map.width, map.height);
        let mut out = Vec::new();
        for t in Truth::BOTH {
            let (truth, truth_log) = self.truth(t);
            let ImageScore { buckets, ring } =
                score_image(map, &truth.rgb, rgb, truth_log, &log, &self.stride);
            out.push(Record {
                method: name.to_string(),
                domain: Domain::Image,
                truth: t,
                ceiling,
                buckets,
                ring: Some(ring),
            });
            if let Some(m) = mosaic {
                out.push(Record {
                    method: name.to_string(),
                    domain: Domain::Mosaic,
                    truth: t,
                    ceiling,
                    buckets: score_mosaic(map, &truth.mosaic, m, &self.stride),
                    ring: None,
                });
            }
        }
        out
    }
}

/// Score every method (and, with `oracles`, the oracles and the raised
/// ceiling) on `case`; with a sheet, add each picture to it as it is
/// made. Returns the records and what the segment pass did.
pub fn score_case(
    case: &Case,
    oracles: bool,
    mut sheet: Option<&mut SheetWriter>,
) -> Result<(Vec<Record>, Option<SegmentRun>)> {
    let map = &case.map;
    let scorer = Scorer::new(case);
    // Ceilings are stated in units of the develop's.
    let unit = |c: f32| c / case.ceiling;
    if let Some(s) = sheet.as_deref_mut() {
        s.add("truth", &case.truth.rgb)?;
        s.add("truth at the ceiling", &case.capped.rgb)?;
        let input = develop_scaled(&case.input, 1.0, case.demosaic, WhitePoint::AsShot)?;
        s.add("input as clipped", &input.rgb)?;
    }
    let mut records = Vec::new();
    let mut segments = None;
    let mut base = None;
    for (name, mode) in METHODS {
        let d = develop_mode(&case.input, mode, case.demosaic)?;
        let ceiling = if mode == HighlightMode::Clip {
            unit(1.0)
        } else {
            1.0
        };
        records.extend(scorer.score(name, ceiling, &d.rgb, Some(&d.mosaic)));
        if let Some(s) = sheet.as_deref_mut() {
            s.add(name, &d.rgb)?;
        }
        if mode == HighlightMode::Segments {
            segments = d.segments;
        }
        if name == ORACLE_BASE {
            base = Some(d.rgb);
        }
    }
    // The truth at the ceiling as a method: the floor under every method
    // the engine can run today, scored against the raw truth.
    records.extend(scorer.score(
        "truth-at-ceiling",
        1.0,
        &case.capped.rgb,
        Some(&case.capped.mosaic),
    ));

    // Opposed with the ceiling lifted to the truth's brightest: what
    // raising the ceiling alone would win.
    let prepared = prepare_scaled(&case.input, 1.0, case.demosaic, WhitePoint::AsShot)?;
    let gains = prepared.gains;
    let input = prepared.samples.clone();
    let clips = gains.map(|g| g * CLIP_MAGIC);
    let (lifted, _) = inpaint_opposed(&input, map.width, map.height, &case.pattern, clips)?;
    let raised = case
        .truth
        .mosaic
        .iter()
        .copied()
        .fold(case.ceiling, f32::max);
    let d = develop_mosaic(
        &case.input,
        prepared.clone(),
        lifted.clone(),
        raised,
        case.demosaic,
    )?;
    records.extend(scorer.score("opposed-raised", unit(raised), &d.rgb, Some(&d.mosaic)));
    if let Some(s) = sheet.as_deref_mut() {
        s.add("opposed-raised", &d.rgb)?;
    }
    drop(d);

    if oracles {
        let base = base.expect("the oracles' base method ran");
        let l = oracle::luma(map, &case.truth.rgb, &base);
        records.extend(scorer.score("oracle-luma", f32::NAN, &l, None));
        if let Some(s) = sheet.as_deref_mut() {
            s.add("oracle-luma", &l)?;
        }
        drop(l);
        let c = oracle::chroma(map, &case.truth.rgb, &base);
        records.extend(scorer.score("oracle-chroma", f32::NAN, &c, None));
        if let Some(s) = sheet.as_deref_mut() {
            s.add("oracle-chroma", &c)?;
        }
        drop(c);
        drop(base);

        let m = oracle::offset(map, &input, &case.truth.mosaic, &case.pattern, gains);
        let d = develop_mosaic(
            &case.input,
            prepared.clone(),
            m,
            case.ceiling,
            case.demosaic,
        )?;
        records.extend(scorer.score("oracle-offset", 1.0, &d.rgb, Some(&d.mosaic)));
        if let Some(s) = sheet.as_deref_mut() {
            s.add("oracle-offset", &d.rgb)?;
        }
        drop(d);

        // Opposed where anything is left to reconstruct from, the truth
        // where every color clipped, the ceiling raised: the headroom of
        // a method that invents a fully clipped region's light.
        let m = oracle::fully_clipped(map, &lifted, &case.truth.mosaic);
        let d = develop_mosaic(&case.input, prepared, m, raised, case.demosaic)?;
        records.extend(scorer.score("oracle-raised", unit(raised), &d.rgb, Some(&d.mosaic)));
        if let Some(s) = sheet {
            s.add("oracle-raised", &d.rgb)?;
        }
    }
    Ok((records, segments))
}

/// Score an outside developer's picture of the case (working space,
/// fitted to the engine's) as one more method, against both truths.
pub fn score_external(case: &Case, name: &str, rgb: &[f32]) -> Vec<Record> {
    Scorer::new(case).score(name, f32::NAN, rgb, None)
}

//! `greycard merge hdr` and `greycard merge pano`: a tripod bracket, or
//! a panorama's frames, merged into one linear DNG beside the first
//! frame, through the engine's merge, the one the window's Merge to HDR
//! and Merge to Panorama run.

use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use greycard_core::output::OnExists;
use greycard_engine::merge::{self, CaFits, Options, Step};

use crate::common;

#[derive(Subcommand)]
pub(crate) enum MergeKind {
    /// Merge a tripod bracket into one frame of its whole range: each
    /// frame put on the reference's scale by its exposure (from EXIF,
    /// then measured), its clipped samples dropped, the rest averaged
    /// by their noise, and written as a linear DNG with float samples
    /// beside the first frame (`<first>-HDR.dng`). Exits 0 when the
    /// bracket merged as a tripod's should; 2 when it was written but
    /// a frame had moved and was resampled, or an exposure could not
    /// be measured, each said; 1 when nothing was written
    Hdr(HdrArgs),
    /// Merge a panorama's frames into one picture: their lens corrected
    /// from its profile, each frame placed by its corners and turned
    /// about the lens onto the reference's view, laid on a plane or,
    /// past ninety degrees, a cylinder, their exposures matched where
    /// they overlap, the seams led around what moved, and the frames
    /// blended band by band, cropped to the largest rectangle they
    /// cover, and written as a linear DNG with float samples beside the
    /// first frame (`<first>-pano.dng`). Exits 0 when it merged as
    /// asked; 2 when it was written but went without something, each
    /// said; 1 when nothing was written
    Pano(PanoArgs),
}

#[derive(Args)]
pub(crate) struct PanoArgs {
    /// The panorama's frames, in the order they were shot: raw files of
    /// one body, one size, and one focal length, each overlapping the
    /// next
    #[arg(required = true, num_args = 2..)]
    frames: Vec<PathBuf>,
    /// The frame whose view the others are turned onto, counting from
    /// 0; the middle one without one
    #[arg(long, value_name = "N")]
    reference: Option<usize>,
    /// Where the panorama goes, a .dng; without one, beside the first
    /// frame and named after it
    #[arg(long, short, value_name = "OUT.dng")]
    out: Option<PathBuf>,
    /// What to do when the file is there already
    #[arg(long, value_name = "WHAT", default_value = "increment",
          value_parser = common::on_exists_named)]
    on_exists: OnExists,
}

#[derive(Args)]
pub(crate) struct HdrArgs {
    /// The bracket's frames: raw files of one body and one size
    #[arg(required = true, num_args = 2..)]
    frames: Vec<PathBuf>,
    /// The frame the others are put onto, counting from 0; the middle
    /// exposure without one
    #[arg(long, value_name = "N")]
    reference: Option<usize>,
    /// Where the merge goes, a .dng; without one, beside the first
    /// frame and named after it
    #[arg(long, short, value_name = "OUT.dng")]
    out: Option<PathBuf>,
    /// What to do when the file is there already
    #[arg(long, value_name = "WHAT", default_value = "increment",
          value_parser = common::on_exists_named)]
    on_exists: OnExists,
}

/// The merge, to its exit code: 0 merged, 2 merged with something
/// said, an error (1) when nothing was written.
pub(crate) fn run(kind: MergeKind) -> Result<u8> {
    match kind {
        MergeKind::Hdr(args) => hdr(args),
        MergeKind::Pano(args) => pano(args),
    }
}

/// An `--out` that is no DNG is refused.
fn dng_only(out: Option<&PathBuf>) -> Result<()> {
    if let Some(out) = out
        && !out
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("dng"))
    {
        bail!(
            "a merge is written as a linear DNG; {} is not a .dng",
            out.display()
        );
    }
    Ok(())
}

/// What a merge says as it goes, on the standard error.
fn said(step: Step) {
    match step {
        Step::Frame { index, of } => eprintln!("reading frame {index} of {of}"),
        Step::Placing => eprintln!("placing the frames"),
        Step::Blending { index, of } => eprintln!("blending frame {index} of {of}"),
        Step::Writing => eprintln!("writing"),
    }
}

fn pano(args: PanoArgs) -> Result<u8> {
    dng_only(args.out.as_ref())?;
    let options = greycard_engine::merge::pano::Options {
        reference: args.reference,
        out: args.out,
        on_exists: args.on_exists,
        ..Default::default()
    };
    let report = greycard_engine::merge::pano::pano(
        &args.frames,
        &greycard_edit::Edit::default(),
        &options,
        &mut greycard_engine::walk::NoModels,
        &said,
    )?;
    println!(
        "frames        {}, turned onto {}",
        report.frames.len(),
        report.frames[report.reference]
    );
    for p in &report.pairs {
        let fit = p.fit.map_or(
            "the fit did not hold; the features' kept".into(),
            |(r, m)| format!("fit residual {r:.4}, moved {m:.2} px"),
        );
        println!(
            "  {} and {}: {}/{} corners, {} matches, {} inliers at {:.2} px; {fit}; \
             placed at {:.2} px ({:.2} s)",
            report.frames[p.a],
            report.frames[p.b],
            p.corners.0,
            p.corners.1,
            p.matches,
            p.inliers,
            p.feature_rms,
            p.placed_rms,
            p.seconds
        );
    }
    if let Some((exif, fitted)) = report.focal {
        println!("focal length  {exif:.1} px from EXIF, {fitted:.1} px fitted");
    }
    match report.span {
        Some((across, down)) => println!(
            "projection    {}, the frames spanning {across:.1} by {down:.1} degrees",
            report.projection
        ),
        None => println!("projection    {}", report.projection),
    }
    for (a, b, r, n) in &report.ratios {
        println!(
            "overlap       {} over {}: {:.4} {:.4} {:.4} from {n} px",
            report.frames[*b], report.frames[*a], r[0], r[1], r[2]
        );
    }
    for (name, g) in report.frames.iter().zip(&report.gains) {
        println!("gain          {name}: {:.4} {:.4} {:.4}", g[0], g[1], g[2]);
    }
    for ((name, s), at) in report
        .frames
        .iter()
        .zip(&report.seams)
        .zip(&report.seam_points)
        .skip(1)
    {
        let at: String = at
            .iter()
            .map(|(x, y)| format!(", through {x}, {y}"))
            .collect();
        println!(
            "seam          {name}: took {} of {} reduced px it shared, cost {:.1}{at}",
            s.taken, s.overlap, s.cost
        );
    }
    println!(
        "canvas        {}x{}, cropped to {}x{} at {}, {} ({:.1} MP); {} bands, {:?}",
        report.canvas.0,
        report.canvas.1,
        report.crop.width,
        report.crop.height,
        report.crop.x,
        report.crop.y,
        report.crop.area() as f64 / 1e6,
        report.levels,
        report.kernel
    );
    println!(
        "headroom      {:.2} stops; noise S {:.3e}, O {:.3e}; bands {:.2} GB",
        report.headroom_stops,
        report.noise_profile[0],
        report.noise_profile[1],
        report.bands_bytes as f64 / 1e9
    );
    for (name, s) in &report.seconds {
        println!("time          {name}: {s:.2} s");
    }
    if let Some(gb) = peak_gb() {
        println!("memory        peak {gb:.2} GB");
    }
    for w in &report.warnings {
        println!("note          {w}");
    }
    println!("wrote         {}", report.out.display());
    Ok(if report.warnings.is_empty() { 0 } else { 2 })
}

fn hdr(args: HdrArgs) -> Result<u8> {
    dng_only(args.out.as_ref())?;
    let options = Options {
        reference: args.reference,
        out: args.out,
        on_exists: args.on_exists,
        denoiser: None,
        noise: None,
        ca: CaFits::Once,
    };
    let report = merge::hdr(
        &args.frames,
        &greycard_edit::Edit::default(),
        &options,
        &mut greycard_engine::walk::NoModels,
        &said,
    )?;
    println!(
        "frames        {} at {}x{}",
        report.frames.len(),
        report.width,
        report.height
    );
    for (i, f) in report.frames.iter().enumerate() {
        let exif = f.exif.map_or("-".into(), |e| format!("x{e:.4}"));
        let measured = f.measured.map_or("-".into(), |m| {
            format!("x{:.4} from {} px", m.ratio, m.samples)
        });
        let moved = match (f.moved, f.resampled) {
            (Some(by), true) => format!(", moved {by:.2} px, resampled"),
            (Some(by), false) => format!(", moved {by:.3} px"),
            (None, _) => String::new(),
        };
        let role = if i == report.reference {
            "the reference".to_string()
        } else {
            format!("EXIF {exif}, measured {measured}, used x{:.4}", f.exposure)
        };
        println!("  {:<24}  {role}{moved}", f.name);
    }
    println!(
        "headroom      {:.2} stops over the reference's clip; {} px clipped in every frame, \
         from the darkest",
        report.headroom_stops, report.all_clipped
    );
    println!(
        "noise         S {:.3e}, O {:.3e}",
        report.noise_profile[0], report.noise_profile[1]
    );
    let total = report
        .seconds
        .iter()
        .find(|(n, _)| *n == "total")
        .map_or(0.0, |(_, s)| *s);
    match peak_gb() {
        Some(gb) => println!("took          {total:.1} s, peak memory {gb:.2} GB"),
        None => println!("took          {total:.1} s"),
    }
    for w in &report.warnings {
        println!("note          {w}");
    }
    println!("wrote         {}", report.out.display());
    Ok(if report.warnings.is_empty() { 0 } else { 2 })
}

/// The process's peak resident memory, where the system says it.
fn peak_gb() -> Option<f64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find(|l| l.starts_with("VmHWM:"))?
        .split_whitespace()
        .nth(1)?
        .parse::<f64>()
        .ok()
        .map(|kb| kb / 1024.0 / 1024.0)
}

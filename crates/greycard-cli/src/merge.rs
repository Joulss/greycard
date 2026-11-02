//! `greycard merge hdr`: a tripod bracket merged into one linear DNG
//! beside its first frame, through the engine's merge, the one the
//! window's Merge to HDR runs.

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
    }
}

fn hdr(args: HdrArgs) -> Result<u8> {
    if let Some(out) = &args.out
        && !out
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("dng"))
    {
        bail!(
            "a merge is written as a linear DNG; {} is not a .dng",
            out.display()
        );
    }
    let options = Options {
        reference: args.reference,
        out: args.out,
        on_exists: args.on_exists,
        denoiser: None,
        noise: None,
        ca: CaFits::Once,
    };
    let progress = |step: Step| match step {
        Step::Frame { index, of } => eprintln!("reading frame {index} of {of}"),
        Step::Writing => eprintln!("writing"),
    };
    let report = merge::hdr(
        &args.frames,
        &greycard_edit::Edit::default(),
        &options,
        &mut greycard_engine::walk::NoModels,
        &progress,
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

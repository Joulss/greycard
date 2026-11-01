//! The camera match's measurements and the Lightroom mapper's, with no
//! window: their flags read into the engine's options, and the engine's
//! runs (`greycard_engine::match_compare` and the rest) do the rest.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use greycard_edit_io::lightroom::Scales;
use greycard_engine::{
    camera_match, fit_lightroom, match_compare, measure_baseline, measure_crop, measure_lightroom,
};

use crate::Cli;

/// The library the flags name, else the user's.
fn library(cli: &Cli) -> Result<PathBuf> {
    match &cli.library {
        Some(p) => Ok(p.clone()),
        None => greycard_library::Library::user_path().context("no data directory"),
    }
}

/// The mapper's scales with each `NAME=VALUE` given laid over the
/// shipped ones; `flag` names the flag in a refusal.
fn scales(given: &[String], flag: &str) -> Result<Scales> {
    let mut scales = Scales::DEFAULT;
    for g in given {
        scales
            .set_from(g)
            .map_err(|e| anyhow::anyhow!("{flag} {e}"))?;
    }
    Ok(scales)
}

/// `--match-compare`, from the command line to the exit code.
pub(crate) fn match_compare(cli: &Cli, out: &Path) -> Result<ExitCode> {
    match_compare::headless(&match_compare::Options {
        out: out.to_path_buf(),
        library: library(cli)?,
        roots: (!cli.roots.is_empty()).then(|| cli.roots.clone()),
        frames: cli.match_frames.unwrap_or(camera_match::SAMPLE),
        only: cli.match_group.clone(),
        reuse: cli.match_reuse_records.clone(),
        write: cli.match_write.clone(),
    })
}

/// `--measure-baseline`, from the command line to the exit code.
pub(crate) fn measure_baseline(cli: &Cli, out: &Path) -> Result<ExitCode> {
    measure_baseline::headless(&measure_baseline::Options {
        out: out.to_path_buf(),
        library: library(cli)?,
        roots: (!cli.roots.is_empty()).then(|| cli.roots.clone()),
        folder: cli.path.clone().filter(|p| p.is_dir()),
        frames: cli.baseline_frames.unwrap_or(measure_baseline::FRAMES),
        only: cli.baseline_body.clone(),
        before: cli.baseline_before,
    })
}

/// `--measure-lightroom --crops`, from the command line to the exit
/// code.
pub(crate) fn measure_crop(cli: &Cli, catalog: &Path) -> Result<ExitCode> {
    measure_crop::headless(&measure_crop::Options {
        catalog: catalog.to_path_buf(),
        roots: cli.root.clone(),
        folders: cli.exports.clone(),
        limit: cli.limit,
        out: cli.out.clone(),
    })
}

/// `--measure-lightroom`, from the command line to the exit code.
pub(crate) fn measure_lightroom(cli: &Cli, catalog: &Path) -> Result<ExitCode> {
    measure_lightroom::headless(&measure_lightroom::Options {
        catalog: catalog.to_path_buf(),
        roots: cli.root.clone(),
        folders: cli.exports.clone(),
        limit: cli.limit,
        out: cli.out.clone(),
        blocks: cli.blocks.clone(),
        no_body_offset: cli.no_body_offset,
        scales: scales(&cli.scale, "--scale")?,
    })
}

/// `--fit-lightroom`, from the command line to the exit code.
pub(crate) fn fit_lightroom(cli: &Cli, dirs: &[PathBuf]) -> Result<ExitCode> {
    fit_lightroom::headless(&fit_lightroom::Options {
        dirs: dirs.to_vec(),
        out: cli.out.clone(),
        rounds: cli.fit_rounds,
        scales: !cli.offsets_only,
        hold_out: !cli.no_hold_out,
        start: scales(&cli.fit_scale, "--fit-scale")?,
    })
}

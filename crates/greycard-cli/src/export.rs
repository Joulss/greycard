//! `greycard export`: frames written as the editor writes them, each
//! under its sidecar's edit, through the engine's export with no
//! window (`greycard_engine::headless`), the sheet and the presets
//! read from the editor's settings.

use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use greycard_core::output::OnExists;
use greycard_edit::{DisplayCurve, preset};
use greycard_engine::headless::{Overrides, Request, plan, run};
use greycard_engine::settings::Shared;

#[derive(Args)]
pub(crate) struct ExportArgs {
    /// The frames, then where they go. The frames: a raw, JPEG, PNG or
    /// TIFF, several, or a folder of them; a folder alone exports the
    /// frame last open in it (else its first), with --also the rows
    /// named, with --all every frame. Where they go: the file to
    /// write, or the folder a set goes into (one that is there, or a
    /// path ending in a separator)
    #[arg(required = true, value_name = "PATH")]
    paths: Vec<PathBuf>,
    /// Every frame of the folder named, in its order (with several
    /// paths, every frame of each folder among them)
    #[arg(long, conflicts_with = "also")]
    all: bool,
    /// With a folder: these rows of it as well (from 0, comma-separated)
    #[arg(long, value_name = "ROWS", value_delimiter = ',')]
    also: Vec<usize>,
    /// Lay this develop preset (by name, or a preset file's path) over
    /// the frame's edit first, as a step in its history
    #[arg(long, value_name = "NAME")]
    preset: Option<String>,
    /// Fill the export sheet from this export preset, by name; the
    /// output's extension still names the format
    #[arg(long, value_name = "NAME")]
    export_preset: Option<String>,
    /// The long edge in pixels, over the sheet's size; never enlarges
    #[arg(long, value_name = "PX")]
    long_edge: Option<u32>,
    /// What a file there already gets: increment, overwrite or skip;
    /// without it, the sheet's choice
    #[arg(long, value_name = "WHAT", value_parser = crate::common::on_exists_named)]
    on_exists: Option<OnExists>,
    /// Export this version of each frame, by its name, rather than the
    /// file's own edit
    #[arg(long, value_name = "NAME")]
    version_name: Option<String>,
    /// Exposure in stops, over the edit's, for this run alone; 0
    /// resets it
    #[arg(long, value_name = "EV", allow_negative_numbers = true)]
    exposure: Option<f32>,
    /// White balance at this temperature, over the edit's, for this
    /// run alone
    #[arg(long, value_name = "K")]
    develop_temperature: Option<f32>,
    /// AgX as the display curve, over the edit's, for this run alone
    #[arg(long)]
    agx: bool,
    /// Neither read nor write sidecars
    #[arg(long)]
    no_sidecars: bool,
    /// Write sidecars under a hidden `.greycard` folder rather than
    /// beside the frame, over the editor's setting
    #[arg(long)]
    sidecar_folder: bool,
    /// Write an `.xmp` with each sidecar, over the editor's setting
    #[arg(long)]
    xmp_sidecars: bool,
    /// Develop on the CPU alone, without the GPU the editor would use:
    /// the same pixels on every machine
    #[arg(long)]
    cpu: bool,
}

impl ExportArgs {
    /// What the engine is asked to write.
    fn request(mut self) -> Result<Request> {
        anyhow::ensure!(
            self.paths.len() >= 2,
            "give the frames, then the file or folder they go to"
        );
        let target = self.paths.pop().expect("two paths at least");
        Ok(Request {
            paths: self.paths,
            target,
            also: self.also,
            all: self.all,
            preset: self.preset,
            export_preset: self.export_preset,
            long_edge: self.long_edge,
            on_exists: self.on_exists,
            version_name: self.version_name,
            overrides: Overrides {
                temperature: self.develop_temperature,
                exposure: self.exposure,
                display_curve: self.agx.then_some(DisplayCurve::Agx),
            },
            no_sidecars: self.no_sidecars,
            sidecar_folder: self.sidecar_folder,
            xmp_sidecars: self.xmp_sidecars,
        })
    }
}

/// The export, to its exit code: 0 for every frame written as its edit
/// asks, 2 for every frame written but some without something the edit
/// names that this machine has not got, an error (1) for a frame not
/// written or a run that could not begin.
pub(crate) fn export(args: ExportArgs) -> Result<u8> {
    let presets = preset::Store::user();
    let cpu = args.cpu;
    let plan = plan(
        &args.request()?,
        &Shared::load(),
        presets.as_ref(),
        greycard_library::Library::user_path(),
    )?;
    let finished = run(plan, lent(cpu));
    // The line a script wants, whichever way it went.
    eprintln!("{}", finished.line);
    Ok(finished.code())
}

/// The context the run's CA, demosaic, and lens go on: none under
/// `--cpu`, else one of our own where the machine has a GPU, a
/// software adapter taken only when asked for by hand, as the editor
/// takes one.
fn lent(cpu: bool) -> greycard_engine::headless::Lent {
    if cpu {
        return Err("--cpu".into());
    }
    let asked = greycard_gpu::asked_by_hand(|name| std::env::var_os(name));
    greycard_gpu::Context::for_export(asked).map_err(|d| d.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Line {
        #[command(flatten)]
        args: ExportArgs,
    }

    fn request(args: &[&str]) -> Request {
        let mut all = vec!["export"];
        all.extend_from_slice(args);
        Line::try_parse_from(all).unwrap().args.request().unwrap()
    }

    /// `--exposure` given is the override, 0 and below included, as
    /// the editor's `--export` reads it; absent, the edit's own.
    #[test]
    fn a_given_exposure_is_the_override_and_zero_resets_it() {
        let ev = |args: &[&str]| request(args).overrides.exposure;
        assert_eq!(ev(&["a.CR3", "out.jpg"]), None);
        assert_eq!(ev(&["a.CR3", "out.jpg", "--exposure", "0"]), Some(0.0));
        assert_eq!(ev(&["a.CR3", "--exposure", "-1", "out.jpg"]), Some(-1.0));
        let req = request(&["a.CR3", "b.CR3", "out/"]);
        assert_eq!(req.paths, [PathBuf::from("a.CR3"), PathBuf::from("b.CR3")]);
        assert_eq!(req.target, PathBuf::from("out/"));
        let cpu = |args: &[&str]| {
            let mut all = vec!["export"];
            all.extend_from_slice(args);
            Line::try_parse_from(all).unwrap().args.cpu
        };
        assert!(!cpu(&["a.CR3", "out.jpg"]));
        assert!(cpu(&["a.CR3", "out.jpg", "--cpu"]));
        assert!(
            Line::try_parse_from(["export", "a.CR3"])
                .unwrap()
                .args
                .request()
                .is_err()
        );
    }
}

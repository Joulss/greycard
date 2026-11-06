//! The focus stack: every frame developed the short way to camera RGB,
//! registered onto a reference, merged band by band from whichever
//! frame holds each sharpest (the core's `stack_camera`), and written
//! as a linear DNG beside the first frame, `<first> stack.dng`.
//!
//! The command line's `greycard stack` and the window's Merge to Focus
//! stack both run [`frames`]; the window, and the command line when it
//! writes a DNG, write it with [`write_dng`]. Neither has a stack of its
//! own.
//!
//! Every frame is held at once, as the core's merge wants them: about a
//! gigabyte a frame at 24 megapixels.

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use greycard_core::decode::RawMetadata;
use greycard_core::dng::{Compression, LinearDngOptions, write_linear_dng};
use greycard_core::output::OnExists;
use greycard_core::raw::{RawFrame, Samples};
use greycard_core::stack::{self, Reference, StackReport};
use greycard_core::{
    CameraImage, DemosaicMethod, DevelopSettings, HighlightMode, WhiteBalance, demosaic,
};
use greycard_edit::Edit;

use super::{Files, Progress, Step, hold, name_of};

/// What goes after the first frame's name.
pub const SUFFIX: &str = " stack";

/// What a stack is asked. The default is what the window asks: the
/// middle frame, the core's selectivity and residual, lossless
/// samples, beside the first frame, a name already there numbered.
#[derive(Debug, Clone)]
pub struct Options {
    /// The frame the others are put onto.
    pub reference: Reference,
    /// How far each band is taken from the frame with the most to say
    /// in it rather than averaged.
    pub selectivity: f32,
    /// The residual a link between two neighboring frames may have and
    /// still be believed.
    pub max_residual: f32,
    /// The DNG's samples stored uncompressed rather than as lossless
    /// JPEG.
    pub uncompressed: bool,
    /// Where the file goes; none for beside the first frame
    /// ([`default_output`]).
    pub out: Option<PathBuf>,
    /// What to do when that file is there already.
    pub on_exists: OnExists,
    /// The machine's memory in bytes, in place of asking the system:
    /// for a test of the refusal.
    pub memory: Option<u64>,
}

impl Default for Options {
    fn default() -> Self {
        let core = stack::Options::default();
        Self {
            reference: Reference::Middle,
            selectivity: core.selectivity,
            max_residual: core.max_residual,
            uncompressed: false,
            out: None,
            on_exists: OnExists::default(),
            memory: None,
        }
    }
}

/// About what a stack holds a frame's pixel while it merges: the
/// demosaiced frame (12 bytes), and the core's pyramids, sharpness
/// maps, and output over it.
pub const BYTES_A_PIXEL: u64 = 40;

/// Whether `frames` frames of `pixels` pixels fit in `memory` bytes
/// at [`BYTES_A_PIXEL`]: refused when they do not. Every frame is held
/// at once, and a stack past the machine's memory would take the
/// process down with it. A machine that will not say its memory is
/// not asked.
pub fn room(frames: usize, pixels: usize, memory: Option<u64>) -> Result<()> {
    let Some(memory) = memory else {
        return Ok(());
    };
    let wants = (frames as u64)
        .saturating_mul(pixels as u64)
        .saturating_mul(BYTES_A_PIXEL);
    if wants > memory {
        let gb = |b: u64| b as f64 / 1e9;
        bail!(
            "a stack of {frames} frames of {:.0} megapixels wants about {:.1} GB of memory, \
             and this machine has {:.1} GB: stack fewer frames at a time",
            pixels as f64 / 1e6,
            gb(wants),
            gb(memory)
        );
    }
    Ok(())
}

/// `<first frame> stack.dng`, beside the first frame.
pub fn default_output(first: &Path) -> PathBuf {
    let stem = first
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "stack".into());
    let name = format!("{stem}{SUFFIX}.dng");
    match first.parent() {
        Some(dir) => dir.join(name),
        None => PathBuf::from(name),
    }
}

/// The short develop each frame takes: bilinear, hot pixels repaired,
/// no CA correction, highlights clipped rather than reconstructed, as
/// `register` has it, stopped at the demosaic, which leaves camera RGB
/// with no white balance and no matrix on it. That is the space the
/// merge belongs in and the space a linear DNG stores.
pub fn develop_settings() -> DevelopSettings {
    DevelopSettings {
        demosaic: DemosaicMethod::Bilinear,
        highlights: HighlightMode::Clip,
        chromatic_aberration: None,
        ..Default::default()
    }
}

/// The frames merged, not yet written.
pub struct Stacked {
    /// Camera RGB, the white balance divided back out.
    pub merged: CameraImage,
    /// The reference frame, its samples let go: its tags are the
    /// file's.
    pub frame: RawFrame,
    pub metadata: RawMetadata,
    /// The reference's white balance: the merge is at it.
    pub balance: WhiteBalance,
    pub report: StackReport,
    /// Each frame's name, in the order given.
    pub names: Vec<String>,
    /// Seconds reading and developing the frames, and merging them.
    pub developing: f64,
    pub merging: f64,
}

/// `files`, in focus order, each developed by [`develop_settings`] and
/// merged under `options`. Each frame read is said to `progress`, and
/// the merge as placing. A developed picture is refused: it is past the
/// white balance and the matrix. So is a stack the machine's memory
/// will not hold ([`room`]), read off the first frame's size before
/// any other frame is decoded.
pub fn frames(files: &[PathBuf], options: &Options, progress: &dyn Progress) -> Result<Stacked> {
    if files.len() < 2 {
        bail!("a stack wants at least two frames; got {}", files.len());
    }
    let settings = develop_settings();
    let developing = Instant::now();
    let of = files.len();
    let mut images = Vec::with_capacity(of);
    let mut heads = Vec::with_capacity(of);
    for (k, file) in files.iter().enumerate() {
        if greycard_core::picture::is_picture_path(file) {
            bail!(
                "{} is not a RAW file: a stack is merged in camera space, before white \
                 balance and the matrix, and a developed picture is past both",
                file.display()
            );
        }
        progress.step(Step::Frame { index: k + 1, of });
        let (mut frame, metadata) = greycard_core::decode::decode_path_with_metadata(file)
            .with_context(|| format!("decoding {}", file.display()))?;
        if k == 0 {
            let memory = options.memory.or_else(greycard_ai::ram::physical);
            room(of, frame.width * frame.height, memory)?;
        }
        let demosaiced = demosaic(&frame, &settings)
            .with_context(|| format!("developing {}", file.display()))?;
        images.push(demosaiced.image);
        // The tags are read from the frame; its samples are let go.
        frame.samples = Samples::U16(Vec::new());
        heads.push((frame, metadata, demosaiced.white_balance));
    }
    let developing = developing.elapsed().as_secs_f64();

    let core = stack::Options {
        reference: options.reference,
        selectivity: options.selectivity,
        max_residual: options.max_residual,
        ..Default::default()
    };
    progress.step(Step::Placing);
    let merging = Instant::now();
    let (merged, report) = stack::stack_camera(&images, &core)?;
    let merging = merging.elapsed().as_secs_f64();
    drop(images);
    let (frame, metadata, balance) = heads.swap_remove(report.reference);
    Ok(Stacked {
        merged,
        frame,
        metadata,
        balance,
        report,
        names: files.iter().map(|f| name_of(f)).collect(),
        developing,
        merging,
    })
}

/// `stacked` written as a linear DNG to `out`: the reference's color
/// tags and EXIF (the merge is in its geometry and at its white
/// balance), the samples as lossless JPEG unless `uncompressed`, and a
/// small sRGB preview.
pub fn write_dng<W: std::io::Write + std::io::Seek>(
    out: W,
    stacked: &Stacked,
    uncompressed: bool,
) -> Result<()> {
    let preview = greycard_core::merge::preview(&stacked.merged, &stacked.balance, 1024)?;
    let options = LinearDngOptions {
        compression: if uncompressed {
            Compression::Uncompressed
        } else {
            Compression::Lossless
        },
        metadata: Some(&stacked.metadata),
        preview: Some(&preview),
        software: Some(concat!("greycard ", env!("CARGO_PKG_VERSION"))),
        headroom: None,
    };
    write_linear_dng(
        out,
        &stacked.frame,
        &stacked.merged,
        stacked.balance.coefficients_f32(),
        &options,
    )?;
    Ok(())
}

/// What a stack written did.
pub struct Report {
    pub out: PathBuf,
    pub stack: StackReport,
    pub names: Vec<String>,
    /// The seconds each part took, in order.
    pub seconds: Vec<(&'static str, f64)>,
}

/// The focus stack `files` merged and written as a linear DNG, beside
/// the first frame unless `options` says where. The name is held
/// before a frame is read, and the file written under the hidden folder
/// that holds it and renamed at the end, as every merge's is: a stack
/// cut short leaves no half file, and a frame is never written over.
pub fn stack(files: &[PathBuf], options: &Options, progress: &dyn Progress) -> Result<Report> {
    if files.len() < 2 {
        bail!("a stack wants at least two frames; got {}", files.len());
    }
    let started = Instant::now();
    let out = match &options.out {
        Some(p) => p.clone(),
        None => default_output(&files[0]),
    };
    let reserved = hold(&Files(files), &out, options.on_exists)?;
    let stacked = frames(files, options, progress)?;
    progress.step(Step::Writing);
    let t = Instant::now();
    let part = reserved.part();
    let written = (|| -> Result<()> {
        let file = std::fs::File::create(&part)?;
        let mut writer = std::io::BufWriter::new(file);
        write_dng(&mut writer, &stacked, options.uncompressed)?;
        use std::io::Write as _;
        writer.flush()?;
        writer.get_ref().sync_all()?;
        Ok(())
    })();
    let out = reserved.out().to_path_buf();
    if let Err(e) = written {
        return Err(e.context(format!("writing {}", out.display())));
    }
    reserved.promote(&part)?;
    drop(reserved);
    let seconds = vec![
        ("develop", stacked.developing),
        ("merge", stacked.merging),
        ("write", t.elapsed().as_secs_f64()),
        ("total", started.elapsed().as_secs_f64()),
    ];
    Ok(Report {
        out,
        stack: stacked.report,
        names: stacked.names,
        seconds,
    })
}

/// The edit a stack's file starts with: the reference frame's, less
/// the learned denoiser, which a file with no mosaic cannot run. Its
/// exposure stays: a stack is at the reference's.
pub fn edit_for_stack(reference: &Edit) -> Edit {
    let mut edit = reference.clone();
    edit.noise.learned = greycard_edit::Learned::Off;
    edit
}

#[cfg(test)]
mod tests {
    use super::*;
    use greycard_core::merge::Noise;
    use greycard_core::merge::testing::{mosaic, write_dng as write_raw};

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("greycard-focus-{name}-{}", std::process::id()));
        greycard_core::testing::remove_dir_retry(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Two frames of one scene, as a focus stack's are of one subject.
    fn two_frames(dir: &Path) -> Vec<PathBuf> {
        let (w, h) = (192, 128);
        (0..2)
            .map(|k| {
                let p = dir.join(format!("IMG_{k}.dng"));
                write_raw(
                    &p,
                    mosaic(w, h, 0.25, Noise::ASSUMED, 40 + k as u64),
                    w,
                    h,
                    (1, 100),
                    "Stack",
                );
                p
            })
            .collect()
    }

    /// The stack is its steps done by hand, byte for byte: each frame
    /// decoded and demosaiced the short way, merged by the core, and
    /// written with the reference's tags.
    #[test]
    fn the_stack_is_its_steps_by_hand() {
        let dir = scratch("same");
        let files = two_frames(&dir);
        let report = stack(&files, &Options::default(), &|_: Step| {}).expect("a stack");
        assert_eq!(report.out, dir.join("IMG_0 stack.dng"));
        assert_eq!(report.stack.used(), 2, "{:#?}", report.stack.frames);

        let settings = DevelopSettings {
            demosaic: DemosaicMethod::Bilinear,
            highlights: HighlightMode::Clip,
            chromatic_aberration: None,
            ..Default::default()
        };
        let mut sources = Vec::new();
        let mut images = Vec::new();
        for f in &files {
            let (frame, metadata) = greycard_core::decode::decode_path_with_metadata(f).unwrap();
            let d = demosaic(&frame, &settings).unwrap();
            images.push(d.image);
            sources.push((frame, metadata, d.white_balance));
        }
        let (merged, by_hand) = stack::stack_camera(&images, &stack::Options::default()).unwrap();
        assert_eq!(by_hand.reference, report.stack.reference);
        let (frame, metadata, balance) = &sources[by_hand.reference];
        let preview = greycard_core::merge::preview(&merged, balance, 1024).unwrap();
        let mut want = std::io::Cursor::new(Vec::new());
        write_linear_dng(
            &mut want,
            frame,
            &merged,
            balance.coefficients_f32(),
            &LinearDngOptions {
                compression: Compression::Lossless,
                metadata: Some(metadata),
                preview: Some(&preview),
                software: Some(concat!("greycard ", env!("CARGO_PKG_VERSION"))),
                headroom: None,
            },
        )
        .unwrap();
        let got = std::fs::read(&report.out).unwrap();
        assert!(
            got == want.into_inner(),
            "the stack is not its steps by hand"
        );
        // Nothing of the hidden folder is left beside it.
        let left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(left.len(), 3, "{left:?}");
        greycard_core::testing::remove_dir_retry(&dir);
    }

    /// A name there already is numbered, and a frame named as the
    /// output is refused before anything is read.
    #[test]
    fn a_stack_is_numbered_and_never_written_over_a_frame() {
        let dir = scratch("names");
        let files = two_frames(&dir);
        std::fs::write(dir.join("IMG_0 stack.dng"), b"there").unwrap();
        let report = stack(&files, &Options::default(), &|_: Step| {}).expect("a stack");
        assert_eq!(report.out, dir.join("IMG_0 stack-1.dng"));
        assert_eq!(
            std::fs::read(dir.join("IMG_0 stack.dng")).unwrap(),
            b"there"
        );
        let e = stack(
            &files,
            &Options {
                out: Some(files[1].clone()),
                on_exists: OnExists::Overwrite,
                ..Options::default()
            },
            &|_: Step| {},
        )
        .err()
        .expect("a frame refused");
        assert!(e.to_string().contains("one of the frames"), "{e}");
        let one = stack(&files[..1], &Options::default(), &|_: Step| {})
            .err()
            .expect("one frame");
        assert!(one.to_string().contains("at least two"), "{one}");
        greycard_core::testing::remove_dir_retry(&dir);
    }

    /// A stack past the machine's memory is refused before a second
    /// frame is read, and nothing is written.
    #[test]
    fn a_stack_past_the_memory_is_refused() {
        assert!(room(15, 24_000_000, Some(8_000_000_000)).is_err());
        assert!(room(15, 24_000_000, Some(16_000_000_000)).is_ok());
        assert!(room(50, 45_000_000, None).is_ok());
        let dir = scratch("memory");
        let files = two_frames(&dir);
        let heard = std::cell::RefCell::new(Vec::new());
        let e = stack(
            &files,
            &Options {
                memory: Some(1_000_000),
                ..Options::default()
            },
            &|s: Step| heard.borrow_mut().push(s),
        )
        .err()
        .expect("refused");
        assert!(e.to_string().contains("stack fewer frames"), "{e}");
        assert_eq!(*heard.borrow(), [Step::Frame { index: 1, of: 2 }]);
        assert!(!dir.join("IMG_0 stack.dng").exists());
        greycard_core::testing::remove_dir_retry(&dir);
    }
}

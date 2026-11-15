//! Adapters for other raw developers' command-line tools: darktable-cli
//! and rawtherapee-cli, used when they are on `PATH`.
//!
//! Each is handed the clipped mosaic as a CFA DNG ([`write_dng`],
//! through the engine's DNG writer: black 0, white 65535 at the clip,
//! the pattern, the as-shot white and the source's color matrices), with
//! a sidecar that turns off every tone and look module and picks one
//! highlight method, and asked for a 32-bit float TIFF. The TIFF is
//! read back to linear Rec.2020 (through its embedded profile when it
//! has one), aligned to the engine's frame, and put through one 3x3
//! matrix fitted per frame on an unclipped round trip ([`Fit`]): the
//! frame's own mosaic as a DNG, developed by the tool and by the engine,
//! compared outside the highlights. The residual of that fit is the
//! floor under any difference the scores then show.
//!
//! The sidecars are written where the case's files go, so a comparison
//! repeats from what is on disk. Neither tool was installed when this
//! was written, and the darktable history blobs are from memory of
//! `dt_iop_highlights_params_t` v4 and `dt_iop_exposure_params_t` v6, so
//! a run fails loudly rather than score a sidecar a tool ignored: a
//! darktable export must carry the highlights mode asked for in its
//! embedded history ([`check_darktable_history`]), and no two methods of
//! one tool may give the same picture where the input clipped
//! ([`require_distinct`]).

use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use greycard_core::dng::{CfaMosaic, LinearDngOptions, write_cfa_dng};
use greycard_core::raw::{CfaPattern, RawFrame};

use super::clipmap::BORDER;
use super::score::luma;

/// An outside developer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Darktable,
    RawTherapee,
}

impl Tool {
    pub const ALL: [Tool; 2] = [Tool::Darktable, Tool::RawTherapee];

    pub fn binary(self) -> &'static str {
        match self {
            Tool::Darktable => "darktable-cli",
            Tool::RawTherapee => "rawtherapee-cli",
        }
    }

    /// What to install for it.
    pub fn package(self) -> &'static str {
        match self {
            Tool::Darktable => "darktable (4.4 or later, for inpaint opposed and segmentation)",
            Tool::RawTherapee => "rawtherapee (5.10 or later: its CLI, float TIFF, and Coloropp)",
        }
    }

    /// The binary on `PATH`, if it is there.
    pub fn find(self) -> Option<PathBuf> {
        let path = std::env::var_os("PATH")?;
        std::env::split_paths(&path)
            .map(|d| d.join(self.binary()))
            .find(|p| p.is_file())
    }

    pub fn methods(self) -> &'static [Method] {
        match self {
            Tool::Darktable => &DARKTABLE,
            Tool::RawTherapee => &RAWTHERAPEE,
        }
    }

    /// The method a validation round trip develops with: the tool's
    /// default, on a frame with nothing clipped it hardly matters.
    pub fn default_method(self) -> &'static Method {
        &self.methods()[0]
    }
}

/// One of a tool's highlight methods.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Method {
    pub tool: Tool,
    /// The name the tables give it.
    pub label: &'static str,
    /// darktable's `dt_iop_highlights_mode_t`, or RawTherapee's
    /// `[HLRecovery] Method`.
    pub code: Code,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Code {
    Darktable(i32),
    RawTherapee(&'static str),
}

const DARKTABLE: [Method; 5] = [
    Method {
        tool: Tool::Darktable,
        label: "dt-opposed",
        code: Code::Darktable(5),
    },
    Method {
        tool: Tool::Darktable,
        label: "dt-segments",
        code: Code::Darktable(4),
    },
    Method {
        tool: Tool::Darktable,
        label: "dt-laplacians",
        code: Code::Darktable(3),
    },
    Method {
        tool: Tool::Darktable,
        label: "dt-lch",
        code: Code::Darktable(1),
    },
    Method {
        tool: Tool::Darktable,
        label: "dt-clip",
        code: Code::Darktable(0),
    },
];

const RAWTHERAPEE: [Method; 4] = [
    Method {
        tool: Tool::RawTherapee,
        label: "rt-coloropp",
        code: Code::RawTherapee("Coloropp"),
    },
    Method {
        tool: Tool::RawTherapee,
        label: "rt-color",
        code: Code::RawTherapee("Color"),
    },
    Method {
        tool: Tool::RawTherapee,
        label: "rt-blend",
        code: Code::RawTherapee("Blend"),
    },
    Method {
        tool: Tool::RawTherapee,
        label: "rt-luminance",
        code: Code::RawTherapee("Luminance"),
    },
];

/// The stops both tools' exposure is pulled down by, after their
/// highlight reconstruction, so their float output stays under 1 even if
/// the TIFF writer clamps there; the per-frame fit takes it back out.
/// darktable's exposure module and RawTherapee's `Compensation` both act
/// after the demosaic, which is after the highlight pass in both.
pub const PULL: f32 = 4.0;

/// Write a mosaic (levels normalized, white balance not applied, the clip
/// at 1) as the CFA DNG the tools read, with `frame`'s color tags.
pub fn write_dng(
    path: &Path,
    frame: &RawFrame,
    samples: &[f32],
    (width, height): (usize, usize),
    pattern: &CfaPattern,
    gains: [f32; 3],
) -> Result<()> {
    let file =
        std::fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mosaic = CfaMosaic {
        samples,
        width,
        height,
        pattern,
    };
    let options = LinearDngOptions {
        software: Some("greycard highlight bench"),
        ..Default::default()
    };
    write_cfa_dng(BufWriter::new(file), frame, &mosaic, gains, &options)
        .with_context(|| format!("writing {}", path.display()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// darktable's `dt_iop_highlights_params_t`, version 4, as its history
/// stores it: little-endian, hex. Mode, then the defaults: blendL 1,
/// blendC 0, strength 0, clip 1, noise level 0, 30 iterations, scales
/// index 6, candidating 0.4, combine 2, recovery off, solid color 0.
/// Unverified against a darktable build: see the notes' list.
pub fn darktable_params(mode: i32) -> String {
    let mut bytes = Vec::with_capacity(48);
    bytes.extend(mode.to_le_bytes());
    for v in [1.0f32, 0.0, 0.0, 1.0, 0.0] {
        bytes.extend(v.to_le_bytes());
    }
    bytes.extend(30i32.to_le_bytes());
    bytes.extend(6i32.to_le_bytes());
    for v in [0.4f32, 2.0] {
        bytes.extend(v.to_le_bytes());
    }
    bytes.extend(0i32.to_le_bytes());
    bytes.extend(0.0f32.to_le_bytes());
    hex(&bytes)
}

/// darktable's `dt_iop_exposure_params_t`, version 6: manual mode, black
/// 0, `stops` of exposure, the deflicker defaults (50, −4), no
/// compensation of the camera's exposure bias. Unverified, as above.
pub fn darktable_exposure(stops: f32) -> String {
    let mut bytes = Vec::with_capacity(24);
    bytes.extend(0i32.to_le_bytes());
    for v in [0.0f32, stops, 50.0, -4.0] {
        bytes.extend(v.to_le_bytes());
    }
    bytes.extend(0i32.to_le_bytes());
    hex(&bytes)
}

/// The darktable sidecar: a history of the highlights module and the
/// exposure pull, and the automatic presets marked applied so the
/// workflow's own exposure, filmic or sigmoid and color calibration
/// stay off. The demosaic is darktable's default: no setting pins it.
pub fn darktable_xmp(mode: i32) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="XMP Core 4.4.0-Exiv2">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:darktable="http://darktable.sf.net/"
   darktable:xmp_version="5"
   darktable:raw_params="0"
   darktable:auto_presets_applied="1"
   darktable:history_end="2"
   darktable:iop_order_version="4">
   <darktable:history>
    <rdf:Seq>
     <rdf:li
      darktable:num="0"
      darktable:operation="highlights"
      darktable:enabled="1"
      darktable:modversion="4"
      darktable:params="{}"
      darktable:multi_name=""
      darktable:multi_priority="0"/>
     <rdf:li
      darktable:num="1"
      darktable:operation="exposure"
      darktable:enabled="1"
      darktable:modversion="6"
      darktable:params="{}"
      darktable:multi_name=""
      darktable:multi_priority="0"/>
    </rdf:Seq>
   </darktable:history>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
"#,
        darktable_params(mode),
        darktable_exposure(-PULL)
    )
}

/// Confirm from the history darktable embeds in its export that the
/// highlights module ran in `mode`: the first `highlights` entry's
/// params must start with the mode. A file with no history, or with
/// another mode, fails: a sidecar darktable ignored would otherwise
/// score as its default.
pub fn check_darktable_history(tif: &Path, mode: i32) -> Result<()> {
    let bytes = std::fs::read(tif).with_context(|| format!("reading {}", tif.display()))?;
    let text = String::from_utf8_lossy(&bytes);
    let entry = text
        .find("darktable:operation=\"highlights\"")
        .with_context(|| {
            format!(
                "{} carries no darktable history: cannot confirm the highlight method (export with the develop history in the metadata)",
                tif.display()
            )
        })?;
    let rest = &text[entry..];
    let at = rest
        .find("darktable:params=\"")
        .context("the highlights entry has no params")?;
    let params = &rest[at + 18..];
    let want = hex(&mode.to_le_bytes());
    if !params.starts_with(&want) {
        bail!(
            "darktable ran highlights with params {}…, not mode {mode} ({want}…): the sidecar was not honored",
            &params[..params.len().min(16)]
        );
    }
    Ok(())
}

/// The RawTherapee profile: neutral throughout, the method's highlight
/// recovery on with its own defaults (no `Hlbl` or `Hlth`), the same
/// demosaic as the engine's cases, the file's matrix, Rec.2020 working
/// and output space, exposure pulled down by [`PULL`]. `Coloropp`
/// (inpaint opposed) needs RawTherapee 5.10.
pub fn rawtherapee_pp3(method: &str, xtrans: bool) -> String {
    let demosaic = if xtrans {
        "[RAW X-Trans]\nMethod=3-pass (best)\nCcSteps=0\n"
    } else {
        "[RAW Bayer]\nMethod=rcd\nCcSteps=0\nLineDenoise=0\nGreenEqThreshold=0\n"
    };
    format!(
        "[Version]\nAppVersion=5.10\nVersion=351\n\n\
[Exposure]\nAuto=false\nClip=0.02\nCompensation=-{PULL}\nBrightness=0\nContrast=0\nSaturation=0\nBlack=0\nHighlightCompr=0\nHighlightComprThreshold=0\nShadowCompr=0\nHistogramMatching=false\nCurveMode=Standard\nCurveMode2=Standard\nCurve=0;\nCurve2=0;\n\n\
[HLRecovery]\nEnabled=true\nMethod={method}\n\n\
[White Balance]\nEnabled=true\nSetting=Camera\n\n\
[Color Management]\nInputProfile=(camera)\nToneCurve=false\nApplyLookTable=false\nApplyBaselineExposureOffset=false\nApplyHueSatMap=false\nWorkingProfile=Rec2020\nWorkingTRC=none\nOutputProfile=RTv4_Rec2020\nOutputProfileIntent=Relative\nOutputBPC=true\n\n\
[RAW]\nCA=false\nCAAvoidColourshift=true\nHotPixelFilter=false\nDeadPixelFilter=false\nHotDeadPixelThresh=100\nPreExposure=1\n\n\
{demosaic}\n\
[Sharpening]\nEnabled=false\n\n[SharpenMicro]\nEnabled=false\n\n[Vibrance]\nEnabled=false\n\n\
[Luminance Curve]\nEnabled=false\n\n[Shadows & Highlights]\nEnabled=false\n\n\
[Directional Pyramid Denoising]\nEnabled=false\n\n[Impulse Denoising]\nEnabled=false\n\n\
[Defringing]\nEnabled=false\n\n[LensProfile]\nLcMode=lfnone\n\n[Dehaze]\nEnabled=false\n\n\
[Local Contrast]\nEnabled=false\n\n[Color appearance]\nEnabled=false\n\n[RGB Curves]\nEnabled=false\n\n\
[Black & White]\nEnabled=false\n\n[Film Simulation]\nEnabled=false\n\n[Resize]\nEnabled=false\n"
    )
}

/// A tool's picture where the input clipped, sampled: luminance and
/// Oklab ab at unit luminance, every few pixels. Two methods of one tool
/// compared on these must differ, or the tool did not take its sidecar.
pub fn fingerprint(rgb: &[f32], clipped: &[bool]) -> Vec<[f32; 3]> {
    let lab = greycard_core::color::Oklab::for_working_space();
    (0..clipped.len())
        .step_by(17)
        .filter(|&i| clipped[i])
        .map(|i| {
            let px = [rgb[3 * i], rgb[3 * i + 1], rgb[3 * i + 2]];
            let y = luma(px).max(1e-6);
            let l = lab.of(px.map(|v| v / y));
            [y.log2(), l[1], l[2]]
        })
        .collect()
}

/// Fail when two fingerprints are near enough the same: median luminance
/// apart under a hundredth of a stop and median ab under 0.001.
pub fn require_distinct(a: (&str, &[[f32; 3]]), b: (&str, &[[f32; 3]])) -> Result<()> {
    let n = a.1.len().min(b.1.len());
    if n == 0 {
        return Ok(());
    }
    let mut stops: Vec<f32> = (0..n).map(|i| (a.1[i][0] - b.1[i][0]).abs()).collect();
    let mut ab: Vec<f32> = (0..n)
        .map(|i| (a.1[i][1] - b.1[i][1]).hypot(a.1[i][2] - b.1[i][2]))
        .collect();
    let k = n / 2;
    let s = *stops.select_nth_unstable_by(k, f32::total_cmp).1;
    let d = *ab.select_nth_unstable_by(k, f32::total_cmp).1;
    if s < 0.01 && d < 0.001 {
        bail!(
            "{} and {} give the same picture where the input clipped (median {s:.4} stop, ab {d:.5}): the tool did not take its sidecar",
            a.0,
            b.0
        );
    }
    Ok(())
}

/// Write `method`'s sidecar beside `dng` and return its path.
pub fn write_sidecar(method: &Method, dir: &Path, xtrans: bool) -> Result<PathBuf> {
    let (name, body) = match method.code {
        Code::Darktable(mode) => (format!("{}.xmp", method.label), darktable_xmp(mode)),
        Code::RawTherapee(m) => (format!("{}.pp3", method.label), rawtherapee_pp3(m, xtrans)),
    };
    let path = dir.join(name);
    std::fs::write(&path, body).with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

/// The command that develops `dng` with `sidecar` into `out` (a float
/// TIFF). darktable runs on a configuration and cache of its own under
/// `scratch`, never the user's, with an in-memory library.
pub fn command(method: &Method, dng: &Path, sidecar: &Path, out: &Path, scratch: &Path) -> Command {
    match method.tool {
        Tool::Darktable => {
            let mut c = Command::new(Tool::Darktable.binary());
            c.arg(dng)
                .arg(sidecar)
                .arg(out)
                .args(["--apply-custom-presets", "false"])
                .args(["--icc-type", "LIN_REC2020"])
                .arg("--core")
                .arg("--configdir")
                .arg(scratch.join("darktable-config"))
                .arg("--cachedir")
                .arg(scratch.join("darktable-cache"))
                .args(["--library", ":memory:"])
                .args(["--conf", "plugins/imageio/format/tiff/bpp=32"])
                .args(["--conf", "plugins/imageio/format/tiff/compress=0"])
                .args(["--conf", "plugins/darkroom/workflow=none"])
                .args(["--conf", "plugins/darkroom/chromatic-adaptation=legacy"]);
            c
        }
        Tool::RawTherapee => {
            let mut c = Command::new(Tool::RawTherapee.binary());
            c.arg("-o")
                .arg(out)
                .args(["-t", "-b32", "-Y", "-p"])
                .arg(sidecar)
                .arg("-c")
                .arg(dng);
            c
        }
    }
}

/// A command as a shell would read it, for a dry run.
pub fn shown(c: &Command) -> String {
    std::iter::once(c.get_program())
        .chain(c.get_args())
        .map(|a| {
            let s = a.to_string_lossy();
            if s.contains([' ', '\'', '"', '*', '$']) {
                format!("'{}'", s.replace('\'', "'\\''"))
            } else {
                s.into_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Run a command, failing with its output when it fails.
pub fn run(mut c: Command) -> Result<()> {
    let shown = shown(&c);
    let out = c.output().with_context(|| format!("running {shown}"))?;
    if !out.status.success() {
        bail!(
            "{shown} failed ({}):\n{}{}",
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(())
}

/// A tool's output read as linear Rec.2020: interleaved RGB, its size.
/// A TIFF with an embedded profile is converted from it (unbounded, in
/// floating point); one without is taken to be linear Rec.2020.
pub fn read_output(path: &Path) -> Result<(Vec<f32>, usize, usize)> {
    use image::ImageDecoder;
    let file = std::io::BufReader::new(
        std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?,
    );
    let mut decoder = image::codecs::tiff::TiffDecoder::new(file)?;
    let icc = decoder.icc_profile()?;
    let img = image::DynamicImage::from_decoder(decoder)?;
    let (w, h) = (img.width() as usize, img.height() as usize);
    let mut rgb = img.into_rgb32f().into_raw();
    if let Some(icc) = icc {
        to_linear_rec2020(&mut rgb, &icc)?;
    }
    Ok((rgb, w, h))
}

/// The working space as an lcms profile: Rec.2020 primaries, D65, linear.
fn linear_rec2020() -> Result<lcms2::Profile> {
    use lcms2::{CIExyY, CIExyYTRIPLE, ToneCurve};
    let s = greycard_core::color::WORKING_SPACE;
    let p = |x: f64, y: f64| CIExyY { x, y, Y: 1.0 };
    let linear = ToneCurve::new_parametric(1, &[1.0]).map_err(|e| anyhow::anyhow!("{e}"))?;
    lcms2::Profile::new_rgb(
        &p(s.white.x, s.white.y),
        &CIExyYTRIPLE {
            Red: p(s.red.x, s.red.y),
            Green: p(s.green.x, s.green.y),
            Blue: p(s.blue.x, s.blue.y),
        },
        &[&linear, &linear, &linear],
    )
    .map_err(|e| anyhow::anyhow!("the working space's profile: {e}"))
}

/// Convert interleaved RGB from the space `icc` describes to linear
/// Rec.2020 in place.
pub fn to_linear_rec2020(rgb: &mut [f32], icc: &[u8]) -> Result<()> {
    use lcms2::{Intent, PixelFormat, Profile, Transform};
    let from = Profile::new_icc(icc).map_err(|e| anyhow::anyhow!("the file's profile: {e}"))?;
    let to = linear_rec2020()?;
    let t: Transform<[f32; 3], [f32; 3]> = Transform::new(
        &from,
        PixelFormat::RGB_FLT,
        &to,
        PixelFormat::RGB_FLT,
        Intent::RelativeColorimetric,
    )
    .map_err(|e| anyhow::anyhow!("a transform from the file's profile: {e}"))?;
    let (px, _) = rgb.as_chunks_mut::<3>();
    let src = px.to_vec();
    t.transform_pixels(&src, px);
    Ok(())
}

/// Where a tool's picture sits on the engine's: the engine's pixel
/// `(x + dx, y + dy)` is the tool's `(x, y)`. Searched over the
/// difference in size (a tool that crops a margin) and up to
/// [`BORDER`] pixels either way, on log luminance every fourth pixel.
pub fn align(
    tool: &[f32],
    (tw, th): (usize, usize),
    ours: &[f32],
    (w, h): (usize, usize),
) -> Result<(isize, isize)> {
    let reach = BORDER as isize;
    let (sx, sy) = ((w as isize - tw as isize), (h as isize - th as isize));
    if sx.abs() > 2 * reach || sy.abs() > 2 * reach {
        bail!("the tool's picture is {tw}x{th}, ours {w}x{h}: too far apart to align");
    }
    let log = |rgb: &[f32], i: usize| {
        luma([rgb[3 * i], rgb[3 * i + 1], rgb[3 * i + 2]])
            .max(1e-6)
            .log2()
    };
    let mut best = (f64::INFINITY, (0, 0));
    for dy in (sy.min(0) - reach / 2)..=(sy.max(0) + reach / 2) {
        for dx in (sx.min(0) - reach / 2)..=(sx.max(0) + reach / 2) {
            let mut diffs = Vec::new();
            for y in (BORDER..th.saturating_sub(BORDER)).step_by(4) {
                for x in (BORDER..tw.saturating_sub(BORDER)).step_by(4) {
                    let (ox, oy) = (x as isize + dx, y as isize + dy);
                    if ox < 0 || oy < 0 || ox >= w as isize || oy >= h as isize {
                        continue;
                    }
                    diffs.push(log(tool, y * tw + x) - log(ours, oy as usize * w + ox as usize));
                }
            }
            if diffs.len() < 16 {
                continue;
            }
            // Exposure differs by a constant: score the spread round the
            // median, not the difference itself.
            let k = diffs.len() / 2;
            let med = *diffs.select_nth_unstable_by(k, f32::total_cmp).1;
            let cost = diffs
                .iter()
                .map(|d| f64::from((d - med).abs()))
                .sum::<f64>()
                / diffs.len() as f64;
            if cost < best.0 {
                best = (cost, (dx, dy));
            }
        }
    }
    if !best.0.is_finite() {
        bail!("no overlap to align on");
    }
    Ok(best.1)
}

/// The tool's picture placed on the engine's frame; pixels it does not
/// cover (only ever in the border no score reads) take `fill`'s.
pub fn placed(
    tool: &[f32],
    (tw, th): (usize, usize),
    (dx, dy): (isize, isize),
    fill: &[f32],
    (w, h): (usize, usize),
) -> Vec<f32> {
    let mut out = fill.to_vec();
    for y in 0..th {
        for x in 0..tw {
            let (ox, oy) = (x as isize + dx, y as isize + dy);
            if ox < 0 || oy < 0 || ox >= w as isize || oy >= h as isize {
                continue;
            }
            let (i, o) = (3 * (y * tw + x), 3 * (oy as usize * w + ox as usize));
            out[o..o + 3].copy_from_slice(&tool[i..i + 3]);
        }
    }
    out
}

/// One 3x3 matrix, rows, taking a tool's linear output to the engine's,
/// fitted by least squares on unclipped pixels of a round trip; and how
/// far apart the two still are after it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fit {
    pub matrix: [[f64; 3]; 3],
    pub offset: (isize, isize),
    /// Median absolute luminance difference, stops, before and after.
    pub stops_before: f64,
    pub stops_after: f64,
    /// Median Oklab ab distance at matched lightness, after.
    pub dab_after: f64,
}

impl Fit {
    pub fn apply(&self, rgb: &mut [f32]) {
        let m = self.matrix;
        for px in rgb.as_chunks_mut::<3>().0 {
            let v = [f64::from(px[0]), f64::from(px[1]), f64::from(px[2])];
            for (c, o) in px.iter_mut().enumerate() {
                *o = (m[c][0] * v[0] + m[c][1] * v[1] + m[c][2] * v[2]) as f32;
            }
        }
    }
}

/// Fit `tool` (already placed on the engine's frame) to `ours` over the
/// pixels whose engine luminance lies in `0.01..0.8` of the clip,
/// every third pixel, away from the border.
pub fn fit(tool: &[f32], ours: &[f32], (w, h): (usize, usize)) -> Result<Fit> {
    let mut a = [[0.0f64; 3]; 3]; // sum of tool tool^T
    let mut b = [[0.0f64; 3]; 3]; // sum of ours tool^T
    let mut picks = Vec::new();
    for y in (BORDER..h.saturating_sub(BORDER)).step_by(3) {
        for x in (BORDER..w.saturating_sub(BORDER)).step_by(3) {
            let i = 3 * (y * w + x);
            let o = [ours[i], ours[i + 1], ours[i + 2]];
            let t = [tool[i], tool[i + 1], tool[i + 2]];
            let yo = luma(o);
            if !(0.01..0.8).contains(&yo) || luma(t) <= 0.0 {
                continue;
            }
            picks.push(i);
            for r in 0..3 {
                for c in 0..3 {
                    a[r][c] += f64::from(t[r]) * f64::from(t[c]);
                    b[r][c] += f64::from(o[r]) * f64::from(t[c]);
                }
            }
        }
    }
    if picks.len() < 100 {
        bail!("too few unclipped pixels to fit on ({})", picks.len());
    }
    let inv = invert(a).context("a singular fit")?;
    let mut matrix = [[0.0f64; 3]; 3];
    for r in 0..3 {
        for c in 0..3 {
            matrix[r][c] = (0..3).map(|k| b[r][k] * inv[k][c]).sum();
        }
    }
    let lab = greycard_core::color::Oklab::for_working_space();
    let mut before = Vec::with_capacity(picks.len());
    let mut after = Vec::with_capacity(picks.len());
    let mut dab = Vec::with_capacity(picks.len());
    let draft = Fit {
        matrix,
        offset: (0, 0),
        stops_before: 0.0,
        stops_after: 0.0,
        dab_after: 0.0,
    };
    for &i in &picks {
        let o = [ours[i], ours[i + 1], ours[i + 2]];
        let t = [tool[i], tool[i + 1], tool[i + 2]];
        let mut f = t;
        draft.apply(&mut f);
        let (yo, yt, yf) = (luma(o), luma(t), luma(f).max(1e-6));
        before.push((yt / yo).log2().abs());
        after.push((yf / yo).log2().abs());
        let (lo, lf) = (lab.of(o.map(|v| v / yo)), lab.of(f.map(|v| v / yf)));
        dab.push((lo[1] - lf[1]).hypot(lo[2] - lf[2]));
    }
    let median = |v: &mut Vec<f32>| {
        let k = v.len() / 2;
        f64::from(*v.select_nth_unstable_by(k, f32::total_cmp).1)
    };
    Ok(Fit {
        matrix,
        offset: (0, 0),
        stops_before: median(&mut before),
        stops_after: median(&mut after),
        dab_after: median(&mut dab),
    })
}

/// How far apart two pictures of a frame are outside the highlights:
/// median absolute luminance difference in stops and median Oklab ab
/// distance at matched lightness, over pixels whose luminance in `b`
/// lies in `0.01..0.8`, every third pixel.
pub fn residual(a: &[f32], b: &[f32], (w, h): (usize, usize)) -> (f64, f64) {
    let lab = greycard_core::color::Oklab::for_working_space();
    let mut stops = Vec::new();
    let mut dab = Vec::new();
    for y in (BORDER..h.saturating_sub(BORDER)).step_by(3) {
        for x in (BORDER..w.saturating_sub(BORDER)).step_by(3) {
            let i = 3 * (y * w + x);
            let (pa, pb) = ([a[i], a[i + 1], a[i + 2]], [b[i], b[i + 1], b[i + 2]]);
            let (ya, yb) = (luma(pa).max(1e-6), luma(pb));
            if !(0.01..0.8).contains(&yb) {
                continue;
            }
            stops.push((ya / yb).log2().abs());
            let (la, lb) = (lab.of(pa.map(|v| v / ya)), lab.of(pb.map(|v| v / yb)));
            dab.push((la[1] - lb[1]).hypot(la[2] - lb[2]));
        }
    }
    let median = |v: &mut Vec<f32>| {
        if v.is_empty() {
            return f64::NAN;
        }
        let k = v.len() / 2;
        f64::from(*v.select_nth_unstable_by(k, f32::total_cmp).1)
    };
    (median(&mut stops), median(&mut dab))
}

/// Run a method's command and, for darktable, confirm from the file's
/// embedded history that it ran the method asked for.
fn run_checked(method: &Method, dng: &Path, sidecar: &Path, out: &Path, dir: &Path) -> Result<()> {
    run(command(method, dng, sidecar, out, dir))?;
    if let Code::Darktable(mode) = method.code {
        check_darktable_history(out, mode)
            .with_context(|| format!("{} on {}", method.label, dng.display()))?;
    }
    Ok(())
}

/// Develop `dng` with a tool's `method` into `dir`, read it back and
/// put it on the engine's frame: placed at the fit's offset, through its
/// matrix when there is a fit, the gaps filled from `fill`.
pub fn develop_with(
    method: &Method,
    dng: &Path,
    dir: &Path,
    xtrans: bool,
    fit: Option<&Fit>,
    fill: &[f32],
    size: (usize, usize),
) -> Result<Vec<f32>> {
    let sidecar = write_sidecar(method, dir, xtrans)?;
    let stem = dng
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let out = dir.join(format!("{stem}-{}.tif", method.label));
    let _ = std::fs::remove_file(&out);
    run_checked(method, dng, &sidecar, &out, dir)?;
    let (rgb, tw, th) = read_output(&out)?;
    let offset = match fit {
        Some(f) => f.offset,
        None => align(&rgb, (tw, th), fill, size)?,
    };
    let mut placed = placed(&rgb, (tw, th), offset, fill, size);
    if let Some(f) = fit {
        f.apply(&mut placed);
    }
    Ok(placed)
}

/// The per-frame fit of a tool: its default method on the unclipped
/// round trip `dng`, against `ours`, the engine's develop of the same
/// file.
pub fn validate(
    tool: Tool,
    dng: &Path,
    dir: &Path,
    xtrans: bool,
    ours: &[f32],
    size: (usize, usize),
) -> Result<Fit> {
    let method = tool.default_method();
    let sidecar = write_sidecar(method, dir, xtrans)?;
    let out = dir.join(format!("valid-{}.tif", method.label));
    let _ = std::fs::remove_file(&out);
    run_checked(method, dng, &sidecar, &out, dir)?;
    let (rgb, tw, th) = read_output(&out)?;
    let offset = align(&rgb, (tw, th), ours, size)?;
    let placed = placed(&rgb, (tw, th), offset, ours, size);
    let mut f = fit(&placed, ours, size)?;
    f.offset = offset;
    Ok(f)
}

fn invert(m: [[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if det.abs() < 1e-300 {
        return None;
    }
    let c = |r: usize, s: usize| {
        let (r1, r2) = ((r + 1) % 3, (r + 2) % 3);
        let (s1, s2) = ((s + 1) % 3, (s + 2) % 3);
        m[r1][s1] * m[r2][s2] - m[r1][s2] * m[r2][s1]
    };
    let mut out = [[0.0; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = c(j, i) / det;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_darktable_blob_is_the_version_4_struct() {
        let hex = darktable_params(5);
        assert_eq!(hex.len(), 96);
        assert!(hex.starts_with("05000000"));
        // blendL 1.0, then blendC 0.
        assert_eq!(&hex[8..16], "0000803f");
        // Iterations 30 at the seventh field.
        assert_eq!(&hex[48..56], "1e000000");
        let xmp = darktable_xmp(4);
        assert!(xmp.contains("darktable:operation=\"highlights\""));
        assert!(xmp.contains("darktable:auto_presets_applied=\"1\""));
        assert!(xmp.contains(&darktable_params(4)));
    }

    #[test]
    fn the_rawtherapee_profile_names_the_method_and_turns_the_rest_off() {
        let pp3 = rawtherapee_pp3("Coloropp", false);
        assert!(pp3.contains("[HLRecovery]\nEnabled=true\nMethod=Coloropp"));
        assert!(pp3.contains("Method=rcd"));
        assert!(pp3.contains(&format!("Compensation=-{PULL}")));
        assert!(!pp3.contains("Hlth"));
        assert!(pp3.contains("[Sharpening]\nEnabled=false"));
        assert!(rawtherapee_pp3("Blend", true).contains("3-pass"));
    }

    #[test]
    fn commands_read_as_the_tools_take_them() {
        let dt = command(
            &DARKTABLE[1],
            Path::new("/w/in.dng"),
            Path::new("/w/dt-segments.xmp"),
            Path::new("/w/out.tif"),
            Path::new("/w"),
        );
        let s = shown(&dt);
        assert!(s.starts_with("darktable-cli /w/in.dng /w/dt-segments.xmp /w/out.tif"));
        assert!(s.contains("--icc-type LIN_REC2020"));
        // Joined by the platform, so the separator is Windows' own there.
        let config = Path::new("/w").join("darktable-config");
        assert!(s.contains(&format!("--core --configdir {}", config.display())));
        assert!(s.contains("--library :memory:"));
        let rt = command(
            &RAWTHERAPEE[0],
            Path::new("/w/in.dng"),
            Path::new("/w/rt.pp3"),
            Path::new("/w/out.tif"),
            Path::new("/w"),
        );
        assert_eq!(
            shown(&rt),
            "rawtherapee-cli -o /w/out.tif -t -b32 -Y -p /w/rt.pp3 -c /w/in.dng"
        );
        assert_eq!(Tool::Darktable.methods().len(), 5);
        assert_eq!(Tool::RawTherapee.methods().len(), 4);
    }

    /// A picture under a known matrix and offset: the fit and the
    /// alignment find them, and a float TIFF of it reads back.
    #[test]
    fn fit_align_and_read_recover_a_known_transform() {
        let (w, h) = (64usize, 48usize);
        let ours: Vec<f32> = (0..w * h)
            .flat_map(|i| {
                let (x, y) = ((i % w) as f32, (i / w) as f32);
                let v = 0.05 + 0.5 * ((x * 0.37).sin() * (y * 0.23).cos()).abs();
                [
                    v,
                    v * (0.6 + 0.3 * (x / w as f32)),
                    v * (0.4 + 0.5 * (y / h as f32)),
                ]
            })
            .collect();
        let m = [[1.1, 0.05, -0.02], [0.03, 0.95, 0.01], [-0.01, 0.04, 1.2]];
        let inv = invert(m).unwrap();
        let mut tool = ours.clone();
        Fit {
            matrix: inv,
            offset: (0, 0),
            stops_before: 0.0,
            stops_after: 0.0,
            dab_after: 0.0,
        }
        .apply(&mut tool);
        let f = fit(&tool, &ours, (w, h)).unwrap();
        for r in 0..3 {
            for c in 0..3 {
                assert!((f.matrix[r][c] - m[r][c]).abs() < 1e-3, "{:?}", f.matrix);
            }
        }
        assert!(f.stops_after < 1e-4 && f.stops_before > 0.0);

        // The tool's picture cropped by two pixels left and three on top.
        let (tw, th) = (w - 4, h - 6);
        let cropped: Vec<f32> = (0..th)
            .flat_map(|y| {
                let ours = &ours;
                (0..tw).flat_map(move |x| {
                    let i = 3 * ((y + 3) * w + x + 2);
                    [ours[i], ours[i + 1], ours[i + 2]]
                })
            })
            .collect();
        assert_eq!(align(&cropped, (tw, th), &ours, (w, h)).unwrap(), (2, 3));
        let back = placed(&cropped, (tw, th), (2, 3), &ours, (w, h));
        assert_eq!(back, ours);

        let dir = std::env::temp_dir().join(format!("greycard-hb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.tif");
        image::Rgb32FImage::from_raw(w as u32, h as u32, ours.clone())
            .unwrap()
            .save(&path)
            .unwrap();
        let (read, rw, rh) = read_output(&path).unwrap();
        greycard_core::testing::remove_dir_retry(&dir);
        assert_eq!((rw, rh), (w, h));
        assert_eq!(read, ours);
    }

    /// The exposure pull is in the darktable history, and a darktable
    /// export whose history names another mode, or none, fails.
    #[test]
    fn darktable_is_held_to_its_sidecar() {
        let xmp = darktable_xmp(5);
        assert!(xmp.contains("darktable:operation=\"exposure\""));
        let pull = darktable_exposure(-PULL);
        assert_eq!(pull.len(), 48);
        assert_eq!(&pull[16..24], hex(&(-PULL).to_le_bytes()));
        assert!(xmp.contains(&pull));

        let dir = std::env::temp_dir().join(format!("greycard-hb-dt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let tif = dir.join("out.tif");
        // A file with the history embedded, as darktable writes it.
        std::fs::write(&tif, format!("II*\0junk{}junk", darktable_xmp(5))).unwrap();
        assert!(check_darktable_history(&tif, 5).is_ok());
        assert!(check_darktable_history(&tif, 4).is_err());
        std::fs::write(&tif, "II*\0no history").unwrap();
        let e = check_darktable_history(&tif, 5).unwrap_err().to_string();
        greycard_core::testing::remove_dir_retry(&dir);
        assert!(e.contains("no darktable history"), "{e}");
    }

    /// Two methods that give the same picture where the input clipped
    /// fail; two that differ pass.
    #[test]
    fn identical_methods_fail() {
        let clipped = vec![true; 340];
        let a: Vec<f32> = (0..340 * 3).map(|i| 1.0 + (i % 7) as f32 * 0.1).collect();
        let fa = fingerprint(&a, &clipped);
        assert!(!fa.is_empty());
        assert!(require_distinct(("clip", &fa), ("opposed", &fa)).is_err());
        let b: Vec<f32> = a.iter().map(|v| v * 1.5).collect();
        let fb = fingerprint(&b, &clipped);
        assert!(require_distinct(("clip", &fa), ("opposed", &fb)).is_ok());
    }

    /// An sRGB-encoded profile's mid grey comes back linear.
    #[test]
    fn an_embedded_profile_is_undone() {
        let srgb = lcms2::Profile::new_srgb().icc().unwrap();
        let mut px = vec![0.5f32, 0.5, 0.5];
        to_linear_rec2020(&mut px, &srgb).unwrap();
        let want = greycard_core::color::srgb_decode(0.5);
        for v in px {
            assert!((v - want).abs() < 2e-3, "{v} vs {want}");
        }
    }
}

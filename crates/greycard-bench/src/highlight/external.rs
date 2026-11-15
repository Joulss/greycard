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
//! compared outside the highlights. The fit must find the output linear
//! (its log slope within 5% of 1). The tool's own develop of that
//! unclipped frame, scaled by a case's overexposure, is scored where the
//! input clipped as `dt-unclipped` and `rt-unclipped`: the floor under
//! any difference the scores then show; and every method is also scored
//! against it, its own truth.
//!
//! The sidecars are written where the case's files go, so a comparison
//! repeats from what is on disk. Checked against darktable 5.6.2 and
//! RawTherapee 5.13, and still held to account on every run, so a run
//! fails loudly rather than score a sidecar a tool ignored: a darktable
//! export must carry in its embedded history the demosaic, highlights,
//! and exposure entries written and no module beyond the plumbing
//! ([`check_darktable_history`]); RawTherapee's own copy of the profile
//! it ran must say what was asked and have nothing else on
//! ([`check_rawtherapee_profile`]). Two methods of different algorithms
//! that give the same picture where the input clipped are logged
//! ([`require_distinct`]); they may agree on a case, legitimately.
//!
//! The parameter layouts are read from darktable 5.6.2's
//! `src/iop/highlights.c`, `exposure.c`, and `demosaic.c` (the darktable
//! developers, GPL-3.0-or-later), and the profile keys from RawTherapee
//! 5.13's `rtengine/procparams.cc` (the RawTherapee developers,
//! GPL-3.0-or-later).

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

    /// The prefix its methods' labels carry.
    pub fn prefix(self) -> &'static str {
        match self {
            Tool::Darktable => "dt",
            Tool::RawTherapee => "rt",
        }
    }

    /// What to install for it.
    pub fn package(self) -> &'static str {
        match self {
            Tool::Darktable => {
                "darktable (5.x: checked with 5.6, whose exposure module is version 7)"
            }
            Tool::RawTherapee => "rawtherapee (5.10 or later for Coloropp: checked with 5.13)",
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
    /// darktable's highlights parameters, or RawTherapee's
    /// `[HLRecovery] Method`.
    pub code: Code,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Code {
    Darktable(DtHighlights),
    RawTherapee(&'static str),
}

impl Code {
    /// The algorithm without its settings. Two methods of one family may
    /// give the same picture where they differ only in a setting a case
    /// does not exercise (a rebuild where no cell clipped in every
    /// color), so only methods of different families are held apart.
    pub fn family(&self) -> (i32, &'static str) {
        match *self {
            Code::Darktable(h) => (h.mode, ""),
            Code::RawTherapee(m) => (-1, m),
        }
    }
}

/// The fields of darktable's `dt_iop_highlights_params_t` (version 4,
/// darktable 4.4 to 5.6) a method sets; the rest are the module's
/// defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DtHighlights {
    /// `dt_iop_highlights_mode_t`: clip 0, reconstruct in LCh 1,
    /// reconstruct color 2, guided laplacians 3, segmentation based 4,
    /// inpaint opposed 5.
    pub mode: i32,
    /// The laplacians' iterations, 1 to 256 (default 30).
    pub iterations: i32,
    /// The laplacians' diameter, `dt_atrous_wavelets_scales_t`: index k
    /// is 2^(k+1) px; the default 6 is 128 px.
    pub scales: i32,
    /// The segmentation's rebuild of fully clipped segments,
    /// `dt_recovery_mode_t`: off 0 (the default), generic 5.
    pub recovery: i32,
    /// The rebuild's strength, 0 to 1 (default 0).
    pub strength: f32,
}

impl DtHighlights {
    /// `mode` with the module's defaults.
    pub const fn mode(mode: i32) -> Self {
        Self {
            mode,
            iterations: 30,
            scales: 6,
            recovery: 0,
            strength: 0.0,
        }
    }
}

const DARKTABLE: [Method; 7] = [
    Method {
        tool: Tool::Darktable,
        label: "dt-opposed",
        code: Code::Darktable(DtHighlights::mode(5)),
    },
    Method {
        tool: Tool::Darktable,
        label: "dt-segments",
        code: Code::Darktable(DtHighlights::mode(4)),
    },
    // The segmentation with its rebuild of fully clipped segments on,
    // "generic", at full strength.
    Method {
        tool: Tool::Darktable,
        label: "dt-segments-rebuild",
        code: Code::Darktable(DtHighlights {
            recovery: 5,
            strength: 1.0,
            ..DtHighlights::mode(4)
        }),
    },
    Method {
        tool: Tool::Darktable,
        label: "dt-laplacians",
        code: Code::Darktable(DtHighlights::mode(3)),
    },
    // Guided laplacians at a high-quality setting: 128 iterations over a
    // 512 px diameter.
    Method {
        tool: Tool::Darktable,
        label: "dt-laplacians-hq",
        code: Code::Darktable(DtHighlights {
            iterations: 128,
            scales: 8,
            ..DtHighlights::mode(3)
        }),
    },
    Method {
        tool: Tool::Darktable,
        label: "dt-lch",
        code: Code::Darktable(DtHighlights::mode(1)),
    },
    Method {
        tool: Tool::Darktable,
        label: "dt-clip",
        code: Code::Darktable(DtHighlights::mode(0)),
    },
];

const RAWTHERAPEE: [Method; 6] = [
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
    Method {
        tool: Tool::RawTherapee,
        label: "rt-cielab",
        code: Code::RawTherapee("CIELab blending"),
    },
    Method {
        tool: Tool::RawTherapee,
        label: "rt-off",
        code: Code::RawTherapee(RT_OFF),
    },
];

/// The RawTherapee "method" that turns highlight recovery off: what it
/// does to a clipped highlight without one, its clip.
pub const RT_OFF: &str = "off";

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

/// darktable's `dt_iop_highlights_params_t`, version 4 (darktable 4.4 to
/// 5.6, checked against 5.6.2's `src/iop/highlights.c`), as its history
/// stores it: little-endian, hex, 48 bytes. Mode, blendL 1, blendC 0
/// (both unused), the rebuild's strength, clip 1, noise level 0,
/// iterations, scales, candidating 0.4, combine 2, the rebuild's mode,
/// solid color 0.
pub fn darktable_params(h: &DtHighlights) -> String {
    let mut bytes = Vec::with_capacity(48);
    bytes.extend(h.mode.to_le_bytes());
    for v in [1.0f32, 0.0, h.strength, 1.0, 0.0] {
        bytes.extend(v.to_le_bytes());
    }
    bytes.extend(h.iterations.to_le_bytes());
    bytes.extend(h.scales.to_le_bytes());
    for v in [0.4f32, 2.0] {
        bytes.extend(v.to_le_bytes());
    }
    bytes.extend(h.recovery.to_le_bytes());
    bytes.extend(0.0f32.to_le_bytes());
    hex(&bytes)
}

/// darktable's `dt_iop_exposure_params_t`, version 7 (5.6.2's
/// `src/iop/exposure.c`), 28 bytes: manual mode, black 0, `stops` of
/// exposure, the deflicker defaults (50, −4), no compensation of the
/// camera's exposure bias, no compensation of a highlight preservation
/// mode's bias.
pub fn darktable_exposure(stops: f32) -> String {
    let mut bytes = Vec::with_capacity(28);
    bytes.extend(0i32.to_le_bytes());
    for v in [0.0f32, stops, 50.0, -4.0] {
        bytes.extend(v.to_le_bytes());
    }
    bytes.extend(0i32.to_le_bytes());
    bytes.extend(0i32.to_le_bytes());
    hex(&bytes)
}

/// darktable's demosaic method codes: RCD for Bayer (its default too),
/// Markesteijn 3-pass for X-Trans (its default is 1-pass), the engine's
/// pair.
pub fn darktable_demosaic_method(xtrans: bool) -> i32 {
    if xtrans { 1024 | 2 } else { 5 }
}

/// darktable's `dt_iop_demosaic_params_t`, version 6, 48 bytes: no green
/// equilibration, edge threshold 0, no color smoothing, the method, LMMSE
/// refine 1, dual threshold 0.2, and capture sharpening off (its radius
/// 0, threshold 0.4, boost 0, 8 iterations, center 0).
pub fn darktable_demosaic(xtrans: bool) -> String {
    let mut bytes = Vec::with_capacity(48);
    bytes.extend(0i32.to_le_bytes());
    bytes.extend(0.0f32.to_le_bytes());
    bytes.extend(0i32.to_le_bytes());
    bytes.extend(darktable_demosaic_method(xtrans).to_le_bytes());
    bytes.extend(1i32.to_le_bytes());
    for v in [0.2f32, 0.0, 0.4, 0.0] {
        bytes.extend(v.to_le_bytes());
    }
    bytes.extend(8i32.to_le_bytes());
    bytes.extend(0.0f32.to_le_bytes());
    bytes.extend(0i32.to_le_bytes());
    hex(&bytes)
}

/// The history entries the sidecar sets: operation, module version,
/// params.
pub fn darktable_history(h: &DtHighlights, xtrans: bool) -> [(&'static str, u32, String); 3] {
    [
        ("demosaic", 6, darktable_demosaic(xtrans)),
        ("highlights", 4, darktable_params(h)),
        ("exposure", 7, darktable_exposure(-PULL)),
    ]
}

/// The modules a raw's export may run besides those: what darktable
/// always runs on a raw (the black and white levels, the white balance,
/// the input and output profiles, the orientation, the final gamma
/// stage), none of which changes a linear export's tone. Anything else
/// in the history (filmic, sigmoid, color calibration, a lens
/// correction, a denoise, a sharpen, a second exposure) fails the check.
pub const DARKTABLE_PLUMBING: [&str; 6] = [
    "rawprepare",
    "temperature",
    "colorin",
    "colorout",
    "flip",
    "gamma",
];

/// The darktable sidecar: a history of the demosaic, the highlights
/// module, and the exposure pull, and the automatic presets marked
/// applied so the workflow's own exposure, filmic or sigmoid, and color
/// calibration stay off.
pub fn darktable_xmp(h: &DtHighlights, xtrans: bool) -> String {
    let entries: String = darktable_history(h, xtrans)
        .iter()
        .enumerate()
        .map(|(num, (op, version, params))| {
            format!(
                r#"     <rdf:li
      darktable:num="{num}"
      darktable:operation="{op}"
      darktable:enabled="1"
      darktable:modversion="{version}"
      darktable:params="{params}"
      darktable:multi_name=""
      darktable:multi_priority="0"/>
"#
            )
        })
        .collect();
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
   darktable:history_end="3"
   darktable:iop_order_version="4">
   <darktable:history>
    <rdf:Seq>
{entries}    </rdf:Seq>
   </darktable:history>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
"#
    )
}

/// One entry of a darktable history.
#[derive(Debug, Clone, PartialEq)]
struct DtEntry {
    num: usize,
    operation: String,
    enabled: bool,
    modversion: u32,
    params: String,
}

fn attribute<'a>(entry: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("darktable:{name}=\"");
    let at = entry.find(&key)? + key.len();
    let rest = &entry[at..];
    Some(&rest[..rest.find('"')?])
}

/// The entries of the history darktable embeds in an export, those under
/// its `history_end` only (the rest are undone steps), and that end.
fn darktable_entries(text: &str) -> Option<Vec<DtEntry>> {
    let start = text.find("<darktable:history>")?;
    let end = text[start..]
        .find("</darktable:history>")
        .map_or(text.len(), |e| start + e);
    let history_end: Option<usize> = attribute(text, "history_end").and_then(|v| v.parse().ok());
    let mut out = Vec::new();
    for li in text[start..end].split("<rdf:li").skip(1) {
        let (Some(num), Some(operation)) = (
            attribute(li, "num").and_then(|v| v.parse().ok()),
            attribute(li, "operation"),
        ) else {
            continue;
        };
        if history_end.is_some_and(|e| num >= e) {
            continue;
        }
        out.push(DtEntry {
            num,
            operation: operation.to_string(),
            enabled: attribute(li, "enabled") == Some("1"),
            modversion: attribute(li, "modversion")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            params: attribute(li, "params").unwrap_or("").to_string(),
        });
    }
    Some(out)
}

/// Confirm from the history darktable embeds in its export that it ran
/// what the sidecar asked and nothing else: the last entry of each of
/// the sidecar's modules must be enabled with the version and params
/// written (the highlights module's whole blob, so a laplacians setting
/// darktable ignored fails too), and every other enabled module must be
/// plumbing ([`DARKTABLE_PLUMBING`]). A file with no history fails: a
/// sidecar darktable ignored would otherwise score as its default.
pub fn check_darktable_history(tif: &Path, h: &DtHighlights, xtrans: bool) -> Result<()> {
    let bytes = std::fs::read(tif).with_context(|| format!("reading {}", tif.display()))?;
    let text = String::from_utf8_lossy(&bytes);
    let entries = darktable_entries(&text).with_context(|| {
        format!(
            "{} carries no darktable history: cannot confirm the highlight method (export with the develop history in the metadata)",
            tif.display()
        )
    })?;
    let last = |op: &str| {
        entries
            .iter()
            .filter(|e| e.operation == op)
            .max_by_key(|e| e.num)
    };
    let wanted = darktable_history(h, xtrans);
    for (op, version, params) in &wanted {
        let e = last(op)
            .with_context(|| format!("{}: darktable's history has no {op} entry", tif.display()))?;
        if !e.enabled || e.modversion != *version || e.params != *params {
            bail!(
                "{}: darktable ran {op} v{} {} with params {}, not v{version} with {params}: the sidecar was not honored",
                tif.display(),
                e.modversion,
                if e.enabled { "on" } else { "off" },
                e.params
            );
        }
    }
    let mut ops: Vec<&str> = entries.iter().map(|e| e.operation.as_str()).collect();
    ops.sort_unstable();
    ops.dedup();
    for op in ops {
        let on = last(op).is_some_and(|e| e.enabled);
        if on && !DARKTABLE_PLUMBING.contains(&op) && !wanted.iter().any(|(w, _, _)| *w == op) {
            bail!(
                "{}: darktable also ran {op}, which the comparison wants off",
                tif.display()
            );
        }
    }
    Ok(())
}

/// The RawTherapee profile: neutral throughout, the method's highlight
/// recovery on with its own defaults (no `Hlbl` or `Hlth`), the same
/// demosaic as the engine's cases, the file's matrix, Rec.2020 working
/// and output space, exposure pulled down by [`PULL`]. `Coloropp`
/// (inpaint opposed) needs RawTherapee 5.10. [`RT_OFF`] turns the
/// recovery off: RawTherapee's clip.
pub fn rawtherapee_pp3(method: &str, xtrans: bool) -> String {
    let (enabled, named) = if method == RT_OFF {
        (false, "Coloropp")
    } else {
        (true, method)
    };
    let demosaic = if xtrans {
        "[RAW X-Trans]\nMethod=3-pass (best)\nCcSteps=0\n"
    } else {
        "[RAW Bayer]\nMethod=rcd\nCcSteps=0\nLineDenoise=0\nGreenEqThreshold=0\n"
    };
    format!(
        "[Version]\nAppVersion=5.10\nVersion=351\n\n\
[Exposure]\nAuto=false\nClip=0.02\nCompensation=-{PULL}\nBrightness=0\nContrast=0\nSaturation=0\nBlack=0\nHighlightCompr=0\nHighlightComprThreshold=0\nShadowCompr=0\nHistogramMatching=false\nCurveMode=Standard\nCurveMode2=Standard\nCurve=0;\nCurve2=0;\n\n\
[HLRecovery]\nEnabled={enabled}\nMethod={named}\n\n\
[White Balance]\nEnabled=true\nSetting=Camera\n\n\
[Color Management]\nInputProfile=(camera)\nToneCurve=false\nApplyLookTable=false\nApplyBaselineExposureOffset=false\nApplyHueSatMap=false\nWorkingProfile=Rec2020\nWorkingTRC=none\nOutputProfile=RTv4_Rec2020\nOutputProfileIntent=Relative\nOutputBPC=true\n\n\
[RAW]\nCA=false\nCAAvoidColourshift=true\nHotPixelFilter=false\nDeadPixelFilter=false\nHotDeadPixelThresh=100\nPreExposure=1\n\n\
{demosaic}\n\
[Sharpening]\nEnabled=false\n\n[SharpenMicro]\nEnabled=false\n\n[Vibrance]\nEnabled=false\n\n\
[Luminance Curve]\nEnabled=false\n\n[Shadows & Highlights]\nEnabled=false\n\n\
[Directional Pyramid Denoising]\nEnabled=false\n\n[Impulse Denoising]\nEnabled=false\n\n\
[Defringing]\nEnabled=false\n\n[LensProfile]\nLcMode=none\n\n[Dehaze]\nEnabled=false\n\n\
[Local Contrast]\nEnabled=false\n\n[Color appearance]\nEnabled=false\n\n[RGB Curves]\nEnabled=false\n\n\
[Black & White]\nEnabled=false\n\n[Film Simulation]\nEnabled=false\n\n[Resize]\nEnabled=false\n"
    )
}

/// What RawTherapee's own copy of the profile it ran (`-O` writes it
/// beside the output, every key merged over its neutral defaults) must
/// say: section, key, value.
pub fn rawtherapee_expected(
    method: &str,
    xtrans: bool,
) -> Vec<(&'static str, &'static str, String)> {
    let mut v = vec![
        ("Exposure", "Compensation", format!("-{PULL}")),
        ("Exposure", "Auto", "false".into()),
        ("Exposure", "HighlightCompr", "0".into()),
        ("HLRecovery", "Enabled", (method != RT_OFF).to_string()),
        ("White Balance", "Setting", "Camera".into()),
        ("Color Management", "InputProfile", "(camera)".into()),
        ("Color Management", "ToneCurve", "false".into()),
        ("Color Management", "ApplyLookTable", "false".into()),
        ("Color Management", "WorkingProfile", "Rec2020".into()),
        ("Color Management", "WorkingTRC", "none".into()),
        ("Color Management", "OutputProfile", "RTv4_Rec2020".into()),
        ("LensProfile", "LcMode", "none".into()),
        ("RAW", "CA", "false".into()),
        ("RAW", "HotPixelFilter", "false".into()),
        ("RAW", "DeadPixelFilter", "false".into()),
    ];
    if method != RT_OFF {
        v.push(("HLRecovery", "Method", method.to_string()));
    }
    v.push(if xtrans {
        ("RAW X-Trans", "Method", "3-pass (best)".into())
    } else {
        ("RAW Bayer", "Method", "rcd".into())
    });
    v
}

/// The sections RawTherapee may have on (`Enabled=true`) besides the
/// highlight recovery: the white balance and the crop guide (a display
/// aid). Anything else fails the check.
pub const RAWTHERAPEE_ON: [&str; 2] = ["White Balance", "CropGuide"];

/// Confirm from the profile RawTherapee wrote beside its output that it
/// ran what was asked and nothing else: every key of
/// [`rawtherapee_expected`], and no other section on.
pub fn check_rawtherapee_profile(pp3: &Path, method: &str, xtrans: bool) -> Result<()> {
    let text =
        std::fs::read_to_string(pp3).with_context(|| format!("reading {}", pp3.display()))?;
    let mut section = "";
    let mut values: Vec<(&str, &str, &str)> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(s) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = s;
        } else if let Some((k, v)) = line.split_once('=') {
            values.push((section, k, v));
        }
    }
    let get = |s: &str, k: &str| {
        values
            .iter()
            .find(|(vs, vk, _)| *vs == s && *vk == k)
            .map(|(_, _, v)| *v)
    };
    for (s, k, want) in rawtherapee_expected(method, xtrans) {
        let got = get(s, k);
        // A number may come back as "-4" or "-4.0".
        let same = got.is_some_and(|g| {
            g == want || matches!((g.parse::<f64>(), want.parse::<f64>()), (Ok(a), Ok(b)) if a == b)
        });
        if !same {
            bail!(
                "{}: RawTherapee ran [{s}] {k}={}, not {want}: the profile was not honored",
                pp3.display(),
                got.unwrap_or("(missing)")
            );
        }
    }
    for (s, k, v) in &values {
        if *k == "Enabled" && *v == "true" && *s != "HLRecovery" && !RAWTHERAPEE_ON.contains(s) {
            bail!(
                "{}: RawTherapee also ran [{s}], which the comparison wants off",
                pp3.display()
            );
        }
    }
    Ok(())
}

/// A tool's picture where the input clipped, sampled: luminance and
/// Oklab ab at unit luminance, every few pixels. Two methods of one tool
/// compared on these that agree are logged.
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
            "{} and {} give the same picture where the input clipped (median {s:.4} stop, ab {d:.5})",
            a.0,
            b.0
        );
    }
    Ok(())
}

/// Write `method`'s sidecar beside `dng` and return its path.
pub fn write_sidecar(method: &Method, dir: &Path, xtrans: bool) -> Result<PathBuf> {
    let (name, body) = match method.code {
        Code::Darktable(h) => (format!("{}.xmp", method.label), darktable_xmp(&h, xtrans)),
        Code::RawTherapee(m) => (format!("{}.pp3", method.label), rawtherapee_pp3(m, xtrans)),
    };
    let path = dir.join(name);
    std::fs::write(&path, body).with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

/// The command that develops `dng` with `sidecar` into `out` (a float
/// TIFF). Both run on a configuration and cache of their own under
/// `scratch`, never the user's. darktable by its own flags, on every
/// platform: a configuration and cache folder, an in-memory library, and
/// the CPU (its OpenCL path is another implementation, and failed on one
/// driver here). RawTherapee by `RT_SETTINGS` and `RT_CACHE`, which its
/// `Options::load` reads before any platform default (`%LOCALAPPDATA%`
/// on Windows, `~/Library/Application Support` on macOS, the XDG folders
/// elsewhere) and which must be absolute; the XDG variables are moved
/// too, for anything else of its that reads them on Linux.
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
                .arg("--disable-opencl")
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
            // RawTherapee refuses a relative RT_SETTINGS or RT_CACHE.
            let scratch = std::path::absolute(scratch).unwrap_or_else(|_| scratch.to_path_buf());
            c.env("RT_SETTINGS", scratch.join("rawtherapee-config"))
                .env("RT_CACHE", scratch.join("rawtherapee-cache"))
                .env("XDG_CONFIG_HOME", scratch.join("rawtherapee-config"))
                .env("XDG_CACHE_HOME", scratch.join("rawtherapee-cache"))
                // -O, not -o: the profile it ran is written beside the
                // output, for check_rawtherapee_profile.
                .arg("-O")
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
    read_output_within(path, READ_LIMIT)
}

/// The most a tool's TIFF may decode to, bytes: 2 GiB, over a float
/// TIFF of a 100 MP frame (1.2 GB). The TIFF decoder's own default, 256
/// MiB, is under one of a 24 MP frame (288 MB).
pub const READ_LIMIT: u64 = 2 << 30;

/// [`read_output`] with the decoder held to `limit` bytes.
pub fn read_output_within(path: &Path, limit: u64) -> Result<(Vec<f32>, usize, usize)> {
    use image::ImageDecoder;
    let file = std::io::BufReader::new(
        std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?,
    );
    let mut decoder = image::codecs::tiff::TiffDecoder::new(file)?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(limit);
    decoder.set_limits(limits)?;
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
    /// The slope of the tool's log luminance, after, against the
    /// engine's: 1 for a linear output, about 0.45 for a gamma-encoded
    /// one read as linear, below 1 for one through a tone curve.
    pub log_slope: f64,
    /// Median signed difference, stops, after, in four bands of the
    /// engine's luminance: 0.01 to 0.03, to 0.1, to 0.3, to 0.8. A
    /// linear output holds them together.
    pub bands: [f64; 4],
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
        log_slope: 1.0,
        bands: [0.0; 4],
    };
    let mut bands: [Vec<f32>; 4] = Default::default();
    let (mut sx, mut sy, mut sxx, mut sxy, mut n) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for &i in &picks {
        let o = [ours[i], ours[i + 1], ours[i + 2]];
        let t = [tool[i], tool[i + 1], tool[i + 2]];
        let mut f = t;
        draft.apply(&mut f);
        let (yo, yt, yf) = (luma(o), luma(t), luma(f).max(1e-6));
        before.push((yt / yo).log2().abs());
        let d = (yf / yo).log2();
        after.push(d.abs());
        let band = [0.03, 0.1, 0.3].iter().filter(|&&b| yo >= b).count();
        bands[band].push(d);
        let (lx, ly) = (f64::from(yo.log2()), f64::from(yf.log2()));
        (sx, sy, sxx, sxy, n) = (sx + lx, sy + ly, sxx + lx * lx, sxy + lx * ly, n + 1.0);
        let (lo, lf) = (lab.of(o.map(|v| v / yo)), lab.of(f.map(|v| v / yf)));
        dab.push((lo[1] - lf[1]).hypot(lo[2] - lf[2]));
    }
    let median = |v: &mut Vec<f32>| {
        if v.is_empty() {
            return f64::NAN;
        }
        let k = v.len() / 2;
        f64::from(*v.select_nth_unstable_by(k, f32::total_cmp).1)
    };
    Ok(Fit {
        matrix,
        offset: (0, 0),
        stops_before: median(&mut before),
        stops_after: median(&mut after),
        dab_after: median(&mut dab),
        log_slope: (n * sxy - sx * sy) / (n * sxx - sx * sx),
        bands: bands.each_mut().map(median),
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
/// embedded history that it ran the method asked for and nothing else.
fn run_checked(
    method: &Method,
    dng: &Path,
    sidecar: &Path,
    out: &Path,
    dir: &Path,
    xtrans: bool,
) -> Result<()> {
    let pp3 = rawtherapee_profile_of(out);
    let _ = std::fs::remove_file(&pp3);
    run(command(method, dng, sidecar, out, dir))?;
    match method.code {
        Code::Darktable(h) => check_darktable_history(out, &h, xtrans),
        Code::RawTherapee(m) => check_rawtherapee_profile(&pp3, m, xtrans),
    }
    .with_context(|| format!("{} on {}", method.label, dng.display()))
}

/// Where `rawtherapee-cli -O` writes the profile of an output.
pub fn rawtherapee_profile_of(out: &Path) -> PathBuf {
    let mut s = out.as_os_str().to_owned();
    s.push(".pp3");
    PathBuf::from(s)
}

/// Develop `dng` with a tool's `method` into `dir`, read it back and
/// put it on the engine's frame: through the fit's matrix when there is
/// a fit, placed at its offset, the gaps (only ever in the border no
/// score reads) filled from `fill` as it is.
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
    run_checked(method, dng, &sidecar, &out, dir, xtrans)?;
    let (mut rgb, tw, th) = read_output(&out)?;
    // A float TIFF is 12 bytes a pixel; once read and checked it is
    // dropped (the sidecar and the DNG remain to repeat it).
    let _ = std::fs::remove_file(&out);
    let offset = match fit {
        Some(f) => f.offset,
        None => align(&rgb, (tw, th), fill, size)?,
    };
    // The matrix acts on the tool's pixels alone: applied after placing,
    // it would also move the fill's.
    if let Some(f) = fit {
        f.apply(&mut rgb);
    }
    Ok(placed(&rgb, (tw, th), offset, fill, size))
}

/// The per-frame fit of a tool: its default method on the unclipped
/// round trip `dng`, against `ours`, the engine's develop of the same
/// file. Also returns the tool's picture of that file through the fit,
/// on the engine's frame: the tool's own unclipped develop, which, scaled
/// by a case's overexposure, is its truth.
pub fn validate(
    tool: Tool,
    dng: &Path,
    dir: &Path,
    xtrans: bool,
    ours: &[f32],
    size: (usize, usize),
) -> Result<(Fit, Vec<f32>)> {
    let method = tool.default_method();
    let sidecar = write_sidecar(method, dir, xtrans)?;
    let out = dir.join(format!("valid-{}.tif", method.label));
    let _ = std::fs::remove_file(&out);
    run_checked(method, dng, &sidecar, &out, dir, xtrans)?;
    let (mut rgb, tw, th) = read_output(&out)?;
    // A float TIFF is 12 bytes a pixel; once read and checked it is
    // dropped (the sidecar and the DNG remain to repeat it).
    let _ = std::fs::remove_file(&out);
    let offset = align(&rgb, (tw, th), ours, size)?;
    let mut f = fit(&placed(&rgb, (tw, th), offset, ours, size), ours, size)?;
    f.offset = offset;
    // A tone curve or an encoding the reader did not undo would scale a
    // reconstruction's error with its brightness.
    if (f.log_slope - 1.0).abs() > 0.05 {
        bail!(
            "{}'s output is not linear: its log luminance runs at {:.3} of the engine's (bands {:?})",
            tool.binary(),
            f.log_slope,
            f.bands
        );
    }
    f.apply(&mut rgb);
    Ok((f, placed(&rgb, (tw, th), offset, ours, size)))
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

    /// The blobs, against what darktable 5.6.2 itself wrote into the
    /// history of an export of a default develop (inpaint opposed, RCD,
    /// an exposure of −4 upgraded from version 6).
    #[test]
    fn the_darktable_blobs_are_darktable_5_6s_structs() {
        assert_eq!(
            darktable_params(&DtHighlights::mode(5)),
            "050000000000803f00000000000000000000803f000000001e00000006000000cdcccc3e000000400000000000000000"
        );
        assert_eq!(
            darktable_exposure(-PULL),
            "0000000000000000000080c000004842000080c00000000000000000"
        );
        // The capture sharpening's threshold (bytes 28 to 32) is set per
        // image by darktable and unused with it off.
        let written = "0000000000000000000000000500000001000000cdcc4c3e00000000713d8a3e00000000080000000000000000000000";
        let ours = darktable_demosaic(false);
        assert_eq!(ours.len(), written.len());
        assert_eq!(&ours[..56], &written[..56]);
        assert_eq!(&ours[64..], &written[64..]);
        // Markesteijn 3-pass, 1024 | 2, on X-Trans.
        assert_eq!(&darktable_demosaic(true)[24..32], "02040000");

        // The variants: laplacians at 128 iterations and 512 px, the
        // segmentation's generic rebuild at full strength.
        let Code::Darktable(hq) = DARKTABLE[4].code else {
            unreachable!()
        };
        let hq = darktable_params(&hq);
        assert_eq!(&hq[..8], "03000000");
        assert_eq!(&hq[48..64], "8000000008000000");
        let Code::Darktable(rebuild) = DARKTABLE[2].code else {
            unreachable!()
        };
        let rebuild = darktable_params(&rebuild);
        assert_eq!(&rebuild[24..32], "0000803f");
        assert_eq!(&rebuild[80..88], "05000000");

        let xmp = darktable_xmp(&DtHighlights::mode(4), false);
        assert!(xmp.contains("darktable:operation=\"highlights\""));
        assert!(xmp.contains("darktable:modversion=\"7\""));
        assert!(xmp.contains("darktable:history_end=\"3\""));
        assert!(xmp.contains("darktable:auto_presets_applied=\"1\""));
        assert!(xmp.contains(&darktable_params(&DtHighlights::mode(4))));
    }

    #[test]
    fn the_rawtherapee_profile_names_the_method_and_turns_the_rest_off() {
        let pp3 = rawtherapee_pp3("Coloropp", false);
        assert!(pp3.contains("[HLRecovery]\nEnabled=true\nMethod=Coloropp"));
        assert!(pp3.contains("Method=rcd"));
        assert!(pp3.contains(&format!("Compensation=-{PULL}")));
        assert!(pp3.contains("LcMode=none\n"));
        assert!(!pp3.contains("Hlth"));
        assert!(pp3.contains("[Sharpening]\nEnabled=false"));
        assert!(rawtherapee_pp3("Blend", true).contains("3-pass"));
        assert!(rawtherapee_pp3(RT_OFF, false).contains("[HLRecovery]\nEnabled=false\n"));
    }

    /// RawTherapee's own copy of the profile it ran is held to what was
    /// asked: a method it did not run, or a module left on, fails.
    #[test]
    fn rawtherapee_is_held_to_its_profile() {
        let dir = std::env::temp_dir().join(format!("greycard-hb-rt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pp3 = dir.join("out.tif.pp3");
        assert_eq!(rawtherapee_profile_of(&dir.join("out.tif")), pp3);
        // As RawTherapee writes it: every section, numbers its own way.
        let ran = rawtherapee_pp3("Color", false)
            .replace("Compensation=-4", "Compensation=-4.0")
            .replace(
                "[Sharpening]\nEnabled=false",
                "[Sharpening]\nEnabled=false\nMethod=usm",
            )
            + "\n[CropGuide]\nEnabled=true\n";
        std::fs::write(&pp3, &ran).unwrap();
        let ok = check_rawtherapee_profile(&pp3, "Color", false);
        let other = check_rawtherapee_profile(&pp3, "Blend", false);
        let xtrans = check_rawtherapee_profile(&pp3, "Color", true);
        std::fs::write(
            &pp3,
            ran.replace("[Sharpening]\nEnabled=false", "[Sharpening]\nEnabled=true"),
        )
        .unwrap();
        let sharpened = check_rawtherapee_profile(&pp3, "Color", false);
        greycard_core::testing::remove_dir_retry(&dir);
        assert!(ok.is_ok(), "{ok:?}");
        assert!(other.unwrap_err().to_string().contains("Method=Color"));
        assert!(xtrans.is_err());
        assert!(sharpened.unwrap_err().to_string().contains("[Sharpening]"));
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
        assert!(s.contains(&format!(
            "--core --disable-opencl --configdir {}",
            config.display()
        )));
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
            "rawtherapee-cli -O /w/out.tif -t -b32 -Y -p /w/rt.pp3 -c /w/in.dng"
        );
        // RawTherapee's configuration and cache are the scratch folder's.
        // RawTherapee's configuration and cache are the scratch folder's,
        // by the variables it reads on every platform, made absolute.
        let scratch = std::path::absolute(Path::new("/w")).unwrap();
        let envs: Vec<_> = rt.get_envs().collect();
        for (key, dir) in [
            ("RT_SETTINGS", "rawtherapee-config"),
            ("RT_CACHE", "rawtherapee-cache"),
            ("XDG_CONFIG_HOME", "rawtherapee-config"),
        ] {
            let want = scratch.join(dir);
            assert!(
                envs.contains(&(std::ffi::OsStr::new(key), Some(want.as_os_str()))),
                "{key}: {envs:?}"
            );
        }
        let relative = command(
            &RAWTHERAPEE[0],
            Path::new("in.dng"),
            Path::new("rt.pp3"),
            Path::new("out.tif"),
            Path::new("w"),
        );
        assert!(
            relative
                .get_envs()
                .all(|(_, v)| v.is_some_and(|v| Path::new(v).is_absolute()))
        );
        assert_eq!(Tool::Darktable.methods().len(), 7);
        assert_eq!(Tool::RawTherapee.methods().len(), 6);
    }

    /// The decoder is held to the limit given, not its own default: a
    /// float TIFF over a lowered limit fails and reads under a higher
    /// one, and the bench's limit takes a 100 MP frame where the TIFF
    /// decoder's default (256 MiB) failed a 24 MP one.
    #[test]
    fn a_float_tiff_reads_within_the_limit_given() {
        let (w, h) = (64u32, 48u32);
        let bytes = u64::from(3 * 4 * w * h);
        let mut px = vec![0.0f32; (3 * w * h) as usize];
        px[3 * (w as usize * 12 + 34) + 1] = 0.25;
        let dir = std::env::temp_dir().join(format!("greycard-hb-lim-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("small.tif");
        image::Rgb32FImage::from_raw(w, h, px)
            .unwrap()
            .save(&path)
            .unwrap();
        let under = read_output_within(&path, bytes / 2);
        let over = read_output_within(&path, 2 * bytes);
        greycard_core::testing::remove_dir_retry(&dir);
        assert!(under.is_err());
        let (rgb, rw, rh) = over.unwrap();
        assert_eq!((rw, rh), (w as usize, h as usize));
        assert_eq!(rgb[3 * (w as usize * 12 + 34) + 1], 0.25);
        // A float TIFF of a 100 MP frame (11648 x 8736) fits.
        const { assert!(READ_LIMIT > 12 * 11_648 * 8_736) };
    }

    /// The same at full size: a float TIFF of a 24 MP frame (288 MB)
    /// reads. Writes and reads that much, so run only when asked.
    #[test]
    #[ignore = "writes a 288 MB TIFF"]
    fn a_full_size_float_tiff_reads() {
        let (w, h) = (6000u32, 4000u32);
        let mut px = vec![0.0f32; (3 * w * h) as usize];
        px[3 * (w as usize * 1234 + 567) + 1] = 0.25;
        let dir = std::env::temp_dir().join(format!("greycard-hb-big-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("big.tif");
        image::Rgb32FImage::from_raw(w, h, px)
            .unwrap()
            .save(&path)
            .unwrap();
        let read = read_output(&path);
        greycard_core::testing::remove_dir_retry(&dir);
        let (rgb, rw, rh) = read.unwrap();
        assert_eq!((rw, rh), (w as usize, h as usize));
        assert_eq!(rgb[3 * (w as usize * 1234 + 567) + 1], 0.25);
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
            log_slope: 1.0,
            bands: [0.0; 4],
        }
        .apply(&mut tool);
        let f = fit(&tool, &ours, (w, h)).unwrap();
        for r in 0..3 {
            for c in 0..3 {
                assert!((f.matrix[r][c] - m[r][c]).abs() < 1e-3, "{:?}", f.matrix);
            }
        }
        assert!(f.stops_after < 1e-4 && f.stops_before > 0.0);
        assert!((f.log_slope - 1.0).abs() < 1e-3, "{}", f.log_slope);

        // A gamma-encoded output read as linear is caught by its slope.
        let encoded: Vec<f32> = tool.iter().map(|v| v.max(0.0).powf(1.0 / 2.2)).collect();
        let g = fit(&encoded, &ours, (w, h)).unwrap();
        assert!(g.log_slope < 0.6, "{}", g.log_slope);

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
    /// export whose history differs from the sidecar's, runs a module
    /// the comparison wants off, or has none, fails.
    #[test]
    fn darktable_is_held_to_its_sidecar() {
        let opposed = DtHighlights::mode(5);
        let xmp = darktable_xmp(&opposed, false);
        assert!(xmp.contains("darktable:operation=\"exposure\""));
        let pull = darktable_exposure(-PULL);
        assert_eq!(pull.len(), 56);
        assert_eq!(&pull[16..24], hex(&(-PULL).to_le_bytes()));
        assert!(xmp.contains(&pull));

        // An export's history as darktable writes it: the plumbing it
        // adds, then the sidecar's entries.
        let plumbing = r#"<darktable:history><rdf:Seq>
     <rdf:li darktable:num="0" darktable:operation="rawprepare" darktable:enabled="1" darktable:modversion="2" darktable:params="00"/>
     <rdf:li darktable:num="1" darktable:operation="temperature" darktable:enabled="1" darktable:modversion="4" darktable:params="00"/>
     <rdf:li darktable:num="2" darktable:operation="filmicrgb" darktable:enabled="0" darktable:modversion="7" darktable:params="00"/>"#;
        let ours: String = xmp
            .split("<rdf:li")
            .skip(1)
            .map(|li| li.split("/>").next().unwrap_or(""))
            .enumerate()
            .map(|(k, li)| {
                format!(
                    "<rdf:li{}/>\n",
                    li.replace(&format!("num=\"{k}\""), &format!("num=\"{}\"", k + 3))
                )
            })
            .collect();
        let export = |extra: &str, end: usize| {
            format!(
                "II*\0junk darktable:history_end=\"{end}\" {plumbing}\n{ours}{extra}</rdf:Seq></darktable:history>junk"
            )
        };
        let sigmoid = r#"<rdf:li darktable:num="6" darktable:operation="sigmoid" darktable:enabled="1" darktable:modversion="3" darktable:params="00"/>"#;
        let dir = std::env::temp_dir().join(format!("greycard-hb-dt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let tif = dir.join("out.tif");
        let mut results = Vec::new();
        for (text, h, xtrans) in [
            (export("", 6), opposed, false),
            (export("", 6), DtHighlights::mode(4), false),
            // A laplacians setting darktable did not take.
            (
                export("", 6),
                DtHighlights {
                    iterations: 128,
                    ..opposed
                },
                false,
            ),
            (export("", 6), opposed, true),
            (export(sigmoid, 7), opposed, false),
            // An entry past history_end is undone, not run.
            (export(sigmoid, 6), opposed, false),
            ("II*\0no history".to_string(), opposed, false),
        ] {
            std::fs::write(&tif, text).unwrap();
            results.push(check_darktable_history(&tif, &h, xtrans).map_err(|e| e.to_string()));
        }
        greycard_core::testing::remove_dir_retry(&dir);
        assert!(results[0].is_ok(), "{:?}", results[0]);
        assert!(results[1].as_ref().unwrap_err().contains("highlights"));
        assert!(results[2].is_err());
        assert!(results[3].as_ref().unwrap_err().contains("demosaic"));
        assert!(results[4].as_ref().unwrap_err().contains("sigmoid"));
        assert!(results[5].is_ok(), "{:?}", results[5]);
        assert!(
            results[6]
                .as_ref()
                .unwrap_err()
                .contains("no darktable history")
        );
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

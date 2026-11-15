//! greycard-highlight-bench: score highlight reconstruction against
//! ground truth made by clipping well-exposed raws. See the library's
//! `highlight` module for the method; in short:
//!
//! - `survey DIR...` decodes every raw and prints, per frame, the share
//!   of photosites near the white (unusable as truth) and the shares a
//!   1, 2 and 3 stop overexposure would clip, to choose frames by; with
//!   `--contact DIR`, contact sheets of the camera previews.
//! - `run --frames LIST --out DIR` overexposes each listed frame by
//!   every `--stops` that clips a useful share of it, develops the
//!   engine's methods and the oracles, scores them, and writes
//!   `frames.tsv`, `rows.csv`, `summary.csv`, `summary.md` and
//!   `sheets/index.html` (the `--sheets` most telling cases) under DIR.
//!   `--brackets DIR` adds pairs from real exposure brackets found
//!   there; `--external` adds darktable-cli's and rawtherapee-cli's
//!   methods when they are on PATH, and says so when they are not.
//! - `external --frames LIST --frame f01 --stops 2 --out DIR` writes one
//!   case's DNGs and sidecars, checks the engine's own round trip of the
//!   DNG, and prints (`--dry-run`) or runs the tools' commands.
//!
//! The frame list is one raw a line, optionally followed by a tab and a
//! word for what the frame shows. Nothing the harness writes names a
//! file except `frames.tsv`; the list, the output and every raw stay
//! outside the repository.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use greycard_bench::highlight::brackets::{agreement, bracket_case, find_pairs, raws};
use greycard_bench::highlight::external::{self, Tool};
use greycard_bench::highlight::report::{
    CaseInfo, CaseResult, case_label, incomplete, rows_csv, summary_csv, summary_md,
};
use greycard_bench::highlight::run::{
    Case, Domain, ORACLE_BASE, Truth, UNRELIABLE, demosaic_for, score_case, score_external,
    synthetic_case,
};
use greycard_bench::highlight::score::summarize;
use greycard_bench::highlight::sheet::{SheetWriter, page};
use greycard_bench::highlight::sim::{Scene, develop_scaled};
use greycard_core::WhitePoint;
use greycard_core::decode::{decode_path, is_raw_path, preview_path};
use greycard_core::develop::segments::SegmentStats;

#[derive(Parser)]
#[command(
    name = "greycard-highlight-bench",
    version,
    about = "Score highlight reconstruction against synthetic and real overexposure"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// One CSV line a raw: the shares near white and clipped at 1 to 3 stops
    Survey {
        inputs: Vec<PathBuf>,
        /// Write contact sheets of the camera previews here, in the
        /// survey's order, six across
        #[arg(long)]
        contact: Option<PathBuf>,
    },
    /// Score every listed frame and write the tables and sheets
    Run {
        #[command(flatten)]
        frames: Frames,
        #[arg(long)]
        out: PathBuf,
        /// Overexposures to try, stops
        #[arg(long, default_value = "1,2,3", value_delimiter = ',')]
        stops: Vec<f32>,
        /// An overexposure is run when it clips at least this share of
        /// the photosites…
        #[arg(long, default_value_t = 0.0005)]
        min_share: f64,
        /// …and at most this share
        #[arg(long, default_value_t = 0.4)]
        max_share: f64,
        /// A frame with more photosites than this share near its white
        /// is not truth and is skipped
        #[arg(long, default_value_t = 0.002)]
        max_unreliable: f64,
        /// Leave the oracles out
        #[arg(long)]
        no_oracles: bool,
        /// Picture sheets for this many cases
        #[arg(long, default_value_t = 10)]
        sheets: usize,
        /// Only these frames, by id (f01,f02,…)
        #[arg(long, value_delimiter = ',')]
        only: Vec<String>,
        /// Add pairs from real exposure brackets among the raws here
        #[arg(long)]
        brackets: Vec<PathBuf>,
        /// Seconds within which a bracket's frames must have been taken
        #[arg(long, default_value_t = 60)]
        bracket_window: i64,
        /// The largest bracket step paired, stops
        #[arg(long, default_value_t = 3.0)]
        bracket_stops: f64,
        /// Score darktable-cli's and rawtherapee-cli's methods too, when
        /// they are on PATH
        #[arg(long)]
        external: bool,
    },
    /// One case's DNGs and sidecars, the engine's DNG round trip, and the
    /// tools' commands
    External {
        #[command(flatten)]
        frames: Frames,
        #[arg(long)]
        out: PathBuf,
        /// The frame, by id
        #[arg(long)]
        frame: String,
        #[arg(long, default_value_t = 2.0)]
        stops: f32,
        /// Print the commands instead of running them
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(clap::Args)]
struct Frames {
    /// The frame list: one raw a line, optionally a tab and its kind
    #[arg(long = "frames")]
    list: PathBuf,
}

struct Listed {
    id: String,
    path: PathBuf,
    kind: String,
}

impl Frames {
    fn read(&self) -> Result<Vec<Listed>> {
        let text = std::fs::read_to_string(&self.list)
            .with_context(|| format!("reading {}", self.list.display()))?;
        let mut out = Vec::new();
        for line in text.lines() {
            let line = line.trim_end();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.splitn(2, '\t');
            let path = PathBuf::from(parts.next().unwrap_or_default());
            let kind = parts.next().unwrap_or("").trim().to_string();
            out.push(Listed {
                id: format!("f{:02}", out.len() + 1),
                path,
                kind,
            });
        }
        Ok(out)
    }
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Survey { inputs, contact } => survey(&inputs, contact.as_deref()),
        Command::Run {
            frames,
            out,
            stops,
            min_share,
            max_share,
            max_unreliable,
            no_oracles,
            sheets,
            only,
            brackets,
            bracket_window,
            bracket_stops,
            external,
        } => run(RunArgs {
            frames: frames.read()?,
            out,
            stops,
            min_share,
            max_share,
            max_unreliable,
            oracles: !no_oracles,
            sheets,
            only,
            brackets,
            bracket_window,
            bracket_stops,
            external,
        }),
        Command::External {
            frames,
            out,
            frame,
            stops,
            dry_run,
        } => external_one(&frames.read()?, &out, &frame, stops, dry_run),
    }
}

fn walk(inputs: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = inputs.to_vec();
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&p) {
                stack.extend(rd.filter_map(|e| e.ok().map(|e| e.path())));
            }
        } else if is_raw_path(&p) {
            out.push(p);
        }
    }
    out.sort();
    out
}

fn survey(inputs: &[PathBuf], contact: Option<&Path>) -> Result<()> {
    let files = walk(inputs);
    println!("idx,path,make,model,sensor,width,height,near_white,clip1,clip2,clip3,p999");
    let mut thumbs = Vec::new();
    for (k, path) in files.iter().enumerate() {
        let frame = match decode_path(path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("{k}: {e}");
                continue;
            }
        };
        let (make, model) = (frame.make.clone(), frame.model.clone());
        let scene = match Scene::new(frame) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{k}: {e}");
                continue;
            }
        };
        let mut sample: Vec<f32> = scene.normalized.iter().step_by(13).copied().collect();
        let at = ((sample.len() as f64 * 0.999) as usize).min(sample.len() - 1);
        let p999 = *sample.select_nth_unstable_by(at, f32::total_cmp).1;
        println!(
            "{k},{},{},{},{},{},{},{:.5},{:.5},{:.5},{:.5},{:.4}",
            path.display(),
            make,
            model,
            if scene.is_xtrans() { "xtrans" } else { "bayer" },
            scene.width,
            scene.height,
            scene.share_at_or_above(UNRELIABLE),
            scene.clipped_share(1.0),
            scene.clipped_share(2.0),
            scene.clipped_share(3.0),
            p999
        );
        if contact.is_some() {
            let thumb = preview_path(path)
                .ok()
                .flatten()
                .map(|(img, _)| image::imageops::thumbnail(&img, 200, 134));
            thumbs.push(thumb);
        }
    }
    if let Some(dir) = contact {
        std::fs::create_dir_all(dir)?;
        for (sheet, chunk) in thumbs.chunks(30).enumerate() {
            let mut grid = image::RgbImage::new(6 * 204, 5 * 138);
            for (n, t) in chunk.iter().enumerate() {
                if let Some(t) = t {
                    image::imageops::overlay(
                        &mut grid,
                        t,
                        (n % 6 * 204) as i64,
                        (n / 6 * 138) as i64,
                    );
                }
            }
            grid.save(dir.join(format!("contact-{sheet}.png")))?;
        }
    }
    Ok(())
}

struct RunArgs {
    frames: Vec<Listed>,
    out: PathBuf,
    stops: Vec<f32>,
    min_share: f64,
    max_share: f64,
    max_unreliable: f64,
    oracles: bool,
    sheets: usize,
    only: Vec<String>,
    brackets: Vec<PathBuf>,
    bracket_window: i64,
    bracket_stops: f64,
    external: bool,
}

/// The maker as the tables name it.
fn maker(make: &str) -> String {
    let m = make.to_ascii_lowercase();
    for (key, name) in [
        ("canon", "Canon"),
        ("nikon", "Nikon"),
        ("sony", "Sony"),
        ("fuji", "Fujifilm"),
        ("panasonic", "Panasonic"),
        ("olympus", "OM System"),
        ("om digital", "OM System"),
        ("leica", "Leica"),
        ("pentax", "Pentax"),
        ("ricoh", "Pentax"),
        ("google", "Google"),
        ("apple", "Apple"),
    ] {
        if m.contains(key) {
            return name.into();
        }
    }
    make.split_whitespace().next().unwrap_or("unknown").into()
}

fn camera(scene: &Scene) -> String {
    let make = maker(&scene.frame.make);
    let model = scene.frame.model.trim();
    if model
        .to_ascii_lowercase()
        .starts_with(&make.to_ascii_lowercase())
    {
        model.to_string()
    } else {
        format!("{make} {model}")
    }
}

fn info(
    scene: &Scene,
    id: &str,
    kind: &str,
    stops: String,
    case: &Case,
    segments: Option<SegmentStats>,
) -> CaseInfo {
    let clipped = case.map.clipped.iter().filter(|c| **c).count();
    CaseInfo {
        frame: id.to_string(),
        camera: camera(scene),
        maker: maker(&scene.frame.make),
        sensor: if scene.is_xtrans() { "xtrans" } else { "bayer" },
        kind: kind.to_string(),
        stops,
        clipped_share: clipped as f64 / case.map.clipped.len().max(1) as f64,
        counts: case.map.counts(),
        segments,
    }
}

/// The tools on PATH, saying which are not.
fn tools(wanted: bool) -> Vec<Tool> {
    if !wanted {
        return Vec::new();
    }
    Tool::ALL
        .into_iter()
        .filter(|t| {
            let found = t.find().is_some();
            if !found {
                eprintln!(
                    "{} is not on PATH: its methods are skipped (install {})",
                    t.binary(),
                    t.package()
                );
            }
            found
        })
        .collect()
}

/// The unclipped round trip of a frame for the tools: its DNG, and the
/// engine's develop of that DNG.
fn round_trip(scene: &Scene, dir: &Path, gains: [f32; 3]) -> Result<(PathBuf, Vec<f32>)> {
    std::fs::create_dir_all(dir)?;
    let dng = dir.join("valid.dng");
    external::write_dng(
        &dng,
        &scene.frame,
        &scene.normalized,
        (scene.width, scene.height),
        &scene.pattern,
        gains,
    )?;
    let back = Scene::new(decode_path(&dng)?)?;
    let ours = develop_scaled(
        &back.frame,
        1.0,
        demosaic_for(&back.pattern),
        WhitePoint::AsShot,
    )?
    .rgb;
    Ok((dng, ours))
}

/// The clipped input as a DNG.
fn clipped_dng(scene: &Scene, stops: f32, dir: &Path, gains: [f32; 3]) -> Result<PathBuf> {
    let factor = 2f32.powf(stops);
    let clipped: Vec<f32> = scene
        .normalized
        .iter()
        .map(|v| (v * factor).min(1.0))
        .collect();
    let dng = dir.join(format!("clipped-{stops}.dng"));
    external::write_dng(
        &dng,
        &scene.frame,
        &clipped,
        (scene.width, scene.height),
        &scene.pattern,
        gains,
    )?;
    Ok(dng)
}

fn run(args: RunArgs) -> Result<()> {
    let out = &args.out;
    std::fs::create_dir_all(out)?;
    let tools = tools(args.external);
    let started = Instant::now();
    let mut frames_tsv = String::from("id\tpath\tcamera\tsensor\tkind\tnear_white\tcases\n");
    let mut results: Vec<CaseResult> = Vec::new();
    // Every case's rows as they come, so a long run that stops keeps them.
    let mut rows = std::fs::File::create(out.join("rows.partial.csv"))?;
    let mut header_written = false;
    let mut flush = |results: &[CaseResult]| -> Result<()> {
        let mut text = rows_csv(results);
        if header_written {
            text = text
                .split_once('\n')
                .map(|(_, t)| t.to_string())
                .unwrap_or_default();
        }
        header_written = true;
        rows.write_all(text.as_bytes())?;
        Ok(())
    };

    for listed in &args.frames {
        if !args.only.is_empty() && !args.only.contains(&listed.id) {
            continue;
        }
        let t = Instant::now();
        let scene = match decode_path(&listed.path)
            .map_err(anyhow::Error::from)
            .and_then(Scene::new)
        {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{}: skipped, {e}", listed.id);
                frames_tsv += &format!(
                    "{}\t{}\t\t\t{}\t\tskipped: {e}\n",
                    listed.id,
                    listed.path.display(),
                    listed.kind
                );
                continue;
            }
        };
        let near = scene.share_at_or_above(UNRELIABLE);
        let mut ran = Vec::new();
        if near > args.max_unreliable {
            eprintln!("{}: skipped, {:.3}% near white", listed.id, 100.0 * near);
        } else {
            for &stops in &args.stops {
                let share = scene.clipped_share(stops);
                if share < args.min_share || share > args.max_share {
                    continue;
                }
                let case = synthetic_case(&scene, stops)?;
                let (mut records, segments) = score_case(&case, args.oracles, None)?;
                if !tools.is_empty() {
                    // A case a tool fails on keeps the engine's rows in
                    // rows.csv and says so; a long run goes on, and the
                    // case is left out of the summary and named at the
                    // end (`incomplete`).
                    match external_records(
                        &scene,
                        &case,
                        stops,
                        &tools,
                        &out.join("external").join(&listed.id),
                        None,
                    ) {
                        Ok(r) => records.extend(r),
                        Err(e) => eprintln!(
                            "{} at +{stops}: the outside tools not scored, {e:#}",
                            listed.id
                        ),
                    }
                }
                let result = CaseResult {
                    info: info(
                        &scene,
                        &listed.id,
                        &listed.kind,
                        format!("{stops}"),
                        &case,
                        segments,
                    ),
                    records,
                };
                flush(std::slice::from_ref(&result))?;
                results.push(result);
                ran.push(format!("{stops}"));
            }
        }
        frames_tsv += &format!(
            "{}\t{}\t{}\t{}\t{}\t{:.5}\t{}\n",
            listed.id,
            listed.path.display(),
            camera(&scene),
            if scene.is_xtrans() { "xtrans" } else { "bayer" },
            listed.kind,
            near,
            ran.join(",")
        );
        eprintln!(
            "{} ({}): stops [{}] in {:.0} s, {:.0} s so far",
            listed.id,
            camera(&scene),
            ran.join(","),
            t.elapsed().as_secs_f64(),
            started.elapsed().as_secs_f64()
        );
    }
    std::fs::write(out.join("frames.tsv"), &frames_tsv)?;

    let mut pairs_tsv =
        String::from("pair\tdark\tbright\tnominal\tmeasured\tshift\tblocks\tmoved\tdecision\n");
    if !args.brackets.is_empty() {
        let files = raws(&args.brackets);
        let pairs = find_pairs(&files, args.bracket_window, args.bracket_stops);
        eprintln!(
            "brackets: {} candidate pairs among {} raws",
            pairs.len(),
            files.len()
        );
        for (k, pair) in pairs.iter().enumerate() {
            let id = format!("b{:02}", k + 1);
            // A merge or a linear DNG of the same body probes alike; it
            // is no mosaic to clip, and the pair is passed by.
            let scenes = decode_path(&pair.dark)
                .map_err(anyhow::Error::from)
                .and_then(Scene::new)
                .and_then(|d| {
                    let b = Scene::new(decode_path(&pair.bright)?)?;
                    Ok((d, b))
                });
            let (dark, bright) = match scenes {
                Ok(s) => s,
                Err(e) => {
                    pairs_tsv += &format!(
                        "{id}\t{}\t{}\t{:.3}\t\t\t\tskipped: {e}\n",
                        pair.dark.display(),
                        pair.bright.display(),
                        pair.nominal
                    );
                    continue;
                }
            };
            let a = match agreement(&dark, &bright) {
                Ok(a) => a,
                Err(e) => {
                    pairs_tsv += &format!(
                        "{id}\t{}\t{}\t{:.3}\t\t\t\trejected: {e}\n",
                        pair.dark.display(),
                        pair.bright.display(),
                        pair.nominal
                    );
                    continue;
                }
            };
            let off = (a.ratio / pair.nominal).log2().abs();
            // Up to a tenth of the blocks may have moved: they and their
            // neighbors are left out of the scores.
            let decision = if a.moved > 0.1 {
                "rejected: moved"
            } else if off > 0.3 {
                "rejected: ratio far from nominal"
            } else {
                "kept"
            };
            pairs_tsv += &format!(
                "{id}\t{}\t{}\t{:.3}\t{:.3}\t{},{}\t{}\t{:.4}\t{decision}\n",
                pair.dark.display(),
                pair.bright.display(),
                pair.nominal,
                a.ratio,
                a.shift.0,
                a.shift.1,
                a.blocks,
                a.moved
            );
            eprintln!(
                "{id}: ratio {:.3} (nominal {:.3}), shift {:?}, {:.2}% of {} blocks moved: {decision}",
                a.ratio,
                pair.nominal,
                a.shift,
                100.0 * a.moved,
                a.blocks
            );
            if decision != "kept" {
                continue;
            }
            let stops = a.ratio.log2();
            let scored = bracket_case(dark, &bright, &a)
                .and_then(|case| score_case(&case, args.oracles, None).map(|r| (case, r)));
            let (case, (records, segments)) = match scored {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("{id}: not scored, {e}");
                    continue;
                }
            };
            let result = CaseResult {
                info: info(
                    &bright,
                    &id,
                    "bracket",
                    format!("bracket {stops:.1}"),
                    &case,
                    segments,
                ),
                records,
            };
            flush(std::slice::from_ref(&result))?;
            results.push(result);
        }
        std::fs::write(out.join("brackets.tsv"), &pairs_tsv)?;
    }

    std::fs::write(out.join("rows.csv"), rows_csv(&results))?;
    let _ = std::fs::remove_file(out.join("rows.partial.csv"));
    // The summary pools every method over the same cases: those every
    // method scored.
    let missing = incomplete(&results);
    let tabled: Vec<CaseResult> = results
        .iter()
        .filter(|c| !missing.contains(&case_label(&c.info)))
        .cloned()
        .collect();
    std::fs::write(out.join("summary.csv"), summary_csv(&tabled))?;
    let preamble = preamble(&tabled, &args);
    std::fs::write(out.join("summary.md"), summary_md(&tabled, &preamble))?;
    eprintln!("tables written in {:.0} s", started.elapsed().as_secs_f64());

    if args.sheets > 0 {
        sheets(&args, &tabled)?;
    }
    eprintln!("done in {:.0} s", started.elapsed().as_secs_f64());
    if !missing.is_empty() {
        bail!(
            "{} of {} cases lack a method (an outside tool failed on them; see the log) and are left out of the summary, kept in rows.csv: {}",
            missing.len(),
            results.len(),
            missing.join(", ")
        );
    }
    Ok(())
}

fn preamble(results: &[CaseResult], args: &RunArgs) -> String {
    let frames: std::collections::BTreeSet<&str> = results
        .iter()
        .filter(|r| !r.info.stops.starts_with("bracket"))
        .map(|r| r.info.frame.as_str())
        .collect();
    format!(
        "# Highlight reconstruction benchmark\n\n\
{} synthetic cases from {} frames (overexposures {:?} stops, each run when it clips {}% to {}% of the photosites), {} bracket pairs.\n\n\
Scores are taken only where the input was clipped (a photosite at or above 0.987 of the white, the engine's own threshold), in the working space (linear Rec.2020) after one shared demosaic (RCD on Bayer, Markesteijn 3-pass on X-Trans), split by regime (colors clipped in the 2x2 or 3x3 cell) and region size (small up to 256 px, medium up to 16384, large above). \
Every score is taken against two truths: the raw truth, nothing cut (what is lost), and the truth cut per photosite at the develop's ceiling, the largest gain, before the demosaic (what a method can do within today's engine, which cuts every reconstruction there). \
stops: log2 of luminance over the truth's; dab: Oklab ab distance with both at unit luminance; dE: Oklab distance with both scaled by the truth's luminance; contrast: the energy ratio of 5x5-smoothed log-luminance gradients, taken only where no smoothing window reaches past the clipped region's edge (the stated structure number; the gradients' correlation is in the CSV). \
Ceilings are in units of the develop's: ×1 the engine's methods, below for clip (it cuts at 1.0), raised to the truth's brightest for opposed-raised and oracle-raised. \
Pooled over cases, each case's regime-and-size bucket weighing at most {} pixels. The oracles start from {ORACLE_BASE}.\n",
        results
            .iter()
            .filter(|r| !r.info.stops.starts_with("bracket"))
            .count(),
        frames.len(),
        args.stops,
        100.0 * args.min_share,
        100.0 * args.max_share,
        results
            .iter()
            .filter(|r| r.info.stops.starts_with("bracket"))
            .count(),
        greycard_bench::highlight::score::POOL_CAP,
    )
}

/// The cases that tell the most: the frames where the default method is
/// furthest from the truth over a sizable clipped area, one case a
/// frame (its largest overexposure), and the brackets' worst.
fn sheets(args: &RunArgs, results: &[CaseResult]) -> Result<()> {
    let mut scored: Vec<(f64, &CaseResult)> = results
        .iter()
        .filter_map(|r| {
            let rec = r.records.iter().find(|x| {
                x.method == ORACLE_BASE && x.domain == Domain::Image && x.truth == Truth::Raw
            })?;
            let s = summarize(rec.buckets.iter().flatten(), f64::INFINITY);
            (s.n >= 2000).then_some((s.med_de, r))
        })
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut picked: Vec<&CaseResult> = Vec::new();
    for (_, r) in &scored {
        if picked.iter().any(|p| p.info.frame == r.info.frame) {
            continue;
        }
        // The frame's largest overexposure among its cases.
        let best = results
            .iter()
            .filter(|x| x.info.frame == r.info.frame)
            .max_by(|a, b| a.info.stops.cmp(&b.info.stops))
            .unwrap_or(r);
        picked.push(best);
        if picked.len() == args.sheets {
            break;
        }
    }
    let dir = args.out.join("sheets");
    let mut sections = Vec::new();
    for r in picked {
        let i = &r.info;
        eprintln!("sheet {} at {}", i.frame, i.stops);
        let case = if let Some(stops) = i.stops.strip_prefix("bracket ") {
            let _ = stops;
            continue;
        } else {
            let listed = args
                .frames
                .iter()
                .find(|l| l.id == i.frame)
                .context("listed frame")?;
            let scene = Scene::new(decode_path(&listed.path)?)?;
            synthetic_case(&scene, i.stops.parse()?)?
        };
        let title = format!(
            "{} · {} · {} · +{} stops · {:.2}% clipped",
            i.frame,
            i.camera,
            if i.kind.is_empty() { "–" } else { &i.kind },
            i.stops,
            100.0 * i.clipped_share
        );
        let mut w = SheetWriter::new(
            &dir,
            &format!("{}-{}", i.frame, i.stops),
            &title,
            &case.map,
            &case.truth.rgb,
        )?;
        score_case(&case, args.oracles, Some(&mut w))?;
        sections.push(w.finish());
    }
    std::fs::write(
        dir.join("index.html"),
        page(
            "Highlight reconstruction sheets",
            "Panels: the truth, the truth cut at the develop's ceiling (the largest gain), the input as clipped (no reconstruction), each method, opposed with the ceiling raised, the oracles. One tone mapping per frame, stated in each section.",
            &sections,
        ),
    )?;
    Ok(())
}

/// The tools' methods on one case, each fitted by the frame's round trip,
/// and each tool's own develop of the unclipped frame (`dt-unclipped`,
/// `rt-unclipped`): its floor against the engine's truth, and the truth
/// its methods are also scored against ([`Truth::Own`]). With a sheet,
/// each picture is added to it. Fails, rather than score it, when a
/// tool's round trip fails, when darktable's export shows another
/// history than asked, or when RawTherapee's own profile shows another
/// than asked: each means a sidecar was not honored. Two methods of
/// different algorithms that give the same picture where the input
/// clipped are logged, not failed: they may agree on a case.
fn external_records(
    scene: &Scene,
    case: &Case,
    stops: f32,
    tools: &[Tool],
    dir: &Path,
    mut sheet: Option<&mut SheetWriter>,
) -> Result<Vec<greycard_bench::highlight::run::Record>> {
    let gains = case.truth.gains;
    let size = (scene.width, scene.height);
    let factor = 2f32.powf(stops);
    let (valid, ours) = round_trip(scene, dir, gains)?;
    let dng = clipped_dng(scene, stops, dir, gains)?;
    let clipped: Vec<bool> = case
        .map
        .regime
        .iter()
        .map(|r| (1..=3).contains(r))
        .collect();
    let mut out = Vec::new();
    for &tool in tools {
        let (fit, unclipped) =
            external::validate(tool, &valid, dir, scene.is_xtrans(), &ours, size)
                .with_context(|| format!("{}: the unclipped round trip", tool.binary()))?;
        eprintln!(
            "{}: offset {:?}, residual {:.4} stops before the fit, {:.4} after, dab {:.4}, log slope {:.4}, bands {:.4?}, matrix {:.4?}",
            tool.binary(),
            fit.offset,
            fit.stops_before,
            fit.stops_after,
            fit.dab_after,
            fit.log_slope,
            fit.bands,
            fit.matrix
        );
        let own: Vec<f32> = unclipped.iter().map(|v| v * factor).collect();
        drop(unclipped);
        let floor = format!("{}-unclipped", tool.prefix());
        out.extend(score_external(case, &floor, &own, None));
        if let Some(s) = sheet.as_deref_mut() {
            s.add(&floor, &own)?;
        }
        let mut prints: Vec<(&external::Method, Vec<[f32; 3]>)> = Vec::new();
        for method in tool.methods() {
            let t = Instant::now();
            let rgb = external::develop_with(
                method,
                &dng,
                dir,
                scene.is_xtrans(),
                Some(&fit),
                &case.truth.rgb,
                size,
            )?;
            eprintln!(
                "{}: {:.1} s (the tool's whole run)",
                method.label,
                t.elapsed().as_secs_f64()
            );
            prints.push((method, external::fingerprint(&rgb, &clipped)));
            out.extend(score_external(case, method.label, &rgb, Some(&own)));
            if let Some(s) = sheet.as_deref_mut() {
                s.add(method.label, &rgb)?;
            }
        }
        // Methods that give the same picture where the input clipped are
        // said, not failed: what each tool ran is proved by its own
        // record (darktable's history, RawTherapee's profile), and two
        // methods may legitimately agree on a case (darktable's
        // segmentation is its opposed where no segment has a candidate;
        // RawTherapee's Blend and Luminance both hold the clip's
        // luminance).
        for (k, (method, print)) in prints.iter().enumerate() {
            for (other, p) in &prints[..k] {
                if other.code.family() != method.code.family()
                    && let Err(e) =
                        external::require_distinct((method.label, print), (other.label, p))
                {
                    eprintln!("note at +{stops}: {e}");
                }
            }
        }
    }
    Ok(out)
}

/// One case: its DNGs and sidecars and the tools' commands; unless a dry
/// run, every method of the engine's and the tools' scored, and a picture
/// sheet of them all under `out/sheets`.
fn external_one(
    frames: &[Listed],
    out: &Path,
    frame: &str,
    stops: f32,
    dry_run: bool,
) -> Result<()> {
    let listed = frames
        .iter()
        .find(|l| l.id == frame)
        .with_context(|| format!("no frame {frame} in the list"))?;
    let dir = out.join("external").join(frame);
    let scene = Scene::new(decode_path(&listed.path)?)?;
    let case = synthetic_case(&scene, stops)?;
    let gains = case.truth.gains;
    let size = (scene.width, scene.height);
    let (valid, ours) = round_trip(&scene, &dir, gains)?;
    let direct = develop_scaled(&scene.frame, 1.0, case.demosaic, WhitePoint::AsShot)?.rgb;
    let (s, d) = external::residual(&ours, &direct, size);
    println!(
        "the engine's develop of the round-trip DNG against its develop of the raw: median {s:.5} stops, dab {d:.5}"
    );
    drop((ours, direct));
    let dng = clipped_dng(&scene, stops, &dir, gains)?;
    println!("wrote {} and {}", valid.display(), dng.display());
    let mut present = Vec::new();
    for tool in Tool::ALL {
        if tool.find().is_some() {
            present.push(tool);
        } else {
            println!(
                "# {} is not on PATH (install {})",
                tool.binary(),
                tool.package()
            );
        }
        for method in tool.methods() {
            let sidecar = external::write_sidecar(method, &dir, scene.is_xtrans())?;
            let tif = dir.join(format!("clipped-{stops}-{}.tif", method.label));
            println!(
                "{}",
                external::shown(&external::command(method, &dng, &sidecar, &tif, &dir))
            );
        }
    }
    if dry_run {
        return bail_if_nothing(&dir);
    }
    let sheets = out.join("sheets");
    let mut sheet = SheetWriter::new(
        &sheets,
        &format!("{frame}-{stops}"),
        &format!("{frame} · {} · +{stops} stops", camera(&scene)),
        &case.map,
        &case.truth.rgb,
    )?;
    let (mut records, _) = score_case(&case, false, Some(&mut sheet))?;
    records.extend(external_records(
        &scene,
        &case,
        stops,
        &present,
        &dir,
        Some(&mut sheet),
    )?);
    std::fs::write(
        sheets.join("index.html"),
        page(
            "Highlight reconstruction, every developer",
            "Panels: the truth, the truth cut at the develop's ceiling, the input as clipped, the engine's methods, opposed with the ceiling raised, then each outside tool's develop of the unclipped frame and its methods, fitted to the engine's color by the frame's round trip. One tone mapping for the frame.",
            &[sheet.finish()],
        ),
    )?;
    for r in records.iter().filter(|r| r.domain == Domain::Image) {
        for regime in 0..3 {
            let s = summarize(r.buckets[regime].iter(), f64::INFINITY);
            if s.n == 0 {
                continue;
            }
            println!(
                "{:<22} {:<6} regime {}: med abs stops {:.3}, bias {:+.3}, med dE {:.4} ({} px)",
                r.method,
                r.truth.name(),
                regime + 1,
                s.med_abs_stops,
                s.med_stops,
                s.med_de,
                s.n
            );
        }
    }
    Ok(())
}

fn bail_if_nothing(dir: &Path) -> Result<()> {
    if !dir.join("valid.dng").is_file() {
        bail!("nothing written under {}", dir.display());
    }
    Ok(())
}

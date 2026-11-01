//! The list of stages a develop walks, derived once from the edit and
//! what the develop is for.
//!
//! Every develop of an edit, the viewport's, an export's, the reduced
//! picture's, the models', and the prefetch's, is one list of steps
//! made here: [`needs`] before the decode says what must be had, and
//! [`stages`] after it says what runs, in order, with every option
//! resolved from the edit, so two lists compare by `==` and a walk can
//! start from the last point a list before it kept. A stage the
//! purpose leaves out of what the edit asks for is named in
//! [`Steps::skipped`] with its reason.

use std::sync::Arc;

use greycard_core::color::Profile;
use greycard_core::develop::ca::{self, CaOptions};
use greycard_core::develop::defringe::DefringeOptions;
use greycard_core::develop::dehaze::DehazeOptions;
use greycard_core::develop::denoise::DenoiseOptions;
use greycard_core::develop::dual::DualContrast;
use greycard_core::develop::lens::LensCorrection;
use greycard_core::develop::local_contrast::LocalContrastOptions;
use greycard_core::develop::reduced;
use greycard_core::develop::sharpen::SharpenOptions;
use greycard_core::develop::{DemosaicMethod, DevelopSettings};
use greycard_core::raw::{CfaPattern, Orientation, RawFrame, SensorLayout, Shot};
use greycard_edit::geometry::Geometry;
use greycard_edit::mask::Shape;
use greycard_edit::retouch::Retouch;
use greycard_edit::{DetailWants, Edit};
use greycard_render::export;

/// What a develop is for: all that varies between the callers.
#[derive(Debug, Clone, PartialEq)]
pub enum Purpose {
    /// The open frame on screen: on the GPU where one is lent, ending
    /// at the viewport's shader.
    Viewport,
    /// A file under the sheet's settings: the CPU reference, ending at
    /// the file.
    Export(Box<export::Settings>),
    /// The edit's pictures of a frame not open: binned, the masks from
    /// the cache, finished to a 2048 sRGB picture.
    Reduced,
    /// What the learned masks and the person test are made on: the
    /// reduced develop without the retouch, through the Detail
    /// section, as they see it today.
    ModelsSee,
    /// A neighbor's base for the shelf, made ahead of its open. Its
    /// list is the viewport's to the base, the learned denoiser kept
    /// in it so the shelf's base serves the open; the prefetch's
    /// refusal of a learned tier and of a picture's file, which it
    /// leaves to the open, stays with its caller.
    Prefetch,
}

impl Purpose {
    /// Whether the develop is the reduced one, binned and with its
    /// skips.
    pub fn is_reduced(&self) -> bool {
        matches!(self, Purpose::Reduced | Purpose::ModelsSee)
    }
}

/// What the decode says that the list depends on.
#[derive(Debug, Clone, PartialEq)]
pub struct Facts {
    pub input: InputKind,
    /// The body and the shot, for the lens's profile.
    pub identity: Identity,
    /// The frame's quarter turns, clockwise, on top of the camera's
    /// tag.
    pub turn: u8,
}

/// A raw or a picture already rendered, with what the list needs of
/// each.
#[derive(Debug, Clone, PartialEq)]
pub enum InputKind {
    Raw {
        /// The mosaic's pattern; none for a linear raw.
        pattern: Option<CfaPattern>,
        /// The size the develop crops the sensor to, unturned.
        crop: (usize, usize),
        /// The camera's orientation tag.
        tag: Orientation,
    },
    /// A picture is turned by its tag at decode, so its size is the
    /// size it stands at.
    Picture { size: (usize, usize) },
}

/// What a file says of its body and its lens.
#[derive(Debug, Clone, PartialEq)]
pub struct Identity {
    pub make: String,
    pub model: String,
    pub shot: Shot,
}

impl Facts {
    /// A raw's facts at `turn`.
    pub fn raw(frame: &RawFrame, turn: u8) -> Self {
        let crop = frame.effective_crop();
        Self {
            input: InputKind::Raw {
                pattern: match &frame.layout {
                    SensorLayout::Cfa(p) => Some(p.clone()),
                    _ => None,
                },
                crop: (crop.width, crop.height),
                tag: frame.orientation,
            },
            identity: Identity {
                make: frame.make.clone(),
                model: frame.model.clone(),
                shot: frame.shot.clone(),
            },
            turn: turn % 4,
        }
    }

    /// A picture's facts at `turn`, its size as it was decoded.
    pub fn picture(size: (usize, usize), make: &str, model: &str, shot: &Shot, turn: u8) -> Self {
        Self {
            input: InputKind::Picture { size },
            identity: Identity {
                make: make.to_string(),
                model: model.to_string(),
                shot: shot.clone(),
            },
            turn: turn % 4,
        }
    }

    pub fn is_raw(&self) -> bool {
        matches!(self.input, InputKind::Raw { .. })
    }

    /// The orientation the develop shows a raw in: the camera's tag
    /// with the frame's turns on it.
    pub fn orientation(&self) -> Option<Orientation> {
        match &self.input {
            InputKind::Raw { tag, .. } => Some(tag.turned(i32::from(self.turn))),
            InputKind::Picture { .. } => None,
        }
    }

    /// The full develop's size as it stands, turned.
    pub fn full(&self) -> (usize, usize) {
        match &self.input {
            InputKind::Raw { crop, .. } => {
                if self.orientation().is_some_and(swaps) {
                    (crop.1, crop.0)
                } else {
                    *crop
                }
            }
            InputKind::Picture { size } => {
                if self.turn % 2 == 1 {
                    (size.1, size.0)
                } else {
                    *size
                }
            }
        }
    }

    /// The bin of the reduced develop under `geometry`: the mosaic's
    /// cell (or two for a picture or a linear raw) while the framed
    /// picture keeps that many times the 2048 on its long edge, else
    /// one. As the reduced develop chose it, with X-Trans taken up to
    /// a whole cell.
    pub fn bin_under(&self, geometry: &Geometry) -> usize {
        let cell = match &self.input {
            InputKind::Raw {
                pattern: Some(p), ..
            } => reduced::cell_of(p),
            _ => reduced::BIN,
        };
        let full = self.full();
        let (w, h) = framed_size((full.0 as u32, full.1 as u32), geometry);
        let bin = reduced::bin_for_cell(cell, w as usize, h as usize);
        match &self.input {
            InputKind::Raw {
                pattern: Some(p), ..
            } if bin > 1 && p.is_xtrans() => {
                bin.div_ceil(reduced::XTRANS_BIN) * reduced::XTRANS_BIN
            }
            InputKind::Raw { pattern: None, .. } => 1,
            _ => bin,
        }
    }

    /// The size of the picture a bin of `bin` makes, as it stands.
    fn binned(&self, bin: usize) -> (usize, usize) {
        let (w, h) = self.full();
        if bin <= 1 {
            return (w, h);
        }
        match &self.input {
            // The mosaic's rows and columns past the last whole cell
            // are left out, before the turn.
            InputKind::Raw { crop, .. } => {
                let (cw, ch) = (crop.0 / bin, crop.1 / bin);
                if self.orientation().is_some_and(swaps) {
                    (ch, cw)
                } else {
                    (cw, ch)
                }
            }
            InputKind::Picture { .. } => ((w / bin).max(1), (h / bin).max(1)),
        }
    }
}

/// Whether an orientation stands a picture on its side.
fn swaps(o: Orientation) -> bool {
    matches!(
        o,
        Orientation::Transpose
            | Orientation::Rotate90
            | Orientation::Transverse
            | Orientation::Rotate270
    )
}

/// The size of a develop of `full` once `geometry` has framed it, as
/// the geometry cuts it.
pub(crate) fn framed_size(full: (u32, u32), geometry: &Geometry) -> (u32, u32) {
    if geometry.is_identity() {
        return full;
    }
    let frame = geometry.frame(full.0 as f32, full.1 as f32);
    (frame.size.0 as u32, frame.size.1 as u32)
}

/// The engine's settings before the demosaic: the edit's white, its
/// camera profile, its demosaic, and the defaults it fixes (the
/// highlights, the hot pixels, the dual demosaic's contrast). The CA
/// is a stage of its own and the orientation is the move into the
/// working space's.
#[derive(Debug, Clone)]
pub struct PrepareOptions {
    pub settings: DevelopSettings,
    /// Whether the frame's noise is measured: the learned denoiser
    /// asks for it whatever the settings say.
    pub measure_noise: bool,
}

impl PartialEq for PrepareOptions {
    fn eq(&self, other: &Self) -> bool {
        // Every field named, so one added to the settings later cannot
        // be left out of the cache's key unseen.
        let DevelopSettings {
            white_point,
            profile,
            demosaic,
            highlights,
            segments,
            chromatic_aberration,
            dual_contrast,
            denoise,
            hot_pixels,
            orientation,
        } = &self.settings;
        let b = &other.settings;
        // A camera profile is read once and kept, so the same file is
        // the same pointer; a hundred kilobytes is not compared.
        same_profile(profile, &b.profile)
            && *white_point == b.white_point
            && *demosaic == b.demosaic
            && *highlights == b.highlights
            && *segments == b.segments
            && *chromatic_aberration == b.chromatic_aberration
            && *dual_contrast == b.dual_contrast
            && *denoise == b.denoise
            && *hot_pixels == b.hot_pixels
            && *orientation == b.orientation
            && self.measure_noise == other.measure_noise
    }
}

fn same_profile(a: &Option<Arc<Profile>>, b: &Option<Arc<Profile>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// One transform, its options resolved from the edit.
#[derive(Debug, Clone, PartialEq)]
pub enum Stage {
    /// Mosaic to mosaic: the levels, the noise, the hot pixels, the
    /// gains, the highlights.
    Prepare(Box<PrepareOptions>),
    /// The lateral chromatic aberration, on the mosaic.
    Ca(CaOptions),
    /// Mosaic to camera RGB.
    Demosaic(DemosaicMethod, DualContrast),
    /// Mosaic to camera RGB at a bin's fraction, in place of the
    /// demosaic.
    Bin(usize),
    /// Mosaic to the network's camera RGB and the plain demosaic's
    /// beside it.
    Learned(&'static greycard_ai::Model),
    /// The profiled denoise, on camera RGB.
    Denoise(DenoiseOptions),
    /// Camera RGB into the working space: the matrix, the camera
    /// profile's map, the orientation. Each of a pair.
    IntoWorking {
        profile: Option<ProfileRef>,
        orientation: Orientation,
    },
    /// The learned pair blended by strength.
    Blend(f32),
    /// A picture input's turn.
    Orient(Orientation),
    /// A picture input reduced by a whole factor, in place of the bin.
    Reduce(usize),
    /// The lens's corrections, in order; working to working from here
    /// on, the same size.
    Lens(Vec<LensCorrection>),
    Defringe(DefringeOptions),
    /// The retouch, its fills made or refused.
    Retouch(Retouch, Fills),
    LocalContrast(LocalContrastOptions),
    Dehaze(DehazeOptions),
    Sharpen(SharpenOptions),
    /// Working to what the purpose ends at.
    Finish(Finish),
}

/// A camera profile compared by pointer.
#[derive(Debug, Clone)]
pub struct ProfileRef(pub Arc<Profile>);

impl PartialEq for ProfileRef {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Stage {
    /// Whether the stage has a device version a plan can place on the
    /// GPU: the CA, the lens when it moves a pixel, the local
    /// contrast, the dehaze, and the sharpen.
    pub fn device(&self) -> bool {
        match self {
            Stage::Ca(_) | Stage::LocalContrast(_) | Stage::Dehaze(_) | Stage::Sharpen(_) => true,
            Stage::Lens(c) => c.iter().any(LensCorrection::moves),
            _ => false,
        }
    }

    /// Whether the stage's picture at one turn is its picture at
    /// another turned, to the bit. The lens is radial and the
    /// defringe symmetric in exact arithmetic, but a turn changes the
    /// order of their sums, so in floats neither is. A reduce drops
    /// the pixels past the last whole block, which a turn moves from
    /// the right and the bottom to other edges.
    pub fn commutes_with_turn(&self) -> bool {
        !matches!(self, Stage::Lens(_) | Stage::Defringe(_) | Stage::Reduce(_))
    }

    /// The stage's name, for the log and for a skip.
    pub fn name(&self) -> &'static str {
        match self {
            Stage::Prepare(_) => "prepare",
            Stage::Ca(_) => "CA",
            Stage::Demosaic(..) => "demosaic",
            Stage::Bin(_) => "bin",
            Stage::Learned(_) => "learned denoiser",
            Stage::Denoise(_) => "denoise",
            Stage::IntoWorking { .. } => "move into the working space",
            Stage::Blend(_) => "blend",
            Stage::Orient(_) => "orient",
            Stage::Reduce(_) => "reduce",
            Stage::Lens(_) => "lens",
            Stage::Defringe(_) => "defringe",
            Stage::Retouch(..) => "retouch",
            Stage::LocalContrast(_) => "local contrast",
            Stage::Dehaze(_) => "dehaze",
            Stage::Sharpen(_) => "sharpen",
            Stage::Finish(_) => "finish",
        }
    }
}

/// Whether a retouch's fills are made by their model or refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fills {
    Run,
    /// A fill refuses: the patch is left as it was and the picture is
    /// not the edit's.
    Refuse,
}

/// Where the walk ends.
#[derive(Debug, Clone, PartialEq)]
pub enum Finish {
    /// The viewport's shader runs it: the walk yields the picture and
    /// what the shader reads.
    Shader,
    /// The render crate's, on the CPU, then the border and the mark.
    Cpu {
        settings: Box<export::Settings>,
        fit: Fit,
    },
}

/// The size the CPU's finish makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// The export's long edge, its settings say.
    LongEdge,
    /// Exactly this long edge: the edit's pictures.
    Exact(u32),
}

/// How the learned masks' rasters are had at the tap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Masks {
    /// Made when a job asks: the open frame's.
    OnAsk,
    /// Made now if they are not yet: an export's.
    Make,
    /// Read from the cache alone: the reduced develop's.
    CacheOnly,
}

/// A product read off the picture where it stands; not a transform.
#[derive(Debug, Clone, PartialEq)]
pub enum Tap {
    /// The tone equalizer's read of the scene.
    Guide,
    /// The masks' detail bands, the haze read after the global's local
    /// contrast when one is asked for; beside the walk on a thread of
    /// their own, or inline.
    Planes {
        wants: DetailWants,
        haze_after: Option<LocalContrastOptions>,
        beside: bool,
    },
    /// The learned masks' rasters.
    Masks(Masks),
}

/// Where a walk leaves what it has for the next to start from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keep {
    /// The learned denoiser's pair, before the blend.
    Pair,
    /// The base: everything before the retouch.
    Base,
    /// The base with its retouch.
    Patched,
    /// The picture before the sharpen.
    PreSharpen,
    /// The developed picture, before its finish.
    Developed,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Op(Stage),
    Tap(Tap),
    Keep(Keep),
}

/// Why a stage the edit asks for is not in a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skip {
    /// The reduced develop leaves it out.
    Reduced,
    /// What the models see leaves it out.
    ModelsSee,
    /// The frame cannot take it; says why.
    Unavailable(&'static str),
}

/// A develop's steps in order, and what of the edit it leaves out.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Steps {
    pub steps: Vec<Step>,
    /// What the purpose left out of what the edit asks, by name, and
    /// why; a list with any is kept under the reduced recipe.
    pub skipped: Vec<(&'static str, Skip)>,
}

impl Steps {
    fn op(&mut self, stage: Stage) {
        self.steps.push(Step::Op(stage));
    }

    fn op_if(&mut self, stage: Option<Stage>) {
        if let Some(stage) = stage {
            self.op(stage);
        }
    }

    fn tap(&mut self, tap: Tap) {
        self.steps.push(Step::Tap(tap));
    }

    fn keep(&mut self, keep: Keep) {
        self.steps.push(Step::Keep(keep));
    }

    fn skip(&mut self, name: &'static str, why: Skip) {
        self.skipped.push((name, why));
    }

    /// The stages alone, in order.
    pub fn ops(&self) -> impl Iterator<Item = &Stage> {
        self.steps.iter().filter_map(|s| match s {
            Step::Op(stage) => Some(stage),
            _ => None,
        })
    }

    /// Where the list keeps `keep`, if it does.
    pub fn at(&self, keep: Keep) -> Option<usize> {
        self.steps.iter().position(|s| *s == Step::Keep(keep))
    }

    /// Whether the picture this list makes is kept under the reduced
    /// recipe: the reduced develop or the models' left something of
    /// the edit out. A stage the frame cannot take is not one: an
    /// X-Trans frame's learned tier stands the full develop in, which
    /// is the edit's picture.
    pub fn reduced_recipe(&self) -> bool {
        self.skipped
            .iter()
            .any(|(_, why)| matches!(why, Skip::Reduced | Skip::ModelsSee))
    }

    /// The list a learned denoiser that is not to be had stands in
    /// with: the engine's own demosaic in its place, the noise
    /// measured as the edit's settings ask and no pair to blend. The
    /// learned tier stops the profiled denoise, so none comes in.
    pub fn without_learned(&self) -> Steps {
        let mut out = Steps {
            steps: Vec::with_capacity(self.steps.len()),
            skipped: self.skipped.clone(),
        };
        for step in &self.steps {
            match step {
                Step::Op(Stage::Prepare(p)) => out.op(Stage::Prepare(Box::new(PrepareOptions {
                    settings: p.settings.clone(),
                    measure_noise: false,
                }))),
                Step::Op(Stage::Learned(_)) => {
                    let Some(Stage::Prepare(p)) = self.ops().next() else {
                        unreachable!("a learned denoiser follows a prepare");
                    };
                    out.op(Stage::Demosaic(
                        p.settings.demosaic,
                        p.settings.dual_contrast,
                    ));
                }
                Step::Keep(Keep::Pair) | Step::Op(Stage::Blend(_)) => {}
                step => out.steps.push(step.clone()),
            }
        }
        out
    }
}

/// What a develop must have before the decode.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Needs {
    /// The learned denoiser's model.
    pub denoiser: Option<&'static greycard_ai::Model>,
    /// The look the edit names, when it has strength.
    pub look: Option<String>,
    /// The learned shapes the edit draws, by adjustment id and
    /// component.
    pub masks: Vec<(u64, usize)>,
    /// Whether the retouch has a patch a model fills.
    pub fills: bool,
}

/// What `edit` needs under `purpose` before the decode: the models it
/// runs, the look, and the masks' rasters.
pub fn needs(edit: &Edit, purpose: &Purpose) -> Needs {
    let denoiser = match purpose {
        Purpose::Viewport | Purpose::Export(_) | Purpose::Prefetch => edit
            .noise
            .tier()
            .map(|t| greycard_ai::denoiser(t).expect("the edit's tiers are the registry's")),
        Purpose::Reduced | Purpose::ModelsSee => None,
    };
    let look = match &edit.look_lut.lut {
        greycard_edit::look::LutChoice::Named(name) if edit.look_lut.strength > 0.0 => {
            Some(name.clone())
        }
        _ => None,
    };
    let fills = !matches!(purpose, Purpose::ModelsSee | Purpose::Prefetch)
        && !edit.retouch.is_empty()
        && edit
            .retouch
            .patches
            .iter()
            .any(|p| p.method == greycard_edit::retouch::Method::Fill);
    Needs {
        denoiser,
        look,
        masks: learned_shapes(edit),
        fills,
    }
}

/// The learned shapes `edit` draws: an adjustment on, its mask live,
/// and a shape the models are prompted for.
pub fn learned_shapes(edit: &Edit) -> Vec<(u64, usize)> {
    let mut out = Vec::new();
    for a in &edit.adjustments {
        if !a.enabled || a.mask.is_empty() {
            continue;
        }
        for (i, c) in a.mask.live() {
            if c.shape.is_learned() && prompted(&c.shape) {
                out.push((a.id, i));
            }
        }
    }
    out
}

/// Whether a learned shape has what its model is prompted with: an
/// Object needs a pick or a box, a Landscape a feature.
pub fn prompted(shape: &Shape) -> bool {
    match shape {
        Shape::Subject {} | Shape::Background {} | Shape::Sky { .. } | Shape::Part { .. } => true,
        Shape::Landscape { .. } => !shape.is_unknown(),
        Shape::Object { picks, boxes } => !picks.is_empty() || !boxes.is_empty(),
        _ => false,
    }
}

/// The steps of `edit`'s develop for `purpose` on the frame `facts`
/// describe, with the lens corrections `lenses` holds for it.
pub fn stages(
    edit: &Edit,
    purpose: &Purpose,
    facts: &Facts,
    lenses: Option<&greycard_lens::Database>,
) -> Steps {
    let reduced = purpose.is_reduced();
    let bin = if reduced {
        facts.bin_under(&edit.geometry)
    } else {
        1
    };
    let mut s = Steps::default();
    let settings = edit.settings();
    match &facts.input {
        InputKind::Raw { pattern, .. } => {
            let tier = edit
                .noise
                .tier()
                .map(|t| greycard_ai::denoiser(t).expect("the edit's tiers are the registry's"));
            let xtrans = pattern.as_ref().is_some_and(CfaPattern::is_xtrans);
            let learned = match tier {
                Some(_) if reduced => {
                    s.skip("the learned denoiser", Skip::Reduced);
                    None
                }
                Some(_) if xtrans => {
                    s.skip(
                        "the learned denoiser",
                        Skip::Unavailable("the learned denoiser has no X-Trans version yet"),
                    );
                    None
                }
                tier => tier,
            };
            let denoise = settings.denoise.filter(|_| learned.is_none());
            if reduced && denoise.is_some() {
                s.skip("the denoise", Skip::Reduced);
            }
            let denoise = denoise.filter(|_| !reduced);
            s.op(Stage::Prepare(Box::new(PrepareOptions {
                settings: DevelopSettings {
                    chromatic_aberration: None,
                    denoise,
                    orientation: None,
                    ..settings.clone()
                },
                measure_noise: learned.is_some(),
            })));
            s.op_if(
                settings
                    .chromatic_aberration
                    .filter(|_| pattern.as_ref().is_some_and(ca::corrects))
                    .map(Stage::Ca),
            );
            match (bin, learned) {
                (b, _) if b > 1 && pattern.is_some() => s.op(Stage::Bin(b)),
                (_, Some(model)) => s.op(Stage::Learned(model)),
                _ => s.op(Stage::Demosaic(settings.demosaic, settings.dual_contrast)),
            }
            s.op_if(
                denoise
                    .filter(|_| bin == 1 || pattern.is_none())
                    .map(Stage::Denoise),
            );
            s.op(Stage::IntoWorking {
                profile: settings.profile.clone().map(ProfileRef),
                orientation: facts.orientation().expect("a raw's orientation"),
            });
            if learned.is_some() {
                s.keep(Keep::Pair);
                s.op(Stage::Blend(edit.noise.learned_strength));
            }
        }
        InputKind::Picture { .. } => {
            s.op_if(quarters(facts.turn).map(Stage::Orient));
            s.op_if((bin > 1).then_some(Stage::Reduce(bin)));
        }
    }
    let (w, h) = facts.binned(bin);
    if !edit.lens.is_identity(lenses.is_some()) {
        s.op(Stage::Lens(corrections(w, h, facts, edit, lenses)));
    }
    s.op_if(edit.lens.defringe().map(|mut o| {
        // The radius is in pixels, and the binned picture's are larger.
        if bin > 1 {
            o.radius /= bin as f32;
        }
        Stage::Defringe(o)
    }));
    if !learned_shapes(edit).is_empty() {
        s.tap(Tap::Masks(match purpose {
            Purpose::Viewport | Purpose::Prefetch => Masks::OnAsk,
            Purpose::Export(_) => Masks::Make,
            Purpose::Reduced | Purpose::ModelsSee => Masks::CacheOnly,
        }));
    }
    s.keep(Keep::Base);
    if *purpose == Purpose::Prefetch {
        return s;
    }
    if !edit.retouch.is_empty() {
        if *purpose == Purpose::ModelsSee {
            s.skip("the retouch", Skip::ModelsSee);
        } else {
            s.op(Stage::Retouch(
                edit.retouch.clone(),
                if reduced { Fills::Refuse } else { Fills::Run },
            ));
        }
    }
    if *purpose != Purpose::ModelsSee {
        s.tap(Tap::Guide);
    }
    s.keep(Keep::Patched);
    let wants = edit.detail_wants();
    let global = edit.detail.options();
    if wants.any() && *purpose != Purpose::ModelsSee {
        s.tap(Tap::Planes {
            wants,
            haze_after: global.filter(|_| wants.haze),
            beside: !reduced,
        });
    }
    s.op_if(global.map(Stage::LocalContrast));
    s.op_if(edit.detail.dehaze_options().map(Stage::Dehaze));
    if *purpose == Purpose::ModelsSee {
        return s;
    }
    s.keep(Keep::PreSharpen);
    match edit.sharpen.options() {
        Some(_) if reduced => s.skip("the capture sharpen", Skip::Reduced),
        options => s.op_if(options.map(Stage::Sharpen)),
    }
    s.keep(Keep::Developed);
    s.op(Stage::Finish(match purpose {
        Purpose::Viewport => Finish::Shader,
        Purpose::Export(settings) => Finish::Cpu {
            settings: settings.clone(),
            fit: Fit::LongEdge,
        },
        _ => Finish::Cpu {
            settings: Box::new(reduced_settings()),
            fit: Fit::Exact(BIG),
        },
    }));
    s
}

/// The long edge of the edit's pictures.
pub const BIG: u32 = 2048;

/// The edit's pictures' finish: sRGB, no output sharpen.
fn reduced_settings() -> export::Settings {
    export::Settings {
        sharpen: export::Sharpen::Off,
        ..Default::default()
    }
}

/// The lens's corrections for a picture `w` by `h` of the frame.
fn corrections(
    w: usize,
    h: usize,
    facts: &Facts,
    edit: &Edit,
    db: Option<&greycard_lens::Database>,
) -> Vec<LensCorrection> {
    let id = &facts.identity;
    let profile = db
        .and_then(|db| db.profile_for(&id.make, &id.model, &id.shot, facts.is_raw()))
        .map(|p| p.correction(w, h, &id.shot, greycard_lens::Wanted::ALL));
    edit.lens.corrections(profile)
}

/// The orientation of a picture input's turn, none for none.
pub fn quarters(turn: u8) -> Option<Orientation> {
    match turn % 4 {
        1 => Some(Orientation::Rotate90),
        2 => Some(Orientation::Rotate180),
        3 => Some(Orientation::Rotate270),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use greycard_edit::Adjustment;
    use greycard_edit::retouch::{Method, Patch};

    use super::*;

    /// A 24 MP Bayer raw, landscape, at `turn`.
    fn raw(turn: u8) -> Facts {
        Facts {
            input: InputKind::Raw {
                pattern: Some(CfaPattern::rggb()),
                crop: (6000, 4000),
                tag: Orientation::Normal,
            },
            identity: identity(),
            turn,
        }
    }

    fn xtrans() -> Facts {
        Facts {
            input: InputKind::Raw {
                pattern: Some(CfaPattern::xtrans()),
                crop: (7728, 5152),
                tag: Orientation::Normal,
            },
            ..raw(0)
        }
    }

    fn picture(turn: u8) -> Facts {
        Facts::picture((6000, 4000), "Canon", "EOS R5", &Shot::default(), turn)
    }

    fn identity() -> Identity {
        Identity {
            make: "Canon".into(),
            model: "EOS R5".into(),
            shot: Shot::default(),
        }
    }

    /// An edit that asks for every stage: the defringe, a retouch,
    /// the profiled denoise, the Detail section, and the sharpen.
    fn everything() -> Edit {
        let mut edit = Edit::default();
        edit.noise.profiled = true;
        edit.lens.defringe = true;
        edit.detail.clarity = 0.4;
        edit.detail.dehaze = 0.25;
        edit.retouch.patches.push(Patch {
            method: Method::Fill,
            ..Patch::default()
        });
        edit
    }

    fn names(s: &Steps) -> Vec<String> {
        s.steps
            .iter()
            .map(|step| match step {
                Step::Op(Stage::Bin(n)) => format!("bin {n}"),
                Step::Op(Stage::Reduce(n)) => format!("reduce {n}"),
                Step::Op(Stage::Retouch(_, Fills::Refuse)) => "retouch, fills refused".into(),
                Step::Op(Stage::Finish(Finish::Shader)) => "shader".into(),
                Step::Op(Stage::Finish(Finish::Cpu { fit, .. })) => format!("finish {fit:?}"),
                Step::Op(Stage::IntoWorking { .. }) => "into working".into(),
                Step::Op(stage) => stage.name().into(),
                Step::Tap(Tap::Guide) => "guide".into(),
                Step::Tap(Tap::Planes { beside, .. }) => format!("planes beside {beside}"),
                Step::Tap(Tap::Masks(m)) => format!("masks {m:?}"),
                Step::Keep(k) => format!("keep {k:?}"),
            })
            .collect()
    }

    fn export() -> Purpose {
        Purpose::Export(Box::default())
    }

    #[test]
    fn a_raw_s_list_for_each_purpose() {
        let edit = everything();
        let full = [
            "prepare",
            "CA",
            "demosaic",
            "denoise",
            "into working",
            "defringe",
            "keep Base",
            "retouch",
            "guide",
            "keep Patched",
            "local contrast",
            "dehaze",
            "keep PreSharpen",
            "sharpen",
            "keep Developed",
        ];
        let viewport = stages(&edit, &Purpose::Viewport, &raw(0), None);
        assert_eq!(names(&viewport)[..full.len()], full);
        assert_eq!(names(&viewport)[full.len()..], ["shader"]);
        assert!(viewport.skipped.is_empty());

        let export = stages(&edit, &export(), &raw(0), None);
        assert_eq!(names(&export)[..full.len()], full);
        assert_eq!(names(&export)[full.len()..], ["finish LongEdge"]);

        let reduced = stages(&edit, &Purpose::Reduced, &raw(0), None);
        assert_eq!(
            names(&reduced),
            [
                "prepare",
                "CA",
                "bin 2",
                "into working",
                "defringe",
                "keep Base",
                "retouch, fills refused",
                "guide",
                "keep Patched",
                "local contrast",
                "dehaze",
                "keep PreSharpen",
                "keep Developed",
                "finish Exact(2048)",
            ]
        );
        assert_eq!(
            reduced.skipped,
            [
                ("the denoise", Skip::Reduced),
                ("the capture sharpen", Skip::Reduced)
            ]
        );
        assert!(reduced.reduced_recipe());
        // The defringe's radius is in the binned picture's pixels.
        let radius = |s: &Steps| {
            s.ops()
                .find_map(|o| match o {
                    Stage::Defringe(d) => Some(d.radius),
                    _ => None,
                })
                .unwrap()
        };
        assert_eq!(radius(&reduced), radius(&viewport) / 2.0);

        let models = stages(&edit, &Purpose::ModelsSee, &raw(0), None);
        assert_eq!(
            names(&models),
            [
                "prepare",
                "CA",
                "bin 2",
                "into working",
                "defringe",
                "keep Base",
                "keep Patched",
                "local contrast",
                "dehaze",
            ]
        );
        assert!(models.skipped.contains(&("the retouch", Skip::ModelsSee)));

        let prefetch = stages(&edit, &Purpose::Prefetch, &raw(0), None);
        assert_eq!(names(&prefetch), full[..7]);
        // The shelf's base is the viewport's: the same steps to it.
        let base = viewport.at(Keep::Base).unwrap();
        assert_eq!(prefetch.steps, viewport.steps[..=base]);
    }

    #[test]
    fn a_picture_s_list_turns_and_reduces_it() {
        let edit = everything();
        let viewport = stages(&edit, &Purpose::Viewport, &picture(1), None);
        assert_eq!(names(&viewport)[..3], ["orient", "defringe", "keep Base"]);
        let unturned = stages(&edit, &Purpose::Viewport, &picture(0), None);
        assert_eq!(names(&unturned)[..2], ["defringe", "keep Base"]);
        let reduced = stages(&edit, &Purpose::Reduced, &picture(1), None);
        assert_eq!(
            names(&reduced)[..4],
            ["orient", "reduce 2", "defringe", "keep Base"]
        );
        // A picture is not denoised, so nothing of that is skipped.
        assert_eq!(reduced.skipped, [("the capture sharpen", Skip::Reduced)]);
    }

    #[test]
    fn a_learned_tier_makes_a_pair_and_blends_it() {
        let mut edit = everything();
        edit.noise.learned = greycard_edit::Learned::Best;
        edit.noise.learned_strength = 0.6;
        let viewport = stages(&edit, &Purpose::Viewport, &raw(0), None);
        assert_eq!(
            names(&viewport)[..7],
            [
                "prepare",
                "CA",
                "learned denoiser",
                "into working",
                "keep Pair",
                "blend",
                "defringe"
            ]
        );
        assert!(viewport.ops().any(|o| *o == Stage::Blend(0.6)));
        // The learned tier measures the noise, and stands in for the
        // profiled denoise, which does not run.
        let Some(Stage::Prepare(p)) = viewport.ops().next() else {
            panic!("a raw's list begins at its prepare");
        };
        assert!(p.measure_noise);
        assert!(p.settings.denoise.is_none());
        assert_eq!(needs(&edit, &Purpose::Viewport).denoiser.map(|m| m.id), {
            greycard_ai::denoiser("best").map(|m| m.id)
        });

        // Without its model, the engine's own path stands in.
        let stand_in = viewport.without_learned();
        assert_eq!(
            names(&stand_in)[..5],
            ["prepare", "CA", "demosaic", "into working", "defringe"]
        );

        // The reduced develop has no learned denoiser, and says so.
        let reduced = stages(&edit, &Purpose::Reduced, &raw(0), None);
        assert!(!reduced.ops().any(|o| matches!(o, Stage::Learned(_))));
        assert!(
            reduced
                .skipped
                .contains(&("the learned denoiser", Skip::Reduced))
        );
        assert_eq!(needs(&edit, &Purpose::Reduced).denoiser, None);

        // Nor does an X-Trans mosaic, which it has no version for; the
        // full develop stands in, which is the edit's picture, so the
        // list is not the reduced recipe's.
        let x = stages(&edit, &Purpose::Viewport, &xtrans(), None);
        assert!(!x.ops().any(|o| matches!(o, Stage::Learned(_))));
        assert!(matches!(
            x.skipped[..],
            [("the learned denoiser", Skip::Unavailable(_))]
        ));
        assert!(!x.reduced_recipe());
        assert!(!stages(&edit, &export(), &xtrans(), None).reduced_recipe());
    }

    #[test]
    fn the_bin_follows_the_mosaic_and_the_crop() {
        let edit = Edit::default();
        let x = stages(&edit, &Purpose::Reduced, &xtrans(), None);
        assert!(x.ops().any(|o| *o == Stage::Bin(3)));
        // A crop that leaves less than twice the 2048 develops in full.
        let mut cropped = Edit::default();
        cropped.geometry.crop = Some(greycard_edit::geometry::Crop {
            x: 0.0,
            y: 0.0,
            w: 0.5,
            h: 0.5,
        });
        let r = stages(&cropped, &Purpose::Reduced, &raw(0), None);
        assert!(r.ops().any(|o| matches!(o, Stage::Demosaic(..))));
        assert!(!r.ops().any(|o| matches!(o, Stage::Bin(_))));
    }

    #[test]
    fn a_learned_mask_is_tapped_at_the_base_as_the_purpose_has_it() {
        let mut edit = Edit::default();
        let mut a = Adjustment::default();
        a.mask.components.push(greycard_edit::mask::Component {
            shape: Shape::Subject {},
            ..Default::default()
        });
        edit.adjustments.push(a);
        let masks = |p: &Purpose| {
            stages(&edit, p, &raw(0), None)
                .steps
                .iter()
                .find_map(|s| match s {
                    Step::Tap(Tap::Masks(m)) => Some(*m),
                    _ => None,
                })
        };
        assert_eq!(masks(&Purpose::Viewport), Some(Masks::OnAsk));
        assert_eq!(masks(&export()), Some(Masks::Make));
        assert_eq!(masks(&Purpose::Reduced), Some(Masks::CacheOnly));
        assert_eq!(
            needs(&edit, &Purpose::Viewport).masks,
            [(edit.adjustments[0].id, 0)]
        );
        assert_eq!(masks(&Purpose::Viewport), masks(&Purpose::Prefetch));
    }

    #[test]
    fn a_list_equals_itself_and_not_one_option_moved() {
        let edit = everything();
        let a = stages(&edit, &Purpose::Viewport, &raw(0), None);
        assert_eq!(a, stages(&edit, &Purpose::Viewport, &raw(0), None));
        let moved = |change: &dyn Fn(&mut Edit)| {
            let mut e = edit.clone();
            change(&mut e);
            stages(&e, &Purpose::Viewport, &raw(0), None)
        };
        let sharper = moved(&|e| e.sharpen.iterations += 1);
        assert_ne!(a, sharper);
        // Everything up to the sharpen is the same, so the picture
        // before it can be kept.
        let pre = a.at(Keep::PreSharpen).unwrap();
        assert_eq!(a.steps[..=pre], sharper.steps[..=pre]);
        let hazier = moved(&|e| e.detail.dehaze = 0.5);
        let patched = a.at(Keep::Patched).unwrap();
        assert_eq!(a.steps[..=patched], hazier.steps[..=patched]);
        assert_ne!(a.steps[..=pre], hazier.steps[..=pre]);
        let warmer = moved(&|e| {
            e.white_balance = greycard_edit::WhiteBalance::Custom {
                temperature: 3200.0,
                tint: 0.0,
            }
        });
        assert_ne!(a.steps[0], warmer.steps[0]);
        // A turn moves the step into the working space and nothing
        // before it.
        let turned = stages(&edit, &Purpose::Viewport, &raw(1), None);
        assert_eq!(a.steps[..4], turned.steps[..4]);
        assert_ne!(a.steps[4], turned.steps[4]);
        // An exposure is the finish's, which reads it from the edit.
        assert_eq!(a, moved(&|e| e.light.exposure = 1.0));
    }
}

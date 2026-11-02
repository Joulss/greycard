//! The walk: one function that runs a list of steps, from the last
//! point a walk before it kept, with each stage on the CPU or the
//! device as a plan places it.
//!
//! What a walk keeps is data: at each [`Keep`] it passes it stores the
//! picture, what the develop measured of it, the taps so far, the
//! steps up to there, and where each ran. The next walk starts from
//! the last kept point whose steps are its own and whose places its
//! plan allows, so an export, whose plan is all CPU, never starts from
//! a picture the device touched.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use greycard_core::develop::ca::{self, CaStats};
use greycard_core::develop::dehaze::{self, DehazeStats};
use greycard_core::develop::denoise;
use greycard_core::develop::local_contrast::{LocalContrastStats, local_contrast};
use greycard_core::develop::retouch::Region;
use greycard_core::develop::sharpen::{SharpenStats, sharpen_with_mask};
use greycard_core::develop::warp::{WarpPlacement, apply_warps};
use greycard_core::develop::{
    self as core_develop, DemosaicMethod, DevelopSettings, Prepared, demosaic_prepared,
    prepare_with,
};
use greycard_core::image::WorkingImage;
use greycard_core::picture::Picture;
use greycard_core::raw::{Orientation, RawFrame};
use greycard_core::{CameraProfile, WhiteBalance};
use greycard_edit::retouch::{Patch, Retouch};
use greycard_edit::{DetailWants, WhiteShift};
use greycard_gpu::wgpu;
use greycard_render::detail::DetailPlanes;
use greycard_render::finish::{Guide, Source, guide_plane};

use crate::stages::{Facts, Fills, Keep, Masks, Purpose, Stage, Step, Steps, Tap, quarters};

/// The open file: a raw to develop, or a picture already in the
/// working space, its image shared rather than copied.
#[derive(Clone)]
pub enum Input {
    Raw(Arc<RawFrame>),
    Picture {
        image: Arc<WorkingImage>,
        meta: Arc<Picture>,
    },
}

impl Input {
    /// What the list is derived from, at the frame's `turn`.
    pub fn facts(&self, turn: u8) -> Facts {
        match self {
            Input::Raw(frame) => Facts::raw(frame, turn),
            Input::Picture { image, meta } => Facts::picture(
                (image.width, image.height),
                &meta.make,
                &meta.model,
                &meta.shot,
                turn,
            ),
        }
    }
}

/// Where a step runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Cpu,
    Gpu,
}

/// Where each step of `steps` runs for `purpose`, with a device `lent`
/// or not: every stage with a device version on it for the viewport
/// and the prefetch, everything on the CPU for the rest, which are
/// the reference.
pub fn plan(steps: &Steps, purpose: &Purpose, lent: bool) -> Vec<Place> {
    let device = lent && matches!(purpose, Purpose::Viewport | Purpose::Prefetch);
    steps
        .steps
        .iter()
        .map(|step| match step {
            Step::Op(Stage::Finish(crate::stages::Finish::Shader)) if device => Place::Gpu,
            Step::Op(stage) if device && stage.device() => Place::Gpu,
            _ => Place::Cpu,
        })
        .collect()
}

/// What the picture is between two steps.
pub enum Buf {
    /// One sample a site, balanced, in camera space.
    Mosaic(Prepared),
    /// Camera RGB, with what the move into the working space reads of
    /// the prepare.
    Camera {
        prepared: Prepared,
        rgb: Vec<f32>,
    },
    /// The learned denoiser's camera RGB and the plain demosaic's.
    CameraPair {
        prepared: Prepared,
        network: Vec<f32>,
        plain: Vec<f32>,
    },
    /// The pair in the working space.
    Pair {
        model: Arc<WorkingImage>,
        plain: Arc<WorkingImage>,
    },
    Working(Arc<WorkingImage>),
    /// A picture on the device.
    Device(Arc<greycard_gpu::Image>),
    /// The viewport's texture, which a run ending at the shader wrote.
    Texture(wgpu::Texture),
}

/// What the develop measured of the picture, carried beside it.
#[derive(Debug, Clone)]
pub struct Meta {
    /// A raw's white; none for a picture already rendered.
    pub white: Option<WhiteBalance>,
    /// The working-space value a channel clips at.
    pub clip_level: f32,
    /// The point spread the mosaic suggests, for an automatic sharpen.
    pub radius: Option<f32>,
    /// A raw's scene or a picture already rendered, for the finish.
    pub source: Source,
    /// The camera profile a raw's white was resolved through.
    pub profile: Option<Arc<CameraProfile>>,
    /// The full develop's size as it stands, before any bin.
    pub full: (u32, u32),
    /// The bin the picture was made at.
    pub bin: usize,
    /// Why the demosaic the edit chose did not run, when one of the
    /// frame's own stood in.
    pub gave_way: Option<String>,
    /// Why the learned denoiser did not run, when the engine's own
    /// path stood in for it: carried with the picture, so a walk that
    /// starts past the stand-in still says so.
    pub stood_in: Option<NoLearned>,
}

impl Meta {
    /// Nothing measured yet.
    pub fn empty() -> Self {
        Self {
            white: None,
            clip_level: f32::INFINITY,
            radius: None,
            source: Source::default(),
            profile: None,
            full: (0, 0),
            bin: 1,
            gave_way: None,
            stood_in: None,
        }
    }

    /// The masks' white balances are made from this develop's white
    /// and the profile it was resolved through.
    pub fn white_shift(&self) -> Option<WhiteShift> {
        let (profile, wb) = (self.profile.as_ref()?, self.white.as_ref()?);
        let cols = wb.matrix_f32();
        let rows = std::array::from_fn(|r| std::array::from_fn(|c| cols[c][r]));
        Some(WhiteShift::from_parts(
            (**profile).clone(),
            wb.coefficients_f32(),
            rows,
            wb.temp_tint,
        ))
    }
}

/// The products read off the picture so far.
#[derive(Clone, Default)]
pub struct Taps {
    pub guide: Option<Arc<Guide>>,
    pub planes: Option<Arc<DetailPlanes>>,
    /// The picture the learned masks are made on, and how.
    pub masks: Option<(Masks, Arc<WorkingImage>)>,
    /// The picture the refined masks are made against.
    pub picture: Option<Arc<greycard_render::refine::MaskPicture>>,
}

impl Taps {
    /// The guide plane, with the refined masks' picture beside it when
    /// one was made: what every finish of this walk reads.
    pub fn guide_for_finish(&self) -> Option<Arc<Guide>> {
        match (&self.guide, &self.picture) {
            (Some(g), Some(p)) => Some(Arc::new(g.with_picture(Some(p.clone())))),
            (g, _) => g.clone(),
        }
    }
}

/// The rasters of the learned masks, by adjustment id and component.
pub type Rasters = std::collections::HashMap<(u64, usize), Arc<greycard_edit::brush::Raster>>;

/// Why the learned denoiser did not run.
#[derive(Debug, Clone)]
pub enum NoLearned {
    /// Its model is not in the store.
    Missing(&'static greycard_ai::Model),
    /// It could not run.
    Failed(String),
}

/// The picture with its retouch applied, beside the base the learned
/// masks are made on.
#[derive(Clone, Copy)]
pub struct Patched<'a> {
    pub image: &'a Arc<WorkingImage>,
    /// The retouch as it was applied, its sources chosen.
    pub retouch: &'a Retouch,
    /// The Fill patches left as they were, by id: the picture there is
    /// the base's.
    pub unmade: &'a [u64],
}

/// The models a walk runs, lent by whoever holds them.
pub trait Models {
    /// Whether the learned denoiser `model` is to be had, and why not
    /// when it is not: asked before the walk starts, so a list that
    /// cannot run it is the engine's own from its first step, and
    /// resumes from what a walk of the same stand-in kept.
    fn can_denoise(&mut self, model: &'static greycard_ai::Model) -> Result<(), NoLearned>;

    /// The learned denoiser's camera RGB of `prepared`.
    fn denoise(
        &mut self,
        model: &'static greycard_ai::Model,
        frame: &RawFrame,
        prepared: &Prepared,
    ) -> Result<Vec<f32>, NoLearned>;

    /// A fill's pixels for `patch` over `region` of `picture`; none
    /// leaves the patch as it was.
    fn fill(&mut self, patch: &Patch, region: &Region, picture: &WorkingImage) -> Option<Vec<f32>>;

    /// Whether a fill left unmade in `retouch` could be made now.
    fn could_fill_more(&mut self, retouch: &Retouch, picture: &WorkingImage) -> bool;

    /// The learned masks' rasters, made on `base` (and on `patched`
    /// where a patch meets a shape) as `how` says.
    fn rasters(
        &mut self,
        how: Masks,
        base: &Arc<WorkingImage>,
        patched: Option<Patched<'_>>,
        meta: &Meta,
    ) -> Rasters;
}

/// No models at all: the command line's, and a test's.
pub struct NoModels;

impl Models for NoModels {
    fn can_denoise(&mut self, model: &'static greycard_ai::Model) -> Result<(), NoLearned> {
        Err(NoLearned::Missing(model))
    }

    fn denoise(
        &mut self,
        model: &'static greycard_ai::Model,
        _: &RawFrame,
        _: &Prepared,
    ) -> Result<Vec<f32>, NoLearned> {
        Err(NoLearned::Missing(model))
    }

    fn fill(&mut self, _: &Patch, _: &Region, _: &WorkingImage) -> Option<Vec<f32>> {
        None
    }

    fn could_fill_more(&mut self, _: &Retouch, _: &WorkingImage) -> bool {
        false
    }

    fn rasters(
        &mut self,
        _: Masks,
        _: &Arc<WorkingImage>,
        _: Option<Patched<'_>>,
        _: &Meta,
    ) -> Rasters {
        Rasters::new()
    }
}

/// The background work a develop holds, and the give-way the prefetch
/// asks between its steps.
pub trait Hold {
    /// Hold the background work while a walk runs, or let it go.
    fn hold(&self, _on: bool) {}

    /// Asked before each stage: whether to stop the walk there.
    fn give_way(&self, _next: &Stage) -> bool {
        false
    }

    /// Whether a device error short of a lost device leaves the
    /// session its context, this walk going on on the CPU: a walk
    /// nobody waits on (a prefetch) is not worth the session's device
    /// over a failure that may be its own, such as running out of
    /// memory on a large neighbor.
    fn keeps_device(&self) -> bool {
        false
    }
}

/// Whether `e`, met by a walk under `hold`, leaves the session its
/// context.
fn keeps_device(hold: &dyn Hold, e: &greycard_gpu::Error) -> bool {
    hold.keeps_device() && !matches!(e, greycard_gpu::Error::Lost(_))
}

/// Nothing to hold and never a give-way: headless export's and the
/// command line's.
pub struct NoHold;

impl Hold for NoHold {}

/// What a kept point holds of the picture.
#[derive(Clone)]
enum KeptBuf {
    Pair {
        model: Arc<WorkingImage>,
        plain: Arc<WorkingImage>,
    },
    Working(Arc<WorkingImage>),
    Device(Arc<greycard_gpu::Image>),
}

impl KeptBuf {
    fn buf(&self) -> Buf {
        match self {
            KeptBuf::Pair { model, plain } => Buf::Pair {
                model: model.clone(),
                plain: plain.clone(),
            },
            KeptBuf::Working(image) => Buf::Working(image.clone()),
            KeptBuf::Device(image) => Buf::Device(image.clone()),
        }
    }
}

/// A retouch as a walk applied it, and the Fill patches it left as
/// they were, by id: a walk that starts past it never asks for its
/// fills, and the masks must still know where the picture is the
/// base's.
#[derive(Clone)]
struct Applied {
    retouch: Retouch,
    unmade: Vec<u64>,
}

/// What the stages up to a kept point measured, and the sources the
/// retouch chose: a walk that starts from the point reports them as
/// its own, since its picture is theirs.
#[derive(Clone, Default)]
struct Ran {
    ca: Option<CaStats>,
    detail: Option<LocalContrastStats>,
    dehaze: Option<DehazeStats>,
    sources: Vec<Patch>,
}

impl Ran {
    fn of(walked: &Walked) -> Self {
        Self {
            ca: walked.ca,
            detail: walked.detail,
            dehaze: walked.dehaze,
            sources: walked.sources.clone(),
        }
    }
}

/// One point a walk kept.
#[derive(Clone)]
struct Point {
    keep: Keep,
    steps: Vec<Step>,
    places: Vec<Place>,
    buf: KeptBuf,
    meta: Meta,
    taps: Taps,
    /// The retouch this point's picture carries; none before the
    /// retouch.
    retouch: Option<Applied>,
    ran: Ran,
}

/// What walks keep for the walks after them. A clone shares every
/// picture it holds, and is a walk's own to resume from and keep in
/// without touching the original's.
#[derive(Clone, Default)]
pub struct Kept {
    points: Vec<Point>,
    /// The masks' bands, with the steps to the picture they were made
    /// from and what was asked of them.
    planes: Option<KeptPlanes>,
    /// A picture's upload, with the picture it was made from: the very
    /// one, not one of the same steps, since a picture made again with
    /// a step in another place (the CA on the CPU for an export) is not
    /// the same to the bit.
    uploaded: Option<(Arc<WorkingImage>, Arc<greycard_gpu::Image>)>,
    /// The dehaze's reduced copy on the device's picture, with the
    /// steps to the picture it was read off.
    haze: Option<(Vec<Step>, Arc<dehaze::Reduced>)>,
    /// The refined masks' picture, with the steps to the picture it
    /// was tapped from.
    mask_picture: Option<(Vec<Step>, Arc<greycard_render::refine::MaskPicture>)>,
}

#[derive(Clone)]
struct KeptPlanes {
    steps: Vec<Step>,
    wants: DetailWants,
    haze_after: Option<greycard_core::develop::local_contrast::LocalContrastOptions>,
    planes: Arc<DetailPlanes>,
}

impl KeptPlanes {
    /// Whether these are the bands of the picture `to` makes, with
    /// every band `wants` asks for and a haze read after `haze_after`.
    fn serves(
        &self,
        to: &[Step],
        wants: &DetailWants,
        haze_after: Option<greycard_core::develop::local_contrast::LocalContrastOptions>,
    ) -> bool {
        same_picture(&self.steps, to)
            && self.wants.covers(wants)
            && (!wants.haze || self.haze_after == haze_after)
    }
}

/// Whether a point that ran at `kept` may serve a plan of `plan`: a
/// step the device ran serves only a plan that puts it there.
fn allowed(kept: &[Place], plan: &[Place]) -> bool {
    kept.iter()
        .zip(plan)
        .all(|(k, p)| *k == Place::Cpu || *k == *p)
}

/// [`allowed`], for a point whose steps `map` puts at those indices of
/// the walk's ([`align`]).
fn allowed_at(kept: &[Place], map: &[Option<usize>], plan: &[Place]) -> bool {
    kept.iter()
        .zip(map)
        .all(|(k, j)| *k == Place::Cpu || j.is_none_or(|j| plan.get(j) == Some(k)))
}

/// A step as the picture sees it: none for a keep, which changes
/// nothing, and the learned masks' tap whatever its purpose makes of
/// it, since the picture it reads is the same. The lists of two
/// purposes are one list so far as these agree.
fn keyed(step: &Step) -> Option<Step> {
    match step {
        Step::Keep(_) => None,
        Step::Tap(Tap::Masks(_)) => Some(Step::Tap(Tap::Masks(Masks::OnAsk))),
        step => Some(step.clone()),
    }
}

/// Whether two lists of steps make the same picture ([`keyed`]).
fn same_picture(a: &[Step], b: &[Step]) -> bool {
    a.iter().filter_map(keyed).eq(b.iter().filter_map(keyed))
}

/// Where a walk of `steps` goes on from a point kept after `kept`, when
/// those make the same picture as the start of `steps` ([`keyed`]):
/// the index in `steps` past the last of them, and the index in
/// `steps` of each of `kept` (none for a keep).
fn align(kept: &[Step], steps: &[Step]) -> Option<(usize, Vec<Option<usize>>)> {
    let mut j = 0;
    let mut map = Vec::with_capacity(kept.len());
    for step in kept {
        let Some(want) = keyed(step) else {
            map.push(None);
            continue;
        };
        while matches!(steps.get(j), Some(Step::Keep(_))) {
            j += 1;
        }
        if keyed(steps.get(j)?) != Some(want) {
            return None;
        }
        map.push(Some(j));
        j += 1;
    }
    Some((j, map))
}

impl Kept {
    /// Nothing kept.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether anything is kept at `keep`.
    pub fn holds(&self, keep: Keep) -> bool {
        self.points.iter().any(|p| p.keep == keep)
    }

    /// Whether a point kept for `steps` serves `plan` at `keep`.
    pub fn serves(&self, steps: &Steps, plan: &[Place], keep: Keep) -> bool {
        let Some(at) = steps.at(keep) else {
            return false;
        };
        self.points.iter().any(|p| {
            p.keep == keep
                && same_picture(&p.steps, &steps.steps[..=at])
                && align(&p.steps, &steps.steps)
                    .is_some_and(|(_, map)| allowed_at(&p.places, &map, plan))
        })
    }

    /// The point a walk of `steps` under `plan` starts from: the last
    /// one whose steps make the picture the start of its own make
    /// ([`align`]), whatever purpose kept it, and whose places the plan
    /// allows, its patched picture only while no fill left unmade in it
    /// could be made now. The points past it are let go, as they were
    /// another list's.
    fn resume(&mut self, steps: &Steps, plan: &[Place], models: &mut dyn Models) -> Option<usize> {
        let found = self
            .points
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                let (at, map) = align(&p.steps, &steps.steps)?;
                // The walk's next stage, past its own keeps there.
                let next = (at..steps.steps.len())
                    .find(|&k| !matches!(steps.steps[k], Step::Keep(_)))
                    .unwrap_or(steps.steps.len());
                let serves = allowed_at(&p.places, &map, plan)
                    // A picture on the device serves only a walk that
                    // goes on there: the viewport's texture is made by
                    // a run, not from a picture left on the device.
                    && (!matches!(p.buf, KeptBuf::Device(_))
                        || (plan.get(next) == Some(&Place::Gpu)
                            && !matches!(steps.steps.get(next), Some(Step::Op(Stage::Finish(_))))))
                    && match (&p.retouch, &p.buf) {
                        (Some(a), KeptBuf::Working(image)) if !a.unmade.is_empty() => {
                            !models.could_fill_more(&a.retouch, image)
                        }
                        _ => true,
                    };
                serves.then_some((i, (at, p.steps.len())))
            })
            .max_by_key(|(_, key)| *key)?;
        let (best, (at, _)) = found;
        let keep: Vec<bool> = self
            .points
            .iter()
            .enumerate()
            .map(|(i, p)| i == best || align(&p.steps, &steps.steps).is_some_and(|(a, _)| a <= at))
            .collect();
        let index = keep[..best].iter().filter(|k| **k).count();
        let mut flags = keep.into_iter();
        self.points.retain(|_| flags.next().unwrap_or(false));
        self.drop_orphaned_upload();
        Some(index)
    }

    /// The kept base and the learned pair made at another turn, each
    /// turned to this one where nothing after the turn minds which way
    /// up it is: the index of the later of them, now turned. A base
    /// blended at an end of the strength shares the pair's picture, and
    /// still does turned. The masks' tap reads the base, so it reads the
    /// turned one.
    fn turned(&mut self, steps: &Steps, plan: &[Place]) -> Option<usize> {
        let want = turnless(&steps.steps);
        let to = turn_of(&steps.steps[..])?;
        let fits = |p: &Point| {
            matches!(p.keep, Keep::Base | Keep::Pair)
                && !matches!(p.buf, KeptBuf::Device(_))
                && allowed(&p.places, plan)
                && turnless(&p.steps) == want[..turnless(&p.steps).len().min(want.len())]
                && commutes_after_turn(&p.steps)
                && steps.at(p.keep).is_some()
                && turn_of(&p.steps)
                    .and_then(|from| relative(from, to))
                    .is_some()
        };
        if !self.points.iter().any(fits) {
            return None;
        }
        let mut points: Vec<Point> = std::mem::take(&mut self.points)
            .into_iter()
            .filter(fits)
            .collect();
        points.sort_by_key(|p| p.steps.len());
        // Each picture turned once, and shared on where it was shared.
        let mut done: Vec<(*const WorkingImage, Arc<WorkingImage>)> = Vec::new();
        for p in &mut points {
            let from = turn_of(&p.steps).expect("fits");
            let q = relative(from, to).expect("fits");
            let o = quarters(q).expect("a turn that moves");
            let mut turn = |image: &Arc<WorkingImage>| {
                let at = Arc::as_ptr(image);
                if let Some((_, t)) = done.iter().find(|(p, _)| *p == at) {
                    return t.clone();
                }
                let t = Arc::new(core_develop::orient((**image).clone(), o));
                done.push((at, t.clone()));
                t
            };
            p.buf = match &p.buf {
                KeptBuf::Pair { model, plain } => KeptBuf::Pair {
                    model: turn(model),
                    plain: turn(plain),
                },
                KeptBuf::Working(image) => KeptBuf::Working(turn(image)),
                KeptBuf::Device(_) => unreachable!("fits"),
            };
            if let (KeptBuf::Working(image), Some((_, tapped))) = (&p.buf, p.taps.masks.as_mut()) {
                *tapped = image.clone();
            }
            if q % 2 == 1 {
                p.meta.full = (p.meta.full.1, p.meta.full.0);
            }
            let n = steps.at(p.keep).expect("fits");
            p.steps = steps.steps[..=n].to_vec();
            p.places = plan[..=n].to_vec();
        }
        self.planes = None;
        self.uploaded = None;
        self.haze = None;
        self.points = points;
        Some(self.points.len() - 1)
    }

    fn put(&mut self, point: Point) {
        self.points.retain(|p| p.keep != point.keep);
        self.points.push(point);
        self.drop_orphaned_upload();
    }

    /// The upload, and the dehaze's copy read off its run, let go once
    /// no point holds the picture it was made from: that picture was
    /// made again (an export's, with the CA on the CPU), and the upload
    /// would hold the old one for nothing until the next run.
    fn drop_orphaned_upload(&mut self) {
        let held = |from: &Arc<WorkingImage>| {
            self.points
                .iter()
                .any(|p| matches!(&p.buf, KeptBuf::Working(image) if Arc::ptr_eq(image, from)))
        };
        if self.uploaded.as_ref().is_some_and(|(from, _)| !held(from)) {
            self.uploaded = None;
            self.haze = None;
        }
    }

    /// Take the picture kept in memory at `keep` out of what is kept.
    pub fn take_working(&mut self, keep: Keep) -> Option<Arc<WorkingImage>> {
        let i = self
            .points
            .iter()
            .position(|p| p.keep == keep && matches!(p.buf, KeptBuf::Working(_)))?;
        let KeptBuf::Working(image) = self.points.remove(i).buf else {
            unreachable!("found in memory")
        };
        self.drop_orphaned_upload();
        Some(image)
    }

    /// Let go of everything kept on the device: its context is gone.
    pub fn forget_device(&mut self) {
        self.points.retain(|p| !matches!(p.buf, KeptBuf::Device(_)));
        self.uploaded = None;
        self.haze = None;
    }

    /// What a frame left behind keeps: the learned pair, the base, and
    /// the patched picture, in memory. Everything on the device goes,
    /// which it would hold for a frame not on screen, and the masks'
    /// bands, which are made again beside the develop and cost as much
    /// as the base.
    pub fn shelve(&mut self) {
        self.forget_device();
        self.points
            .retain(|p| matches!(p.keep, Keep::Pair | Keep::Base | Keep::Patched));
        self.planes = None;
        self.mask_picture = None;
    }

    /// Nothing kept at all.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// The picture kept at `keep`, when it is one in memory.
    pub fn picture(&self, keep: Keep) -> Option<&Arc<WorkingImage>> {
        self.points
            .iter()
            .find(|p| p.keep == keep)
            .and_then(|p| match &p.buf {
                KeptBuf::Working(image) => Some(image),
                _ => None,
            })
    }

    /// What the develop measured of the picture kept at `keep`.
    pub fn meta(&self, keep: Keep) -> Option<&Meta> {
        self.points.iter().find(|p| p.keep == keep).map(|p| &p.meta)
    }

    /// The patched picture, when a retouch made it, with the retouch as
    /// it was applied and the Fill patches it left as they were.
    pub fn patched(&self) -> Option<Patched<'_>> {
        let p = self.points.iter().find(|p| p.keep == Keep::Patched)?;
        match (&p.buf, &p.retouch) {
            (KeptBuf::Working(image), Some(applied)) => Some(Patched {
                image,
                retouch: &applied.retouch,
                unmade: &applied.unmade,
            }),
            _ => None,
        }
    }

    /// The masks' bands kept, when they hold every band `wants` asks
    /// for, with the haze read after `haze_after` when it asks for one.
    pub fn planes_for(
        &self,
        wants: &DetailWants,
        haze_after: Option<greycard_core::develop::local_contrast::LocalContrastOptions>,
    ) -> Option<Arc<DetailPlanes>> {
        self.planes
            .as_ref()
            .filter(|k| k.wants.covers(wants) && (!wants.haze || k.haze_after == haze_after))
            .map(|k| k.planes.clone())
    }

    /// Whether the base kept here and the one `other` keeps are the
    /// same picture: made by the same steps.
    pub fn same_base(&self, other: &Kept) -> bool {
        let steps = |k: &Kept| {
            k.points
                .iter()
                .find(|p| p.keep == Keep::Base)
                .map(|p| p.steps.clone())
        };
        match (steps(self), steps(other)) {
            (Some(a), Some(b)) => same_picture(&a, &b),
            _ => false,
        }
    }

    /// Let go of the refined masks' picture: the edit refines no mask.
    pub fn forget_mask_picture(&mut self) {
        self.mask_picture = None;
    }

    /// The refined masks' picture a walk on the CPU tapped.
    pub fn mask_picture(&self) -> Option<&Arc<greycard_render::refine::MaskPicture>> {
        self.mask_picture.as_ref().map(|(_, picture)| picture)
    }

    /// The bytes of the pictures kept in memory, each counted once
    /// however many points share it, and the refined masks' picture.
    pub fn bytes(&self) -> u64 {
        let mut seen: Vec<*const WorkingImage> = Vec::new();
        let mut total = 0u64;
        let mut count = |image: &Arc<WorkingImage>| {
            let at = Arc::as_ptr(image);
            if !seen.contains(&at) {
                seen.push(at);
                total += (image.data.len() * std::mem::size_of::<f32>()) as u64;
            }
        };
        for p in &self.points {
            match &p.buf {
                KeptBuf::Pair { model, plain } => {
                    count(model);
                    count(plain);
                }
                KeptBuf::Working(image) => count(image),
                KeptBuf::Device(_) => {}
            }
        }
        total + self.mask_picture().map_or(0, |p| p.bytes())
    }

    /// Whether the point at `keep` is a picture on the device.
    pub fn on_device(&self, keep: Keep) -> bool {
        self.points
            .iter()
            .any(|p| p.keep == keep && matches!(p.buf, KeptBuf::Device(_)))
    }

    /// The steps for which `stage` holds marked as run on the device,
    /// as a run there would have left them: for the tests of what a
    /// device's points serve.
    #[cfg(test)]
    pub(crate) fn as_if_on_device(&mut self, stage: impl Fn(&Stage) -> bool) {
        for p in &mut self.points {
            for (s, place) in p.steps.iter().zip(p.places.iter_mut()) {
                if matches!(s, Step::Op(op) if stage(op)) {
                    *place = Place::Gpu;
                }
            }
        }
    }

    /// The patched picture's upload, for the tests of what is kept.
    #[cfg(test)]
    pub(crate) fn upload(&self) -> Option<&Arc<greycard_gpu::Image>> {
        self.uploaded.as_ref().map(|(_, image)| image)
    }

    /// The dehaze's reduced copy, for the tests of what is kept.
    #[cfg(test)]
    pub(crate) fn haze_copy(&self) -> Option<&Arc<dehaze::Reduced>> {
        self.haze.as_ref().map(|(_, reduced)| reduced)
    }

    /// The picture kept on the device at `keep`.
    pub fn device_picture(&self, keep: Keep) -> Option<&Arc<greycard_gpu::Image>> {
        self.points.iter().find_map(|p| match &p.buf {
            KeptBuf::Device(image) if p.keep == keep => Some(image),
            _ => None,
        })
    }

    /// Whether the patched picture's upload is kept on the device.
    pub fn has_upload(&self) -> bool {
        self.uploaded.is_some()
    }

    /// Whether the dehaze's reduced copy is kept.
    pub fn has_haze(&self) -> bool {
        self.haze.is_some()
    }

    /// Whether the step `stage` of the point kept at `keep` ran on the
    /// device.
    pub fn ran_on_device(&self, keep: Keep, stage: impl Fn(&Stage) -> bool) -> bool {
        self.points.iter().any(|p| {
            p.keep == keep
                && p.steps.iter().zip(&p.places).any(|(s, place)| {
                    matches!(s, Step::Op(op) if stage(op)) && *place == Place::Gpu
                })
        })
    }
}

/// The steps with every turn left out: what two lists that differ only
/// in the frame's turn share.
fn turnless(steps: &[Step]) -> Vec<Step> {
    steps
        .iter()
        .filter(|s| !matches!(s, Step::Op(Stage::Orient(_))))
        .map(|s| match s {
            Step::Op(Stage::IntoWorking { profile, .. }) => Step::Op(Stage::IntoWorking {
                profile: profile.clone(),
                orientation: Orientation::Normal,
            }),
            s => s.clone(),
        })
        .collect()
}

/// Whether everything after the turn in `steps` gives the same picture
/// turned, to the bit, and no tap made data on the unturned picture.
fn commutes_after_turn(steps: &[Step]) -> bool {
    steps.iter().all(|s| match s {
        Step::Op(stage) => stage.commutes_with_turn(),
        Step::Tap(Tap::Masks(m)) => *m == Masks::OnAsk,
        Step::Tap(_) => false,
        Step::Keep(_) => true,
    })
}

/// The orientation a list shows its frame in: a raw's move into the
/// working space, or a picture's turn (none for none).
fn turn_of(steps: &[Step]) -> Option<Option<Orientation>> {
    for s in steps {
        match s {
            Step::Op(Stage::IntoWorking { orientation, .. }) => return Some(Some(*orientation)),
            Step::Op(Stage::Orient(o)) => return Some(Some(*o)),
            _ => {}
        }
    }
    // A picture at no turn has no step for it.
    Some(None)
}

/// The quarter turns that take a picture shown at `from` to `to`.
fn relative(from: Option<Orientation>, to: Option<Orientation>) -> Option<u8> {
    let from = from.unwrap_or(Orientation::Normal);
    let to = to.unwrap_or(Orientation::Normal);
    (1..4u8).find(|q| from.turned(i32::from(*q)) == to)
}

/// How a walk did not end at its last step.
#[derive(Debug)]
pub enum Stopped {
    /// It gave way before this stage.
    GaveWay(&'static str),
    /// A stage failed.
    Failed(String),
}

impl std::fmt::Display for Stopped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Stopped::GaveWay(stage) => write!(f, "gave way before the {stage}"),
            Stopped::Failed(why) => f.write_str(why),
        }
    }
}

/// What a walk made.
pub struct Walked {
    /// The picture where the walk stopped: before the finish, or the
    /// viewport's texture a run on the device wrote.
    pub buf: Buf,
    pub meta: Meta,
    pub taps: Taps,
    /// The learned masks' rasters, made at the end from the tap.
    pub rasters: Rasters,
    /// The sharpen's blend, of the CPU's sharpen, for the viewport to
    /// paint where it acted.
    pub sharpen_mask: Option<Vec<f32>>,
    pub ca: Option<CaStats>,
    pub detail: Option<LocalContrastStats>,
    pub dehaze: Option<DehazeStats>,
    pub sharpen: Option<SharpenStats>,
    /// The patches with the sources the retouch chose for them.
    pub sources: Vec<Patch>,
    /// What was left of the edit: a fill refused, by name.
    pub without: Vec<String>,
    /// The kept point the walk started from; none for the decode.
    pub from: Option<Keep>,
    /// The step the walk started at: the ones before it are the kept
    /// point's, and their stats are what it measured then.
    pub begin: usize,
    /// Where each step ran, the kept ones as they ran then.
    pub places: Vec<Place>,
    /// The seconds each stage took, in order, for the log.
    pub seconds: Vec<(&'static str, f64)>,
}

/// The state a walk carries between its steps.
struct Carried {
    buf: Buf,
    meta: Meta,
    taps: Taps,
    retouch: Option<Applied>,
    /// What the stages before the walk's start measured.
    ran: Ran,
    /// The prepare's settings, which the demosaic, the denoise, and
    /// the move into the working space read.
    settings: DevelopSettings,
}

/// An attempt's walk, and the planes still being made beside it.
struct Out {
    walked: Walked,
    planes: Option<PlanesJob>,
}

/// Run `steps` under `plan` on `input`, from what `kept` holds,
/// keeping at each keep; the models are `models`'s, the device `gpu`'s,
/// asked of `hold` between steps whether to give way. A device op the
/// picture is too much for (`Unsupported`) runs its run again on the
/// CPU, the context kept; any other device error lets the context go
/// and runs the rest on the CPU. A learned denoiser that is not to be
/// had stands in the engine's own path.
pub fn walk(
    steps: &Steps,
    plan: &[Place],
    input: &Input,
    kept: &mut Kept,
    models: &mut dyn Models,
    gpu: &mut Option<greycard_gpu::Context>,
    hold: &dyn Hold,
) -> Result<Walked, Stopped> {
    hold.hold(true);
    let out = walk_held(steps, plan, input, kept, models, gpu, hold);
    hold.hold(false);
    out
}

fn walk_held(
    steps: &Steps,
    plan: &[Place],
    input: &Input,
    kept: &mut Kept,
    models: &mut dyn Models,
    gpu: &mut Option<greycard_gpu::Context>,
    hold: &dyn Hold,
) -> Result<Walked, Stopped> {
    let mut steps = steps.clone();
    let mut plan = plan.to_vec();
    if gpu.is_none() {
        plan.iter_mut().for_each(|p| *p = Place::Cpu);
    }
    let mut stood_in = None;
    let learned = steps.ops().find_map(|s| match s {
        Stage::Learned(model) => Some(*model),
        _ => None,
    });
    if let Some(model) = learned
        && let Err(why) = models.can_denoise(model)
    {
        log::info!("the learned denoiser: {why:?}; the engine's own path stands in");
        steps = steps.without_learned();
        plan = replan(&steps, plan.contains(&Place::Gpu));
        stood_in = Some(why);
    }
    loop {
        match attempt(&steps, &mut plan, input, kept, models, gpu, hold, &stood_in) {
            Ok(mut out) => {
                let mut walked = out.walked;
                if let Some(job) = out.planes.take() {
                    walked.taps.planes = job.land(kept);
                }
                if let Some((how, base)) = walked.taps.masks.clone() {
                    // The patched picture, only where a retouch made it.
                    let point = kept.points.iter().find(|p| p.keep == Keep::Patched);
                    debug_assert!(
                        !point.is_some_and(|p| matches!(p.buf, KeptBuf::Device(_))),
                        "the patched picture is kept in memory, before any device run"
                    );
                    let patched = point.and_then(|p| match (&p.buf, &p.retouch) {
                        (KeptBuf::Working(image), Some(applied)) => Some(Patched {
                            image,
                            retouch: &applied.retouch,
                            unmade: &applied.unmade,
                        }),
                        _ => None,
                    });
                    walked.rasters = models.rasters(how, &base, patched, &walked.meta);
                }
                return Ok(walked);
            }
            Err(Retry::Learned(why)) => {
                log::info!("the learned denoiser: {why:?}; the engine's own path stands in");
                steps = steps.without_learned();
                plan = replan(&steps, plan.contains(&Place::Gpu));
                stood_in = Some(why);
            }
            Err(Retry::Cpu { from, keep_context }) => {
                if !keep_context && let Some(ctx) = gpu.take() {
                    ctx.release();
                    kept.forget_device();
                }
                let rest = if keep_context { from.1 } else { plan.len() };
                plan[from.0..rest].iter_mut().for_each(|p| *p = Place::Cpu);
            }
            Err(Retry::Stop(stopped)) => return Err(stopped),
        }
    }
}

/// `steps`' places after the learned denoiser was taken out of them:
/// on the device where it was `lent` and a stage has a version there.
fn replan(steps: &Steps, lent: bool) -> Vec<Place> {
    steps
        .steps
        .iter()
        .map(|s| match s {
            Step::Op(Stage::Finish(crate::stages::Finish::Shader)) if lent => Place::Gpu,
            Step::Op(stage) if lent && stage.device() => Place::Gpu,
            _ => Place::Cpu,
        })
        .collect()
}

/// Why an attempt at a walk is to be made again, or not.
enum Retry {
    /// The learned denoiser is not to be had.
    Learned(NoLearned),
    /// A run on the device declined or failed: the steps `from` (a
    /// range) go to the CPU, and the context stays when the decline
    /// was the picture's.
    Cpu {
        from: (usize, usize),
        keep_context: bool,
    },
    Stop(Stopped),
}

fn failed(e: impl std::fmt::Display) -> Retry {
    Retry::Stop(Stopped::Failed(e.to_string()))
}

/// The start of a walk: a kept point, a kept base turned, or the
/// decode.
fn start(
    steps: &Steps,
    plan: &[Place],
    input: &Input,
    kept: &mut Kept,
    models: &mut dyn Models,
) -> Result<(usize, Carried, Option<Keep>, Vec<Place>), Retry> {
    let point = kept
        .resume(steps, plan, models)
        .or_else(|| kept.turned(steps, plan));
    if let Some(i) = point {
        let p = &kept.points[i];
        let (at, map) = align(&p.steps, &steps.steps).expect("a point resumed is the walk's");
        let mut places = vec![Place::Cpu; at];
        for (place, j) in p.places.iter().zip(&map) {
            if let Some(j) = j {
                places[*j] = *place;
            }
        }
        // The masks are made as this walk's purpose makes them.
        let mut taps = p.taps.clone();
        let how = steps.steps[..at].iter().find_map(|s| match s {
            Step::Tap(Tap::Masks(how)) => Some(*how),
            _ => None,
        });
        if let (Some(how), Some((made, _))) = (how, taps.masks.as_mut()) {
            *made = how;
        }
        // The masks' bands are made beside the run on the device and land
        // after its keeps are put: a point kept there holds none, and the
        // walk resumed from it does not pass their tap again.
        if taps.planes.is_none() {
            taps.planes = steps.steps[..at]
                .iter()
                .enumerate()
                .find_map(|(t, s)| match s {
                    Step::Tap(Tap::Planes {
                        wants, haze_after, ..
                    }) => kept
                        .planes
                        .as_ref()
                        .filter(|k| k.serves(&steps.steps[..t], wants, *haze_after))
                        .map(|k| k.planes.clone()),
                    _ => None,
                });
        }
        let settings = steps
            .ops()
            .find_map(|s| match s {
                Stage::Prepare(p) => Some(p.settings.clone()),
                _ => None,
            })
            .unwrap_or_default();
        return Ok((
            at,
            Carried {
                buf: p.buf.buf(),
                meta: p.meta.clone(),
                taps,
                retouch: p.retouch.clone(),
                ran: p.ran.clone(),
                settings,
            },
            Some(p.keep),
            places,
        ));
    }
    kept.points.clear();
    let (buf, meta) = match input {
        Input::Raw(frame) => (
            // Made by the prepare, which reads the frame itself.
            Buf::Working(Arc::new(WorkingImage::new(0, 0))),
            Meta {
                white: None,
                clip_level: f32::INFINITY,
                radius: None,
                source: Source::of_frame(frame),
                profile: None,
                full: (0, 0),
                bin: 1,
                gave_way: None,
                stood_in: None,
            },
        ),
        Input::Picture { image, meta } => (
            Buf::Working(image.clone()),
            Meta {
                white: None,
                clip_level: meta.clip_level(),
                radius: None,
                source: Source::Display,
                profile: None,
                full: (image.width as u32, image.height as u32),
                bin: 1,
                gave_way: None,
                stood_in: None,
            },
        ),
    };
    Ok((
        0,
        Carried {
            buf,
            meta,
            taps: Taps::default(),
            retouch: None,
            ran: Ran::default(),
            settings: DevelopSettings::default(),
        },
        None,
        Vec::new(),
    ))
}

#[allow(clippy::too_many_arguments)]
fn attempt(
    steps: &Steps,
    plan: &mut [Place],
    input: &Input,
    kept: &mut Kept,
    models: &mut dyn Models,
    gpu: &mut Option<greycard_gpu::Context>,
    hold: &dyn Hold,
    stood_in: &Option<NoLearned>,
) -> Result<Out, Retry> {
    let (begin, mut st, from, mut places) = start(steps, plan, input, kept, models)?;
    // Said afresh on every walk: the stand-in's list is the one an
    // edit without the learned denoiser makes, so a point kept by the
    // one serves the other, and must not say a stand-in it never had.
    st.meta.stood_in = stood_in.clone();
    let ran = std::mem::take(&mut st.ran);
    let mut walked = Walked {
        buf: Buf::Working(Arc::new(WorkingImage::new(0, 0))),
        meta: st.meta.clone(),
        taps: Taps::default(),
        rasters: Rasters::new(),
        sharpen_mask: None,
        ca: ran.ca,
        detail: ran.detail,
        dehaze: ran.dehaze,
        sharpen: None,
        sources: ran.sources,
        without: Vec::new(),
        from,
        begin,
        places: Vec::new(),
        seconds: Vec::new(),
    };
    let mut planes = None;
    let mut i = begin;
    while i < steps.steps.len() {
        let step = &steps.steps[i];
        if let Step::Op(stage) = step
            && !matches!(stage, Stage::Finish(_))
            && hold.give_way(stage)
        {
            return Err(Retry::Stop(Stopped::GaveWay(stage.name())));
        }
        match step {
            Step::Keep(keep) => {
                places.push(Place::Cpu);
                let buf = match &st.buf {
                    Buf::Pair { model, plain } => KeptBuf::Pair {
                        model: model.clone(),
                        plain: plain.clone(),
                    },
                    Buf::Working(image) => KeptBuf::Working(image.clone()),
                    Buf::Device(image) => KeptBuf::Device(image.clone()),
                    _ => unreachable!("a keep follows a picture"),
                };
                kept.put(Point {
                    keep: *keep,
                    steps: steps.steps[..=i].to_vec(),
                    places: places.clone(),
                    buf,
                    meta: st.meta.clone(),
                    taps: st.taps.clone(),
                    retouch: st.retouch.clone(),
                    ran: Ran::of(&walked),
                });
                i += 1;
            }
            Step::Tap(tap) => {
                places.push(Place::Cpu);
                let Buf::Working(image) = &st.buf else {
                    unreachable!("a tap reads a picture in memory")
                };
                match tap {
                    Tap::Guide => {
                        let started = Instant::now();
                        st.taps.guide = Some(Arc::new(guide_plane(image)));
                        walked
                            .seconds
                            .push(("guide", started.elapsed().as_secs_f64()));
                    }
                    Tap::Masks(how) => st.taps.masks = Some((*how, image.clone())),
                    Tap::MaskPicture => {
                        let to = &steps.steps[..i];
                        match &kept.mask_picture {
                            Some((s, picture)) if same_picture(s, to) => {
                                st.taps.picture = Some(picture.clone());
                            }
                            _ => {
                                let started = Instant::now();
                                let picture =
                                    Arc::new(greycard_render::refine::mask_picture(image));
                                kept.mask_picture = Some((to.to_vec(), picture.clone()));
                                st.taps.picture = Some(picture);
                                walked
                                    .seconds
                                    .push(("mask picture", started.elapsed().as_secs_f64()));
                            }
                        }
                    }
                    Tap::Planes {
                        wants,
                        haze_after,
                        beside,
                    } => {
                        let to = steps.steps[..i].to_vec();
                        let kept_planes =
                            kept.planes.as_ref().filter(|k| same_picture(&k.steps, &to));
                        match kept_planes {
                            Some(k) if k.serves(&to, wants, *haze_after) => {
                                st.taps.planes = Some(k.planes.clone());
                            }
                            k => {
                                let reuse = k.map(|k| k.planes.clone());
                                kept.planes = None;
                                if *beside {
                                    planes = PlanesJob::start(
                                        image.clone(),
                                        to,
                                        *haze_after,
                                        st.meta.clip_level,
                                        *wants,
                                        reuse,
                                    );
                                } else {
                                    st.taps.planes = DetailPlanes::make(
                                        image,
                                        *haze_after,
                                        st.meta.clip_level,
                                        *wants,
                                    )
                                    .map(Arc::new);
                                }
                            }
                        }
                    }
                }
                i += 1;
            }
            Step::Op(Stage::Finish(_)) => {
                // The finish is the caller's: the shader's, or the
                // render crate's on what the walk yields.
                places.push(plan[i]);
                i += 1;
            }
            Step::Op(_) if plan[i] == Place::Gpu => {
                let end = run_end(steps, plan, i);
                let ctx = gpu.as_ref().expect("a plan on the device has a context");
                let inline = matches!(step, Step::Op(Stage::Ca(_) | Stage::Lens(_)));
                match device_run(steps, i, end, ctx, &mut st, kept, &mut places, &mut walked) {
                    Ok(()) => i = end,
                    // The CA and the lens read a picture still in hand:
                    // their run again on the CPU is the stage itself.
                    Err(e) if inline => {
                        let Step::Op(stage) = step else {
                            unreachable!()
                        };
                        match e {
                            greycard_gpu::Error::Unsupported(why) => {
                                log::info!("the {} on the CPU: {why}", stage.name());
                            }
                            e if keeps_device(hold, &e) => {
                                log::warn!(
                                    "the GPU's {}: {e}; this frame on the CPU",
                                    stage.name()
                                );
                                ctx.release();
                                plan[i..].iter_mut().for_each(|p| *p = Place::Cpu);
                            }
                            e => {
                                log::warn!(
                                    "the GPU's {}: {e}; the CPU's from now on",
                                    stage.name()
                                );
                                if let Some(ctx) = gpu.take() {
                                    ctx.release();
                                }
                                kept.forget_device();
                                plan[i..].iter_mut().for_each(|p| *p = Place::Cpu);
                            }
                        }
                        places.push(Place::Cpu);
                        let started = Instant::now();
                        cpu_op(stage, input, &mut st, models, &mut walked)?;
                        walked
                            .seconds
                            .push((stage.name(), started.elapsed().as_secs_f64()));
                        i += 1;
                    }
                    Err(greycard_gpu::Error::Unsupported(why)) => {
                        log::info!("the {} on the CPU: {why}", steps_name(steps, i));
                        ctx.release();
                        return Err(Retry::Cpu {
                            from: (i, end),
                            keep_context: true,
                        });
                    }
                    Err(e) if keeps_device(hold, &e) => {
                        log::warn!(
                            "the GPU's {}: {e}; this frame on the CPU",
                            steps_name(steps, i)
                        );
                        ctx.release();
                        return Err(Retry::Cpu {
                            from: (i, plan.len()),
                            keep_context: true,
                        });
                    }
                    Err(e) => {
                        log::warn!(
                            "the GPU's {}: {e}; the CPU's from now on",
                            steps_name(steps, i)
                        );
                        return Err(Retry::Cpu {
                            from: (i, end),
                            keep_context: false,
                        });
                    }
                }
            }
            Step::Op(stage) => {
                places.push(Place::Cpu);
                let started = Instant::now();
                cpu_op(stage, input, &mut st, models, &mut walked)?;
                walked
                    .seconds
                    .push((stage.name(), started.elapsed().as_secs_f64()));
                i += 1;
            }
        }
    }
    walked.buf = st.buf;
    walked.meta = st.meta;
    walked.taps = st.taps;
    walked.places = places;
    Ok(Out { walked, planes })
}

fn steps_name(steps: &Steps, i: usize) -> &'static str {
    match &steps.steps[i] {
        Step::Op(stage) => stage.name(),
        _ => "step",
    }
}

/// The end of the run on the device that begins at `i`: the steps
/// after it the plan puts there, and the keeps between them.
fn run_end(steps: &Steps, plan: &[Place], i: usize) -> usize {
    let mut end = i + 1;
    // The CA and the lens run on pictures in memory, one at a time.
    if matches!(steps.steps[i], Step::Op(Stage::Ca(_) | Stage::Lens(_))) {
        return end;
    }
    while end < steps.steps.len() {
        match &steps.steps[end] {
            Step::Keep(_) => end += 1,
            Step::Op(Stage::Ca(_) | Stage::Lens(_) | Stage::Finish(_)) => break,
            Step::Op(_) if plan[end] == Place::Gpu => end += 1,
            _ => break,
        }
    }
    end
}

/// A stage on the CPU.
fn cpu_op(
    stage: &Stage,
    input: &Input,
    st: &mut Carried,
    models: &mut dyn Models,
    walked: &mut Walked,
) -> Result<(), Retry> {
    let take =
        |buf: &mut Buf| std::mem::replace(buf, Buf::Working(Arc::new(WorkingImage::new(0, 0))));
    match stage {
        Stage::Prepare(options) => {
            let Input::Raw(frame) = input else {
                unreachable!("a prepare is a raw's")
            };
            st.settings = options.settings.clone();
            let prepared = if options.clip {
                prepare_with(frame, &options.settings, options.measure_noise, None)
            } else {
                core_develop::prepare_unclipped(frame, &options.settings, options.measure_noise)
            }
            .map_err(failed)?;
            st.meta.white = Some(prepared.white_balance);
            st.meta.radius = prepared.sharpen_radius;
            st.meta.profile = options
                .settings
                .profile
                .as_ref()
                .map(|p| p.camera.clone())
                .or_else(|| greycard_core::color::profile_from_frame(frame).ok())
                .map(Arc::new);
            st.meta.full = (prepared.width as u32, prepared.height as u32);
            st.meta.gave_way = Some(prepared.demosaic)
                .filter(|ran| {
                    *ran != options.settings.demosaic
                        && options.settings.demosaic != DemosaicMethod::default()
                        && prepared.pattern.is_some()
                })
                .map(|ran| {
                    format!(
                        "the demosaic: {} is not one for this raw's mosaic; {} instead",
                        options.settings.demosaic.name(),
                        ran.name()
                    )
                });
            st.buf = Buf::Mosaic(prepared);
        }
        Stage::Ca(options) => {
            let Buf::Mosaic(prepared) = &mut st.buf else {
                unreachable!("the CA is the mosaic's")
            };
            let pattern = prepared.pattern.as_ref().expect("a CA is a mosaic's");
            let (samples, stats) = ca::correct_ca(
                &prepared.samples,
                prepared.width,
                prepared.height,
                pattern,
                options,
            )
            .map_err(failed)?;
            prepared.samples = samples;
            prepared.chromatic_aberration = Some(stats);
            walked.ca = Some(stats);
        }
        Stage::Demosaic(..) => {
            let Buf::Mosaic(prepared) = take(&mut st.buf) else {
                unreachable!("the demosaic is the mosaic's")
            };
            let settings = DevelopSettings {
                denoise: None,
                ..st.settings.clone()
            };
            let (rgb, _, _) = demosaic_prepared(&prepared, &settings).map_err(failed)?;
            st.buf = Buf::Camera { prepared, rgb };
        }
        Stage::Bin(bin) => {
            let Buf::Mosaic(prepared) = take(&mut st.buf) else {
                unreachable!("the bin is the mosaic's")
            };
            let pattern = prepared.pattern.as_ref().expect("a bin is a mosaic's");
            let (rgb, width, height) = core_develop::reduced::bin_mosaic(
                &prepared.samples,
                prepared.width,
                prepared.height,
                pattern,
                *bin,
            );
            st.meta.bin = *bin;
            st.buf = Buf::Camera {
                prepared: Prepared {
                    samples: Vec::new(),
                    width,
                    height,
                    pattern: None,
                    ..prepared
                },
                rgb,
            };
        }
        Stage::Learned(model) => {
            let Input::Raw(frame) = input else {
                unreachable!("the learned denoiser is a raw's")
            };
            let Buf::Mosaic(prepared) = take(&mut st.buf) else {
                unreachable!("the learned denoiser is the mosaic's")
            };
            let network = models
                .denoise(model, frame, &prepared)
                .map_err(Retry::Learned)?;
            // The network is the demosaic: the one the edit chose gave
            // way to nothing.
            st.meta.gave_way = None;
            let (plain, _, _) = demosaic_prepared(&prepared, &st.settings).map_err(failed)?;
            st.buf = Buf::CameraPair {
                prepared,
                network,
                plain,
            };
        }
        Stage::Denoise(options) => {
            let Buf::Camera { prepared, rgb } = &mut st.buf else {
                unreachable!("the denoise is camera RGB's")
            };
            if let Some(model) = prepared.noise_model() {
                denoise::denoise_profiled(rgb, prepared.width, prepared.height, &model, options)
                    .map_err(failed)?;
            }
        }
        Stage::Warp(options) => {
            let at = WarpPlacement::binned(options.area, st.meta.bin);
            match &mut st.buf {
                Buf::Camera { prepared, rgb } => {
                    apply_warps(rgb, prepared.width, prepared.height, at, &options.warps);
                }
                Buf::CameraPair {
                    prepared,
                    network,
                    plain,
                } => {
                    apply_warps(network, prepared.width, prepared.height, at, &options.warps);
                    apply_warps(plain, prepared.width, prepared.height, at, &options.warps);
                }
                _ => unreachable!("the file's warp is camera RGB's"),
            }
        }
        Stage::IntoWorking { orientation, .. } => {
            let swapped = |o: Orientation| {
                matches!(
                    o,
                    Orientation::Transpose
                        | Orientation::Rotate90
                        | Orientation::Transverse
                        | Orientation::Rotate270
                )
            };
            if swapped(*orientation) {
                st.meta.full = (st.meta.full.1, st.meta.full.0);
            }
            match take(&mut st.buf) {
                Buf::Camera { mut prepared, rgb } => {
                    prepared.orientation = *orientation;
                    let d = core_develop::finish(prepared, rgb, None, None, &st.settings)
                        .map_err(failed)?;
                    st.meta.clip_level = d.clip_level;
                    st.buf = Buf::Working(Arc::new(d.image));
                }
                Buf::CameraPair {
                    mut prepared,
                    network,
                    plain,
                } => {
                    prepared.orientation = *orientation;
                    let p = core_develop::finish(prepared.clone(), plain, None, None, &st.settings)
                        .map_err(failed)?;
                    let m = core_develop::finish(prepared, network, None, None, &st.settings)
                        .map_err(failed)?;
                    st.meta.clip_level = m.clip_level;
                    st.buf = Buf::Pair {
                        model: Arc::new(m.image),
                        plain: Arc::new(p.image),
                    };
                }
                _ => unreachable!("the move into the working space is camera RGB's"),
            }
        }
        Stage::Blend(strength) => {
            let Buf::Pair { model, plain } = take(&mut st.buf) else {
                unreachable!("the blend is the pair's")
            };
            st.buf = Buf::Working(blend(&model, &plain, *strength));
        }
        Stage::Orient(o) => {
            let image = working(&mut st.buf);
            st.buf = Buf::Working(Arc::new(core_develop::orient(owned(image), *o)));
            if swaps(*o) {
                st.meta.full = (st.meta.full.1, st.meta.full.0);
            }
        }
        Stage::Reduce(bin) => {
            let image = working(&mut st.buf);
            st.meta.bin = *bin;
            st.buf = Buf::Working(Arc::new(greycard_render::reduce::reduce(&image, *bin)));
        }
        Stage::Lens(corrections) => {
            let mut image = owned(working(&mut st.buf));
            for c in corrections {
                image = greycard_core::develop::lens::correct(&image, c).0;
            }
            st.buf = Buf::Working(Arc::new(image));
        }
        Stage::Defringe(options) => {
            let mut image = owned(working(&mut st.buf));
            greycard_core::develop::defringe::defringe(&mut image, options);
            st.buf = Buf::Working(Arc::new(image));
        }
        Stage::Retouch(retouch, fills) => {
            let image = working(&mut st.buf);
            let chosen = retouch.choose_sources(&image);
            let applied = retouch.with_sources(&chosen);
            walked.sources = applied.chosen_patches(&chosen);
            let mut picture = (*image).clone();
            let mut unmade = Vec::new();
            applied.apply_with(&mut picture, |patch, region, before| {
                let made = match fills {
                    Fills::Run => models.fill(patch, region, before),
                    Fills::Refuse => {
                        walked.without.push(format!("the fill {}", patch.name()));
                        None
                    }
                };
                if made.is_none() {
                    unmade.push(patch.id);
                }
                made
            });
            st.retouch = Some(Applied {
                retouch: applied,
                unmade,
            });
            st.buf = Buf::Working(Arc::new(picture));
        }
        Stage::LocalContrast(options) => {
            let mut image = owned(working(&mut st.buf));
            walked.detail = Some(local_contrast(&mut image, options, st.meta.clip_level));
            st.buf = Buf::Working(Arc::new(image));
        }
        Stage::Dehaze(options) => {
            let mut image = owned(working(&mut st.buf));
            walked.dehaze = Some(dehaze::dehaze(&mut image, options));
            st.buf = Buf::Working(Arc::new(image));
        }
        Stage::Sharpen(options) => {
            let mut image = owned(working(&mut st.buf));
            let (stats, mask) =
                sharpen_with_mask(&mut image, options, st.meta.radius, st.meta.clip_level);
            walked.sharpen = Some(stats);
            walked.sharpen_mask = Some(mask);
            st.buf = Buf::Working(Arc::new(image));
        }
        Stage::Finish(_) => unreachable!("the finish is the caller's"),
    }
    Ok(())
}

/// A run of stages on the device, from `i` to `end`: the CA and the
/// lens one at a time on pictures in memory, the Detail section and the
/// sharpen as one run with one upload at its head and the device's
/// picture between, a run ending at the shader writing its texture.
#[allow(clippy::too_many_arguments)]
fn device_run(
    steps: &Steps,
    i: usize,
    end: usize,
    ctx: &greycard_gpu::Context,
    st: &mut Carried,
    kept: &mut Kept,
    places: &mut Vec<Place>,
    walked: &mut Walked,
) -> greycard_gpu::Result<()> {
    match &steps.steps[i] {
        Step::Op(Stage::Ca(options)) => {
            let Buf::Mosaic(prepared) = &mut st.buf else {
                unreachable!("the CA is the mosaic's")
            };
            let started = Instant::now();
            let pattern = prepared.pattern.clone().expect("a CA is a mosaic's");
            let (samples, stats) = ctx.correct_ca(
                &prepared.samples,
                prepared.width,
                prepared.height,
                &pattern,
                options,
            )?;
            prepared.samples = samples;
            prepared.chromatic_aberration = Some(stats);
            walked.ca = Some(stats);
            walked.seconds.push(("CA", started.elapsed().as_secs_f64()));
            places.push(Place::Gpu);
            return Ok(());
        }
        Step::Op(Stage::Lens(corrections)) => {
            let started = Instant::now();
            let Buf::Working(input) = &st.buf else {
                unreachable!("the lens is the working picture's")
            };
            let mut image: Option<WorkingImage> = None;
            for c in corrections {
                let from = image.as_ref().unwrap_or(input);
                // A correction that moves nothing is a multiply a
                // pixel, quicker on the CPU than the trip to the device.
                image = Some(if c.moves() {
                    ctx.correct_lens(from, c)?.0
                } else {
                    greycard_core::develop::lens::correct(from, c).0
                });
            }
            if let Some(image) = image {
                st.buf = Buf::Working(Arc::new(image));
            }
            walked
                .seconds
                .push(("lens", started.elapsed().as_secs_f64()));
            places.push(Place::Gpu);
            return Ok(());
        }
        _ => {}
    }
    let shader = matches!(
        steps.steps.get(end),
        Some(Step::Op(Stage::Finish(crate::stages::Finish::Shader)))
    );
    // The last op of the run writes the viewport's texture when the
    // shader is next.
    let last = (i..end)
        .rev()
        .find(|&j| matches!(steps.steps[j], Step::Op(_)))
        .expect("a run holds an op");
    let mut image = match &st.buf {
        Buf::Device(image) => image.clone(),
        Buf::Working(picture) => match &kept.uploaded {
            Some((from, image)) if Arc::ptr_eq(from, picture) => image.clone(),
            _ => {
                // The dehaze's copy was read off the old upload's run.
                kept.uploaded = None;
                kept.haze = None;
                let started = Instant::now();
                let image = Arc::new(ctx.upload(picture)?);
                walked
                    .seconds
                    .push(("upload", started.elapsed().as_secs_f64()));
                kept.uploaded = Some((picture.clone(), image.clone()));
                image
            }
        },
        _ => unreachable!("a run on the device begins at a picture"),
    };
    let texture = shader.then(|| ctx.viewport_texture(image.width(), image.height()));
    // Once the texture is written there is no picture to keep.
    let mut written = None;
    for j in i..end {
        match &steps.steps[j] {
            Step::Keep(_) if written.is_some() => places.push(Place::Cpu),
            Step::Keep(keep) => {
                places.push(Place::Cpu);
                kept.put(Point {
                    keep: *keep,
                    steps: steps.steps[..=j].to_vec(),
                    places: places.clone(),
                    buf: KeptBuf::Device(image.clone()),
                    meta: st.meta.clone(),
                    taps: st.taps.clone(),
                    retouch: st.retouch.clone(),
                    ran: Ran::of(walked),
                });
            }
            Step::Op(Stage::LocalContrast(options)) => {
                match texture.as_ref().filter(|_| j == last) {
                    Some(out) => {
                        walked.detail = Some(ctx.local_contrast_to_viewport(
                            &image,
                            options,
                            st.meta.clip_level,
                            out,
                        )?);
                        written = Some(out.clone());
                    }
                    None => {
                        let (out, stats) =
                            ctx.local_contrast(&image, options, st.meta.clip_level)?;
                        walked.detail = Some(stats);
                        image = Arc::new(out);
                    }
                }
                places.push(Place::Gpu);
            }
            Step::Op(Stage::Dehaze(options)) => {
                let to = steps.steps[..j].to_vec();
                let reduced = match &kept.haze {
                    Some((s, reduced)) if same_picture(s, &to) => reduced.clone(),
                    _ => {
                        kept.haze = None;
                        let reduced = Arc::new(ctx.dehaze_reduced(&image)?);
                        kept.haze = Some((to, reduced.clone()));
                        reduced
                    }
                };
                let Some(model) = reduced.model(options) else {
                    return Err(greycard_gpu::Error::Unsupported(format!(
                        "a strength of zero at {}",
                        options.amount
                    )));
                };
                match texture.as_ref().filter(|_| j == last) {
                    Some(out) => {
                        walked.dehaze = Some(ctx.dehaze_to_viewport(&image, &model, out)?);
                        written = Some(out.clone());
                    }
                    None => {
                        let (out, stats) = ctx.dehaze(&image, &model)?;
                        walked.dehaze = Some(stats);
                        image = Arc::new(out);
                    }
                }
                places.push(Place::Gpu);
            }
            Step::Op(Stage::Sharpen(options)) => {
                let out = match &texture {
                    Some(out) => out.clone(),
                    None => ctx.viewport_texture(image.width(), image.height()),
                };
                walked.sharpen =
                    Some(ctx.sharpen(&image, options, st.meta.radius, st.meta.clip_level, &out)?);
                places.push(Place::Gpu);
                written = Some(out);
            }
            Step::Op(stage) => unreachable!("no device version of the {}", stage.name()),
            Step::Tap(_) => unreachable!("a run holds no tap"),
        }
    }
    st.buf = match (written, texture) {
        (Some(out), _) => Buf::Texture(out),
        // Neither the Detail section nor the sharpen wrote it: the
        // upload goes to the viewport as it is.
        (None, Some(_)) => {
            return Err(greycard_gpu::Error::Unsupported(
                "nothing on the device to write the viewport's texture".into(),
            ));
        }
        (None, None) => Buf::Device(image),
    };
    Ok(())
}

/// The working picture out of `buf`.
fn working(buf: &mut Buf) -> Arc<WorkingImage> {
    match std::mem::replace(buf, Buf::Working(Arc::new(WorkingImage::new(0, 0)))) {
        Buf::Working(image) => image,
        _ => unreachable!("a stage on the working picture"),
    }
}

/// The picture to change in place: its own when nothing else holds
/// it, a copy otherwise.
fn owned(image: Arc<WorkingImage>) -> WorkingImage {
    Arc::try_unwrap(image).unwrap_or_else(|a| (*a).clone())
}

fn swaps(o: Orientation) -> bool {
    matches!(
        o,
        Orientation::Transpose
            | Orientation::Rotate90
            | Orientation::Transverse
            | Orientation::Rotate270
    )
}

/// `plain` taken `strength` of the way to `model`, per sample: the two
/// are one picture in one space, so the mix is that much of the
/// denoise. At the ends no copy is made.
pub fn blend(
    model: &Arc<WorkingImage>,
    plain: &Arc<WorkingImage>,
    strength: f32,
) -> Arc<WorkingImage> {
    let s = strength.clamp(0.0, 1.0);
    if s >= 1.0 {
        return model.clone();
    }
    if s <= 0.0 {
        return plain.clone();
    }
    let mut out = (**plain).clone();
    for (o, m) in out.data.iter_mut().zip(&model.data) {
        *o += (m - *o) * s;
    }
    Arc::new(out)
}

/// The masks' bands being made on a thread of their own beside the
/// walk: on the device's path the CPU is otherwise idle.
struct PlanesJob {
    made: std::sync::mpsc::Receiver<Option<DetailPlanes>>,
    cancel: Arc<AtomicBool>,
    steps: Vec<Step>,
    wants: DetailWants,
    haze_after: Option<greycard_core::develop::local_contrast::LocalContrastOptions>,
    started: Instant,
}

impl Drop for PlanesJob {
    /// A walk that ends without taking the planes up stops them.
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl PlanesJob {
    /// A thread and not the pool's, whose panic would end the process;
    /// a panic here drops the sender, and the walk goes on without the
    /// planes.
    fn start(
        patched: Arc<WorkingImage>,
        steps: Vec<Step>,
        haze_after: Option<greycard_core::develop::local_contrast::LocalContrastOptions>,
        clip_level: f32,
        wants: DetailWants,
        reuse: Option<Arc<DetailPlanes>>,
    ) -> Option<Self> {
        let (send, made) = std::sync::mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let spawned = std::thread::Builder::new()
            .name("detail planes".into())
            .spawn(move || {
                let planes = DetailPlanes::make_with(
                    &patched,
                    haze_after,
                    clip_level,
                    wants,
                    reuse.as_deref(),
                    &stop,
                );
                if !stop.load(Ordering::Relaxed) {
                    let _ = send.send(planes);
                }
            });
        match spawned {
            Ok(_) => Some(Self {
                made,
                cancel,
                steps,
                wants,
                haze_after,
                started: Instant::now(),
            }),
            Err(e) => {
                log::warn!("the masks' detail bands: no thread to make them on: {e}");
                None
            }
        }
    }

    /// The planes, waited for, and kept for the next walk.
    fn land(self, kept: &mut Kept) -> Option<Arc<DetailPlanes>> {
        let waited = Instant::now();
        let planes = match self.made.recv() {
            Ok(planes) => planes.map(Arc::new),
            Err(_) => {
                log::warn!(
                    "the masks' detail bands: the thread making them died; developing without them"
                );
                None
            }
        };
        log::info!(
            "the masks' detail bands in {:.3} s, {:.3} s of it waited for",
            self.started.elapsed().as_secs_f64(),
            waited.elapsed().as_secs_f64()
        );
        kept.planes = planes.clone().map(|planes| KeptPlanes {
            steps: self.steps.clone(),
            wants: self.wants,
            haze_after: self.haze_after,
            planes,
        });
        planes
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use greycard_core::develop::defringe::defringe;
    use greycard_core::raw::{
        Calibration, CfaColor, CfaPattern, LevelPattern, Levels, Samples, SensorLayout,
    };
    use greycard_edit::Edit;

    use super::*;
    use crate::stages::{self, Purpose};

    /// A small Bayer raw with edges and texture for every stage to
    /// bite on: a ramp, a bright disk, and a fine checker.
    fn frame() -> Arc<RawFrame> {
        frame_of(160, 120, [1.0, 1.0])
    }

    /// A Bayer raw `w` by `h`, its red and blue drawn from the scene at
    /// `scale` times their radius from the center: a lateral chromatic
    /// aberration for the CA to find.
    fn frame_of(w: usize, h: usize, scale: [f32; 2]) -> Arc<RawFrame> {
        let pattern = CfaPattern::rggb();
        let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
        let scene = |x: f32, y: f32| {
            let ramp = 200.0 + 2400.0 * x / w as f32;
            let (dx, dy) = (x - w as f32 * 0.6, y - h as f32 * 0.4);
            let r = (w.min(h) as f32 / 6.0).powi(2);
            let disk = if dx * dx + dy * dy < r { 1200.0 } else { 0.0 };
            let checker = if ((x / 3.0).floor() + (y / 3.0).floor()) as i64 % 2 == 0 {
                150.0
            } else {
                0.0
            };
            ramp + disk + checker
        };
        let mut samples = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let (gain, k) = match pattern.color_at(y, x) {
                    CfaColor::Red => (0.55, scale[0]),
                    CfaColor::Green => (1.0, 1.0),
                    CfaColor::Blue => (0.4, scale[1]),
                    CfaColor::Other(_) => unreachable!(),
                };
                let (fx, fy) = (cx + (x as f32 - cx) * k, cy + (y as f32 - cy) * k);
                samples.push((scene(fx, fy) * gain).min(4000.0) as u16);
            }
        }
        Arc::new(RawFrame {
            make: "Test".into(),
            model: "Cam".into(),
            other_names: Vec::new(),
            width: w,
            height: h,
            channels: 1,
            layout: SensorLayout::Cfa(pattern),
            samples: Samples::U16(samples),
            levels: Levels {
                black: LevelPattern::uniform(64.0, 1),
                white: LevelPattern::uniform(4095.0, 1),
                black_deltas: None,
            },
            as_shot_coefficients: Some([2.0, 1.0, 1.6]),
            calibrations: vec![Calibration {
                illuminant: 21,
                color_matrix: vec![
                    0.9766, -0.2953, -0.1254, -0.4276, 1.2116, 0.2433, -0.0437, 0.1336, 0.5131,
                ],
                forward_matrix: None,
            }],
            crop: None,
            orientation: Orientation::Normal,
            shot: Default::default(),
            dng: Default::default(),
        })
    }

    /// An edit with every CPU stage after the base on.
    fn edit() -> Edit {
        let mut edit = Edit::default();
        edit.noise.profiled = true;
        edit.lens.defringe = true;
        edit.detail.texture = 0.3;
        edit.detail.clarity = 0.4;
        edit.detail.dehaze = 0.25;
        edit
    }

    fn walk_cpu(
        edit: &Edit,
        purpose: &Purpose,
        input: &Input,
        turn: u8,
        kept: &mut Kept,
    ) -> Walked {
        let steps = stages::stages(edit, purpose, &input.facts(turn), None);
        let plan = plan(&steps, purpose, false);
        walk(
            &steps,
            &plan,
            input,
            kept,
            &mut NoModels,
            &mut None,
            &NoHold,
        )
        .expect("walks")
    }

    /// A device of the tests' own, none where there is no adapter;
    /// under CI (GREYCARD_REQUIRE_GPU) a missing one fails the test
    /// rather than pass it.
    fn device() -> Option<Option<greycard_gpu::Context>> {
        match greycard_gpu::Context::own() {
            Ok(ctx) => Some(Some(ctx)),
            Err(e) => {
                assert!(
                    std::env::var_os("GREYCARD_REQUIRE_GPU").is_none_or(|v| v.is_empty()),
                    "no device for the walk ({e}) and GREYCARD_REQUIRE_GPU is set"
                );
                eprintln!("SKIPPED: no device for the walk ({e})");
                None
            }
        }
    }

    fn picture(w: &Walked) -> Arc<WorkingImage> {
        match &w.buf {
            Buf::Working(image) => image.clone(),
            _ => panic!("a CPU walk ends at a picture in memory"),
        }
    }

    /// The walk on the CPU is the stages run by hand in their order:
    /// the engine's develop with its CA inside the prepare and its
    /// denoise inside the demosaic, then the defringe, the local
    /// contrast, the dehaze, and the sharpen, to the bit.
    #[test]
    fn a_walk_on_the_cpu_is_the_stages_run_by_hand() {
        // Past the CA's smallest side, with an aberration for it to
        // correct, so its half of the split is held too.
        let raw = frame_of(640, 480, [1.004, 0.996]);
        let edit = edit();
        let walked = walk_cpu(
            &edit,
            &Purpose::Viewport,
            &Input::Raw(raw.clone()),
            0,
            &mut Kept::new(),
        );

        let settings = DevelopSettings {
            orientation: Some(raw.orientation),
            ..edit.settings(&greycard_edit_io::Stores::user())
        };
        let prepared = prepare_with(&raw, &settings, false, None).unwrap();
        let radius = prepared.sharpen_radius;
        let (rgb, dual, denoised) = demosaic_prepared(&prepared, &settings).unwrap();
        assert!(denoised.is_some(), "the profiled denoise ran");
        let d = core_develop::finish(prepared, rgb, dual, denoised, &settings).unwrap();
        let mut want = d.image;
        defringe(&mut want, &edit.lens.defringe().unwrap());
        local_contrast(&mut want, &edit.detail.options().unwrap(), d.clip_level);
        dehaze::dehaze(&mut want, &edit.detail.dehaze_options().unwrap());
        let (_, mask) = sharpen_with_mask(
            &mut want,
            &edit.sharpen.options().unwrap(),
            radius,
            d.clip_level,
        );

        let got = picture(&walked);
        assert_eq!((got.width, got.height), (want.width, want.height));
        assert!(got.data == want.data, "the walk is the stages by hand");
        assert_eq!(walked.sharpen_mask.as_deref(), Some(&mask[..]));
        assert_eq!(walked.meta.clip_level, d.clip_level);
        assert!(walked.taps.guide.is_some());
        assert!(walked.ca.is_some_and(|c| c.corrected), "the CA corrected");
        assert_eq!(walked.from, None);
        assert!(walked.places.iter().all(|p| *p == Place::Cpu));
    }

    /// A walk after another starts from the last point they share: a
    /// sharpen moved starts at the picture before the sharpen, a
    /// Detail move (Structure's as Clarity's) at the patched picture,
    /// a white balance at the decode; and what each makes is what a
    /// walk from nothing makes.
    #[test]
    fn a_kept_prefix_is_reused_and_a_changed_stage_past_it_is_not() {
        let input = Input::Raw(frame());
        let mut kept = Kept::new();
        let first = walk_cpu(&edit(), &Purpose::Viewport, &input, 0, &mut kept);
        assert_eq!(first.from, None);

        let again = walk_cpu(&edit(), &Purpose::Viewport, &input, 0, &mut kept);
        assert_eq!(again.from, Some(Keep::Developed));
        assert!(Arc::ptr_eq(&picture(&again), &picture(&first)));

        // An edit's change, and the point the walk after it starts at.
        type Case<'a> = (&'a dyn Fn(&mut Edit), Option<Keep>);
        let cases: [Case; 4] = [
            (&|e| e.sharpen.iterations = 5, Some(Keep::PreSharpen)),
            (&|e| e.detail.clarity = -0.2, Some(Keep::Patched)),
            (&|e| e.detail.structure = 0.6, Some(Keep::Patched)),
            (
                &|e| {
                    e.white_balance = greycard_edit::WhiteBalance::Custom {
                        temperature: 4300.0,
                        tint: 0.0,
                    }
                },
                None,
            ),
        ];
        for (change, from) in cases {
            let mut e = edit();
            change(&mut e);
            let walked = walk_cpu(&e, &Purpose::Viewport, &input, 0, &mut kept);
            assert_eq!(walked.from, from);
            let fresh = walk_cpu(&e, &Purpose::Viewport, &input, 0, &mut Kept::new());
            assert!(picture(&walked).data == picture(&fresh).data);
            assert!(picture(&walked).data != picture(&first).data);
        }

        // An export of the same edit takes the developed picture: the
        // list is the viewport's up to the finish, and all of it ran on
        // the CPU.
        let mut kept = Kept::new();
        let viewport = walk_cpu(&edit(), &Purpose::Viewport, &input, 0, &mut kept);
        let export = walk_cpu(
            &edit(),
            &Purpose::Export(Box::default()),
            &input,
            0,
            &mut kept,
        );
        assert_eq!(export.from, Some(Keep::Developed));
        assert!(Arc::ptr_eq(&picture(&export), &picture(&viewport)));
    }

    /// A plan all on the CPU never starts from a point the device
    /// touched, however much of its list it shares; a plan that puts
    /// the same step on the device does.
    #[test]
    fn the_all_cpu_plan_never_matches_a_kept_buffer_the_device_touched() {
        let input = Input::Raw(frame());
        let edit = edit();
        let mut kept = Kept::new();
        let first = walk_cpu(&edit, &Purpose::Viewport, &input, 0, &mut kept);
        let steps = stages::stages(&edit, &Purpose::Viewport, &input.facts(0), None);
        let ca = steps
            .steps
            .iter()
            .position(|s| matches!(s, Step::Op(Stage::Ca(_))))
            .unwrap();
        // As the viewport's CA would have left them, on the device.
        for p in &mut kept.points {
            p.places[ca] = Place::Gpu;
        }
        let cpu = plan(&steps, &Purpose::Viewport, false);
        let device = plan(&steps, &Purpose::Viewport, true);
        assert_eq!(device[ca], Place::Gpu);
        assert!(!kept.serves(&steps, &cpu, Keep::Base));
        assert!(kept.serves(&steps, &device, Keep::Base));
        let export = stages::stages(
            &edit,
            &Purpose::Export(Box::default()),
            &input.facts(0),
            None,
        );
        let export_plan = plan(&export, &Purpose::Export(Box::default()), true);
        assert!(export_plan.iter().all(|p| *p == Place::Cpu));
        assert!(!kept.serves(&export, &export_plan, Keep::Base));
        let walked = walk(
            &export,
            &export_plan,
            &input,
            &mut kept,
            &mut NoModels,
            &mut None,
            &NoHold,
        )
        .unwrap();
        assert_eq!(walked.from, None);
        assert!(picture(&walked).data == picture(&first).data);
    }

    /// A turn of the frame with nothing after it that minds which way
    /// up the picture is permutes the kept base rather than develop it
    /// again, and the picture is the one a develop at that turn makes,
    /// to the bit. With a defringe on, it develops again.
    #[test]
    fn a_turn_permutes_the_kept_base() {
        let input = Input::Raw(frame());
        let mut e = edit();
        e.lens.defringe = false;
        let mut kept = Kept::new();
        walk_cpu(&e, &Purpose::Viewport, &input, 0, &mut kept);
        let turned = walk_cpu(&e, &Purpose::Viewport, &input, 1, &mut kept);
        assert_eq!(turned.from, Some(Keep::Base));
        let fresh = walk_cpu(&e, &Purpose::Viewport, &input, 1, &mut Kept::new());
        let (a, b) = (picture(&turned), picture(&fresh));
        assert_eq!((a.width, a.height), (120, 160));
        assert!(a.data == b.data);
        assert_eq!(turned.meta.full, fresh.meta.full);

        let mut kept = Kept::new();
        walk_cpu(&edit(), &Purpose::Viewport, &input, 0, &mut kept);
        let turned = walk_cpu(&edit(), &Purpose::Viewport, &input, 1, &mut kept);
        assert_eq!(turned.from, None);
    }

    /// A DNG's warp is walked on camera RGB as core's develop runs it,
    /// to the bit, and moves the picture; the reduced walk warps its
    /// binned picture as well.
    #[test]
    fn a_files_warp_is_walked_as_cores_develop_runs_it() {
        let mut frame = (*frame_of(96, 64, [1.0, 1.0])).clone();
        let mut lens = greycard_core::opcodes::WarpPlane::identity();
        (lens.radial[0], lens.radial[2]) = (0.97, 0.06);
        frame.dng.warps = vec![greycard_core::opcodes::Warp {
            planes: vec![lens],
            center_x: 0.48,
            center_y: 0.53,
            reciprocal: false,
        }];
        let warped = Arc::new(frame.clone());
        frame.dng.warps.clear();
        let plain = Arc::new(frame);
        let mut edit = Edit::default();
        edit.sharpen.enabled = false;
        let steps = stages::stages(
            &edit,
            &Purpose::Viewport,
            &Input::Raw(warped.clone()).facts(0),
            None,
        );
        assert_eq!(
            steps.ops().map(Stage::name).collect::<Vec<_>>(),
            [
                "prepare",
                "CA",
                "demosaic",
                "the file's warp",
                "move into the working space",
                "finish"
            ]
        );
        let walked = walk_cpu(
            &edit,
            &Purpose::Viewport,
            &Input::Raw(warped.clone()),
            0,
            &mut Kept::new(),
        );
        let Some(Stage::Prepare(prepare)) = steps.ops().next() else {
            unreachable!("a raw's list starts at its prepare")
        };
        let ca = steps.ops().find_map(|s| match s {
            Stage::Ca(o) => Some(*o),
            _ => None,
        });
        // Core's develop runs the CA inside its prepare.
        let settings = DevelopSettings {
            chromatic_aberration: ca,
            ..prepare.settings.clone()
        };
        let want = core_develop::develop(&warped, &settings).unwrap().image;
        let got = picture(&walked);
        assert!(got.data == want.data, "the walk is core's develop");
        let unwarped = walk_cpu(
            &edit,
            &Purpose::Viewport,
            &Input::Raw(plain),
            0,
            &mut Kept::new(),
        );
        assert!(
            picture(&unwarped).data != got.data,
            "the warp moved nothing"
        );

        let reduced = walk_cpu(
            &edit,
            &Purpose::Reduced,
            &Input::Raw(warped),
            0,
            &mut Kept::new(),
        );
        assert!(picture(&reduced).data.iter().all(|v| v.is_finite()));
    }

    /// The give-way is asked before each stage, and a walk told to
    /// stop says before which.
    #[test]
    fn a_walk_gives_way_between_its_stages() {
        struct Before(&'static str, Cell<usize>);
        impl Hold for Before {
            fn give_way(&self, next: &Stage) -> bool {
                self.1.set(self.1.get() + 1);
                next.name() == self.0
            }
        }
        let input = Input::Raw(frame());
        let steps = stages::stages(&edit(), &Purpose::Prefetch, &input.facts(0), None);
        let plan = plan(&steps, &Purpose::Prefetch, false);
        let hold = Before("demosaic", Cell::new(0));
        let mut kept = Kept::new();
        let stopped = walk(
            &steps,
            &plan,
            &input,
            &mut kept,
            &mut NoModels,
            &mut None,
            &hold,
        );
        assert!(matches!(stopped, Err(Stopped::GaveWay("demosaic"))));
        assert_eq!(
            hold.1.get(),
            3,
            "asked before the prepare, the CA, and the demosaic"
        );
        assert!(!kept.holds(Keep::Base));
        let walked = walk(
            &steps,
            &plan,
            &input,
            &mut kept,
            &mut NoModels,
            &mut None,
            &NoHold,
        )
        .unwrap();
        assert_eq!(walked.from, None);
        assert!(kept.holds(Keep::Base));
    }

    /// A learned tier whose model is not to be had stands the engine's
    /// own path in, and says why.
    #[test]
    fn a_missing_learned_model_stands_the_engine_in() {
        let input = Input::Raw(frame());
        let mut learned = edit();
        learned.noise.learned = greycard_edit::Learned::Fast;
        let walked = walk_cpu(&learned, &Purpose::Viewport, &input, 0, &mut Kept::new());
        assert!(matches!(walked.meta.stood_in, Some(NoLearned::Missing(_))));
        let mut plain = learned.clone();
        plain.noise.learned = greycard_edit::Learned::Off;
        plain.noise.profiled = false;
        let engine = walk_cpu(&plain, &Purpose::Viewport, &input, 0, &mut Kept::new());
        assert!(picture(&walked).data == picture(&engine).data);
    }

    /// The reduced develop is the same walk under its purpose: the
    /// mosaic binned, the defringe's radius over the bin, no sharpen.
    #[test]
    fn the_reduced_walk_bins_and_leaves_the_sharpen_out() {
        let raw = frame();
        let input = Input::Raw(raw.clone());
        // Small enough that no bin is taken; a bin of two by hand.
        let walked = walk_cpu(&edit(), &Purpose::Reduced, &input, 0, &mut Kept::new());
        assert_eq!(walked.meta.bin, 1);
        assert!(walked.sharpen.is_none());
        assert!(walked.sharpen_mask.is_none());

        let mut steps = stages::stages(&edit(), &Purpose::Reduced, &input.facts(0), None);
        for s in &mut steps.steps {
            if let Step::Op(stage @ Stage::Demosaic(..)) = s {
                *stage = Stage::Bin(2);
            }
        }
        let plan = plan(&steps, &Purpose::Reduced, false);
        let binned = walk(
            &steps,
            &plan,
            &input,
            &mut Kept::new(),
            &mut NoModels,
            &mut None,
            &NoHold,
        )
        .unwrap();
        let got = picture(&binned);
        assert_eq!((got.width, got.height), (80, 60));
        assert_eq!(binned.meta.full, (160, 120));
        assert_eq!(binned.meta.bin, 2);
    }

    /// On a device, the viewport's walk runs the CA, the Detail
    /// section, and the sharpen there and ends at the viewport's
    /// texture; a sharpen moved starts from the picture before it, kept
    /// on the device; and an export after it, all on the CPU, takes
    /// none of it and makes the reference's picture. Skipped where no
    /// adapter is, unless GREYCARD_REQUIRE_GPU says one must be.
    #[test]
    fn on_a_device_the_viewport_s_run_ends_at_its_texture() {
        let Some(mut gpu) = device() else {
            return;
        };
        let input = Input::Raw(frame());
        let mut e = edit();
        e.lens.defringe = false;
        let mut kept = Kept::new();
        let go = |e: &Edit,
                  purpose: &Purpose,
                  kept: &mut Kept,
                  gpu: &mut Option<greycard_gpu::Context>| {
            let steps = stages::stages(e, purpose, &input.facts(0), None);
            let plan = plan(&steps, purpose, gpu.is_some());
            walk(&steps, &plan, &input, kept, &mut NoModels, gpu, &NoHold).unwrap()
        };
        let first = go(&e, &Purpose::Viewport, &mut kept, &mut gpu);
        assert!(matches!(first.buf, Buf::Texture(_)));
        assert!(first.places.contains(&Place::Gpu));
        assert!(first.sharpen.is_some() && first.sharpen_mask.is_none());

        e.sharpen.iterations = 5;
        let sharper = go(&e, &Purpose::Viewport, &mut kept, &mut gpu);
        assert_eq!(sharper.from, Some(Keep::PreSharpen));
        assert!(matches!(sharper.buf, Buf::Texture(_)));

        let export = go(&e, &Purpose::Export(Box::default()), &mut kept, &mut gpu);
        assert_eq!(export.from, None);
        assert!(export.places.iter().all(|p| *p == Place::Cpu));
        let reference = go(
            &e,
            &Purpose::Export(Box::default()),
            &mut Kept::new(),
            &mut None,
        );
        assert!(picture(&export).data == picture(&reference).data);
    }

    /// A reduce drops the pixels past its last whole block, which a
    /// turn puts on another edge: an odd-sized picture's kept base is
    /// developed again at a new turn, not permuted, and is the picture
    /// a walk from nothing makes.
    #[test]
    fn a_reduced_picture_of_an_odd_size_is_not_permuted() {
        let mut image = WorkingImage::new(9, 7);
        for (i, v) in image.data.iter_mut().enumerate() {
            *v = (i % 23) as f32 / 23.0;
        }
        let input = Input::Picture {
            image: Arc::new(image),
            meta: Arc::new(Picture {
                image: WorkingImage::new(0, 0),
                make: String::new(),
                model: String::new(),
                shot: Default::default(),
                metadata: Default::default(),
                space: "sRGB".into(),
                bits: 16,
            }),
        };
        let edit = Edit::default();
        let reduced = |turn: u8| {
            let mut steps = stages::stages(&edit, &Purpose::Reduced, &input.facts(turn), None);
            // Too small for the reduced develop's own bin: one by hand.
            let at = steps
                .steps
                .iter()
                .position(|s| matches!(s, Step::Keep(Keep::Base)))
                .unwrap();
            steps.steps.insert(at, Step::Op(Stage::Reduce(2)));
            steps
        };
        let go = |turn: u8, kept: &mut Kept| {
            let steps = reduced(turn);
            let plan = plan(&steps, &Purpose::Reduced, false);
            walk(
                &steps,
                &plan,
                &input,
                kept,
                &mut NoModels,
                &mut None,
                &NoHold,
            )
            .unwrap()
        };
        let mut kept = Kept::new();
        go(0, &mut kept);
        let turned = go(1, &mut kept);
        assert_eq!(turned.from, None);
        let fresh = go(1, &mut Kept::new());
        let (a, b) = (picture(&turned), picture(&fresh));
        assert_eq!((a.width, a.height), (3, 4));
        assert!(a.data == b.data);
    }

    /// A device op that declines the picture (a dehaze whose strength
    /// comes to zero has no model to apply) runs its whole run again on
    /// the CPU, the context kept, and the picture is the CPU's.
    #[test]
    fn a_declined_run_runs_again_on_the_cpu_with_the_context_kept() {
        let Some(mut gpu) = device() else {
            return;
        };
        let input = Input::Raw(frame());
        let mut e = edit();
        e.lens.defringe = false;
        let mut steps = stages::stages(&e, &Purpose::Viewport, &input.facts(0), None);
        for s in &mut steps.steps {
            if let Step::Op(Stage::Dehaze(o)) = s {
                o.amount = 0.0;
            }
        }
        let device_plan = plan(&steps, &Purpose::Viewport, true);
        let walked = walk(
            &steps,
            &device_plan,
            &input,
            &mut Kept::new(),
            &mut NoModels,
            &mut gpu,
            &NoHold,
        )
        .unwrap();
        assert!(gpu.is_some(), "the context is kept");
        let dehaze = steps
            .steps
            .iter()
            .position(|s| matches!(s, Step::Op(Stage::Dehaze(_))))
            .unwrap();
        assert_eq!(walked.places[dehaze], Place::Cpu);
        assert!(
            walked.sharpen_mask.is_some(),
            "the sharpen ran on the CPU too"
        );
        let cpu_plan = plan(&steps, &Purpose::Viewport, false);
        let cpu = walk(
            &steps,
            &cpu_plan,
            &input,
            &mut Kept::new(),
            &mut NoModels,
            &mut None,
            &NoHold,
        )
        .unwrap();
        assert!(picture(&walked).data == picture(&cpu).data);
    }

    /// The sharpen switched off after it ran on the device: the Detail
    /// section's run writes the viewport's texture itself, from the
    /// patched picture, and not from the picture before the sharpen
    /// left on the device.
    #[test]
    fn the_sharpen_switched_off_on_a_device_ends_at_the_detail_s_texture() {
        let Some(mut gpu) = device() else {
            return;
        };
        let input = Input::Raw(frame());
        let mut e = edit();
        e.lens.defringe = false;
        let mut kept = Kept::new();
        let mut go = |e: &Edit, kept: &mut Kept| {
            let steps = stages::stages(e, &Purpose::Viewport, &input.facts(0), None);
            let plan = plan(&steps, &Purpose::Viewport, true);
            walk(
                &steps,
                &plan,
                &input,
                kept,
                &mut NoModels,
                &mut gpu,
                &NoHold,
            )
            .unwrap()
        };
        let on = go(&e, &mut kept);
        assert!(on.sharpen.is_some());
        e.sharpen.enabled = false;
        let off = go(&e, &mut kept);
        assert_eq!(off.from, Some(Keep::Patched));
        assert!(matches!(off.buf, Buf::Texture(_)));
        assert!(off.sharpen.is_none());
        assert!(off.dehaze.is_some());
    }

    /// Models of the tests' own: a network that is the plain demosaic
    /// when it is `there`, no fills, and the unmade Fill patches each
    /// raster was made beside.
    #[derive(Default)]
    struct Fake {
        there: bool,
        unmade: Vec<Option<Vec<u64>>>,
    }

    impl Models for Fake {
        fn can_denoise(&mut self, model: &'static greycard_ai::Model) -> Result<(), NoLearned> {
            if self.there {
                Ok(())
            } else {
                Err(NoLearned::Missing(model))
            }
        }

        fn denoise(
            &mut self,
            _: &'static greycard_ai::Model,
            _: &RawFrame,
            prepared: &Prepared,
        ) -> Result<Vec<f32>, NoLearned> {
            let (rgb, _, _) = demosaic_prepared(prepared, &DevelopSettings::default())
                .map_err(|e| NoLearned::Failed(e.to_string()))?;
            Ok(rgb)
        }

        fn fill(&mut self, _: &Patch, _: &Region, _: &WorkingImage) -> Option<Vec<f32>> {
            None
        }

        fn could_fill_more(&mut self, _: &Retouch, _: &WorkingImage) -> bool {
            false
        }

        fn rasters(
            &mut self,
            _: Masks,
            _: &Arc<WorkingImage>,
            patched: Option<Patched<'_>>,
            _: &Meta,
        ) -> Rasters {
            self.unmade.push(patched.map(|p| p.unmade.to_vec()));
            Rasters::new()
        }
    }

    fn walk_with(
        edit: &Edit,
        purpose: &Purpose,
        input: &Input,
        kept: &mut Kept,
        models: &mut dyn Models,
    ) -> Walked {
        walk_at(edit, purpose, input, 0, kept, models)
    }

    fn walk_at(
        edit: &Edit,
        purpose: &Purpose,
        input: &Input,
        turn: u8,
        kept: &mut Kept,
        models: &mut dyn Models,
    ) -> Walked {
        let steps = stages::stages(edit, purpose, &input.facts(turn), None);
        let plan = plan(&steps, purpose, false);
        walk(&steps, &plan, input, kept, models, &mut None, &NoHold).expect("walks")
    }

    /// `edit` with a learned Subject mask, which puts the masks' tap
    /// in its list.
    fn with_subject(mut edit: Edit) -> Edit {
        edit.adjustments.push(greycard_edit::Adjustment {
            id: 11,
            mask: greycard_edit::mask::Mask {
                components: vec![greycard_edit::mask::Component {
                    shape: greycard_edit::mask::Shape::Subject {},
                    ..Default::default()
                }],
                invert: false,
                refine: 0.0,
            },
            ..Default::default()
        });
        edit
    }

    /// The export's list, its keeps left out as an export leaves them,
    /// starts from the last point the viewport's walk kept on the CPU:
    /// the two lists make the same picture to there, whatever each
    /// purpose makes of the masks. The picture is the one a fresh
    /// export walk makes, and the masks are made as the export makes
    /// them.
    #[test]
    fn an_export_list_resumes_from_the_viewport_s_kept_points_on_the_cpu() {
        let input = Input::Raw(frame());
        let edit = with_subject(edit());
        let mut kept = Kept::new();
        walk_cpu(&edit, &Purpose::Viewport, &input, 0, &mut kept);
        let purpose = Purpose::Export(Box::default());
        let mut export = stages::stages(&edit, &purpose, &input.facts(0), None);
        export.steps.retain(|s| !matches!(s, Step::Keep(_)));
        let export_plan = plan(&export, &purpose, false);
        let resumed = walk(
            &export,
            &export_plan,
            &input,
            &mut kept,
            &mut NoModels,
            &mut None,
            &NoHold,
        )
        .unwrap();
        assert_eq!(resumed.from, Some(Keep::Developed));
        assert!(matches!(resumed.taps.masks, Some((Masks::Make, _))));
        assert_eq!(resumed.places.len(), export.steps.len());
        let fresh = walk(
            &export,
            &export_plan,
            &input,
            &mut Kept::new(),
            &mut NoModels,
            &mut None,
            &NoHold,
        )
        .unwrap();
        assert_eq!(fresh.from, None);
        assert!(picture(&resumed).data == picture(&fresh).data);
        assert!(kept.holds(Keep::Developed), "the viewport's points stay");
    }

    /// A learned denoiser not to be had is known before the walk: the
    /// mosaic is prepared once, and the walk after it, of the same
    /// stand-in, starts from its points and still says why.
    #[test]
    fn a_stand_in_is_known_before_the_walk_and_kept_with_its_points() {
        let input = Input::Raw(frame());
        let mut learned = edit();
        learned.noise.learned = greycard_edit::Learned::Fast;
        let mut kept = Kept::new();
        let first = walk_cpu(&learned, &Purpose::Viewport, &input, 0, &mut kept);
        let prepares = first
            .seconds
            .iter()
            .filter(|(s, _)| *s == "prepare")
            .count();
        assert_eq!(prepares, 1);
        assert!(matches!(first.meta.stood_in, Some(NoLearned::Missing(_))));
        let again = walk_cpu(&learned, &Purpose::Viewport, &input, 0, &mut kept);
        assert_eq!(again.from, Some(Keep::Developed));
        assert!(matches!(again.meta.stood_in, Some(NoLearned::Missing(_))));
    }

    /// An edit without the learned denoiser makes the stand-in's list,
    /// so it starts from the stand-in's points; it says no stand-in.
    #[test]
    fn a_plain_edit_resumed_from_a_stand_in_s_points_says_none() {
        let input = Input::Raw(frame());
        let mut e = edit();
        e.noise.profiled = false;
        e.noise.learned = greycard_edit::Learned::Fast;
        let mut kept = Kept::new();
        let stood = walk_cpu(&e, &Purpose::Viewport, &input, 0, &mut kept);
        assert!(matches!(stood.meta.stood_in, Some(NoLearned::Missing(_))));
        e.noise.learned = greycard_edit::Learned::Off;
        let plain = walk_cpu(&e, &Purpose::Viewport, &input, 0, &mut kept);
        assert_eq!(plain.from, Some(Keep::Developed));
        assert!(plain.meta.stood_in.is_none());
    }

    /// A walk that starts past the retouch never asks for its fills,
    /// and the masks are still told which were left as they were.
    #[test]
    fn a_resumed_patched_picture_carries_its_unmade_fills() {
        use greycard_edit::retouch::Method;
        let input = Input::Raw(frame());
        let mut e = with_subject(edit());
        e.retouch.patches.push(Patch {
            id: 21,
            method: Method::Fill,
            points: vec![[0.5, 0.5]],
            radius: 0.08,
            ..Patch::default()
        });
        let mut kept = Kept::new();
        let mut models = Fake::default();
        walk_with(&e, &Purpose::Viewport, &input, &mut kept, &mut models);
        e.detail.clarity = 0.1;
        let again = walk_with(&e, &Purpose::Viewport, &input, &mut kept, &mut models);
        assert_eq!(again.from, Some(Keep::Patched));
        assert_eq!(models.unmade, [Some(vec![21]), Some(vec![21])]);
    }

    /// The learned denoiser is the demosaic: a method of the other
    /// mosaic chosen in the edit gives way to nothing when it runs,
    /// and to the frame's own when it does not.
    #[test]
    fn the_learned_denoiser_clears_a_demosaic_that_gave_way() {
        let input = Input::Raw(frame());
        let mut e = edit();
        e.demosaic = greycard_edit::Demosaic::Markesteijn3;
        let plain = walk_cpu(&e, &Purpose::Viewport, &input, 0, &mut Kept::new());
        assert!(plain.meta.gave_way.is_some());
        e.noise.learned = greycard_edit::Learned::Fast;
        let mut models = Fake {
            there: true,
            ..Fake::default()
        };
        let learned = walk_with(
            &e,
            &Purpose::Viewport,
            &input,
            &mut Kept::new(),
            &mut models,
        );
        assert!(learned.meta.stood_in.is_none());
        assert_eq!(learned.meta.gave_way, None);
    }

    /// The learned pair turns with the base: at either end of the
    /// strength the base is one of the pair itself and stays shared
    /// with it, turned once; in between it is a blend of its own,
    /// turned. Each is the picture a walk at the new turn makes.
    #[test]
    fn a_turned_base_takes_the_learned_pair_with_it() {
        let input = Input::Raw(frame());
        let mut e = edit();
        e.lens.defringe = false;
        e.noise.learned = greycard_edit::Learned::Fast;
        let pair = |kept: &Kept| match &kept
            .points
            .iter()
            .find(|p| p.keep == Keep::Pair)
            .expect("a pair")
            .buf
        {
            KeptBuf::Pair { model, plain } => (model.clone(), plain.clone()),
            _ => panic!("a pair in memory"),
        };
        for strength in [0.0f32, 0.5, 1.0] {
            e.noise.learned_strength = strength;
            let mut models = Fake {
                there: true,
                ..Fake::default()
            };
            let mut kept = Kept::new();
            walk_at(&e, &Purpose::Viewport, &input, 0, &mut kept, &mut models);
            let turned = walk_at(&e, &Purpose::Viewport, &input, 1, &mut kept, &mut models);
            assert_eq!(turned.from, Some(Keep::Base), "strength {strength}");
            let mut fresh = Kept::new();
            let made = walk_at(&e, &Purpose::Viewport, &input, 1, &mut fresh, &mut models);
            assert!(
                picture(&turned).data == picture(&made).data,
                "strength {strength}"
            );
            let ((model, plain), (fresh_model, fresh_plain)) = (pair(&kept), pair(&fresh));
            assert!(model.data == fresh_model.data && plain.data == fresh_plain.data);
            let base = kept.picture(Keep::Base).expect("a base");
            assert!(base.data == fresh.picture(Keep::Base).unwrap().data);
            match strength {
                1.0 => assert!(Arc::ptr_eq(base, &model), "the model, shared"),
                0.0 => assert!(Arc::ptr_eq(base, &plain), "the plain, shared"),
                _ => assert!(!Arc::ptr_eq(base, &model) && !Arc::ptr_eq(base, &plain)),
            }
        }
    }

    /// A walk resumed from a turned base still makes its masks: the
    /// masks' tap is turned with the base, not left behind.
    #[test]
    fn a_turned_base_still_makes_the_masks() {
        let input = Input::Raw(frame());
        let mut e = with_subject(edit());
        e.lens.defringe = false;
        let mut models = Fake::default();
        let mut kept = Kept::new();
        walk_at(&e, &Purpose::Viewport, &input, 0, &mut kept, &mut models);
        let turned = walk_at(&e, &Purpose::Viewport, &input, 1, &mut kept, &mut models);
        assert_eq!(turned.from, Some(Keep::Base));
        assert_eq!(models.unmade.len(), 2, "the masks made at each turn");
    }

    #[test]
    fn the_blend_is_linear_and_shares_at_the_ends() {
        let model = Arc::new(WorkingImage::from_data(2, 1, vec![1.0; 6]).unwrap());
        let plain = Arc::new(WorkingImage::from_data(2, 1, vec![0.0; 6]).unwrap());
        assert!(Arc::ptr_eq(&blend(&model, &plain, 1.0), &model));
        assert!(Arc::ptr_eq(&blend(&model, &plain, 0.0), &plain));
        assert!(Arc::ptr_eq(&blend(&model, &plain, 2.0), &model));
        let half = blend(&model, &plain, 0.25);
        assert!(half.data.iter().all(|v| (*v - 0.25).abs() < 1e-6));
    }
}

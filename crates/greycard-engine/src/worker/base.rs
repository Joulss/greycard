//! A frame's develop as its walks kept it, and the models those walks
//! run.

use greycard_core::develop::retouch::Region;
use greycard_edit::retouch::Patch;

use super::*;
use crate::stages::{Facts, Keep, Purpose, Step, Steps, Tap, stages};
use crate::walk::{Kept, Meta, NoLearned, Place};

/// The seconds each stage of a develop took, in the order they ran,
/// for its log line ([`super::develop`]'s `note_developed`).
pub(super) type Stages = Vec<(&'static str, f64)>;

/// `run`, its seconds noted in `stages` as `name`.
pub(super) fn timed<T>(stages: &mut Stages, name: &'static str, run: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let out = run();
    stages.push((name, started.elapsed().as_secs_f64()));
    out
}

/// A frame's develop as its walks kept it: the points they kept, and
/// what the window's asks (its masks, its people, its pictures, its
/// export) read of the last one. The open frame's, and each frame's
/// left behind on the shelf.
pub(super) struct FrameKept {
    pub kept: Kept,
    /// What the frame's decode said, which its lists are derived from,
    /// at no turn.
    pub facts: Facts,
    /// The edit the base was made under, which the masks' models see
    /// it through, and the quarter turns of the last walk.
    pub edit: Edit,
    pub turn: u8,
    /// Counts the base pictures made of the frame, for the learned
    /// masks to know theirs.
    pub stamp: u64,
    /// Taken back from the shelf and not walked from since: the next
    /// develop's line says where its base came from.
    pub from_shelf: bool,
    /// The base picture of the last walk.
    pub image: Arc<WorkingImage>,
    /// What the last walk measured of the picture.
    pub meta: Meta,
    /// The last walk's guide plane, and the masks' bands it made.
    pub guide: Arc<greycard_render::finish::Guide>,
    pub planes: Option<Arc<greycard_render::detail::DetailPlanes>>,
    /// Whether the CA or the lens in the base ran on the device: the
    /// export's picture is the reference's.
    pub on_gpu: bool,
    /// The picture the refined masks are made against, with what it
    /// was made from: made only for an edit that refines a mask
    /// ([`super::masks::mask_picture`]), kept for the next develop of
    /// it, and shelved with the base.
    pub mask_picture: Option<KeptMaskPicture>,
    /// What the picture kept on the device before the sharpen was made
    /// from: the patched picture and the Detail section run on it, for
    /// the refined masks' picture to be read back from it.
    pub pre: Option<(Arc<WorkingImage>, greycard_edit::Detail)>,
    /// The edit and the quarter turns of the last develop, and what it
    /// left out of what the edit asked for ([`left_out_of`]): a save of
    /// the frame makes its pictures from that develop when it is of the
    /// edit saved, and says nothing kept when it went without.
    pub developed_as: Option<(Edit, u8)>,
    pub left_out: Vec<String>,
}

/// The refined masks' picture ([`greycard_render::refine::MaskPicture`])
/// with what it was made from: the patched picture and the Detail
/// section run on it before the sharpen.
pub(super) struct KeptMaskPicture {
    pub patched: Arc<WorkingImage>,
    pub detail: greycard_edit::Detail,
    pub picture: Arc<greycard_render::refine::MaskPicture>,
}

impl FrameKept {
    /// Nothing kept yet of a frame `facts` describes.
    pub(super) fn new(facts: &Facts) -> Self {
        Self {
            kept: Kept::new(),
            facts: Facts {
                turn: 0,
                ..facts.clone()
            },
            edit: Edit::default(),
            turn: 0,
            stamp: 0,
            from_shelf: false,
            image: Arc::new(WorkingImage::new(0, 0)),
            meta: Meta::empty(),
            guide: Arc::new(greycard_render::finish::Guide::NONE),
            planes: None,
            on_gpu: false,
            mask_picture: None,
            pre: None,
            developed_as: None,
            left_out: Vec::new(),
        }
    }

    /// The picture the retouch made, or the base where there is none:
    /// what the Detail section and the sharpen run on.
    pub(super) fn patched_picture(&self) -> Arc<WorkingImage> {
        self.kept
            .patched()
            .map_or_else(|| self.image.clone(), |p| p.image.clone())
    }

    /// The bytes of the guide plane and the refined masks' picture,
    /// unless that is the one the walk's own count has.
    fn beside_bytes(&self) -> u64 {
        let picture = self
            .mask_picture
            .as_ref()
            .filter(|k| {
                !self
                    .kept
                    .mask_picture()
                    .is_some_and(|p| Arc::ptr_eq(p, &k.picture))
            })
            .map_or(0, |k| k.picture.bytes());
        (self.guide.data.len() * std::mem::size_of::<f32>()) as u64 + picture
    }

    /// The frame's facts at `turn`.
    pub(super) fn facts_at(&self, turn: u8) -> Facts {
        Facts {
            turn,
            ..self.facts.clone()
        }
    }

    /// The white balance the last walk used, as the window reads it.
    pub(super) fn white(&self) -> WhiteBase {
        match &self.meta.white {
            Some(wb) => WhiteBase::from(wb, self.meta.clip_level),
            None => WhiteBase {
                clip: self.meta.clip_level,
                ..WhiteBase::IDENTITY
            },
        }
    }

    /// What the masks' white balances are made matrices from.
    pub(super) fn white_shift(&self) -> Option<greycard_edit::WhiteShift> {
        self.meta.white_shift()
    }

    /// The masks' bands kept, when they hold what `edit`'s masks read.
    pub(super) fn planes_for(
        &self,
        edit: &Edit,
    ) -> Option<Arc<greycard_render::detail::DetailPlanes>> {
        let wants = edit.detail_wants();
        self.kept
            .planes_for(&wants, edit.detail.options().filter(|_| wants.haze))
    }

    /// The bytes the frame's develop holds in memory: its pictures, the
    /// guide plane, and the refined masks' picture.
    pub(super) fn bytes(&self) -> u64 {
        self.kept.bytes() + self.beside_bytes()
    }

    /// The bytes of the base and the patched picture, the guide plane,
    /// and the refined masks' picture: what a frame of this size puts
    /// on the shelf without a learned pair.
    pub(super) fn base_bytes(&self) -> u64 {
        let base = self.kept.picture(Keep::Base);
        let patched = self.kept.patched().map(|p| p.image);
        let size =
            |image: &Arc<WorkingImage>| (image.data.len() * std::mem::size_of::<f32>()) as u64;
        let mut total = base.map_or(0, size);
        if let Some(p) = patched
            && !base.is_some_and(|b| Arc::ptr_eq(b, p))
        {
            total += size(p);
        }
        total + self.beside_bytes()
    }

    /// Whether the base kept here serves a develop of `edit` at `turn`,
    /// with a device `lent` or not, as the window's walk would resume
    /// from it.
    pub(super) fn serves(
        &self,
        edit: &Edit,
        turn: u8,
        lenses: Option<&greycard_lens::Database>,
        lent: bool,
    ) -> bool {
        let (steps, plan) = window_steps(edit, &self.facts_at(turn), lenses, lent);
        self.kept.serves(&steps, &plan, Keep::Base)
    }
}

/// The window's list for `edit` on the frame `facts` describes, and
/// where each step runs with a device `lent` or not. On the CPU the
/// window keeps its developed picture, which an export of the open
/// frame and a save of its edit start from, and not its picture before
/// the sharpen, one more picture the size of the frame held for a
/// slider the CPU's sharpen is quick at. On the device it keeps the
/// picture before the sharpen, which a sharpen move starts from, and
/// its developed picture is the viewport's texture. On the device the
/// refined masks' picture is read back from the picture kept before
/// the sharpen rather than tapped from one in memory, which a run
/// there does not have.
pub(super) fn window_steps(
    edit: &Edit,
    facts: &Facts,
    lenses: Option<&greycard_lens::Database>,
    lent: bool,
) -> (Steps, Vec<Place>) {
    let purpose = Purpose::Viewport;
    let mut steps = stages(edit, &purpose, facts, lenses);
    steps.steps.retain(|s| match s {
        Step::Keep(Keep::Developed) => !lent,
        Step::Keep(Keep::PreSharpen) => lent,
        Step::Tap(Tap::MaskPicture) => !lent,
        _ => true,
    });
    let plan = crate::walk::plan(&steps, &purpose, lent);
    (steps, plan)
}

/// Why the learned denoiser is not to be had, as the walk says it.
fn no_learned(e: NoDenoiser, model: &'static greycard_ai::Model) -> NoLearned {
    match e {
        NoDenoiser::Missing => NoLearned::Missing(model),
        NoDenoiser::Failed(why) => NoLearned::Failed(why),
    }
}

/// The models a walk of the worker's runs, lent from the `Ai` it is
/// given: the learned denoiser through the disk cache, the fills, and
/// the learned masks an export makes, each one that could not be had
/// noted in the words the window and the export say it in.
pub(super) struct WorkerModels<'a> {
    ai: &'a mut Ai,
    cache: Option<&'a greycard_ai::DenoiseCache>,
    deliver: &'a Deliver,
    /// The job's generation, which its fills are said under.
    generation: u64,
    /// The edit and the turn the walk is of.
    pub edit: Edit,
    pub turn: u8,
    /// How the learned denoiser's answer was had, for the log line.
    pub learned: LearnedReport,
    pub fills: FillReport,
    /// Each learned mask not made, in words.
    pub masks: Vec<String>,
    /// Run the retouch again over fills a kept point left unmade rather
    /// than go on past them: an export says what each fill went
    /// without, which only a run of the fill finds out.
    pub run_unmade: bool,
}

impl<'a> WorkerModels<'a> {
    pub(super) fn new(
        ai: &'a mut Ai,
        cache: Option<&'a greycard_ai::DenoiseCache>,
        deliver: &'a Deliver,
        generation: u64,
    ) -> Self {
        Self {
            ai,
            cache,
            deliver,
            generation,
            edit: Edit::default(),
            turn: 0,
            learned: LearnedReport::Off,
            fills: FillReport::default(),
            masks: Vec::new(),
            run_unmade: false,
        }
    }
}

impl WorkerModels<'_> {
    /// The learned denoiser `model`, loaded now if it is not yet; why
    /// not when it cannot be.
    fn denoiser(
        &mut self,
        model: &'static greycard_ai::Model,
    ) -> Result<&mut greycard_ai::Denoiser, NoLearned> {
        self.ai.denoiser(model).map_err(|e| no_learned(e, model))
    }
}

/// What the models' cache knows an export's picture by: each frame of
/// a set is the first picture made of it.
const EXPORT_STAMP: u64 = 1;

impl crate::walk::Models for WorkerModels<'_> {
    fn can_denoise(&mut self, model: &'static greycard_ai::Model) -> Result<(), NoLearned> {
        self.ai
            .denoiser_there(model)
            .map_err(|e| no_learned(e, model))
    }

    fn denoise(
        &mut self,
        model: &'static greycard_ai::Model,
        frame: &RawFrame,
        prepared: &Prepared,
    ) -> Result<Vec<f32>, NoLearned> {
        let cache = self.cache;
        let denoiser = self.denoiser(model)?;
        let (rgb, report) = network(frame, prepared, model, denoiser, cache)
            .map_err(|e| NoLearned::Failed(e.to_string()))?;
        self.learned = report;
        Ok(rgb)
    }

    fn fill(&mut self, patch: &Patch, region: &Region, picture: &WorkingImage) -> Option<Vec<f32>> {
        // A fill the model has to make is said so before it starts,
        // since it takes a while.
        let kept = self.ai.fill_kept(patch, region, picture);
        if !kept && self.ai.engine(patch, region, picture).is_ok() {
            (self.deliver)(Outcome::Filling {
                generation: self.generation,
                name: patch.name(),
            });
        }
        let started = Instant::now();
        match self.ai.fill(patch, region, picture) {
            Ok((data, engine)) => {
                if !kept {
                    self.fills.made.push(patch.name());
                    self.fills.seconds += started.elapsed().as_secs_f64();
                }
                self.fills.engines.push((patch.id, engine));
                Some(data)
            }
            Err(NoFill::Missing(model)) => {
                self.fills.missing.push((patch.name(), model));
                None
            }
            Err(NoFill::NoGpu) => {
                self.fills.no_gpu.push(patch.name());
                None
            }
            Err(NoFill::Failed(why)) => {
                self.fills.failed.push((patch.name(), why));
                None
            }
        }
    }

    fn could_fill_more(&mut self, retouch: &Retouch, picture: &WorkingImage) -> bool {
        self.run_unmade || self.ai.could_fill_more(retouch, picture)
    }

    fn rasters(
        &mut self,
        how: crate::stages::Masks,
        base: &Arc<WorkingImage>,
        patched: Option<crate::walk::Patched<'_>>,
        meta: &crate::walk::Meta,
    ) -> crate::walk::Rasters {
        // The window asks for its masks on their own job, when it
        // shows them; only an export makes them beside the develop.
        if how != crate::stages::Masks::Make {
            return crate::walk::Rasters::new();
        }
        let (rasters, left_out) = super::masks::learned_rasters(
            &self.edit,
            self.ai,
            &super::masks::On {
                stamp: EXPORT_STAMP,
                image: base,
                retouched: patched.map(|p| crate::ai::Retouched {
                    retouch: p.retouch,
                    image: p.image,
                    unmade: p.unmade,
                }),
                edit: &self.edit,
                source: meta.source,
                turn: self.turn,
            },
        );
        self.masks = left_out;
        rasters
    }
}

/// The learned denoiser's network takes a packed 2x2 Bayer mosaic.
fn bayer_only(frame: &RawFrame) -> anyhow::Result<()> {
    if let greycard_core::raw::SensorLayout::Cfa(p) = &frame.layout
        && !greycard_core::develop::rcd::is_bayer(p)
    {
        anyhow::bail!("the learned denoiser needs a 2x2 Bayer mosaic, not a {p} one");
    }
    Ok(())
}

/// The network's camera RGB of `prepared`: from the disk cache when
/// it is there, else run and put there.
pub(super) fn network(
    frame: &RawFrame,
    prepared: &Prepared,
    model: &'static greycard_ai::Model,
    denoiser: &mut greycard_ai::Denoiser,
    cache: Option<&greycard_ai::DenoiseCache>,
) -> anyhow::Result<(Vec<f32>, LearnedReport)> {
    use anyhow::Context;
    bayer_only(frame)?;
    let pattern = prepared
        .pattern
        .clone()
        .context("the learned denoiser needs a Bayer mosaic")?;
    let noise = prepared
        .noise_model()
        .context("the learned denoiser needs the frame's noise model")?;
    let tiling = denoiser.tiling();
    let start = Instant::now();
    let key = cache.map(|c| (c, greycard_ai::DenoiseCache::key(model, tiling, prepared)));
    let (rgb, report) = match key.and_then(|(c, k)| c.read(k, prepared)) {
        Some(rgb) => (
            rgb,
            LearnedReport::Cached {
                seconds: start.elapsed().as_secs_f64(),
            },
        ),
        None => {
            let rgb = denoiser.run(
                &prepared.samples,
                prepared.width,
                prepared.height,
                &pattern,
                &noise,
                tiling,
            )?;
            let report = LearnedReport::Ran {
                version: denoiser.version().to_string(),
                provider: denoiser.label(),
                seconds: start.elapsed().as_secs_f64(),
            };
            if let Some((c, k)) = key
                && let Err(e) = c.write(k, frame, prepared, &rgb)
            {
                tracing::warn!("denoise cache {}: {e}", c.path(k).display());
            }
            (rgb, report)
        }
    };
    Ok((rgb, report))
}

#[cfg(test)]
impl FrameKept {
    /// What the window's walk of `edit` at `turn` on `input` keeps, on
    /// the CPU, with `models`: for the tests of what holds it.
    pub(super) fn walked_with(
        input: &crate::walk::Input,
        edit: &Edit,
        turn: u8,
        models: &mut dyn crate::walk::Models,
    ) -> FrameKept {
        let facts = input.facts(turn);
        let (steps, plan) = window_steps(edit, &facts, None, false);
        let mut held = FrameKept::new(&facts);
        let walked = crate::walk::walk(
            &steps,
            &plan,
            input,
            &mut held.kept,
            models,
            &mut None,
            &crate::walk::NoHold,
        )
        .expect("the walk");
        held.edit = edit.clone();
        held.turn = turn;
        held.stamp = 1;
        held.meta = walked.meta;
        held.guide = walked
            .taps
            .guide
            .unwrap_or_else(|| Arc::new(greycard_render::finish::Guide::NONE));
        held.planes = walked.taps.planes;
        if let Some(image) = held.kept.picture(Keep::Base) {
            held.image = image.clone();
        }
        held
    }

    /// [`FrameKept::walked_with`] a raw, with no models.
    pub(super) fn walked(frame: &Arc<RawFrame>, edit: &Edit, turn: u8) -> FrameKept {
        Self::walked_with(
            &crate::walk::Input::Raw(frame.clone()),
            edit,
            turn,
            &mut crate::walk::NoModels,
        )
    }
}

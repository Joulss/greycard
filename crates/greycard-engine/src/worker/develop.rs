//! The develop: the open frame's picture, a walk of the window's list
//! from what its last walks kept.

use super::*;
use crate::stages::{Keep, LEARNED_DENOISER, Skip, Stage, Step};
use crate::walk::{Buf, NoHold, NoLearned, Place, walk};

/// [`develop_round`] with every fill made in the one walk, as the
/// tests develop.
// The job, the frame kept, and each model, device, and channel, held apart.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn develop_job(
    input: &Input,
    edit: &Edit,
    turn: u8,
    generation: u64,
    held: &mut Option<FrameKept>,
    ai: &mut Ai,
    cache: Option<&greycard_ai::DenoiseCache>,
    lenses: Option<&greycard_lens::Database>,
    gpu: &mut Option<greycard_gpu::Context>,
    deliver: &Deliver,
) -> (Outcome, Option<Arc<WorkingImage>>) {
    develop_round(
        input,
        edit,
        turn,
        generation,
        held,
        ai,
        (cache, lenses),
        gpu,
        deliver,
        &mut Round::all(),
    )
}

/// How many of a develop's fills a walk of it makes, and how many the
/// develop had ahead and has made: the window's develop delivers its
/// picture with no fill made ([`Round::first`]), then the worker walks
/// it again for each fill left ([`Round::next`]), so the picture comes
/// first and each fill lands as it is made.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Round {
    budget: Option<usize>,
    pub ahead: (usize, usize),
}

impl Round {
    /// Every fill made in the one walk, as an export makes them.
    #[cfg(test)]
    pub(super) fn all() -> Self {
        Self {
            budget: None,
            ahead: (0, 0),
        }
    }

    /// The window's develop: its picture with every fill kept in
    /// place and none made.
    pub(super) fn first() -> Self {
        Self {
            budget: Some(0),
            ahead: (0, 0),
        }
    }

    /// The next fill of a develop whose rounds so far left `ahead`.
    pub(super) fn next(ahead: (usize, usize)) -> Self {
        Self {
            budget: Some(1),
            ahead,
        }
    }
}

/// Develop `edit` on the open frame at `turn`: a walk of the window's
/// list ([`window_steps`]) from the last point `held` kept whose steps
/// are this list's, with `ai`'s models. With `gpu`, every stage with a
/// version on the device runs there: the CA and the lens one at a time
/// on pictures in memory, the Detail section and the sharpen as one
/// run with the picture before the sharpen kept on the device, ending
/// at the viewport's texture, so a sharpen slider costs the sharpen
/// alone. Without it (the export of the open frame), the reference's
/// path from points the device never touched, and the picture comes
/// back with its half floats. A device that declines a picture runs
/// that run on the CPU with its context kept; any other device error
/// lets the context go for the session.
/// The walk makes as many fills as `round` says and notes in it how
/// far the develop's fills have got.
#[allow(clippy::too_many_arguments)]
pub(super) fn develop_round(
    input: &Input,
    edit: &Edit,
    turn: u8,
    generation: u64,
    held: &mut Option<FrameKept>,
    ai: &mut Ai,
    (cache, lenses): (
        Option<&greycard_ai::DenoiseCache>,
        Option<&greycard_lens::Database>,
    ),
    gpu: &mut Option<greycard_gpu::Context>,
    deliver: &Deliver,
    round: &mut Round,
) -> (Outcome, Option<Arc<WorkingImage>>) {
    let start = Instant::now();
    let walking = input.walked();
    let facts = walking.facts(turn);
    let (steps, plan) = window_steps(edit, &facts, lenses, gpu.is_some());
    let h = match held {
        Some(h) if h.facts == FrameKept::new(&facts).facts => h,
        _ => held.insert(FrameKept::new(&facts)),
    };
    let from_shelf = std::mem::take(&mut h.from_shelf);
    h.developed_as = None;
    // The developed picture the last walk kept is the export's and the
    // save's to start from, not the window's: a walk from it would not
    // run the sharpen, whose blend and measure the viewport shows, and
    // a walk with a device ends at the viewport's texture.
    h.kept.take_working(Keep::Developed);
    let print = ai.base_print(edit, turn, crate::mask_base::Size::Full);
    // A round whose base is not kept makes it again, the learned
    // denoiser with it: not beside a UNet the queue held.
    if round.budget.is_some_and(|b| b > 0) && !h.kept.holds(Keep::Base) {
        ai.let_go_held_unet();
    }
    let mut models = WorkerModels::new(ai, cache, deliver, generation);
    models.edit = edit.clone();
    models.turn = turn;
    models.budget = round.budget;
    models.ahead = round.ahead;
    // A fill to make: the walk starts before the retouch, whatever a
    // point past it would serve.
    if round.budget.is_some_and(|b| b > 0) {
        h.kept.drop_unmade();
    }
    let walked = match walk(
        &steps,
        &plan,
        &walking,
        &mut h.kept,
        &mut models,
        gpu,
        &NoHold,
    ) {
        Ok(walked) => walked,
        Err(e) => {
            return (
                Outcome::Failed {
                    generation,
                    message: e.to_string(),
                },
                None,
            );
        }
    };
    // A base made in this walk is a new picture for the masks; one
    // turned where it stood is too, though it is the base kept.
    let turned = walked.from == Some(Keep::Base) && h.turn != turn && h.stamp > 0;
    let made = matches!(walked.from, None | Some(Keep::Pair));
    if made || turned {
        h.stamp += 1;
    }
    if turned {
        tracing::info!("base turned to {turn} quarter turns");
    }
    let from = if made {
        BaseFrom::Made
    } else if from_shelf {
        BaseFrom::Shelf
    } else {
        BaseFrom::Kept
    };
    // The masks' models see the base under the edit it was made for,
    // as their kept work of it is known by the base's stamp.
    if made || turned || h.stamp == 0 {
        h.edit = edit.clone();
    }
    h.turn = turn;
    // The masks' print is the edit's now, not the one the base was made
    // under: an export of this edit works it out alike, so a change the
    // base did not need makes the masks again on the same picture
    // rather than leave the viewport's apart from the export's.
    h.print = print;
    h.meta = walked.meta.clone();
    if let Some(image) = h.kept.picture(Keep::Base) {
        h.image = image.clone();
    }
    h.guide = walked
        .taps
        .guide
        .clone()
        .unwrap_or_else(|| Arc::new(greycard_render::finish::Guide::NONE));
    // The range masks' picture from before the Detail section is the
    // patched picture kept here, shared, not copied: the viewport puts
    // it on the GPU beside the planes.
    h.planes = greycard_render::detail::DetailPlanes::with_sample(
        walked.taps.planes.clone(),
        edit.samples_before_detail().then(|| h.patched_picture()),
        walked.meta.clip_level,
    );
    // Where each stage ran, and whether this walk ran it.
    let at = |want: &dyn Fn(&Stage) -> bool| {
        steps
            .steps
            .iter()
            .position(|s| matches!(s, Step::Op(stage) if want(stage)))
    };
    let on_device = |i: usize| walked.places.get(i) == Some(&Place::Gpu);
    let seconds_of = |name: &str| {
        walked
            .seconds
            .iter()
            .filter(|(n, _)| *n == name)
            .map(|(_, s)| s)
            .sum::<f64>()
    };
    h.on_gpu = steps.steps.iter().zip(&walked.places).any(|(s, p)| {
        *p == Place::Gpu
            && matches!(
                s,
                Step::Op(Stage::Ca(_) | Stage::Demosaic(..) | Stage::Lens(_))
            )
    });
    // A develop with a device lent that came back in memory ran
    // nothing of the edit there past the inline ops: what the device
    // holds was for another edit, and goes. One without a device (the
    // open file's export) leaves the window's as they are.
    if gpu.is_some() && !matches!(walked.buf, Buf::Texture(_)) {
        h.kept.forget_device();
    }
    // A run the device declined went on on the CPU, which kept its
    // picture before the sharpen in memory: a picture the size of the
    // frame on the frames largest, which the window keeps only on the
    // device; it goes. (With the Detail section off it is the patched
    // picture itself, which costs nothing more.)
    if gpu
        .as_ref()
        .and_then(|_| h.kept.picture(Keep::PreSharpen))
        .is_some_and(|pre| !Arc::ptr_eq(pre, &h.patched_picture()))
    {
        h.kept.take_working(Keep::PreSharpen);
    }
    let patched = h.patched_picture();
    // The refined masks' picture, made only for an edit that refines a
    // mask, and let go when it no longer does: tapped by the walk at the
    // patched picture, or made of it here.
    let mask_picture = if !greycard_render::refine::refines(edit) {
        h.mask_picture = None;
        h.kept.forget_mask_picture();
        None
    } else if let Some(picture) = walked.taps.picture.clone() {
        h.mask_picture = Some(KeptMaskPicture {
            patched: patched.clone(),
            picture: picture.clone(),
        });
        Some(picture)
    } else {
        Some(super::masks::mask_picture(h, &patched))
    };
    if let Some(ctx) = gpu.as_ref() {
        // What the device holds for a stage that is off goes.
        if edit.sharpen.options().is_none() && !edit.detail_wants().sharpen {
            ctx.release_sharpen();
        }
        if edit.detail.options().is_none() {
            ctx.release_local_contrast();
        }
    }
    let ca = at(&|s| matches!(s, Stage::Ca(_)))
        .filter(|&i| i >= walked.begin)
        .map(|i| {
            let ran = if on_device(i) { CaRan::Gpu } else { CaRan::Cpu };
            (ran, seconds_of("CA"))
        });
    let detail_ran = at(&|s| matches!(s, Stage::LocalContrast(_))).map(|i| {
        if i < walked.begin {
            DetailRan::Kept
        } else if on_device(i) {
            DetailRan::Gpu
        } else {
            DetailRan::Cpu(seconds_of("local contrast"))
        }
    });
    let dehaze_ran = at(&|s| matches!(s, Stage::Dehaze(_))).map(|i| {
        if i < walked.begin {
            DehazeRan::Kept
        } else if on_device(i) {
            DehazeRan::Gpu
        } else {
            DehazeRan::Cpu
        }
    });
    let sharpened = at(&|s| matches!(s, Stage::Sharpen(..)))
        .filter(|_| walked.sharpen.is_some())
        .map(|i| {
            if on_device(i) {
                Sharpened::Gpu
            } else {
                Sharpened::Cpu
            }
        });
    let learned = learned_report(
        edit,
        &steps,
        &walked,
        std::mem::replace(&mut models.learned, LearnedReport::Off),
    );
    for (name, why) in stood_down(&steps) {
        tracing::debug!("{name} stood down: {why}");
    }
    models.pending_after(&h.kept);
    round.ahead = models.ahead;
    let fills = std::mem::take(&mut models.fills);
    h.filling = !fills.pending.is_empty();
    let mut stages: Stages = walked
        .seconds
        .iter()
        .filter(|(n, _)| !matches!(*n, "CA" | "local contrast" | "dehaze" | "sharpen"))
        .copied()
        .collect();
    let (image, picture) = match walked.buf {
        Buf::Texture(texture) => (Developed::Texture(texture), None),
        Buf::Working(image) => {
            let halves = Arc::new(timed(&mut stages, "halves", || {
                Halves::from_image(&image, walked.sharpen_mask.as_deref())
            }));
            (Developed::Halves(halves), Some(image))
        }
        _ => {
            return (
                Outcome::Failed {
                    generation,
                    message: "the develop ended on the device with no texture".into(),
                },
                None,
            );
        }
    };
    let (width, height) = match &image {
        Developed::Texture(t) => (t.width(), t.height()),
        Developed::Halves(h) => (h.width, h.height),
    };
    let seconds = start.elapsed().as_secs_f64();
    note_developed(&DevelopLog {
        width,
        height,
        from,
        ca,
        learned: &learned,
        fills: &fills,
        stages: &stages,
        detail: detail_ran,
        dehazed: walked.dehaze.and(dehaze_ran),
        sharpened,
        total: seconds,
    });
    let outcome = Outcome::Developed {
        generation,
        turn,
        image,
        kind: walked.meta.source,
        guide: super::masks::guide_with(h, mask_picture),
        planes: h.planes.clone(),
        white: h.white(),
        seconds,
        detail: walked.detail.zip(detail_ran),
        sharpen: walked.sharpen,
        dehaze: walked.dehaze,
        sources: walked.sources,
        learned,
        fills,
    };
    h.developed_as = Some((edit.clone(), turn));
    h.left_out = left_out_of(&outcome, Some(h));
    (outcome, picture)
}

/// What of the edit the frame could not take, by name and why, other
/// than the learned denoiser, which [`learned_report`] says: a
/// panorama's lens profile, standing down on a canvas no one frame's
/// profile fits. For the log.
pub(super) fn stood_down(
    steps: &crate::stages::Steps,
) -> impl Iterator<Item = (&'static str, &'static str)> + '_ {
    steps.skipped.iter().filter_map(|(name, why)| match why {
        Skip::Unavailable(why) if *name != LEARNED_DENOISER => Some((*name, *why)),
        _ => None,
    })
}

/// What the learned denoiser did for a walk of `edit`'s `steps`, as
/// the window says it: its answer kept when the walk started past it,
/// none on a mosaic it has no version for, why the engine's own stood
/// in, or how `ran` had the network's answer.
fn learned_report(
    edit: &Edit,
    steps: &crate::stages::Steps,
    walked: &crate::walk::Walked,
    ran: LearnedReport,
) -> LearnedReport {
    if edit.noise.tier().is_none() {
        return LearnedReport::Off;
    }
    if walked.from.is_some() {
        return LearnedReport::Kept;
    }
    let unavailable = steps.skipped.iter().find_map(|(name, why)| match why {
        Skip::Unavailable(why) if *name == LEARNED_DENOISER => Some(*why),
        _ => None,
    });
    match (&walked.meta.stood_in, unavailable) {
        (_, Some(why)) => LearnedReport::Unavailable(why),
        (Some(NoLearned::Missing(model)), _) => LearnedReport::Missing(model),
        (Some(NoLearned::Failed(why) | NoLearned::Short(why)), _) => {
            LearnedReport::Failed(why.clone())
        }
        (None, None) => ran,
    }
}

/// Where a develop's sharpen ran.
#[derive(Clone, Copy)]
enum Sharpened {
    Cpu,
    Gpu,
}

/// Where a base develop's CA correction ran.
#[derive(Clone, Copy)]
pub(super) enum CaRan {
    Cpu,
    Gpu,
}

/// Where a develop's base came from.
#[derive(Clone, Copy, Debug, PartialEq)]
enum BaseFrom {
    /// Made in this develop.
    Made,
    /// The last develop's, kept.
    Kept,
    /// Taken back from the shelf of frames left behind.
    Shelf,
}

/// What a develop's log line reads: the picture's size, what was made
/// anew and what was kept, and the seconds in all.
struct DevelopLog<'a> {
    width: u32,
    height: u32,
    from: BaseFrom,
    ca: Option<(CaRan, f64)>,
    learned: &'a LearnedReport,
    fills: &'a FillReport,
    stages: &'a Stages,
    detail: Option<DetailRan>,
    dehazed: Option<DehazeRan>,
    sharpened: Option<Sharpened>,
    total: f64,
}

/// One line for a develop: what was made anew and what was kept,
/// and the seconds, the same the outcome carries. A model that is
/// missing is a state, named here and offered by the UI, not warned
/// on every develop; one that would not load is warned where the
/// load fails. A fill that failed is warned here, with its name.
fn note_developed(log: &DevelopLog<'_>) {
    warn_failed_fills(log.fills);
    tracing::info!("{}", developed_line(log));
}

/// The learned denoiser's part of a develop's line: how its answer
/// was had, and on which provider when it ran.
pub(super) fn learned_part(learned: &LearnedReport) -> Option<String> {
    Some(match learned {
        LearnedReport::Off => return None,
        LearnedReport::Ran {
            version,
            provider,
            seconds,
        } => format!("denoiser {version} on {provider} {seconds:.2} s"),
        LearnedReport::Cached { seconds } => format!("denoiser cached {seconds:.2} s"),
        LearnedReport::Kept => "denoiser kept".into(),
        LearnedReport::Missing(_) => "denoiser missing".into(),
        LearnedReport::Failed(_) => "denoiser failed".into(),
        LearnedReport::Unavailable(_) => "no learned denoiser for this mosaic".into(),
    })
}

/// The fills' parts of a develop's line: those made and their
/// seconds, and those whose model is missing.
pub(super) fn fill_parts(fills: &FillReport) -> Vec<String> {
    let mut parts = Vec::new();
    if !fills.made.is_empty() {
        parts.push(format!(
            "{} fill(s) {:.2} s",
            fills.made.len(),
            fills.seconds
        ));
    }
    if !fills.missing.is_empty() {
        parts.push(format!("{} fill(s) missing", fills.missing.len()));
    }
    if !fills.pending.is_empty() {
        parts.push(format!("{} fill(s) to make after", fills.pending.len()));
    }
    parts
}

/// Each fill the model failed, warned: the line counts none of them.
pub(super) fn warn_failed_fills(fills: &FillReport) {
    for (name, why) in &fills.failed {
        tracing::warn!("fill {name}: {why}");
    }
}

/// [`note_developed`]'s line: where the base came from, then the CA
/// and the denoiser, then each stage this develop ran and its seconds
/// (a fresh base's own, the retouch, and the picture's way to the
/// viewport: its upload to the device or its half floats), then the
/// fills and the ops after the base, and the total.
fn developed_line(log: &DevelopLog<'_>) -> String {
    let &DevelopLog {
        width,
        height,
        from,
        ca,
        learned,
        fills,
        stages,
        detail,
        dehazed,
        sharpened,
        total,
    } = log;
    let mut parts: Vec<String> = Vec::new();
    parts.push(
        match from {
            BaseFrom::Made => "base made",
            BaseFrom::Kept => "base kept",
            BaseFrom::Shelf => "base from the shelf",
        }
        .into(),
    );
    match ca {
        Some((CaRan::Gpu, s)) => parts.push(format!("CA on the GPU {s:.2} s")),
        Some((CaRan::Cpu, s)) => parts.push(format!("CA {s:.2} s")),
        None => {}
    }
    parts.extend(learned_part(learned));
    for (name, s) in stages {
        parts.push(format!("{name} {s:.2} s"));
    }
    parts.extend(fill_parts(fills));
    match detail {
        Some(DetailRan::Gpu) => parts.push("local contrast on the GPU".into()),
        Some(DetailRan::Cpu(s)) => parts.push(format!("local contrast {s:.2} s")),
        Some(DetailRan::Kept) | None => {}
    }
    match dehazed {
        Some(DehazeRan::Gpu) => parts.push("dehaze on the GPU".into()),
        Some(DehazeRan::Cpu) => parts.push("dehaze".into()),
        Some(DehazeRan::Kept) | None => {}
    }
    match sharpened {
        Some(Sharpened::Cpu) => parts.push("sharpen".into()),
        Some(Sharpened::Gpu) => parts.push("sharpen on the GPU".into()),
        None => {}
    }
    format!(
        "developed {width}x{height}: {}, total {total:.2} s",
        parts.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the frame could not take is read for the log: a panorama's
    /// lens profile, not the learned denoiser (its own report says it)
    /// nor what a purpose leaves out.
    #[test]
    fn what_stood_down_is_read_for_the_log() {
        let steps = crate::stages::Steps {
            steps: Vec::new(),
            skipped: vec![
                (LEARNED_DENOISER, Skip::Unavailable("no version for it")),
                (
                    "the lens profile",
                    Skip::Unavailable("a panorama's lens was corrected when it was merged"),
                ),
                ("the retouch", Skip::ModelsSee),
            ],
        };
        assert_eq!(
            stood_down(&steps).collect::<Vec<_>>(),
            [(
                "the lens profile",
                "a panorama's lens was corrected when it was merged"
            )]
        );
    }

    /// A fresh base's line names each of its stages with its seconds,
    /// the prepare after the CA, between the parts it had before; a
    /// base from the shelf says so, with its upload; a kept one is the
    /// line it was.
    #[test]
    fn the_develop_s_line_times_its_stages() {
        let stages: Stages = vec![
            ("prepare", 0.31),
            ("demosaic", 0.92),
            ("finish", 0.204),
            ("guide", 0.02),
            ("upload", 0.12),
        ];
        let line = developed_line(&DevelopLog {
            width: 8192,
            height: 5464,
            from: BaseFrom::Made,
            ca: Some((CaRan::Gpu, 0.25)),
            learned: &LearnedReport::Off,
            fills: &FillReport::default(),
            stages: &stages,
            detail: None,
            dehazed: None,
            sharpened: Some(Sharpened::Gpu),
            total: 2.5,
        });
        assert_eq!(
            line,
            "developed 8192x5464: base made, CA on the GPU 0.25 s, prepare 0.31 s, \
             demosaic 0.92 s, finish 0.20 s, guide 0.02 s, upload 0.12 s, \
             sharpen on the GPU, total 2.50 s"
        );

        let line = developed_line(&DevelopLog {
            width: 8192,
            height: 5464,
            from: BaseFrom::Shelf,
            ca: None,
            learned: &LearnedReport::Kept,
            fills: &FillReport::default(),
            stages: &vec![("upload", 0.118)],
            detail: None,
            dehazed: None,
            sharpened: Some(Sharpened::Gpu),
            total: 0.4,
        });
        assert_eq!(
            line,
            "developed 8192x5464: base from the shelf, denoiser kept, upload 0.12 s, \
             sharpen on the GPU, total 0.40 s"
        );

        // The demosaic on the device carries its seconds, as the CPU's
        // does, so a tester's log shows it.
        let line = developed_line(&DevelopLog {
            width: 8192,
            height: 5464,
            from: BaseFrom::Made,
            ca: Some((CaRan::Gpu, 0.05)),
            learned: &LearnedReport::Off,
            fills: &FillReport::default(),
            stages: &vec![
                ("prepare", 0.31),
                ("demosaic on the GPU", 0.094),
                ("finish", 0.2),
            ],
            detail: None,
            dehazed: None,
            sharpened: Some(Sharpened::Gpu),
            total: 0.9,
        });
        assert_eq!(
            line,
            "developed 8192x5464: base made, CA on the GPU 0.05 s, prepare 0.31 s, \
             demosaic on the GPU 0.09 s, finish 0.20 s, sharpen on the GPU, total 0.90 s"
        );

        let line = developed_line(&DevelopLog {
            width: 300,
            height: 200,
            from: BaseFrom::Kept,
            ca: None,
            learned: &LearnedReport::Off,
            fills: &FillReport::default(),
            stages: &Stages::new(),
            detail: Some(DetailRan::Cpu(0.5)),
            dehazed: Some(DehazeRan::Cpu),
            sharpened: Some(Sharpened::Cpu),
            total: 0.75,
        });
        assert_eq!(
            line,
            "developed 300x200: base kept, local contrast 0.50 s, dehaze, sharpen, total 0.75 s"
        );
    }
}

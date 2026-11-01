//! The export path: opening a frame for it, and writing its picture.

use greycard_core::develop::retouch::Region;
use greycard_edit::retouch::Patch;

use super::*;
use crate::walk::NoLearned;

/// The open file's picture under `edit` at `turn`: the last develop's
/// when it was of the same develop and turn, else developed afresh.
/// The export's picture is the reference's, on the CPU, whatever the
/// viewport ran on. If the last develop had its CA or its lens on the
/// GPU, redevelop.
#[allow(clippy::too_many_arguments)]
pub(super) fn open_picture(
    edit: &Edit,
    turn: u8,
    input: Option<&Input>,
    last: &mut Option<Last>,
    base: &mut Option<Base>,
    learned: &mut Option<LearnedBase>,
    ai: &mut Ai,
    cache: Option<&greycard_ai::DenoiseCache>,
    lenses: Option<&greycard_lens::Database>,
    deliver: &Deliver,
) -> Result<(Arc<WorkingImage>, Vec<String>), String> {
    if let Some(l) = &*last
        && l.turn == turn
        && l.edit.same_develop(edit)
        && !l.on_gpu
    {
        let mut left_out = l.left_out.clone();
        left_out.extend(input.and_then(|f| lens_left_out(f, edit, lenses)));
        return Ok((l.image.clone(), left_out));
    }
    let Some(f) = input else {
        return Err("no file is open".into());
    };
    match develop_job(
        f, edit, turn, 0, base, learned, ai, cache, lenses, &mut None, deliver,
    ) {
        (outcome, Some(image)) => {
            *last = base
                .as_ref()
                .map(|b| Last::made(edit.clone(), b, image.clone(), &outcome));
            let mut left_out = left_out_of(&outcome, base.as_ref());
            left_out.extend(lens_left_out(f, edit, lenses));
            Ok((image, left_out))
        }
        (Outcome::Failed { message, .. }, None) => Err(message),
        _ => Err("the develop made no picture".into()),
    }
}

/// The lens picked by hand that the export goes without, when it does
/// ([`crate::lens::left_out`]).
fn lens_left_out(
    input: &Input,
    edit: &Edit,
    lenses: Option<&greycard_lens::Database>,
) -> Option<String> {
    let (make, model, shot) = input.identity();
    crate::lens::left_out((make, model, shot, input.is_raw()), &edit.lens, lenses)
}

/// A set frame's edit with its learned-denoiser blend seeded from the
/// raw's ISO when it has no sidecar yet, as its first open would.
pub(super) fn seeded(edit: &Edit, seed: bool, input: Option<&Input>) -> Edit {
    let mut edit = edit.clone();
    if seed && let Some(Input::Raw(f)) = input {
        edit.noise.learned_strength = Noise::blend_for_iso(f.shot.iso);
    }
    edit
}

/// A frame of a set that is not the open file: opened, walked through
/// its stage list for the export ([`walk_export`]), and written to
/// `path`, the open file's base and learned pair left as they were for
/// the next develop. The edit it was written under, and what of it
/// was left out.
pub(super) fn export_other(
    frame: &crate::queue::Frame,
    ai: &mut Ai,
    cache: Option<&greycard_ai::DenoiseCache>,
    lenses: Option<&greycard_lens::Database>,
    deliver: &Deliver,
    settings: &greycard_render::export::Settings,
    path: &std::path::Path,
) -> Result<(Edit, Vec<String>), String> {
    let (input, metadata) = timed_open(&frame.source).map_err(|e| format!("{e:#}"))?;
    let edit = seeded(&frame.edit, frame.seed_blend, Some(&input));
    ai.forget(Some(frame.source.clone()));
    let lens_words = lens_left_out(&input, &edit, lenses);
    let (rendered, mut left_out) = walk_export(
        input,
        &edit,
        frame.turn % 4,
        // A set frame is no develop of the window's: its fills are
        // said under the generation no develop has.
        &mut ExportModels::new(ai, cache, deliver, 0),
        lenses,
        settings,
    )?;
    left_out.extend(lens_words);
    write_rendered(
        &rendered,
        &edit,
        Some(&frame.source),
        Some(&metadata),
        frame.time_shift,
        settings,
        path,
    )?;
    Ok((edit, left_out))
}

/// `input` walked under `edit` at `turn` through its stage list for
/// the export, on the CPU and from nothing kept, with `models`'s
/// models, and finished under the sheet's `settings` with its border
/// and mark: the file's pixels. Beside them, what of the edit was left
/// out, in [`left_out_of`]'s words and then [`learned_masks`]'.
pub(super) fn walk_export(
    input: Input,
    edit: &Edit,
    turn: u8,
    models: &mut ExportModels<'_>,
    lenses: Option<&greycard_lens::Database>,
    settings: &greycard_render::export::Settings,
) -> Result<(greycard_render::export::Rendered, Vec<String>), String> {
    use crate::stages::{Keep, LEARNED_DENOISER, Purpose, Skip, Stage, Step, Tap, stages};
    use crate::walk::{Buf, Kept, NoHold, plan, walk};

    let started = Instant::now();
    let input = match input {
        Input::Raw(f) => crate::walk::Input::Raw(f),
        Input::Picture { image, meta } => crate::walk::Input::Picture { image, meta },
    };
    models.edit = edit.clone();
    models.turn = turn;
    let purpose = Purpose::Export(Box::new(settings.clone()));
    let mut steps = stages(edit, &purpose, &input.facts(turn), lenses);
    // Nothing is kept for a later walk: each kept point holds a picture
    // the size of the frame, and the stage after it would copy it
    // rather than change it. The patched picture alone stays when a
    // learned mask is made beside a Fill, which it is made on where the
    // Fill meets it; no other patch changes what a mask is made on.
    let on_patched = steps
        .steps
        .iter()
        .any(|s| matches!(s, Step::Tap(Tap::Masks(_))))
        && steps.ops().any(|s| {
            matches!(s, Stage::Retouch(r, _)
                if r.patches.iter().any(|p| p.method == greycard_edit::retouch::Method::Fill))
        });
    steps.steps.retain(|s| match s {
        Step::Keep(Keep::Patched) => on_patched,
        Step::Keep(_) => false,
        _ => true,
    });
    let plan = plan(&steps, &purpose, false);
    let mut kept = Kept::new();
    let walked = walk(&steps, &plan, &input, &mut kept, models, &mut None, &NoHold)
        .map_err(|e| e.to_string())?;
    drop(kept);
    drop(input);
    debug_assert!(walked.without.is_empty(), "an export runs its fills");
    let fills = std::mem::take(&mut models.fills);
    let masks = std::mem::take(&mut models.masks);
    let learned = std::mem::replace(&mut models.learned, LearnedReport::Off);
    // The learned denoiser as the worker's develop reports it: what
    // stood in for it, or the mosaic it has no version for.
    let unavailable = steps.skipped.iter().find_map(|(name, why)| match why {
        Skip::Unavailable(why) if *name == LEARNED_DENOISER => Some(*why),
        _ => None,
    });
    let learned = match (&walked.meta.stood_in, unavailable) {
        (Some(NoLearned::Missing(model)), _) => LearnedReport::Missing(model),
        (Some(NoLearned::Failed(why)), _) => LearnedReport::Failed(why.clone()),
        (None, Some(why)) => LearnedReport::Unavailable(why),
        (None, None) => learned,
    };
    let mut left_out = left_out_words(&learned, walked.meta.gave_way.as_ref(), &fills);
    left_out.extend(super::masks::look_left_out(edit));
    left_out.extend(masks);
    let mut walked = walked;
    // Neither is read by the finish: the base the masks were made on,
    // and the sharpen's blend, which the viewport alone paints.
    walked.taps.masks = None;
    walked.sharpen_mask = None;
    let finish = walked.finish_input();
    let Buf::Working(image) = walked.buf else {
        unreachable!("a walk on the CPU ends at a picture in memory")
    };
    // The develop's line, its denoiser and fills in its words.
    super::develop::warn_failed_fills(&fills);
    let parts: Vec<String> = super::develop::learned_part(&learned)
        .into_iter()
        .chain(
            walked
                .seconds
                .iter()
                .map(|(stage, s)| format!("{stage} {s:.2} s")),
        )
        .chain(super::develop::fill_parts(&fills))
        .collect();
    tracing::info!(
        "developed {}x{} for the export: {}, total {:.2} s",
        image.width,
        image.height,
        parts.join(", "),
        started.elapsed().as_secs_f64()
    );
    let rendered = crate::finish::finish_file(&image, edit, &finish, settings)
        .map_err(|e| format!("{e:#}"))?;
    Ok((rendered, left_out))
}

/// The models an export's walk runs, the set's own: the learned
/// denoiser through the disk cache, the fills, and the learned masks,
/// each one that could not be had noted as the worker's develop and
/// [`learned_masks`] note it.
pub(super) struct ExportModels<'a> {
    ai: &'a mut Ai,
    cache: Option<&'a greycard_ai::DenoiseCache>,
    deliver: &'a Deliver,
    /// The job's generation, which its fills are said under.
    generation: u64,
    /// The edit and the turn the walk is of.
    edit: Edit,
    turn: u8,
    /// How the learned denoiser's answer was had, for the log line.
    learned: LearnedReport,
    fills: FillReport,
    /// Each learned mask not made, in words.
    masks: Vec<String>,
}

impl<'a> ExportModels<'a> {
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
        }
    }
}

impl ExportModels<'_> {
    /// The learned denoiser `model`, loaded now if it is not yet; why
    /// not when it cannot be.
    fn denoiser(
        &mut self,
        model: &'static greycard_ai::Model,
    ) -> Result<&mut greycard_ai::Denoiser, NoLearned> {
        self.ai.denoiser(model).map_err(|e| match e {
            NoDenoiser::Missing => NoLearned::Missing(model),
            NoDenoiser::Failed(why) => NoLearned::Failed(why),
        })
    }
}

/// What the models' cache knows an export's picture by: each frame of
/// a set is the first picture made of it.
const EXPORT_STAMP: u64 = 1;

impl crate::walk::Models for ExportModels<'_> {
    fn can_denoise(&mut self, model: &'static greycard_ai::Model) -> Result<(), NoLearned> {
        self.denoiser(model).map(|_| ())
    }

    fn denoise(
        &mut self,
        model: &'static greycard_ai::Model,
        frame: &RawFrame,
        prepared: &Prepared,
    ) -> Result<Vec<f32>, NoLearned> {
        let cache = self.cache;
        let denoiser = self.denoiser(model)?;
        let (rgb, report) = super::base::network(frame, prepared, model, denoiser, cache)
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
        self.ai.could_fill_more(retouch, picture)
    }

    fn rasters(
        &mut self,
        how: crate::stages::Masks,
        base: &Arc<WorkingImage>,
        patched: Option<crate::walk::Patched<'_>>,
        meta: &crate::walk::Meta,
    ) -> crate::walk::Rasters {
        debug_assert_eq!(how, crate::stages::Masks::Make, "an export makes its masks");
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

/// Finish `image`, the develop `base` was made for, under `edit` and
/// the sheet's `settings`, and write it to `path`: the learned masks
/// made now if they are not yet, the edit's geometry, the finish, the
/// mark, and the file with the source's EXIF, its capture times moved
/// by the frame's `time_shift`. What of the edit's masks could not be
/// made, and so was left out.
#[allow(clippy::too_many_arguments)]
pub(super) fn write_export(
    image: Arc<WorkingImage>,
    edit: &Edit,
    base: Option<&Base>,
    ai: &mut Ai,
    source_path: Option<&std::path::Path>,
    metadata: Option<&greycard_core::decode::RawMetadata>,
    time_shift: i64,
    settings: &greycard_render::export::Settings,
    path: &std::path::Path,
) -> Result<Vec<String>, String> {
    let (mut rendered, left_out) = finish_export(image, edit, base, ai, settings);
    // The border and then the mark, on the export alone; a mark that
    // cannot be drawn fails the export rather than let an unmarked
    // picture out.
    greycard_render::export::border(&mut rendered, settings);
    greycard_render::export::mark(&mut rendered, settings).map_err(|e| format!("{e:#}"))?;
    write_rendered(
        &rendered,
        edit,
        source_path,
        metadata,
        time_shift,
        settings,
        path,
    )?;
    Ok(left_out)
}

/// The file of `rendered`, finished under `edit`, at `path`: the
/// source's EXIF with its capture times moved by the frame's
/// `time_shift`, and its name and edit as where it came from.
fn write_rendered(
    rendered: &greycard_render::export::Rendered,
    edit: &Edit,
    source_path: Option<&std::path::Path>,
    metadata: Option<&greycard_core::decode::RawMetadata>,
    time_shift: i64,
    settings: &greycard_render::export::Settings,
    path: &std::path::Path,
) -> Result<(), String> {
    let shifted = metadata.map(|m| greycard_core::exif::with_capture_shifted(m, time_shift));
    let origin = greycard_render::export::Origin {
        source_name: source_path
            .and_then(|s| s.file_name())
            .map(|n| n.to_string_lossy().into_owned()),
        edit: Some(edit.to_json()),
    };
    greycard_render::export::write(rendered, settings, path, shifted.as_ref(), &origin)
        .map_err(|e| format!("{e:#}"))
}

/// What [`finish_export`] would hand the finish for `image` under
/// `edit` and `settings`, before any pixel is finished
/// ([`greycard_render::export::Scene`]): the edit's geometry, the resize, the
/// guide plane and the vignette, as the export has them. No masks: the
/// measurement this serves carries none.
pub(super) fn scene_export(
    image: &WorkingImage,
    edit: &Edit,
    base: Option<&Base>,
    settings: &greycard_render::export::Settings,
) -> greycard_render::export::Scene {
    let source = (image.width as u32, image.height as u32);
    let framed;
    let image: &WorkingImage = if edit.geometry.is_identity() {
        image
    } else {
        framed = greycard_render::geometry::apply(image, &edit.geometry);
        &framed
    };
    greycard_render::export::scene_framed(image, edit, source, settings, base.map(|b| &*b.guide))
}

/// `image`, the develop `base` was made for, finished under `edit` and
/// `settings` as an export is: the learned masks made now if they are
/// not yet, the edit's geometry, then the finish. No mark and no file.
/// Beside it, what the edit names that could not be had, which the
/// picture is finished without, as the window draws it without: a
/// look not in the look directory, and each learned mask whose model
/// is not downloaded or failed.
pub(super) fn finish_export(
    image: Arc<WorkingImage>,
    edit: &Edit,
    base: Option<&Base>,
    ai: &mut Ai,
    settings: &greycard_render::export::Settings,
) -> (greycard_render::export::Rendered, Vec<String>) {
    let source = (image.width as u32, image.height as u32);
    let (rasters, left_out) = learned_masks(edit, base, ai);
    let framed;
    let image: &WorkingImage = if edit.geometry.is_identity() {
        &image
    } else {
        framed = greycard_render::geometry::apply(&image, &edit.geometry);
        &framed
    };
    let clip_level = base.map(|b| b.clip_level).unwrap_or(f32::INFINITY);
    // The masks' bands through the same geometry as the picture, when
    // they were made for this edit and this picture's size.
    let planes = base
        .and_then(|b| b.planes.as_ref())
        .filter(|k| k.serves_edit(edit))
        .map(|k| &k.planes)
        .filter(|p| (p.width as u32, p.height as u32) == source);
    let detail = planes.map(|p| {
        let bands = p.to_image();
        if edit.geometry.is_identity() {
            bands
        } else {
            greycard_render::geometry::apply(&bands, &edit.geometry)
        }
    });
    let guide = base.map(|b| super::masks::guide_with(b, refined_picture(b, edit)));
    let rendered = greycard_render::export::render(
        image,
        edit,
        &greycard_edit_io::Stores::user(),
        source,
        settings,
        &rasters,
        clip_level,
        guide.as_deref(),
        base.map(|b| b.source).unwrap_or_default(),
        base.and_then(Base::white_shift).as_ref(),
        detail.as_ref().zip(planes.map(|p| &p.meta)),
    );
    (rendered, left_out)
}

/// The picture `edit`'s refined masks are made against for an export
/// of `b`: the one its develop kept when that is of this picture and
/// Detail section, else made now from the patched picture. None when
/// the edit refines no mask.
///
/// The kept one may be the GPU's: with the Detail section on, the
/// window's develop reads it back from the device's picture before
/// the sharpen, where a set's walk makes its own on the CPU, so the
/// two exports' refined masks can differ by what the GPU's Detail
/// differs from the CPU's.
fn refined_picture(b: &Base, edit: &Edit) -> Option<Arc<greycard_render::refine::MaskPicture>> {
    if !greycard_render::refine::refines(edit) {
        return None;
    }
    if let Some(kept) = super::masks::kept_picture(b, edit) {
        return Some(kept);
    }
    let patched = b
        .patched
        .as_ref()
        .map_or_else(|| b.image.clone(), |p| p.1.clone());
    let pre = super::masks::pre_sharpen(&patched, edit, b.clip_level);
    Some(Arc::new(greycard_render::refine::mask_picture(&pre)))
}

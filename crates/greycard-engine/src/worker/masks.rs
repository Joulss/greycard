//! The learned masks' rasters, for an export, a save and the reduced
//! develop.

use super::*;

/// Learned masks' rasters by adjustment id and component.
type Rasters = std::collections::HashMap<(u64, usize), Arc<Raster>>;

/// The learned masks `edit` draws, made now if they are not yet, for
/// the picture `base` was made for; and what the edit names that could
/// not be had: a look not in the look directory, and each learned mask
/// whose model is not downloaded or failed.
pub(super) fn learned_masks(
    edit: &Edit,
    base: Option<&FrameKept>,
    ai: &mut Ai,
) -> (Rasters, Vec<String>) {
    let mut left_out: Vec<String> = look_left_out(edit).into_iter().collect();
    let Some(b) = base else {
        return (Rasters::new(), left_out);
    };
    let (rasters, masks) = learned_rasters(
        edit,
        ai,
        &On {
            stamp: b.stamp,
            image: &b.image,
            retouched: retouched_of(b, Some(edit)),
            edit: &b.edit,
            source: b.meta.source,
            turn: b.turn,
        },
    );
    left_out.extend(masks);
    (rasters, left_out)
}

/// A look the edit names that the export cannot have, not in the
/// look directory or with no table for the picture's display curve:
/// the picture is finished without it, as the window draws it
/// without, and said in the Look section's own words.
pub(super) fn look_left_out(edit: &Edit) -> Option<String> {
    let greycard_edit::look::LutChoice::Named(name) = &edit.look_lut.lut else {
        return None;
    };
    let missed = edit.look_lut.strength > 0.0
        && edit
            .look_lut
            .look_under(&greycard_edit_io::Stores::user(), edit.display_curve)
            .is_none();
    missed.then(|| {
        let why = if greycard_edit_io::looks::is_there(&edit.look_lut) {
            greycard_edit::look::mismatch(
                &greycard_edit_io::looks::list(),
                name,
                edit.display_curve,
            )
            .map(|(why, _)| why)
            .unwrap_or_else(|| "its table would not read".into())
        } else {
            "not in the look directory".into()
        };
        format!(
            "the look {name}: {}; written without it",
            why.trim_end_matches(['.', ' '])
        )
    })
}

/// The picture the learned masks are made on, and what the models'
/// cache knows it by.
pub(super) struct On<'a> {
    /// Counts the pictures made, for the masks to know theirs.
    pub stamp: u64,
    pub image: &'a WorkingImage,
    /// The picture with its retouch applied, when a patch is on it.
    pub retouched: Option<crate::ai::Retouched<'a>>,
    /// The edit the picture was developed under.
    pub edit: &'a Edit,
    pub source: greycard_render::finish::Source,
    pub turn: u8,
}

/// Each learned mask `edit` draws, made now on `on` if it is not yet;
/// and each one whose model is not downloaded or failed, in words.
pub(super) fn learned_rasters(edit: &Edit, ai: &mut Ai, on: &On<'_>) -> (Rasters, Vec<String>) {
    let mut rasters = Rasters::new();
    let mut left_out = Vec::new();
    for a in &edit.adjustments {
        // An adjustment switched off, or with nothing live in its
        // mask, is not drawn (`finish::Local::of`): nothing of it
        // is made, and nothing of it is missed.
        if !a.enabled || a.mask.is_empty() {
            continue;
        }
        for (i, c) in a.mask.live() {
            // An Object with nothing picked yet asks for nothing.
            if !c.shape.is_learned() || !crate::ai::prompted(&c.shape) {
                continue;
            }
            match ai.raster_retouched(
                on.stamp,
                on.image,
                on.retouched,
                on.edit,
                on.source,
                on.turn,
                (a.id, i),
                &c.shape,
                None,
            ) {
                Ok((made, _)) => {
                    // A Part whose person is not settled here goes
                    // out empty, and the log says so.
                    if let Some(part) = &made.part {
                        let why = part.left_out();
                        left_out.push(format!(
                            "{}'s {} shape: {why}; its mask written empty",
                            a.name,
                            c.shape.name()
                        ));
                    }
                    rasters.insert((a.id, i), made.raster);
                }
                Err(why) => left_out.push(format!(
                    "{}'s {} shape: {why}; its mask written without it",
                    a.name,
                    c.shape.name()
                )),
            }
        }
    }
    (rasters, left_out)
}

/// The base's picture with its retouch applied, when that is `edit`'s
/// retouch (any edit for `None`): the picture a learned shape a patch
/// meets is made on ([`Ai::raster_retouched`]).
pub(super) fn retouched_of<'a>(
    b: &'a FrameKept,
    edit: Option<&Edit>,
) -> Option<crate::ai::Retouched<'a>> {
    let patched = b.kept.patched()?;
    if edit.is_some_and(|e| !same_retouch(&e.retouch, patched.retouch)) {
        return None;
    }
    Some(crate::ai::Retouched {
        retouch: patched.retouch,
        image: patched.image,
        unmade: patched.unmade,
    })
}

/// Whether `applied`, a retouch as a develop applied it, is `edit`'s:
/// the same patches, a source the develop chose standing for one the
/// edit has not been given yet.
pub(super) fn same_retouch(edit: &Retouch, applied: &Retouch) -> bool {
    edit.enabled == applied.enabled
        && edit.patches.len() == applied.patches.len()
        && edit.patches.iter().zip(&applied.patches).all(|(p, q)| {
            p == q
                || (p.source.is_none()
                    && *p
                        == greycard_edit::retouch::Patch {
                            source: None,
                            ..q.clone()
                        })
        })
}

/// The learned components the panel was given a raster of whose
/// patches the develop of `edit` on `base` changed: each to be asked for
/// again, behind the develop.
pub(super) fn to_ask_again(
    edit: &Edit,
    base: Option<&FrameKept>,
    ai: &Ai,
) -> Vec<(Key, Shape, Option<&'static greycard_ai::Model>)> {
    let Some(b) = base else {
        return Vec::new();
    };
    let live = |key: Key, shape: &Shape| {
        edit.adjustments
            .iter()
            .find(|a| a.id == key.0)
            .and_then(|a| a.mask.components.get(key.1))
            .is_some_and(|c| c.enabled && c.shape == *shape)
    };
    ai.to_ask_again(retouched_of(b, None), b.turn, live)
}

/// How [`masks_for`] ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MasksMade {
    /// Every learned shape's raster is in the cache (or the edit draws
    /// none any more): the frame is asked for again.
    All,
    /// Not every one could be made (a model missing or failing, the
    /// develop refused): the frame waits for a model to be fetched.
    Short,
    /// It gave way between two shapes to a develop, a save, or a frame
    /// opened in the editor; the shapes made so far are in the cache.
    GaveWay,
}

/// The learned masks of `frame` under its line's edit, made and put in
/// the mask cache (`Job::MasksFor`): the frame is decoded and developed
/// reduced for the picture the models see, and each learned shape the
/// edit draws is made as an export of the frame would make it,
/// `waiting` asked between two whether to give way. The rasters go in
/// the cache the open frame's masks are read from, made from the
/// reduced develop's picture, which the models see at their own 2048
/// anyway.
pub(super) fn masks_for(
    frame: &Frame,
    ai: &mut Ai,
    lenses: Option<&greycard_lens::Database>,
    waiting: &dyn Fn() -> bool,
) -> MasksMade {
    let path = frame.path.as_path();
    let name = frame.said();
    let started = Instant::now();
    let (mut sidecar, trouble) = greycard_edit_io::on_disk::load_sidecar_said(path, true);
    if trouble.is_some() {
        tracing::info!("the masks of {name}: its sidecar could not be read");
        return MasksMade::Short;
    }
    let Some(line) = sidecar.line_mut(frame.version()) else {
        tracing::info!("the masks of {name}: no such version in its sidecar");
        return MasksMade::Short;
    };
    let edit = std::mem::take(&mut line.current);
    let turn = sidecar.turn;
    if !crate::reduced::draws_learned(&edit) {
        return MasksMade::All;
    }
    // The models see the reduced develop without the retouch, as they
    // do the open frame's base, so the rasters go in the base's slots,
    // and a Fill, which the reduced develop refuses, does not stop them.
    let developed = match crate::reduced::develop_as(
        path,
        &edit,
        turn,
        lenses,
        &crate::stages::Purpose::ModelsSee,
    ) {
        Ok(developed) => developed,
        Err(why) => {
            tracing::info!("the masks of {name}: not made: {why}");
            return MasksMade::Short;
        }
    };
    ai.forget(Some(path.to_path_buf()));
    let (mut made, mut wanted) = (0, 0);
    for a in &edit.adjustments {
        if !a.enabled || a.mask.is_empty() {
            continue;
        }
        for (i, c) in a.mask.live() {
            if !c.shape.is_learned() || !crate::ai::prompted(&c.shape) {
                continue;
            }
            if wanted > 0 && waiting() {
                tracing::info!(
                    "the masks of {name}: {made} made, giving way to the editor; the rest after"
                );
                return MasksMade::GaveWay;
            }
            wanted += 1;
            // The raster's width is the frame's, not the half-size
            // picture's: the open frame makes the same one.
            let full = (developed.full.0 as usize, developed.full.1 as usize);
            match ai.raster_for(
                full,
                1,
                &developed.image,
                &edit,
                developed.kind,
                turn,
                (a.id, i),
                &c.shape,
                None,
            ) {
                Ok(_) => made += 1,
                Err(why) => tracing::info!(
                    "the masks of {name}: {}'s {} shape: {why}",
                    a.name,
                    c.shape.name()
                ),
            }
        }
    }
    tracing::info!(
        "the masks of {name}: {made} of {wanted} made for the reduced develop in {:.1} s",
        started.elapsed().as_secs_f64()
    );
    if made == wanted {
        MasksMade::All
    } else {
        MasksMade::Short
    }
}

/// A person picked on one frame, looked for on another frame of the
/// set ([`Job::PersonOn`]): the frame decoded and developed reduced
/// under its line's edit, as [`masks_for`] does, and its people found
/// on the People model's look-neutral view of that.
pub(super) fn person_on(
    job: &PersonJob,
    ai: &mut Ai,
    lenses: Option<&greycard_lens::Database>,
) -> Result<crate::ai::PersonOn, String> {
    let started = Instant::now();
    // A Fill's patch is the open frame's alone, and refuses the reduced
    // develop; the people are found on the base, which it is not in.
    let developed = crate::reduced::develop_as(
        &job.frame.path,
        &job.edit,
        job.turn,
        lenses,
        &crate::stages::Purpose::ModelsSee,
    )
    .map_err(|why| why.to_string())?;
    // Another file: its hash is the one a person found here records.
    ai.forget(Some(job.frame.path.clone()));
    let found = ai.person_on(
        &developed.image,
        developed.kind,
        job.turn,
        job.carried.as_ref(),
        &job.wanted,
    );
    let what = match &found {
        Ok(crate::ai::PersonOn::Found(_)) => "found".to_string(),
        Ok(crate::ai::PersonOn::Settled) => "its own person decided already".to_string(),
        Ok(crate::ai::PersonOn::NotFound) => "not found".to_string(),
        Err(why) => format!("not looked for ({why})"),
    };
    tracing::info!(
        "the person picked: {what} on {} in {:.1} s",
        job.frame.said(),
        started.elapsed().as_secs_f64()
    );
    found
}

/// The picture before the sharpen, where a develop has it.
pub(super) enum Pre<'a> {
    /// In memory.
    Cpu(&'a WorkingImage),
    /// The viewport's texture the GPU's Detail section wrote, with the
    /// sharpen off: the picture before the sharpen as it is shown.
    Viewport(&'a greycard_gpu::Context, &'a greycard_gpu::wgpu::Texture),
    /// Not in hand: read back from the GPU's kept picture when that is
    /// this one, else made again on the CPU.
    Find(Option<&'a greycard_gpu::Context>),
}

/// Rows the GPU's pictures are read back in: 256 rows of an 8256-wide
/// frame are 34 MB of staging, where the whole frame is 720 MB.
const BAND: usize = 256;

/// The picture the refined masks of `edit` are made against
/// ([`greycard_render::refine::mask_picture`]): of the picture before
/// the sharpen, `patched` with the Detail section run on it, as the
/// finish's range shapes read it. `pre` says where that picture is;
/// with the section off it is `patched` itself. A picture on the GPU
/// is read back a band at a time, straight into the masks' grid. Kept
/// on the base for the next develop of the same picture and section.
pub(super) fn mask_picture(
    b: &mut FrameKept,
    patched: &Arc<WorkingImage>,
    edit: &Edit,
    pre: Pre<'_>,
) -> Arc<greycard_render::refine::MaskPicture> {
    if let Some(k) = &b.mask_picture
        && Arc::ptr_eq(&k.patched, patched)
        && k.detail == edit.detail
    {
        return k.picture.clone();
    }
    use greycard_render::refine::{mask_picture as made, mask_picture_by_bands};
    let started = Instant::now();
    let detail_on = edit.detail.options().is_some() || edit.detail.dehaze_options().is_some();
    let read = match pre {
        Pre::Cpu(image) => Some(made(image)),
        _ if !detail_on => Some(made(patched)),
        Pre::Viewport(ctx, texture) => {
            let (w, h) = (texture.width() as usize, texture.height() as usize);
            mask_picture_by_bands(w, h, BAND, |y, n| {
                let bytes = ctx.read_viewport_rows(texture, y as u32, n as u32).ok()?;
                let halves: &[half::f16] = bytemuck::cast_slice(&bytes);
                Some(
                    halves
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .flat_map(|px| [px[0].to_f32(), px[1].to_f32(), px[2].to_f32()])
                        .collect(),
                )
            })
        }
        Pre::Find(gpu) => gpu
            .zip(b.kept.device_picture(crate::stages::Keep::PreSharpen))
            .filter(|_| {
                b.pre
                    .as_ref()
                    .is_some_and(|(p, detail)| Arc::ptr_eq(p, patched) && *detail == edit.detail)
            })
            .and_then(|(ctx, image)| {
                let (w, h) = (image.width() as usize, image.height() as usize);
                mask_picture_by_bands(w, h, BAND, |y, n| {
                    ctx.download_rows(image, y as u32, n as u32).ok()
                })
            }),
    };
    let picture =
        Arc::new(read.unwrap_or_else(|| made(&pre_sharpen(patched, edit, b.meta.clip_level))));
    tracing::info!(
        "the refined masks' picture: {:.3} s",
        started.elapsed().as_secs_f64()
    );
    b.mask_picture = Some(KeptMaskPicture {
        patched: patched.clone(),
        detail: edit.detail,
        picture: picture.clone(),
    });
    picture
}

/// The picture `edit`'s refined masks are made against on `b`, kept
/// or made now from the base's patched picture (read back from the
/// GPU's when that is it): None when the edit refines no mask.
pub(super) fn picture_for(
    b: &mut FrameKept,
    edit: &Edit,
    gpu: Option<&greycard_gpu::Context>,
) -> Option<Arc<greycard_render::refine::MaskPicture>> {
    if !greycard_render::refine::refines(edit) {
        return None;
    }
    let patched = b.patched_picture();
    Some(mask_picture(b, &patched, edit, Pre::Find(gpu)))
}

/// `patched` with `edit`'s Detail section run on it on the CPU, as the
/// develop runs it before the sharpen: itself when the section is off.
pub(super) fn pre_sharpen<'a>(
    patched: &'a Arc<WorkingImage>,
    edit: &Edit,
    clip_level: f32,
) -> std::borrow::Cow<'a, WorkingImage> {
    let local = edit.detail.options();
    let haze = edit.detail.dehaze_options();
    if local.is_none() && haze.is_none() {
        return std::borrow::Cow::Borrowed(patched);
    }
    let mut image = (**patched).clone();
    if let Some(options) = local {
        greycard_core::develop::local_contrast::local_contrast(&mut image, &options, clip_level);
    }
    if let Some(options) = haze {
        greycard_core::develop::dehaze::dehaze(&mut image, &options);
    }
    std::borrow::Cow::Owned(image)
}

/// `b`'s guide plane with the refined masks' `picture` beside it, when
/// there is one: a plane of its own, so the base's stays without.
pub(super) fn guide_with(
    b: &FrameKept,
    picture: Option<Arc<greycard_render::refine::MaskPicture>>,
) -> Arc<greycard_render::finish::Guide> {
    match picture {
        Some(p) => Arc::new(b.guide.with_picture(Some(p))),
        None => b.guide.clone(),
    }
}

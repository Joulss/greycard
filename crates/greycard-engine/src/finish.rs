//! The one finish: what the shader and the render crate's CPU finish
//! both read off a walk, and the CPU's.

use std::sync::Arc;

use greycard_core::image::WorkingImage;
use greycard_edit::{Edit, WhiteShift};
use greycard_render::detail::DetailPlanes;
use greycard_render::export;
use greycard_render::finish::{Guide, Source};

use crate::walk::{Buf, Rasters, Walked};

/// What either finish reads beside the picture.
#[derive(Clone)]
pub struct FinishInput {
    /// The developed picture's size, before its geometry.
    pub source: (u32, u32),
    /// The full develop's size, before any bin.
    pub full: (u32, u32),
    pub guide: Option<Arc<Guide>>,
    pub planes: Option<Arc<DetailPlanes>>,
    pub rasters: Rasters,
    pub white: Option<WhiteShift>,
    pub clip_level: f32,
    pub kind: Source,
}

impl Walked {
    /// What the finish reads of this walk.
    pub fn finish_input(&self) -> FinishInput {
        let source = match &self.buf {
            Buf::Working(image) => (image.width as u32, image.height as u32),
            Buf::Device(image) => (image.width(), image.height()),
            Buf::Texture(texture) => (texture.width(), texture.height()),
            _ => (0, 0),
        };
        FinishInput {
            source,
            full: self.meta.full,
            guide: self.taps.guide.clone(),
            planes: self.taps.planes.clone(),
            rasters: self.rasters.clone(),
            white: self.meta.white_shift(),
            clip_level: self.meta.clip_level,
            kind: self.meta.source,
        }
    }
}

/// `image` finished on the CPU under `edit` and the sheet's
/// `settings`, as an export is: the edit's geometry, then the render
/// crate's finish, with the masks' bands taken through the same
/// geometry when they are the picture's size. No border and no mark.
pub fn render(
    image: &WorkingImage,
    edit: &Edit,
    input: &FinishInput,
    settings: &export::Settings,
) -> export::Rendered {
    let framed;
    let picture: &WorkingImage = if edit.geometry.is_identity() {
        image
    } else {
        framed = greycard_render::geometry::apply(image, &edit.geometry);
        &framed
    };
    let planes = input
        .planes
        .as_ref()
        .filter(|p| (p.width as u32, p.height as u32) == input.source);
    let detail = planes.map(|p| {
        let bands = p.to_image();
        if edit.geometry.is_identity() {
            bands
        } else {
            greycard_render::geometry::apply(&bands, &edit.geometry)
        }
    });
    export::render(
        picture,
        edit,
        &greycard_edit_io::Stores::user(),
        input.source,
        settings,
        &input.rasters,
        input.clip_level,
        input.guide.as_deref(),
        input.kind,
        input.white.as_ref(),
        detail.as_ref().zip(planes.map(|p| &p.meta)),
    )
}

/// [`render`], then the border and the mark: the file's pixels. A mark
/// that cannot be drawn fails the finish rather than let an unmarked
/// picture out.
pub fn finish_file(
    image: &WorkingImage,
    edit: &Edit,
    input: &FinishInput,
    settings: &export::Settings,
) -> anyhow::Result<export::Rendered> {
    let mut rendered = render(image, edit, input, settings);
    export::border(&mut rendered, settings);
    export::mark(&mut rendered, settings)?;
    Ok(rendered)
}

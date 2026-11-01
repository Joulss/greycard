//! The sizes thumbnails are made at, which the window's grid asks
//! for and the thumbnail cache keeps.

/// The largest a thumbnail is ever made, whatever the cell asks.
/// The grid's last step shows a 360 px picture stretched by a factor of
/// 1.4, which is soft under a loupe and indistinguishable at arm's
/// length, and costs two fifths of what a 512 costs to hold: a
/// picture is kept twice over, once as the bytes the worker made
/// and once as the image the renderer uploaded.
pub const MAX_RENDER: u32 = 360;

/// The long edges a picture is actually made at: the strip's 170 and
/// the grid's cells rounded up to one of these, so the thumbnail
/// cache, which keeps a picture a size, holds a few sizes of a frame
/// rather than one for every size a timing happened to ask for. The
/// grid's smallest cells get a 128 where they asked for 96, which is
/// the most this costs.
pub const MADE: [u32; 4] = [128, 176, 256, MAX_RENDER];

/// The size from [`MADE`] a picture asked for at `asked` is made at:
/// the least that is no smaller, or `asked` itself past the last.
pub fn made_size(asked: u32) -> u32 {
    MADE.iter().copied().find(|&m| m >= asked).unwrap_or(asked)
}

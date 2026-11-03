//! The pull on a merge's kept range: what Highlights' negative travel
//! does to a frame that keeps range over display white, one copy for
//! the finish (`Kept` in `greycard-render`'s `finish.rs`) and Auto
//! ([`crate::auto`]). The viewport's shader carries the same
//! arithmetic.
//!
//! In region readings, stops over mid grey: what reads over a `knee`
//! is compressed toward it, `share` of an `excess` at the frame's
//! `top`, by `ramp(g - knee) / ramp(top - knee)`, where `ramp(u)` is
//! `u - KEPT_KNEE * (1 - e^(-u / KEPT_KNEE))`: a soft knee whose slope
//! rises from nothing to one.

/// The Highlights slider's negative travel, at whose end a merge's
/// kept range is pulled under display white in full.
pub const HIGHLIGHTS_TRAVEL: f32 = 2.0;

/// The soft knee of the pull, in stops: its slope rises from nothing
/// at its foot to most of its full value over about this much.
pub const KEPT_KNEE: f32 = 0.5;

/// The share of the pull a blended Highlights asks for: its negative
/// travel, held to the slider's.
#[inline]
pub fn share(highlights: f32) -> f32 {
    (-highlights / HIGHLIGHTS_TRAVEL).clamp(0.0, 1.0)
}

/// The soft knee: `u` less what the knee holds back.
#[inline]
pub fn ramp(u: f32) -> f32 {
    u - KEPT_KNEE * (1.0 - (-u / KEPT_KNEE).exp())
}

/// A region reading `g` with `share` of the pull on it, for a frame
/// whose top reads `top`, `excess` of it to take off at the full
/// travel, from a `knee`.
#[inline]
pub fn pull(g: f32, share: f32, knee: f32, top: f32, excess: f32) -> f32 {
    let u = g - knee;
    if u <= 0.0 {
        return g;
    }
    g - share * excess * ramp(u) / ramp(top - knee)
}

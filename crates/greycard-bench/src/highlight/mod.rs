//! The highlight reconstruction benchmark: ground truth by synthetic
//! clipping, scored where the input was clipped.
//!
//! A raw whose highlights the sensor did not clip is the truth. The
//! harness overexposes it by 1, 2 or 3 stops in raw space, before the
//! white balance ([`sim`]), so each channel clips at its own scene level,
//! develops the clipped frame with every method through the engine's
//! own pipeline (one demosaic for all, no CA correction, hot pixel
//! repair or denoise), develops the truth through the same stages twice
//! (nothing cut, and cut at the develop's ceiling as every method is:
//! what is lost, and what a method can do within today's engine), and
//! scores them only where the input was clipped ([`score`]), split by
//! regime (one, two or all three colors clipped in the cell) and by the
//! clipped region's size ([`clipmap`]). Oracles that borrow half the
//! answer from the truth say where the remaining error lives
//! ([`oracle`]); real exposure brackets add cases the simulation cannot
//! make ([`brackets`]); other developers' command-line tools are scored
//! the same way when they are installed ([`external`]).
//!
//! The binary is `greycard-highlight-bench`:
//!
//! ```text
//! greycard-highlight-bench survey DIR...            # one CSV line a raw: clipping shares
//! greycard-highlight-bench run --frames LIST --out DIR [--brackets DIR] [--external]
//! greycard-highlight-bench external --frames LIST --frame f01 --stops 2 --out DIR [--dry-run]
//! ```
//!
//! The frame list is one raw a line, optionally a tab and a word for what
//! it shows; frames are named `f01`, `f02`… in its order everywhere the
//! harness writes, and only `frames.tsv` in the output maps them back to
//! files. `run` writes `rows.csv` (every case, method, regime and size),
//! `summary.csv` and `summary.md` (pooled), and `sheets/index.html`, the
//! picture sheets of the most telling cases.
//!
//! The same pieces make training data for a learned reconstruction:
//! [`sim::lowered`] and [`sim::prepare_scaled`] give a clipped input and
//! its truth as white-balanced mosaics of one frame, [`clipmap::ClipMap`]
//! the mask a loss is taken over.

pub mod brackets;
pub mod clipmap;
pub mod external;
pub mod oracle;
pub mod report;
pub mod run;
pub mod score;
pub mod sheet;
pub mod sim;

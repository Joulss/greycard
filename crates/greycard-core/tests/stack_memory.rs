//! What a focus stack read in passes holds: the same at three frames
//! and at twelve, counted by the allocator rather than read off the
//! process's resident set. A binary of its own, since the count is the
//! whole process's and one test at a time is the only way to read it.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use greycard_core::CameraImage;
use greycard_core::error::Error;
use greycard_core::stack::{self, Options};

/// Every byte the process has live, and the most it has had since the
/// mark was last reset.
struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller's contract, passed on.
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            let now = LIVE.fetch_add(layout.size(), Ordering::SeqCst) + layout.size();
            PEAK.fetch_max(now, Ordering::SeqCst);
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller's contract, passed on.
        unsafe { System.dealloc(ptr, layout) };
        LIVE.fetch_sub(layout.size(), Ordering::SeqCst);
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller's contract, passed on.
        let p = unsafe { System.alloc_zeroed(layout) };
        if !p.is_null() {
            let now = LIVE.fetch_add(layout.size(), Ordering::SeqCst) + layout.size();
            PEAK.fetch_max(now, Ordering::SeqCst);
        }
        p
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller's contract, passed on.
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            if new_size >= layout.size() {
                let grown = new_size - layout.size();
                let now = LIVE.fetch_add(grown, Ordering::SeqCst) + grown;
                PEAK.fetch_max(now, Ordering::SeqCst);
            } else {
                LIVE.fetch_sub(layout.size() - new_size, Ordering::SeqCst);
            }
        }
        p
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

/// The most `f` had live at once over what was live before it.
fn peak_of<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let before = LIVE.load(Ordering::SeqCst);
    PEAK.store(before, Ordering::SeqCst);
    let out = f();
    (out, PEAK.load(Ordering::SeqCst) - before)
}

/// A value from the lattice point `(ix, iy)`, 0 to 1.
fn lattice(ix: i64, iy: i64) -> f32 {
    let mut h = (ix as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((iy as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F));
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 32;
    (h >> 40) as f32 / (1u64 << 24) as f32
}

/// Value noise on a lattice of `cell` pixels.
fn noise(x: f32, y: f32, cell: f32) -> f32 {
    let (gx, gy) = (x / cell, y / cell);
    let (i, j) = (gx.floor(), gy.floor());
    let (fx, fy) = (gx - i, gy - j);
    let (i, j) = (i as i64, j as i64);
    let a = lattice(i, j) + fx * (lattice(i + 1, j) - lattice(i, j));
    let b = lattice(i, j + 1) + fx * (lattice(i + 1, j + 1) - lattice(i, j + 1));
    a + fy * (b - a)
}

/// Frame `k` of a stack: one texture, each frame shifted a little
/// further, so every link is a real fit and every frame but the
/// reference is warped.
fn frame(width: usize, height: usize, k: usize) -> CameraImage {
    let (dx, dy) = (0.7 * k as f32, -0.4 * k as f32);
    let mut data = Vec::with_capacity(width * height * 3);
    for y in 0..height {
        for x in 0..width {
            let (u, v) = (x as f32 + dx, y as f32 + dy);
            let g = 0.1 + 0.5 * noise(u, v, 24.0) + 0.3 * noise(u, v, 5.0);
            data.extend_from_slice(&[g * 0.6, g, g * 0.8]);
        }
    }
    CameraImage::from_data(width, height, data).expect("a frame")
}

fn streamed(width: usize, height: usize, n: usize) -> usize {
    let (merged, peak) = peak_of(|| {
        stack::stack_camera_from(
            n,
            |k| Ok::<_, Error>(frame(width, height, k)),
            |_| {},
            &Options::default(),
        )
        .expect("a stack")
    });
    assert_eq!(merged.1.used(), n, "{:#?}", merged.1.frames);
    peak
}

fn held(width: usize, height: usize, n: usize) -> usize {
    let (_, peak) = peak_of(|| {
        let frames: Vec<CameraImage> = (0..n).map(|k| frame(width, height, k)).collect();
        stack::stack_camera(&frames, &Options::default()).expect("a stack")
    });
    peak
}

#[test]
fn a_stack_read_in_passes_holds_the_same_at_any_count() {
    let (width, height) = (320usize, 240);
    let pixels = width * height;
    // The pool's threads and their first allocations, out of the way.
    let _ = streamed(width, height, 2);

    let (three, twelve) = (streamed(width, height, 3), streamed(width, height, 12));
    let (held_three, held_twelve) = (held(width, height, 3), held(width, height, 12));
    let per_pixel = |b: usize| b as f64 / pixels as f64;
    println!(
        "read in passes: 3 frames {:.1} B/px, 12 frames {:.1} B/px; held: 3 frames {:.1} \
         B/px, 12 frames {:.1} B/px",
        per_pixel(three),
        per_pixel(twelve),
        per_pixel(held_three),
        per_pixel(held_twelve),
    );
    // Nine more frames held are nine more frames of 12 bytes a pixel:
    // the count sees them.
    assert!(
        held_twelve >= held_three + 9 * 12 * pixels,
        "{held_three} {held_twelve}"
    );
    // Read in passes, nine more frames are nine more reports and
    // nothing a pixel: under a byte a pixel in all, at this small size,
    // where a frame is 12.
    assert!(
        twelve <= three + pixels,
        "three frames {three} bytes, twelve {twelve}"
    );

    // The peak a pixel at a size whose pyramid has the depth a real
    // frame's has, for `greycard_engine::merge::focus::BYTES_A_PIXEL`.
    let (width, height) = (1536usize, 1024);
    let big = streamed(width, height, 4);
    println!(
        "read in passes at {width}x{height}: {:.1} B/px",
        big as f64 / (width * height) as f64
    );
    assert!(big < 100 * width * height, "{big}");
}

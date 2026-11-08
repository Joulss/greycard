//! What a learned mask's model takes of the card at its peak, as the
//! editor's guard reads the card: `mask_peak [IMAGE] [MODEL...]` with
//! `GREYCARD_MODELS` set to a store root.
//!
//! MODEL is `sky`, `subject`, `sam`, or `sam3` (the People model); with
//! none named, each the store has. Each is loaded on WebGPU and run
//! twice on IMAGE (any picture the `image` crate reads; without one, a
//! made-up 2048 × 1365 picture, the size of the editor's preview: the
//! models squash it to their own square, so its content does not move
//! the figure), then dropped. The card's free memory is read as the
//! guard reads it (`runtime::webgpu_free_memory`: Vulkan's or DXGI's
//! budget less this process's use) before the load, every 5 ms in a
//! thread through the load and the runs, with the session still held
//! after them, and after the drop. The peak is the figure before less
//! the least seen. The models run in turn in one process with nothing
//! held between them; one a process (`mask_peak IMAGE sky`, then
//! `mask_peak IMAGE subject`) leaves ONNX Runtime's WebGPU context out
//! of it. The store's `providers.json` is written as the editor writes
//! it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use greycard_ai::runtime::{self, webgpu_free_memory};
use greycard_ai::{Provider, Rgb8, SAM, SAM3, SKY, Sam, Sam3, Sky, Store, Subject};

const GIB: f64 = (1u64 << 30) as f64;

fn free() -> Option<u64> {
    webgpu_free_memory().map(|(b, _)| b)
}

fn gib(b: u64) -> f64 {
    b as f64 / GIB
}

/// A picture the size of the editor's preview, with edges and smooth
/// ground for the models to find something in.
fn made_up() -> Rgb8 {
    let (w, h) = (2048usize, 1365usize);
    let mut data = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for x in 0..w {
            let sky = y < h / 2 - (x * 80 / w);
            let disc = {
                let (dx, dy) = (x as f32 - 1300.0, y as f32 - 900.0);
                dx * dx + dy * dy < 260.0 * 260.0
            };
            let px = if disc {
                [180, 120, 90]
            } else if sky {
                [90 + (y * 60 / h) as u8, 140 + (y * 60 / h) as u8, 230]
            } else {
                [60, 110 - (y * 40 / h) as u8, 50]
            };
            data.extend_from_slice(&px);
        }
    }
    Rgb8::new(w, h, data)
}

/// The least free figure seen while `run` runs, read every 5 ms.
fn least_while<T>(run: impl FnOnce() -> T) -> (T, Option<u64>) {
    let least = Arc::new(AtomicU64::new(u64::MAX));
    let stop = Arc::new(AtomicBool::new(false));
    let sampler = {
        let (least, stop) = (least.clone(), stop.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                if let Some(b) = free() {
                    least.fetch_min(b, Ordering::Relaxed);
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        })
    };
    let out = run();
    if let Some(b) = free() {
        least.fetch_min(b, Ordering::Relaxed);
    }
    stop.store(true, Ordering::Relaxed);
    let _ = sampler.join();
    let l = least.load(Ordering::Relaxed);
    (out, (l != u64::MAX).then_some(l))
}

/// Let the driver settle after a drop, and the heap go back.
fn settle() {
    runtime::trim_heap();
    std::thread::sleep(Duration::from_millis(500));
}

/// One model loaded and run twice, held while `held` is read: a
/// session kept alive by the closure's return.
type Run = Box<dyn FnOnce(&Store, &Rgb8) -> Result<(Box<dyn std::any::Any>, String), String>>;

fn model(name: &str) -> Option<Run> {
    let on = [Provider::WebGpu];
    Some(match name {
        "sky" => Box::new(move |store: &Store, rgb: &Rgb8| {
            let mut m = Sky::load(store, &on).map_err(|e| e.to_string())?;
            for _ in 0..2 {
                m.scene(rgb).map_err(|e| e.to_string())?;
            }
            let p = m.provider().label().to_string();
            Ok((Box::new(m) as Box<dyn std::any::Any>, p))
        }),
        "subject" => Box::new(move |store: &Store, rgb: &Rgb8| {
            let mut m = Subject::load(store, &on).map_err(|e| e.to_string())?;
            for _ in 0..2 {
                m.mask(rgb).map_err(|e| e.to_string())?;
            }
            let p = format!("{} ({})", m.provider().label(), m.model().id);
            Ok((Box::new(m) as Box<dyn std::any::Any>, p))
        }),
        "sam" => Box::new(move |store: &Store, rgb: &Rgb8| {
            let mut m = Sam::load(store, &on).map_err(|e| e.to_string())?;
            for _ in 0..2 {
                m.embed(rgb).map_err(|e| e.to_string())?;
            }
            let p = m.providers().0.label().to_string();
            Ok((Box::new(m) as Box<dyn std::any::Any>, p))
        }),
        "sam3" => Box::new(move |store: &Store, rgb: &Rgb8| {
            let mut m = Sam3::load(store, &on).map_err(|e| e.to_string())?;
            for _ in 0..2 {
                m.encode(rgb).map_err(|e| e.to_string())?;
            }
            let p = m.providers().0.label().to_string();
            Ok((Box::new(m) as Box<dyn std::any::Any>, p))
        }),
        _ => return None,
    })
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let store = Store::at(std::env::var_os("GREYCARD_MODELS").expect("GREYCARD_MODELS"));
    let known = ["sky", "subject", "sam", "sam3"];
    let (picture, mut names): (Option<&String>, Vec<&str>) = match a.first() {
        Some(first) if !known.contains(&first.as_str()) => {
            (Some(first), a[1..].iter().map(String::as_str).collect())
        }
        _ => (None, a.iter().map(String::as_str).collect()),
    };
    if names.is_empty() {
        let have = |n: &str| match n {
            "sky" => store.have(&SKY),
            "subject" => {
                store.have(&greycard_ai::SUBJECT) || store.have(&greycard_ai::SUBJECT_WEBGPU)
            }
            "sam" => store.have(&SAM),
            _ => store.have(&SAM3),
        };
        names = known.into_iter().filter(|n| have(n)).collect();
    }
    let rgb = match picture {
        Some(path) => {
            let img = image::open(path).expect("the picture").to_rgb8();
            Rgb8::new(img.width() as usize, img.height() as usize, img.into_raw())
        }
        None => made_up(),
    };
    if let Err(why) = runtime::status() {
        eprintln!("ONNX Runtime is not to be had here: {why}");
        std::process::exit(1);
    }
    let card = runtime::webgpu_name().unwrap_or_else(|| "the card".into());
    let Some(start) = free() else {
        eprintln!(
            "{card}: its free memory is not known (unified memory, a CPU adapter, or no budget)"
        );
        std::process::exit(1);
    };
    println!(
        "{card}: {:.2} GiB free at the start; a {}x{} picture",
        gib(start),
        rgb.width,
        rgb.height
    );
    let mut most = 0u64;
    for name in names {
        let Some(run) = model(name) else {
            eprintln!("{name}: not a model this measures ({})", known.join(", "));
            continue;
        };
        settle();
        let before = free().unwrap_or(0);
        let t = Instant::now();
        let (made, least) = least_while(|| run(&store, &rgb));
        let seconds = t.elapsed().as_secs_f64();
        let (session, provider) = match made {
            Ok(m) => m,
            Err(e) => {
                println!("{name}: failed: {e}");
                continue;
            }
        };
        let held = free().unwrap_or(0);
        drop(session);
        settle();
        let after = free().unwrap_or(0);
        let least = least.unwrap_or(before);
        let peak = before.saturating_sub(least);
        most = most.max(peak);
        println!(
            "{name} on {provider}: {:.2} GiB free before, {:.2} at the least, {:.2} held after two runs, \
             {:.2} after the drop; peak {:.2} GiB, held {:.2} GiB; {seconds:.1} s",
            gib(before),
            gib(least),
            gib(held),
            gib(after),
            gib(peak),
            gib(before.saturating_sub(held)),
        );
    }
    let tenths = (most as f64 / (GIB / 10.0)).ceil() as u64;
    println!(
        "the largest peak {:.2} GiB: MASK_PEAK at {tenths} tenths of a GiB would cover it",
        gib(most)
    );
}

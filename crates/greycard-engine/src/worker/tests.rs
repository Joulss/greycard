use super::*;
use crate::stages::Keep;

#[test]
fn the_strip_s_own_frames_are_made_first_then_the_ones_beside_them() {
    let mut pending: Vec<(usize, PathBuf)> = (0..10)
        .map(|i| (i, PathBuf::from(format!("{i}"))))
        .collect();
    order_thumbnails(&mut pending, 4, 6);
    let order: Vec<usize> = pending.iter().map(|(i, _)| *i).collect();
    // The range in file order, then outward from its middle, 5.
    assert_eq!(order, vec![4, 5, 6, 3, 7, 2, 8, 1, 9, 0]);
}

/// Run a set of `names` (files of junk, which nothing decodes)
/// through a worker of its own, cancelling the set as frame
/// `cancel_at` is begun, and asking for an open (of junk, generation
/// 7) as frame `open_at` is; what the worker said, in order, in
/// short.
fn run_fake_set(
    tag: &str,
    names: &[&str],
    cancel_at: Option<usize>,
    open_at: Option<usize>,
) -> Vec<String> {
    let dir = thumb_scratch(tag);
    let junk = dir.join("junk.CR3");
    std::fs::write(&junk, vec![0x33u8; 4096]).unwrap();
    // The worker's queue, for the deliver to put a job on as a
    // window would, mid-set.
    type Shared = Arc<(Mutex<Queue>, Condvar)>;
    let slot: Arc<std::sync::OnceLock<Shared>> = Arc::new(std::sync::OnceLock::new());
    let seen = slot.clone();
    let frames: Vec<crate::queue::Frame> = names
        .iter()
        .map(|n| {
            let source = dir.join(n);
            std::fs::write(&source, vec![0x5au8; 4096]).unwrap();
            crate::queue::Frame {
                source: source.clone(),
                edit: Edit::default(),
                version: None,
                turn: 0,
                time_shift: 0,
                seed_blend: false,
                moved_from: None,
                out: dir.join("out").join(format!("{n}.jpg")),
            }
        })
        .collect();
    let set = Arc::new(crate::queue::Set::new(
        frames.len(),
        Some(dir.join("out")),
        greycard_render::export::Settings::default(),
        greycard_render::export::OnExists::Increment,
    ));
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = Mutex::new(tx);
    let worker = Worker::new(move |o| {
        let line = match o {
            Outcome::SetFrameStarted { set, index, name } => {
                // Pressed while this frame is in hand: the deliver
                // runs on the worker's thread, before the work.
                if Some(index) == cancel_at {
                    set.cancel();
                }
                if Some(index) == open_at {
                    let (lock, cv) = &**seen.get().expect("the queue is handed over");
                    lock.lock().unwrap().develop = Some(Job::Open {
                        path: junk.clone(),
                        version: None,
                        edit: Edit::default(),
                        generation: 7,
                        seed_blend: false,
                        turn: 0,
                        stance_known: false,
                    });
                    cv.notify_one();
                }
                format!("begin {index} {name}")
            }
            Outcome::Failed { generation, .. } => format!("develop {generation} failed"),
            Outcome::SetFrameDone { index, done, .. } => match done {
                crate::queue::Done::Failed { message } => {
                    assert!(!message.is_empty());
                    format!("failed {index}")
                }
                crate::queue::Done::Canceled => format!("canceled {index}"),
                other => format!("{other:?} {index}"),
            },
            Outcome::SetDone { tally, .. } => format!(
                "done {} exported, {} failed, {} canceled",
                tally.exported,
                tally.failed.len(),
                tally.canceled
            ),
            _ => return,
        };
        tx.lock().unwrap().send(line).unwrap();
    });
    slot.set(worker.queue.clone()).ok().unwrap();
    worker.send(Job::ExportSet {
        set: set.clone(),
        frames,
    });
    let mut said = Vec::new();
    loop {
        let line = rx
            .recv_timeout(std::time::Duration::from_secs(30))
            .expect("the worker answers");
        let done = line.starts_with("done");
        said.push(line);
        if done {
            break;
        }
    }
    worker.stop();
    assert!(!dir.join("out").exists(), "nothing was written");
    crate::testing::remove_dir_retry(&dir);
    said
}

/// A set whose every frame fails: each is begun in order and is
/// one failure, the rest of the set goes on regardless, and the
/// set is said done once, after its last, with the count.
#[test]
fn a_set_runs_in_order_and_counts_its_failures() {
    let said = run_fake_set("setfail", &["A.CR3", "B.CR3", "C.CR3"], None, None);
    assert_eq!(
        said,
        vec![
            "begin 0 A.CR3.jpg",
            "failed 0",
            "begin 1 B.CR3.jpg",
            "failed 1",
            "begin 2 C.CR3.jpg",
            "failed 2",
            "done 0 exported, 3 failed, 0 canceled",
        ]
    );
}

/// Canceled while the second of four is in hand: that one is
/// finished, the last two are passed over without being begun.
#[test]
fn a_cancelled_set_finishes_the_frame_in_hand_and_stops() {
    let said = run_fake_set(
        "setcancel",
        &["A.CR3", "B.CR3", "C.CR3", "D.CR3"],
        Some(1),
        None,
    );
    assert_eq!(
        said,
        vec![
            "begin 0 A.CR3.jpg",
            "failed 0",
            "begin 1 B.CR3.jpg",
            "failed 1",
            "canceled 2",
            "canceled 3",
            "done 0 exported, 2 failed, 2 canceled",
        ]
    );
}

/// A stop that finds the worker still in a job says so and keeps
/// its thread, so the quit can tell it is there and not run the C
/// library's exit under it; once out of the job the worker sees
/// the stop, ends, and is joined. The job held here is an open of
/// junk whose failure the deliver sits on, on the worker's thread,
/// as a develop sits in the driver.
#[test]
fn a_stop_that_finds_the_worker_in_a_job_keeps_it_until_it_ends() {
    let dir = thumb_scratch("busystop");
    let junk = dir.join("junk.CR3");
    std::fs::write(&junk, vec![0x33u8; 4096]).unwrap();
    let (entered, in_job) = std::sync::mpsc::channel();
    let (release, released) = std::sync::mpsc::channel::<()>();
    let (entered, released) = (Mutex::new(entered), Mutex::new(released));
    let worker = Worker::new(move |o| {
        if let Outcome::Failed { .. } = o {
            entered.lock().unwrap().send(()).unwrap();
            let _ = released.lock().unwrap().recv();
        }
    });
    assert!(!worker.finished(), "a worker not asked to stop is there");
    worker.send(Job::Open {
        path: junk,
        version: None,
        edit: Edit::default(),
        generation: 1,
        seed_blend: false,
        turn: 0,
        stance_known: false,
    });
    in_job
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("the worker takes the open");
    assert!(
        !worker.stop_within(std::time::Duration::from_millis(50)),
        "the stop finds it in its job"
    );
    assert!(worker.running(), "its thread is kept");
    assert!(!worker.finished());
    release.send(()).unwrap();
    let asked = Instant::now();
    while !worker.finished() {
        assert!(
            asked.elapsed() < std::time::Duration::from_secs(30),
            "out of its job, the worker ends"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(!worker.running());
    assert!(worker.stop(), "a stop after the end has nothing to wait on");
    crate::testing::remove_dir_retry(&dir);
}

/// A stop that finds the worker between jobs joins it at once.
#[test]
fn a_stop_between_jobs_joins_the_worker() {
    let worker = Worker::new(|_| {});
    assert!(worker.stop());
    assert!(worker.finished());
    assert!(!worker.running());
}

/// A develop asked for while a frame of a set is in hand goes
/// before the set's next frame: the set waits behind the window.
#[test]
fn a_develop_asked_for_mid_set_goes_before_the_next_frame() {
    let said = run_fake_set("setdevelop", &["A.CR3", "B.CR3", "C.CR3"], None, Some(0));
    assert_eq!(
        said,
        vec![
            "begin 0 A.CR3.jpg",
            "failed 0",
            "develop 7 failed",
            "begin 1 B.CR3.jpg",
            "failed 1",
            "begin 2 C.CR3.jpg",
            "failed 2",
            "done 0 exported, 3 failed, 0 canceled",
        ]
    );
}

#[test]
fn a_panic_in_a_job_is_the_failure_that_job_was_waited_on_for() {
    let develop = Job::Develop {
        edit: Edit::default(),
        generation: 7,
        turn: 0,
    };
    // The panics here are on purpose; the hook has nothing to say
    // about them. The hook is the process's, and the other tests
    // run beside this one: theirs still print, or a failure
    // elsewhere in the run would be a bare FAILED with no words.
    let hook = std::panic::take_hook();
    let mine = std::thread::current().id();
    let hushed: std::sync::Arc<std::sync::Mutex<Option<_>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Some(hook)));
    let shared = hushed.clone();
    std::panic::set_hook(Box::new(move |info| {
        if std::thread::current().id() != mine
            && let Ok(guard) = shared.lock()
            && let Some(hook) = guard.as_ref()
        {
            hook(info);
        }
    }));
    let payload = std::panic::catch_unwind(|| panic!("on purpose {}", 1)).unwrap_err();
    let plain = std::panic::catch_unwind(|| panic!("plain")).unwrap_err();
    let _ = std::panic::take_hook();
    if let Some(hook) = hushed.lock().unwrap().take() {
        std::panic::set_hook(hook);
    }
    let message = panic_message(payload.as_ref());
    assert_eq!(message, "on purpose 1");
    match Blame::of(&develop).outcome(message) {
        Some(Outcome::Failed {
            generation,
            message,
        }) => {
            assert_eq!(generation, 7);
            assert!(message.contains("panicked: on purpose 1"), "{message}");
        }
        _ => panic!("not the develop's failure"),
    }
    let mask = Job::Mask {
        key: (3, 1),
        shape: Shape::Linear {
            from: [0.0, 0.0],
            to: [1.0, 1.0],
        },
        model: None,
    };
    assert!(matches!(
        Blame::of(&mask).outcome("x".into()),
        Some(Outcome::MaskFailed { key: (3, 1), .. })
    ));
    let thumbnail = Job::Thumbnail {
        index: 0,
        frame: Frame::file("a"),
    };
    assert_eq!(Blame::of(&thumbnail), Blame::Nobody);
    assert!(Blame::Nobody.outcome("x".into()).is_none());
    assert_eq!(panic_message(plain.as_ref()), "plain");
}

/// A Bayer frame of a textured scene with lateral CA in it, large
/// enough for the correction to run, in sensor units.
pub(super) fn aberrated_frame(width: usize, height: usize) -> RawFrame {
    use greycard_core::raw::{
        Calibration, CfaColor, CfaPattern, LevelPattern, Levels, Samples, SensorLayout,
    };
    let pattern = CfaPattern::rggb();
    let texture = |x: f32, y: f32| {
        let v = (x * 0.11).sin() * (y * 0.07).cos()
            + (x * 0.031 + y * 0.052).sin()
            + ((x * 0.9 + y * 0.37) * 0.23).sin() * 0.5
            + ((x - y) * 0.017).cos() * 0.7;
        0.5 + 0.3 * v / 3.2
    };
    let (cx, cy) = ((width - 1) as f32 / 2.0, (height - 1) as f32 / 2.0);
    let mut samples = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let (scale, gain) = match pattern.color_at(y, x) {
                CfaColor::Red => (1.005, 0.5),
                CfaColor::Blue => (0.996, 0.25),
                _ => (1.0, 1.0),
            };
            let sx = cx + (x as f32 - cx) / scale;
            let sy = cy + (y as f32 - cy) / scale;
            samples.push(100 + (texture(sx, sy) * gain * 1000.0) as u16);
        }
    }
    let m = greycard_core::color::WORKING_SPACE
        .from_xyz_matrix()
        .expect("the working space has a matrix");
    let color_matrix = m.rows.iter().flatten().map(|v| *v as f32).collect();
    RawFrame {
        make: "Test".into(),
        model: "Cam".into(),
        other_names: Vec::new(),
        width,
        height,
        channels: 1,
        layout: SensorLayout::Cfa(pattern),
        samples: Samples::U16(samples),
        levels: Levels {
            black: LevelPattern::uniform(100.0, 1),
            white: LevelPattern::uniform(1100.0, 1),
        },
        as_shot_coefficients: Some([2.0, 1.0, 4.0]),
        calibrations: vec![Calibration {
            illuminant: 21,
            color_matrix,
            forward_matrix: None,
        }],
        crop: None,
        orientation: greycard_core::raw::Orientation::Normal,
        shot: Default::default(),
        dng: Default::default(),
    }
}

/// `develop_job` on `frame` under the default edit with `gpu`,
/// from `base`: the outcome and the picture.
fn develop_once(
    frame: &Arc<RawFrame>,
    base: &mut Option<FrameKept>,
    gpu: &mut Option<greycard_gpu::Context>,
) -> (Outcome, Option<Arc<WorkingImage>>) {
    let deliver: Deliver = Arc::new(|_| {});
    develop_job(
        &Input::Raw(frame.clone()),
        &Edit::default(),
        0,
        1,
        base,
        &mut Ai::new(),
        None,
        None,
        gpu,
        &deliver,
    )
}

/// `develop_job` on `frame` under `edit` at the frame's `turn`,
/// on the CPU, from `base`: the picture.
fn develop_turned(
    frame: &Arc<RawFrame>,
    edit: &Edit,
    turn: u8,
    base: &mut Option<FrameKept>,
) -> Arc<WorkingImage> {
    let deliver: Deliver = Arc::new(|_| {});
    let (outcome, image) = develop_job(
        &Input::Raw(frame.clone()),
        edit,
        turn,
        1,
        base,
        &mut Ai::new(),
        None,
        None,
        &mut None,
        &deliver,
    );
    assert!(matches!(outcome, Outcome::Developed { .. }));
    image.expect("a picture on the CPU")
}

/// A develop that finds its base at another turn turns it rather than
/// making it again, and the base, its guide plane, and the develop are
/// the ones a develop at the new turn makes, to the bit: for a frame
/// whose tag is a plain turn and one whose tag mirrors, for a quarter
/// each way and a half.
#[test]
fn a_base_turned_in_place_is_the_base_developed_at_that_turn() {
    use greycard_core::raw::Orientation;
    let edit = Edit::default();
    for tag in [Orientation::Normal, Orientation::FlipHorizontal] {
        let mut frame = aberrated_frame(120, 80);
        frame.orientation = tag;
        let frame = Arc::new(frame);
        for (from, to) in [(0u8, 1u8), (1, 0), (0, 3), (1, 3), (2, 0)] {
            let what = format!("{tag:?}, {from} to {to}");
            let mut base = None;
            develop_turned(&frame, &edit, from, &mut base);
            let made = base.as_ref().map(|b| b.stamp);
            let image = develop_turned(&frame, &edit, to, &mut base);
            let b = base.as_ref().expect("a base");
            assert_eq!(b.turn, to);
            assert_eq!(Some(b.stamp), made.map(|s| s + 1), "{what}");
            let mut fresh = None;
            let reference = develop_turned(&frame, &edit, to, &mut fresh);
            let fresh = fresh.expect("a base");
            assert_eq!(
                (b.image.width, b.image.height),
                (fresh.image.width, fresh.image.height),
                "{what}"
            );
            assert!(
                b.image.data == fresh.image.data,
                "{what}: the pixels differ"
            );
            assert_eq!(b.guide.data, fresh.guide.data, "{what}: the guide differs");
            assert!(image.data == reference.data, "{what}: the develop differs");
        }
    }
}

/// A context on a device of our own with `limits`, for driving
/// the GPU paths into their errors; none without an adapter.
fn context_with(limits: greycard_gpu::wgpu::Limits) -> Option<greycard_gpu::Context> {
    use greycard_gpu::wgpu;
    let instance = greycard_gpu::instance();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .ok()?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("worker test"),
        required_limits: limits,
        ..Default::default()
    }))
    .ok()?;
    greycard_gpu::Context::from_device(&device, &queue).ok()
}

/// A GPU test with no adapter says so and returns; under
/// `GREYCARD_REQUIRE_GPU` (CI's software adapter, lavapipe here)
/// that is a failure, as greycard-gpu's own tests have it.
pub(super) fn skipped(what: &str) {
    assert!(
        std::env::var_os("GREYCARD_REQUIRE_GPU").is_none_or(|v| v.is_empty()),
        "{what} has no GPU to run on and GREYCARD_REQUIRE_GPU is set"
    );
    eprintln!("SKIPPED: {what} has no GPU to run on");
    println!("SKIPPED: {what} has no GPU to run on");
}

/// The export's picture is the reference's: a base whose CA ran on
/// the GPU is not reused by a develop without one (the export's),
/// which makes the base afresh on the CPU, and that picture is the
/// one a session that never had a GPU makes, byte for byte.
#[test]
fn an_export_after_a_gpu_base_develops_the_reference_s() {
    let Some(ctx) = greycard_gpu::Context::own().ok() else {
        skipped("the export's base check");
        return;
    };
    let frame = Arc::new(aberrated_frame(400, 320));
    let mut gpu = Some(ctx);
    let mut base = None;
    let (outcome, _) = develop_once(&frame, &mut base, &mut gpu);
    assert!(matches!(outcome, Outcome::Developed { .. }));
    assert!(base.as_ref().is_some_and(|b| b.on_gpu));
    let stamp = base.as_ref().unwrap().stamp;
    // The export: no GPU, the same edit.
    let (outcome, exported) = develop_once(&frame, &mut base, &mut None);
    assert!(matches!(outcome, Outcome::Developed { .. }));
    let b = base.as_ref().unwrap();
    assert!(!b.on_gpu && b.stamp == stamp + 1, "a fresh base on the CPU");
    // And once made on the CPU the base is kept.
    let (_, again) = develop_once(&frame, &mut base, &mut None);
    assert_eq!(base.as_ref().unwrap().stamp, stamp + 1);
    // The reference's own develop, from nothing.
    let (_, reference) = develop_once(&frame, &mut None, &mut None);
    let (exported, again, reference) = (exported.unwrap(), again.unwrap(), reference.unwrap());
    assert_eq!(exported.data, reference.data);
    assert_eq!(again.data, reference.data);
}

/// The upload is of the very picture a run on the device starts from:
/// after a develop without the device (the open file's export) made
/// the base again with the CA on the CPU, the upload of the old one is
/// let go with it, and the window's next develop uploads the new
/// picture rather than run on the GPU's, whose steps are the same; one
/// that changes nothing before the upload keeps it.
#[test]
fn a_picture_made_again_is_uploaded_again() {
    let Some(ctx) = greycard_gpu::Context::own().ok() else {
        skipped("the upload's picture");
        return;
    };
    let frame = Arc::new(aberrated_frame(400, 320));
    let mut gpu = Some(ctx);
    let mut base = None;
    let upload = |base: &Option<FrameKept>| {
        base.as_ref()
            .and_then(|b| b.kept.upload())
            .expect("uploaded")
            .clone()
    };
    develop_once(&frame, &mut base, &mut gpu);
    let first = upload(&base);
    develop_once(&frame, &mut base, &mut None);
    assert!(
        base.as_ref().unwrap().kept.upload().is_none(),
        "let go with its picture"
    );
    develop_once(&frame, &mut base, &mut gpu);
    let again = upload(&base);
    assert!(!Arc::ptr_eq(&again, &first), "the CPU's picture uploaded");
    develop_once(&frame, &mut base, &mut gpu);
    assert!(Arc::ptr_eq(&upload(&base), &again), "kept");
}

/// A learned pair whose CA ran on the GPU serves a develop with a
/// GPU and not one without, which makes the pair again on the CPU;
/// a pair whose CA was the CPU's serves both. The turn and the
/// learned settings still have to match.
#[test]
fn a_gpu_ca_learned_pair_is_not_served_to_an_export() {
    use crate::stages::{Keep, Stage};
    let mut edit = Edit::default();
    edit.noise.learned = greycard_edit::Learned::Fast;
    let input = Input::Raw(Arc::new(aberrated_frame(64, 48)));
    let walked = || FrameKept::walked_with(&input.walked(), &edit, 0, &mut LearnedFake);
    let serves = |b: &FrameKept, edit: &Edit, turn: u8, lent: bool| {
        let (steps, plan) = window_steps(edit, &b.facts_at(turn), None, lent);
        b.kept.serves(&steps, &plan, Keep::Pair)
    };
    let mut gpu = walked();
    gpu.kept.as_if_on_device(|s| matches!(s, Stage::Ca(_)));
    assert!(serves(&gpu, &edit, 0, true));
    assert!(!serves(&gpu, &edit, 0, false), "the export makes it again");
    let cpu = walked();
    assert!(serves(&cpu, &edit, 0, true));
    assert!(serves(&cpu, &edit, 0, false));
    assert!(!serves(&cpu, &edit, 1, false), "another turn");
    let mut other = edit.clone();
    other.noise.learned = greycard_edit::Learned::Best;
    assert!(!serves(&cpu, &other, 0, false), "other learned settings");
}

/// Models whose network is the plain demosaic, for a learned pair
/// without a model.
struct LearnedFake;

impl crate::walk::Models for LearnedFake {
    fn can_denoise(
        &mut self,
        _: &'static greycard_ai::Model,
    ) -> Result<(), crate::walk::NoLearned> {
        Ok(())
    }

    fn denoise(
        &mut self,
        _: &'static greycard_ai::Model,
        _: &RawFrame,
        prepared: &greycard_core::develop::Prepared,
    ) -> Result<Vec<f32>, crate::walk::NoLearned> {
        greycard_core::develop::demosaic_prepared(
            prepared,
            &greycard_core::develop::DevelopSettings::default(),
        )
        .map(|(rgb, _, _)| rgb)
        .map_err(|e| crate::walk::NoLearned::Failed(e.to_string()))
    }

    fn fill(
        &mut self,
        _: &greycard_edit::retouch::Patch,
        _: &greycard_core::develop::retouch::Region,
        _: &WorkingImage,
    ) -> Option<Vec<f32>> {
        None
    }

    fn could_fill_more(&mut self, _: &greycard_edit::retouch::Retouch, _: &WorkingImage) -> bool {
        false
    }

    fn rasters(
        &mut self,
        _: crate::stages::Masks,
        _: &Arc<WorkingImage>,
        _: Option<crate::walk::Patched<'_>>,
        _: &crate::walk::Meta,
    ) -> crate::walk::Rasters {
        crate::walk::Rasters::new()
    }
}

/// The export, with no window and no panel, makes a mask's white
/// balance from its own develop: the base keeps the profile its
/// white was resolved through, and a mask over everything at that
/// same white moves nothing while one at another moves the picture
/// the way the matrix says.
#[test]
fn an_export_makes_a_masks_white_balance_from_its_own_develop() {
    let frame = Arc::new(aberrated_frame(200, 160));
    // Developed at 5000 K, inside the range a mask's white takes.
    let global = Edit {
        white_balance: greycard_edit::WhiteBalance::Custom {
            temperature: 5000.0,
            tint: 0.002,
        },
        ..Edit::default()
    };
    let mut base = None;
    let image = develop_turned(&frame, &global, 0, &mut base);
    assert!(
        base.as_ref().unwrap().white_shift().is_some(),
        "a raw's base keeps its profile"
    );
    let everywhere = |white: greycard_edit::LocalWhite| greycard_edit::Adjustment {
        id: 1,
        mask: greycard_edit::mask::Mask {
            components: vec![greycard_edit::mask::Component {
                shape: Shape::Linear {
                    from: [0.5, 10.0],
                    to: [0.5, 11.0],
                },
                ..Default::default()
            }],
            invert: false,
            ..Default::default()
        },
        look: greycard_edit::Look {
            white_balance: white,
            ..Default::default()
        },
        ..Default::default()
    };
    let settings = greycard_render::export::Settings {
        format: greycard_render::export::Format::Tiff,
        sharpen: greycard_render::export::Sharpen::Off,
        ..Default::default()
    };
    let mut ai = Ai::new();
    let mut render = |white| {
        let mut edit = global.clone();
        edit.adjustments.push(everywhere(white));
        let (r, _) = finish_export(image.clone(), &edit, base.as_ref(), &mut ai, &settings);
        let greycard_render::export::Pixels::Sixteen(p) = r.pixels else {
            panic!("a TIFF is sixteen bits")
        };
        p
    };
    let off = render(greycard_edit::LocalWhite::Off);
    let same = render(greycard_edit::LocalWhite::Absolute {
        temperature: 5000.0,
        tint: 0.002,
    });
    let worst = off.iter().zip(&same).map(|(a, b)| a.abs_diff(*b)).max();
    assert!(worst <= Some(2), "{worst:?}");
    let warm = render(greycard_edit::LocalWhite::Absolute {
        temperature: 2800.0,
        tint: 0.0,
    });
    // Warmer light named: the picture comes out bluer.
    let blue = |p: &[u16]| {
        p.chunks(3).map(|c| c[2] as f64 - c[0] as f64).sum::<f64>() / (p.len() / 3) as f64
    };
    assert!(
        blue(&warm) > blue(&off) + 500.0,
        "{} {}",
        blue(&warm),
        blue(&off)
    );
    // Switched off with that light kept, it is the global's picture
    // to the last bit.
    let kept = render(greycard_edit::LocalWhite::Kept {
        temperature: 2800.0,
        tint: 0.0,
    });
    assert!(kept == off, "a kept light reaches no pixel");
}

/// A mask whose white follows the global keeps its offset when the
/// global moves, and a mask that names a light keeps the light: at a
/// 6500 K global, 46 mired warmer light is the picture a 5000 K lamp
/// makes; at a 5000 K global the same offset is another light, about
/// 4060 K, and the lamp is the global's own. No offset is the global's
/// picture at either.
#[test]
fn a_masks_offset_follows_the_global_and_its_light_does_not() {
    use greycard_edit::LocalWhite;
    let frame = Arc::new(aberrated_frame(200, 160));
    let everywhere = |white: LocalWhite| greycard_edit::Adjustment {
        id: 1,
        mask: greycard_edit::mask::Mask {
            components: vec![greycard_edit::mask::Component {
                shape: Shape::Linear {
                    from: [0.5, 10.0],
                    to: [0.5, 11.0],
                },
                ..Default::default()
            }],
            invert: false,
            ..Default::default()
        },
        look: greycard_edit::Look {
            white_balance: white,
            ..Default::default()
        },
        ..Default::default()
    };
    let settings = greycard_render::export::Settings {
        format: greycard_render::export::Format::Tiff,
        sharpen: greycard_render::export::Sharpen::Off,
        ..Default::default()
    };
    let mut ai = Ai::new();
    let mut render = |temperature: f64, white: LocalWhite| {
        let global = Edit {
            white_balance: greycard_edit::WhiteBalance::Custom {
                temperature,
                tint: 0.002,
            },
            ..Edit::default()
        };
        let mut base = None;
        let image = develop_turned(&frame, &global, 0, &mut base);
        let mut edit = global.clone();
        edit.adjustments.push(everywhere(white));
        let (r, _) = finish_export(image, &edit, base.as_ref(), &mut ai, &settings);
        let greycard_render::export::Pixels::Sixteen(p) = r.pixels else {
            panic!("a TIFF is sixteen bits")
        };
        p
    };
    let worst = |a: &[u16], b: &[u16]| a.iter().zip(b).map(|(a, b)| a.abs_diff(*b)).max();
    let blue = |p: &[u16]| {
        p.chunks(3).map(|c| c[2] as f64 - c[0] as f64).sum::<f64>() / (p.len() / 3) as f64
    };
    let lamp = LocalWhite::Absolute {
        temperature: 5000.0,
        tint: 0.002,
    };
    // Toward the lower kelvin, so negative.
    let cooler = LocalWhite::Relative {
        mired: 1e6 / 6500.0 - 1e6 / 5000.0,
        tint: 0.0,
    };
    let none = LocalWhite::Relative {
        mired: 0.0,
        tint: 0.0,
    };
    for global in [5000.0, 6500.0] {
        let off = render(global, LocalWhite::Off);
        let d = worst(&off, &render(global, none));
        assert!(d <= Some(2), "{global} K: {d:?}");
    }
    // At 6500 K the offset names the lamp's light.
    let d = worst(&render(6500.0, cooler), &render(6500.0, lamp));
    assert!(d <= Some(2), "{d:?}");
    // The global moved to 5000 K: the lamp is the global's own, the
    // offset moved with it to about 4060 K, a warmer light, so a bluer
    // picture under it.
    let off = render(5000.0, LocalWhite::Off);
    let d = worst(&off, &render(5000.0, lamp));
    assert!(d <= Some(2), "{d:?}");
    let followed = render(5000.0, cooler);
    assert!(
        blue(&followed) > blue(&off) + 500.0,
        "{} {}",
        blue(&followed),
        blue(&off)
    );
}

/// A last develop whose CA ran on the GPU is not handed to an export:
/// it develops afresh on the CPU, and that picture is then reused.
#[test]
fn an_export_does_not_reuse_a_gpu_ca_develop() {
    let frame = Arc::new(aberrated_frame(400, 320));
    // The reference, from nothing.
    let (_, reference) = develop_once(&frame, &mut None, &mut None);
    let reference = reference.unwrap();
    // A base as a GPU session leaves it, and a last develop from it
    // that is plainly not the reference: a flat grey picture.
    let mut base = None;
    let (_, image) = develop_once(&frame, &mut base, &mut None);
    let image = image.unwrap();
    let b = base.as_mut().unwrap();
    b.on_gpu = true;
    b.kept
        .as_if_on_device(|s| matches!(s, crate::stages::Stage::Ca(_)));
    let wrong = Arc::new(
        WorkingImage::from_data(image.width, image.height, vec![0.5; image.data.len()]).unwrap(),
    );
    let mut last = Some(Last::made(
        Edit::default(),
        base.as_ref().unwrap(),
        wrong.clone(),
        &Outcome::Failed {
            generation: 0,
            message: String::new(),
        },
    ));
    let mut ai = Ai::new();
    let deliver: Deliver = Arc::new(|_| {});
    let input = Input::Raw(frame.clone());
    let export = |last: &mut Option<Last>, base: &mut Option<FrameKept>, ai: &mut Ai| {
        open_picture(
            &Edit::default(),
            0,
            Some(&input),
            last,
            base,
            ai,
            None,
            None,
            &deliver,
        )
        .unwrap()
        .0
    };
    let got = export(&mut last, &mut base, &mut ai);
    assert!(!Arc::ptr_eq(&got, &wrong), "the GPU develop was reused");
    assert_eq!(
        got.data, reference.data,
        "the export's picture is the reference's"
    );
    assert!(
        last.as_ref().is_some_and(|l| !l.on_gpu),
        "the last develop is the CPU's now"
    );
    assert!(base.as_ref().is_some_and(|b| !b.on_gpu));
    // Now that it is the CPU's, it is reused.
    let again = export(&mut last, &mut base, &mut ai);
    assert!(Arc::ptr_eq(&got, &again), "the CPU develop is kept");
}

/// A device error in the CA correction falls back to the reference
/// for that develop and drops the context for the session; a
/// picture the device cannot hold falls back for that op and keeps
/// it. A device allowed 20 workgroups a dimension cannot run the
/// 400x320's 26; one whose textures stop at 512 a side holds a
/// 500-wide picture but not the CA's padded green plane.
#[test]
fn a_gpu_error_in_the_ca_falls_back_and_an_unsupported_size_keeps_the_context() {
    use greycard_gpu::wgpu::Limits;
    let Some(ctx) = context_with(Limits {
        max_compute_workgroups_per_dimension: 20,
        ..Limits::default()
    }) else {
        skipped("the CA fallback check");
        return;
    };
    let frame = Arc::new(aberrated_frame(400, 320));
    let mut gpu = Some(ctx);
    let mut base = None;
    let (outcome, image) = develop_once(&frame, &mut base, &mut gpu);
    assert!(matches!(outcome, Outcome::Developed { .. }));
    assert!(gpu.is_none(), "the context is dropped after a device error");
    assert!(base.as_ref().is_some_and(|b| !b.on_gpu));
    let (_, reference) = develop_once(&frame, &mut None, &mut None);
    assert_eq!(image.unwrap().data, reference.unwrap().data);

    let Some(ctx) = context_with(Limits {
        max_texture_dimension_2d: 512,
        ..Limits::default()
    }) else {
        skipped("the CA unsupported-size check");
        return;
    };
    let frame = Arc::new(aberrated_frame(500, 300));
    let mut gpu = Some(ctx);
    let mut base = None;
    let (outcome, _) = develop_once(&frame, &mut base, &mut gpu);
    assert!(matches!(outcome, Outcome::Developed { .. }));
    assert!(
        gpu.is_some(),
        "the context is kept for a picture it cannot hold"
    );
    assert!(base.as_ref().is_some_and(|b| !b.on_gpu));
}

/// An edit whose lens section moves the picture without a database:
/// the manual distortion and the manual aberration.
fn lens_edit() -> Edit {
    let mut edit = Edit::default();
    edit.lens.enabled = true;
    edit.lens.manual = -0.05;
    edit.lens.ca_red = 0.003;
    edit.lens.ca_blue = -0.002;
    assert!(!edit.lens.is_identity(false));
    edit
}

/// The base the CPU makes of `frame` under `edit` with the lens
/// switched off: a picture for the lens alone to move.
fn picture_before_lens(frame: &Arc<RawFrame>, edit: &Edit) -> Arc<WorkingImage> {
    let mut before = edit.clone();
    before.lens.enabled = false;
    let mut base = None;
    develop_turned(frame, &before, 0, &mut base);
    base.expect("a base").image
}

/// `image` as a picture that is not a raw: no CA correction runs on
/// it, so only the lens can put a base of it on the GPU.
fn picture_input(image: &Arc<WorkingImage>) -> Input {
    Input::Picture {
        image: image.clone(),
        meta: Arc::new(greycard_core::picture::Picture {
            image: WorkingImage::new(0, 0),
            make: String::new(),
            model: String::new(),
            shot: Default::default(),
            metadata: Default::default(),
            space: "sRGB".into(),
            bits: 16,
        }),
    }
}

/// `develop_job` on `input` under `edit`: the picture, when it came
/// back on the CPU.
fn develop_picture(
    input: &Input,
    edit: &Edit,
    base: &mut Option<FrameKept>,
    gpu: &mut Option<greycard_gpu::Context>,
) -> Option<Arc<WorkingImage>> {
    let deliver: Deliver = Arc::new(|_| {});
    let (outcome, image) = develop_job(
        input,
        edit,
        0,
        1,
        base,
        &mut Ai::new(),
        None,
        None,
        gpu,
        &deliver,
    );
    assert!(matches!(outcome, Outcome::Developed { .. }));
    image
}

/// The base's lens on the GPU is the reference's within the GPU op's
/// own tolerance (greycard-gpu's lens tests say why 1e-4 of a value in
/// 0 to 1), and a base it ran in says so, so the export makes its own.
/// On a picture that is not a raw, which has no CA correction, so only
/// the lens can have marked it; the sharpen off, so the develop ends
/// on the CPU.
#[test]
fn a_base_with_the_gpu_lens_is_the_cpu_s() {
    let Some(ctx) = greycard_gpu::Context::own().ok() else {
        skipped("the GPU lens step");
        return;
    };
    let frame = Arc::new(aberrated_frame(400, 320));
    let mut edit = lens_edit();
    edit.sharpen.enabled = false;
    let before = picture_before_lens(&frame, &edit);
    let input = picture_input(&before);
    let develop = |base: &mut Option<FrameKept>, gpu: &mut Option<greycard_gpu::Context>| {
        develop_picture(&input, &edit, base, gpu)
    };
    let mut gpu = Some(ctx);
    let mut on_gpu = None;
    develop(&mut on_gpu, &mut gpu);
    let mut on_cpu = None;
    develop(&mut on_cpu, &mut None);
    let (g, c) = (on_gpu.as_ref().unwrap(), on_cpu.as_ref().unwrap());
    assert!(g.on_gpu && !c.on_gpu);
    let lens = |s: &crate::stages::Stage| matches!(s, crate::stages::Stage::Lens(_));
    assert!(g.kept.ran_on_device(crate::stages::Keep::Base, lens));
    assert_ne!(c.image.data, before.data, "the lens moved something");
    let scale = c.image.data.iter().fold(1f32, |m, v| m.max(v.abs()));
    let worst = c
        .image
        .data
        .iter()
        .zip(&g.image.data)
        .map(|(a, b)| (a - b).abs())
        .fold(0f32, f32::max);
    assert!(worst <= 1e-4 * scale, "{worst} apart, values up to {scale}");

    // The export's develop, which has no GPU, makes the reference's
    // afresh from the base the GPU marked.
    let exported = develop(&mut on_gpu, &mut None).expect("a picture on the CPU");
    assert!(on_gpu.as_ref().is_some_and(|b| !b.on_gpu));
    let reference = develop(&mut None, &mut None).expect("a picture on the CPU");
    assert_eq!(exported.data, reference.data);
}

/// A device error in the lens falls back to the reference for that
/// base, the base the CPU's to the bit and not marked, and the develop
/// drops the context for the session, as it does after the CA's. A
/// picture the device cannot hold falls back the same way and is not
/// an error: the context stays. A device allowed 20 workgroups a
/// dimension cannot run the 400x320's 25 across; one whose textures
/// stop at 256 a side cannot hold it. On a picture that is not a raw,
/// where only the lens would run on the device, with the sharpen off,
/// so that no op but the lens asks the device for anything; without
/// the lens the same develop keeps it.
#[test]
fn a_gpu_error_in_the_lens_falls_back_and_an_unsupported_size_keeps_the_context() {
    use greycard_gpu::wgpu::Limits;
    let frame = Arc::new(aberrated_frame(400, 320));
    let mut lensed = lens_edit();
    lensed.sharpen.enabled = false;
    let input = picture_input(&picture_before_lens(&frame, &lensed));
    let mut cpu = None;
    develop_picture(&input, &lensed, &mut cpu, &mut None);
    let cpu = cpu.expect("a base");
    for (limits, device_error) in [
        (
            Limits {
                max_compute_workgroups_per_dimension: 20,
                ..Limits::default()
            },
            true,
        ),
        (
            Limits {
                max_texture_dimension_2d: 256,
                ..Limits::default()
            },
            false,
        ),
    ] {
        let Some(ctx) = context_with(limits.clone()) else {
            skipped("the lens fallback check");
            return;
        };
        let mut quiet = Edit::default();
        quiet.sharpen.enabled = false;
        let mut gpu = Some(ctx);
        develop_picture(&input, &quiet, &mut None, &mut gpu);
        assert!(gpu.is_some(), "{limits:?}");
        let mut base = None;
        develop_picture(&input, &lensed, &mut base, &mut gpu);
        let b = base.expect("a base");
        assert!(!b.on_gpu, "{limits:?}");
        assert_eq!(b.image.data, cpu.image.data, "{limits:?}");
        assert_eq!(gpu.is_none(), device_error, "{limits:?}");
    }
}

/// The Detail section on the GPU: a move runs the local contrast
/// there from the patched picture's upload, which is kept for the
/// next; the dehaze runs there after it, from the same upload, its
/// reduced copy kept for a move of the Dehaze alone and made again
/// when the local contrast moves; with the sharpen off the last op
/// writes the viewport's texture itself; with neither op on,
/// nothing is kept. And each path keeps on the device only what it
/// uses: the local contrast's planes those its sliders need (none
/// when they are made for each run, as on an integrated GPU), none
/// with the Detail section off, and the sharpen's none with the
/// sharpen off.
#[test]
fn the_detail_section_runs_on_the_gpu_from_the_kept_upload() {
    for keep in [true, false] {
        let Some(ctx) = greycard_gpu::Context::own().ok() else {
            skipped("the Detail section's GPU path");
            return;
        };
        ctx.keep_local_contrast_planes(keep);
        detail_section_on_the_gpu(ctx, keep);
    }
}

fn detail_section_on_the_gpu(ctx: greycard_gpu::Context, keep: bool) {
    let frame = Arc::new(aberrated_frame(400, 320));
    let deliver: Deliver = Arc::new(|_| {});
    let mut gpu = Some(ctx);
    let mut base = None;
    let develop = |edit: &Edit, base: &mut Option<FrameKept>, gpu: &mut Option<_>| {
        develop_job(
            &Input::Raw(frame.clone()),
            edit,
            0,
            1,
            base,
            &mut Ai::new(),
            None,
            None,
            gpu,
            &deliver,
        )
    };
    let kept = |gpu: &Option<greycard_gpu::Context>| gpu.as_ref().unwrap().kept_bytes();
    let mut edit = Edit::default();
    edit.detail.clarity = 0.5;
    let (outcome, image) = develop(&edit, &mut base, &mut gpu);
    assert!(
        matches!(
            outcome,
            Outcome::Developed {
                image: Developed::Texture(_),
                detail: Some((_, DetailRan::Gpu)),
                sharpen: Some(_),
                ..
            }
        ),
        "the local contrast and the sharpen on the GPU"
    );
    assert!(image.is_none());
    let b = base.as_ref().unwrap();
    let uploaded = b
        .kept
        .upload()
        .expect("the patched picture is kept")
        .clone();
    assert!(b.kept.on_device(Keep::PreSharpen));
    // A plane of the picture's size; Clarity's exact filter at this
    // size takes the log, its scratch, and the slope and intercept.
    let plane = u64::from(uploaded.width()) * u64::from(uploaded.height()) * 4;
    let planes = |n: u64| if keep { n * plane } else { 0 };
    assert!(kept(&gpu).sharpen > 0);
    assert_eq!(kept(&gpu).local_contrast, planes(4), "Clarity's planes");
    // Another move keeps the upload and remakes the picture
    // before the sharpen from it.
    edit.detail.clarity = -0.5;
    let (outcome, _) = develop(&edit, &mut base, &mut gpu);
    assert!(matches!(
        outcome,
        Outcome::Developed {
            image: Developed::Texture(_),
            detail: Some((_, DetailRan::Gpu)),
            ..
        }
    ));
    let b = base.as_ref().unwrap();
    assert!(Arc::ptr_eq(b.kept.upload().unwrap(), &uploaded));
    assert!(b.kept.on_device(Keep::PreSharpen));
    // A sharpen move keeps the picture before it, and says so.
    edit.sharpen.iterations += 1;
    let (outcome, _) = develop(&edit, &mut base, &mut gpu);
    assert!(matches!(
        outcome,
        Outcome::Developed {
            detail: Some((_, DetailRan::Kept)),
            sharpen: Some(_),
            ..
        }
    ));
    // Texture as well: its gain beside them.
    edit.detail.texture = 0.5;
    develop(&edit, &mut base, &mut gpu);
    assert_eq!(kept(&gpu).local_contrast, planes(5), "Texture's planes too");
    // The Detail section off, the sharpen on: the sharpen reads the
    // upload itself, and the local contrast keeps nothing.
    edit.detail.texture = 0.0;
    edit.detail.clarity = 0.0;
    develop(&edit, &mut base, &mut gpu);
    let b = base.as_ref().unwrap();
    assert!(Arc::ptr_eq(b.kept.upload().unwrap(), &uploaded));
    assert_eq!(kept(&gpu).local_contrast, 0, "no Detail, no planes");
    // The dehaze after the local contrast, both on the GPU from the
    // kept upload; its reduced copy kept with what it was made from.
    edit.detail.clarity = -0.5;
    develop(&edit, &mut base, &mut gpu);
    assert_eq!(kept(&gpu).local_contrast, planes(4));
    assert!(!base.as_ref().unwrap().kept.has_haze());
    edit.detail.dehaze = 0.5;
    // The dehaze goes on from the local contrast's picture kept on
    // the device, which a slider after it left as it was.
    let (outcome, _) = develop(&edit, &mut base, &mut gpu);
    let Outcome::Developed {
        image: Developed::Texture(_),
        detail: Some((_, DetailRan::Kept)),
        dehaze: Some(first),
        ..
    } = outcome
    else {
        panic!("the dehaze on the GPU after the kept local contrast");
    };
    let b = base.as_ref().unwrap();
    assert!(Arc::ptr_eq(b.kept.upload().unwrap(), &uploaded));
    assert!(b.kept.on_device(Keep::PreSharpen));
    let reduced = b
        .kept
        .haze_copy()
        .expect("the reduced copy is kept")
        .clone();
    assert_eq!(kept(&gpu).local_contrast, planes(4));
    assert!(kept(&gpu).sharpen > 0);
    // A move of the Dehaze alone fits again from the kept copy, so
    // the airlight is the same.
    edit.detail.dehaze = 0.8;
    let (outcome, _) = develop(&edit, &mut base, &mut gpu);
    let Outcome::Developed {
        dehaze: Some(second),
        ..
    } = outcome
    else {
        panic!("a Dehaze move");
    };
    let b = base.as_ref().unwrap();
    assert!(Arc::ptr_eq(b.kept.haze_copy().unwrap(), &reduced));
    assert_eq!(second.airlight, first.airlight);
    assert!(second.strength > first.strength);
    // A sharpen move keeps the picture before it, dehaze and all.
    edit.sharpen.iterations += 1;
    let (outcome, _) = develop(&edit, &mut base, &mut gpu);
    assert!(matches!(
        outcome,
        Outcome::Developed {
            detail: Some((_, DetailRan::Kept)),
            dehaze: Some(_),
            ..
        }
    ));
    // A move of the local contrast makes the copy again.
    edit.detail.clarity = -0.3;
    develop(&edit, &mut base, &mut gpu);
    let b = base.as_ref().unwrap();
    assert!(!Arc::ptr_eq(b.kept.haze_copy().unwrap(), &reduced));
    // The dehaze alone: from the upload, the local contrast's planes
    // let go.
    edit.detail.clarity = 0.0;
    let (outcome, _) = develop(&edit, &mut base, &mut gpu);
    assert!(matches!(
        outcome,
        Outcome::Developed {
            image: Developed::Texture(_),
            detail: None,
            dehaze: Some(_),
            ..
        }
    ));
    assert!(base.as_ref().unwrap().kept.has_haze());
    assert_eq!(kept(&gpu).local_contrast, 0, "no local contrast, no planes");
    // With the sharpen off, the dehaze writes the viewport's
    // texture, after the local contrast or alone.
    edit.sharpen.enabled = false;
    for clarity in [0.4, 0.0] {
        edit.detail.clarity = clarity;
        let (outcome, image) = develop(&edit, &mut base, &mut gpu);
        assert!(
            matches!(
                outcome,
                Outcome::Developed {
                    image: Developed::Texture(_),
                    sharpen: None,
                    dehaze: Some(_),
                    ..
                }
            ),
            "the dehaze into the viewport's texture"
        );
        assert!(image.is_none());
        let b = base.as_ref().unwrap();
        assert!(!b.kept.on_device(Keep::PreSharpen) && b.kept.has_haze());
        assert_eq!(kept(&gpu).sharpen, 0);
    }
    edit.sharpen.enabled = true;
    edit.detail.clarity = -0.5;
    // Sharpen off, dehaze off: the op writes the viewport's texture
    // itself, and nothing waits for a sharpen: its planes go.
    edit.detail.dehaze = 0.0;
    edit.sharpen.enabled = false;
    let (outcome, image) = develop(&edit, &mut base, &mut gpu);
    assert!(
        matches!(
            outcome,
            Outcome::Developed {
                image: Developed::Texture(_),
                detail: Some((_, DetailRan::Gpu)),
                sharpen: None,
                ..
            }
        ),
        "the local contrast alone, on the GPU"
    );
    assert!(image.is_none());
    let b = base.as_ref().unwrap();
    assert!(b.kept.has_upload() && !b.kept.on_device(Keep::PreSharpen));
    assert_eq!(kept(&gpu).sharpen, 0, "the sharpen's planes are let go");
    assert_eq!(kept(&gpu).local_contrast, planes(4));
    // Neither op: the CPU's halves, and nothing kept on the device.
    edit.detail.clarity = 0.0;
    let (outcome, image) = develop(&edit, &mut base, &mut gpu);
    assert!(matches!(
        outcome,
        Outcome::Developed {
            image: Developed::Halves(_),
            detail: None,
            sharpen: None,
            ..
        }
    ));
    assert!(image.is_some());
    let b = base.as_ref().unwrap();
    assert!(!b.kept.has_upload() && !b.kept.on_device(Keep::PreSharpen));
    assert_eq!(kept(&gpu), greycard_gpu::KeptBytes::default());
    assert!(gpu.is_some(), "the context is kept throughout");
}

/// The Detail section on the GPU gives the picture the CPU's does:
/// the local contrast, then the dehaze, in the reference's order,
/// with the sharpen off so that the viewport's texture is the
/// Detail section's output alone. From one base made on the CPU,
/// so that the CA correction is the same on both. The two differ by
/// the viewport's half floats, the local contrast's rounding and
/// the dehaze's gain on it; the dehaze run before the local
/// contrast would be percents off.
#[test]
fn the_detail_section_on_the_gpu_is_the_cpus() {
    let Some(ctx) = greycard_gpu::Context::own().ok() else {
        skipped("the Detail section's GPU picture");
        return;
    };
    let frame = Arc::new(aberrated_frame(400, 320));
    let deliver: Deliver = Arc::new(|_| {});
    let mut gpu = Some(ctx);
    let mut base = None;
    let mut edit = Edit::default();
    edit.sharpen.enabled = false;
    for (texture, clarity, dehaze) in [(0.4, 0.6, 0.7), (0.0, 0.0, -0.8), (-0.5, 0.3, 1.0)] {
        edit.detail.texture = texture;
        edit.detail.clarity = clarity;
        edit.detail.dehaze = dehaze;
        let mut develop = |gpu: &mut Option<greycard_gpu::Context>| {
            develop_job(
                &Input::Raw(frame.clone()),
                &edit,
                0,
                1,
                &mut base,
                &mut Ai::new(),
                None,
                None,
                gpu,
                &deliver,
            )
        };
        let (cpu_outcome, cpu) = develop(&mut None);
        let (outcome, _) = develop(&mut gpu);
        let (
            Outcome::Developed {
                dehaze: Some(cpu_stats),
                ..
            },
            Outcome::Developed {
                image: Developed::Texture(out),
                dehaze: Some(gpu_stats),
                ..
            },
        ) = (cpu_outcome, outcome)
        else {
            panic!("both develops, the GPU's into a texture");
        };
        let cpu = cpu.unwrap();
        let viewport = read_viewport(gpu.as_ref().unwrap(), &out);
        let worst = cpu
            .data
            .iter()
            .zip(&viewport.data)
            .map(|(a, b)| (a - b).abs() / a.abs().max(0.05))
            .fold(0.0, f32::max);
        println!(
            "texture {texture}, clarity {clarity}, dehaze {dehaze}: worst {worst:.2e}; airlight {:?} / {:?}",
            cpu_stats.airlight, gpu_stats.airlight
        );
        // Two of a half float's ulps.
        assert!(
            worst < 2e-3,
            "the viewport is {worst:.2e} off the CPU's picture"
        );
        for c in 0..3 {
            assert!((cpu_stats.airlight[c] - gpu_stats.airlight[c]).abs() < 1e-4);
        }
        assert!((cpu_stats.transmission_mean - gpu_stats.transmission_mean).abs() < 1e-4);
    }
}

/// A GPU error in the dehaze is warned once and takes the session
/// to the CPU, as the other ops' do: here a device whose storage
/// buffers are too small for the dehaze's block sums, which its
/// error scope catches. The develop that met it is the CPU's, to
/// the bit.
#[test]
fn a_gpu_error_in_the_dehaze_takes_the_session_to_the_cpu() {
    let Some(ctx) = context_with(greycard_gpu::wgpu::Limits {
        max_storage_buffer_binding_size: 256,
        ..greycard_gpu::wgpu::Limits::default()
    }) else {
        skipped("the dehaze's device error");
        return;
    };
    let frame = Arc::new(aberrated_frame(400, 320));
    let deliver: Deliver = Arc::new(|_| {});
    let mut edit = Edit::default();
    edit.detail.dehaze = 0.6;
    for sharpen in [true, false] {
        edit.sharpen.enabled = sharpen;
        let mut gpu = Some(
            context_with(greycard_gpu::wgpu::Limits {
                max_storage_buffer_binding_size: 256,
                ..greycard_gpu::wgpu::Limits::default()
            })
            .unwrap(),
        );
        let mut base = None;
        let mut develop = |gpu: &mut Option<greycard_gpu::Context>| {
            develop_job(
                &Input::Raw(frame.clone()),
                &edit,
                0,
                1,
                &mut base,
                &mut Ai::new(),
                None,
                None,
                gpu,
                &deliver,
            )
        };
        let (_, cpu) = develop(&mut None);
        let (outcome, image) = develop(&mut gpu);
        assert!(gpu.is_none(), "the context is dropped after a device error");
        assert!(matches!(
            outcome,
            Outcome::Developed {
                image: Developed::Halves(_),
                dehaze: Some(_),
                ..
            }
        ));
        assert_eq!(image.unwrap().data, cpu.unwrap().data);
    }
    drop(ctx);
}

/// A picture larger than the device's textures, with a Detail
/// slider set, is the CPU's for that develop, with the sharpen off
/// and on, and the context is kept: the next picture that fits is
/// the GPU's again. No texture is made for it before the upload
/// has found it fits, so nothing reaches wgpu's uncaptured handler
/// (which panics the worker).
#[test]
fn a_picture_the_device_cannot_hold_is_the_cpus_for_that_develop() {
    let Some(ctx) = context_with(greycard_gpu::wgpu::Limits {
        max_texture_dimension_2d: 256,
        ..greycard_gpu::wgpu::Limits::default()
    }) else {
        skipped("the oversized picture's develop");
        return;
    };
    let deliver: Deliver = Arc::new(|_| {});
    let mut gpu = Some(ctx);
    let large = Arc::new(aberrated_frame(400, 320));
    let small = Arc::new(aberrated_frame(200, 160));
    for sharpen in [false, true] {
        for dehaze in [0.0, 0.6] {
            let mut edit = Edit::default();
            edit.sharpen.enabled = sharpen;
            edit.detail.clarity = 0.5;
            edit.detail.dehaze = dehaze;
            let develop = |frame: &Arc<RawFrame>,
                           base: &mut Option<FrameKept>,
                           gpu: &mut Option<greycard_gpu::Context>| {
                develop_job(
                    &Input::Raw(frame.clone()),
                    &edit,
                    0,
                    1,
                    base,
                    &mut Ai::new(),
                    None,
                    None,
                    gpu,
                    &deliver,
                )
            };
            let what = format!("sharpen {sharpen}, dehaze {dehaze}");
            let mut base = None;
            let (outcome, image) = develop(&large, &mut base, &mut gpu);
            assert!(
                matches!(
                    outcome,
                    Outcome::Developed {
                        image: Developed::Halves(_),
                        detail: Some((_, DetailRan::Cpu(_))),
                        ..
                    }
                ),
                "{what}: the CPU's develop"
            );
            assert!(gpu.is_some(), "{what}: the context is kept");
            let (_, reference) = develop(&large, &mut None, &mut None);
            assert_eq!(image.unwrap().data, reference.unwrap().data, "{what}");
            let b = base.as_ref().unwrap();
            assert!(
                !b.kept.has_upload() && !b.kept.on_device(Keep::PreSharpen) && !b.kept.has_haze()
            );
            // Nor the CPU's picture before the sharpen, the size of
            // the frame, kept on the frames largest.
            assert!(!b.kept.holds(Keep::PreSharpen), "{what}");
            let (outcome, _) = develop(&small, &mut None, &mut gpu);
            assert!(
                matches!(
                    outcome,
                    Outcome::Developed {
                        image: Developed::Texture(_),
                        detail: Some((_, DetailRan::Gpu)),
                        ..
                    }
                ),
                "{what}: the next picture that fits is the GPU's"
            );
        }
    }
}

/// A move of the sharpen alone goes on from the picture before it,
/// kept on the device, and gives the texture a develop made afresh
/// gives, to the bit; so does a move of the light, which goes on from
/// the same point. A move of the Detail section goes back to the
/// patched picture's upload.
#[test]
fn a_sharpen_move_from_the_kept_picture_is_a_fresh_develop() {
    use crate::stages::Keep;
    let Some(ctx) = greycard_gpu::Context::own().ok() else {
        skipped("the sharpen move's picture");
        return;
    };
    let frame = Arc::new(aberrated_frame(400, 320));
    let deliver: Deliver = Arc::new(|_| {});
    let mut gpu = Some(ctx);
    let develop =
        |edit: &Edit, base: &mut Option<FrameKept>, gpu: &mut Option<greycard_gpu::Context>| {
            let (outcome, _) = develop_job(
                &Input::Raw(frame.clone()),
                edit,
                0,
                1,
                base,
                &mut Ai::new(),
                None,
                None,
                gpu,
                &deliver,
            );
            let Outcome::Developed {
                image: Developed::Texture(texture),
                detail: Some((_, ran)),
                sharpen: Some(_),
                ..
            } = outcome
            else {
                panic!("the local contrast and the sharpen on the GPU, into a texture");
            };
            (texture, ran)
        };
    let mut edit = Edit::default();
    edit.detail.clarity = 0.5;
    let mut base = None;
    let (_, ran) = develop(&edit, &mut base, &mut gpu);
    assert_eq!(ran, DetailRan::Gpu);
    let b = base.as_ref().unwrap();
    assert!(b.kept.on_device(Keep::PreSharpen) && b.kept.has_upload());
    let mut moves = Vec::new();
    edit.sharpen.iterations += 1;
    moves.push((edit.clone(), DetailRan::Kept));
    edit.light.exposure = 0.7;
    moves.push((edit.clone(), DetailRan::Kept));
    edit.detail.clarity = 0.2;
    moves.push((edit.clone(), DetailRan::Gpu));
    for (edit, expected) in moves {
        let (moved, ran) = develop(&edit, &mut base, &mut gpu);
        assert_eq!(ran, expected);
        let (fresh, _) = develop(&edit, &mut None, &mut gpu);
        let ctx = gpu.as_ref().unwrap();
        let (moved, fresh) = (read_viewport(ctx, &moved), read_viewport(ctx, &fresh));
        assert!(
            moved.data == fresh.data,
            "{expected:?}: the move's picture is not a fresh develop's"
        );
    }
}

/// A mask's Detail reads bands made beside the run on the device,
/// which land after the run's keeps are put: a sharpen move, and a
/// Dehaze move that goes on from the local contrast kept on the device,
/// still hand the window the bands, so a mask's Texture does not go.
#[test]
fn a_masks_detail_bands_outlive_a_move_after_them() {
    let Some(ctx) = greycard_gpu::Context::own().ok() else {
        skipped("the masks' bands on a move after them");
        return;
    };
    let frame = Arc::new(aberrated_frame(400, 320));
    let deliver: Deliver = Arc::new(|_| {});
    let mut gpu = Some(ctx);
    let mut edit = Edit::default();
    edit.detail.clarity = 0.4;
    let mut a = greycard_edit::Adjustment {
        id: 1,
        ..Default::default()
    };
    a.mask
        .components
        .push(greycard_edit::mask::Component::default());
    a.look.detail.texture = -0.5;
    edit.adjustments.push(a);
    let mut base = None;
    let mut develop = |edit: &Edit| {
        let (outcome, _) = develop_job(
            &Input::Raw(frame.clone()),
            edit,
            0,
            1,
            &mut base,
            &mut Ai::new(),
            None,
            None,
            &mut gpu,
            &deliver,
        );
        let Outcome::Developed {
            planes,
            detail: Some((_, ran)),
            ..
        } = outcome
        else {
            panic!("developed, with the local contrast");
        };
        (planes.is_some(), ran)
    };
    assert_eq!(develop(&edit), (true, DetailRan::Gpu));
    edit.sharpen.iterations += 1;
    assert_eq!(develop(&edit), (true, DetailRan::Kept), "a sharpen move");
    edit.detail.dehaze = 0.5;
    assert_eq!(develop(&edit), (true, DetailRan::Kept), "a Dehaze move");
}

/// A Dehaze set with the sharpen on goes on from the local contrast's
/// picture kept on the device (one moved runs the local contrast
/// again, there being no keep between the two), and gives the texture a develop made
/// afresh gives, to the bit.
#[test]
fn a_dehaze_from_the_kept_local_contrast_is_a_fresh_develop() {
    let Some(ctx) = greycard_gpu::Context::own().ok() else {
        skipped("the Dehaze after the kept local contrast");
        return;
    };
    let frame = Arc::new(aberrated_frame(400, 320));
    let deliver: Deliver = Arc::new(|_| {});
    let mut gpu = Some(ctx);
    let develop =
        |edit: &Edit, base: &mut Option<FrameKept>, gpu: &mut Option<greycard_gpu::Context>| {
            let (outcome, _) = develop_job(
                &Input::Raw(frame.clone()),
                edit,
                0,
                1,
                base,
                &mut Ai::new(),
                None,
                None,
                gpu,
                &deliver,
            );
            let Outcome::Developed {
                image: Developed::Texture(texture),
                detail: Some((_, ran)),
                sharpen: Some(_),
                ..
            } = outcome
            else {
                panic!("the Detail section and the sharpen on the GPU, into a texture");
            };
            (texture, ran)
        };
    let mut edit = Edit::default();
    edit.detail.clarity = 0.5;
    let mut base = None;
    for dehaze in [0.4, 0.9] {
        edit.detail.dehaze = 0.0;
        develop(&edit, &mut base, &mut gpu);
        edit.detail.dehaze = dehaze;
        let (moved, ran) = develop(&edit, &mut base, &mut gpu);
        assert_eq!(ran, DetailRan::Kept, "{dehaze}");
        let (fresh, _) = develop(&edit, &mut None, &mut gpu);
        let ctx = gpu.as_ref().unwrap();
        let (moved, fresh) = (read_viewport(ctx, &moved), read_viewport(ctx, &fresh));
        assert!(
            moved.data == fresh.data,
            "Dehaze {dehaze}: the move's picture is not a fresh develop's"
        );
    }
}

/// A walk nobody waits on (a prefetch) that meets a device error short
/// of a lost device goes on on the CPU and leaves the session its
/// context; the window's walk under the same error lets it go. Either
/// way the picture is the CPU's. A device allowed 20 workgroups a
/// dimension cannot run the 400x320's CA.
#[test]
fn a_prefetch_s_device_error_leaves_the_session_its_device() {
    use crate::stages::{Purpose, stages};
    use crate::walk::{Buf, Hold, Kept, NoHold, NoModels, plan, walk};
    struct Speculative;
    impl Hold for Speculative {
        fn keeps_device(&self) -> bool {
            true
        }
    }
    let limits = greycard_gpu::wgpu::Limits {
        max_compute_workgroups_per_dimension: 20,
        ..greycard_gpu::wgpu::Limits::default()
    };
    let Some(ctx) = context_with(limits.clone()) else {
        skipped("the prefetch's device error");
        return;
    };
    let input = crate::walk::Input::Raw(Arc::new(aberrated_frame(400, 320)));
    let purpose = Purpose::Prefetch;
    let steps = stages(&Edit::default(), &purpose, &input.facts(0), None);
    let picture = |hold: &dyn Hold, gpu: &mut Option<greycard_gpu::Context>| {
        let places = plan(&steps, &purpose, gpu.is_some());
        let walked = walk(
            &steps,
            &places,
            &input,
            &mut Kept::new(),
            &mut NoModels,
            gpu,
            hold,
        )
        .expect("walks");
        match walked.buf {
            Buf::Working(image) => image,
            _ => panic!("a prefetch ends at a picture in memory"),
        }
    };
    let reference = picture(&NoHold, &mut None);
    let mut gpu = Some(ctx);
    let fetched = picture(&Speculative, &mut gpu);
    assert!(gpu.is_some(), "the session keeps its device");
    assert!(fetched.data == reference.data);
    let mut gpu = context_with(limits);
    let window = picture(&NoHold, &mut gpu);
    assert!(gpu.is_none(), "the window's walk lets it go");
    assert!(window.data == reference.data);
}

/// A move of the Dehaze alone, which fits again from the kept
/// reduced copy, gives the picture a develop made afresh at that
/// amount gives, to the bit: with the sharpen off (the dehaze's own
/// output in the viewport's texture) and on.
#[test]
fn a_dehaze_move_from_the_kept_copy_is_a_fresh_develop() {
    let Some(ctx) = greycard_gpu::Context::own().ok() else {
        skipped("the Dehaze move's picture");
        return;
    };
    let frame = Arc::new(aberrated_frame(400, 320));
    let deliver: Deliver = Arc::new(|_| {});
    let mut gpu = Some(ctx);
    for sharpen in [false, true] {
        let mut edit = Edit::default();
        edit.sharpen.enabled = sharpen;
        edit.detail.texture = 0.3;
        edit.detail.clarity = 0.5;
        let mut develop = |edit: &Edit, base: &mut Option<FrameKept>| {
            let (outcome, _) = develop_job(
                &Input::Raw(frame.clone()),
                edit,
                0,
                1,
                base,
                &mut Ai::new(),
                None,
                None,
                &mut gpu,
                &deliver,
            );
            let Outcome::Developed {
                image: Developed::Texture(texture),
                dehaze: Some(_),
                ..
            } = outcome
            else {
                panic!("the dehaze on the GPU, into a texture");
            };
            texture
        };
        let mut base = None;
        edit.detail.dehaze = 0.4;
        develop(&edit, &mut base);
        let reduced = base.as_ref().unwrap().kept.haze_copy().unwrap().clone();
        edit.detail.dehaze = 0.9;
        let moved = develop(&edit, &mut base);
        let kept = base.as_ref().unwrap().kept.haze_copy().unwrap();
        assert!(
            Arc::ptr_eq(kept, &reduced),
            "the move fitted from the kept copy"
        );
        let fresh = develop(&edit, &mut None);
        let ctx = gpu.as_ref().unwrap();
        let (moved, fresh) = (read_viewport(ctx, &moved), read_viewport(ctx, &fresh));
        assert!(
            moved.data == fresh.data,
            "sharpen {sharpen}: the move's picture is not a fresh develop's"
        );
    }
}

pub(super) fn thumb_scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "greycard-thumbjob-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cache_at(dir: &std::path::Path) -> ThumbCache {
    Arc::new(Mutex::new(Some(Thumbs::at(
        dir.join("thumbs"),
        greycard_library::thumbs::DEFAULT_CAP,
    ))))
}

/// A raw nothing can decode: its thumbnail can only have come
/// from the cache.
#[test]
fn a_cached_thumbnail_is_not_made_again() {
    let dir = thumb_scratch("hit");
    let raw = dir.join("IMG_0001.CR3");
    std::fs::write(&raw, vec![0x17u8; 90_000]).unwrap();
    let cache = cache_at(&dir);
    assert!(
        cached_thumbnail_noting(
            &cache,
            None,
            None,
            &Frame::file(&raw),
            THUMB_WIDTH,
            thumbnail
        )
        .is_err(),
        "not a raw, and nothing cached"
    );
    let kept = Thumb {
        width: 3,
        height: 2,
        rgb: vec![128; 18],
    };
    let hash = greycard_library::hash_file(&raw).unwrap();
    cache
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .put(
            &hash,
            THUMB_WIDTH,
            thumb_tag(file_stat(&raw).unwrap()),
            &kept,
        )
        .unwrap();
    // Renamed into another folder: still the cache's.
    let moved_dir = dir.join("renamed");
    std::fs::create_dir_all(&moved_dir).unwrap();
    let moved = moved_dir.join("wedding-0001.CR3");
    std::fs::rename(&raw, &moved).unwrap();
    let (thumb, cached) = cached_thumbnail_noting(
        &cache,
        None,
        None,
        &Frame::file(&moved),
        THUMB_WIDTH,
        thumbnail,
    )
    .unwrap();
    assert!(cached);
    assert_eq!((thumb.width, thumb.height), (3, 2));
    // Another size is not the same entry.
    assert!(
        cached_thumbnail_noting(&cache, None, None, &Frame::file(&moved), 256, thumbnail).is_err()
    );
    crate::testing::remove_dir_retry(&dir);
}

/// An offline frame's picture: looked up by the row's hash and
/// stamp at the size the pool keeps pictures at, the file itself
/// gone.
#[test]
fn a_cached_thumbnail_job_finds_what_the_pool_kept() {
    let dir = thumb_scratch("offline");
    let raw = dir.join("IMG_0002.CR3");
    std::fs::write(&raw, vec![0x2au8; 90_000]).unwrap();
    let hash = greycard_library::hash_file(&raw).unwrap();
    let stamp = thumb_tag(file_stat(&raw).unwrap()).stamp;
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = Mutex::new(tx);
    let worker = Worker::new(move |o| {
        let line = match o {
            Outcome::Thumbnail {
                index,
                size,
                width,
                cached,
                ..
            } => format!("thumbnail {index} {size} {width} {cached}"),
            Outcome::NoThumbnail { index, .. } => format!("none {index}"),
            _ => return,
        };
        tx.lock().unwrap().send(line).unwrap();
    });
    let mut thumbs = Thumbs::at(dir.join("thumbs"), greycard_library::thumbs::DEFAULT_CAP);
    let made = crate::made::made_size(worker.pool().expect("a window's worker").size());
    let kept = Thumb {
        width: 5,
        height: 4,
        rgb: vec![90; 60],
    };
    thumbs
        .put(&hash, made, thumb_tag(file_stat(&raw).unwrap()), &kept)
        .unwrap();
    worker.set_thumb_cache(Some(thumbs));
    std::fs::remove_file(&raw).unwrap();
    worker.send(Job::CachedThumbnail {
        index: 3,
        frame: Frame::file(raw.clone()),
        hash: hash.clone(),
        stamp,
    });
    let wait = std::time::Duration::from_secs(10);
    assert_eq!(
        rx.recv_timeout(wait).unwrap(),
        format!("thumbnail 3 {made} 5 true")
    );
    // Another stamp is another entry: a miss, and nothing made.
    worker.send(Job::CachedThumbnail {
        index: 4,
        frame: Frame::file(raw),
        hash,
        stamp: stamp + 1,
    });
    assert_eq!(rx.recv_timeout(wait).unwrap(), "none 4");
    worker.stop();
    crate::testing::remove_dir_retry(&dir);
}

/// A picture file is made once, kept, found; re-written in place,
/// it is made again, whatever its head says.
#[test]
fn a_picture_is_kept_until_it_is_written_again() {
    let dir = thumb_scratch("picture");
    let png = dir.join("export.png");
    let write = |shade: u8| {
        image::RgbImage::from_pixel(40, 20, image::Rgb([shade, shade, shade]))
            .save(&png)
            .unwrap();
    };
    write(200);
    let cache = cache_at(&dir);
    let (first, cached) =
        cached_thumbnail_noting(&cache, None, None, &Frame::file(&png), 16, thumbnail).unwrap();
    assert!(!cached);
    let (again, cached) =
        cached_thumbnail_noting(&cache, None, None, &Frame::file(&png), 16, thumbnail).unwrap();
    assert!(cached);
    assert_eq!((again.width, again.height), (first.width, first.height));
    // Written again with another mtime.
    write(20);
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
    std::fs::File::options()
        .write(true)
        .open(&png)
        .unwrap()
        .set_modified(later)
        .unwrap();
    let (fresh, cached) =
        cached_thumbnail_noting(&cache, None, None, &Frame::file(&png), 16, thumbnail).unwrap();
    assert!(!cached);
    assert!(
        fresh.rgb.iter().all(|v| *v < 60),
        "the new picture's pixels"
    );
    crate::testing::remove_dir_retry(&dir);
}

/// A stand-in for the decode: a picture whose every byte is the
/// file's last, which is zero while a preallocated copy has not
/// reached its tail, as a cut preview decodes grey.
fn tail_picture(path: &std::path::Path, _size: u32) -> anyhow::Result<(u32, u32, Vec<u8>)> {
    let bytes = std::fs::read(path)?;
    let last = *bytes.last().unwrap_or(&0);
    Ok((4, 3, vec![last; 36]))
}

fn set_time(path: &std::path::Path, secs_from_now: u64) {
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(secs_from_now))
        .unwrap();
}

/// The review's recipe: a raw whose copier set its length first
/// and has written only its head. Its key is already the finished
/// file's, so what keeps the half-made picture from standing for
/// the finished file is the time the copy moves on.
#[test]
fn a_raw_caught_half_copied_is_made_again_when_the_copy_ends() {
    let dir = thumb_scratch("halfcopied");
    let whole: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8 | 1).collect();
    let raw = dir.join("DSC_1981.NEF");
    // `truncate -s` to the full length, then the first 192 KB.
    let mut half = vec![0u8; whole.len()];
    half[..196_608].copy_from_slice(&whole[..196_608]);
    std::fs::write(&raw, &half).unwrap();
    let cache = cache_at(&dir);
    let (grey, cached) =
        cached_thumbnail_noting(&cache, None, None, &Frame::file(&raw), 176, tail_picture).unwrap();
    assert!(!cached);
    assert!(grey.rgb.iter().all(|v| *v == 0));
    // The same head and the same length: the same hash.
    let before = greycard_library::hash_file(&raw).unwrap();
    std::fs::write(&raw, &whole).unwrap();
    set_time(&raw, 7);
    assert_eq!(greycard_library::hash_file(&raw).unwrap(), before);
    let (done, cached) =
        cached_thumbnail_noting(&cache, None, None, &Frame::file(&raw), 176, tail_picture).unwrap();
    assert!(
        !cached,
        "the half-copied picture is not the finished file's"
    );
    assert!(done.rgb.iter().all(|v| *v == *whole.last().unwrap()));
    // And the finished file's picture is kept, and a rename keeps
    // its time and finds it.
    let renamed = dir.join("wedding-1981.NEF");
    std::fs::rename(&raw, &renamed).unwrap();
    let (again, cached) = cached_thumbnail_noting(
        &cache,
        None,
        None,
        &Frame::file(&renamed),
        176,
        tail_picture,
    )
    .unwrap();
    assert!(cached);
    assert_eq!(again.rgb, done.rgb);
    crate::testing::remove_dir_retry(&dir);
}

/// A card's frame and its import by a plain `cp`: one head, one
/// length, two times. Each is made once, and after that both hit,
/// opened by turns, rather than each removing the other's entry.
#[test]
fn two_copies_with_two_times_both_hit() {
    let dir = thumb_scratch("twocopies");
    let card = dir.join("card").join("IMG_0007.CR3");
    let import = dir.join("import").join("IMG_0007.CR3");
    std::fs::create_dir_all(card.parent().unwrap()).unwrap();
    std::fs::create_dir_all(import.parent().unwrap()).unwrap();
    std::fs::write(&card, vec![7u8; 90_000]).unwrap();
    std::fs::copy(&card, &import).unwrap();
    set_time(&import, 60);
    assert_eq!(
        greycard_library::hash_file(&card).unwrap(),
        greycard_library::hash_file(&import).unwrap()
    );
    let cache = cache_at(&dir);
    for (round, path) in [&card, &import, &card, &import, &card]
        .into_iter()
        .enumerate()
    {
        let (_, cached) =
            cached_thumbnail_noting(&cache, None, None, &Frame::file(path), 176, tail_picture)
                .unwrap();
        assert_eq!(cached, round >= 2, "round {round}");
    }
    crate::testing::remove_dir_retry(&dir);
}

/// A file that grows or is touched while its picture is being made
/// gets the picture shown and not kept.
#[test]
fn a_file_changing_under_the_making_is_not_kept() {
    let dir = thumb_scratch("changing");
    let raw = dir.join("IMG_0002.CR3");
    std::fs::write(&raw, vec![5u8; 80_000]).unwrap();
    let cache = cache_at(&dir);
    let grows = |path: &std::path::Path, size: u32| {
        let made = tail_picture(path, size);
        let mut f = std::fs::File::options().append(true).open(path).unwrap();
        std::io::Write::write_all(&mut f, &[9u8; 1000]).unwrap();
        made
    };
    let (_, cached) =
        cached_thumbnail_noting(&cache, None, None, &Frame::file(&raw), 176, grows).unwrap();
    assert!(!cached);
    let touched = |path: &std::path::Path, size: u32| {
        let made = tail_picture(path, size);
        set_time(path, 30);
        made
    };
    let (_, cached) =
        cached_thumbnail_noting(&cache, None, None, &Frame::file(&raw), 176, touched).unwrap();
    assert!(!cached);
    assert_eq!(
        cache.lock().unwrap().as_ref().unwrap().usage().entries,
        0,
        "neither was kept"
    );
    // Left alone, it is made once and kept.
    let (_, cached) =
        cached_thumbnail_noting(&cache, None, None, &Frame::file(&raw), 176, tail_picture).unwrap();
    assert!(!cached);
    let (_, cached) =
        cached_thumbnail_noting(&cache, None, None, &Frame::file(&raw), 176, tail_picture).unwrap();
    assert!(cached);
    crate::testing::remove_dir_retry(&dir);
}

/// A cache with a cap of nothing is off: nothing looked up, nothing
/// written, though it is still there to be cleared.
#[test]
fn a_cap_of_nothing_is_the_cache_off() {
    let dir = thumb_scratch("capzero");
    let png = dir.join("a.png");
    image::RgbImage::from_pixel(8, 8, image::Rgb([9, 9, 9]))
        .save(&png)
        .unwrap();
    let cache: ThumbCache = Arc::new(Mutex::new(Some(Thumbs::at(dir.join("thumbs"), 0))));
    for _ in 0..2 {
        let (_, cached) =
            cached_thumbnail_noting(&cache, None, None, &Frame::file(&png), 8, thumbnail).unwrap();
        assert!(!cached);
    }
    assert!(!dir.join("thumbs").exists());
    crate::testing::remove_dir_retry(&dir);
}

/// The count runs off the caller's thread and seeds the cache.
#[test]
fn the_cache_is_counted_on_a_thread_of_its_own() {
    let dir = thumb_scratch("count");
    let root = dir.join("thumbs");
    let mut filled = Thumbs::at(&root, greycard_library::thumbs::DEFAULT_CAP);
    let kept = Thumb {
        width: 3,
        height: 2,
        rgb: vec![128; 18],
    };
    filled
        .put(&"ab".repeat(32), 128, Tag::default(), &kept)
        .unwrap();
    let cache: ThumbCache = Arc::new(Mutex::new(Some(Thumbs::at(
        &root,
        greycard_library::thumbs::DEFAULT_CAP,
    ))));
    let (tx, rx) = std::sync::mpsc::channel();
    count_thumb_cache(&cache, move |known| tx.send(known).unwrap());
    let known = rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .unwrap()
        .expect("counted");
    assert_eq!(known.entries, 1);
    assert_eq!(
        cache.lock().unwrap().as_ref().unwrap().known_usage(),
        Some(known)
    );
    crate::testing::remove_dir_retry(&dir);
}

#[test]
fn without_a_cache_a_thumbnail_is_made_every_time() {
    let dir = thumb_scratch("off");
    let png = dir.join("a.png");
    image::RgbImage::from_pixel(8, 8, image::Rgb([9, 9, 9]))
        .save(&png)
        .unwrap();
    let off: ThumbCache = Arc::new(Mutex::new(None));
    for _ in 0..2 {
        let (_, cached) =
            cached_thumbnail_noting(&off, None, None, &Frame::file(&png), 8, thumbnail).unwrap();
        assert!(!cached);
    }
    assert!(!dir.join("thumbs").exists());
    crate::testing::remove_dir_retry(&dir);
}

/// A learned mask the edit asks for whose model is not in the
/// store is left out of the picture, as the window draws it, and
/// said: the export does not pass over it in silence. An Object
/// with nothing picked asks for nothing and is not said, nor is a
/// shape switched off or one in an adjustment switched off, which
/// the finish never draws.
#[test]
fn an_export_says_which_learned_mask_it_went_without() {
    use greycard_edit::mask::Component;
    let frame = Arc::new(aberrated_frame(160, 120));
    let mut base = None;
    let (_, image) = develop_once(&frame, &mut base, &mut None);
    let image = image.unwrap();
    let empty = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/test-scratch")
        .join(format!("left-out-{}/models", std::process::id()));
    std::fs::create_dir_all(&empty).unwrap();
    let mut ai = Ai::for_test(greycard_ai::Store::at(&empty), Vec::new());
    let mut edit = Edit::default();
    edit.adjustments.push(greycard_edit::Adjustment {
        name: "Face".into(),
        mask: greycard_edit::mask::Mask {
            components: vec![
                Component {
                    shape: Shape::Subject {},
                    ..Component::default()
                },
                Component {
                    shape: Shape::Object {
                        picks: Vec::new(),
                        boxes: Vec::new(),
                    },
                    ..Component::default()
                },
            ],
            invert: false,
            ..Default::default()
        },
        ..greycard_edit::Adjustment::default()
    });
    // An adjustment switched off is not drawn, so its Subject is
    // not missed; nor is a Subject shape switched off.
    edit.adjustments.push(greycard_edit::Adjustment {
        name: "Off".into(),
        enabled: false,
        mask: greycard_edit::mask::Mask {
            components: vec![Component {
                shape: Shape::Subject {},
                ..Component::default()
            }],
            invert: false,
            ..Default::default()
        },
        ..greycard_edit::Adjustment::default()
    });
    edit.adjustments.push(greycard_edit::Adjustment {
        name: "Shape off".into(),
        mask: greycard_edit::mask::Mask {
            components: vec![Component {
                shape: Shape::Subject {},
                enabled: false,
                ..Component::default()
            }],
            invert: false,
            ..Default::default()
        },
        ..greycard_edit::Adjustment::default()
    });
    let (_, left_out) = finish_export(
        image,
        &edit,
        base.as_ref(),
        &mut ai,
        &greycard_render::export::Settings::default(),
    );
    crate::testing::remove_dir_retry(empty.parent().unwrap());
    assert_eq!(left_out.len(), 1, "{left_out:?}");
    assert!(
        left_out[0].starts_with("Face's Subject shape:"),
        "{}",
        left_out[0]
    );
}

/// A develop whose learned denoiser or fill model is missing says
/// so for the export; one that asked for neither says nothing.
#[test]
fn a_develop_without_its_model_says_what_was_left_out() {
    let developed = |learned, fills| Outcome::Developed {
        generation: 0,
        turn: 0,
        kind: greycard_render::finish::Source::SCENE,
        image: Developed::Halves(Arc::new(Halves {
            width: 1,
            height: 1,
            pixels: vec![half::f16::ZERO; 4],
        })),
        guide: Arc::new(greycard_render::finish::Guide::NONE),
        planes: None,
        white: WhiteBase::IDENTITY,
        seconds: 0.0,
        detail: None,
        sharpen: None,
        dehaze: None,
        sources: Vec::new(),
        learned,
        fills,
    };
    assert!(left_out_of(&developed(LearnedReport::Off, FillReport::default()), None).is_empty());
    let model = greycard_ai::denoiser(greycard_edit::Learned::Fast.tier().unwrap()).unwrap();
    let said = left_out_of(
        &developed(
            LearnedReport::Missing(model),
            FillReport {
                missing: vec![
                    ("Patch 1".into(), &greycard_ai::FILL),
                    ("Patch 2".into(), &greycard_ai::REMOVE),
                ],
                no_gpu: vec!["Patch 3".into()],
                ..FillReport::default()
            },
        ),
        None,
    );
    assert_eq!(said.len(), 4, "{said:?}");
    assert!(said[0].contains(model.name), "{}", said[0]);
    assert!(said[1].starts_with("fill Patch 1:"), "{}", said[1]);
    // A patch the generative remove made, on a machine without it,
    // names the model it waits on.
    assert!(said[2].starts_with("fill Patch 2:"), "{}", said[2]);
    assert!(said[2].contains(greycard_ai::REMOVE.name), "{}", said[2]);
    assert!(said[2].ends_with("left as it was"), "{}", said[2]);
    // One it made, on a machine without a GPU, says so instead.
    assert_eq!(
        said[3],
        "fill Patch 3: the remove needs a GPU; left as it was"
    );
}

/// A base made as the engine's stand-in for a learned denoiser
/// whose model is missing keeps saying so when a later develop
/// keeps it: that develop reports `Kept`, and the export of its
/// picture must still say what it went without.
#[test]
fn a_kept_stand_in_base_still_says_its_denoiser_was_missing() {
    let frame = Arc::new(aberrated_frame(160, 120));
    let empty = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/test-scratch")
        .join(format!("stand-in-{}/models", std::process::id()));
    std::fs::create_dir_all(&empty).unwrap();
    let mut ai = Ai::for_test(greycard_ai::Store::at(&empty), Vec::new());
    let mut edit = Edit::default();
    edit.noise.enabled = true;
    edit.noise.learned = greycard_edit::Learned::Fast;
    let deliver: Deliver = Arc::new(|_| {});
    let input = Input::Raw(frame);
    let mut base = None;
    let mut develop = |edit: &Edit, base: &mut Option<FrameKept>| {
        develop_job(
            &input, edit, 0, 1, base, &mut ai, None, None, &mut None, &deliver,
        )
        .0
    };
    let first = develop(&edit, &mut base);
    // Without ONNX Runtime the denoiser is not missing but cannot run,
    // said with the runtime's line; the kept base says that instead.
    let runtime = greycard_ai::runtime::status().err();
    if let Some(why) = runtime {
        greycard_ai::runtime::absent_in_test(why);
        assert!(
            matches!(
                &first,
                Outcome::Developed {
                    learned: LearnedReport::Failed(m),
                    ..
                } if *m == why.to_string()
            ),
            "the denoiser should fail with the runtime's line"
        );
    } else {
        assert!(matches!(
            &first,
            Outcome::Developed {
                learned: LearnedReport::Missing(_),
                ..
            }
        ));
    }
    // A move that keeps the base: the exposure.
    edit.light.exposure = 0.5;
    let second = develop(&edit, &mut base);
    crate::testing::remove_dir_retry(empty.parent().unwrap());
    assert!(matches!(
        &second,
        Outcome::Developed {
            learned: LearnedReport::Kept,
            ..
        }
    ));
    assert!(left_out_of(&second, None).is_empty(), "the outcome alone");
    let said = left_out_of(&second, base.as_ref());
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].starts_with("the learned denoiser"), "{}", said[0]);
}

/// The viewport's half-float texture read back as a working image,
/// its alpha dropped.
pub(super) fn read_viewport(
    ctx: &greycard_gpu::Context,
    texture: &greycard_gpu::wgpu::Texture,
) -> WorkingImage {
    use greycard_gpu::wgpu;
    let (w, h) = (texture.width(), texture.height());
    let row = (w * 8).div_ceil(256) * 256;
    let band = ((64u32 << 20) / row).max(1);
    let mut data = Vec::with_capacity((w * h * 3) as usize);
    for y0 in (0..h).step_by(band as usize) {
        let rows = band.min(h - y0);
        let staging = ctx.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("read back"),
            size: u64::from(row * rows),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = ctx.device().create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y: y0, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(rows),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: rows,
                depth_or_array_layers: 1,
            },
        );
        ctx.queue().submit(Some(encoder.finish()));
        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        ctx.device()
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        rx.recv().unwrap().unwrap();
        let mapped = slice.get_mapped_range().unwrap();
        for r in 0..rows as usize {
            let line: &[half::f16] =
                bytemuck::cast_slice(&mapped[r * row as usize..][..(w * 8) as usize]);
            for px in line.as_chunks::<4>().0 {
                data.extend([px[0].to_f32(), px[1].to_f32(), px[2].to_f32()]);
            }
        }
    }
    WorkingImage::from_data(w as usize, h as usize, data).unwrap()
}

/// A raw's develop names its body's baseline to everything finished
/// from it (the viewport, the export, the camera match): the GFX 100S
/// II's own, and the default for a body not in the table.
#[test]
fn a_raw_s_develop_carries_its_body_s_baseline() {
    let mut frame = aberrated_frame(120, 80);
    (frame.make, frame.model) = (crate::testing::GFX.0.into(), crate::testing::GFX.1.into());
    let mut base = None;
    develop_once(&Arc::new(frame.clone()), &mut base, &mut None);
    let b = greycard_edit::baseline::for_body(Some("Fujifilm GFX 100S II"));
    assert_eq!(
        base.as_ref().expect("a base").meta.source,
        greycard_render::finish::Source::Scene(greycard_render::finish::Baseline::of_stops(b))
    );
    (frame.make, frame.model) = ("Canon".into(), "Canon EOS R5 Mark II".into());
    let mut base = None;
    develop_once(&Arc::new(frame), &mut base, &mut None);
    assert_eq!(
        base.expect("a base").meta.source,
        greycard_render::finish::Source::SCENE
    );
}

/// The shelf's key for `frame`, as an open of a file at `path` makes it.
fn shelf_key(path: &str, frame: &Arc<RawFrame>) -> super::shelf::FileKey {
    super::shelf::FileKey {
        path: path.into(),
        stat: super::shelf::Stat::default(),
        frame: None,
    }
    .decoded(&Input::Raw(frame.clone()))
}

/// A frame left and opened again develops from the base it left on
/// the shelf: the base not made again, and said to be from the shelf
/// only on its first develop, and the base and the picture both the
/// ones a fresh develop of the same file under the same edit makes, to
/// the bit. Under an edit with another base it is made afresh.
#[test]
fn a_base_taken_back_from_the_shelf_is_developed_from_and_not_made_again() {
    use super::engine::swap_base;
    use super::shelf::{Budget, Shelf};
    let frame = Arc::new(aberrated_frame(120, 80));
    let edit = Edit::default();
    let key = shelf_key("a.CR3", &frame);
    let mut shelf = Shelf::new(Budget::Bytes(1 << 30));
    let mut base = None;
    let first = develop_turned(&frame, &edit, 0, &mut base);
    let made = base.as_ref().expect("a base").image.clone();

    // Away to another file, whose base is made, and back.
    swap_base(
        &mut base,
        &mut shelf,
        Some(key.clone()),
        None,
        &edit,
        0,
        None,
        None,
    );
    assert!(base.is_none());
    develop_turned(&Arc::new(aberrated_frame(64, 48)), &edit, 0, &mut base);
    swap_base(
        &mut base,
        &mut shelf,
        None,
        Some(&key),
        &edit,
        0,
        None,
        None,
    );
    assert!(base.as_ref().is_some_and(|b| b.from_shelf));
    let again = develop_turned(&frame, &edit, 0, &mut base);
    let b = base.as_ref().expect("the base");
    assert!(Arc::ptr_eq(&b.image, &made), "kept, not made again");
    assert!(!b.from_shelf, "said once");
    assert!(again.data == first.data);
    // A fresh develop of the same file under the same edit, from
    // nothing: the same base and the same picture.
    let mut fresh = None;
    let made_fresh = develop_turned(&frame, &edit, 0, &mut fresh);
    let fresh = fresh.expect("a fresh base");
    assert!(!Arc::ptr_eq(&fresh.image, &made));
    assert!(fresh.image.data == base.as_ref().unwrap().image.data);
    assert!(again.data == made_fresh.data);

    // Under another white balance the base is the edit's own.
    let warmer = Edit {
        white_balance: greycard_edit::WhiteBalance::Custom {
            temperature: 6500.0,
            tint: 0.0,
        },
        ..edit.clone()
    };
    swap_base(
        &mut base,
        &mut shelf,
        Some(key.clone()),
        Some(&key),
        &warmer,
        0,
        None,
        None,
    );
    assert!(base.is_none());
    develop_turned(&frame, &warmer, 0, &mut base);
    assert!(!Arc::ptr_eq(&base.as_ref().unwrap().image, &made));
}

/// On the GPU, a base shelved lets go of what it held on the device
/// (the picture before the sharpen, the patched upload, and the
/// dehaze's reduced copy), and its first develop back uploads the
/// picture again and sharpens it there: the viewport's texture is the
/// one a fresh develop of the same file under the same edit leaves, to
/// the bit, and so is the base.
#[test]
fn a_base_taken_back_on_the_gpu_is_uploaded_again_and_develops_as_a_fresh_one() {
    use super::engine::swap_base;
    use super::shelf::{Budget, Shelf};
    let Some(ctx) = greycard_gpu::Context::own().ok() else {
        skipped("the shelf's GPU take-back");
        return;
    };
    let mut gpu = Some(ctx);
    let frame = Arc::new(aberrated_frame(400, 320));
    let key = shelf_key("a.CR3", &frame);
    let edit = Edit::default();
    assert!(edit.sharpen.options().is_some(), "the sharpen on the GPU");
    let texture = |outcome: Outcome| match outcome {
        Outcome::Developed {
            image: Developed::Texture(t),
            ..
        } => t,
        _ => panic!("a texture on the device"),
    };
    let mut shelf = Shelf::new(Budget::Bytes(1 << 30));
    let mut base = None;
    let first = texture(develop_once(&frame, &mut base, &mut gpu).0);
    let b = base.as_ref().expect("a base");
    assert!(b.on_gpu && b.kept.has_upload());
    let made = b.image.clone();
    drop(first);

    swap_base(
        &mut base,
        &mut shelf,
        Some(key.clone()),
        None,
        &edit,
        0,
        gpu.as_ref(),
        None,
    );
    develop_once(&Arc::new(aberrated_frame(200, 160)), &mut base, &mut gpu);
    swap_base(
        &mut base,
        &mut shelf,
        None,
        Some(&key),
        &edit,
        0,
        gpu.as_ref(),
        None,
    );
    let b = base.as_ref().expect("taken back");
    assert!(Arc::ptr_eq(&b.image, &made) && b.from_shelf);
    assert!(!b.kept.on_device(Keep::PreSharpen) && !b.kept.has_upload() && !b.kept.has_haze());

    let back = texture(develop_once(&frame, &mut base, &mut gpu).0);
    let b = base.as_ref().expect("the base");
    assert!(Arc::ptr_eq(&b.image, &made), "kept, not made again");
    assert!(b.kept.has_upload(), "uploaded again");
    let ctx = gpu.as_ref().expect("the device kept");
    let back = read_viewport(ctx, &back);

    let mut fresh = None;
    let fresh_texture = texture(develop_once(&frame, &mut fresh, &mut gpu).0);
    let ctx = gpu.as_ref().expect("the device kept");
    let fresh_picture = read_viewport(ctx, &fresh_texture);
    assert!(fresh.expect("a fresh base").image.data == made.data);
    assert!(back.data == fresh_picture.data);
}

/// A scratch folder with linear DNGs of our own named `names`, each
/// `width` by `height`: raws the worker decodes from the disk.
fn dngs(what: &str, names: &[&str], width: usize, height: usize) -> (PathBuf, Vec<PathBuf>) {
    let dir = crate::testing::scratch_dir(what);
    let paths = names
        .iter()
        .map(|n| {
            let path = dir.join(n);
            crate::testing::linear_dng(&path, width, height, "Test", "Cam");
            super::prefetch::watch(n);
            path
        })
        .collect();
    (dir, paths)
}

fn neighbor_at(path: &std::path::Path, edit: Edit, side: Side) -> Neighbor {
    Neighbor {
        path: path.to_path_buf(),
        edit,
        turn: 0,
        seed_blend: false,
        side,
    }
}

/// A neighbor prefetched onto the shelf is the base its open takes
/// back, said to be from the shelf, and develops to the picture a
/// fresh open of the same file makes, to the bit. Asked again while it
/// is there, it is not made twice.
#[test]
fn a_neighbor_prefetched_is_taken_back_by_its_open() {
    use super::engine::swap_base;
    use super::prefetch::{Ahead, Fetched, prefetch, said};
    use super::shelf::{Budget, FileKey, Shelf};
    let (dir, paths) = dngs("prefetch-took", &["pf-took-B.dng"], 96, 64);
    let b = &paths[0];
    let edit = Edit::default();
    let mut shelf = Shelf::new(Budget::Bytes(1 << 30));
    let n = neighbor_at(b, edit.clone(), Side::Next);
    assert_eq!(
        prefetch(&n, None, &mut shelf, &mut None, None, &|_| Ahead::Nothing),
        Fetched::Landed
    );
    assert_eq!(shelf.len(), 1);
    let lines = said("pf-took-B.dng");
    assert!(
        lines
            .iter()
            .any(|l| l == "prefetching the next frame, pf-took-B.dng"),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(
            |l| l.starts_with("prefetched pf-took-B.dng, the next frame: decode ")
                && l.contains(", prepare ")
                && l.contains(", demosaic ")
                && l.contains(", move into the working space ")
                && l.contains("; the shelf: 1 base(s)")
        ),
        "{lines:?}"
    );
    assert!(matches!(
        prefetch(&n, None, &mut shelf, &mut None, None, &|_| Ahead::Nothing),
        Fetched::Skipped(why) if why == "its base is on the shelf"
    ));
    assert_eq!(shelf.len(), 1);

    // The open of it, as the worker's: the key from the file and its
    // decode, the base taken back, and the develop from it.
    let (input, _) = open(b).expect("our DNG decodes");
    let key = FileKey::of(b).expect("on the disk").decoded(&input);
    let mut base = None;
    swap_base(
        &mut base,
        &mut shelf,
        None,
        Some(&key),
        &edit,
        0,
        None,
        None,
    );
    assert!(base.as_ref().is_some_and(|b| b.from_shelf), "taken back");
    assert!(
        said("pf-took-B.dng")
            .iter()
            .any(|l| l == "the base of pf-took-B.dng from the shelf")
    );
    let taken = base.as_ref().unwrap().image.clone();
    let deliver: Deliver = Arc::new(|_| {});
    let develop = |base: &mut Option<FrameKept>| {
        develop_job(
            &input,
            &edit,
            0,
            1,
            base,
            &mut Ai::new(),
            None,
            None,
            &mut None,
            &deliver,
        )
        .1
        .expect("a picture on the CPU")
    };
    let back = develop(&mut base);
    assert!(
        Arc::ptr_eq(&base.as_ref().unwrap().image, &taken),
        "not made again"
    );
    let mut fresh = None;
    let made = develop(&mut fresh);
    assert!(fresh.unwrap().image.data == taken.data);
    assert!(made.data == back.data);
    crate::testing::remove_dir_retry(&dir);
}

/// Anything asked for while a prefetch is in hand stops it at the next
/// stage, and nothing of it is kept: before the prepare (after the
/// decode), the demosaic, and the move into the working space. The open of
/// the very frame it is making does not; that is said once, and the
/// base lands.
#[test]
fn a_request_stops_a_prefetch_between_its_stages() {
    use super::prefetch::{Ahead, Fetched, prefetch, said};
    use super::shelf::{Budget, Shelf};
    let (dir, paths) = dngs("prefetch-stop", &["pf-stop-B.dng"], 96, 64);
    let n = neighbor_at(&paths[0], Edit::default(), Side::Previous);
    for (asked, stage) in [
        (1, "prepare"),
        (2, "demosaic"),
        (3, "move into the working space"),
    ] {
        let mut shelf = Shelf::new(Budget::Bytes(1 << 30));
        let calls = std::cell::Cell::new(0);
        let ahead = |_: &crate::stages::Facts| {
            calls.set(calls.get() + 1);
            if calls.get() >= asked {
                Ahead::Work
            } else {
                Ahead::Nothing
            }
        };
        assert!(matches!(
            prefetch(&n, None, &mut shelf, &mut None, None, &ahead),
            Fetched::Abandoned(at, _) if at == stage
        ));
        assert_eq!(shelf.len(), 0, "nothing kept when stopped before {stage}");
        assert_eq!(calls.get(), asked, "asked once a stage");
    }
    assert!(said("pf-stop-B.dng").iter().any(|l| l
        == "the prefetch of the previous frame, pf-stop-B.dng, abandoned before its demosaic: \
            the worker was asked for something"));

    let mut shelf = Shelf::new(Budget::Bytes(1 << 30));
    assert_eq!(
        prefetch(&n, None, &mut shelf, &mut None, None, &|_| Ahead::ItsOpen),
        Fetched::Landed
    );
    assert_eq!(shelf.len(), 1);
    let goes_on = said("pf-stop-B.dng")
        .into_iter()
        .filter(|l| l.contains("goes on"))
        .collect::<Vec<_>>();
    assert_eq!(
        goes_on,
        vec![
            "the prefetch of pf-stop-B.dng goes on before its prepare: \
             it is the frame being opened"
        ]
    );
    crate::testing::remove_dir_retry(&dir);
}

/// A frame whose edit asks for the learned denoiser is skipped before
/// anything is read: its model would run for seconds with no stage to
/// stop between, and its pair would hold a gigabyte on the shelf. So
/// is the frame whose base is the one in hand, another version of the
/// open file under the same base.
#[test]
fn a_learned_frame_and_the_open_one_are_not_prefetched() {
    use super::prefetch::{Ahead, Fetched, prefetch, said, watch};
    use super::shelf::{Budget, Shelf};
    watch("pf-learned-B.CR3");
    let mut shelf = Shelf::new(Budget::Bytes(1 << 30));
    let learned = Edit {
        noise: Noise {
            learned: greycard_edit::Learned::Fast,
            ..Noise::default()
        },
        ..Edit::default()
    };
    // No such file: a decode would say so.
    let nowhere = PathBuf::from("/nowhere/pf-learned-B.CR3");
    assert_eq!(
        prefetch(
            &neighbor_at(&nowhere, learned, Side::Next),
            None,
            &mut shelf,
            &mut None,
            None,
            &|_| Ahead::Nothing
        ),
        Fetched::Skipped("its edit asks for the learned denoiser".into())
    );
    assert_eq!(
        said("pf-learned-B.CR3"),
        vec![
            "the prefetch of the next frame, pf-learned-B.CR3, skipped: \
             its edit asks for the learned denoiser"
        ]
    );

    let (dir, paths) = dngs("prefetch-open", &["pf-open-A.dng"], 64, 48);
    let mut base = None;
    let frame = match open(&paths[0]).expect("our DNG decodes").0 {
        Input::Raw(f) => f,
        Input::Picture { .. } => panic!("a raw"),
    };
    develop_turned(&frame, &Edit::default(), 0, &mut base);
    let open_one = base.as_ref().expect("a base");
    assert!(matches!(
        prefetch(
            &neighbor_at(&paths[0], Edit::default(), Side::Next),
            Some((paths[0].as_path(), open_one)),
            &mut shelf,
            &mut None,
            None,
            &|_| Ahead::Nothing
        ),
        Fetched::Skipped(why) if why == "its base is the open frame's"
    ));
    assert_eq!(shelf.len(), 0);
    crate::testing::remove_dir_retry(&dir);
}

/// The worker on its thread: an open, and once its develop has landed
/// and nothing else is asked for, the next frame's base and then the
/// previous one's made onto the shelf; the open of the next takes its
/// base from there.
#[test]
fn the_worker_prefetches_the_neighbors_once_idle_and_their_open_takes_the_base() {
    use super::prefetch::said;
    let (dir, paths) = dngs(
        "prefetch-run",
        &["pf-run-A.dng", "pf-run-B.dng", "pf-run-C.dng"],
        96,
        64,
    );
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = Mutex::new(tx);
    let worker = Worker::new(move |o| {
        if let Outcome::Developed { generation, .. } | Outcome::Failed { generation, .. } = o {
            tx.lock().unwrap().send(generation).unwrap();
        }
    });
    let open = |path: &PathBuf, generation: u64| Job::Open {
        path: path.clone(),
        version: None,
        edit: Edit::default(),
        generation,
        seed_blend: false,
        turn: 0,
        stance_known: false,
    };
    let wait = std::time::Duration::from_secs(60);
    worker.send(open(&paths[0], 1));
    worker.neighbors(
        1,
        vec![
            neighbor_at(&paths[1], Edit::default(), Side::Next),
            neighbor_at(&paths[2], Edit::default(), Side::Previous),
        ],
    );
    assert_eq!(rx.recv_timeout(wait).expect("the open lands"), 1);
    let asked = Instant::now();
    let landed = |name: &str| {
        said(name)
            .iter()
            .any(|l| l.starts_with(&format!("prefetched {name}")))
    };
    while !(landed("pf-run-B.dng") && landed("pf-run-C.dng")) {
        assert!(asked.elapsed() < wait, "both prefetched");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let order: Vec<String> = said("pf-run-")
        .into_iter()
        .filter(|l| l.starts_with("prefetching"))
        .collect();
    assert_eq!(
        order,
        vec![
            "prefetching the next frame, pf-run-B.dng",
            "prefetching the previous frame, pf-run-C.dng"
        ]
    );
    assert!(said("pf-run-A.dng").is_empty(), "the open one is not");
    worker.send(open(&paths[1], 2));
    assert_eq!(rx.recv_timeout(wait).expect("the next lands"), 2);
    assert!(
        said("pf-run-B.dng")
            .iter()
            .any(|l| l == "the base of pf-run-B.dng from the shelf")
    );
    worker.stop();
    crate::testing::remove_dir_retry(&dir);
}

/// A worker that opens no frame (an export's from the command line)
/// prefetches nothing, even when handed neighbors.
#[test]
fn a_worker_that_opens_nothing_prefetches_nothing() {
    use super::prefetch::{QUIET, said};
    let (dir, paths) = dngs("prefetch-none", &["pf-none-B.dng"], 64, 48);
    let worker = Worker::new(|_| {});
    worker.neighbors(1, vec![neighbor_at(&paths[0], Edit::default(), Side::Next)]);
    std::thread::sleep(QUIET * 10);
    assert!(said("pf-none-").is_empty());
    worker.stop();
    crate::testing::remove_dir_retry(&dir);
}

/// The worker's prefetch against the real queue, held at a check
/// between two stages while a request is sent: the open of the very
/// frame being made has it go on, and that open lands from the shelf;
/// a newer open, with its own neighbors, has the one in hand give way,
/// and the older list is not taken up again; a panic in a prefetch
/// lets the rest of its list go, and the worker goes on.
#[test]
fn the_worker_s_prefetch_meets_the_requests_of_the_real_queue() {
    use super::prefetch::{AtCheck, QUIET, at_next_check, said};
    let names = [
        "pf-q-A.dng",
        "pf-q-B.dng",
        "pf-q-C.dng",
        "pf-q-D.dng",
        "pf-q-E.dng",
        "pf-q-F.dng",
        "pf-q-G.dng",
        "pf-q-H.dng",
    ];
    let (dir, p) = dngs("prefetch-queue", &names, 96, 64);
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = Mutex::new(tx);
    let worker = Worker::new(move |o| {
        if let Outcome::Developed { generation, .. } | Outcome::Failed { generation, .. } = o {
            tx.lock().unwrap().send(generation).unwrap();
        }
    });
    let wait = std::time::Duration::from_secs(60);
    let open = |path: &PathBuf, generation: u64| {
        worker.send(Job::Open {
            path: path.clone(),
            version: None,
            edit: Edit::default(),
            generation,
            seed_blend: false,
            turn: 0,
            stance_known: false,
        });
    };
    let landed = |generation: u64| {
        assert_eq!(rx.recv_timeout(wait).expect("a develop lands"), generation);
    };
    let until = |what: &str, seen: &dyn Fn() -> bool| {
        let asked = Instant::now();
        while !seen() {
            assert!(asked.elapsed() < wait, "{what}");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    };
    let said_one = |name: &str, line: &str| said(name).iter().any(|l| l.contains(line));
    let hold = |path: &PathBuf| {
        let (at_tx, at_rx) = std::sync::mpsc::channel();
        let (go_tx, go_rx) = std::sync::mpsc::channel();
        at_next_check(path, AtCheck::Hold(at_tx, go_rx));
        (at_rx, go_tx)
    };

    // B's open sent while B's prefetch is held before its prepare.
    let (at, go) = hold(&p[1]);
    open(&p[0], 1);
    worker.neighbors(1, vec![neighbor_at(&p[1], Edit::default(), Side::Next)]);
    landed(1);
    assert_eq!(at.recv_timeout(wait).expect("held"), "prepare");
    open(&p[1], 2);
    go.send(()).unwrap();
    landed(2);
    assert!(said_one(
        "pf-q-B.dng",
        "the prefetch of pf-q-B.dng goes on before its prepare: it is the frame being opened"
    ));
    assert!(said_one(
        "pf-q-B.dng",
        "prefetched pf-q-B.dng, the next frame"
    ));
    assert!(said_one(
        "pf-q-B.dng",
        "the base of pf-q-B.dng from the shelf"
    ));

    // C's prefetch, for B's open, held; E opened with neighbors of its
    // own: C gives way, D is made as E's next and never as B's previous.
    let (at, go) = hold(&p[2]);
    worker.neighbors(
        2,
        vec![
            neighbor_at(&p[2], Edit::default(), Side::Next),
            neighbor_at(&p[3], Edit::default(), Side::Previous),
        ],
    );
    assert_eq!(at.recv_timeout(wait).expect("held"), "prepare");
    open(&p[4], 3);
    worker.neighbors(3, vec![neighbor_at(&p[3], Edit::default(), Side::Next)]);
    go.send(()).unwrap();
    landed(3);
    until("D made for E", &|| {
        said_one("pf-q-D.dng", "prefetched pf-q-D.dng, the next frame")
    });
    assert!(said_one(
        "pf-q-C.dng",
        "the prefetch of the next frame, pf-q-C.dng, abandoned before its prepare"
    ));
    assert!(!said_one("pf-q-C.dng", "prefetched"), "C never made");
    assert!(
        !said_one("pf-q-D.dng", "the previous frame"),
        "B's list let go"
    );

    // G's prefetch panics: H, after it in the list, is let go.
    at_next_check(&p[6], AtCheck::Panic);
    open(&p[5], 4);
    worker.neighbors(
        4,
        vec![
            neighbor_at(&p[6], Edit::default(), Side::Next),
            neighbor_at(&p[7], Edit::default(), Side::Previous),
        ],
    );
    landed(4);
    until("G panicked", &|| {
        said_one("pf-q-G.dng", "the worker panicked")
    });
    std::thread::sleep(QUIET * 20);
    assert!(said("pf-q-H.dng").is_empty(), "H let go");
    open(&p[7], 5);
    landed(5);
    assert!(!said_one("pf-q-H.dng", "from the shelf"));
    worker.stop();
    crate::testing::remove_dir_retry(&dir);
}

/// The editor left for culling or the grid (no frame open): the
/// neighbors waiting are let go, and a prefetch is not begun for them.
#[test]
fn leaving_the_editor_lets_the_neighbors_go() {
    let worker = Worker::new(|_| {});
    // A frame open whose develop never lands: the list waits for it.
    worker.set_open_frame(Some(Frame::file("/nowhere/pf-left-A.CR3")));
    worker.neighbors(
        1,
        vec![neighbor_at(
            std::path::Path::new("/nowhere/pf-left-B.CR3"),
            Edit::default(),
            Side::Next,
        )],
    );
    assert!(worker.queue.0.lock().unwrap().neighbors.is_some());
    worker.set_open_frame(None);
    assert!(worker.queue.0.lock().unwrap().neighbors.is_none());
    worker.stop();
}

/// The frame still open when the editor comes back from culling or the
/// grid has its neighbors sent again: taken for the open whose develop
/// landed last, on that file, or in place of that open's own list, and
/// never over another open's; the worker says which open that is once
/// its develop lands.
#[test]
fn the_neighbors_come_back_with_the_open_frame() {
    let (dir, paths) = dngs("prefetch-back", &["pf-back-A.dng"], 64, 48);
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = Mutex::new(tx);
    let worker = Worker::new(move |o| {
        if let Outcome::Developed { generation, .. } | Outcome::Failed { generation, .. } = o {
            tx.lock().unwrap().send(generation).unwrap();
        }
    });
    let b = || {
        vec![neighbor_at(
            std::path::Path::new("/nowhere/pf-back-B.CR3"),
            Edit::default(),
            Side::Next,
        )]
    };
    // Nothing landed yet: nothing taken.
    assert!(!worker.neighbors_again(7, &paths[0], b(), "a test"));
    assert!(worker.queue.0.lock().unwrap().neighbors.is_none());
    worker.send(Job::Open {
        path: paths[0].clone(),
        version: None,
        edit: Edit::default(),
        generation: 7,
        seed_blend: false,
        turn: 0,
        stance_known: false,
    });
    let wait = std::time::Duration::from_secs(60);
    assert_eq!(rx.recv_timeout(wait).expect("the open lands"), 7);
    assert_eq!(
        worker.queue.0.lock().unwrap().landed,
        Some((7, paths[0].clone()))
    );
    // Away to the grid: let go. Back on another file: not taken.
    worker.set_open_frame(None);
    assert!(!worker.neighbors_again(7, std::path::Path::new("/nowhere/other.CR3"), b(), "a test"));
    assert!(worker.queue.0.lock().unwrap().neighbors.is_none());
    // Back on A: taken under A's open.
    worker.set_open_frame(Some(Frame::file(paths[0].clone())));
    assert!(worker.neighbors_again(7, &paths[0], b(), "a test"));
    {
        let q = worker.queue.0.lock().unwrap();
        let n = q.neighbors.as_ref().expect("sent again");
        assert_eq!(n.generation, 7);
    }
    // Sent again for the same open (the filter or the order moved):
    // its list, waiting or done, is replaced.
    assert!(worker.neighbors_again(7, &paths[0], Vec::new(), "a test"));
    // A list for another open waiting already is not replaced.
    worker.neighbors(9, b());
    assert!(!worker.neighbors_again(7, &paths[0], b(), "a test"));
    assert_eq!(
        worker
            .queue
            .0
            .lock()
            .unwrap()
            .neighbors
            .as_ref()
            .map(|n| n.generation),
        Some(9)
    );
    worker.stop();
    crate::testing::remove_dir_retry(&dir);
}

/// A neighbor's edit changed after its base was prefetched (a sync, a
/// paste): a change after the base keeps the base on the shelf, and the
/// neighbor sent again under it is not made twice; a change to the base
/// drops it, said, and the neighbor sent again under the new edit is
/// made afresh at the next idle, which its open then takes.
#[test]
fn a_neighbor_whose_edit_changed_is_dropped_and_made_again() {
    use super::prefetch::said;
    let (dir, paths) = dngs("prefetch-edit", &["pf-ed-A.dng", "pf-ed-B.dng"], 96, 64);
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = Mutex::new(tx);
    let worker = Worker::new(move |o| {
        if let Outcome::Developed { generation, .. } | Outcome::Failed { generation, .. } = o {
            tx.lock().unwrap().send(generation).unwrap();
        }
    });
    let wait = std::time::Duration::from_secs(60);
    let until = |what: &str, done: &dyn Fn() -> bool| {
        let asked = Instant::now();
        while !done() {
            assert!(asked.elapsed() < wait, "{what}");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    };
    let count = |starts: &str| {
        said("pf-ed-B.dng")
            .iter()
            .filter(|l| l.starts_with(starts))
            .count()
    };
    let open = |path: &PathBuf, edit: Edit, generation: u64| Job::Open {
        path: path.clone(),
        version: None,
        edit,
        generation,
        seed_blend: false,
        turn: 0,
        stance_known: false,
    };
    worker.set_open_frame(Some(Frame::file(paths[0].clone())));
    worker.send(open(&paths[0], Edit::default(), 1));
    worker.neighbors(1, vec![neighbor_at(&paths[1], Edit::default(), Side::Next)]);
    assert_eq!(rx.recv_timeout(wait).expect("the open lands"), 1);
    until("B prefetched", &|| count("prefetched pf-ed-B.dng") == 1);

    // After the base: kept, and not made again.
    let mut brighter = Edit::default();
    brighter.light.exposure = 1.0;
    worker.edit_changed(paths[1].clone(), Some(holds(&brighter)));
    assert!(worker.neighbors_again(
        1,
        &paths[0],
        vec![neighbor_at(&paths[1], brighter, Side::Next)],
        "a neighbor's edit changed"
    ));
    until("the resent B skipped", &|| {
        said("pf-ed-B.dng")
            .iter()
            .any(|l| l.ends_with("skipped: its base is on the shelf"))
    });
    assert_eq!(count("shelved base of pf-ed-B.dng dropped"), 0);

    // The base itself: dropped, and made again under the new edit.
    let warmer = Edit {
        white_balance: greycard_edit::WhiteBalance::Custom {
            temperature: 6500.0,
            tint: 0.0,
        },
        ..Edit::default()
    };
    worker.edit_changed(paths[1].clone(), Some(holds(&warmer)));
    until("B's base dropped", &|| {
        said("pf-ed-B.dng")
            .iter()
            .any(|l| l == "shelved base of pf-ed-B.dng dropped: edit changed")
    });
    assert!(worker.neighbors_again(
        1,
        &paths[0],
        vec![neighbor_at(&paths[1], warmer.clone(), Side::Next)],
        "a neighbor's edit changed"
    ));
    until("B prefetched again", &|| {
        count("prefetched pf-ed-B.dng") == 2
    });
    worker.set_open_frame(Some(Frame::file(paths[1].clone())));
    worker.send(open(&paths[1], warmer, 2));
    assert_eq!(rx.recv_timeout(wait).expect("B's open lands"), 2);
    assert_eq!(count("the base of pf-ed-B.dng from the shelf"), 1);
    worker.stop();
    crate::testing::remove_dir_retry(&dir);
}

/// What a file holds now, as the window says it after a change: `edit`
/// alone, unturned.
fn holds(edit: &Edit) -> FileNow {
    FileNow {
        edits: vec![edit.clone()],
        turn: 0,
    }
}

/// A neighbor's edit changed while its prefetch is in hand: a change
/// after the base does not stop it; a change to the base has it give
/// way, and it is let go rather than put back on the list the window
/// sent again under the new edit, so only the new base is made, and
/// its open takes it.
#[test]
fn a_prefetch_whose_edit_changes_under_it_gives_way_and_is_let_go() {
    use super::prefetch::{AtCheck, at_next_check, said};
    let (dir, paths) = dngs("prefetch-moved", &["pf-mv-A.dng", "pf-mv-B.dng"], 96, 64);
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = Mutex::new(tx);
    let worker = Worker::new(move |o| {
        if let Outcome::Developed { generation, .. } | Outcome::Failed { generation, .. } = o {
            tx.lock().unwrap().send(generation).unwrap();
        }
    });
    let wait = std::time::Duration::from_secs(60);
    let until = |what: &str, done: &dyn Fn() -> bool| {
        let asked = Instant::now();
        while !done() {
            assert!(asked.elapsed() < wait, "{what}");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    };
    let count = |starts: &str| {
        said("pf-mv-B.dng")
            .iter()
            .filter(|l| l.starts_with(starts))
            .count()
    };
    let hold = |path: &PathBuf| {
        let (at_tx, at_rx) = std::sync::mpsc::channel();
        let (go_tx, go_rx) = std::sync::mpsc::channel();
        at_next_check(path, AtCheck::Hold(at_tx, go_rx));
        (at_rx, go_tx)
    };
    let open = |path: &PathBuf, edit: Edit, generation: u64| Job::Open {
        path: path.clone(),
        version: None,
        edit,
        generation,
        seed_blend: false,
        turn: 0,
        stance_known: false,
    };
    let warmer = Edit {
        white_balance: greycard_edit::WhiteBalance::Custom {
            temperature: 6500.0,
            tint: 0.0,
        },
        ..Edit::default()
    };
    let mut brighter = Edit::default();
    brighter.light.exposure = 1.0;

    // Held before its prepare, an exposure synced onto it: it goes on.
    let (at, go) = hold(&paths[1]);
    worker.set_open_frame(Some(Frame::file(paths[0].clone())));
    worker.send(open(&paths[0], Edit::default(), 1));
    worker.neighbors(1, vec![neighbor_at(&paths[1], Edit::default(), Side::Next)]);
    assert_eq!(rx.recv_timeout(wait).expect("the open lands"), 1);
    assert_eq!(at.recv_timeout(wait).expect("held"), "prepare");
    worker.edit_changed(paths[1].clone(), Some(holds(&brighter)));
    go.send(()).unwrap();
    until("B prefetched", &|| count("prefetched pf-mv-B.dng") == 1);
    assert_eq!(
        count("the prefetch of the next frame, pf-mv-B.dng, abandoned"),
        0
    );

    // Its base dropped by a white balance pasted onto it, and B sent
    // again under it; the new prefetch held, and a second paste, back
    // to the default, comes meanwhile: it gives way and is let go.
    worker.edit_changed(paths[1].clone(), Some(holds(&warmer)));
    until("B's base dropped", &|| {
        count("shelved base of pf-mv-B.dng dropped") == 1
    });
    let (at, go) = hold(&paths[1]);
    assert!(worker.neighbors_again(
        1,
        &paths[0],
        vec![neighbor_at(&paths[1], warmer.clone(), Side::Next)],
        "a neighbor's edit changed"
    ));
    assert_eq!(at.recv_timeout(wait).expect("held"), "prepare");
    worker.edit_changed(paths[1].clone(), Some(holds(&Edit::default())));
    assert!(worker.neighbors_again(
        1,
        &paths[0],
        vec![neighbor_at(&paths[1], Edit::default(), Side::Next)],
        "a neighbor's edit changed"
    ));
    go.send(()).unwrap();
    until("B made again under the default", &|| {
        count("prefetched pf-mv-B.dng") == 2
    });
    assert_eq!(
        count("the prefetch of the next frame, pf-mv-B.dng, abandoned before its prepare"),
        1
    );
    assert_eq!(
        count("the prefetch of pf-mv-B.dng let go: its edit changed"),
        1
    );
    // Not made a third time under the warmer edit it gave way under.
    std::thread::sleep(super::prefetch::QUIET * 20);
    assert_eq!(count("prefetched pf-mv-B.dng"), 2);
    worker.set_open_frame(Some(Frame::file(paths[1].clone())));
    worker.send(open(&paths[1], Edit::default(), 2));
    assert_eq!(rx.recv_timeout(wait).expect("B's open lands"), 2);
    assert_eq!(count("the base of pf-mv-B.dng from the shelf"), 1);
    worker.stop();
    crate::testing::remove_dir_retry(&dir);
}

/// A device lost under a develop's ops (destroyed here, as a driver's
/// reset loses it) costs no panic: each op's error sends it to the
/// CPU, the context goes, and the develop lands a picture made there.
#[test]
fn a_develop_on_a_lost_device_lands_on_the_cpu() {
    let Some(ctx) = greycard_gpu::Context::own().ok() else {
        skipped("the lost device's develop");
        return;
    };
    ctx.device().destroy();
    let frame = Arc::new(aberrated_frame(400, 320));
    let mut gpu = Some(ctx);
    let mut base = None;
    let (outcome, image) = develop_once(&frame, &mut base, &mut gpu);
    assert!(
        matches!(
            outcome,
            Outcome::Developed {
                image: Developed::Halves(_),
                ..
            }
        ),
        "a picture from the CPU"
    );
    assert!(image.is_some());
    assert!(gpu.is_none(), "the lost context let go");
    assert!(!base.as_ref().unwrap().on_gpu);
}

/// A loss found after the develop landed (in a save's read back, by
/// the window's callback): the context goes, and the develop whose
/// texture is on the lost device is asked again, under its own
/// generation, which then lands on the CPU. Told the editor starts
/// again, nothing is asked again.
#[test]
fn a_loss_after_the_develop_develops_it_again_on_the_cpu() {
    for again in [true, false] {
        let Some(ctx) = greycard_gpu::Context::own().ok() else {
            skipped("the develop again after a loss");
            return;
        };
        let frame = Arc::new(aberrated_frame(400, 320));
        let mut gpu = Some(ctx);
        let mut base = None;
        let (outcome, _) = develop_once(&frame, &mut base, &mut gpu);
        let mut shown = ShownTexture::of(&Edit::default(), 0, &outcome, base.as_ref());
        assert!(shown.is_some(), "the develop left a texture on the device");
        gpu.as_ref().unwrap().device().destroy();
        let mut last = None;
        let redo = super::engine::lose_gpu(&mut gpu, &mut base, &mut shown, &mut last, again);
        assert!(gpu.is_none() && shown.is_none());
        let b = base.as_ref().unwrap();
        assert!(!b.kept.has_upload() && !b.kept.on_device(Keep::PreSharpen));
        if !again {
            assert!(redo.is_none());
            continue;
        }
        let Some(Job::Develop {
            edit,
            generation,
            turn,
        }) = redo
        else {
            panic!("the develop asked again");
        };
        assert_eq!((generation, turn), (1, 0));
        assert!(edit == Edit::default());
        let (outcome, _) = develop_once(&frame, &mut base, &mut gpu);
        assert!(matches!(
            outcome,
            Outcome::Developed {
                image: Developed::Halves(_),
                generation: 1,
                ..
            }
        ));
    }
}

/// A mask the worker asks for again behind a develop does not take the
/// place of one the panel queued for the same key, which is newer.
#[test]
fn a_mask_asked_again_leaves_a_queued_ask_of_the_same_key() {
    let lock = Mutex::new(Queue::default());
    let sky = Shape::Sky { picks: Vec::new() };
    lock.lock()
        .unwrap()
        .masks
        .push_back(((1, 0), sky.clone(), None));
    super::engine::ask_again(
        &lock,
        vec![
            ((1, 0), Shape::Subject {}, None),
            ((2, 0), Shape::Subject {}, None),
        ],
    );
    let q = lock.lock().unwrap();
    let queued: Vec<_> = q.masks.iter().map(|(k, s, _)| (*k, s.clone())).collect();
    assert_eq!(queued, vec![((1, 0), sky), ((2, 0), Shape::Subject {})]);
}

/// The models see the base: the mask cache's reduced develop of an
/// edit with a retouch (a Fill would refuse it, a heal would put the
/// patched picture's raster in the base's slot) is, to the bit, the
/// reduced develop of the same edit without its retouch, through the
/// Detail section.
#[test]
fn the_mask_cache_develops_without_the_retouch() {
    use crate::stages::Purpose;
    let dir = crate::testing::scratch_dir("models-see");
    let path = dir.join("frame.dng");
    crate::testing::linear_dng(&path, 96, 64, "Canon", "Canon EOS R5");
    let mut edit = Edit::default();
    edit.light.exposure = 0.7;
    edit.detail.clarity = 0.4;
    edit.retouch.patches.push(greycard_edit::retouch::Patch {
        id: 1,
        points: vec![[0.3, 0.3]],
        radius: 0.05,
        ..Default::default()
    });
    edit.retouch.patches.push(greycard_edit::retouch::Patch {
        id: 2,
        method: greycard_edit::retouch::Method::Fill,
        points: vec![[0.5, 0.3]],
        radius: 0.05,
        ..Default::default()
    });
    let seen = crate::reduced::develop_as(&path, &edit, 0, None, &Purpose::ModelsSee)
        .expect("the models' picture develops");
    let bare = Edit {
        retouch: Default::default(),
        ..edit
    };
    let reduced = crate::reduced::develop_as(&path, &bare, 0, None, &Purpose::Reduced)
        .expect("the bare edit develops reduced");
    assert_eq!((seen.image.width, seen.image.height), (96, 64));
    assert!(seen.image.data == reduced.image.data);
    assert_eq!((seen.full, seen.kind), (reduced.full, reduced.kind));
    crate::testing::remove_dir_retry(&dir);
}

/// A retouch as the develop applied it is the edit's when the only
/// difference is a source the develop chose.
#[test]
fn a_retouch_applied_is_the_edits_with_the_sources_chosen() {
    use greycard_edit::retouch::Patch;
    let heal = Patch {
        id: 1,
        points: vec![[0.2, 0.2]],
        ..Default::default()
    };
    let edit = Retouch {
        enabled: true,
        patches: vec![heal.clone()],
    };
    let applied = Retouch {
        enabled: true,
        patches: vec![Patch {
            source: Some([0.1, 0.0]),
            ..heal.clone()
        }],
    };
    assert!(masks::same_retouch(&edit, &applied));
    let moved = Retouch {
        enabled: true,
        patches: vec![Patch {
            points: vec![[0.3, 0.2]],
            source: Some([0.1, 0.0]),
            ..heal
        }],
    };
    assert!(!masks::same_retouch(&edit, &moved));
    assert!(!masks::same_retouch(&edit, &Retouch::default()));
}

/// A light field with a dark bar down its middle, and an edit that
/// clones the bar away from the field beside it.
fn bar_and_its_clone() -> (Arc<WorkingImage>, Edit) {
    let n = 256;
    let mut image = WorkingImage::new(n, n);
    for (i, px) in image.data.as_chunks_mut::<3>().0.iter_mut().enumerate() {
        let dark = (120..136).contains(&(i % n));
        px.fill(if dark { 0.02 } else { 0.4 });
    }
    let edit = Edit {
        retouch: Retouch {
            enabled: true,
            patches: vec![greycard_edit::retouch::Patch {
                id: 1,
                method: greycard_edit::retouch::Method::Clone,
                points: vec![[0.5, 0.05], [0.5, 0.95]],
                radius: 0.06,
                source: Some([0.25, 0.0]),
                ..Default::default()
            }],
        },
        ..Default::default()
    };
    (Arc::new(image), edit)
}

/// The guide plane is the retouched picture's: a dark bar cloned
/// away leaves no trace in it for the tone sliders to lift, and an
/// unchanged retouch keeps the plane rather than making it again.
#[test]
fn the_guide_is_made_after_the_retouch() {
    let (image, edit) = bar_and_its_clone();
    let input = picture_input(&image);
    let mut base = None;
    develop_picture(&input, &Edit::default(), &mut base, &mut None);
    let before = base.as_ref().unwrap().guide.clone();
    let (bar, beside) = ((128.0, 128.0), (200.0, 128.0));
    assert!(
        before.at(beside.0, beside.1) - before.at(bar.0, bar.1) > 1.0,
        "the bar is in the base's guide"
    );
    develop_picture(&input, &edit, &mut base, &mut None);
    let b = base.as_ref().unwrap();
    let patched = b.kept.patched().expect("the retouch is kept").image;
    let px = &patched.data[(128 * 256 + 128) * 3..][..3];
    assert!((px[1] - 0.4).abs() < 1e-3, "the clone took the bar: {px:?}");
    let at = (b.guide.at(bar.0, bar.1), b.guide.at(beside.0, beside.1));
    assert!(
        (at.0 - at.1).abs() < 0.02,
        "the bar's ghost in the guide: {at:?}"
    );
    let made = b.guide.clone();
    develop_picture(&input, &edit, &mut base, &mut None);
    assert!(Arc::ptr_eq(&made, &base.as_ref().unwrap().guide));
}

/// Clearing the retouch gives the guide back to the base.
#[test]
fn a_cleared_retouch_brings_back_the_bases_guide() {
    let (image, edit) = bar_and_its_clone();
    let input = picture_input(&image);
    let mut base = None;
    develop_picture(&input, &Edit::default(), &mut base, &mut None);
    let own = base.as_ref().unwrap().guide.data.clone();
    develop_picture(&input, &edit, &mut base, &mut None);
    assert_ne!(base.as_ref().unwrap().guide.data, own);
    develop_picture(&input, &Edit::default(), &mut base, &mut None);
    let b = base.as_ref().unwrap();
    assert!(b.kept.patched().is_none());
    assert_eq!(b.guide.data, own);
}

/// What the open file's develop and finish write for `input` under
/// `edit` at `turn`, and what a set frame's walk of its stage list
/// writes: each picture's pixels with what it went without, with no
/// model in the store for either.
fn develop_and_walk(
    what: &str,
    input: &Input,
    edit: &Edit,
    turn: u8,
) -> (
    (greycard_render::export::Rendered, Vec<String>),
    (greycard_render::export::Rendered, Vec<String>),
) {
    let empty = crate::testing::scratch_dir(what).join("models");
    std::fs::create_dir_all(&empty).unwrap();
    let settings = greycard_render::export::Settings {
        format: greycard_render::export::Format::Tiff,
        ..Default::default()
    };
    let deliver: Deliver = Arc::new(|_| {});
    let mut ai = Ai::for_test(greycard_ai::Store::at(&empty), Vec::new());
    let mut base = None;
    let (outcome, image) = develop_job(
        input, edit, turn, 0, &mut base, &mut ai, None, None, &mut None, &deliver,
    );
    let mut said = left_out_of(&outcome, base.as_ref());
    let (mut developed, masks) =
        finish_export(image.unwrap(), edit, base.as_ref(), &mut ai, &settings);
    greycard_render::export::border(&mut developed, &settings);
    greycard_render::export::mark(&mut developed, &settings).unwrap();
    said.extend(masks);
    let mut ai = Ai::for_test(greycard_ai::Store::at(&empty), Vec::new());
    let walked = super::export::walk_export(
        input.clone(),
        edit,
        turn,
        &mut super::base::WorkerModels::new(&mut ai, None, &deliver, 0),
        None,
        &settings,
    )
    .unwrap();
    crate::testing::remove_dir_retry(empty.parent().unwrap());
    ((developed, said), walked)
}

fn same_pixels(a: &greycard_render::export::Rendered, b: &greycard_render::export::Rendered) {
    use greycard_render::export::Pixels;
    assert_eq!((a.width, a.height), (b.width, b.height));
    match (&a.pixels, &b.pixels) {
        (Pixels::Sixteen(a), Pixels::Sixteen(b)) => assert!(a == b, "the pixels differ"),
        (Pixels::Eight(a), Pixels::Eight(b)) => assert!(a == b, "the pixels differ"),
        _ => panic!("one picture is 8-bit and the other 16"),
    }
}

/// An edit that asks for a learned denoiser, a Fill, and a learned
/// mask, none of whose models is in the store, beside a Heal, a
/// Detail of the edit's and the mask's own, a mask refined against the
/// picture, the sharpen, and a straightened crop.
fn edit_without_its_models() -> Edit {
    use greycard_edit::mask::Component;
    use greycard_edit::retouch::{Method, Patch};
    let mut edit = Edit::default();
    edit.noise.learned = greycard_edit::Learned::Fast;
    edit.noise.learned_strength = 0.6;
    edit.detail.clarity = 0.3;
    edit.geometry.angle = 2.5;
    edit.geometry.crop = Some(greycard_edit::geometry::Crop {
        x: 0.1,
        y: 0.05,
        w: 0.7,
        h: 0.8,
    });
    edit.retouch.patches = vec![
        Patch {
            id: 21,
            method: Method::Fill,
            points: vec![[0.5, 0.5]],
            radius: 0.08,
            ..Patch::default()
        },
        Patch {
            id: 22,
            method: Method::Heal,
            points: vec![[0.3, 0.6]],
            radius: 0.05,
            ..Patch::default()
        },
    ];
    let mut look = greycard_edit::Look::default();
    look.light.exposure = 0.8;
    look.detail.clarity = 0.5;
    edit.adjustments.push(greycard_edit::Adjustment {
        id: 11,
        name: "Face".into(),
        mask: greycard_edit::mask::Mask {
            components: vec![Component {
                shape: Shape::Subject {},
                ..Component::default()
            }],
            invert: false,
            ..Default::default()
        },
        look,
        ..greycard_edit::Adjustment::default()
    });
    // A mask refined against the picture: a hard edge across the frame
    // and a luminance window, its Refine edge on.
    let mut look = greycard_edit::Look::default();
    look.light.exposure = -0.7;
    edit.adjustments.push(greycard_edit::Adjustment {
        id: 12,
        name: "Refined".into(),
        mask: greycard_edit::mask::Mask {
            components: vec![
                Component {
                    shape: Shape::Linear {
                        from: [0.55, 0.3],
                        to: [0.549, 0.3],
                    },
                    ..Component::default()
                },
                Component {
                    shape: Shape::LUMINANCE,
                    mode: greycard_edit::mask::Mode::Intersect,
                    ..Component::default()
                },
            ],
            refine: 0.5,
            ..Default::default()
        },
        look,
        ..greycard_edit::Adjustment::default()
    });
    edit
}

/// A set frame's export walks its stage list and writes what the open
/// file's develop and finish write: the same pixels, and the same words
/// for what it went without, the engine's denoise standing in for the
/// learned one, the Fill left as it was, and the mask written without
/// its shape; the refined mask refined on both. At each quarter turn,
/// the crop straightened.
#[test]
fn a_set_frame_s_walk_writes_what_the_develop_writes() {
    let frame = Arc::new(aberrated_frame(160, 120));
    let edit = edit_without_its_models();
    for turn in 0..4 {
        let ((developed, said), (walked, left_out)) =
            develop_and_walk("export-walk", &Input::Raw(frame.clone()), &edit, turn);
        same_pixels(&developed, &walked);
        assert_eq!(left_out, said);
        assert_eq!(left_out.len(), 3, "{left_out:?}");
        assert!(
            left_out[0].starts_with("the learned denoiser"),
            "{}",
            left_out[0]
        );
        assert!(left_out[1].starts_with("fill Fill 21: "), "{}", left_out[1]);
        assert!(
            left_out[2].starts_with("Face's Subject shape: "),
            "{}",
            left_out[2]
        );
        // The refined mask is refined on both: unrefined, another picture.
        if turn == 0 {
            let mut plain = edit.clone();
            plain.adjustments[1].mask.refine = 0.0;
            let ((unrefined, _), _) =
                develop_and_walk("export-walk-plain", &Input::Raw(frame.clone()), &plain, 0);
            use greycard_render::export::Pixels;
            let differs = match (&unrefined.pixels, &walked.pixels) {
                (Pixels::Sixteen(a), Pixels::Sixteen(b)) => a != b,
                (Pixels::Eight(a), Pixels::Eight(b)) => a != b,
                _ => true,
            };
            assert!(differs, "the refine did nothing");
        }
    }
}

/// An X-Trans mosaic has no learned denoiser: the walk says so in the
/// develop's words, and writes its picture.
#[test]
fn a_set_frame_s_walk_says_the_x_trans_has_no_learned_denoiser() {
    let mut frame = aberrated_frame(168, 120);
    frame.layout = greycard_core::raw::SensorLayout::Cfa(greycard_core::raw::CfaPattern::xtrans());
    let mut edit = Edit::default();
    edit.noise.learned = greycard_edit::Learned::Fast;
    let ((developed, said), (walked, left_out)) =
        develop_and_walk("export-walk-xtrans", &Input::Raw(Arc::new(frame)), &edit, 0);
    same_pixels(&developed, &walked);
    assert_eq!(left_out, said);
    assert_eq!(
        left_out,
        ["the denoise: the learned denoiser has no X-Trans version yet; none ran"]
    );
}

/// A picture file walks as its develop does: turned, cropped, its
/// retouch and its mask, with no capture sharpen.
#[test]
fn a_set_picture_s_walk_writes_what_the_develop_writes() {
    let frame = Arc::new(aberrated_frame(160, 120));
    let mut base = None;
    let (_, image) = develop_once(&frame, &mut base, &mut None);
    let input = picture_input(&image.unwrap());
    let mut edit = edit_without_its_models();
    edit.noise.learned = greycard_edit::Learned::Off;
    edit.sharpen.enabled = false;
    for turn in [0, 3] {
        let ((developed, said), (walked, left_out)) =
            develop_and_walk("export-walk-picture", &input, &edit, turn);
        same_pixels(&developed, &walked);
        assert_eq!(left_out, said);
        assert_eq!(left_out.len(), 2, "{left_out:?}");
    }
}

/// An edit that refines one mask, a luminance window of 60 to 80 over
/// the frame, with Dehaze at `dehaze` and no capture sharpen, so the
/// picture the develop hands the finish is the one before the sharpen.
fn refining(dehaze: f32) -> Edit {
    use greycard_edit::mask::Component;
    let mut edit = Edit::default();
    edit.sharpen.enabled = false;
    edit.detail.dehaze = dehaze;
    edit.adjustments.push(greycard_edit::Adjustment {
        id: 3,
        mask: greycard_edit::mask::Mask {
            components: vec![Component {
                shape: Shape::Luminance {
                    low: 0.6,
                    high: 0.8,
                    low_feather: 0.05,
                    high_feather: 0.05,
                },
                ..Component::default()
            }],
            refine: 0.01,
            ..Default::default()
        },
        ..greycard_edit::Adjustment::default()
    });
    edit
}

/// The develop of an edit that refines a mask hands the window the
/// picture its refined masks are made against, made from the picture
/// the finish's range shapes read, after the Dehaze: the same picture
/// on the masks' grid, where the patched picture before the Dehaze is
/// another. And a develop that refines nothing makes none and keeps
/// none.
#[test]
fn the_refined_masks_picture_is_the_one_the_range_shapes_read() {
    let frame = Arc::new(aberrated_frame(160, 120));
    let mut base = None;
    let deliver: Deliver = Arc::new(|_| {});
    let edit = refining(0.4);
    let (outcome, image) = develop_job(
        &Input::Raw(frame.clone()),
        &edit,
        0,
        1,
        &mut base,
        &mut Ai::new(),
        None,
        None,
        &mut None,
        &deliver,
    );
    let Outcome::Developed { guide, .. } = outcome else {
        panic!("developed")
    };
    let picture = guide.picture.clone().expect("a picture beside the guide");
    let developed = image.unwrap();
    assert!(*picture == greycard_render::refine::mask_picture(&developed));
    let b = base.as_ref().unwrap();
    let patched = b.patched_picture();
    assert!(*picture != greycard_render::refine::mask_picture(&patched));
    assert!(b.mask_picture.is_some());
    // The open frame's own pictures (`keep`) are made against it too.
    assert!(masks::kept_picture(b, &edit).is_some());
    // A Luminance shape reads the same lightness in the picture refined
    // against as the finish reads in the developed one, on average over
    // each texel, to the half float's rounding.
    let (w, h) = (developed.width, developed.height);
    let texel = |gx: usize, gy: usize| picture.lab[gy * picture.width + gx][0].to_f32();
    let (gx, gy) = (picture.width / 2, picture.height / 3);
    let (x, y) = (gx * w / picture.width, gy * h / picture.height);
    let read = greycard_render::finish::oklab(developed.pixel(x, y))[0];
    assert!(
        (texel(gx, gy) - read).abs() < 3e-3,
        "{} {read}",
        texel(gx, gy)
    );
    // Refining nothing: no picture made, and the kept one let go.
    let mut plain = edit.clone();
    plain.adjustments[0].mask.refine = 0.0;
    let (outcome, _) = develop_job(
        &Input::Raw(frame),
        &plain,
        0,
        2,
        &mut base,
        &mut Ai::new(),
        None,
        None,
        &mut None,
        &deliver,
    );
    let Outcome::Developed { guide, .. } = outcome else {
        panic!("developed")
    };
    assert!(guide.picture.is_none());
    assert!(base.as_ref().unwrap().mask_picture.is_none());
    // A save of the refining edit with no develop between (a Refine
    // edge moved from 0, `keep_open`) makes it from the base in hand:
    // the picture the develop made, and kept for the next.
    let b = base.as_mut().unwrap();
    let made = masks::picture_for(b, &edit, None).expect("made for the save");
    assert!(*made == *picture);
    assert!(masks::kept_picture(b, &edit).is_some());
    assert!(masks::picture_for(b, &plain, None).is_none());
}

/// On the CPU a move after the picture before the sharpen (the
/// sharpen itself) taps the refined masks' picture no more: the one
/// kept is handed out again.
#[test]
fn a_sharpen_move_keeps_the_refined_masks_picture() {
    let frame = Arc::new(aberrated_frame(160, 120));
    let deliver: Deliver = Arc::new(|_| {});
    let mut base = None;
    let mut develop = |edit: &Edit| {
        let (outcome, _) = develop_job(
            &Input::Raw(frame.clone()),
            edit,
            0,
            1,
            &mut base,
            &mut Ai::new(),
            None,
            None,
            &mut None,
            &deliver,
        );
        let Outcome::Developed { guide, .. } = outcome else {
            panic!("developed")
        };
        guide.picture.clone().expect("a picture beside the guide")
    };
    let mut edit = refining(0.4);
    edit.sharpen.enabled = true;
    let first = develop(&edit);
    edit.sharpen.iterations += 1;
    assert!(Arc::ptr_eq(&develop(&edit), &first), "kept");
    edit.detail.dehaze = 0.6;
    assert!(
        !Arc::ptr_eq(&develop(&edit), &first),
        "a Dehaze move taps it again"
    );
}

/// A set frame's walk makes the refined masks' picture after the
/// Dehaze, as the develop does: the two export the same pixels with
/// Dehaze on, and a walk that refines nothing has no such step.
#[test]
fn a_walk_refines_against_the_picture_after_the_dehaze() {
    let frame = Arc::new(aberrated_frame(160, 120));
    let edit = refining(0.4);
    let ((developed, _), (walked, _)) =
        develop_and_walk("export-walk-refined", &Input::Raw(frame), &edit, 0);
    same_pixels(&developed, &walked);
    let facts = Input::Raw(Arc::new(aberrated_frame(160, 120)));
    let purpose = crate::stages::Purpose::Export(Box::default());
    let has_tap = |edit: &Edit| {
        let input = match &facts {
            Input::Raw(f) => crate::walk::Input::Raw(f.clone()),
            Input::Picture { .. } => unreachable!(),
        };
        crate::stages::stages(edit, &purpose, &input.facts(0), None)
            .steps
            .iter()
            .any(|s| matches!(s, crate::stages::Step::Tap(crate::stages::Tap::MaskPicture)))
    };
    assert!(has_tap(&edit));
    let mut plain = edit.clone();
    plain.adjustments[0].mask.refine = 0.0;
    assert!(!has_tap(&plain));
}

/// The window's ask for the refined masks' picture is answered for the
/// develop it names; with no picture open, with none.
#[test]
fn an_ask_for_the_refined_masks_picture_is_answered() {
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = Mutex::new(tx);
    let worker = Worker::new(move |o| {
        if let Outcome::MaskPicture {
            generation,
            picture,
        } = o
        {
            tx.lock()
                .unwrap()
                .send((generation, picture.is_some()))
                .unwrap();
        }
    });
    worker.send(Job::MaskPicture {
        edit: Box::new(refining(0.0)),
        generation: 7,
    });
    let answer = rx
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("answered");
    assert_eq!(answer, (7, false));
    assert!(worker.stop());
}

/// The edit's pictures of a frame are refined as its export is: a
/// refined mask's reduced develop carries the picture beside its
/// guide, and the picture made from it is another than the same edit's
/// with the mask unrefined.
#[test]
fn the_edits_pictures_are_refined() {
    use greycard_edit::mask::Component;
    let dir = crate::testing::scratch_dir("refined-pictures");
    let path = dir.join("frame.dng");
    crate::testing::linear_dng(&path, 96, 64, "Canon", "Canon EOS R5");
    let mut look = greycard_edit::Look::default();
    look.light.exposure = 1.5;
    let mut edit = Edit::default();
    edit.adjustments.push(greycard_edit::Adjustment {
        id: 4,
        mask: greycard_edit::mask::Mask {
            components: vec![Component {
                shape: Shape::Linear {
                    from: [0.51, 0.3],
                    to: [0.5, 0.3],
                },
                ..Component::default()
            }],
            refine: 0.4,
            ..Default::default()
        },
        look,
        ..greycard_edit::Adjustment::default()
    });
    let picture = |edit: &Edit| {
        let developed = crate::reduced::develop(&path, edit, 0, None).expect("reduced");
        let finish = crate::edited_pictures::Finish {
            edit: edit.clone(),
            turn: 0,
            source: (developed.image.width as u32, developed.image.height as u32),
            full: developed.full,
            rasters: Default::default(),
            clip_level: developed.clip_level,
            guide: Some(developed.guide.clone()),
            kind: developed.kind,
            white: developed.white.clone(),
            detail: developed.detail.clone(),
        };
        (
            developed.guide.picture.is_some(),
            crate::edited_pictures::make(&developed.image, &finish).big,
        )
    };
    let (with, refined) = picture(&edit);
    let mut plain = edit.clone();
    plain.adjustments[0].mask.refine = 0.0;
    let (without, unrefined) = picture(&plain);
    assert!(with && !without);
    assert!(refined != unrefined, "the refine changed nothing");
    crate::testing::remove_dir_retry(&dir);
}

/// With the GPU, the refined masks' picture is read back from the
/// device's picture before the sharpen, a band at a time, and not made
/// again on the CPU: the viewport's texture with the sharpen off, the
/// kept picture before it with the sharpen on. Each is the CPU
/// develop's within the GPU's Detail and the half floats' rounding.
#[test]
fn with_the_gpu_the_refined_masks_picture_is_read_back() {
    let Some(ctx) = greycard_gpu::Context::own().ok() else {
        skipped("the refined masks' picture on the GPU");
        return;
    };
    let frame = Arc::new(aberrated_frame(320, 240));
    let deliver: Deliver = Arc::new(|_| {});
    let picture = |edit: &Edit, gpu: &mut Option<greycard_gpu::Context>| {
        let mut base = None;
        let (outcome, _) = develop_job(
            &Input::Raw(frame.clone()),
            edit,
            0,
            1,
            &mut base,
            &mut Ai::new(),
            None,
            None,
            gpu,
            &deliver,
        );
        let Outcome::Developed { guide, .. } = outcome else {
            panic!("developed")
        };
        guide.picture.clone().expect("a picture")
    };
    let mut gpu = Some(ctx);
    for sharpen in [false, true] {
        let mut edit = refining(0.3);
        edit.detail.clarity = 0.4;
        edit.sharpen.enabled = sharpen;
        let on_gpu = picture(&edit, &mut gpu);
        assert!(gpu.is_some(), "the GPU kept");
        let on_cpu = picture(&edit, &mut None);
        let worst = on_gpu
            .lab
            .iter()
            .zip(&on_cpu.lab)
            .flat_map(|(a, b)| (0..3).map(move |c| (a[c].to_f32() - b[c].to_f32()).abs()))
            .fold(0f32, f32::max);
        assert!(worst < 5e-3, "sharpen {sharpen}: {worst} apart");
    }
}

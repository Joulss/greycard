//! The window's device lost, as the viewport's waits see it: the flag
//! for the run, and the wait that reads it. What the window does about
//! a loss, the quit and the start again, is the window's.

use std::sync::atomic::{AtomicBool, Ordering};

use greycard_gpu::{panic_message, wgpu};

/// Whether the window's device has been lost this run: set by the
/// window's device-lost callback, and by a wait that panicked on it.
pub static LOST: AtomicBool = AtomicBool::new(false);

/// Whether the window's device has been lost this run.
pub fn lost() -> bool {
    LOST.load(Ordering::SeqCst)
}

/// Wait on the window's device as `poll_type` says, for the
/// viewport's read backs and its scopes: an error rather than wgpu's
/// panic once the device is lost.
pub fn poll(device: &wgpu::Device, poll_type: wgpu::PollType) -> Result<(), String> {
    if lost() {
        return Err("the GPU device was lost".into());
    }
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| device.poll(poll_type))) {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(e.to_string()),
        Err(payload) => {
            LOST.store(true, Ordering::SeqCst);
            Err(format!(
                "the GPU device was lost: {}",
                panic_message(payload.as_ref())
            ))
        }
    }
}

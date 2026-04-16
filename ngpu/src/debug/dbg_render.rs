use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::{NvnCommandHandle, NvnQueue};
use crate::debug::dbg_draw::lines_snapshot;

static TARGET_CALLER_OFF: AtomicUsize = AtomicUsize::new(0);
static TARGET_QUEUE_PTR: AtomicUsize = AtomicUsize::new(0);
static WARNED_UNIMPLEMENTED: AtomicBool = AtomicBool::new(false);

#[inline(always)]
pub fn set_overlay_submit_filter(caller_text_off: usize) {
    TARGET_CALLER_OFF.store(caller_text_off, Ordering::Release);
}

#[inline(always)]
pub fn overlay_submit_filter() -> usize {
    TARGET_CALLER_OFF.load(Ordering::Acquire)
}

#[inline(always)]
pub fn set_overlay_target_queue(queue: Option<*mut NvnQueue>) {
    TARGET_QUEUE_PTR.store(queue.unwrap_or(core::ptr::null_mut()) as usize, Ordering::Release);
}

#[inline(always)]
pub fn overlay_target_queue() -> Option<*mut NvnQueue> {
    let q = TARGET_QUEUE_PTR.load(Ordering::Acquire);
    if q == 0 {
        None
    } else {
        Some(q as *mut NvnQueue)
    }
}

/// Queue-submit hook callback: returns an NVN command handle that should be
/// appended to the game's submit list.
///
/// Current state:
/// - Collects debug primitives and logs.
/// - Returns `0` until NVN command-buffer resource allocation is wired.
pub fn overlay_submit_handle_provider(queue: *mut NvnQueue) -> NvnCommandHandle {
    let lines = lines_snapshot();
    if lines.is_empty() {
        return 0;
    }

    if !WARNED_UNIMPLEMENTED.swap(true, Ordering::AcqRel) {
        ncommon::logN!(
            target: "debug.render",
            "overlay provider queued {} line(s) on queue={:p}; NVN cmd recording not wired yet",
            lines.len(),
            queue
        );
    }

    0
}

pub mod dbg_draw;
pub mod dbg_render;
pub mod dbg_shaders;
pub mod dbg_shapes;

pub use dbg_draw::{DebugColor, DebugLine, clear_draw_list, lines_snapshot, push_line};
pub use dbg_render::{
    overlay_submit_handle_provider, set_overlay_submit_filter, set_overlay_target_queue,
};

use core::sync::atomic::{AtomicBool, Ordering};

static DEBUG_OVERLAY_ENABLED: AtomicBool = AtomicBool::new(false);

#[inline(always)]
pub fn set_enabled(enabled: bool) {
    DEBUG_OVERLAY_ENABLED.store(enabled, Ordering::Release);
    ncommon::logN!(target: "debug", "overlay enabled={}", enabled);
}

#[inline(always)]
pub fn enabled() -> bool {
    DEBUG_OVERLAY_ENABLED.load(Ordering::Acquire)
}

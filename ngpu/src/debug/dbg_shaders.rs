use core::sync::atomic::{AtomicBool, Ordering};

static SHADERS_READY: AtomicBool = AtomicBool::new(false);

#[inline(always)]
pub fn ensure_shaders_ready() -> bool {
    SHADERS_READY.load(Ordering::Acquire)
}

#[inline(always)]
pub fn mark_shaders_ready(ready: bool) {
    SHADERS_READY.store(ready, Ordering::Release);
}

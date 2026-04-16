use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::NvnDevice;

static BOOTSTRAP_ACTIVE: AtomicBool = AtomicBool::new(false);
static DEVICE_INITIALIZING: AtomicBool = AtomicBool::new(false);
static DEVICE_READY: AtomicBool = AtomicBool::new(false);

static DEVICE_PTR: AtomicUsize = AtomicUsize::new(0);
static QUEUE_PTR: AtomicUsize = AtomicUsize::new(0);
static WINDOW_PTR: AtomicUsize = AtomicUsize::new(0);
static PRESENT_QUEUE_PTR: AtomicUsize = AtomicUsize::new(0);
static DEVICE_GET_PROC_ADDR: AtomicUsize = AtomicUsize::new(0);

static SUPPORTS_DRAW_TEXTURE: AtomicI32 = AtomicI32::new(-1);
static DRIVER_API_MAJOR: AtomicI32 = AtomicI32::new(-1);
static DRIVER_API_MINOR: AtomicI32 = AtomicI32::new(-1);

const MAX_WINDOW_TEXTURES: usize = 8;

#[derive(Copy, Clone)]
struct WindowTextureState {
    count: usize,
    active_index: i32,
    textures: [usize; MAX_WINDOW_TEXTURES],
}

impl Default for WindowTextureState {
    fn default() -> Self {
        Self {
            count: 0,
            active_index: -1,
            textures: [0; MAX_WINDOW_TEXTURES],
        }
    }
}

static WINDOW_TEXTURE_STATE: OnceLock<Mutex<WindowTextureState>> = OnceLock::new();

#[inline(always)]
fn window_texture_state() -> &'static Mutex<WindowTextureState> {
    WINDOW_TEXTURE_STATE.get_or_init(|| Mutex::new(WindowTextureState::default()))
}

#[inline(always)]
pub fn set_bootstrap_active(active: bool) {
    BOOTSTRAP_ACTIVE.store(active, Ordering::Release);
}

#[inline(always)]
pub fn bootstrap_active() -> bool {
    BOOTSTRAP_ACTIVE.load(Ordering::Acquire)
}

#[inline(always)]
pub fn cache_device_ptr(device: *mut NvnDevice) -> bool {
    let addr = device as usize;
    if addr == 0 {
        return false;
    }
    let previous = DEVICE_PTR.swap(addr, Ordering::AcqRel);
    if previous == addr {
        return false;
    }

    DEVICE_READY.store(false, Ordering::Release);
    DEVICE_INITIALIZING.store(false, Ordering::Release);
    SUPPORTS_DRAW_TEXTURE.store(-1, Ordering::Release);
    DRIVER_API_MAJOR.store(-1, Ordering::Release);
    DRIVER_API_MINOR.store(-1, Ordering::Release);
    true
}

#[inline(always)]
pub fn device_ptr() -> Option<*mut NvnDevice> {
    let addr = DEVICE_PTR.load(Ordering::Acquire);
    if addr == 0 {
        None
    } else {
        Some(addr as *mut NvnDevice)
    }
}

#[inline(always)]
pub fn cache_window_ptr(window: *mut crate::NvnWindow) -> bool {
    let addr = window as usize;
    if addr == 0 {
        return false;
    }
    let previous = WINDOW_PTR.swap(addr, Ordering::AcqRel);
    previous != addr
}

#[inline(always)]
pub fn cache_queue_ptr(queue: *mut crate::NvnQueue) -> bool {
    let addr = queue as usize;
    if addr == 0 {
        return false;
    }
    let previous = QUEUE_PTR.swap(addr, Ordering::AcqRel);
    previous != addr
}

#[inline(always)]
pub fn queue_ptr() -> Option<*mut crate::NvnQueue> {
    let addr = QUEUE_PTR.load(Ordering::Acquire);
    if addr == 0 {
        None
    } else {
        Some(addr as *mut crate::NvnQueue)
    }
}

#[inline(always)]
pub fn cache_present_queue_ptr(queue: *mut crate::NvnQueue) -> bool {
    let addr = queue as usize;
    if addr == 0 {
        return false;
    }
    let previous = PRESENT_QUEUE_PTR.swap(addr, Ordering::AcqRel);
    previous != addr
}

#[inline(always)]
pub fn present_queue_ptr() -> Option<*mut crate::NvnQueue> {
    let addr = PRESENT_QUEUE_PTR.load(Ordering::Acquire);
    if addr == 0 {
        None
    } else {
        Some(addr as *mut crate::NvnQueue)
    }
}

#[inline(always)]
pub fn window_ptr() -> Option<*mut crate::NvnWindow> {
    let addr = WINDOW_PTR.load(Ordering::Acquire);
    if addr == 0 {
        None
    } else {
        Some(addr as *mut crate::NvnWindow)
    }
}

#[inline(always)]
pub fn set_device_get_proc_addr(addr: usize) {
    if addr != 0 {
        DEVICE_GET_PROC_ADDR.store(addr, Ordering::Release);
    }
}

#[inline(always)]
pub fn device_get_proc_addr() -> Option<usize> {
    let addr = DEVICE_GET_PROC_ADDR.load(Ordering::Acquire);
    if addr == 0 {
        None
    } else {
        Some(addr)
    }
}

#[inline(always)]
pub fn begin_device_init() -> bool {
    DEVICE_INITIALIZING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
}

#[inline(always)]
pub fn end_device_init(success: bool) {
    DEVICE_READY.store(success, Ordering::Release);
    DEVICE_INITIALIZING.store(false, Ordering::Release);
}

#[inline(always)]
pub fn device_ready() -> bool {
    DEVICE_READY.load(Ordering::Acquire)
}

#[inline(always)]
pub fn cache_supports_draw_texture(raw: i32) {
    SUPPORTS_DRAW_TEXTURE.store(raw, Ordering::Release);
}

#[inline(always)]
pub fn supports_draw_texture() -> Option<bool> {
    let raw = SUPPORTS_DRAW_TEXTURE.load(Ordering::Acquire);
    if raw < 0 {
        None
    } else {
        Some(raw != 0)
    }
}

#[inline(always)]
pub fn cache_driver_api_versions(major: i32, minor: i32) {
    DRIVER_API_MAJOR.store(major, Ordering::Release);
    DRIVER_API_MINOR.store(minor, Ordering::Release);
}

#[inline(always)]
pub fn driver_api_versions() -> Option<(i32, i32)> {
    let major = DRIVER_API_MAJOR.load(Ordering::Acquire);
    let minor = DRIVER_API_MINOR.load(Ordering::Acquire);
    if major < 0 || minor < 0 {
        None
    } else {
        Some((major, minor))
    }
}

#[inline(always)]
pub fn reset() {
    BOOTSTRAP_ACTIVE.store(false, Ordering::Release);
    DEVICE_INITIALIZING.store(false, Ordering::Release);
    DEVICE_READY.store(false, Ordering::Release);
    DEVICE_PTR.store(0, Ordering::Release);
    QUEUE_PTR.store(0, Ordering::Release);
    WINDOW_PTR.store(0, Ordering::Release);
    PRESENT_QUEUE_PTR.store(0, Ordering::Release);
    DEVICE_GET_PROC_ADDR.store(0, Ordering::Release);
    SUPPORTS_DRAW_TEXTURE.store(-1, Ordering::Release);
    DRIVER_API_MAJOR.store(-1, Ordering::Release);
    DRIVER_API_MINOR.store(-1, Ordering::Release);
    if let Ok(mut st) = window_texture_state().lock() {
        *st = WindowTextureState::default();
    }
}

#[inline(always)]
pub fn cache_window_textures(textures: *const *mut crate::NvnTexture, count: i32) {
    if textures.is_null() || count <= 0 {
        return;
    }
    let capped = (count as usize).min(MAX_WINDOW_TEXTURES);
    if let Ok(mut st) = window_texture_state().lock() {
        st.count = capped;
        let mut i = 0usize;
        while i < capped {
            st.textures[i] = unsafe { *textures.add(i) as usize };
            i += 1;
        }
        while i < MAX_WINDOW_TEXTURES {
            st.textures[i] = 0;
            i += 1;
        }
    }
}

#[inline(always)]
pub fn set_active_window_texture_index(index: i32) {
    if let Ok(mut st) = window_texture_state().lock() {
        st.active_index = index;
    }
}

#[inline(always)]
pub fn active_window_texture_index() -> Option<i32> {
    if let Ok(st) = window_texture_state().lock() {
        if st.active_index >= 0 {
            Some(st.active_index)
        } else {
            None
        }
    } else {
        None
    }
}

#[inline(always)]
pub fn active_window_texture_ptr() -> Option<*mut crate::NvnTexture> {
    if let Ok(st) = window_texture_state().lock() {
        if st.active_index < 0 {
            return None;
        }
        let idx = st.active_index as usize;
        if idx >= st.count {
            return None;
        }
        let ptr = st.textures[idx];
        if ptr == 0 {
            None
        } else {
            Some(ptr as *mut crate::NvnTexture)
        }
    } else {
        None
    }
}

#[inline(always)]
pub fn window_texture_ptr_at(index: i32) -> Option<*mut crate::NvnTexture> {
    if index < 0 {
        return None;
    }
    if let Ok(st) = window_texture_state().lock() {
        let idx = index as usize;
        if idx >= st.count {
            return None;
        }
        let ptr = st.textures[idx];
        if ptr == 0 {
            None
        } else {
            Some(ptr as *mut crate::NvnTexture)
        }
    } else {
        None
    }
}

#[inline(always)]
pub fn first_window_texture_ptr() -> Option<*mut crate::NvnTexture> {
    if let Ok(st) = window_texture_state().lock() {
        if st.count == 0 {
            return None;
        }
        let ptr = st.textures[0];
        if ptr == 0 {
            None
        } else {
            Some(ptr as *mut crate::NvnTexture)
        }
    } else {
        None
    }
}

#[inline(always)]
pub fn window_texture_ptrs_snapshot(out: &mut [*mut crate::NvnTexture; MAX_WINDOW_TEXTURES]) -> usize {
    if let Ok(st) = window_texture_state().lock() {
        let mut i = 0usize;
        while i < st.count && i < MAX_WINDOW_TEXTURES {
            out[i] = st.textures[i] as *mut crate::NvnTexture;
            i += 1;
        }
        while i < MAX_WINDOW_TEXTURES {
            out[i] = core::ptr::null_mut();
            i += 1;
        }
        st.count
    } else {
        let mut i = 0usize;
        while i < MAX_WINDOW_TEXTURES {
            out[i] = core::ptr::null_mut();
            i += 1;
        }
        0
    }
}

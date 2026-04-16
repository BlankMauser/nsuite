use core::ffi::{c_char, c_void};
use core::ptr;
use std::sync::Mutex;

use skyline::nn::ro;

pub const NGPU_RIVE_BRIDGE_ABI_VERSION: u32 = 1;

pub type NgpuAllocFn =
    Option<unsafe extern "C" fn(size: usize, alignment: usize, user: *mut c_void) -> *mut c_void>;
pub type NgpuReallocFn = Option<
    unsafe extern "C" fn(ptr: *mut c_void, new_size: usize, user: *mut c_void) -> *mut c_void,
>;
pub type NgpuFreeFn = Option<unsafe extern "C" fn(ptr: *mut c_void, user: *mut c_void)>;
pub type NgpuQueuePresentSubmitCallback =
    Option<unsafe extern "C" fn(queue: *mut c_void, window: *mut c_void, index: i32) -> u64>;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct NgpuAllocator {
    pub alloc: NgpuAllocFn,
    pub realloc: NgpuReallocFn,
    pub free: NgpuFreeFn,
    pub user: *mut c_void,
}

impl Default for NgpuAllocator {
    fn default() -> Self {
        Self {
            alloc: None,
            realloc: None,
            free: None,
            user: ptr::null_mut(),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct NgpuRiveBridge {
    pub abi_version: u32,
    pub device: *mut c_void,
    pub queue: *mut c_void,
    pub window: *mut c_void,
    pub get_proc_address: *mut c_void,
    pub active_window_texture: *mut c_void,
    pub allocator: NgpuAllocator,
    pub has_allocator: u32,
}

impl Default for NgpuRiveBridge {
    fn default() -> Self {
        Self {
            abi_version: 0,
            device: core::ptr::null_mut(),
            queue: core::ptr::null_mut(),
            window: core::ptr::null_mut(),
            get_proc_address: core::ptr::null_mut(),
            active_window_texture: core::ptr::null_mut(),
            allocator: NgpuAllocator::default(),
            has_allocator: 0,
        }
    }
}

unsafe impl Send for NgpuAllocator {}
unsafe impl Sync for NgpuAllocator {}

static ALLOCATOR: Mutex<Option<NgpuAllocator>> = Mutex::new(None);
static PRESENT_SUBMIT_CALLBACK: Mutex<NgpuQueuePresentSubmitCallback> = Mutex::new(None);

fn current_allocator() -> Option<NgpuAllocator> {
    *ALLOCATOR.lock().unwrap()
}

fn current_present_submit_callback() -> NgpuQueuePresentSubmitCallback {
    *PRESENT_SUBMIT_CALLBACK.lock().unwrap()
}

fn present_submit_callback_bridge(
    queue: *mut crate::NvnQueue,
    window: *mut crate::NvnWindow,
    index: i32,
) -> crate::NvnCommandHandle {
    current_present_submit_callback()
        .map(|callback| unsafe { callback(queue.cast(), window.cast(), index) })
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn ngpu_shim_abi_version() -> u32 {
    NGPU_RIVE_BRIDGE_ABI_VERSION
}

#[no_mangle]
pub extern "C" fn ngpu_shim_get_rive_bridge(out_bridge: *mut NgpuRiveBridge) -> i32 {
    if out_bridge.is_null() {
        return 0;
    }

    let allocator = current_allocator().unwrap_or_default();
    let has_allocator = current_allocator().is_some() as u32;
    let bridge = NgpuRiveBridge {
        abi_version: NGPU_RIVE_BRIDGE_ABI_VERSION,
        device: crate::bootstrap::cached_device()
            .unwrap_or(ptr::null_mut())
            .cast(),
        queue: crate::bootstrap::cached_queue()
            .unwrap_or(ptr::null_mut())
            .cast(),
        window: crate::bootstrap::cached_window()
            .unwrap_or(ptr::null_mut())
            .cast(),
        get_proc_address: crate::bootstrap::cached_device_get_proc_address()
            .unwrap_or(ptr::null_mut()),
        active_window_texture: crate::bootstrap::cached_active_window_texture()
            .unwrap_or(ptr::null_mut())
            .cast(),
        allocator,
        has_allocator,
    };

    unsafe {
        *out_bridge = bridge;
    }

    (bridge.device != ptr::null_mut() && bridge.queue != ptr::null_mut()) as i32
}

#[no_mangle]
pub extern "C" fn ngpu_shim_install_device_hooks() -> i32 {
    unsafe { crate::bootstrap::install_device_hooks() as i32 }
}

#[no_mangle]
pub extern "C" fn ngpu_shim_try_initialize_from_cached_device() -> i32 {
    unsafe { crate::bootstrap::try_initialize_from_cached_device() as i32 }
}

#[no_mangle]
pub extern "C" fn ngpu_shim_get_device() -> *mut c_void {
    crate::bootstrap::cached_device()
        .unwrap_or(ptr::null_mut())
        .cast()
}

#[no_mangle]
pub extern "C" fn ngpu_shim_get_queue() -> *mut c_void {
    crate::bootstrap::cached_queue()
        .unwrap_or(ptr::null_mut())
        .cast()
}

#[no_mangle]
pub extern "C" fn ngpu_shim_get_window() -> *mut c_void {
    crate::bootstrap::cached_window()
        .unwrap_or(ptr::null_mut())
        .cast()
}

#[no_mangle]
pub extern "C" fn ngpu_shim_get_device_get_proc_address() -> *mut c_void {
    crate::bootstrap::cached_device_get_proc_address().unwrap_or(ptr::null_mut())
}

#[no_mangle]
pub extern "C" fn ngpu_shim_get_active_window_texture() -> *mut c_void {
    crate::bootstrap::cached_active_window_texture()
        .unwrap_or(ptr::null_mut())
        .cast()
}

#[no_mangle]
pub extern "C" fn ngpu_shim_get_window_texture(index: i32) -> *mut c_void {
    crate::bootstrap::cached_window_texture(index)
        .unwrap_or(ptr::null_mut())
        .cast()
}

#[no_mangle]
pub extern "C" fn ngpu_shim_get_active_window_texture_index() -> i32 {
    crate::bootstrap::cached_active_window_texture_index().unwrap_or(-1)
}

#[no_mangle]
pub extern "C" fn ngpu_shim_set_allocator(allocator: *const NgpuAllocator) {
    let mut guard = ALLOCATOR.lock().unwrap();
    *guard = unsafe { allocator.as_ref().copied() };
}

#[no_mangle]
pub extern "C" fn ngpu_shim_get_allocator(out_allocator: *mut NgpuAllocator) -> i32 {
    if out_allocator.is_null() {
        return 0;
    }
    let Some(allocator) = current_allocator() else {
        return 0;
    };
    unsafe {
        *out_allocator = allocator;
    }
    1
}

#[no_mangle]
pub extern "C" fn ngpu_shim_set_queue_present_submit_callback(
    callback: NgpuQueuePresentSubmitCallback,
) -> i32 {
    *PRESENT_SUBMIT_CALLBACK.lock().unwrap() = callback;
    crate::bootstrap::set_queue_present_submit_provider(if callback.is_some() {
        Some(present_submit_callback_bridge)
    } else {
        None
    });
    1
}

#[no_mangle]
pub extern "C" fn ngpu_shim_lookup_symbol(name: *const c_char) -> *mut c_void {
    if name.is_null() {
        return ptr::null_mut();
    }

    let mut addr: usize = 0;
    unsafe {
        ro::Initialize();
        ro::LookupSymbol(&mut addr, name as _);
    }
    addr as *mut c_void
}

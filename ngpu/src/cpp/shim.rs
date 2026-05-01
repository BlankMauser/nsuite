use core::ffi::{c_char, c_void};
use core::ptr;
use std::sync::Mutex;

use skyline::nn::ro;

pub const NGPU_SHIM_ABI_VERSION: u32 = 1;

pub type NgpuQueuePresentSubmitCallback =
    Option<unsafe extern "C" fn(queue: *mut c_void, window: *mut c_void, index: i32) -> u64>;

static PRESENT_SUBMIT_CALLBACK: Mutex<NgpuQueuePresentSubmitCallback> = Mutex::new(None);

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
    NGPU_SHIM_ABI_VERSION
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

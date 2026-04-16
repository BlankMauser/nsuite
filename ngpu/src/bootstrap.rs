use core::alloc::Layout;

use skyline::hooks::A64HookFunction;
use skyline::libc::{c_char, c_void};

use crate::consts::{SLOT_NVN_DEVICE_GET_INTEGER, SLOT_NVN_DEVICE_GET_PROC_ADDRESS};
use crate::resources::runtime;
use crate::{NvnBoolean, NvnDevice, NvnDeviceBuilder, NvnDeviceInfo, NvnWindow, NvnWindowBuilder};

const VERSION_1: NvnDeviceInfo = 0;
const VERSION_2: NvnDeviceInfo = 1;
const VERSION_71: NvnDeviceInfo = 71;

const DEVICE_INIT_NAME: &[u8] = b"nvnDeviceInitialize\0";
const WINDOW_INIT_NAME: &[u8] = b"nvnWindowInitialize\0";
const QUEUE_INIT_NAME: &[u8] = b"nvnQueueInitialize\0";
const QUEUE_FINALIZE_NAME: &[u8] = b"nvnQueueFinalize\0";
const QUEUE_SUBMIT_NAME: &[u8] = b"nvnQueueSubmitCommands\0";
const QUEUE_BUILDER_SET_FLAGS_NAME: &[u8] = b"nvnQueueBuilderSetFlags\0";
const QUEUE_PRESENT_TEXTURE_NAME: &[u8] = b"nvnQueuePresentTexture\0";
const QUEUE_ACQUIRE_TEXTURE_NAME: &[u8] = b"nvnQueueAcquireTexture\0";
const WINDOW_SET_TEXTURES_NAME: &[u8] = b"nvnWindowBuilderSetTextures\0";
const WINDOW_ACQUIRE_TEXTURE_NAME: &[u8] = b"nvnWindowAcquireTexture\0";
const GET_PROC_NAME: &[u8] = b"nvnDeviceGetProcAddress\0";
const GET_INTEGER_NAME: &[u8] = b"nvnDeviceGetInteger\0";
const GPU_PIPELINE_TRACE: bool = false;

type GenericFuncPtr = Option<unsafe extern "C" fn()>;
type BootstrapLoaderFn = unsafe extern "C" fn(*const c_char) -> GenericFuncPtr;
type DeviceInitializeFn = unsafe extern "C" fn(*mut NvnDevice, *const NvnDeviceBuilder) -> NvnBoolean;
type WindowInitializeFn = unsafe extern "C" fn(*mut NvnWindow, *const NvnWindowBuilder) -> NvnBoolean;
type QueueInitializeFn =
    unsafe extern "C" fn(*mut crate::NvnQueue, *const crate::NvnQueueBuilder) -> NvnBoolean;
type QueueFinalizeFn = unsafe extern "C" fn(*mut crate::NvnQueue);
type QueueSubmitCommandsFn =
    unsafe extern "C" fn(*mut crate::NvnQueue, i32, *const crate::NvnCommandHandle);
type QueueBuilderSetFlagsFn = unsafe extern "C" fn(*mut crate::NvnQueueBuilder, i32);
type QueuePresentTextureFn =
    unsafe extern "C" fn(*mut crate::NvnQueue, *mut NvnWindow, i32);
type QueueAcquireTextureFn = unsafe extern "C" fn(
    *mut crate::NvnQueue,
    *mut NvnWindow,
    *mut i32,
) -> crate::NvnQueueAcquireTextureResult;
pub type QueueSubmitAppendProvider = fn(
    *mut crate::NvnQueue,
    usize,
    i32,
    *const crate::NvnCommandHandle,
) -> crate::NvnCommandHandle;
pub type QueuePresentSubmitProvider =
    fn(*mut crate::NvnQueue, *mut NvnWindow, i32) -> crate::NvnCommandHandle;
type WindowBuilderSetTexturesFn =
    unsafe extern "C" fn(*mut NvnWindowBuilder, i32, *const *mut crate::NvnTexture);
type WindowAcquireTextureFn = unsafe extern "C" fn(
    *mut NvnWindow,
    *mut crate::NvnSync,
    *mut i32,
) -> crate::NvnWindowAcquireTextureResult;
type DeviceGetProcAddressFn = unsafe extern "C" fn(*const NvnDevice, *const c_char) -> GenericFuncPtr;
type DeviceGetIntegerFn = unsafe extern "C" fn(*const NvnDevice, NvnDeviceInfo, *mut i32);

#[repr(C)]
#[derive(Copy, Clone)]
struct RawNvnQueueErrorMmuFault {
    fault_address: u64,
    access_type: i32,
}

#[repr(C)]
union RawNvnQueueErrorInfo {
    mmu_fault: RawNvnQueueErrorMmuFault,
    reserved: [i32; 16],
}

const NVN_QUEUE_GET_ERROR_GPU_NO_ERROR: i32 = 0;
const NVN_QUEUE_GET_ERROR_GPU_ERROR_UNKNOWN: i32 = 1;
const NVN_QUEUE_GET_ERROR_GPU_ERROR_MMU_FAULT: i32 = 2;
const NVN_QUEUE_GET_ERROR_GPU_ERROR_PBDMA_EXCEPTION: i32 = 3;
const NVN_QUEUE_GET_ERROR_GPU_ERROR_ENGINE_EXCEPTION: i32 = 4;
const NVN_QUEUE_GET_ERROR_GPU_ERROR_TIMEOUT: i32 = 5;
const NVN_MEMORY_ACCESS_WRITE: i32 = 1;
const NVN_QUEUE_BUILDER_FLAGS_NO_FRAGMENT_INTERLOCK: i32 = 0x1;
const NVN_MEMORY_POOL_STORAGE_GRANULARITY: usize = 0x1000;

struct QueueOwnedMemory {
    queue_addr: usize,
    ptr: *mut u8,
    layout: Layout,
    size: usize,
}

static OWNED_QUEUE_MEMORY: locks::Mutex<Vec<QueueOwnedMemory>> =
    locks::Mutex::new(Vec::new());

unsafe extern "C" {
    #[link_name = "nvnBootstrapLoader"]
    fn nvn_bootstrap_loader(symbol: *const c_char) -> GenericFuncPtr;
}

static mut BOOTSTRAP_LOADER_ORIG: Option<BootstrapLoaderFn> = None;
static mut DEVICE_INITIALIZE_ORIG: Option<DeviceInitializeFn> = None;
static mut WINDOW_INITIALIZE_ORIG: Option<WindowInitializeFn> = None;
static mut QUEUE_INITIALIZE_ORIG: Option<QueueInitializeFn> = None;
static mut QUEUE_FINALIZE_ORIG: Option<QueueFinalizeFn> = None;
static mut QUEUE_SUBMIT_ORIG: Option<QueueSubmitCommandsFn> = None;
static mut QUEUE_BUILDER_SET_FLAGS_ORIG: Option<QueueBuilderSetFlagsFn> = None;
static mut QUEUE_PRESENT_TEXTURE_ORIG: Option<QueuePresentTextureFn> = None;
static mut QUEUE_ACQUIRE_TEXTURE_ORIG: Option<QueueAcquireTextureFn> = None;
static mut WINDOW_SET_TEXTURES_ORIG: Option<WindowBuilderSetTexturesFn> = None;
static mut WINDOW_ACQUIRE_TEXTURE_ORIG: Option<WindowAcquireTextureFn> = None;
static mut DEVICE_GET_PROC_BASE: Option<DeviceGetProcAddressFn> = None;
static mut DEVICE_GET_PROC_ACTUAL: Option<DeviceGetProcAddressFn> = None;

const MAX_TRACKED_QUEUES: usize = 64;
const MAX_TRACKED_SUBMIT_CALLERS: usize = 128;
const MAX_TRACKED_SUBMIT_QUEUE_CALLERS: usize = 256;

type QueueCaller = (usize, usize);

static mut INIT_QUEUES: [usize; MAX_TRACKED_QUEUES] = [0; MAX_TRACKED_QUEUES];
static mut INIT_QUEUES_LEN: usize = 0;
static mut FINALIZE_QUEUES: [usize; MAX_TRACKED_QUEUES] = [0; MAX_TRACKED_QUEUES];
static mut FINALIZE_QUEUES_LEN: usize = 0;
static mut SUBMIT_QUEUES: [usize; MAX_TRACKED_QUEUES] = [0; MAX_TRACKED_QUEUES];
static mut SUBMIT_QUEUES_LEN: usize = 0;
static mut SUBMIT_CALLERS: [usize; MAX_TRACKED_SUBMIT_CALLERS] = [0; MAX_TRACKED_SUBMIT_CALLERS];
static mut SUBMIT_CALLERS_LEN: usize = 0;
static mut SUBMIT_QUEUE_CALLERS: [QueueCaller; MAX_TRACKED_SUBMIT_QUEUE_CALLERS] =
    [(0, 0); MAX_TRACKED_SUBMIT_QUEUE_CALLERS];
static mut SUBMIT_QUEUE_CALLERS_LEN: usize = 0;
static mut WINDOW_ACQUIRE_LOG_COUNT: usize = 0;
static mut QUEUE_ACQUIRE_LOG_COUNT: usize = 0;
static mut QUEUE_PRESENT_LOG_COUNT: usize = 0;
static mut APPEND_SUBMIT_LOG_COUNT: usize = 0;
static mut WINDOW_TEXTURE_CONFIG_LOG_COUNT: usize = 0;
static mut APPEND_QUEUE_ERROR_LOG_COUNT: usize = 0;
static mut QUEUE_INIT_FLAG_LOG_COUNT: usize = 0;
static mut QUEUE_BUILDER_FLAG_PATCH_LOG_COUNT: usize = 0;
static SUBMIT_APPEND_PROVIDER: locks::Mutex<Option<QueueSubmitAppendProvider>> =
    locks::Mutex::new(None);
static PRESENT_SUBMIT_PROVIDER: locks::Mutex<Option<QueuePresentSubmitProvider>> =
    locks::Mutex::new(None);

#[inline(always)]
pub fn set_queue_submit_append_provider(provider: Option<QueueSubmitAppendProvider>) {
    *SUBMIT_APPEND_PROVIDER.lock() = provider;
}

#[inline(always)]
pub fn set_queue_present_submit_provider(provider: Option<QueuePresentSubmitProvider>) {
    *PRESENT_SUBMIT_PROVIDER.lock() = provider;
}

#[inline(always)]
pub fn on_loader_called() {
    runtime::set_bootstrap_active(true);
}

#[inline(always)]
pub fn on_device_seen(device: *mut NvnDevice) {
    let _ = runtime::cache_device_ptr(device);
}

#[inline(always)]
pub fn on_window_seen(window: *mut NvnWindow) {
    let _ = runtime::cache_window_ptr(window);
}

#[inline(always)]
pub fn on_queue_seen(queue: *mut crate::NvnQueue) {
    let _ = runtime::cache_queue_ptr(queue);
}

#[inline(always)]
pub fn on_present_queue_seen(queue: *mut crate::NvnQueue) {
    let _ = runtime::cache_present_queue_ptr(queue);
}

#[inline(always)]
unsafe fn cstr_eq_ascii(ptr: *const c_char, bytes_nul: &[u8]) -> bool {
    if ptr.is_null() {
        return false;
    }
    let ptr = ptr as *const u8;
    let mut i = 0usize;
    while i < bytes_nul.len() {
        if *ptr.add(i) != bytes_nul[i] {
            return false;
        }
        if bytes_nul[i] == 0 {
            return true;
        }
        i += 1;
    }
    false
}

#[inline(always)]
unsafe fn push_unique_usize<const N: usize>(list: &mut [usize; N], len: &mut usize, value: usize) -> bool {
    if value == 0 {
        return false;
    }
    let mut i = 0usize;
    while i < *len {
        if list[i] == value {
            return false;
        }
        i += 1;
    }
    if *len < N {
        list[*len] = value;
        *len += 1;
        true
    } else {
        false
    }
}

#[inline(always)]
unsafe fn push_unique_pair<const N: usize>(
    list: &mut [QueueCaller; N],
    len: &mut usize,
    value: QueueCaller,
) -> bool {
    if value.0 == 0 || value.1 == 0 {
        return false;
    }
    let mut i = 0usize;
    while i < *len {
        if list[i] == value {
            return false;
        }
        i += 1;
    }
    if *len < N {
        list[*len] = value;
        *len += 1;
        true
    } else {
        false
    }
}

#[inline(always)]
fn align_up(value: usize, align: usize) -> usize {
    (value + (align - 1)) & !(align - 1)
}

#[inline(always)]
fn queue_memory_ptr_is_owned(ptr: *mut c_void) -> bool {
    if ptr.is_null() {
        return false;
    }
    let guard = OWNED_QUEUE_MEMORY.lock();
    let mut i = 0usize;
    while i < guard.len() {
        if guard[i].ptr == ptr.cast::<u8>() {
            return true;
        }
        i += 1;
    }
    false
}

#[inline(always)]
unsafe fn maybe_patch_queue_builder_memory(
    queue: *mut crate::NvnQueue,
    builder: *const crate::NvnQueueBuilder,
) -> Option<QueueOwnedMemory> {
    if builder.is_null() {
        return None;
    }

    let required = crate::queue::queue_builder_get_queue_memory_size(builder);
    let provided = crate::queue::queue_builder_get_memory_size(builder);
    let provided_ptr = crate::queue::queue_builder_get_memory(builder);
    let reusing_owned_ptr = queue_memory_ptr_is_owned(provided_ptr);

    let needs_size_patch = provided != 0 && required > provided;
    let needs_ptr_patch = provided != 0 && reusing_owned_ptr;
    if !(needs_size_patch || needs_ptr_patch) {
        return None;
    }

    let requested = required.max(provided).max(NVN_MEMORY_POOL_STORAGE_GRANULARITY);
    let alloc_size = align_up(requested, NVN_MEMORY_POOL_STORAGE_GRANULARITY);
    let layout = match Layout::from_size_align(alloc_size, NVN_MEMORY_POOL_STORAGE_GRANULARITY) {
        Ok(v) => v,
        Err(_) => return None,
    };

    let ptr = std::alloc::alloc_zeroed(layout);
    if ptr.is_null() {
        return None;
    }

    crate::queue::queue_builder_set_queue_memory(
        builder as *mut crate::NvnQueueBuilder,
        ptr.cast::<c_void>(),
        alloc_size,
    );

    if QUEUE_INIT_FLAG_LOG_COUNT < 10 {
        ncommon::logN!(
            target: "gpu",
            "patched queue builder memory queue={:p} builder={:p} provided=0x{:x} required=0x{:x} new=0x{:x} reused_ptr={} undersized={}",
            queue,
            builder,
            provided,
            required,
            alloc_size,
            reusing_owned_ptr as u8,
            needs_size_patch as u8
        );
    }

    Some(QueueOwnedMemory {
        queue_addr: queue as usize,
        ptr,
        layout,
        size: alloc_size,
    })
}

#[inline(always)]
unsafe fn release_owned_queue_memory(queue: *mut crate::NvnQueue) {
    let queue_addr = queue as usize;
    let mut guard = OWNED_QUEUE_MEMORY.lock();
    let mut i = 0usize;
    while i < guard.len() {
        if guard[i].queue_addr == queue_addr {
            let entry = guard.remove(i);
            std::alloc::dealloc(entry.ptr, entry.layout);
            if QUEUE_INIT_FLAG_LOG_COUNT < 10 {
                ncommon::logN!(
                    target: "gpu",
                    "released patched queue memory queue={:p} size=0x{:x}",
                    queue,
                    entry.size
                );
            }
            continue;
        }
        i += 1;
    }
}

#[inline(always)]
fn text_base_for_logs() -> usize {
    unsafe { skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as usize }
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn current_link_register() -> usize {
    let lr: usize;
    core::arch::asm!("mov {}, x30", out(reg) lr);
    lr
}

#[cfg(not(target_arch = "aarch64"))]
#[inline(always)]
unsafe fn current_link_register() -> usize {
    0
}

#[inline(always)]
unsafe fn resolve_device_get_proc(device: *const NvnDevice) -> Option<DeviceGetProcAddressFn> {
    if let Some(f) = DEVICE_GET_PROC_ACTUAL {
        return Some(f);
    }

    // Prefer the NGPU-resolved slot because it matches the same dispatch
    // table path used by the rest of NGPU wrappers.
    let slot_addr = crate::load_slot_fn::<usize>(SLOT_NVN_DEVICE_GET_PROC_ADDRESS);
    if slot_addr != 0 {
        let actual: DeviceGetProcAddressFn = core::mem::transmute(slot_addr);
        DEVICE_GET_PROC_ACTUAL = Some(actual);
        runtime::set_device_get_proc_addr(slot_addr);
        return Some(actual);
    }

    let mut base = DEVICE_GET_PROC_BASE;
    if base.is_none() {
        if let Some(loader) = BOOTSTRAP_LOADER_ORIG {
            if let Some(p) = loader(GET_PROC_NAME.as_ptr() as *const c_char) {
                base = Some(core::mem::transmute(p));
                DEVICE_GET_PROC_BASE = base;
            }
        }
    }

    let base = base?;
    let actual = base(device, GET_PROC_NAME.as_ptr() as *const c_char)?;
    let actual: DeviceGetProcAddressFn = core::mem::transmute(actual);
    DEVICE_GET_PROC_ACTUAL = Some(actual);
    runtime::set_device_get_proc_addr(actual as usize);
    Some(actual)
}

#[inline(always)]
unsafe fn resolve_device_get_integer(device: *const NvnDevice) -> Option<DeviceGetIntegerFn> {
    if let Some(get_proc) = resolve_device_get_proc(device) {
        if let Some(p) = get_proc(device, GET_INTEGER_NAME.as_ptr() as *const c_char) {
            return Some(core::mem::transmute(p));
        }
    }

    // Keep slot fallback for resiliency if bootstrap path is unavailable.
    let slot_addr = crate::load_slot_fn::<usize>(SLOT_NVN_DEVICE_GET_INTEGER);
    if slot_addr == 0 {
        return None;
    }
    Some(core::mem::transmute(slot_addr))
}

#[inline(always)]
unsafe fn maybe_install_window_init_hook(device: *const NvnDevice) {
    if WINDOW_INITIALIZE_ORIG.is_some() {
        return;
    }
    let get_proc = match resolve_device_get_proc(device) {
        Some(f) => f,
        None => return,
    };
    let target = match get_proc(device, WINDOW_INIT_NAME.as_ptr() as *const c_char) {
        Some(p) => p,
        None => return,
    };

    let mut trampoline: *mut c_void = core::ptr::null_mut();
    A64HookFunction(
        target as *const c_void,
        nvn_window_initialize_hook as *const c_void,
        &mut trampoline,
    );
    if !trampoline.is_null() {
        WINDOW_INITIALIZE_ORIG = Some(core::mem::transmute(trampoline));
    }
}

#[inline(always)]
unsafe fn maybe_install_queue_init_hook(device: *const NvnDevice) {
    if QUEUE_INITIALIZE_ORIG.is_some() {
        return;
    }
    let get_proc = match resolve_device_get_proc(device) {
        Some(f) => f,
        None => return,
    };
    let target = match get_proc(device, QUEUE_INIT_NAME.as_ptr() as *const c_char) {
        Some(p) => p,
        None => return,
    };

    let mut trampoline: *mut c_void = core::ptr::null_mut();
    A64HookFunction(
        target as *const c_void,
        nvn_queue_initialize_hook as *const c_void,
        &mut trampoline,
    );
    if !trampoline.is_null() {
        QUEUE_INITIALIZE_ORIG = Some(core::mem::transmute(trampoline));
    }
}

#[inline(always)]
unsafe fn maybe_install_queue_finalize_hook(device: *const NvnDevice) {
    if QUEUE_FINALIZE_ORIG.is_some() {
        return;
    }
    let get_proc = match resolve_device_get_proc(device) {
        Some(f) => f,
        None => return,
    };
    let target = match get_proc(device, QUEUE_FINALIZE_NAME.as_ptr() as *const c_char) {
        Some(p) => p,
        None => return,
    };

    let mut trampoline: *mut c_void = core::ptr::null_mut();
    A64HookFunction(
        target as *const c_void,
        nvn_queue_finalize_hook as *const c_void,
        &mut trampoline,
    );
    if !trampoline.is_null() {
        QUEUE_FINALIZE_ORIG = Some(core::mem::transmute(trampoline));
    }
}

#[inline(always)]
unsafe fn maybe_install_queue_submit_hook(device: *const NvnDevice) {
    if QUEUE_SUBMIT_ORIG.is_some() {
        return;
    }
    let get_proc = match resolve_device_get_proc(device) {
        Some(f) => f,
        None => return,
    };
    let target = match get_proc(device, QUEUE_SUBMIT_NAME.as_ptr() as *const c_char) {
        Some(p) => p,
        None => return,
    };

    let mut trampoline: *mut c_void = core::ptr::null_mut();
    A64HookFunction(
        target as *const c_void,
        nvn_queue_submit_commands_hook as *const c_void,
        &mut trampoline,
    );
    if !trampoline.is_null() {
        QUEUE_SUBMIT_ORIG = Some(core::mem::transmute(trampoline));
    }
}

#[inline(always)]
unsafe fn maybe_install_queue_builder_set_flags_hook(device: *const NvnDevice) {
    if QUEUE_BUILDER_SET_FLAGS_ORIG.is_some() {
        return;
    }
    let get_proc = match resolve_device_get_proc(device) {
        Some(f) => f,
        None => return,
    };
    let target = match get_proc(
        device,
        QUEUE_BUILDER_SET_FLAGS_NAME.as_ptr() as *const c_char,
    ) {
        Some(p) => p,
        None => return,
    };

    let mut trampoline: *mut c_void = core::ptr::null_mut();
    A64HookFunction(
        target as *const c_void,
        nvn_queue_builder_set_flags_hook as *const c_void,
        &mut trampoline,
    );
    if !trampoline.is_null() {
        QUEUE_BUILDER_SET_FLAGS_ORIG = Some(core::mem::transmute(trampoline));
        ncommon::logN!(target: "gpu", "installed nvnQueueBuilderSetFlags hook");
    }
}

#[inline(always)]
unsafe fn maybe_install_queue_present_hook(device: *const NvnDevice) {
    if QUEUE_PRESENT_TEXTURE_ORIG.is_some() {
        return;
    }
    let get_proc = match resolve_device_get_proc(device) {
        Some(f) => f,
        None => return,
    };
    let target = match get_proc(device, QUEUE_PRESENT_TEXTURE_NAME.as_ptr() as *const c_char) {
        Some(p) => p,
        None => return,
    };

    let mut trampoline: *mut c_void = core::ptr::null_mut();
    A64HookFunction(
        target as *const c_void,
        nvn_queue_present_texture_hook as *const c_void,
        &mut trampoline,
    );
    if !trampoline.is_null() {
        QUEUE_PRESENT_TEXTURE_ORIG = Some(core::mem::transmute(trampoline));
    }
}

#[inline(always)]
unsafe fn maybe_install_queue_acquire_texture_hook(device: *const NvnDevice) {
    if QUEUE_ACQUIRE_TEXTURE_ORIG.is_some() {
        return;
    }
    let get_proc = match resolve_device_get_proc(device) {
        Some(f) => f,
        None => return,
    };
    let target = match get_proc(device, QUEUE_ACQUIRE_TEXTURE_NAME.as_ptr() as *const c_char) {
        Some(p) => p,
        None => return,
    };

    let mut trampoline: *mut c_void = core::ptr::null_mut();
    A64HookFunction(
        target as *const c_void,
        nvn_queue_acquire_texture_hook as *const c_void,
        &mut trampoline,
    );
    if !trampoline.is_null() {
        QUEUE_ACQUIRE_TEXTURE_ORIG = Some(core::mem::transmute(trampoline));
    }
}

#[inline(always)]
unsafe fn maybe_install_window_texture_hooks(device: *const NvnDevice) {
    let get_proc = match resolve_device_get_proc(device) {
        Some(f) => f,
        None => return,
    };

    if WINDOW_SET_TEXTURES_ORIG.is_none() {
        if let Some(target) = get_proc(device, WINDOW_SET_TEXTURES_NAME.as_ptr() as *const c_char) {
            let mut trampoline: *mut c_void = core::ptr::null_mut();
            A64HookFunction(
                target as *const c_void,
                nvn_window_builder_set_textures_hook as *const c_void,
                &mut trampoline,
            );
            if !trampoline.is_null() {
                WINDOW_SET_TEXTURES_ORIG = Some(core::mem::transmute(trampoline));
            }
        }
    }

    if WINDOW_ACQUIRE_TEXTURE_ORIG.is_none() {
        if let Some(target) = get_proc(device, WINDOW_ACQUIRE_TEXTURE_NAME.as_ptr() as *const c_char) {
            let mut trampoline: *mut c_void = core::ptr::null_mut();
            A64HookFunction(
                target as *const c_void,
                nvn_window_acquire_texture_hook as *const c_void,
                &mut trampoline,
            );
            if !trampoline.is_null() {
                WINDOW_ACQUIRE_TEXTURE_ORIG = Some(core::mem::transmute(trampoline));
            }
        }
    }
}

#[inline(always)]
unsafe fn probe_driver_api_versions_with(get_integer_fn: DeviceGetIntegerFn, device: *const NvnDevice) -> bool {
    let mut major = -1;
    let mut minor = -1;
    get_integer_fn(device, VERSION_1, &mut major);
    get_integer_fn(device, VERSION_2, &mut minor);
    if major >= 0 && minor >= 0 {
        runtime::cache_driver_api_versions(major, minor);
        true
    } else {
        false
    }
}

#[inline(always)]
unsafe fn probe_draw_texture_support_with(get_integer_fn: DeviceGetIntegerFn, device: *const NvnDevice) -> bool {
    let mut supports_draw_texture = -1;
    get_integer_fn(device, VERSION_71, &mut supports_draw_texture);
    if supports_draw_texture >= 0 {
        runtime::cache_supports_draw_texture(supports_draw_texture);
        true
    } else {
        false
    }
}

unsafe extern "C" fn nvn_device_initialize_hook(
    device: *mut NvnDevice,
    builder: *const NvnDeviceBuilder,
) -> NvnBoolean {
    let ret = match DEVICE_INITIALIZE_ORIG {
        Some(f) => f(device, builder),
        None => 0,
    };

    if ret != 0 {
        let _ = ensure_device_initialized(device);
        maybe_install_window_init_hook(device as *const NvnDevice);
        maybe_install_queue_init_hook(device as *const NvnDevice);
        maybe_install_queue_finalize_hook(device as *const NvnDevice);
        maybe_install_queue_submit_hook(device as *const NvnDevice);
        maybe_install_queue_builder_set_flags_hook(device as *const NvnDevice);
        maybe_install_queue_present_hook(device as *const NvnDevice);
        maybe_install_queue_acquire_texture_hook(device as *const NvnDevice);
        maybe_install_window_texture_hooks(device as *const NvnDevice);
    }
    ret
}

unsafe extern "C" fn nvn_window_initialize_hook(
    window: *mut NvnWindow,
    builder: *const NvnWindowBuilder,
) -> NvnBoolean {
    let ret = match WINDOW_INITIALIZE_ORIG {
        Some(f) => f(window, builder),
        None => 0,
    };
    if ret != 0 {
        on_window_seen(window);
    }
    ret
}

unsafe extern "C" fn nvn_queue_builder_set_flags_hook(
    builder: *mut crate::NvnQueueBuilder,
    flags: i32,
) {
    let mut effective_flags = flags;
    let removed_interlock =
        (flags & NVN_QUEUE_BUILDER_FLAGS_NO_FRAGMENT_INTERLOCK) != 0;
    if removed_interlock {
        effective_flags &= !NVN_QUEUE_BUILDER_FLAGS_NO_FRAGMENT_INTERLOCK;
    }

    if QUEUE_BUILDER_FLAG_PATCH_LOG_COUNT < 12 {
        QUEUE_BUILDER_FLAG_PATCH_LOG_COUNT += 1;
        ncommon::logN!(
            target: "gpu",
            "nvnQueueBuilderSetFlags builder={:p} flags_in=0x{:x} flags_effective=0x{:x} removed_no_fragment_interlock={}",
            builder,
            flags as u32,
            effective_flags as u32,
            removed_interlock as u8
        );
    }

    if let Some(f) = QUEUE_BUILDER_SET_FLAGS_ORIG {
        f(builder, effective_flags);
    }
}

unsafe extern "C" fn nvn_queue_initialize_hook(
    queue: *mut crate::NvnQueue,
    builder: *const crate::NvnQueueBuilder,
) -> NvnBoolean {
    let patched_queue_memory = maybe_patch_queue_builder_memory(queue, builder);

    let ret = match QUEUE_INITIALIZE_ORIG {
        Some(f) => f(queue, builder),
        None => 0,
    };

    if ret != 0 {
        if let Some(entry) = patched_queue_memory {
            OWNED_QUEUE_MEMORY.lock().push(entry);
        }

        on_queue_seen(queue);
        if QUEUE_INIT_FLAG_LOG_COUNT < 10 {
            QUEUE_INIT_FLAG_LOG_COUNT += 1;
            let flags = if builder.is_null() {
                0
            } else {
                crate::queue::queue_builder_get_flags(builder)
            };
            let queue_memory_required = if builder.is_null() {
                0
            } else {
                crate::queue::queue_builder_get_queue_memory_size(builder)
            };
            let queue_memory_size = if builder.is_null() {
                0
            } else {
                crate::queue::queue_builder_get_memory_size(builder)
            };
            let queue_memory_ptr = if builder.is_null() {
                core::ptr::null_mut()
            } else {
                crate::queue::queue_builder_get_memory(builder)
            };
            ncommon::logN!(
                target: "gpu",
                "nvnQueueInitialize config queue={:p} builder={:p} flags=0x{:x} queue_mem_size=0x{:x} queue_mem_required=0x{:x} queue_mem_ptr={:p}",
                queue,
                builder,
                flags as u32,
                queue_memory_size,
                queue_memory_required,
                queue_memory_ptr
            );
        }
        let q = queue as usize;
        if push_unique_usize(&mut INIT_QUEUES, &mut INIT_QUEUES_LEN, q) {
            ncommon::logN!(
                target: "gpu",
                "nvnQueueInitialize unique queue={:p} unique_total={}",
                queue, INIT_QUEUES_LEN
            );
        }
    } else if let Some(entry) = patched_queue_memory {
        std::alloc::dealloc(entry.ptr, entry.layout);
    }
    ret
}

unsafe extern "C" fn nvn_queue_finalize_hook(queue: *mut crate::NvnQueue) {
    if let Some(f) = QUEUE_FINALIZE_ORIG {
        f(queue);
    }
    release_owned_queue_memory(queue);
    let q = queue as usize;
    if push_unique_usize(&mut FINALIZE_QUEUES, &mut FINALIZE_QUEUES_LEN, q) {
        ncommon::logN!(
            target: "gpu",
            "nvnQueueFinalize unique queue={:p} unique_total={}",
            queue, FINALIZE_QUEUES_LEN
        );
    }
}

unsafe fn log_queue_error(queue: *mut crate::NvnQueue, reason: &str) {
    #[inline(always)]
    fn queue_error_name(result: i32) -> &'static str {
        match result {
            NVN_QUEUE_GET_ERROR_GPU_NO_ERROR => "NO_ERROR",
            NVN_QUEUE_GET_ERROR_GPU_ERROR_UNKNOWN => "GPU_ERROR_UNKNOWN",
            NVN_QUEUE_GET_ERROR_GPU_ERROR_MMU_FAULT => "GPU_ERROR_MMU_FAULT",
            NVN_QUEUE_GET_ERROR_GPU_ERROR_PBDMA_EXCEPTION => "GPU_ERROR_PBDMA_EXCEPTION",
            NVN_QUEUE_GET_ERROR_GPU_ERROR_ENGINE_EXCEPTION => "GPU_ERROR_ENGINE_EXCEPTION",
            NVN_QUEUE_GET_ERROR_GPU_ERROR_TIMEOUT => "GPU_ERROR_TIMEOUT",
            _ => "GPU_ERROR_UNRECOGNIZED",
        }
    }

    let mut info = RawNvnQueueErrorInfo { reserved: [0; 16] };
    let result = crate::queue::queue_get_error(queue, (&mut info as *mut RawNvnQueueErrorInfo).cast());
    if result == NVN_QUEUE_GET_ERROR_GPU_NO_ERROR {
        return;
    }

    if APPEND_QUEUE_ERROR_LOG_COUNT < 12 {
        APPEND_QUEUE_ERROR_LOG_COUNT += 1;
        if result == NVN_QUEUE_GET_ERROR_GPU_ERROR_MMU_FAULT {
            let fault = unsafe { &info.mmu_fault };
            let access = if fault.access_type == NVN_MEMORY_ACCESS_WRITE {
                "write"
            } else {
                "read"
            };
            ncommon::logN!(
                target: "gpu",
                "queue error reason={} queue={:p} result={} ({}) fault=0x{:x} access={}",
                reason,
                queue,
                result,
                queue_error_name(result),
                fault.fault_address,
                access
            );
        } else {
            let words = unsafe { info.reserved };
            ncommon::logN!(
                target: "gpu",
                "queue error reason={} queue={:p} result={} ({}) info_words=[0x{:x},0x{:x},0x{:x},0x{:x}]",
                reason,
                queue,
                result,
                queue_error_name(result),
                words[0] as u32,
                words[1] as u32,
                words[2] as u32,
                words[3] as u32
            );
        }
    }
}

unsafe extern "C" fn nvn_queue_submit_commands_hook(
    queue: *mut crate::NvnQueue,
    count: i32,
    handles: *const crate::NvnCommandHandle,
) {
    on_queue_seen(queue);
    let queue_addr = queue as usize;
    let caller = current_link_register();
    let new_submit_queue = push_unique_usize(&mut SUBMIT_QUEUES, &mut SUBMIT_QUEUES_LEN, queue_addr);
    let new_submit_caller = push_unique_usize(&mut SUBMIT_CALLERS, &mut SUBMIT_CALLERS_LEN, caller);
    let new_queue_caller = push_unique_pair(
        &mut SUBMIT_QUEUE_CALLERS,
        &mut SUBMIT_QUEUE_CALLERS_LEN,
        (queue_addr, caller),
    );
    if new_submit_queue || new_queue_caller {
        let text_base = text_base_for_logs();
        let caller_off = if caller >= text_base { caller - text_base } else { 0 };
        ncommon::logN!(
            target: "gpu",
            "nvnQueueSubmitCommands queue={:p} cmds={} caller=0x{:x} text_off=0x{:x} unique(queue={},caller={},pair={})",
            queue,
            count,
            caller,
            caller_off,
            new_submit_queue as u8,
            new_submit_caller as u8,
            new_queue_caller as u8
        );
    }

    let caller_off = {
        let text_base = text_base_for_logs();
        if caller >= text_base { caller - text_base } else { 0 }
    };

    let append = match *SUBMIT_APPEND_PROVIDER.lock() {
        Some(provider) => provider(queue, caller_off, count, handles),
        None => 0,
    };

    if let Some(f) = QUEUE_SUBMIT_ORIG {
        if append != 0 && count >= 0 && !handles.is_null() {
            if GPU_PIPELINE_TRACE && APPEND_SUBMIT_LOG_COUNT < 8 {
                APPEND_SUBMIT_LOG_COUNT += 1;
                ncommon::logN!(
                    target: "gpu",
                    "append submit queue={:p} caller_off=0x{:x} in_cmds={} append_handle=0x{:x}",
                    queue,
                    caller_off,
                    count,
                    append
                );
            }
            let out_count = count as usize + 1;
            let mut stack_handles = [0u64; 96];
            if out_count <= stack_handles.len() {
                core::ptr::copy_nonoverlapping(handles, stack_handles.as_mut_ptr(), count as usize);
                stack_handles[count as usize] = append;
                f(queue, out_count as i32, stack_handles.as_ptr());
                log_queue_error(queue, "post-append-submit");
                return;
            }

            let mut heap_handles = Vec::<crate::NvnCommandHandle>::with_capacity(out_count);
            heap_handles.set_len(out_count);
            core::ptr::copy_nonoverlapping(handles, heap_handles.as_mut_ptr(), count as usize);
            heap_handles[count as usize] = append;
            f(queue, out_count as i32, heap_handles.as_ptr());
            log_queue_error(queue, "post-append-submit");
            return;
        }
        f(queue, count, handles);
    }
}

unsafe extern "C" fn nvn_queue_present_texture_hook(
    queue: *mut crate::NvnQueue,
    window: *mut NvnWindow,
    index: i32,
) {
    on_queue_seen(queue);
    on_window_seen(window);
    on_present_queue_seen(queue);
    if GPU_PIPELINE_TRACE && QUEUE_PRESENT_LOG_COUNT < 8 {
        QUEUE_PRESENT_LOG_COUNT += 1;
        ncommon::logN!(
            target: "gpu",
            "nvnQueuePresentTexture queue={:p} window={:p} index={}",
            queue,
            window,
            index
        );
    }
    let overlay = match *PRESENT_SUBMIT_PROVIDER.lock() {
        Some(provider) => provider(queue, window, index),
        None => 0,
    };
    if overlay != 0 {
        if GPU_PIPELINE_TRACE && APPEND_SUBMIT_LOG_COUNT < 8 {
            APPEND_SUBMIT_LOG_COUNT += 1;
            ncommon::logN!(
                target: "gpu",
                "pre-present overlay submit queue={:p} index={} handle=0x{:x}",
                queue,
                index,
                overlay
            );
        }
        if let Some(submit) = QUEUE_SUBMIT_ORIG {
            submit(queue, 1, &overlay as *const crate::NvnCommandHandle);
            log_queue_error(queue, "pre-present-overlay-submit");
        }
    }
    if let Some(f) = QUEUE_PRESENT_TEXTURE_ORIG {
        f(queue, window, index);
    }
}

unsafe extern "C" fn nvn_window_builder_set_textures_hook(
    builder: *mut NvnWindowBuilder,
    count: i32,
    textures: *const *mut crate::NvnTexture,
) {
    if !textures.is_null() && count > 0 {
        runtime::cache_window_textures(textures, count);
    }
    if let Some(f) = WINDOW_SET_TEXTURES_ORIG {
        f(builder, count, textures);
    }
}

unsafe extern "C" fn nvn_window_acquire_texture_hook(
    window: *mut NvnWindow,
    sync: *mut crate::NvnSync,
    index: *mut i32,
) -> crate::NvnWindowAcquireTextureResult {
    let ret = match WINDOW_ACQUIRE_TEXTURE_ORIG {
        Some(f) => f(window, sync, index),
        None => 0,
    };
    if ret == 0 && !index.is_null() {
        runtime::set_active_window_texture_index(*index);
        if GPU_PIPELINE_TRACE && WINDOW_ACQUIRE_LOG_COUNT < 4 {
            WINDOW_ACQUIRE_LOG_COUNT += 1;
            let num_textures = crate::window::window_get_num_textures(window as *const NvnWindow);
            let num_active = crate::window::window_get_num_active_textures(window as *const NvnWindow);
            ncommon::logN!(
                target: "gpu",
                "nvnWindowAcquireTexture window={:p} index={} textures={} active={}",
                window,
                *index,
                num_textures,
                num_active
            );
        }
    } else if GPU_PIPELINE_TRACE && WINDOW_TEXTURE_CONFIG_LOG_COUNT < 2 && !window.is_null() {
        WINDOW_TEXTURE_CONFIG_LOG_COUNT += 1;
        let num_textures = crate::window::window_get_num_textures(window as *const NvnWindow);
        let num_active = crate::window::window_get_num_active_textures(window as *const NvnWindow);
        ncommon::logN!(
            target: "gpu",
            "window texture config window={:p} textures={} active={}",
            window,
            num_textures,
            num_active
        );
    }
    ret
}

unsafe extern "C" fn nvn_queue_acquire_texture_hook(
    queue: *mut crate::NvnQueue,
    window: *mut NvnWindow,
    index: *mut i32,
) -> crate::NvnQueueAcquireTextureResult {
    let _ = queue;
    let _ = window;
    let ret = match QUEUE_ACQUIRE_TEXTURE_ORIG {
        Some(f) => f(queue, window, index),
        None => 1,
    };
    if ret == 0 && !index.is_null() {
        runtime::set_active_window_texture_index(*index);
        if GPU_PIPELINE_TRACE && QUEUE_ACQUIRE_LOG_COUNT < 6 {
            QUEUE_ACQUIRE_LOG_COUNT += 1;
            ncommon::logN!(
                target: "gpu",
                "nvnQueueAcquireTexture queue={:p} window={:p} index={}",
                queue,
                window,
                *index
            );
        }
    }
    ret
}

unsafe extern "C" fn nvn_bootstrap_loader_hook(symbol: *const c_char) -> GenericFuncPtr {
    let ret = match BOOTSTRAP_LOADER_ORIG {
        Some(f) => f(symbol),
        None => None,
    };
    if ret.is_none() {
        return None;
    }

    let ptr = ret.unwrap();
    if cstr_eq_ascii(symbol, GET_PROC_NAME) {
        // Cache only the bootstrap entry; resolve_device_get_proc() stores the
        // device-resolved callable pointer used by runtime bridges.
        DEVICE_GET_PROC_BASE = Some(core::mem::transmute(ptr));
    } else if cstr_eq_ascii(symbol, DEVICE_INIT_NAME) && DEVICE_INITIALIZE_ORIG.is_none() {
        let mut trampoline: *mut c_void = core::ptr::null_mut();
        A64HookFunction(
            ptr as *const c_void,
            nvn_device_initialize_hook as *const c_void,
            &mut trampoline,
        );
        if !trampoline.is_null() {
            DEVICE_INITIALIZE_ORIG = Some(core::mem::transmute(trampoline));
        }
    }

    ret
}

#[inline(always)]
pub unsafe fn probe_driver_api_versions_from_device(device: *const NvnDevice) -> bool {
    let get_integer_fn = match resolve_device_get_integer(device) {
        Some(f) => f,
        None => return false,
    };
    probe_driver_api_versions_with(get_integer_fn, device)
}

pub unsafe fn install_device_hooks() -> bool {
    let mut trampoline: *mut c_void = core::ptr::null_mut();
    A64HookFunction(
        nvn_bootstrap_loader as *const c_void,
        nvn_bootstrap_loader_hook as *const c_void,
        &mut trampoline,
    );
    if trampoline.is_null() {
        return false;
    }

    BOOTSTRAP_LOADER_ORIG = Some(core::mem::transmute(trampoline));
    on_loader_called();
    true
}

#[inline(always)]
pub fn set_device_ptr(device: *mut NvnDevice) {
    on_device_seen(device);
}

#[inline(always)]
pub fn set_window_ptr(window: *mut NvnWindow) {
    on_window_seen(window);
}

#[inline(always)]
pub unsafe fn ensure_device_initialized(device: *mut NvnDevice) -> bool {
    if device.is_null() {
        return false;
    }
    on_loader_called();
    on_device_seen(device);

    if runtime::device_ready() {
        return true;
    }
    if !runtime::begin_device_init() {
        return runtime::device_ready();
    }

    let get_integer_fn = match resolve_device_get_integer(device) {
        Some(f) => f,
        None => {
            runtime::end_device_init(false);
            return false;
        }
    };

    let _ = probe_driver_api_versions_with(get_integer_fn, device);
    let _ = probe_draw_texture_support_with(get_integer_fn, device);
    runtime::end_device_init(true);
    true
}

#[inline(always)]
pub unsafe fn try_initialize_from_cached_device() -> bool {
    if let Some(device) = runtime::device_ptr() {
        ensure_device_initialized(device)
    } else {
        false
    }
}

#[inline(always)]
pub fn cached_device() -> Option<*mut NvnDevice> {
    runtime::device_ptr()
}

#[inline(always)]
pub fn cached_window() -> Option<*mut NvnWindow> {
    runtime::window_ptr()
}

#[inline(always)]
pub fn cached_queue() -> Option<*mut crate::NvnQueue> {
    runtime::queue_ptr()
}

#[inline(always)]
pub fn cached_present_queue() -> Option<*mut crate::NvnQueue> {
    runtime::present_queue_ptr()
}

#[inline(always)]
pub fn cached_active_window_texture() -> Option<*mut crate::NvnTexture> {
    runtime::active_window_texture_ptr()
}

#[inline(always)]
pub fn cached_window_texture(index: i32) -> Option<*mut crate::NvnTexture> {
    runtime::window_texture_ptr_at(index)
}

#[inline(always)]
pub fn cached_active_window_texture_index() -> Option<i32> {
    runtime::active_window_texture_index()
}

#[inline(always)]
pub fn cached_device_get_proc_address() -> Option<*mut c_void> {
    unsafe {
        if let Some(actual) = DEVICE_GET_PROC_ACTUAL {
            return Some(actual as *mut c_void);
        }
        if let Some(device) = runtime::device_ptr() {
            let _ = resolve_device_get_proc(device as *const NvnDevice);
        }
    }
    runtime::device_get_proc_addr().map(|addr| addr as *mut c_void)
}

#[inline(always)]
pub fn cached_first_window_texture() -> Option<*mut crate::NvnTexture> {
    runtime::first_window_texture_ptr()
}

#[inline(always)]
pub fn cached_window_textures_snapshot(
    out: &mut [*mut crate::NvnTexture; 8],
) -> usize {
    runtime::window_texture_ptrs_snapshot(out)
}

#[inline(always)]
pub fn tracked_submit_queues_snapshot(out: &mut [usize; MAX_TRACKED_QUEUES]) -> usize {
    unsafe {
        let len = SUBMIT_QUEUES_LEN.min(MAX_TRACKED_QUEUES);
        let mut i = 0usize;
        while i < len {
            out[i] = SUBMIT_QUEUES[i];
            i += 1;
        }
        while i < MAX_TRACKED_QUEUES {
            out[i] = 0;
            i += 1;
        }
        len
    }
}

#[inline(always)]
pub fn cached_driver_api_versions() -> Option<(i32, i32)> {
    runtime::driver_api_versions()
}

#[inline(always)]
pub fn cached_supports_draw_texture() -> Option<bool> {
    runtime::supports_draw_texture()
}

#[inline(always)]
pub fn bootstrap_active() -> bool {
    runtime::bootstrap_active()
}

#[inline(always)]
pub fn reset_bootstrap_state() {
    runtime::reset();
}

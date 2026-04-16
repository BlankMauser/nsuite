use core::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};

type MallocFn = unsafe extern "C" fn(usize) -> *mut c_void;
type AlignedAllocFn = unsafe extern "C" fn(usize, usize) -> *mut c_void;
type FreeFn = unsafe extern "C" fn(*mut c_void);

static MALLOC_FN: ncommon::symbol::CachedSymbol = ncommon::symbol::CachedSymbol::new(b"malloc\0");
static ALIGNED_ALLOC_FN: ncommon::symbol::CachedSymbol =
    ncommon::symbol::CachedSymbol::new(b"aligned_alloc\0");
static FREE_FN: ncommon::symbol::CachedSymbol = ncommon::symbol::CachedSymbol::new(b"free\0");

#[repr(C)]
struct RiveAllocHeader {
    base: *mut u8,
    alloc_size: usize,
    alloc_align: usize,
    payload_size: usize,
    payload_align: usize,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct RiveAllocatorStats {
    pub alloc_calls: usize,
    pub realloc_calls: usize,
    pub free_calls: usize,
    pub live_bytes: usize,
    pub peak_bytes: usize,
}

static RIVE_ALLOC_CALLS: AtomicUsize = AtomicUsize::new(0);
static RIVE_REALLOC_CALLS: AtomicUsize = AtomicUsize::new(0);
static RIVE_FREE_CALLS: AtomicUsize = AtomicUsize::new(0);
static RIVE_ALLOC_BYTES_LIVE: AtomicUsize = AtomicUsize::new(0);
static RIVE_ALLOC_BYTES_PEAK: AtomicUsize = AtomicUsize::new(0);

const RIVE_ALLOC_LOG_LIMIT: usize = 8;

#[inline(always)]
unsafe fn resolve_malloc() -> Option<MallocFn> {
    if !MALLOC_FN.is_initialized() {
        let _ = MALLOC_FN.init();
    }
    MALLOC_FN.get::<MallocFn>()
}

#[inline(always)]
unsafe fn resolve_aligned_alloc() -> Option<AlignedAllocFn> {
    if !ALIGNED_ALLOC_FN.is_initialized() {
        let _ = ALIGNED_ALLOC_FN.init();
    }
    ALIGNED_ALLOC_FN.get::<AlignedAllocFn>()
}

#[inline(always)]
unsafe fn resolve_free() -> Option<FreeFn> {
    if !FREE_FN.is_initialized() {
        let _ = FREE_FN.init();
    }
    FREE_FN.get::<FreeFn>()
}

#[inline(always)]
unsafe fn switch_alloc_base(size: usize, alignment: usize) -> *mut u8 {
    if size == 0 {
        return core::ptr::null_mut();
    }
    if alignment <= core::mem::align_of::<usize>() {
        return resolve_malloc()
            .map(|f| f(size).cast::<u8>())
            .unwrap_or(core::ptr::null_mut());
    }
    resolve_aligned_alloc()
        .map(|f| f(alignment, size).cast::<u8>())
        .unwrap_or(core::ptr::null_mut())
}

#[inline(always)]
const fn align_up(value: usize, align: usize) -> usize {
    (value + (align - 1)) & !(align - 1)
}

#[inline(always)]
fn track_live_bytes(delta: isize) {
    if delta >= 0 {
        let add = delta as usize;
        let live = RIVE_ALLOC_BYTES_LIVE.fetch_add(add, Ordering::AcqRel) + add;
        let mut peak = RIVE_ALLOC_BYTES_PEAK.load(Ordering::Acquire);
        while live > peak {
            match RIVE_ALLOC_BYTES_PEAK.compare_exchange(
                peak,
                live,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(cur) => peak = cur,
            }
        }
    } else {
        let sub = (-delta) as usize;
        let _ = RIVE_ALLOC_BYTES_LIVE.fetch_update(
            Ordering::AcqRel,
            Ordering::Acquire,
            |live| Some(live.saturating_sub(sub)),
        );
    }
}

pub fn snapshot_rive_allocator_stats() -> RiveAllocatorStats {
    RiveAllocatorStats {
        alloc_calls: RIVE_ALLOC_CALLS.load(Ordering::Acquire),
        realloc_calls: RIVE_REALLOC_CALLS.load(Ordering::Acquire),
        free_calls: RIVE_FREE_CALLS.load(Ordering::Acquire),
        live_bytes: RIVE_ALLOC_BYTES_LIVE.load(Ordering::Acquire),
        peak_bytes: RIVE_ALLOC_BYTES_PEAK.load(Ordering::Acquire),
    }
}

pub fn log_rive_allocator_snapshot(reason: &str) {
    let stats = snapshot_rive_allocator_stats();
    ncommon::logN!(
        target: "overlay.rive",
        "rive allocator {} alloc={} realloc={} free={} live={} peak={}",
        reason,
        stats.alloc_calls,
        stats.realloc_calls,
        stats.free_calls,
        stats.live_bytes,
        stats.peak_bytes
    );
}

unsafe extern "C" fn rive_alloc(
    size: usize,
    alignment: usize,
    _user: *mut c_void,
) -> *mut c_void {
    if size == 0 {
        return core::ptr::null_mut();
    }
    let requested_align = alignment.max(core::mem::align_of::<usize>());
    if !requested_align.is_power_of_two() {
        return core::ptr::null_mut();
    }

    let header_size = core::mem::size_of::<RiveAllocHeader>();
    let alloc_align = requested_align.max(core::mem::align_of::<RiveAllocHeader>());
    let alloc_size = match size
        .checked_add(requested_align)
        .and_then(|v| v.checked_add(header_size))
    {
        Some(v) => v,
        None => return core::ptr::null_mut(),
    };
    let base = switch_alloc_base(alloc_size, alloc_align);
    if base.is_null() {
        return core::ptr::null_mut();
    }

    let payload_start = base.add(header_size) as usize;
    let payload_addr = align_up(payload_start, requested_align) as *mut u8;
    let header_ptr = payload_addr.sub(header_size).cast::<RiveAllocHeader>();
    header_ptr.write(RiveAllocHeader {
        base,
        alloc_size,
        alloc_align,
        payload_size: size,
        payload_align: requested_align,
    });

    let call = RIVE_ALLOC_CALLS.fetch_add(1, Ordering::AcqRel) + 1;
    track_live_bytes(size as isize);
    if call <= RIVE_ALLOC_LOG_LIMIT {
        ncommon::logN!(
            target: "overlay.rive",
            "rive_alloc call={} size={} align={} ptr={:p}",
            call,
            size,
            requested_align,
            payload_addr
        );
    }
    payload_addr.cast()
}

unsafe extern "C" fn rive_free(ptr: *mut c_void, _user: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    let payload = ptr.cast::<u8>();
    let header_ptr = payload
        .sub(core::mem::size_of::<RiveAllocHeader>())
        .cast::<RiveAllocHeader>();
    let header = header_ptr.read();
    if header.base.is_null() {
        return;
    }

    RIVE_FREE_CALLS.fetch_add(1, Ordering::AcqRel);
    track_live_bytes(-(header.payload_size as isize));

    if let Some(free_fn) = resolve_free() {
        free_fn(header.base.cast());
    }
}

unsafe extern "C" fn rive_realloc(
    ptr: *mut c_void,
    new_size: usize,
    _user: *mut c_void,
) -> *mut c_void {
    if ptr.is_null() {
        return rive_alloc(new_size, core::mem::align_of::<usize>(), core::ptr::null_mut());
    }
    if new_size == 0 {
        rive_free(ptr, core::ptr::null_mut());
        return core::ptr::null_mut();
    }

    let payload = ptr.cast::<u8>();
    let header_ptr = payload
        .sub(core::mem::size_of::<RiveAllocHeader>())
        .cast::<RiveAllocHeader>();
    let header = header_ptr.read();
    if header.payload_align == 0 || !header.payload_align.is_power_of_two() {
        return core::ptr::null_mut();
    }

    RIVE_REALLOC_CALLS.fetch_add(1, Ordering::AcqRel);
    let new_ptr = rive_alloc(new_size, header.payload_align, core::ptr::null_mut());
    if new_ptr.is_null() {
        header_ptr.write(header);
        return core::ptr::null_mut();
    }

    let copy_len = header.payload_size.min(new_size);
    core::ptr::copy_nonoverlapping(payload, new_ptr.cast::<u8>(), copy_len);
    if let Some(free_fn) = resolve_free() {
        free_fn(header.base.cast());
    }
    track_live_bytes(-(header.payload_size as isize));
    new_ptr
}

static RIVE_NGPU_ALLOCATOR: ngpu::cpp::shim::NgpuAllocator = ngpu::cpp::shim::NgpuAllocator {
    alloc: Some(rive_alloc),
    realloc: Some(rive_realloc),
    free: Some(rive_free),
    user: core::ptr::null_mut(),
};

pub fn ngpu_rive_allocator() -> &'static ngpu::cpp::shim::NgpuAllocator {
    &RIVE_NGPU_ALLOCATOR
}

pub fn install_ngpu_rive_allocator() {
    ngpu::cpp::shim::ngpu_shim_set_allocator(&RIVE_NGPU_ALLOCATOR as *const _);
}

pub fn clear_ngpu_rive_allocator() {
    ngpu::cpp::shim::ngpu_shim_set_allocator(core::ptr::null());
}

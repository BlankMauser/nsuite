use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
#[cfg(feature = "rive-host-ffi")]
use std::time::Instant;

use skyline::libc::{c_char, c_void};

use nsuite::ngpu;
use nsuite::nmem;
use ngpu::debug::dbg_draw::{clear_draw_list, lines_snapshot, DebugLine};
type OverlayHandleProvider = fn(*mut ngpu::NvnQueue) -> ngpu::NvnCommandHandle;

const OVERLAY_TRACE: bool = false;
const OVERLAY_RENDER_VIA_PRESENT_HOOK: bool = true;

static OVERLAY_HANDLE_PROVIDER: locks::Mutex<Option<OverlayHandleProvider>> =
    locks::Mutex::new(None);

static TARGET_CALLER_TEXT_OFF: AtomicUsize = AtomicUsize::new(0);
static TARGET_QUEUE_PTR: AtomicUsize = AtomicUsize::new(0);
static AUTO_TARGET_QUEUE_PTR: AtomicUsize = AtomicUsize::new(0);
static APPEND_PASS_COUNT: AtomicUsize = AtomicUsize::new(0);
static WARNED_NO_LINES: AtomicUsize = AtomicUsize::new(0);
static WARNED_NO_TEXTURE: AtomicUsize = AtomicUsize::new(0);
static HANDLE_NONZERO_COUNT: AtomicUsize = AtomicUsize::new(0);
static LAST_EMITTED_ACQUIRE_INDEX: AtomicI32 = AtomicI32::new(-1);
static ACQUIRE_INDEX_GATE_LOG_COUNT: AtomicUsize = AtomicUsize::new(0);
static LAST_PRESENT_SUBMIT_INDEX: AtomicI32 = AtomicI32::new(-1);
static LAST_PRESENT_SUBMIT_QUEUE: AtomicUsize = AtomicUsize::new(0);
static PRESENT_INDEX_GATE_LOG_COUNT: AtomicUsize = AtomicUsize::new(0);
static OVERLAY_SLOT_WAIT_LOG_COUNT: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "rive-host-ffi")]
static WARNED_RIVE_INIT_FAIL: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "rive-host-ffi")]
static WARNED_RIVE_DRAW_FAIL: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "rive-host-ffi")]
static RIVE_INFLIGHT_GATE_LOG_COUNT: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "rive-host-ffi")]
static RIVE_PAYLOAD: locks::Mutex<Option<Vec<u8>>> = locks::Mutex::new(None);
#[cfg(feature = "rive-host-ffi")]
static WARNED_RIVE_PREWARM_FAIL: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "rive-host-ffi")]
static WARNED_RIVE_PAYLOAD_VALIDATE_FAIL: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "rive-host-ffi")]
static RIVE_PREWARM_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "rive-host-ffi")]
static RIVE_NVN_LOG_LINES_EMITTED: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "rive-host-ffi")]
const RIVE_PREWARM_WHILE_DISABLED: bool = false;
#[cfg(feature = "rive-host-ffi")]
const RIVE_PREWARM_MAX_ATTEMPTS: usize = 8;
#[cfg(feature = "rive-host-ffi")]
static RIVE_RUNTIME_ENABLED_FLAG: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "rive-host-ffi")]
const RIVE_USE_CUSTOM_ALLOCATOR: bool = true;
#[cfg(feature = "rive-host-ffi")]
const RIVE_DROP_STATE_ON_DRAW_FAIL: bool = true;
#[cfg(feature = "rive-host-ffi")]
const RIVE_ENABLE_ADVANCE: bool = true;
#[cfg(feature = "rive-host-ffi")]

const RIVE_ENABLE_NVN_LOG_CALLBACK: bool = false;
#[cfg(feature = "rive-host-ffi")]
const RIVE_NVN_LOG_LINE_BUDGET: usize = 96;
const RIVE_PREFER_SPIRV_INPUT: bool = false;
#[cfg(feature = "rive-host-ffi")]
const RIVE_BUILD_STAMP: &str = match option_env!("SSBUSYNC_BUILD_STAMP") {
    Some(value) => value,
    None => "missing_build_stamp",
};
#[cfg(feature = "rive-host-ffi")]
const RIVE_ARTBOARD_INDEX: Option<usize> = Some(0);
#[cfg(feature = "rive-host-ffi")]
const RIVE_ARTBOARD_NAME: Option<&str> = None;
#[cfg(feature = "rive-host-ffi")]
const RIVE_DEBUG_FORCE_CLEAR_FIRST_FRAME: bool = false;
#[cfg(feature = "rive-host-ffi")]
const RIVE_DEBUG_FORCE_CLEAR_COLOR: [f32; 4] = [1.0, 0.0, 1.0, 1.0];
#[cfg(feature = "rive-host-ffi")]
#[cfg(feature = "rive-host-ffi")]
const NVN_MAX_TEXTURE_SIZE_NX: u32 = 16384;

const POOL_FLAGS_CPU_UNCACHED_GPU_UNCACHED: i32 = 0x00000002 | 0x00000010;
const NVN_CLEAR_COLOR_MASK_RGBA: i32 = 0x0000000F;
const NVN_SYNC_CONDITION_ALL_GPU_COMMANDS_COMPLETE: i32 = 0;
const NVN_SYNC_WAIT_ALREADY_SIGNALED: i32 = 0;
const NVN_SYNC_WAIT_CONDITION_SATISFIED: i32 = 1;
const NVN_SYNC_WAIT_TIMEOUT_EXPIRED: i32 = 2;
const NVN_SYNC_WAIT_FAILED: i32 = 3;
const NVN_DEVICE_INFO_COMMAND_BUFFER_COMMAND_ALIGNMENT: ngpu::NvnDeviceInfo = 19;
const NVN_DEVICE_INFO_COMMAND_BUFFER_CONTROL_ALIGNMENT: ngpu::NvnDeviceInfo = 20;
const NVN_WINDOW_ORIGIN_MODE_LOWER_LEFT: ngpu::NvnWindowOriginMode = 0;
const NVN_WINDOW_ORIGIN_MODE_UPPER_LEFT: ngpu::NvnWindowOriginMode = 1;
const OVERLAY_RING_MIN_LEN: usize = 8;
const OVERLAY_CMDBUF_COMMAND_BYTES: usize = 0x40000;
const OVERLAY_CMDBUF_CONTROL_BYTES: usize = 0x20000;

#[repr(C, align(8))]
struct RawNvnSync {
    reserved: [u8; 64],
}

#[repr(C)]
struct RawNvnCopyRegion {
    xoffset: i32,
    yoffset: i32,
    zoffset: i32,
    width: i32,
    height: i32,
    depth: i32,
}

struct OverlayCommandSlot {
    cmdbuf: nmem::OwnedCommandBuffer,
    sync: Box<RawNvnSync>,
    submitted_once: bool,
    last_rive_frame: u64,
}

impl OverlayCommandSlot {
    #[inline(always)]
    fn sync_ptr(&self) -> *const ngpu::NvnSync {
        (&*self.sync as *const RawNvnSync).cast()
    }

    #[inline(always)]
    fn sync_mut_ptr(&mut self) -> *mut ngpu::NvnSync {
        (&mut *self.sync as *mut RawNvnSync).cast()
    }
}

struct OverlayRenderer {
    _arena: nmem::CommandBufferArena,
    slots: Vec<OverlayCommandSlot>,
    frame_slot: usize,
    tick: u32,
    completed_rive_frame: u64,
    #[cfg(feature = "rive-host-ffi")]
    rive: Option<RiveOverlayState>,
}

static OVERLAY_RENDERER: locks::Mutex<Option<OverlayRenderer>> = locks::Mutex::new(None);

#[cfg(feature = "rive-host-ffi")]
mod rive_api {
    use core::ffi::{c_char, c_int, c_void};

    #[repr(C)]
    pub struct RiveFile {
        _private: [u8; 0],
    }
    #[repr(C)]
    pub struct RiveArtboard {
        _private: [u8; 0],
    }
    #[repr(C)]
    pub struct RiveLinearAnimation {
        _private: [u8; 0],
    }
    #[repr(C)]
    pub struct RiveStateMachine {
        _private: [u8; 0],
    }
    #[repr(C)]
    pub struct RiveRenderer {
        _private: [u8; 0],
    }
    #[repr(C)]
    pub struct RiveRenderContext {
        _private: [u8; 0],
    }
    #[repr(C)]
    pub struct RiveRenderTarget {
        _private: [u8; 0],
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct RiveAabb {
        pub min_x: f32,
        pub min_y: f32,
        pub max_x: f32,
        pub max_y: f32,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct RiveAlignment {
        pub x: f32,
        pub y: f32,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub enum RiveFit {
        Contain = 1,
        None = 5,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub enum RiveLoadAction {
        PreserveRenderTarget = 1,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct RiveFrameDescriptor {
        pub render_target_width: u32,
        pub render_target_height: u32,
        pub load_action: RiveLoadAction,
        pub clear_color: u32,
        pub msaa_sample_count: u32,
        pub disable_raster_ordering: u8,
        pub wireframe: u8,
        pub fills_disabled: u8,
        pub strokes_disabled: u8,
        pub clockwise_fill_override: u8,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct RiveFlushResources {
        pub render_target: *mut RiveRenderTarget,
        pub external_command_buffer: *mut c_void,
        pub current_frame_number: u64,
        pub safe_frame_number: u64,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct RiveRendererDebugStats {
        pub null_draw: u32,
        pub outside_frame: u32,
        pub apply_clip_failure: u32,
        pub apply_clip_empty: u32,
        pub push_draw_failure: u32,
        pub push_draw_success: u32,
        pub exhausted_retries: u32,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct RiveNVNAllocator {
        pub alloc: Option<unsafe extern "C" fn(usize, usize, *mut c_void) -> *mut c_void>,
        pub realloc: Option<unsafe extern "C" fn(*mut c_void, usize, *mut c_void) -> *mut c_void>,
        pub free: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
        pub user: *mut c_void,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct RiveNVNContextDesc {
        pub device: *mut c_void,
        pub queue: *mut c_void,
        pub get_proc_address: *mut c_void,
        pub max_texture_size: u32,
        pub clip_space_bottom_up: u8,
        pub framebuffer_bottom_up: u8,
        pub allocator: *const RiveNVNAllocator,
    }

    unsafe extern "C" {
        pub fn rive_set_log_callback(callback: Option<unsafe extern "C" fn(*const c_char)>);
        pub fn rive_nvn_set_log_callback(callback: Option<unsafe extern "C" fn(*const c_char)>);
        pub fn rive_runtime_build_stamp() -> *const c_char;
        pub fn rive_renderer_build_stamp() -> *const c_char;
        pub fn rive_runtime_set_allocator(allocator: *const RiveNVNAllocator);
        pub fn rive_nvn_set_prefer_spirv_input(enabled: c_int);
        pub fn rive_nvn_uses_spirv_input() -> c_int;
        pub fn rive_file_import(data: *const u8, data_size: usize, out_result: *mut i32) -> *mut RiveFile;
        pub fn rive_file_rebind_render_context(
            file: *mut RiveFile,
            render_context: *mut RiveRenderContext,
        );
        pub fn rive_file_release(file: *mut RiveFile);
        pub fn rive_file_artboard_count(file: *const RiveFile) -> usize;
        pub fn rive_file_artboard_name(
            file: *const RiveFile,
            index: usize,
            buffer: *mut c_char,
            buffer_size: usize,
        ) -> usize;
        pub fn rive_file_artboard_default(file: *mut RiveFile) -> *mut RiveArtboard;
        pub fn rive_file_artboard_at(file: *mut RiveFile, index: usize) -> *mut RiveArtboard;
        pub fn rive_file_artboard_named(file: *mut RiveFile, name: *const c_char) -> *mut RiveArtboard;
        pub fn rive_artboard_release(artboard: *mut RiveArtboard);
        pub fn rive_artboard_animation_at(
            artboard: *mut RiveArtboard,
            index: usize,
        ) -> *mut RiveLinearAnimation;
        pub fn rive_artboard_state_machine_count(artboard: *const RiveArtboard) -> usize;
        pub fn rive_artboard_state_machine_at(
            artboard: *mut RiveArtboard,
            index: usize,
        ) -> *mut RiveStateMachine;
        pub fn rive_artboard_advance(artboard: *mut RiveArtboard, elapsed_seconds: f32) -> c_int;
        pub fn rive_artboard_width(artboard: *const RiveArtboard) -> f32;
        pub fn rive_artboard_height(artboard: *const RiveArtboard) -> f32;
        pub fn rive_artboard_bounds(artboard: *const RiveArtboard, out_bounds: *mut RiveAabb) -> c_int;
        pub fn rive_artboard_object_count(artboard: *const RiveArtboard) -> usize;
        pub fn rive_artboard_drawable_count(artboard: *const RiveArtboard) -> usize;
        pub fn rive_artboard_will_draw_count(artboard: *const RiveArtboard) -> usize;
        pub fn rive_artboard_hidden_drawable_count(artboard: *const RiveArtboard) -> usize;
        pub fn rive_linear_animation_release(animation: *mut RiveLinearAnimation);
        pub fn rive_linear_animation_advance_and_apply(
            animation: *mut RiveLinearAnimation,
            elapsed_seconds: f32,
            mix: f32,
        ) -> c_int;
        pub fn rive_state_machine_release(machine: *mut RiveStateMachine);
        pub fn rive_state_machine_advance(
            machine: *mut RiveStateMachine,
            elapsed_seconds: f32,
        ) -> c_int;
        pub fn rive_state_machine_advance_and_apply(
            machine: *mut RiveStateMachine,
            elapsed_seconds: f32,
        ) -> c_int;
        pub fn rive_state_machine_fire_trigger(
            machine: *mut RiveStateMachine,
            name: *const c_char,
        ) -> c_int;
        pub fn rive_state_machine_input_count(machine: *const RiveStateMachine) -> usize;
        pub fn rive_state_machine_input_core_type(
            machine: *const RiveStateMachine,
            index: usize,
        ) -> u16;
        pub fn rive_state_machine_fire_trigger_at(
            machine: *mut RiveStateMachine,
            index: usize,
        ) -> c_int;
        pub fn rive_state_machine_bind_default_view_model(
            file: *mut RiveFile,
            artboard: *mut RiveArtboard,
            machine: *mut RiveStateMachine,
        ) -> c_int;
        pub fn rive_nvn_render_context_new(desc: *const RiveNVNContextDesc) -> *mut RiveRenderContext;
        pub fn rive_nvn_initialize(device: *mut c_void, get_proc_address: *mut c_void);
        pub fn rive_render_context_release(context: *mut RiveRenderContext);
        pub fn rive_render_context_begin_frame(context: *mut RiveRenderContext, frame_desc: *const RiveFrameDescriptor);
        pub fn rive_render_context_flush(context: *mut RiveRenderContext, resources: *const RiveFlushResources);
        pub fn rive_renderer_new(context: *mut RiveRenderContext) -> *mut RiveRenderer;
        pub fn rive_renderer_release(renderer: *mut RiveRenderer);
        pub fn rive_renderer_save(renderer: *mut RiveRenderer);
        pub fn rive_renderer_restore(renderer: *mut RiveRenderer);
        pub fn rive_renderer_align(
            renderer: *mut RiveRenderer,
            fit: RiveFit,
            alignment: RiveAlignment,
            frame: RiveAabb,
            content: RiveAabb,
            scale_factor: f32,
        );
        pub fn rive_renderer_draw_artboard(renderer: *mut RiveRenderer, artboard: *mut RiveArtboard);
        pub fn rive_renderer_get_debug_stats(
            renderer: *const RiveRenderer,
            out_stats: *mut RiveRendererDebugStats,
        ) -> c_int;
        pub fn rive_renderer_reset_debug_stats(renderer: *mut RiveRenderer);
        pub fn rive_nvn_render_target_new(
            width: u32,
            height: u32,
            color_texture: *mut c_void,
            depth_texture: *mut c_void,
            sample_count: u32,
        ) -> *mut RiveRenderTarget;
        pub fn rive_render_target_release(target: *mut RiveRenderTarget);
    }

    pub struct File {
        raw: *mut RiveFile,
    }
    impl File {
        pub unsafe fn import(data: &[u8]) -> Result<Self, i32> {
            let mut out = 2i32;
            let raw = rive_file_import(data.as_ptr(), data.len(), &mut out);
            if raw.is_null() || out != 0 {
                Err(out)
            } else {
                Ok(Self { raw })
            }
        }
        pub unsafe fn rebind_render_context(
            &mut self,
            render_context: *mut RiveRenderContext,
        ) {
            if self.raw.is_null() || render_context.is_null() {
                return;
            }
            rive_file_rebind_render_context(self.raw, render_context);
        }
        pub unsafe fn artboard_count(&self) -> usize {
            rive_file_artboard_count(self.raw)
        }
        pub unsafe fn artboard_name(&self, index: usize) -> Option<String> {
            let mut buf = [0u8; 128];
            let written = rive_file_artboard_name(
                self.raw,
                index,
                buf.as_mut_ptr().cast::<c_char>(),
                buf.len(),
            );
            if written == 0 {
                return None;
            }
            let len = written.min(buf.len().saturating_sub(1));
            Some(String::from_utf8_lossy(&buf[..len]).into_owned())
        }
        pub unsafe fn artboard_default(&self) -> Option<Artboard> {
            let raw = rive_file_artboard_default(self.raw);
            if raw.is_null() {
                None
            } else {
                Some(Artboard { raw })
            }
        }
        pub unsafe fn artboard_at(&self, index: usize) -> Option<Artboard> {
            let raw = rive_file_artboard_at(self.raw, index);
            if raw.is_null() {
                None
            } else {
                Some(Artboard { raw })
            }
        }
        pub unsafe fn artboard_named(&self, name: &str) -> Option<Artboard> {
            let c_name = std::ffi::CString::new(name).ok()?;
            let raw = rive_file_artboard_named(self.raw, c_name.as_ptr());
            if raw.is_null() {
                None
            } else {
                Some(Artboard { raw })
            }
        }
    }
    impl Drop for File {
        fn drop(&mut self) {
            unsafe { rive_file_release(self.raw) }
        }
    }

    pub struct Artboard {
        raw: *mut RiveArtboard,
    }
    impl Artboard {
        pub unsafe fn animation_at(&self, index: usize) -> Option<LinearAnimation> {
            let raw = rive_artboard_animation_at(self.raw, index);
            if raw.is_null() {
                None
            } else {
                Some(LinearAnimation { raw })
            }
        }
        pub unsafe fn state_machine_count(&self) -> usize {
            rive_artboard_state_machine_count(self.raw)
        }
        pub unsafe fn state_machine_at(&self, index: usize) -> Option<StateMachine> {
            let raw = rive_artboard_state_machine_at(self.raw, index);
            if raw.is_null() {
                None
            } else {
                Some(StateMachine { raw })
            }
        }
        pub unsafe fn advance(&mut self, seconds: f32) -> bool {
            rive_artboard_advance(self.raw, seconds) != 0
        }
        pub unsafe fn width(&self) -> f32 {
            rive_artboard_width(self.raw)
        }
        pub unsafe fn height(&self) -> f32 {
            rive_artboard_height(self.raw)
        }
        pub unsafe fn bounds(&self) -> Option<RiveAabb> {
            let mut out = RiveAabb {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 0.0,
                max_y: 0.0,
            };
            if rive_artboard_bounds(self.raw, &mut out as *mut _) != 0 {
                Some(out)
            } else {
                None
            }
        }
        pub unsafe fn object_count(&self) -> usize {
            rive_artboard_object_count(self.raw)
        }
        pub unsafe fn drawable_count(&self) -> usize {
            rive_artboard_drawable_count(self.raw)
        }
        pub unsafe fn will_draw_count(&self) -> usize {
            rive_artboard_will_draw_count(self.raw)
        }
        pub unsafe fn hidden_drawable_count(&self) -> usize {
            rive_artboard_hidden_drawable_count(self.raw)
        }
        pub fn as_mut_ptr(&mut self) -> *mut RiveArtboard {
            self.raw
        }
    }
    impl Drop for Artboard {
        fn drop(&mut self) {
            unsafe { rive_artboard_release(self.raw) }
        }
    }

    pub struct LinearAnimation {
        raw: *mut RiveLinearAnimation,
    }
    impl LinearAnimation {
        pub unsafe fn advance_and_apply(&mut self, elapsed: f32, mix: f32) -> bool {
            rive_linear_animation_advance_and_apply(self.raw, elapsed, mix) != 0
        }
    }
    impl Drop for LinearAnimation {
        fn drop(&mut self) {
            unsafe { rive_linear_animation_release(self.raw) }
        }
    }

    pub struct StateMachine {
        raw: *mut RiveStateMachine,
    }
    impl StateMachine {
        pub unsafe fn advance(&mut self, elapsed: f32) -> bool {
            rive_state_machine_advance(self.raw, elapsed) != 0
        }
        pub unsafe fn advance_and_apply(&mut self, elapsed: f32) -> bool {
            rive_state_machine_advance_and_apply(self.raw, elapsed) != 0
        }
        pub unsafe fn fire_trigger(&mut self, name: *const c_char) -> bool {
            rive_state_machine_fire_trigger(self.raw, name) != 0
        }
        pub unsafe fn input_count(&self) -> usize {
            rive_state_machine_input_count(self.raw)
        }
        pub unsafe fn input_core_type(&self, index: usize) -> u16 {
            rive_state_machine_input_core_type(self.raw, index)
        }
        pub unsafe fn fire_trigger_at(&mut self, index: usize) -> bool {
            rive_state_machine_fire_trigger_at(self.raw, index) != 0
        }
        pub unsafe fn bind_default_view_model(
            &mut self,
            file: &File,
            artboard: &Artboard,
        ) -> bool {
            rive_state_machine_bind_default_view_model(file.raw, artboard.raw, self.raw) != 0
        }
    }
    impl Drop for StateMachine {
        fn drop(&mut self) {
            unsafe { rive_state_machine_release(self.raw) }
        }
    }

    pub struct RenderContext {
        raw: *mut RiveRenderContext,
    }
    impl RenderContext {
        pub unsafe fn new(desc: &RiveNVNContextDesc) -> Option<Self> {
            let raw = rive_nvn_render_context_new(desc as *const _);
            if raw.is_null() {
                None
            } else {
                Some(Self { raw })
            }
        }
        pub unsafe fn begin_frame(&mut self, frame: &RiveFrameDescriptor) {
            rive_render_context_begin_frame(self.raw, frame as *const _);
        }
        pub unsafe fn flush(&mut self, flush: &RiveFlushResources) {
            rive_render_context_flush(self.raw, flush as *const _);
        }
        pub fn as_ptr(&self) -> *mut RiveRenderContext {
            self.raw
        }
    }
    impl Drop for RenderContext {
        fn drop(&mut self) {
            unsafe { rive_render_context_release(self.raw) }
        }
    }

    pub struct Renderer {
        raw: *mut RiveRenderer,
    }
    impl Renderer {
        pub unsafe fn new(context: &RenderContext) -> Option<Self> {
            let raw = rive_renderer_new(context.as_ptr());
            if raw.is_null() {
                None
            } else {
                Some(Self { raw })
            }
        }
        pub unsafe fn save(&mut self) {
            rive_renderer_save(self.raw);
        }
        pub unsafe fn restore(&mut self) {
            rive_renderer_restore(self.raw);
        }
        pub unsafe fn align(
            &mut self,
            fit: RiveFit,
            alignment: RiveAlignment,
            frame: RiveAabb,
            content: RiveAabb,
            scale_factor: f32,
        ) {
            rive_renderer_align(self.raw, fit, alignment, frame, content, scale_factor);
        }
        pub unsafe fn draw_artboard(&mut self, artboard: &mut Artboard) {
            rive_renderer_draw_artboard(self.raw, artboard.as_mut_ptr());
        }
        pub unsafe fn debug_stats(&self) -> Option<RiveRendererDebugStats> {
            let mut stats = RiveRendererDebugStats {
                null_draw: 0,
                outside_frame: 0,
                apply_clip_failure: 0,
                apply_clip_empty: 0,
                push_draw_failure: 0,
                push_draw_success: 0,
                exhausted_retries: 0,
            };
            if rive_renderer_get_debug_stats(self.raw, &mut stats as *mut _) != 0 {
                Some(stats)
            } else {
                None
            }
        }
        pub unsafe fn reset_debug_stats(&mut self) {
            rive_renderer_reset_debug_stats(self.raw);
        }
    }
    impl Drop for Renderer {
        fn drop(&mut self) {
            unsafe { rive_renderer_release(self.raw) }
        }
    }

    pub struct RenderTarget {
        raw: *mut RiveRenderTarget,
    }
    impl RenderTarget {
        pub unsafe fn new(
            width: u32,
            height: u32,
            color_texture: *mut c_void,
            depth_texture: *mut c_void,
            sample_count: u32,
        ) -> Option<Self> {
            let raw = rive_nvn_render_target_new(
                width,
                height,
                color_texture,
                depth_texture,
                sample_count,
            );
            if raw.is_null() {
                None
            } else {
                Some(Self { raw })
            }
        }
        pub fn as_ptr(&self) -> *mut RiveRenderTarget {
            self.raw
        }
    }
    impl Drop for RenderTarget {
        fn drop(&mut self) {
            unsafe { rive_render_target_release(self.raw) }
        }
    }
}

#[cfg(feature = "rive-host-ffi")]
unsafe extern "C" fn rive_nvn_debug_log_bridge(message: *const core::ffi::c_char) {
    if message.is_null() {
        return;
    }
    let line_index = RIVE_NVN_LOG_LINES_EMITTED.fetch_add(1, Ordering::AcqRel);
    if line_index >= RIVE_NVN_LOG_LINE_BUDGET {
        return;
    }
    ncommon::logger::write_c_string_line(message as *const c_char);
}

#[cfg(feature = "rive-host-ffi")]
struct RiveOverlayState {
    _file: rive_api::File,
    artboard: rive_api::Artboard,
    animation: Option<rive_api::LinearAnimation>,
    state_machine: Option<rive_api::StateMachine>,
    context: rive_api::RenderContext,
    renderer: rive_api::Renderer,
    _allocator_desc: Option<Box<rive_api::RiveNVNAllocator>>,
    target: Option<rive_api::RenderTarget>,
    last_texture_ptr: usize,
    target_width: u32,
    target_height: u32,
    frame_number: u64,
    bootstrap_trigger_armed: bool,
}

impl Drop for OverlayCommandSlot {
    fn drop(&mut self) {
        unsafe {
            ngpu::sync::sync_finalize(self.sync_mut_ptr());
        }
    }
}

const fn align_up(value: usize, align: usize) -> usize {
    (value + (align - 1)) & !(align - 1)
}

unsafe fn query_command_buffer_alignments(device: *mut ngpu::NvnDevice) -> (usize, usize) {
    let mut command_align = 0i32;
    let mut control_align = 0i32;
    ngpu::device::device_get_integer(
        device as *const ngpu::NvnDevice,
        NVN_DEVICE_INFO_COMMAND_BUFFER_COMMAND_ALIGNMENT,
        &mut command_align as *mut i32,
    );
    ngpu::device::device_get_integer(
        device as *const ngpu::NvnDevice,
        NVN_DEVICE_INFO_COMMAND_BUFFER_CONTROL_ALIGNMENT,
        &mut control_align as *mut i32,
    );

    let command = if command_align > 0 {
        command_align as usize
    } else {
        0x100
    };
    let control = if control_align > 0 {
        control_align as usize
    } else {
        0x40
    };

    (
        if command.is_power_of_two() { command } else { 0x100 },
        if control.is_power_of_two() { control } else { 0x40 },
    )
}

#[inline(always)]
pub fn set_submit_filter(caller_text_off: usize, queue: Option<*mut ngpu::NvnQueue>) {
    TARGET_CALLER_TEXT_OFF.store(caller_text_off, Ordering::Release);
    TARGET_QUEUE_PTR.store(queue.unwrap_or(core::ptr::null_mut()) as usize, Ordering::Release);
    AUTO_TARGET_QUEUE_PTR.store(0, Ordering::Release);
}

#[inline(always)]
pub fn set_overlay_handle_provider(provider: Option<OverlayHandleProvider>) {
    *OVERLAY_HANDLE_PROVIDER.lock() = provider;
}

#[cfg(feature = "rive-host-ffi")]
#[inline(always)]
fn log_rive_allocator_snapshot(reason: &str) {
    nmem::log_rive_allocator_snapshot(reason);
}
#[cfg(feature = "rive-host-ffi")]
#[inline(always)]
fn rive_import_result_name(result: i32) -> &'static str {
    match result {
        0 => "success",
        1 => "unsupported_version",
        2 => "malformed",
        _ => "unknown",
    }
}

#[cfg(feature = "rive-host-ffi")]
unsafe fn rive_build_stamp_string(value: *const core::ffi::c_char) -> String {
    if value.is_null() {
        return "null_build_stamp".to_string();
    }
    match std::ffi::CStr::from_ptr(value).to_str() {
        Ok(text) => text.to_string(),
        Err(_) => std::ffi::CStr::from_ptr(value).to_string_lossy().into_owned(),
    }
}


#[cfg(feature = "rive-host-ffi")]
impl Drop for RiveOverlayState {
    fn drop(&mut self) {
        ncommon::logN!(
            target: "overlay.rive",
            "dropping rive overlay state frame_count={} target={}x{}",
            self.frame_number,
            self.target_width,
            self.target_height
        );
        log_rive_allocator_snapshot("on-drop");
    }
}

#[cfg(feature = "rive-host-ffi")]
#[inline(always)]
pub fn rive_runtime_enabled() -> bool {
    #[cfg(feature = "rive-host-ffi")]
    {
        RIVE_RUNTIME_ENABLED_FLAG.load(Ordering::Acquire)
    }
    #[cfg(not(feature = "rive-host-ffi"))]
    {
        false
    }
}

pub fn set_rive_runtime_enabled(enabled: bool) {
    #[cfg(feature = "rive-host-ffi")]
    {
        RIVE_RUNTIME_ENABLED_FLAG.store(enabled, Ordering::Release);
        ncommon::logN!(target: "overlay.rive", "rive runtime enabled={}", enabled);
    }
}

pub fn toggle_rive_runtime_enabled() -> bool {
    #[cfg(feature = "rive-host-ffi")]
    {
        let next = !RIVE_RUNTIME_ENABLED_FLAG.load(Ordering::Acquire);
        set_rive_runtime_enabled(next);
        next
    }
    #[cfg(not(feature = "rive-host-ffi"))]
    {
        false
    }
}

pub fn set_rive_payload(payload: Option<Vec<u8>>) {
    *RIVE_PAYLOAD.lock() = payload;
    RIVE_PREWARM_ATTEMPTS.store(0, Ordering::Release);
    if let Some(renderer) = OVERLAY_RENDERER.lock().as_mut() {
        renderer.rive = None;
    }

    if let Some(bytes) = RIVE_PAYLOAD.lock().as_ref() {
        ncommon::logN!(
            target: "overlay.rive",
            "set rive payload bytes={} (deferred import/prewarm)",
            bytes.len()
        );
    } else {
        ncommon::logN!(target: "overlay.rive", "cleared rive payload");
    }
}

#[cfg(feature = "rive-host-ffi")]
unsafe fn try_initialize_rive_overlay_state(
    device: *mut ngpu::NvnDevice,
    queue: *mut ngpu::NvnQueue,
) -> Result<RiveOverlayState, &'static str> {
    if device.is_null() {
        return Err("device_null");
    }
    if queue.is_null() {
        return Err("queue_null");
    }

    let payload = match RIVE_PAYLOAD.lock().as_ref() {
        Some(v) if !v.is_empty() => v.clone(),
        _ => return Err("missing_riv_payload"),
    };
    let runtime_build_stamp = rive_build_stamp_string(rive_api::rive_runtime_build_stamp());
    let renderer_build_stamp = rive_build_stamp_string(rive_api::rive_renderer_build_stamp());

    let init_t0 = Instant::now();
    ncommon::logN!(target: "overlay.rive", "rive init stage: build_stamp={}", RIVE_BUILD_STAMP);
    ncommon::logN!(
        target: "overlay.rive",
        "rive init stage: runtime_build_stamp={}",
        runtime_build_stamp
    );
    ncommon::logN!(
        target: "overlay.rive",
        "rive init stage: renderer_build_stamp={}",
        renderer_build_stamp
    );
    ncommon::logN!(
        target: "overlay.rive",
        "rive init begin payload_bytes={} device={:p} queue={:p}",
        payload.len(),
        device,
        queue
    );

    let custom_allocator_enabled = RIVE_USE_CUSTOM_ALLOCATOR;
    if custom_allocator_enabled {
        nmem::install_ngpu_rive_allocator();
    } else {
        nmem::clear_ngpu_rive_allocator();
    }

    if RIVE_ENABLE_NVN_LOG_CALLBACK {
        RIVE_NVN_LOG_LINES_EMITTED.store(0, Ordering::Release);
        rive_api::rive_set_log_callback(None);
        rive_api::rive_nvn_set_log_callback(Some(rive_nvn_debug_log_bridge));
        ncommon::logN!(target: "overlay.rive", "rive init stage: installed NVN log callback (runtime callback disabled)");
    } else {
        rive_api::rive_set_log_callback(None);
        rive_api::rive_nvn_set_log_callback(None);
        ncommon::logN!(
            target: "overlay.rive",
            "rive init stage: runtime+NVN log callbacks disabled (stability guard)"
        );
    }

    rive_api::rive_nvn_set_prefer_spirv_input(if RIVE_PREFER_SPIRV_INPUT { 1 } else { 0 });
    ncommon::logN!(
        target: "overlay.rive",
        "rive init stage: prefer_spirv_input={} runtime_uses_spirv={}",
        RIVE_PREFER_SPIRV_INPUT,
        rive_api::rive_nvn_uses_spirv_input() != 0
    );

    let mut bridge = ngpu::cpp::shim::NgpuRiveBridge::default();
    if ngpu::cpp::shim::ngpu_shim_get_rive_bridge(&mut bridge as *mut _) == 0 {
        ncommon::logN!(target: "overlay.rive", "rive init stage: ngpu bridge unavailable");
        return Err("ngpu_rive_bridge_unavailable");
    }
    if bridge.device.is_null() {
        ncommon::logN!(target: "overlay.rive", "rive init stage: ngpu bridge device is null");
        return Err("ngpu_rive_bridge_device_null");
    }
    if bridge.queue.is_null() {
        ncommon::logN!(target: "overlay.rive", "rive init stage: ngpu bridge queue is null");
        return Err("ngpu_rive_bridge_queue_null");
    }
    if bridge.get_proc_address.is_null() {
        ncommon::logN!(target: "overlay.rive", "rive init stage: ngpu bridge get_proc_address is null");
        return Err("ngpu_rive_bridge_get_proc_null");
    }

    let mut allocator_desc: Option<Box<rive_api::RiveNVNAllocator>> = None;
    if custom_allocator_enabled {
        let alloc = bridge.allocator.alloc.ok_or("ngpu_rive_bridge_alloc_fn_null")?;
        let realloc = bridge.allocator.realloc.ok_or("ngpu_rive_bridge_realloc_fn_null")?;
        let free = bridge.allocator.free.ok_or("ngpu_rive_bridge_free_fn_null")?;
        allocator_desc = Some(Box::new(rive_api::RiveNVNAllocator {
            alloc: Some(alloc),
            realloc: Some(realloc),
            free: Some(free),
            user: bridge.allocator.user,
        }));
    }

    let window_origin_mode =
        ngpu::device::device_get_window_origin_mode(device as *const ngpu::NvnDevice);
    let framebuffer_bottom_up = if window_origin_mode == NVN_WINDOW_ORIGIN_MODE_LOWER_LEFT {
        1
    } else {
        0
    };

    let context_desc = rive_api::RiveNVNContextDesc {
        device: bridge.device,
        queue: bridge.queue,
        get_proc_address: bridge.get_proc_address,
        max_texture_size: NVN_MAX_TEXTURE_SIZE_NX,
        clip_space_bottom_up: 1,
        framebuffer_bottom_up,
        allocator: allocator_desc
            .as_ref()
            .map_or(core::ptr::null(), |a| (&**a) as *const rive_api::RiveNVNAllocator),
    };

    unsafe {
        rive_api::rive_runtime_set_allocator(context_desc.allocator);
    }

    ncommon::logN!(
        target: "overlay.rive",
        "rive init stage: window_origin_mode={} framebuffer_bottom_up={} clip_space_bottom_up={}",
        match window_origin_mode {
            NVN_WINDOW_ORIGIN_MODE_LOWER_LEFT => "lower_left",
            NVN_WINDOW_ORIGIN_MODE_UPPER_LEFT => "upper_left",
            _ => "unknown",
        },
        framebuffer_bottom_up,
        context_desc.clip_space_bottom_up
    );

    ncommon::logN!(
        target: "overlay.rive",
        "rive init stage: nvn_initialize deferred to rive_nvn_render_context_new get_proc={:p}",
        context_desc.get_proc_address
    );

    ncommon::logN!(
        target: "overlay.rive",
        "rive init stage: render_context_new begin get_proc={:p} allocator_from_bridge={} alloc={:p} realloc={:p} free={:p}",
        context_desc.get_proc_address,
        bridge.has_allocator,
        bridge.allocator.alloc.map_or(core::ptr::null(), |f| f as *const c_void),
        bridge.allocator.realloc.map_or(core::ptr::null(), |f| f as *const c_void),
        bridge.allocator.free.map_or(core::ptr::null(), |f| f as *const c_void)
    );
    let context_t0 = Instant::now();
    let context = rive_api::RenderContext::new(&context_desc)
        .ok_or("rive_context_new_failed")?;
    ncommon::logN!(
        target: "overlay.rive",
        "rive init stage: render_context_new end ({}us)",
        context_t0.elapsed().as_micros()
    );

    ncommon::logN!(target: "overlay.rive", "rive init stage: renderer_new begin");
    let renderer_t0 = Instant::now();
    let renderer = rive_api::Renderer::new(&context).ok_or("rive_renderer_new_failed")?;
    ncommon::logN!(
        target: "overlay.rive",
        "rive init stage: renderer_new end ({}us)",
        renderer_t0.elapsed().as_micros()
    );

    ncommon::logN!(target: "overlay.rive", "rive init stage: file_import begin (no_context)");
    let import_t0 = Instant::now();
    let mut file = match rive_api::File::import(payload.as_slice()) {
        Ok(file) => {
            ncommon::logN!(
                target: "overlay.rive",
                "rive init stage: import source=no_context"
            );
            file
        }
        Err(result) => {
            ncommon::logN!(
                target: "overlay.rive",
                "rive init stage: import without context failed result={}({})",
                result,
                rive_import_result_name(result)
            );
            return Err("rive_import_failed");
        }
    };
    ncommon::logN!(
        target: "overlay.rive",
        "rive init stage: file_import end ({}us)",
        import_t0.elapsed().as_micros()
    );
    ncommon::logN!(target: "overlay.rive", "rive init stage: file_rebind_render_context begin");
    let rebind_t0 = Instant::now();
    file.rebind_render_context(context.as_ptr());
    ncommon::logN!(
        target: "overlay.rive",
        "rive init stage: file_rebind_render_context end ({}us)",
        rebind_t0.elapsed().as_micros()
    );

    let artboard_count = file.artboard_count();
    let artboard_name_limit = artboard_count.min(8);
    let mut artboard_index = 0usize;
    while artboard_index < artboard_name_limit {
        if let Some(name) = file.artboard_name(artboard_index) {
            ncommon::logN!(
                target: "overlay.rive",
                "rive init stage: artboard[{}]={}",
                artboard_index,
                name
            );
        }
        artboard_index += 1;
    }
    if artboard_count > artboard_name_limit {
        ncommon::logN!(
            target: "overlay.rive",
            "rive init stage: artboard list truncated shown={} total={}",
            artboard_name_limit,
            artboard_count
        );
    }

    let selected_artboard_name = RIVE_ARTBOARD_NAME.and_then(|name| {
        file.artboard_named(name)
            .map(|artboard| (artboard, format!("name={}", name)))
    });
    let selected_artboard_index = RIVE_ARTBOARD_INDEX.and_then(|index| {
        file.artboard_at(index)
            .map(|artboard| (artboard, format!("index={}", index)))
    });

    ncommon::logN!(target: "overlay.rive", "rive init stage: artboard_select begin");
    let artboard_t0 = Instant::now();
    let (artboard, selected_artboard_source) = if let Some(value) = selected_artboard_name {
        value
    } else if let Some(value) = selected_artboard_index {
        value
    } else {
        (
            file.artboard_default()
                .ok_or("rive_default_artboard_missing")?,
            String::from("default"),
        )
    };
    ncommon::logN!(
        target: "overlay.rive",
        "rive init stage: artboard_select end source={} ({}us)",
        selected_artboard_source,
        artboard_t0.elapsed().as_micros()
    );

    let state_machine_count = artboard.state_machine_count();
    let mut state_machine = artboard.state_machine_at(0);
    // In Rive, a state machine scene and a linear animation scene are
    // alternative top-level controllers. Advancing both against the same
    // artboard can override properties back and forth each frame.
    let animation = if state_machine.is_some() {
        None
    } else {
        artboard.animation_at(0)
    };
    let default_view_model_bound = if let Some(machine) = state_machine.as_mut() {
        let bound = machine.bind_default_view_model(&file, &artboard);
        ncommon::logN!(
            target: "overlay.rive",
            "rive init stage: default_view_model_bind bound={}",
            bound
        );
        bound
    } else {
        false
    };
    let artboard_w = artboard.width();
    let artboard_h = artboard.height();

    ncommon::logN!(
        target: "overlay.rive",
        "rive init complete total={}us custom_allocator={} bridge_allocator={} artboard={}x{} anim0={} sm_count={} sm0={} default_vm_bound={} scene={}",
        init_t0.elapsed().as_micros(),
        custom_allocator_enabled,
        bridge.has_allocator,
        artboard_w,
        artboard_h,
        animation.is_some(),
        state_machine_count,
        state_machine.is_some(),
        default_view_model_bound,
        if state_machine.is_some() { "state_machine" } else if animation.is_some() { "animation" } else { "artboard" }
    );

    Ok(RiveOverlayState {
        _file: file,
        artboard,
        animation,
        state_machine,
        context,
        renderer,
        _allocator_desc: allocator_desc,
        target: None,
        last_texture_ptr: 0,
        target_width: 0,
        target_height: 0,
        frame_number: 0,
        bootstrap_trigger_armed: false,
    })
}

#[cfg(feature = "rive-host-ffi")]
unsafe fn maybe_prewarm_rive_overlay_state(queue: *mut ngpu::NvnQueue) {
    if queue.is_null() {
        return;
    }
    if !RIVE_PREWARM_WHILE_DISABLED {
        return;
    }
    if !ensure_overlay_renderer_initialized() {
        return;
    }

    let mut guard = OVERLAY_RENDERER.lock();
    let renderer = match guard.as_mut() {
        Some(v) => v,
        None => return,
    };
    if renderer.rive.is_some() {
        return;
    }

    let attempt = RIVE_PREWARM_ATTEMPTS.fetch_add(1, Ordering::AcqRel) + 1;
    if attempt > RIVE_PREWARM_MAX_ATTEMPTS {
        return;
    }

    let device = ngpu::bootstrap::cached_device().unwrap_or(core::ptr::null_mut());
    let t0 = Instant::now();
    match try_initialize_rive_overlay_state(device, queue) {
        Ok(state) => {
            ncommon::logN!(
                target: "overlay.rive",
                "rive prewarm success attempt={} elapsed={}us",
                attempt,
                t0.elapsed().as_micros()
            );
            renderer.rive = Some(state);
        }
        Err(reason) => {
            if WARNED_RIVE_PREWARM_FAIL.fetch_add(1, Ordering::AcqRel) < 6 {
                ncommon::logN!(
                    target: "overlay.rive",
                    "rive prewarm failed attempt={} reason={} elapsed={}us",
                    attempt,
                    reason,
                    t0.elapsed().as_micros()
                );
            }
        }
    }
}

#[cfg(feature = "rive-host-ffi")]
unsafe fn try_draw_rive_overlay(
    overlay: &mut RiveOverlayState,
    cmdbuf: *mut ngpu::NvnCommandBuffer,
    texture: *mut ngpu::NvnTexture,
    safe_frame_number: u64,
) -> Result<u64, &'static str> {
    let width = ngpu::resource::texture_get_width(texture as *const ngpu::NvnTexture);
    let height = ngpu::resource::texture_get_height(texture as *const ngpu::NvnTexture);
    let format = ngpu::resource::texture_get_format(texture as *const ngpu::NvnTexture);
    let samples = ngpu::resource::texture_get_samples(texture as *const ngpu::NvnTexture);
    if width <= 0 || height <= 0 {
        return Err("invalid_render_target_size");
    }
    let width = width as u32;
    let height = height as u32;
    let first_frame = overlay.frame_number == 0;
    let target_sample_count = if samples > 0 { samples as u32 } else { 1 };
    let rive_msaa_sample_count = if target_sample_count > 1 {
        target_sample_count
    } else {
        0
    };

    let texture_changed = overlay.last_texture_ptr != texture as usize;
    let size_changed = overlay.target_width != width || overlay.target_height != height;
    if texture_changed || size_changed || overlay.target.is_none() {
        if first_frame {
            ncommon::logN!(
                target: "overlay.rive",
                "rive draw stage: render_target_new begin tex={:p} size={}x{} fmt={} samples={}",
                texture,
                width,
                height,
                format,
                samples
            );
        }
        let target_t0 = Instant::now();
        overlay.target = rive_api::RenderTarget::new(
            width,
            height,
            texture.cast(),
            core::ptr::null_mut(),
            target_sample_count,
        );
        if overlay.target.is_none() {
            return Err("rive_render_target_new_failed");
        }
        if first_frame {
            ncommon::logN!(
                target: "overlay.rive",
                "rive draw stage: render_target_new end ({}us)",
                target_t0.elapsed().as_micros()
            );
        }
        overlay.last_texture_ptr = texture as usize;
        overlay.target_width = width;
        overlay.target_height = height;
    }

    if RIVE_DEBUG_FORCE_CLEAR_FIRST_FRAME && first_frame {
        let region = RawNvnCopyRegion {
            xoffset: 0,
            yoffset: 0,
            zoffset: 0,
            width: width as i32,
            height: height as i32,
            depth: 1,
        };
        let color = RIVE_DEBUG_FORCE_CLEAR_COLOR;
        ngpu::cmdbuf::command_buffer_clear_texture(
            cmdbuf,
            texture as *const ngpu::NvnTexture,
            core::ptr::null(),
            (&region as *const RawNvnCopyRegion).cast::<ngpu::NvnCopyRegion>(),
            color.as_ptr(),
            NVN_CLEAR_COLOR_MASK_RGBA,
        );
        ncommon::logN!(
            target: "overlay.rive",
            "rive draw debug: forced clear on first frame color=rgba({:.2},{:.2},{:.2},{:.2}) size={}x{} fmt={} samples={}",
            color[0],
            color[1],
            color[2],
            color[3],
            width,
            height,
            format,
            samples
        );
    }

    if !overlay.bootstrap_trigger_armed {
        overlay.bootstrap_trigger_armed = true;
        if let Some(machine) = overlay.state_machine.as_mut() {
            const RIVE_STATE_MACHINE_TRIGGER_CORE_TYPE: u16 = 58;
            let input_count = machine.input_count();
            let mut fired = 0u32;
            for index in 0..input_count {
                if machine.input_core_type(index) == RIVE_STATE_MACHINE_TRIGGER_CORE_TYPE
                    && machine.fire_trigger_at(index)
                {
                    fired = fired.saturating_add(1);
                }
            }
            if first_frame || fired != 0 {
                ncommon::logN!(
                    target: "overlay.rive",
                    "rive draw stage: bootstrap trigger sweep fired={} inputs={}"
                    ,
                    fired,
                    input_count
                );
            }
        }
    }
    let dt = 1.0f32 / 60.0f32;
    if RIVE_ENABLE_ADVANCE {
        if first_frame {
            ncommon::logN!(target: "overlay.rive", "rive draw stage: advance begin");
        }
        if let Some(anim) = overlay.animation.as_mut() {
            let _ = anim.advance_and_apply(dt, 1.0);
        }
        if let Some(machine) = overlay.state_machine.as_mut() {
            let _ = machine.advance_and_apply(dt);
            if first_frame {
                ncommon::logN!(target: "overlay.rive", "rive draw stage: advance mode=state_machine_advance_and_apply");
            }
        } else {
            let _ = overlay.artboard.advance(dt);
            if first_frame {
                ncommon::logN!(target: "overlay.rive", "rive draw stage: advance mode=artboard");
            }
        }
        if first_frame {
            ncommon::logN!(target: "overlay.rive", "rive draw stage: advance end");
        }
    } else if first_frame {
        ncommon::logN!(target: "overlay.rive", "rive draw stage: advance skipped (bring-up safety)");
    }

    let frame_desc = rive_api::RiveFrameDescriptor {
        render_target_width: width,
        render_target_height: height,
        load_action: rive_api::RiveLoadAction::PreserveRenderTarget,
        clear_color: 0,
        msaa_sample_count: rive_msaa_sample_count,
        disable_raster_ordering: 0,
        wireframe: 0,
        fills_disabled: 0,
        strokes_disabled: 0,
        clockwise_fill_override: 0,
    };

    let cmd_used_before = ngpu::cmdbuf::command_buffer_get_command_memory_used(cmdbuf as *const ngpu::NvnCommandBuffer);
    let ctl_used_before = ngpu::cmdbuf::command_buffer_get_control_memory_used(cmdbuf as *const ngpu::NvnCommandBuffer);
    if first_frame {
        ncommon::logN!(
            target: "overlay.rive",
            "rive draw stage: cmdbuf before begin_frame cmd_used=0x{:x} ctl_used=0x{:x}",
            cmd_used_before,
            ctl_used_before
        );
        ncommon::logN!(target: "overlay.rive", "rive draw stage: begin_frame begin");
    }
    overlay.context.begin_frame(&frame_desc);
    if first_frame {
        ncommon::logN!(target: "overlay.rive", "rive draw stage: begin_frame end");
    }

    overlay.renderer.save();
    if first_frame {
        let bounds = overlay.artboard.bounds();
        let object_count = overlay.artboard.object_count();
        let drawable_count = overlay.artboard.drawable_count();
        let will_draw_count = overlay.artboard.will_draw_count();
        let hidden_drawable_count = overlay.artboard.hidden_drawable_count();
        ncommon::logN!(
            target: "overlay.rive",
            "rive draw stage: content bounds={:?}",
            bounds.map(|b| (b.min_x, b.min_y, b.max_x, b.max_y))
        );
        ncommon::logN!(
            target: "overlay.rive",
            "rive draw stage: artboard stats objects={} drawables={} will_draw={} hidden={}",
            object_count,
            drawable_count,
            will_draw_count,
            hidden_drawable_count
        );
    }
    let content_bounds = overlay.artboard.bounds().unwrap_or(rive_api::RiveAabb {
        min_x: 0.0,
        min_y: 0.0,
        max_x: overlay.artboard.width().max(1.0),
        max_y: overlay.artboard.height().max(1.0),
    });
    overlay.renderer.align(
        rive_api::RiveFit::None,
        rive_api::RiveAlignment { x: 1.0, y: 1.0 },
        rive_api::RiveAabb {
            min_x: 0.0,
            min_y: 0.0,
            max_x: width as f32,
            max_y: height as f32,
        },
        content_bounds,
        1.0,
    );
    if first_frame {
        ncommon::logN!(
            target: "overlay.rive",
            "rive draw stage: align fit=none alignment=bottom_right content=({:.1},{:.1},{:.1},{:.1}) frame={}x{}",
            content_bounds.min_x,
            content_bounds.min_y,
            content_bounds.max_x,
            content_bounds.max_y,
            width,
            height
        );
    }
    overlay.renderer.reset_debug_stats();
    if first_frame {
        ncommon::logN!(target: "overlay.rive", "rive draw stage: draw_artboard begin");
    }
    overlay.renderer.draw_artboard(&mut overlay.artboard);
    overlay.renderer.restore();
    if first_frame {
        ncommon::logN!(target: "overlay.rive", "rive draw stage: draw_artboard end");
        if let Some(stats) = overlay.renderer.debug_stats() {
            ncommon::logN!(
                target: "overlay.rive",
                "rive draw stage: renderer reject stats null={} outside={} clip_fail={} clip_empty={} push_fail={} push_ok={} exhausted={}",
                stats.null_draw,
                stats.outside_frame,
                stats.apply_clip_failure,
                stats.apply_clip_empty,
                stats.push_draw_failure,
                stats.push_draw_success,
                stats.exhausted_retries
            );
        }
    }

    let target_ref = match overlay.target.as_ref() {
        Some(v) => v,
        None => return Err("rive_target_missing_after_creation"),
    };
    let current_frame = overlay.frame_number.saturating_add(1);
    let safe_frame = safe_frame_number.min(current_frame);
    let flush = rive_api::RiveFlushResources {
        render_target: target_ref.as_ptr(),
        external_command_buffer: cmdbuf.cast(),
        current_frame_number: current_frame,
        safe_frame_number: safe_frame,
    };

    if first_frame {
        ncommon::logN!(
            target: "overlay.rive",
            "rive draw stage: flush begin current_frame={} safe_frame={}",
            current_frame,
            safe_frame
        );
    }
    overlay.context.flush(&flush);
    let cmd_used_after = ngpu::cmdbuf::command_buffer_get_command_memory_used(cmdbuf as *const ngpu::NvnCommandBuffer);
    let ctl_used_after = ngpu::cmdbuf::command_buffer_get_control_memory_used(cmdbuf as *const ngpu::NvnCommandBuffer);
    if first_frame {
        ncommon::logN!(target: "overlay.rive", "rive draw stage: flush end");
        ncommon::logN!(
            target: "overlay.rive",
            "rive draw stage: cmdbuf after flush cmd_used=0x{:x} ctl_used=0x{:x} delta_cmd=0x{:x} delta_ctl=0x{:x}",
            cmd_used_after,
            ctl_used_after,
            cmd_used_after.saturating_sub(cmd_used_before),
            ctl_used_after.saturating_sub(ctl_used_before)
        );
        log_rive_allocator_snapshot("after-first-flush");
    }

    overlay.frame_number = current_frame;
    Ok(current_frame)
}

#[cfg(feature = "rive-host-ffi")]
unsafe fn rive_submission_still_in_flight(renderer: &mut OverlayRenderer) -> bool {
    let mut in_flight = false;
    for (slot_index, slot) in renderer.slots.iter_mut().enumerate() {
        if !slot.submitted_once || slot.last_rive_frame == 0 {
            continue;
        }

        let wait_result = ngpu::sync::sync_wait(slot.sync_ptr(), 0);
        if wait_result == NVN_SYNC_WAIT_ALREADY_SIGNALED
            || wait_result == NVN_SYNC_WAIT_CONDITION_SATISFIED
        {
            if slot.last_rive_frame > renderer.completed_rive_frame {
                renderer.completed_rive_frame = slot.last_rive_frame;
            }
            slot.last_rive_frame = 0;
            continue;
        }

        in_flight = true;
        if wait_result != NVN_SYNC_WAIT_TIMEOUT_EXPIRED
            && wait_result != NVN_SYNC_WAIT_FAILED
            && OVERLAY_SLOT_WAIT_LOG_COUNT.fetch_add(1, Ordering::AcqRel) < 6
        {
            ncommon::logN!(
                target: "overlay.rive",
                "overlay slot sync_wait returned unexpected result={} slot={}",
                wait_result,
                slot_index
            );
        } else if RIVE_INFLIGHT_GATE_LOG_COUNT.fetch_add(1, Ordering::AcqRel) < 6 {
            ncommon::logN!(
                target: "overlay.rive",
                "rive submission gate waiting slot={} wait_result={}",
                slot_index,
                wait_result
            );
        }
    }
    in_flight
}

unsafe fn try_acquire_overlay_slot(renderer: &mut OverlayRenderer) -> Option<usize> {
    let slot_count = renderer.slots.len();
    if slot_count == 0 {
        return None;
    }

    let mut attempt = 0usize;
    while attempt < slot_count {
        let slot_index = (renderer.frame_slot + attempt) % slot_count;
        let slot = &mut renderer.slots[slot_index];
        if !slot.submitted_once {
            renderer.frame_slot = slot_index.wrapping_add(1);
            return Some(slot_index);
        }

        let wait_result = ngpu::sync::sync_wait(slot.sync_ptr(), 0);
        if wait_result == NVN_SYNC_WAIT_ALREADY_SIGNALED
            || wait_result == NVN_SYNC_WAIT_CONDITION_SATISFIED
        {
            if slot.last_rive_frame > renderer.completed_rive_frame {
                renderer.completed_rive_frame = slot.last_rive_frame;
            }
            slot.last_rive_frame = 0;
            renderer.frame_slot = slot_index.wrapping_add(1);
            return Some(slot_index);
        }

        if wait_result != NVN_SYNC_WAIT_TIMEOUT_EXPIRED
            && wait_result != NVN_SYNC_WAIT_FAILED
            && OVERLAY_SLOT_WAIT_LOG_COUNT.fetch_add(1, Ordering::AcqRel) < 6
        {
            ncommon::logN!(
                target: "overlay.rive",
                "overlay slot sync_wait returned unexpected result={} slot={}",
                wait_result,
                slot_index
            );
        }

        attempt += 1;
    }

    ncommon::logN!(
        target: "overlay.rive",
        "overlay slot reuse blocked; no fenced slot is ready across ring_len={}",
        slot_count
    );
    None
}

unsafe fn ensure_overlay_renderer_initialized() -> bool {
    let mut guard = OVERLAY_RENDERER.lock();
    if guard.is_none() {
        let device = match ngpu::bootstrap::cached_device() {
            Some(d) => d,
            None => return false,
        };
        let ring_len = {
            let mut n = 3usize;
            if let Some(window) = ngpu::bootstrap::cached_window() {
                let active = ngpu::window::window_get_num_active_textures(
                    window as *const ngpu::NvnWindow,
                );
                if (2..=3).contains(&active) {
                    n = active as usize;
                }
            }
            n.max(OVERLAY_RING_MIN_LEN)
        };
        let (command_alignment, control_alignment) =
            query_command_buffer_alignments(device);
        let cmdbuf_command_bytes = align_up(OVERLAY_CMDBUF_COMMAND_BYTES, command_alignment);
        let cmdbuf_control_bytes = align_up(OVERLAY_CMDBUF_CONTROL_BYTES, control_alignment);
        let command_pool_size = ring_len * cmdbuf_command_bytes;
        let control_pool_size = ring_len * cmdbuf_control_bytes;
        let mut arena = match nmem::CommandBufferArena::new(
            device,
            command_pool_size,
            control_pool_size,
            POOL_FLAGS_CPU_UNCACHED_GPU_UNCACHED,
            0x1000, // pool storage alignment
            command_alignment,
            control_alignment,
            Some(b"nsuite_overlay_pool\0"),
        ) {
            Ok(v) => v,
            Err(_) => return false,
        };

        let mut slots = Vec::with_capacity(ring_len);
        let mut i = 0usize;
        while i < ring_len {
            let cmdbuf_label = match i {
                0 => Some(b"nsuite_overlay_cmdbuf_0\0".as_slice()),
                1 => Some(b"nsuite_overlay_cmdbuf_1\0".as_slice()),
                _ => Some(b"nsuite_overlay_cmdbuf_2\0".as_slice()),
            };
            let cmdbuf = match nmem::OwnedCommandBuffer::new(
                &mut arena,
                device,
                cmdbuf_command_bytes,
                cmdbuf_control_bytes,
                cmdbuf_label,
            ) {
                Ok(v) => v,
                Err(_) => return false,
            };

            let mut sync = Box::new(core::mem::zeroed::<RawNvnSync>());
            if ngpu::sync::sync_initialize((&mut *sync as *mut RawNvnSync).cast(), device) == 0 {
                ncommon::logN!(
                    target: "overlay.rive",
                    "overlay sync init failed slot={}",
                    i
                );
                return false;
            }

            let sync_label = match i {
                0 => Some(b"nsuite_overlay_sync_0\0".as_slice()),
                1 => Some(b"nsuite_overlay_sync_1\0".as_slice()),
                _ => Some(b"nsuite_overlay_sync_2\0".as_slice()),
            };
            if let Some(label) = sync_label {
                ngpu::sync::sync_set_debug_label(
                    (&mut *sync as *mut RawNvnSync).cast(),
                    label.as_ptr(),
                );
            }

            slots.push(OverlayCommandSlot {
                cmdbuf,
                sync,
                submitted_once: false,
                last_rive_frame: 0,
            });
            i += 1;
        }

        *guard = Some(OverlayRenderer {
            _arena: arena,
            slots,
            frame_slot: 0,
            tick: 0,
            completed_rive_frame: 0,
            #[cfg(feature = "rive-host-ffi")]
            rive: None,
        });
        if OVERLAY_TRACE {
            ncommon::logN!(
                target: "overlay.rive",
                "overlay ring initialized ring_len={} cmd_pool=0x{:x} ctl_pool=0x{:x}",
                ring_len,
                command_pool_size,
                control_pool_size
            );
        }
        ncommon::logN!(target: "overlay.rive", "initialized overlay renderer");
    }
    true
}

fn default_overlay_handle_provider(queue: *mut ngpu::NvnQueue) -> ngpu::NvnCommandHandle {
    let texture = match ngpu::bootstrap::cached_active_window_texture() {
        Some(p) => p,
        None => return 0,
    };
    unsafe { build_overlay_handle_for_texture(queue, texture) }
}

fn present_overlay_handle_provider(
    queue: *mut ngpu::NvnQueue,
    _window: *mut ngpu::NvnWindow,
    index: i32,
) -> ngpu::NvnCommandHandle {
    let texture = match ngpu::bootstrap::cached_window_texture(index)
        .or_else(ngpu::bootstrap::cached_active_window_texture)
    {
        Some(p) => p,
        None => return 0,
    };

    // Some games present more than once for the same acquired index; avoid
    // redrawing the full Rive graph redundantly in that case.
    if index >= 0 {
        let mut textures = [core::ptr::null_mut(); 8];
        let texture_count = ngpu::bootstrap::cached_window_textures_snapshot(&mut textures);
        if texture_count >= 2 {
            let queue_addr = queue as usize;
            let prev_queue = LAST_PRESENT_SUBMIT_QUEUE.load(Ordering::Acquire);
            let prev_index = LAST_PRESENT_SUBMIT_INDEX.load(Ordering::Acquire);
            if prev_queue == queue_addr && prev_index == index {
                let gate_logs = PRESENT_INDEX_GATE_LOG_COUNT.fetch_add(1, Ordering::AcqRel);
                if gate_logs < 6 {
                    ncommon::logN!(
                        target: "overlay.rive",
                        "present-index gate skipped duplicate queue={:p} index={}",
                        queue,
                        index
                    );
                }
                return 0;
            }
            LAST_PRESENT_SUBMIT_QUEUE.store(queue_addr, Ordering::Release);
            LAST_PRESENT_SUBMIT_INDEX.store(index, Ordering::Release);
        }
    }

    unsafe { build_overlay_handle_for_texture(queue, texture) }
}

unsafe fn build_overlay_handle_for_texture(
    queue: *mut ngpu::NvnQueue,
    texture: *mut ngpu::NvnTexture,
) -> ngpu::NvnCommandHandle {
    unsafe {
        let debug_enabled = ngpu::debug::enabled();
        #[cfg(feature = "rive-host-ffi")]
        let rive_enabled = rive_runtime_enabled();
        #[cfg(not(feature = "rive-host-ffi"))]
        let rive_enabled = false;

        if !(debug_enabled || rive_enabled) {
            return 0;
        }

        #[cfg(feature = "rive-host-ffi")]
        {
            if rive_enabled && (RIVE_PREWARM_WHILE_DISABLED || debug_enabled) {
                maybe_prewarm_rive_overlay_state(queue);
            }
        }

        let lines = lines_snapshot();
        if OVERLAY_TRACE && lines.is_empty() && WARNED_NO_LINES.fetch_add(1, Ordering::AcqRel) == 0 {
            ncommon::logN!(
                target: "overlay.rive",
                "append called with empty draw list; issuing clear-only probe"
            );
        }
        if !lines.is_empty() {
            clear_draw_list();
        }

        if texture.is_null() {
            if OVERLAY_TRACE && WARNED_NO_TEXTURE.fetch_add(1, Ordering::AcqRel) == 0 {
                ncommon::logN!(
                    target: "overlay.rive",
                    "overlay build called but target texture pointer is null"
                );
            }
            return 0;
        }

        if !ensure_overlay_renderer_initialized() {
            return 0;
        }
        let mut guard = OVERLAY_RENDERER.lock();
        let renderer = match guard.as_mut() {
            Some(r) => r,
            None => return 0,
        };

        renderer.tick = renderer.tick.wrapping_add(1);

        #[cfg(feature = "rive-host-ffi")]
        let rive_blocked_on_inflight = if rive_enabled {
            let blocked = rive_submission_still_in_flight(renderer);
            if blocked && !debug_enabled {
                return 0;
            }
            blocked
        } else {
            false
        };

        let slot_index = match try_acquire_overlay_slot(renderer) {
            Some(slot) => slot,
            None => return 0,
        };
        {
            let slot = &renderer.slots[slot_index];
            slot.cmdbuf.rebind_recording_memory(&renderer._arena);
            slot.cmdbuf.begin_recording();
        }
        let cmdbuf = renderer.slots[slot_index].cmdbuf.as_raw_ptr();

        let mut rive_drawn = false;
        let mut current_rive_frame = 0u64;
        #[cfg(feature = "rive-host-ffi")]
        {
            if rive_enabled && !rive_blocked_on_inflight && renderer.rive.is_none() {
                match try_initialize_rive_overlay_state(
                    ngpu::bootstrap::cached_device().unwrap_or(core::ptr::null_mut()),
                    queue,
                ) {
                    Ok(state) => {
                        ncommon::logN!(
                            target: "overlay.rive",
                            "initialized rive overlay state (artboard+renderer+context)"
                        );
                        renderer.rive = Some(state);
                    }
                    Err(reason) => {
                        if WARNED_RIVE_INIT_FAIL.fetch_add(1, Ordering::AcqRel) == 0 {
                            ncommon::logN!(
                                target: "overlay.rive",
                                "rive overlay init skipped/fail reason={} (fallback debug clear path active)",
                                reason
                            );
                        }
                    }
                }
            }
            let mut drop_rive_state = false;
            if rive_enabled {
                if !rive_blocked_on_inflight {
                    if let Some(rive_state) = renderer.rive.as_mut() {
                        match try_draw_rive_overlay(
                            rive_state,
                            cmdbuf,
                            texture,
                            renderer.completed_rive_frame,
                        ) {
                            Ok(frame_number) => {
                                rive_drawn = true;
                                current_rive_frame = frame_number;
                            }
                            Err(reason) => {
                                if WARNED_RIVE_DRAW_FAIL.fetch_add(1, Ordering::AcqRel) < 8 {
                                    ncommon::logN!(
                                        target: "overlay.rive",
                                        "rive overlay draw failed reason={} (using debug fallback)",
                                        reason
                                    );
                                }
                                if RIVE_DROP_STATE_ON_DRAW_FAIL {
                                    drop_rive_state = true;
                                }
                            }
                        }
                    }
                }
            } else {
                if renderer.rive.is_some() {
                    renderer.rive = None;
                    ncommon::logN!(
                        target: "overlay.rive",
                        "rive runtime disabled; dropped live rive state"
                    );
                } else if WARNED_RIVE_INIT_FAIL.fetch_add(1, Ordering::AcqRel) == 0 {
                    ncommon::logN!(
                        target: "overlay.rive",
                        "rive runtime disabled; using legacy debug clear overlay only"
                    );
                }
            }
            if drop_rive_state {
                renderer.rive = None;
                ncommon::logN!(
                    target: "overlay.rive",
                    "rive overlay state dropped after draw failure; next frame will reinit"
                );
                log_rive_allocator_snapshot("after-draw-failure");
            }
        }
        let mut emitted_any = rive_drawn;
        if debug_enabled {
            // Fallback path for the legacy debug overlay: mutate the acquired swap texture directly.
            let pulse = ((renderer.tick >> 2) & 0x3F) as f32 / 63.0;
            let gray = pulse * 0.35;
            let clear_width =
                ngpu::resource::texture_get_width(texture as *const ngpu::NvnTexture).max(1);
            let clear_height =
                ngpu::resource::texture_get_height(texture as *const ngpu::NvnTexture).max(1);
            let fullscreen = RawNvnCopyRegion {
                xoffset: 0,
                yoffset: 0,
                zoffset: 0,
                width: clear_width,
                height: clear_height,
                depth: 1,
            };
            let clear = [gray, gray, gray, 1.0];
            ngpu::cmdbuf::command_buffer_clear_texture(
                cmdbuf,
                texture as *const ngpu::NvnTexture,
                core::ptr::null(),
                (&fullscreen as *const RawNvnCopyRegion).cast::<ngpu::NvnCopyRegion>(),
                clear.as_ptr(),
                NVN_CLEAR_COLOR_MASK_RGBA,
            );
            emitted_any = true;
        }
        if debug_enabled {
            if !lines.is_empty() {
                emitted_any = true;
            }
            draw_debug_lines_to_texture(cmdbuf, texture, &lines);
        }

        if !emitted_any {
            return 0;
        }

        let handle = {
            let slot = &mut renderer.slots[slot_index];
            ngpu::cmdbuf::command_buffer_fence_sync(
                slot.cmdbuf.as_raw_ptr(),
                slot.sync_mut_ptr(),
                NVN_SYNC_CONDITION_ALL_GPU_COMMANDS_COMPLETE,
                0,
            );
            let handle = slot.cmdbuf.end_recording();
            slot.submitted_once = true;
            slot.last_rive_frame = if rive_drawn { current_rive_frame } else { 0 };
            handle
        };
        if OVERLAY_TRACE && handle != 0 {
            let n = HANDLE_NONZERO_COUNT.fetch_add(1, Ordering::AcqRel) + 1;
            if n <= 3 {
                ncommon::logN!(
                    target: "overlay.rive",
                    "overlay command handle ready=0x{:x} active_texture={:p}",
                    handle,
                    texture
                );
            }
        }
        handle
    }
}

#[inline(always)]
fn to_screen_xy(line: &DebugLine, t: f32) -> (f32, f32, [f32; 4]) {
    let x = line.x0 + (line.x1 - line.x0) * t;
    let y = line.y0 + (line.y1 - line.y0) * t;
    let max_abs = x.abs().max(y.abs());
    let (sx, sy) = if max_abs <= 2.5 {
        ((x * 0.5 + 0.5) * 1920.0, (1.0 - (y * 0.5 + 0.5)) * 1080.0)
    } else {
        (960.0 + x, 540.0 - y)
    };
    (
        sx,
        sy,
        [line.color.r, line.color.g, line.color.b, line.color.a.max(0.05)],
    )
}

unsafe fn draw_debug_lines_to_texture(
    cmdbuf: *mut ngpu::NvnCommandBuffer,
    texture: *mut ngpu::NvnTexture,
    lines: &[DebugLine],
) {
    if lines.is_empty() {
        return;
    }

    const MAX_LINES: usize = 64;
    const MAX_STEPS: i32 = 256;
    const STAMP: i32 = 2;
    let count = lines.len().min(MAX_LINES);

    for line in &lines[..count] {
        let dx = (line.x1 - line.x0).abs();
        let dy = (line.y1 - line.y0).abs();
        let mut steps = dx.max(dy) as i32;
        if steps < 1 {
            steps = 1;
        }
        if steps > MAX_STEPS {
            steps = MAX_STEPS;
        }

        let mut i = 0;
        while i <= steps {
            let t = i as f32 / steps as f32;
            let (sx, sy, color) = to_screen_xy(line, t);
            let xi = sx as i32;
            let yi = sy as i32;
            if (0..1920).contains(&xi) && (0..1080).contains(&yi) {
                let w = (1920 - xi).min(STAMP);
                let h = (1080 - yi).min(STAMP);
                let region = RawNvnCopyRegion {
                    xoffset: xi,
                    yoffset: yi,
                    zoffset: 0,
                    width: w,
                    height: h,
                    depth: 1,
                };
                ngpu::cmdbuf::command_buffer_clear_texture(
                    cmdbuf,
                    texture as *const ngpu::NvnTexture,
                    core::ptr::null(),
                    (&region as *const RawNvnCopyRegion).cast::<ngpu::NvnCopyRegion>(),
                    color.as_ptr(),
                    NVN_CLEAR_COLOR_MASK_RGBA,
                );
            }
            i += 1;
        }
    }
}

fn queue_submit_append_provider(
    queue: *mut ngpu::NvnQueue,
    caller_off: usize,
    count: i32,
    _handles: *const ngpu::NvnCommandHandle,
) -> ngpu::NvnCommandHandle {
    let target_off = TARGET_CALLER_TEXT_OFF.load(Ordering::Acquire);
    let target_queue = TARGET_QUEUE_PTR.load(Ordering::Acquire);

    let mut effective_queue = target_queue;
    if effective_queue == 0 {
        if let Some(present_queue) = ngpu::bootstrap::cached_present_queue() {
            effective_queue = present_queue as usize;
        }
    }
    if effective_queue == 0 {
        let discovered = AUTO_TARGET_QUEUE_PTR.load(Ordering::Acquire);
        if discovered != 0 {
            effective_queue = discovered;
        } else if count >= 8 {
            let queue_addr = queue as usize;
            let _ = AUTO_TARGET_QUEUE_PTR.compare_exchange(
                0,
                queue_addr,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
            effective_queue = AUTO_TARGET_QUEUE_PTR.load(Ordering::Acquire);
            if effective_queue == queue_addr {
                ncommon::logN!(
                    target: "overlay.rive",
                    "auto-selected overlay queue={:p} cmds={} caller_off=0x{:x}",
                    queue,
                    count,
                    caller_off
                );
            }
        }
    }

    // Some builds currently report caller_off=0 for the present-bound submit. Once the queue is
    // known, do not let that stale caller sample block the legacy overlay path.
    let pass_caller = target_off == 0 || caller_off == target_off || (caller_off == 0 && effective_queue != 0);
    let pass_queue = effective_queue == 0 || effective_queue == queue as usize;
    if !(pass_caller && pass_queue) {
        return 0;
    }

    // Isolate to one overlay append per acquired swap-texture index transition.
    if let Some(active_index) = ngpu::bootstrap::cached_active_window_texture_index() {
        let prev = LAST_EMITTED_ACQUIRE_INDEX.swap(active_index, Ordering::AcqRel);
        if prev == active_index {
            return 0;
        }
        let gate_logs = ACQUIRE_INDEX_GATE_LOG_COUNT.fetch_add(1, Ordering::AcqRel);
        if OVERLAY_TRACE && gate_logs < 6 {
            ncommon::logN!(
                target: "overlay.rive",
                "acquire-index gate accepted index={} caller_off=0x{:x}",
                active_index,
                caller_off
            );
        }
    }

    let pass_num = APPEND_PASS_COUNT.fetch_add(1, Ordering::AcqRel) + 1;
    if OVERLAY_TRACE && pass_num <= 3 {
        ncommon::logN!(
            target: "overlay.rive",
            "append provider matched submit caller_off=0x{:x} queue={:p} pass={}",
            caller_off,
            queue,
            pass_num
        );
    }

    match *OVERLAY_HANDLE_PROVIDER.lock() {
        Some(f) => f(queue),
        None => 0,
    }
}

/// Example install path for "append one overlay command handle to a specific queue submit callsite".
///
/// Usage:
/// 1) Call `set_submit_filter(caller_text_off, Some(queue_ptr))` after discovering target values.
/// 2) Call `set_overlay_handle_provider(...)` to return a valid ended command handle each frame.
/// 3) Call `install_queue_submit_overlay_hook()`.
pub unsafe fn install_queue_submit_overlay_hook() -> bool {
    {
        let mut provider = OVERLAY_HANDLE_PROVIDER.lock();
        if provider.is_none() {
            *provider = Some(default_overlay_handle_provider);
        }
    }
    if OVERLAY_RENDER_VIA_PRESENT_HOOK {
        ngpu::bootstrap::set_queue_submit_append_provider(None);
        ngpu::bootstrap::set_queue_present_submit_provider(Some(present_overlay_handle_provider));
        ncommon::logN!(
            target: "overlay.rive",
            "registered overlay provider into bootstrap queue present hook"
        );
    } else {
        ngpu::bootstrap::set_queue_present_submit_provider(None);
        ngpu::bootstrap::set_queue_submit_append_provider(Some(queue_submit_append_provider));
        ncommon::logN!(
            target: "overlay.rive",
            "registered overlay append provider into bootstrap queue submit hook"
        );
    }
    true
}



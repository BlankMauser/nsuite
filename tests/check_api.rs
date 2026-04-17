use nsuite::ngpu;
use ngpu::consts::*;

const NVN_DEVICE_INFO_API_MAJOR_VERSION: ngpu::NvnDeviceInfo = 0;
const NVN_DEVICE_INFO_API_MINOR_VERSION: ngpu::NvnDeviceInfo = 1;
const NVN_DEVICE_INFO_SUPPORTS_DRAW_TEXTURE: ngpu::NvnDeviceInfo = 71;
const HEADER_API_MAJOR_VERSION: i32 = 55;
const HEADER_API_MINOR_VERSION: i32 = 13;

#[repr(C)]
pub struct ApiVersionCheck {
    pub header_major: i32,
    pub header_minor: i32,
    pub driver_major: i32,
    pub driver_minor: i32,
    pub compatible: ngpu::NvnBoolean,
}

#[repr(C)]
pub struct DrawTextureSupportCheck {
    pub slot_mapped: ngpu::NvnBoolean,
    pub supports_draw_texture: ngpu::NvnBoolean,
}

#[inline(always)]
pub fn header_api_versions() -> (i32, i32) {
    (HEADER_API_MAJOR_VERSION, HEADER_API_MINOR_VERSION)
}

#[inline(always)]
pub unsafe fn slot_is_mapped(slot: usize) -> bool {
    ngpu::load_slot_fn::<usize>(slot) != 0
}

#[inline(always)]
pub unsafe fn check_api_version_compatibility() -> ApiVersionCheck {
    let _ = ngpu::bootstrap::try_initialize_from_cached_device();
    let _ = ngpu::bootstrap::probe_driver_api_versions_from_device(std::ptr::null());
    let (header_major, header_minor) = header_api_versions();

    let mut driver_major = -1;
    let mut driver_minor = -1;
    if let Some(device) = ngpu::bootstrap::cached_device() {
        ngpu::device::device_get_integer(
            device as *const ngpu::NvnDevice,
            NVN_DEVICE_INFO_API_MAJOR_VERSION,
            &mut driver_major,
        );
        ngpu::device::device_get_integer(
            device as *const ngpu::NvnDevice,
            NVN_DEVICE_INFO_API_MINOR_VERSION,
            &mut driver_minor,
        );
    } else if let Some((major, minor)) = ngpu::bootstrap::cached_driver_api_versions() {
        driver_major = major;
        driver_minor = minor;
    }

    let compatible = (driver_major == header_major && driver_minor >= header_minor) as ngpu::NvnBoolean;
    ApiVersionCheck {
        header_major,
        header_minor,
        driver_major,
        driver_minor,
        compatible,
    }
}

#[inline(always)]
pub unsafe fn check_clear_present_slots_mapped() -> ngpu::NvnBoolean {
    if !ngpu::is_initialized() {
        return 0;
    }
    (slot_is_mapped(SLOT_NVN_QUEUE_ACQUIRE_TEXTURE)
        && slot_is_mapped(SLOT_NVN_COMMAND_BUFFER_SET_RENDER_TARGETS)
        && slot_is_mapped(SLOT_NVN_COMMAND_BUFFER_CLEAR_COLOR)
        && slot_is_mapped(SLOT_NVN_COMMAND_BUFFER_END_RECORDING)
        && slot_is_mapped(SLOT_NVN_QUEUE_SUBMIT_COMMANDS)
        && slot_is_mapped(SLOT_NVN_QUEUE_PRESENT_TEXTURE)) as ngpu::NvnBoolean
}

#[inline(always)]
pub unsafe fn check_draw_texture_support() -> DrawTextureSupportCheck {
    let _ = ngpu::bootstrap::try_initialize_from_cached_device();
    let slot_mapped = if ngpu::is_initialized() {
        slot_is_mapped(SLOT_NVN_COMMAND_BUFFER_DRAW_TEXTURE) as ngpu::NvnBoolean
    } else {
        0
    };

    let mut supports_draw_texture = 0;
    if let Some(device) = ngpu::bootstrap::cached_device() {
        ngpu::device::device_get_integer(
            device as *const ngpu::NvnDevice,
            NVN_DEVICE_INFO_SUPPORTS_DRAW_TEXTURE,
            &mut supports_draw_texture,
        );
    } else if let Some(cached) = ngpu::bootstrap::cached_supports_draw_texture() {
        supports_draw_texture = cached as i32;
    } else {
        ngpu::device::device_get_integer(
            std::ptr::null(),
            NVN_DEVICE_INFO_SUPPORTS_DRAW_TEXTURE,
            &mut supports_draw_texture,
        );
    }

    DrawTextureSupportCheck {
        slot_mapped,
        supports_draw_texture: (supports_draw_texture != 0) as ngpu::NvnBoolean,
    }
}

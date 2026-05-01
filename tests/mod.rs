#![cfg(feature = "tests")]

pub mod check_api;
pub mod check_input;
pub mod dpad_menu;

#[repr(C)]
pub struct CompatibilityTestReport {
    pub api: check_api::ApiVersionCheck,
    pub clear_present_slots_mapped: crate::ngpu::NvnBoolean,
    pub draw_texture: check_api::DrawTextureSupportCheck,
    pub passed: crate::ngpu::NvnBoolean,
}

#[inline(always)]
pub unsafe fn run_compatibility_tests() -> CompatibilityTestReport {
    let api = check_api::check_api_version_compatibility();
    let clear_present_slots_mapped = check_api::check_clear_present_slots_mapped();
    let draw_texture = check_api::check_draw_texture_support();

    let passed = (api.compatible != 0
        && clear_present_slots_mapped != 0
        && draw_texture.slot_mapped != 0
        && draw_texture.supports_draw_texture != 0) as crate::ngpu::NvnBoolean;

    CompatibilityTestReport {
        api,
        clear_present_slots_mapped,
        draw_texture,
        passed,
    }
}

#[inline(always)]
pub unsafe fn run_compatibility_tests_passed() -> crate::ngpu::NvnBoolean {
    run_compatibility_tests().passed
}

pub fn install_tests() {
    crate::ngpu::debug::set_enabled(false);
    dpad_menu::install_dpad_debug();
}

pub mod check_api;
pub mod check_input;
pub mod dpad_menu;
#[cfg(feature = "rive-host-ffi")]
pub mod overlay;

#[cfg(feature = "rive-host-ffi")]
const RIVE_SAMPLE_RIV_SD_PATH: &str = "sd:/ultimate/ssbusync/sample.riv";

#[cfg(feature = "rive-host-ffi")]
fn load_rive_payload() -> Option<Vec<u8>> {
    match std::fs::read(RIVE_SAMPLE_RIV_SD_PATH) {
        Ok(bytes) => {
            ncommon::logN!(
                target: "overlay.rive",
                "loaded rive payload from {} bytes={}",
                RIVE_SAMPLE_RIV_SD_PATH,
                bytes.len()
            );
            Some(bytes)
        }
        Err(err) => {
            ncommon::logN!(
                target: "overlay.rive",
                "failed reading {}; rive overlay payload disabled ({})",
                RIVE_SAMPLE_RIV_SD_PATH,
                err
            );
            None
        }
    }
}

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

pub fn install_tests()  {
    #[cfg(feature = "rive-host-ffi")]
    unsafe {
        let rive_payload = load_rive_payload();
        let rive_payload_len = rive_payload.as_ref().map(|bytes| bytes.len()).unwrap_or(0);

        crate::tests::overlay::rive_overlay::set_rive_payload(rive_payload);
        // Known present-bound submit caller offset from prior trace: 0x37f0b40.
        // Keep wildcard caller matching for now because this offset drifts across builds;
        // once re-pinned for this exact binary, switch back to the concrete offset above.
        crate::tests::overlay::rive_overlay::set_submit_filter(0, None);
        crate::tests::overlay::rive_overlay::set_overlay_handle_provider(None);
        let overlay_hooked = crate::tests::overlay::rive_overlay::install_queue_submit_overlay_hook();
        ncommon::logN!(
            "overlay queue_submit_hook_installed={} rive_payload_len={}",
            overlay_hooked,
            rive_payload_len
        );
    }

    #[cfg(not(feature = "rive-host-ffi"))]
    ncommon::logN!("overlay queue_submit_hook_skipped rive feature disabled");

    crate::ngpu::debug::set_enabled(false);
    dpad_menu::install_dpad_debug();
}

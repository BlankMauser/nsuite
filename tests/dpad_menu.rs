use skyline::install_hooks;

use nsuite::ngpu;
use nsuite::ninput;

#[path = "check_api.rs"]
mod check_api;

pub type DebugBlitCallback = unsafe extern "C" fn(controller_id: u8);
pub type BufferSwapCallback = unsafe extern "C" fn(controller_id: u8);
pub type BufferSwapTickCallback = unsafe extern "C" fn();
pub type IndexToggleCallback = unsafe extern "C" fn(controller_id: u8);

static mut DEBUG_BLIT_CALLBACK: Option<DebugBlitCallback> = None;
static mut BUFFER_SWAP_CALLBACK: Option<BufferSwapCallback> = None;
static mut BUFFER_SWAP_TICK_CALLBACK: Option<BufferSwapTickCallback> = None;
static mut INDEX_TOGGLE_CALLBACK: Option<IndexToggleCallback> = None;
static mut PREV_MENU_COMBO_HELD: bool = false;
static mut PREV_SWAP_COMBO_HELD: bool = false;
static mut PREV_INDEX_COMBO_HELD: bool = false;
static mut INDEX_COMBO_DEBOUNCE_FRAMES_LEFT: u32 = 0;

const INDEX_COMBO_DEBOUNCE_FRAMES: u32 = 20;
   
#[repr(C, align(8))]
#[derive(Debug, Copy, Clone)]
pub struct CharaSelect {
    addr: *const u64,
    some_node: *const u64,
    unk1: [u64; 3],
    unk_structs: [[u64; 2]; 12],
    _0xe8: u32,
    _0xec: u32,
    _0xf0: u32,
    _0xf4: u32,
    _0xf8: u64,
    _ptr_100: *const u64, // layout related
    _pad1: [u64; 6],
    _0x138: u32,
    frames_elapsed: i32,
    loading_state: u32,
    _0x144: u32,
    _ptr_148: *const u64,
    _ptr_150: *const u64,
    unk_bytes: [u8; 8],
    current_player_count: u32, //union
    css_mode: u32, //union
    _0x168: u8,
    is_team_battle: bool,
    _0x16a: u8,
    _0x16b: u8,
    game_mode: u32,
    local_wireless: u32, // 1 in local wireless, otherwise may be unrelated
    ready_state: u32,
    _0x178: u32,
    min_players_allowed: u32, // aka min # of ui panes
    max_players_allowed: u32,
    _0x184: u32,
    _0x188: u64,
    player_buffer: *const u64,
    player_root: *const u64,
    _ptr_1a0: *const u64,
    players: [[u64;2]; 8], // not researched enough
    _pad2: [u64; 2],
    first_player: *const PlayerInfo,
    max_allowed_player: *const PlayerInfo,
    _ptr_248: *const u64,
    player_base: *const PlayerInfo,
    player_max: *const PlayerInfo,
    _ptr_260: *const u64,
    card_array_start: *const u64,
    card_array_end: *const u64,
    // theres way more here :)
}

#[repr(C, align(8))]
#[derive(Debug, Copy, Clone)]
pub struct PlayerInfo {
    root: *const u64,
    card: *const PlayerCard,
    next: *const PlayerInfo
}

// much like the other struct, largely undefined and potentially inaccurate
#[repr(C, align(8))]
#[derive(Debug, Copy, Clone)]
pub struct PlayerCard {
    _0x0: u64,
    parts: *const u64,
    layout: *const u64,
    _0x18: u64,
    css_instance: *const CharaSelect,
    active_slot_id: u32,
    current_state: u8,
    target_state: u8,
    bool_2e: bool,
    _0x2f: u8,
    index: u32,
    _0x34: u8,
    is_visible: bool,
    is_active: bool,
    root_pane: *const u64,
    _unk_range_40: [u64; 34],
    max_card_count: u32,
    _0x154: u32,
    _unk_range_158: [u64; 13],
    bool_1c0: bool,
    bool_1c1: bool,
    bool_1c2: bool,
    bool_1c3: bool,
    bool_1c4: bool,
    bool_1c5: bool,
    bool_1c6: bool,
    bool_1c7: bool,
    _0x1c8: u64,
    current_id: u16,
    id_1: u16,
    id_2: u16,
    id_3: u16,
    id_4: u16,
    id_5: u16,
    id_6: u16,
    id_7: u16,
    id_8: u16,
    id_9: u16,
    id_10: u16,
    _0x1e8: u64,
    player_num: u32,
    _0x1f4: u32,
    player_kind: i32, // 0 = player, 1 = cpu, 2 = amiibo, 3 = none
    _0x1fc: u32,
    _0x200: u64,
    _0x208: u64,
    _0x210: u8,
    bool_211: bool,
    _0x212: u8,
    _0x213: u8,
    _0x214: u16,
    _0x216: u16,
    _0x218: u32,
    card_type: u32,
    team_id: u32,
    _0x224: u32,
    _0x228: u64,
    _0x230: u64,
    _0x238: u64,
    max_card_count2: u32,
    layout_variant: u32,
    _0x248: u64,
    _unk_range_250: [u64; 40],
    controller_id: u32,
    some_state: u32,
    // theres more here
}

// this function loops while the css is active
#[skyline::hook(offset = 0x1a2b570)]
unsafe fn css_dpad_debug (arg: *const CharaSelect) {
    if ngpu::debug::enabled() {
        ngpu::debug::dbg_shapes::axis_cross_2d(
            0.0,
            0.0,
            32.0,
            0.0,
            ngpu::debug::DebugColor {
                r: 0.6,
                g: 0.6,
                b: 0.6,
                a: 1.0,
            },
        );
    }
    if on_frame_check_menu_combo()  {
        ncommon::logN!("Dpad Debug Successful");
    }
    if on_frame_check_swap_combo() {
        ncommon::logN!("Buffer swap combo triggered");
    }
    if on_frame_check_index_combo() {
        ncommon::logN!("Index toggle combo triggered");
    }
    if let Some(cb) = BUFFER_SWAP_TICK_CALLBACK {
        cb();
    }
    call_original!(arg)
}
    
pub fn install_dpad_debug()  {
    ncommon::logN!("dpad debug test");
    install_hooks!(css_dpad_debug);
}

#[inline(always)]
pub unsafe fn set_debug_blit_callback(callback: Option<DebugBlitCallback>) {
    DEBUG_BLIT_CALLBACK = callback;
}

#[inline(always)]
pub unsafe fn clear_debug_blit_callback() {
    DEBUG_BLIT_CALLBACK = None;
}

#[inline(always)]
pub unsafe fn set_buffer_swap_callback(callback: Option<BufferSwapCallback>) {
    BUFFER_SWAP_CALLBACK = callback;
}

#[inline(always)]
pub unsafe fn clear_buffer_swap_callback() {
    BUFFER_SWAP_CALLBACK = None;
}

#[inline(always)]
pub unsafe fn set_buffer_swap_tick_callback(callback: Option<BufferSwapTickCallback>) {
    BUFFER_SWAP_TICK_CALLBACK = callback;
}

#[inline(always)]
pub unsafe fn clear_buffer_swap_tick_callback() {
    BUFFER_SWAP_TICK_CALLBACK = None;
}

#[inline(always)]
pub unsafe fn set_index_toggle_callback(callback: Option<IndexToggleCallback>) {
    INDEX_TOGGLE_CALLBACK = callback;
}

#[inline(always)]
pub unsafe fn clear_index_toggle_callback() {
    INDEX_TOGGLE_CALLBACK = None;
}

#[inline(always)]
fn l_r_z_down_masks() -> (u64, u64) {
    (
        ninput::button_mask!(
            ninput::gamepad::KEY_L,
            ninput::gamepad::KEY_R,
            ninput::gamepad::KEY_ZL,
            ninput::gamepad::KEY_DDOWN
        ),
        ninput::button_mask!(
            ninput::gamepad::KEY_L,
            ninput::gamepad::KEY_R,
            ninput::gamepad::KEY_ZR,
            ninput::gamepad::KEY_DDOWN
        ),
    )
}

#[inline(always)]
fn l_r_z_right_masks() -> (u64, u64) {
    (
        ninput::button_mask!(
            ninput::gamepad::KEY_L,
            ninput::gamepad::KEY_R,
            ninput::gamepad::KEY_ZL,
            ninput::gamepad::KEY_DRIGHT
        ),
        ninput::button_mask!(
            ninput::gamepad::KEY_L,
            ninput::gamepad::KEY_R,
            ninput::gamepad::KEY_ZR,
            ninput::gamepad::KEY_DRIGHT
        ),
    )
}

#[inline(always)]
fn l_r_z_left_masks() -> (u64, u64) {
    (
        ninput::button_mask!(
            ninput::gamepad::KEY_L,
            ninput::gamepad::KEY_R,
            ninput::gamepad::KEY_ZL,
            ninput::gamepad::KEY_DLEFT
        ),
        ninput::button_mask!(
            ninput::gamepad::KEY_L,
            ninput::gamepad::KEY_R,
            ninput::gamepad::KEY_ZR,
            ninput::gamepad::KEY_DLEFT
        ),
    )
}

#[inline(always)]
pub unsafe fn find_menu_combo_controller() -> Option<u8> {
    let (zl_mask, zr_mask) = l_r_z_down_masks();
    ninput::FirstControllerWithAll(zl_mask).or_else(|| ninput::FirstControllerWithAll(zr_mask))
}

#[inline(always)]
pub unsafe fn find_swap_combo_controller() -> Option<u8> {
    let (zl_mask, zr_mask) = l_r_z_right_masks();
    ninput::FirstControllerWithAll(zl_mask).or_else(|| ninput::FirstControllerWithAll(zr_mask))
}

#[inline(always)]
pub unsafe fn find_index_combo_controller() -> Option<u8> {
    let (zl_mask, zr_mask) = l_r_z_left_masks();
    ninput::FirstControllerWithAll(zl_mask).or_else(|| ninput::FirstControllerWithAll(zr_mask))
}

#[inline(always)]
pub unsafe fn on_frame_check_menu_combo() -> bool {
    ninput::CheckInputs();
    let controller = find_menu_combo_controller();
    let held = controller.is_some();
    let rising_edge = held && !PREV_MENU_COMBO_HELD;
    PREV_MENU_COMBO_HELD = held;
    if !rising_edge {
        return false;
    }

    let api = check_api::check_api_version_compatibility();
    let draw = check_api::check_draw_texture_support();
    ncommon::logN!(
        target: "menu",
        "combo=L+R+Z+DDown controller={:?} nvn hdr={}.{} drv={}.{} compat={} draw(slot={},supported={})",
        controller,
        api.header_major,
        api.header_minor,
        api.driver_major,
        api.driver_minor,
        api.compatible,
        draw.slot_mapped,
        draw.supports_draw_texture
    );

    let next_overlay_enabled = !ngpu::debug::enabled();
    ngpu::debug::set_enabled(next_overlay_enabled);
    ncommon::logN!(
        target: "menu",
        "debug overlay toggled enabled={}",
        next_overlay_enabled
    );

    if ngpu::debug::enabled() {
        ngpu::debug::dbg_shapes::axis_cross_2d(
            0.0,
            0.0,
            32.0,
            0.0,
            ngpu::debug::DebugColor {
                r: 0.6,
                g: 0.6,
                b: 0.6,
                a: 1.0,
            },
        );
        ncommon::logN!(target: "menu", "queued debug axis cross");
    }

    if let (Some(cb), Some(id)) = (DEBUG_BLIT_CALLBACK, controller) {
        cb(id);
    }

    true
}

#[inline(always)]
pub unsafe fn on_frame_check_swap_combo() -> bool {
    ninput::CheckInputs();
    let controller = find_swap_combo_controller();
    let held = controller.is_some();
    let rising_edge = held && !PREV_SWAP_COMBO_HELD;
    PREV_SWAP_COMBO_HELD = held;
    if !rising_edge {
        return false;
    }

    if let (Some(cb), Some(id)) = (BUFFER_SWAP_CALLBACK, controller) {
        cb(id);
    }

    true
}

#[inline(always)]
pub unsafe fn on_frame_check_index_combo() -> bool {
    let callback = INDEX_TOGGLE_CALLBACK;
    if callback.is_none() {
        PREV_INDEX_COMBO_HELD = false;
        INDEX_COMBO_DEBOUNCE_FRAMES_LEFT = 0;
        return false;
    }

    ninput::CheckInputs();
    let controller = find_index_combo_controller();
    let held = controller.is_some();
    let rising_edge = held && !PREV_INDEX_COMBO_HELD;
    PREV_INDEX_COMBO_HELD = held;

    if INDEX_COMBO_DEBOUNCE_FRAMES_LEFT != 0 {
        INDEX_COMBO_DEBOUNCE_FRAMES_LEFT -= 1;
        if rising_edge {
            return false;
        }
    }

    if !rising_edge {
        return false;
    }

    INDEX_COMBO_DEBOUNCE_FRAMES_LEFT = INDEX_COMBO_DEBOUNCE_FRAMES;
    if let (Some(cb), Some(id)) = (callback, controller) {
        cb(id);
    }

    true
}

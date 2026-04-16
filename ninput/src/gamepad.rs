use std::sync::atomic::{AtomicBool, Ordering};
use skyline::nn::hid::*;

pub type Buttons = u64;

pub const NPAD_ID_PLAYER_1: u32 = 0;
pub const NPAD_ID_HANDHELD: u32 = 0x20;

pub const NPAD_CONNECTED: u32 = 1 << 0;

pub const KEY_X: Buttons = 1 << 2;
pub const KEY_PLUS: Buttons = 1 << 10;
pub const KEY_L: Buttons = 1 << 6;
pub const KEY_R: Buttons = 1 << 7;
pub const KEY_ZL: Buttons = 1 << 8;
pub const KEY_ZR: Buttons = 1 << 9;
pub const KEY_DLEFT: Buttons = 1 << 12;
pub const KEY_DUP: Buttons = 1 << 13;
pub const KEY_DRIGHT: Buttons = 1 << 14;
pub const KEY_DDOWN: Buttons = 1 << 15;

pub const STYLE_FULL_KEY: u32 = 1 << 0;
pub const STYLE_HANDHELD: u32 = 1 << 1;
pub const STYLE_JOY_DUAL: u32 = 1 << 2;
pub const STYLE_JOY_LEFT: u32 = 1 << 3;
pub const STYLE_JOY_RIGHT: u32 = 1 << 4;
pub const STYLE_GC: u32 = 1 << 5;

pub const MAX_SCANNED_PLAYER_IDS: u32 = 8;

static HID_INITIALIZED: AtomicBool = AtomicBool::new(false);
static HID_INIT_REQUESTED: AtomicBool = AtomicBool::new(true);

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ControllerKind {
    None = 0,
    Handheld = 1,
    ProController = 2,
    GcController = 3,
    XInput = 4,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct UnifiedNpadState {
    pub update_count: i64,
    pub buttons: Buttons,
    pub left_x: i32,
    pub left_y: i32,
    pub right_x: i32,
    pub right_y: i32,
    pub flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct InputProbe {
    pub id: u32,
    pub kind: ControllerKind,
    pub style_flags: u32,
    pub state: UnifiedNpadState,
}

impl Default for InputProbe {
    fn default() -> Self {
        Self {
            id: NPAD_ID_PLAYER_1,
            kind: ControllerKind::None,
            style_flags: 0,
            state: UnifiedNpadState::default(),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Controller {
    pub id: u32,
    pub kind: ControllerKind,
    pub style_flags: u32,
    pub buttons: Buttons,
    pub state: UnifiedNpadState,
}

impl Controller {
    pub const fn new(id: u32) -> Self {
        Self {
            id,
            kind: ControllerKind::None,
            style_flags: 0,
            buttons: 0,
            state: UnifiedNpadState {
                update_count: 0,
                buttons: 0,
                left_x: 0,
                left_y: 0,
                right_x: 0,
                right_y: 0,
                flags: 0,
            },
        }
    }

    #[inline(always)]
    pub unsafe fn update(&mut self) {
        let probe = probe_input(self.id);
        self.kind = probe.kind;
        self.style_flags = probe.style_flags;
        self.state = probe.state;
        self.buttons = probe.state.buttons;
    }
}

#[macro_export]
macro_rules! buttons_all_held {
    ($buttons:expr, $mask:expr) => {
        (($buttons as u64) & ($mask as u64)) == ($mask as u64)
    };
}

#[macro_export]
macro_rules! buttons_any_held {
    ($buttons:expr, $mask:expr) => {
        (($buttons as u64) & ($mask as u64)) != 0
    };
}

#[macro_export]
macro_rules! button_mask {
    ($first:expr $(, $rest:expr)+ $(,)?) => {
        (($first as u64) $(| ($rest as u64))+)
    };
    ($single:expr $(,)?) => {
        $single as u64
    };
}

#[inline(always)]
pub unsafe fn ensure_hid_initialized() {
    if !HID_INIT_REQUESTED.load(Ordering::Acquire) {
        return;
    }
    if HID_INITIALIZED.swap(true, Ordering::AcqRel) {
        return;
    }
    InitializeNpad();
}

/// Call this only in standalone tools where the game has not already initialized hid.
#[inline(always)]
pub fn request_hid_initialize() {
    HID_INIT_REQUESTED.store(true, Ordering::Release);
}

#[inline(always)]
pub fn disable_hid_initialize() {
    HID_INIT_REQUESTED.store(false, Ordering::Release);
}

#[inline(always)]
unsafe fn from_handheld_state(state: NpadHandheldState) -> UnifiedNpadState {
    UnifiedNpadState {
        update_count: state.updateCount,
        buttons: state.Buttons,
        left_x: state.LStickX,
        left_y: state.LStickY,
        right_x: state.RStickX,
        right_y: state.RStickY,
        flags: state.Flags,
    }
}

#[inline(always)]
unsafe fn read_full_key_state(id: u32) -> UnifiedNpadState {
    let mut state = NpadHandheldState::default();
    GetNpadFullKeyState(&mut state, &id);
    from_handheld_state(state)
}

#[inline(always)]
unsafe fn read_joy_dual_state(id: u32) -> UnifiedNpadState {
    let mut state = NpadHandheldState::default();
    GetNpadJoyDualState(&mut state, &id);
    from_handheld_state(state)
}

#[inline(always)]
unsafe fn read_joy_left_state(id: u32) -> UnifiedNpadState {
    let mut state = NpadHandheldState::default();
    GetNpadJoyLeftState(&mut state, &id);
    from_handheld_state(state)
}

#[inline(always)]
unsafe fn read_joy_right_state(id: u32) -> UnifiedNpadState {
    let mut state = NpadHandheldState::default();
    GetNpadJoyRightState(&mut state, &id);
    from_handheld_state(state)
}

#[inline(always)]
unsafe fn read_handheld_state(id: u32) -> UnifiedNpadState {
    let mut state = NpadHandheldState::default();
    GetNpadHandheldState(&mut state, &id);
    from_handheld_state(state)
}

#[inline(always)]
unsafe fn read_gc_state(id: u32) -> UnifiedNpadState {
    let mut gc = NpadGcState::default();
    GetNpadGcState(&mut gc, &id);
    UnifiedNpadState {
        update_count: gc.updateCount,
        buttons: gc.Buttons,
        left_x: gc.LStickX,
        left_y: gc.LStickY,
        right_x: gc.RStickX,
        right_y: gc.RStickY,
        flags: gc.Flags,
    }
}

#[inline(always)]
unsafe fn is_active(state: UnifiedNpadState) -> bool {
    (state.flags & NPAD_CONNECTED) != 0 || state.buttons != 0
}

#[inline(always)]
unsafe fn read_primary_state(id: u32, style_flags: u32) -> UnifiedNpadState {
    if (style_flags & STYLE_GC) != 0 {
        let gc = read_gc_state(id);
        if is_active(gc) {
            return gc;
        }
    }

    if (style_flags & STYLE_FULL_KEY) != 0 {
        let full = read_full_key_state(id);
        if is_active(full) {
            return full;
        }
    }

    if (style_flags & STYLE_JOY_DUAL) != 0 {
        let dual = read_joy_dual_state(id);
        if is_active(dual) {
            return dual;
        }
    }

    if (style_flags & STYLE_JOY_LEFT) != 0 {
        let left = read_joy_left_state(id);
        if is_active(left) {
            return left;
        }
    }

    if (style_flags & STYLE_JOY_RIGHT) != 0 {
        let right = read_joy_right_state(id);
        if is_active(right) {
            return right;
        }
    }

    let full = read_full_key_state(id);
    if is_active(full) {
        return full;
    }
    let dual = read_joy_dual_state(id);
    if is_active(dual) {
        return dual;
    }

    if id == NPAD_ID_PLAYER_1 {
        let hh = read_handheld_state(NPAD_ID_HANDHELD);
        if is_active(hh) {
            return hh;
        }
    }

    if (style_flags & STYLE_HANDHELD) != 0 {
        let hh = read_handheld_state(id);
        if is_active(hh) {
            return hh;
        }
    }

    UnifiedNpadState::default()
}

#[inline(always)]
unsafe fn read_style_flags(id: u32) -> u32 {
    GetNpadStyleSet(&id).flags
}

#[inline(always)]
unsafe fn detect_kind(style_flags: u32, state: UnifiedNpadState) -> ControllerKind {
    if (style_flags & STYLE_GC) != 0 {
        if (state.flags & NPAD_CONNECTED) != 0 || state.buttons != 0 {
            return ControllerKind::GcController;
        }
    }
    if (style_flags & STYLE_HANDHELD) != 0 {
        return ControllerKind::Handheld;
    }
    if (style_flags & (STYLE_FULL_KEY | STYLE_JOY_DUAL | STYLE_JOY_LEFT | STYLE_JOY_RIGHT)) != 0 {
        return ControllerKind::ProController;
    }
    if (state.flags & NPAD_CONNECTED) != 0 || state.buttons != 0 {
        return ControllerKind::ProController;
    }
    ControllerKind::None
}

#[inline(always)]
pub unsafe fn probe_input(id: u32) -> InputProbe {
    ensure_hid_initialized();
    let style_flags = read_style_flags(id);
    let state = read_primary_state(id, style_flags);

    InputProbe {
        id,
        kind: detect_kind(style_flags, state),
        style_flags,
        state,
    }
}

#[inline(always)]
pub unsafe fn modifier_l_r_z_is_held(id: u32) -> bool {
    let probe = probe_input(id);
    buttons_all_held!(probe.state.buttons, button_mask!(KEY_L, KEY_R, KEY_ZL))
        || buttons_all_held!(probe.state.buttons, button_mask!(KEY_L, KEY_R, KEY_ZR))
}

#[inline(always)]
pub unsafe fn check_inputs() -> u32 {
    ensure_hid_initialized();
    let mut connected = 0u32;
    for id in 0..MAX_SCANNED_PLAYER_IDS {
        let probe = probe_input(id);
        if (probe.state.flags & NPAD_CONNECTED) == 0 {
            return connected;
        }
        connected += 1;
    }
    connected
}

#[allow(non_snake_case)]
#[inline(always)]
pub unsafe fn ConnectedControllerCount() -> u32 {
    check_inputs()
}

use crate::ninput::gamepad;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct InputCheckReport {
    pub id: u32,
    pub kind: gamepad::ControllerKind,
    pub style_flags: u32,
    pub buttons: gamepad::Buttons,
    pub modifier_l_r_z_held: bool,
}

#[inline(always)]
pub unsafe fn run_input_check(id: u32) -> InputCheckReport {
    let probe = gamepad::probe_input(id);
    let buttons = probe.state.buttons;
    let modifier_l_r_z_held = ((buttons & (gamepad::KEY_L | gamepad::KEY_R | gamepad::KEY_ZL))
        == (gamepad::KEY_L | gamepad::KEY_R | gamepad::KEY_ZL))
        || ((buttons & (gamepad::KEY_L | gamepad::KEY_R | gamepad::KEY_ZR))
            == (gamepad::KEY_L | gamepad::KEY_R | gamepad::KEY_ZR));
    InputCheckReport {
        id,
        kind: probe.kind,
        style_flags: probe.style_flags,
        buttons,
        modifier_l_r_z_held,
    }
}

#[inline(always)]
pub unsafe fn run_player1_input_check() -> InputCheckReport {
    run_input_check(gamepad::NPAD_ID_PLAYER_1)
}

#[inline(always)]
pub unsafe fn log_input_check(id: u32) -> InputCheckReport {
    let report = run_input_check(id);
    println!(
        "[nsuite][input] id={} kind={:?} style=0x{:08x} buttons=0x{:016x} L+R+Z={}",
        report.id,
        report.kind,
        report.style_flags,
        report.buttons,
        report.modifier_l_r_z_held
    );
    report
}

#[inline(always)]
pub unsafe fn log_player1_input_check() -> InputCheckReport {
    log_input_check(gamepad::NPAD_ID_PLAYER_1)
}

use std::sync::{Mutex, OnceLock};

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct DebugColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct DebugLine {
    pub x0: f32,
    pub y0: f32,
    pub z0: f32,
    pub x1: f32,
    pub y1: f32,
    pub z1: f32,
    pub color: DebugColor,
}

static DRAW_LINES: OnceLock<Mutex<Vec<DebugLine>>> = OnceLock::new();

#[inline(always)]
fn list() -> &'static Mutex<Vec<DebugLine>> {
    DRAW_LINES.get_or_init(|| Mutex::new(Vec::with_capacity(1024)))
}

#[inline(always)]
pub fn clear_draw_list() {
    if let Ok(mut lines) = list().lock() {
        lines.clear();
    }
}

#[inline(always)]
pub fn push_line(line: DebugLine) {
    if let Ok(mut lines) = list().lock() {
        lines.push(line);
    }
}

#[inline(always)]
pub fn lines_snapshot() -> Vec<DebugLine> {
    if let Ok(lines) = list().lock() {
        lines.clone()
    } else {
        Vec::new()
    }
}

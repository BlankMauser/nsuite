use crate::debug::dbg_draw::{DebugColor, DebugLine, push_line};

#[inline(always)]
pub fn line3(x0: f32, y0: f32, z0: f32, x1: f32, y1: f32, z1: f32, color: DebugColor) {
    push_line(DebugLine {
        x0,
        y0,
        z0,
        x1,
        y1,
        z1,
        color,
    });
}

#[inline(always)]
pub fn axis_cross_2d(cx: f32, cy: f32, extent: f32, z: f32, color: DebugColor) {
    line3(cx - extent, cy, z, cx + extent, cy, z, color);
    line3(cx, cy - extent, z, cx, cy + extent, z, color);
}

pub fn circle_2d(cx: f32, cy: f32, radius: f32, z: f32, segments: usize, color: DebugColor) {
    if segments < 3 || radius <= 0.0 {
        return;
    }
    let step = core::f32::consts::TAU / segments as f32;
    let mut prev_x = cx + radius;
    let mut prev_y = cy;
    let mut i = 1usize;
    while i <= segments {
        let t = i as f32 * step;
        let x = cx + radius * t.cos();
        let y = cy + radius * t.sin();
        line3(prev_x, prev_y, z, x, y, z, color);
        prev_x = x;
        prev_y = y;
        i += 1;
    }
}

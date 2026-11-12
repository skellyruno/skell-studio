// Copyright (C) 2026 Skelly and upstream contributors
// SPDX-License-Identifier: GPL-3.0-or-later
//
// Shared, source-drawn Skell Studio chrome. No imported product artwork,
// runtime textures, allocations, timers or UI hit regions.

use os_ui::{Color, Rect};
use crate::app::Ui;

/// A low-glare, hardware-inspired face that follows the active theme.
/// Returns early for tiny rectangles so resized tracks cannot create invalid geometry.
pub fn panel(ui: &mut Ui, rect: Rect, selected: bool, tint: Option<Color>) {
    if rect.w < 4.0 || rect.h < 4.0 { return; }
    let t = ui.t;
    let edge = if selected { t.accent } else { t.window_border };
    let background = t.window_body.mix(t.track_header, 0.34);
    let tint = tint.unwrap_or(background);
    let face = background.mix(tint, if selected { 0.30 } else { 0.12 });
    let radius = t.style.radius_small.min(rect.h * 0.28).max(1.0);
    ui.c.fill_rect_gradient(rect, face.shade(0.06), face.shade(-0.13), radius);
    ui.c.stroke_rrect(rect.inset(0.5), radius, 1.0, edge.with_alpha(if selected { 215 } else { 125 }));
    // Machined bevel and a thin signal rail, separate from text and controls.
    if rect.w > 26.0 && rect.h > 10.0 {
        ui.c.fill_rect(
            Rect::new(rect.x + 4.0, rect.y + 2.0, rect.w - 8.0, 1.0),
            t.text.with_alpha(if selected { 52 } else { 24 }),
        );
    }
    if selected && rect.h >= 14.0 {
        ui.c.fill_rrect(
            Rect::new(rect.x + 2.0, rect.y + 4.0, 2.0, (rect.h - 8.0).max(2.0)),
            1.0,
            t.accent,
        );
    }
}

/// A small metallic cross-head fastener, drawn as vectors (no external assets).
pub fn screw(ui: &mut Ui, x: f32, y: f32, radius: f32) {
    if radius < 2.0 { return; }
    let t = ui.t;
    ui.c.fill_circle(x, y, radius, t.s(0x111911));
    ui.c.fill_circle(x - 0.3, y - 0.4, radius - 0.65, t.s(0x667166));
    ui.c.stroke_circle(x, y, radius - 0.2, 0.8, t.s(0x0b0e0c));
    let line = (radius * 0.51).max(1.0);
    let ink = t.s(0x202820);
    ui.c.line(x - line, y, x + line, y, 0.85, ink);
    ui.c.line(x, y - line, x, y + line, 0.85, ink);
}

/// A glassy, neutral meter well, used only behind the existing meter DSP visuals.
pub fn meter_well(ui: &mut Ui, rect: Rect, selected: bool) {
    if rect.w < 5.0 || rect.h < 8.0 { return; }
    let t = ui.t;
    ui.c.fill_rrect(rect, 3.0, t.s(0x050a07));
    ui.c.fill_rect(
        Rect::new(rect.x + 1.0, rect.y + 2.0, 1.0, (rect.h - 4.0).max(0.0)),
        t.text.with_alpha(27),
    );
    ui.c.stroke_rrect(rect.inset(0.5), 3.0, 1.0,
        if selected { t.accent.with_alpha(170) } else { t.window_border });
}

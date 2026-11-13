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

/// Shared, responsive button face for arrangement, piano and mixer tools.
/// We deliberately draw only visuals: each caller retains its existing hit
/// rect, shortcut, accessibility hint, custom skins and state transitions.
pub fn tool(ui: &mut Ui, rect: Rect, selected: bool, hovered: bool) -> Color {
    let t = ui.t;
    if rect.w < 4.0 || rect.h < 4.0 {
        return if selected { t.accent } else { t.glyph };
    }
    let radius = (rect.h * 0.23).clamp(2.0, 4.5);
    let face = if selected {
        t.button.mix(t.accent, if hovered { 0.23 } else { 0.15 })
    } else if hovered {
        t.button_hi
    } else {
        t.button
    };
    ui.c.fill_rect_gradient(rect, face.shade(0.12), face.shade(-0.14), radius);
    ui.c.stroke_rrect(rect.inset(0.5), radius, 1.0,
        if selected { t.accent.with_alpha(220) }
        else if hovered { t.text_dim.with_alpha(130) }
        else { t.window_border.with_alpha(110) });
    if selected && rect.w > 10.0 && rect.h > 9.0 {
        // A slim "energized" underline, independent of all hover animations.
        ui.c.fill_rrect(
            Rect::new(rect.x + 3.0, rect.bottom() - 2.7, (rect.w - 6.0).max(1.0), 1.5),
            0.75, t.accent,
        );
    }
    if selected || hovered { t.accent } else { t.glyph }
}

/// Uniform low-contrast title-band trim. No extra window hit regions.
pub fn title_trim(ui: &mut Ui, rect: Rect) {
    if rect.w < 35.0 || rect.h < 20.0 { return; }
    let t = ui.t;
    ui.c.fill_rect(Rect::new(rect.x, rect.y + 21.0, rect.w, 1.0), t.window_border);
    ui.c.fill_rect(Rect::new(rect.x + 1.0, rect.y + 21.0,
        (rect.w * 0.27).min(210.0), 1.0), t.accent.with_alpha(150));
}

/// Restrained Skell signal trace for large panel dividers. It never owns a hit
/// region and scales down without clipping or obscuring labels.
pub fn signal_trace(ui: &mut Ui, rect: Rect, active: bool) {
    if rect.w < 24.0 || rect.h < 6.0 { return; }
    let t = ui.t;
    let y = rect.bottom() - 1.5;
    let length = (rect.w * 0.24).clamp(12.0, 92.0).min(rect.w - 6.0);
    ui.c.fill_rect(Rect::new(rect.x + 3.0, y, rect.w - 6.0, 1.0),
        t.window_border.with_alpha(110));
    ui.c.fill_rect(Rect::new(rect.x + 3.0, y, length, 1.0),
        t.accent.with_alpha(if active { 225 } else { 100 }));
}

/// A passive status stripe for browser rows and FX slots. Existing click,
/// keyboard and accessibility behavior stays with the caller.
pub fn status_stripe(ui: &mut Ui, rect: Rect, active: bool) {
    if rect.w < 18.0 || rect.h < 10.0 { return; }
    let t = ui.t;
    let color = if active { t.accent } else { t.window_border };
    ui.c.fill_rrect(Rect::new(rect.x + 1.0, rect.y + 3.0, 2.0,
        (rect.h - 6.0).max(1.0)), 1.0, color.with_alpha(if active { 220 } else { 85 }));
}


/// Arrangement track identity: a short, dimmed color rail that does not
/// compete with the selection marker or the track's mute/solo controls.
pub fn track_rail(ui: &mut Ui, rect: Rect, tint: Option<Color>, enabled: bool) {
    if rect.w < 28.0 || rect.h < 15.0 { return; }
    let t = ui.t;
    let color = tint.unwrap_or(t.accent);
    let rail = Rect::new(rect.x + 8.0, rect.bottom() - 3.0,
        (rect.w * 0.31).clamp(14.0, 50.0).min(rect.w - 16.0), 1.0);
    ui.c.fill_rect(rail, color.with_alpha(if enabled { 175 } else { 58 }));
}

/// A passive two-stroke note edge. The actual note body, selection and resize
/// handles are still painted by the piano roll; this is only a legibility cue.
pub fn note_edge(ui: &mut Ui, rect: Rect, selected: bool) {
    if rect.w < 14.0 || rect.h < 9.0 { return; }
    let t = ui.t;
    ui.c.fill_rect(Rect::new(rect.x + 3.0, rect.y + 1.0,
        (rect.w - 6.0).min(28.0), 1.0),
        t.text.with_alpha(if selected { 110 } else { 53 }));
    ui.c.fill_rect(Rect::new(rect.x + 2.0, rect.bottom() - 2.0,
        (rect.w - 4.0).min(18.0), 1.0),
        t.accent.with_alpha(if selected { 200 } else { 85 }));
}

/// Fixed-height calibration marks around the existing mixer meter. It does not
/// sample audio or replace any VU/peak rendering, avoiding DSP/latency changes.
pub fn meter_scale(ui: &mut Ui, rect: Rect, selected: bool) {
    if rect.w < 22.0 || rect.h < 48.0 { return; }
    let t = ui.t;
    for index in 1..6 {
        let y = rect.y + rect.h * index as f32 / 6.0;
        let width = if index % 2 == 0 { 5.0 } else { 3.0 };
        ui.c.fill_rect(Rect::new(rect.right() - width - 2.0, y, width, 1.0),
            if selected { t.accent.with_alpha(120) } else { t.text_dim.with_alpha(90) });
    }
}

/// Browser category rows share the same subdued chrome as the arrangement and
/// mixer. All folder and plugin activation targets remain in their callers.
pub fn browser_row(ui: &mut Ui, rect: Rect, active: bool, hovered: bool) {
    if rect.w < 35.0 || rect.h < 13.0 { return; }
    let t = ui.t;
    if active || hovered {
        ui.c.fill_rect(Rect::new(rect.x + 5.0, rect.bottom() - 1.0,
            (rect.w - 10.0).max(0.0), 1.0),
            t.accent.with_alpha(if active { 145 } else { 55 }));
    }
}

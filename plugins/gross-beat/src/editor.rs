// Copyright (C) 2026 Skelly Audio
// SPDX-License-Identifier: AGPL-3.0-only

//! Skell Beat's native 72-slot envelope editor.
//! State parsing and serialization happen on the editor thread. The C++ bridge hands prepared
//! snapshots to the audio thread, where GrossBeat::Engine::update() uses its pre-reserved slots.

use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use nih_plug::prelude::GuiContext;
use lemon_small_fx::editor::{self, Extension};
use lemon_small_fx::params::FxParams;
use lemon_small_fx::view::{self, ViewState};
use os_theme::design::lemon as brand;
use os_ui::kit::lemon::plug;
use os_ui::{Canvas, Color, Font, Rect};

unsafe extern "C" {
    fn lynx_gb_state_point_count(data: *const u8, size: usize, slot: i32) -> i32;
    fn lynx_gb_state_get_point(data: *const u8, size: usize, slot: i32, index: i32,
        x: *mut f64, y: *mut f32, tension: *mut f32, kind: *mut i32) -> bool;
    fn lynx_gb_state_render_slot(data: *const u8, size: usize, slot: i32, out: *mut f32, count: i32) -> i32;
    fn lynx_gb_state_level(data: *const u8, size: usize, slot: i32, beat: f64) -> f32;
    fn lynx_gb_state_midi_trigger(data: *const u8, size: usize) -> i32;
    fn lynx_gb_state_edit(data: *const u8, size: usize, operation: i32, slot: i32, index: i32,
        x: f64, y: f32, kind: i32, tension: f32, out: *mut u8, capacity: i32, result_index: *mut i32) -> i32;
    fn lynx_gb_state_set_midi_trigger(data: *const u8, size: usize, enabled: bool,
        out: *mut u8, capacity: i32) -> i32;
    fn lynx_gb_queue_state(instance: *mut c_void, data: *const u8, size: usize) -> bool;
}

const LEFT_X: f32 = 8.0;
const LEFT_Y: f32 = 210.0;
const LEFT_W: f32 = 308.0;
const PANEL_H: f32 = 300.0;
const GRAPH_X: f32 = 338.0;
const GRAPH_Y: f32 = 258.0;
const GRAPH_W: f32 = 742.0;
const GRAPH_H: f32 = 211.0;
const SLOT_X: f32 = 16.0;
const SLOT_Y: f32 = 268.0;
const SLOT_W: f32 = 46.0;
const SLOT_H: f32 = 29.0;
const SLOT_GAP: f32 = 4.0;

const CURVE_NAMES: [&str; 13] = [
    "Single", "Double", "Hold", "Stairs", "Smooth stairs", "Pulse", "Wave",
    "Single 2", "Double 2", "Half sine", "Smooth", "Single 3", "Double 3",
];

#[derive(Clone, Copy, Default)]
struct Point {
    x: f64,
    y: f32,
    tension: f32,
    kind: i32,
}

#[derive(Default)]
struct UiState {
    kind: usize,
    slot: usize,
    selected: Option<usize>,
    dragging: bool,
}

pub struct BeatEditorExtension {
    params: Arc<FxParams>,
    engine: AtomicUsize,
    ui: Mutex<UiState>,
}

impl BeatEditorExtension {
    pub fn new(params: Arc<FxParams>) -> Self {
        Self { params, engine: AtomicUsize::new(0), ui: Mutex::new(UiState::default()) }
    }

    fn raw(&self) -> Vec<u8> { self.params.raw_copy() }

    fn slot_index(ui: &UiState) -> i32 { (ui.kind * 36 + ui.slot) as i32 }

    fn point_count(&self, raw: &[u8], slot: i32) -> usize {
        unsafe { lynx_gb_state_point_count(raw.as_ptr(), raw.len(), slot).max(0) as usize }
    }

    fn point(&self, raw: &[u8], slot: i32, index: usize) -> Option<Point> {
        let (mut x, mut y, mut tension, mut kind) = (0.0, 1.0, 0.0, 2);
        let ok = unsafe {
            lynx_gb_state_get_point(raw.as_ptr(), raw.len(), slot, index as i32,
                &mut x, &mut y, &mut tension, &mut kind)
        };
        ok.then_some(Point { x, y, tension, kind })
    }

    fn points(&self, raw: &[u8], slot: i32) -> Vec<Point> {
        (0..self.point_count(raw, slot)).filter_map(|i| self.point(raw, slot, i)).collect()
    }

    fn commit_state(&self, state: Vec<u8>) -> bool {
        let handle = self.engine.load(Ordering::Acquire);
        if handle != 0 {
            let ok = unsafe {
                lynx_gb_queue_state(handle as *mut c_void, state.as_ptr(), state.len())
            };
            if !ok { return false; }
        }
        self.params.set_raw(state);
        true
    }

    fn edit(&self, operation: i32, slot: i32, index: usize, x: f64, y: f32,
        kind: i32, tension: f32) -> Option<usize> {
        let raw = self.raw();
        let mut result_index = index as i32;
        let needed = unsafe {
            lynx_gb_state_edit(raw.as_ptr(), raw.len(), operation, slot, index as i32,
                x, y, kind, tension, std::ptr::null_mut(), 0, &mut result_index)
        };
        if needed <= 0 { return None; }
        let mut updated = vec![0u8; needed as usize];
        let written = unsafe {
            lynx_gb_state_edit(raw.as_ptr(), raw.len(), operation, slot, index as i32,
                x, y, kind, tension, updated.as_mut_ptr(), needed, &mut result_index)
        };
        if written != needed { return None; }
        if !self.commit_state(updated) { return None; }
        Some(result_index.max(0) as usize)
    }

    fn set_midi(&self, enabled: bool) -> bool {
        let raw = self.raw();
        let needed = unsafe {
            lynx_gb_state_set_midi_trigger(raw.as_ptr(), raw.len(), enabled, std::ptr::null_mut(), 0)
        };
        if needed <= 0 { return false; }
        let mut updated = vec![0u8; needed as usize];
        let written = unsafe {
            lynx_gb_state_set_midi_trigger(raw.as_ptr(), raw.len(), enabled, updated.as_mut_ptr(), needed)
        };
        written == needed && self.commit_state(updated)
    }

    fn midi_enabled(&self, raw: &[u8]) -> bool {
        unsafe { lynx_gb_state_midi_trigger(raw.as_ptr(), raw.len()) > 0 }
    }

    fn graph_point(x: f32, y: f32) -> (f64, f32) {
        let beat = ((x - GRAPH_X) / GRAPH_W).clamp(0.0, 1.0) as f64 * 4.0;
        let level = (1.0 - (y - GRAPH_Y) / GRAPH_H).clamp(0.0, 1.0);
        (beat, level)
    }

    fn graph_xy(point: Point) -> (f32, f32) {
        (GRAPH_X + (point.x / 4.0) as f32 * GRAPH_W, GRAPH_Y + (1.0 - point.y) * GRAPH_H)
    }

    fn cycle_curve(&self, ui: &UiState) {
        let Some(index) = ui.selected else { return };
        let raw = self.raw();
        let slot = Self::slot_index(ui);
        let Some(p) = self.point(&raw, slot, index) else { return };
        let kind = (p.kind + 1) % CURVE_NAMES.len() as i32;
        let _ = self.edit(3, slot, index, p.x, p.y, kind, p.tension);
    }

    fn draw_text(c: &mut Canvas, x: f32, y: f32, color: Color, text: &str) {
        c.text(Font::new(11).bold(), x, y, color, text);
    }
}

impl Extension for BeatEditorExtension {
    fn bind_engine_handle(&self, handle: usize) {
        self.engine.store(handle, Ordering::Release);
    }

    fn render(&self, _canvas: &mut Canvas, _model: &dyn view::Model,
        _state: &ViewState, _hint: &str) -> Option<Vec<view::Hit>> {
        None // retain the standard eight automation controls; the envelope editor is an overlay panel
    }

    fn paint(&self, c: &mut Canvas, _model: &dyn view::Model) {
        let ui = self.ui.lock().unwrap_or_else(|e| e.into_inner());
        let raw = self.raw();
        let slot = Self::slot_index(&ui);
        let points = self.points(&raw, slot);
        let midi = self.midi_enabled(&raw);

        let panel = Color::rgb(0x0b100f);
        let tile = Color::rgb(0x141c19);
        let muted = Color::rgb(0x74827b);
        let line = Color::rgb(0x2a3932);
        let bright = brand::MAIN;

        c.fill_rrect(Rect::new(LEFT_X, LEFT_Y, LEFT_W, PANEL_H), 8.0, panel);
        c.fill_rrect(Rect::new(GRAPH_X, LEFT_Y, 752.0, PANEL_H), 8.0, panel);
        Self::draw_text(c, 18.0, 225.0, bright, "72 ENVELOPE SLOTS");
        Self::draw_text(c, 340.0, 225.0, bright, "CURVE / POINT EDITOR");

        let time_tab = Rect::new(16.0, 232.0, 142.0, 26.0);
        let volume_tab = Rect::new(166.0, 232.0, 142.0, 26.0);
        c.fill_rrect(time_tab, 5.0, if ui.kind == 0 { bright } else { tile });
        c.fill_rrect(volume_tab, 5.0, if ui.kind == 1 { bright } else { tile });
        Self::draw_text(c, 52.0, 240.0, if ui.kind == 0 { Color::rgb(0x06100b) } else { brand::TEXT }, "TIME");
        Self::draw_text(c, 199.0, 240.0, if ui.kind == 1 { Color::rgb(0x06100b) } else { brand::TEXT }, "VOLUME");

        for row in 0..6 {
            for col in 0..6 {
                let index = row * 6 + col;
                let x = SLOT_X + col as f32 * (SLOT_W + SLOT_GAP);
                let y = SLOT_Y + row as f32 * (SLOT_H + SLOT_GAP);
                let selected = index == ui.slot;
                let r = Rect::new(x, y, SLOT_W, SLOT_H);
                c.fill_rrect(r, 4.0, if selected { bright } else { tile });
                let label = format!("{:02}", index + 1);
                Self::draw_text(c, x + 16.0, y + 8.0,
                    if selected { Color::rgb(0x07110b) } else { brand::TEXT }, &label);
            }
        }
        let slot_name = if ui.kind == 0 { "TIME" } else { "VOLUME" };
        Self::draw_text(c, 18.0, 472.0, muted, &format!("{} SLOT {:02}", slot_name, ui.slot + 1));

        // Graph background and grid.
        c.fill_rrect(Rect::new(GRAPH_X, GRAPH_Y, GRAPH_W, GRAPH_H), 4.0, Color::rgb(0x080c0b));
        for i in 0..=4 {
            let x = GRAPH_X + GRAPH_W * i as f32 / 4.0;
            c.line(x, GRAPH_Y, x, GRAPH_Y + GRAPH_H, 1.0, line);
            if i < 4 {
                Self::draw_text(c, x + 4.0, GRAPH_Y + GRAPH_H + 2.0, muted, &format!("{}", i));
            }
        }
        for i in 0..=4 {
            let y = GRAPH_Y + GRAPH_H * i as f32 / 4.0;
            c.line(GRAPH_X, y, GRAPH_X + GRAPH_W, y, 1.0, line);
        }
        let mut curve = vec![0.0f32; 128];
        let rendered = unsafe {
            lynx_gb_state_render_slot(raw.as_ptr(), raw.len(), slot, curve.as_mut_ptr(), curve.len() as i32)
        };
        if rendered == curve.len() as i32 {
            let path: Vec<(f32, f32)> = curve.iter().enumerate().map(|(i, level)| {
                let x = GRAPH_X + i as f32 / (curve.len() - 1) as f32 * GRAPH_W;
                let y = GRAPH_Y + (1.0 - level.clamp(0.0, 1.0)) * GRAPH_H;
                (x, y)
            }).collect();
            c.polyline(&path, 2.0, bright);
        }
        for (i, point) in points.iter().copied().enumerate() {
            let (x, y) = Self::graph_xy(point);
            c.fill_circle(x, y, if ui.selected == Some(i) { 7.5 } else { 5.0 },
                if ui.selected == Some(i) { Color::rgb(0xf3fff5) } else { bright });
            c.fill_circle(x, y, 2.2, Color::rgb(0x08110c));
        }

        let selected = ui.selected.and_then(|i| points.get(i).copied());
        let curve_label = selected.map_or("SELECT POINT".to_string(), |p| {
            format!("{}  |  TENSION {:+.2}", CURVE_NAMES[p.kind.clamp(0, 12) as usize], p.tension)
        });
        c.fill_rrect(Rect::new(340.0, 480.0, 190.0, 27.0), 5.0, tile);
        Self::draw_text(c, 350.0, 488.0, brand::TEXT, &curve_label);
        c.fill_rrect(Rect::new(538.0, 480.0, 84.0, 27.0), 5.0, tile);
        Self::draw_text(c, 560.0, 488.0, bright, "ADD");
        c.fill_rrect(Rect::new(628.0, 480.0, 90.0, 27.0), 5.0, tile);
        Self::draw_text(c, 647.0, 488.0, if selected.is_some_and(|_| ui.selected.unwrap_or(0) > 0 && ui.selected.unwrap_or(0) + 1 < points.len()) { brand::TEXT } else { muted }, "REMOVE");
        c.fill_rrect(Rect::new(918.0, 480.0, 162.0, 27.0), 5.0, if midi { bright } else { tile });
        Self::draw_text(c, 944.0, 488.0, if midi { Color::rgb(0x07110b) } else { brand::TEXT },
            if midi { "MIDI: ON" } else { "MIDI: OFF" });

        // Keep the current point values visible without stealing room from the curve.
        if let Some(p) = selected {
            let x = format!("X {:.3} beats", p.x);
            let y = format!("Y {:.3}", p.y);
            Self::draw_text(c, 734.0, 488.0, muted, &x);
            Self::draw_text(c, 832.0, 488.0, muted, &y);
        }
    }

    fn click(&self, x: f32, y: f32, _context: Option<&Arc<dyn GuiContext>>) -> bool {
        if y < LEFT_Y || y > 512.0 { return false; }
        if x < LEFT_X || x > 1090.0 { return false; }
        let mut ui = self.ui.lock().unwrap_or_else(|e| e.into_inner());
        if y >= 232.0 && y <= 258.0 && x >= 16.0 && x <= 308.0 {
            ui.kind = if x < 160.0 { 0 } else { 1 };
            ui.selected = None;
            ui.dragging = false;
            return true;
        }
        if y >= SLOT_Y && y <= SLOT_Y + 6.0 * SLOT_H + 5.0 * SLOT_GAP && x >= SLOT_X && x <= SLOT_X + 6.0 * SLOT_W + 5.0 * SLOT_GAP {
            let col = ((x - SLOT_X) / (SLOT_W + SLOT_GAP)).floor() as usize;
            let row = ((y - SLOT_Y) / (SLOT_H + SLOT_GAP)).floor() as usize;
            if row < 6 && col < 6 {
                ui.slot = row * 6 + col;
                ui.selected = None;
                ui.dragging = false;
            }
            return true;
        }
        if x >= GRAPH_X && x <= GRAPH_X + GRAPH_W && y >= GRAPH_Y && y <= GRAPH_Y + GRAPH_H {
            let raw = self.raw();
            let slot = Self::slot_index(&ui);
            let (beat, level) = Self::graph_point(x, y);
            let points = self.points(&raw, slot);
            let nearest = points.iter().enumerate().map(|(i, point)| {
                let (px, py) = Self::graph_xy(*point);
                (i, (px - x).hypot(py - y))
            }).filter(|(_, distance)| *distance < 14.0).min_by(|a,b| a.1.total_cmp(&b.1));
            if let Some((index, _)) = nearest {
                ui.selected = Some(index);
                ui.dragging = true;
            } else if let Some(index) = self.edit(1, slot, 0, beat, level, 2, 0.0) {
                ui.selected = Some(index);
                ui.dragging = true;
            }
            return true;
        }
        if y >= 480.0 && y <= 510.0 {
            let slot = Self::slot_index(&ui);
            if x >= 340.0 && x <= 530.0 {
                self.cycle_curve(&ui);
                return true;
            }
            if x >= 538.0 && x <= 622.0 {
                let raw = self.raw();
                let count = self.point_count(&raw, slot);
                let beat = if let Some(p) = ui.selected.and_then(|i| self.point(&raw, slot, i)) { (p.x + 0.25).min(3.9) } else { 2.0 };
                let level = if count > 0 {
                    unsafe { lynx_gb_state_level(raw.as_ptr(), raw.len(), slot, beat) }
                } else { 1.0 };
                if let Some(index) = self.edit(1, slot, 0, beat, level, 2, 0.0) {
                    ui.selected = Some(index);
                }
                return true;
            }
            if x >= 628.0 && x <= 718.0 {
                if let Some(index) = ui.selected {
                    let _ = self.edit(2, slot, index, 0.0, 1.0, 2, 0.0);
                    ui.selected = None;
                }
                return true;
            }
            if x >= 918.0 && x <= 1080.0 {
                let raw = self.raw();
                let enabled = self.midi_enabled(&raw);
                let _ = self.set_midi(!enabled);
                return true;
            }
        }
        false
    }

    fn moved(&self, x: f32, y: f32) -> bool {
        let mut ui = self.ui.lock().unwrap_or_else(|e| e.into_inner());
        if !ui.dragging { return false; }
        if let Some(index) = ui.selected {
            if x >= GRAPH_X && x <= GRAPH_X + GRAPH_W && y >= GRAPH_Y && y <= GRAPH_Y + GRAPH_H {
                let (beat, level) = Self::graph_point(x, y);
                let raw = self.raw();
                let slot = Self::slot_index(&ui);
                if let Some(point) = self.point(&raw, slot, index) {
                    let _ = self.edit(0, slot, index, beat, level, point.kind, point.tension);
                }
            }
        }
        true
    }

    fn wheel(&self, x: f32, y: f32, notches: f32) -> bool {
        if !(x >= GRAPH_X && x <= GRAPH_X + GRAPH_W && y >= GRAPH_Y && y <= GRAPH_Y + GRAPH_H) {
            return false;
        }
        let ui = self.ui.lock().unwrap_or_else(|e| e.into_inner());
        let Some(index) = ui.selected else { return true };
        let raw = self.raw();
        let slot = Self::slot_index(&ui);
        if let Some(point) = self.point(&raw, slot, index) {
            let tension = (point.tension + notches.signum() * 0.05).clamp(-1.0, 1.0);
            let _ = self.edit(3, slot, index, point.x, point.y, point.kind, tension);
        }
        true
    }

    fn dragging(&self) -> bool {
        self.ui.lock().map(|ui| ui.dragging).unwrap_or(false)
    }

    fn release(&self, _context: Option<&Arc<dyn GuiContext>>) -> bool {
        let mut ui = self.ui.lock().unwrap_or_else(|e| e.into_inner());
        let was_dragging = ui.dragging;
        ui.dragging = false;
        was_dragging
    }
}

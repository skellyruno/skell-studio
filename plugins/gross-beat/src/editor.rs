// Copyright (C) 2026 Skelly Audio
// UI layout and 4x9 slot-grid behavior adapted from LYNX SHARE 0.1.30,
// Copyright (C) 2026 ItsAlphaNeon, AGPL-3.0-only.
// SPDX-License-Identifier: AGPL-3.0-only

//! Skell Beat's native 72-slot envelope editor.
//! State parsing and serialization happen on the editor thread. The C++ bridge hands prepared
//! snapshots to the audio thread, where GrossBeat::Engine::update() uses its pre-reserved slots.

use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use nih_plug::prelude::{GuiContext, Param};
use lemon_small_fx::editor::{self, Extension};
use lemon_small_fx::params::FxParams;
use lemon_small_fx::view::{self, ViewState};
use os_theme::design::lemon as brand;
use os_ui::{Canvas, Color, Font, Rect};

unsafe extern "C" {
    fn lynx_gb_state_point_count(data: *const u8, size: usize, slot: i32) -> i32;
    fn lynx_gb_state_get_point(data: *const u8, size: usize, slot: i32, index: i32,
        x: *mut f64, y: *mut f32, tension: *mut f32, kind: *mut i32) -> bool;
    fn lynx_gb_state_render_slot(data: *const u8, size: usize, slot: i32, out: *mut f32, count: i32) -> i32;
    fn lynx_gb_state_level(data: *const u8, size: usize, slot: i32, beat: f64) -> f32;
    fn lynx_gb_state_midi_trigger(data: *const u8, size: usize) -> i32;
    fn lynx_gb_state_thumbnails(data: *const u8, size: usize,
        out: *mut f32, samples: i32, names: *mut u8, name_bytes: i32) -> i32;
    fn lynx_gb_state_edit(data: *const u8, size: usize, operation: i32, slot: i32, index: i32,
        x: f64, y: f32, kind: i32, tension: f32, out: *mut u8, capacity: i32, result_index: *mut i32) -> i32;
    fn lynx_gb_state_set_midi_trigger(data: *const u8, size: usize, enabled: bool,
        out: *mut u8, capacity: i32) -> i32;
    fn lynx_gb_queue_state(instance: *mut c_void, data: *const u8, size: usize) -> bool;
}

// Layout follows LYNX SHARE 0.1.30 src/core/PluginLayout.cpp::grossBeat:
// two independent 4x9 slot grids, a right-hand envelope and a snap toolbar.
// Coordinates are adapted to Skell's retained native Canvas/host controls.
const LEFT_X: f32 = 8.0;
const LEFT_Y: f32 = 130.0;
const LEFT_W: f32 = 308.0;
const PANEL_H: f32 = 490.0;
const GRAPH_X: f32 = 412.0;
const GRAPH_Y: f32 = 90.0;
const GRAPH_W: f32 = 668.0;
const GRAPH_H: f32 = 443.0;
const SLOT_X: f32 = 16.0;
const SLOT_W: f32 = 70.0;
const SLOT_H: f32 = 18.0;
const SLOT_GAP: f32 = 4.0;
const TIME_GRID_Y: f32 = 163.0;
const VOLUME_GRID_Y: f32 = 388.0;
const THUMB_SAMPLES: usize = 16;
const NAME_BYTES: usize = 32;
const SNAP_LABELS: [&str; 6] = ["OFF", "1/16", "1/8", "1/4", "1/2", "1 BEAT"];
const SNAP_BEATS: [f64; 6] = [0.0, 0.0625, 0.125, 0.25, 0.5, 1.0];

fn slot_hit(x: f32, y: f32) -> Option<(usize, usize)> {
    for (kind, top) in [TIME_GRID_Y, VOLUME_GRID_Y].into_iter().enumerate() {
        let col = ((x - SLOT_X) / (SLOT_W + SLOT_GAP)).floor() as isize;
        let row = ((y - top) / (SLOT_H + SLOT_GAP)).floor() as isize;
        if (0..4).contains(&col) && (0..9).contains(&row) {
            let cell_x = SLOT_X + col as f32 * (SLOT_W + SLOT_GAP);
            let cell_y = top + row as f32 * (SLOT_H + SLOT_GAP);
            if x >= cell_x && x <= cell_x + SLOT_W && y >= cell_y && y <= cell_y + SLOT_H {
                return Some((kind, row as usize * 4 + col as usize));
            }
        }
    }
    None
}

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

struct UiState {
    kind: usize,
    slot: usize,
    snap: usize,
    selected: Option<usize>,
    dragging: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self { kind: 0, slot: 0, snap: 2, selected: None, dragging: false }
    }
}

// One LYNX parser pass per changed raw state, not 72 parses per UI frame.
struct TileCache {
    version: u64,
    ready: bool,
    levels: [f32; 72 * THUMB_SAMPLES],
    titles: [u8; 72 * NAME_BYTES],
}
impl Default for TileCache {
    fn default() -> Self {
        Self { version: 0, ready: false, levels: [0.0; 72 * THUMB_SAMPLES],
            titles: [0; 72 * NAME_BYTES] }
    }
}

pub struct BeatEditorExtension {
    params: Arc<FxParams>,
    engine: AtomicUsize,
    ui: Mutex<UiState>,
    tiles: Mutex<TileCache>,
}

impl BeatEditorExtension {
    pub fn new(params: Arc<FxParams>) -> Self {
        Self { params, engine: AtomicUsize::new(0), ui: Mutex::new(UiState::default()),
            tiles: Mutex::new(TileCache::default()) }
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

    fn graph_point(x: f32, y: f32, snap: usize) -> (f64, f32) {
        let beat = ((x - GRAPH_X) / GRAPH_W).clamp(0.0, 1.0) as f64 * 4.0;
        let step = SNAP_BEATS[snap.min(SNAP_BEATS.len() - 1)];
        let beat = if step > 0.0 { (beat / step).round() * step } else { beat };
        let level = (1.0 - (y - GRAPH_Y) / GRAPH_H).clamp(0.0, 1.0);
        (beat.clamp(0.0, 4.0), level)
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

    /// Slot selection also changes the existing automatable VST3 parameter.
    /// No parameter ID or stored-state encoding changes.
    fn select_slot(&self, kind: usize, slot: usize, context: Option<&Arc<dyn GuiContext>>) {
        let param = &self.params.p[kind];
        let norm = param.preview_normalized(slot as i32);
        self.params.live.set(param.as_ptr(), norm);
        if let Some(context) = context {
            unsafe {
                context.raw_begin_set_parameter(param.as_ptr());
                context.raw_set_parameter_normalized(param.as_ptr(), norm);
                context.raw_end_set_parameter(param.as_ptr());
            }
        }
    }

    fn slot_name(titles: &[u8; 72 * NAME_BYTES], slot: usize) -> String {
        let bytes = &titles[slot * NAME_BYTES..(slot + 1) * NAME_BYTES];
        let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        let label = String::from_utf8_lossy(&bytes[..end]);
        if label.trim().is_empty() { format!("{:02}", slot % 36 + 1) }
        else { label.into_owned() }
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
        let mut tiles = self.tiles.lock().unwrap_or_else(|e| e.into_inner());
        let version = self.params.raw_version.load(Ordering::Acquire);
        if !tiles.ready || tiles.version != version {
            // Editor thread only; the audio callback uses its own pre-parsed state.
            let TileCache { levels, titles, .. } = &mut *tiles;
            let count = unsafe { lynx_gb_state_thumbnails(raw.as_ptr(), raw.len(),
                levels.as_mut_ptr(), THUMB_SAMPLES as i32, titles.as_mut_ptr(), NAME_BYTES as i32) };
            tiles.ready = count == 72;
            tiles.version = version;
        }
        let time_color = brand::MAIN;
        let volume_color = Color::rgb(0x61c9e7);
        let accent = if ui.kind == 0 { time_color } else { volume_color };
        let body = Color::rgb(0x0c1418);
        let tile = Color::rgb(0x19252b);
        let inactive = Color::rgb(0x26333a);
        let muted = Color::rgb(0x83979d);
        let text = brand::TEXT;
        let grid_line = Color::rgb(0x29404b);
        let dark = Color::rgb(0x091014);
        c.fill_rrect(Rect::new(LEFT_X, LEFT_Y, LEFT_W, PANEL_H), 7.0, body);
        c.fill_rrect(Rect::new(335.0, 20.0, 757.0, 600.0), 7.0, body);
        c.stroke_rrect(Rect::new(335.0, 20.0, 757.0, 600.0), 7.0, 1.0, inactive);
        Self::draw_text(c, 19.0, 139.0, time_color, "TIME SLOTS  /  36");
        Self::draw_text(c, 19.0, 364.0, volume_color, "VOLUME SLOTS  /  36");
        Self::draw_text(c, 348.0, 35.0, muted, "LYNX ENVELOPE ENGINE");
        // LYNX's two independent 4x9 grids (not a single tabbed 6x6 matrix).
        for kind in 0..2 {
            let top = if kind == 0 { TIME_GRID_Y } else { VOLUME_GRID_Y };
            let color = if kind == 0 { time_color } else { volume_color };
            let live_slot = self.params.v(kind).clamp(0, 35) as usize;
            for index in 0..36 {
                let x = SLOT_X + (index % 4) as f32 * (SLOT_W + SLOT_GAP);
                let y = top + (index / 4) as f32 * (SLOT_H + SLOT_GAP);
                let active = index == live_slot;
                let editing = ui.kind == kind && ui.slot == index;
                let rect = Rect::new(x, y, SLOT_W, SLOT_H);
                c.fill_rrect(rect, 2.0, if editing { Color::rgb(0x304449) } else if active { tile } else { dark });
                let source = kind * 36 + index;
                if tiles.ready {
                    let mut path = [(0.0f32, 0.0f32); THUMB_SAMPLES];
                    let values = &tiles.levels[source * THUMB_SAMPLES..(source + 1) * THUMB_SAMPLES];
                    if values.iter().any(|v| (v - values[0]).abs() > 0.0001) {
                        for (j, level) in values.iter().enumerate() {
                            path[j] = (x + 2.0 + j as f32 / (THUMB_SAMPLES - 1) as f32 * (SLOT_W - 4.0),
                                // Keep the mini envelope below the slot title, never behind text.
                                y + 12.0 + (1.0 - level.clamp(0.0, 1.0)) * 4.0);
                        }
                        c.polyline(&path, 1.0, if editing { color } else { inactive });
                    }
                }
                if active || editing { c.stroke_rrect(rect, 2.0, if editing { 1.5 } else { 1.0 }, color); }
                let name = Self::slot_name(&tiles.titles, source);
                let title: String = name.chars().take(14).collect();
                c.text_clip(Font::new(9).bold(), x + 5.0, y + 1.0, SLOT_W - 10.0,
                    if active || editing { text } else { muted }, &title);
            }
        }
        // Time and Volume tabs, independent of the slot grids.
        for (kind, x) in [(0, 415.0), (1, 526.0)] {
            let on = ui.kind == kind;
            let color = if kind == 0 { time_color } else { volume_color };
            c.fill_rrect(Rect::new(x, 42.0, 104.0, 30.0), 4.0, if on { Color::rgb(0x2b3b40) } else { tile });
            if on { c.line(x + 3.0, 70.0, x + 101.0, 70.0, 2.0, color); }
            Self::draw_text(c, x + 25.0, 51.0, if on { text } else { muted },
                if kind == 0 { "TIME" } else { "VOLUME" });
        }
        let name = Self::slot_name(&tiles.titles, ui.kind * 36 + ui.slot);
        c.text_clip(Font::new(12).bold(), 643.0, 51.0, 425.0, accent,
            &format!("{} SLOT {:02}  /  {}", if ui.kind == 0 { "TIME" } else { "VOLUME" }, ui.slot + 1,
                if name == format!("{:02}", ui.slot + 1) { "UNTITLED" } else { &name }));
        // Narrow FL/LYNX-style envelope tool column with a real snap selector.
        c.fill_rrect(Rect::new(344.0, 90.0, 58.0, 443.0), 3.0, dark);
        Self::draw_text(c, 350.0, 104.0, muted, "SNAP");
        c.fill_rrect(Rect::new(347.0, 122.0, 52.0, 28.0), 3.0, Color::rgb(0x263a3c));
        c.text_clip(Font::new(10).bold(), 352.0, 130.0, 45.0, text, SNAP_LABELS[ui.snap]);
        Self::draw_text(c, 353.0, 172.0, muted, "EDIT");
        Self::draw_text(c, 351.0, 190.0, accent, "POINT");
        // The actual curve workspace: LYNX's beat lines, safety line and 8 level divisions.
        c.fill_rrect(Rect::new(GRAPH_X, GRAPH_Y, GRAPH_W, GRAPH_H), 3.0, dark);
        let snap = SNAP_BEATS[ui.snap];
        if snap > 0.0 {
            let mut beat = snap;
            while beat < 4.0 - 1e-8 {
                let x = GRAPH_X + (beat as f32 / 4.0) * GRAPH_W;
                c.line(x, GRAPH_Y, x, GRAPH_Y + GRAPH_H, 0.6, grid_line);
                beat += snap;
            }
        }
        for b in 0..=4 {
            let x = GRAPH_X + GRAPH_W * b as f32 / 4.0;
            c.line(x, GRAPH_Y, x, GRAPH_Y + GRAPH_H, 1.0, Color::rgb(0x35515b));
            if b < 4 { c.text(Font::new(10), x + 5.0, GRAPH_Y + GRAPH_H + 3.0, muted, &b.to_string()); }
        }
        for i in 0..=8 {
            let y = GRAPH_Y + GRAPH_H * i as f32 / 8.0;
            c.line(GRAPH_X, y, GRAPH_X + GRAPH_W, y, 0.7, grid_line);
        }
        if ui.kind == 0 { c.line(GRAPH_X, GRAPH_Y, GRAPH_X + GRAPH_W, GRAPH_Y + GRAPH_H * 0.5, 0.8, inactive); }
        let mut curve = [0.0f32; 256];
        let rendered = unsafe { lynx_gb_state_render_slot(raw.as_ptr(), raw.len(), slot,
            curve.as_mut_ptr(), curve.len() as i32) };
        if rendered == curve.len() as i32 {
            let mut path = [(0.0f32, 0.0f32); 256];
            for (i, level) in curve.iter().enumerate() {
                path[i] = (GRAPH_X + i as f32 / (curve.len() - 1) as f32 * GRAPH_W,
                    GRAPH_Y + (1.0 - level.clamp(0.0, 1.0)) * GRAPH_H);
            }
            c.polyline(&path, 2.0, accent);
        }
        for (i, point) in points.iter().copied().enumerate() {
            let (x, y) = Self::graph_xy(point);
            c.fill_circle(x, y, if ui.selected == Some(i) { 7.0 } else { 4.0 },
                if ui.selected == Some(i) { text } else { accent });
            c.fill_circle(x, y, 2.0, dark);
        }
        Self::draw_text(c, 415.0, 555.0, accent,
            if ui.kind == 0 { "TIME TRAVEL" } else { "VOLUME ENVELOPE" });
        let selected = ui.selected.and_then(|i| points.get(i).copied());
        let shape = selected.map_or("SELECT POINT".to_string(), |p|
            format!("{} / TENSION {:+.2}", CURVE_NAMES[p.kind.clamp(0, 12) as usize], p.tension));
        c.fill_rrect(Rect::new(414.0, 583.0, 190.0, 28.0), 4.0, tile);
        c.text_clip(Font::new(10).bold(), 422.0, 592.0, 177.0, text, &shape);
        c.fill_rrect(Rect::new(612.0, 583.0, 86.0, 28.0), 4.0, tile);
        Self::draw_text(c, 637.0, 592.0, accent, "ADD");
        c.fill_rrect(Rect::new(706.0, 583.0, 106.0, 28.0), 4.0, tile);
        Self::draw_text(c, 720.0, 592.0, if selected.is_some() { text } else { muted }, "REMOVE");
        c.fill_rrect(Rect::new(921.0, 583.0, 156.0, 28.0), 4.0,
            if midi { Color::rgb(0x234533) } else { tile });
        Self::draw_text(c, 951.0, 592.0, if midi { time_color } else { muted },
            if midi { "MIDI: ON" } else { "MIDI: OFF" });
        if let Some(p) = selected {
            c.text(Font::new(10), 826.0, 555.0, muted,
                &format!("X {:.2}   Y {:.2}", p.x, p.y));
        }
    }

    fn click(&self, x: f32, y: f32, context: Option<&Arc<dyn GuiContext>>) -> bool {
        if let Some((kind, index)) = slot_hit(x, y) {
            let mut ui = self.ui.lock().unwrap_or_else(|e| e.into_inner());
            ui.kind = kind;
            ui.slot = index;
            ui.selected = None;
            ui.dragging = false;
            self.select_slot(kind, index, context);
            return true;
        }
        if y >= 42.0 && y <= 72.0 && x >= 415.0 && x <= 630.0 {
            let mut ui = self.ui.lock().unwrap_or_else(|e| e.into_inner());
            ui.kind = if x < 523.0 { 0 } else { 1 };
            ui.slot = self.params.v(ui.kind).clamp(0, 35) as usize;
            ui.selected = None;
            ui.dragging = false;
            return true;
        }
        if x >= 347.0 && x <= 399.0 && y >= 122.0 && y <= 150.0 {
            let mut ui = self.ui.lock().unwrap_or_else(|e| e.into_inner());
            ui.snap = (ui.snap + 1) % SNAP_LABELS.len();
            return true;
        }
        if x >= GRAPH_X && x <= GRAPH_X + GRAPH_W && y >= GRAPH_Y && y <= GRAPH_Y + GRAPH_H {
            let mut ui = self.ui.lock().unwrap_or_else(|e| e.into_inner());
            let raw = self.raw();
            let slot = Self::slot_index(&ui);
            let (beat, level) = Self::graph_point(x, y, ui.snap);
            let points = self.points(&raw, slot);
            let nearest = points.iter().enumerate().map(|(i, point)| {
                let (px, py) = Self::graph_xy(*point);
                (i, (px - x).hypot(py - y))
            }).filter(|(_, distance)| *distance < 14.0).min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((index, _)) = nearest {
                ui.selected = Some(index);
                ui.dragging = true;
            } else if let Some(index) = self.edit(1, slot, 0, beat, level, 2, 0.0) {
                ui.selected = Some(index);
                ui.dragging = true;
            }
            return true;
        }
        if y >= 583.0 && y <= 613.0 {
            let mut ui = self.ui.lock().unwrap_or_else(|e| e.into_inner());
            let slot = Self::slot_index(&ui);
            if x >= 414.0 && x <= 604.0 { self.cycle_curve(&ui); return true; }
            if x >= 612.0 && x <= 698.0 {
                let raw = self.raw();
                let beat = ui.selected.and_then(|i| self.point(&raw, slot, i))
                    .map_or(2.0, |p| (p.x + 0.25).min(3.9));
                let level = unsafe { lynx_gb_state_level(raw.as_ptr(), raw.len(), slot, beat) };
                if let Some(index) = self.edit(1, slot, 0, beat, level, 2, 0.0) { ui.selected = Some(index); }
                return true;
            }
            if x >= 706.0 && x <= 812.0 {
                if let Some(index) = ui.selected {
                    let raw = self.raw();
                    let count = self.point_count(&raw, slot);
                    if index > 0 && index + 1 < count {
                        let _ = self.edit(2, slot, index, 0.0, 1.0, 2, 0.0);
                        ui.selected = None;
                    }
                }
                return true;
            }
            if x >= 921.0 && x <= 1077.0 {
                let raw = self.raw();
                let _ = self.set_midi(!self.midi_enabled(&raw));
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
                let (beat, level) = Self::graph_point(x, y, ui.snap);
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

#[cfg(test)]
mod lynx_grid_tests {
    use super::*;
    #[test]
    fn both_four_by_nine_slot_banks_cover_all_72_slots() {
        assert_eq!(slot_hit(SLOT_X + 5.0, TIME_GRID_Y + 5.0), Some((0, 0)));
        assert_eq!(slot_hit(SLOT_X + 3.0 * (SLOT_W + SLOT_GAP) + 5.0,
            TIME_GRID_Y + 8.0 * (SLOT_H + SLOT_GAP) + 5.0), Some((0, 35)));
        assert_eq!(slot_hit(SLOT_X + 5.0, VOLUME_GRID_Y + 5.0), Some((1, 0)));
        assert_eq!(slot_hit(SLOT_X + 3.0 * (SLOT_W + SLOT_GAP) + 5.0,
            VOLUME_GRID_Y + 8.0 * (SLOT_H + SLOT_GAP) + 5.0), Some((1, 35)));
        assert_eq!(slot_hit(9.0, TIME_GRID_Y + 2.0), None);
    }
    #[test]
    fn slot_clicks_change_existing_host_automation_values() {
        let params = Arc::new(FxParams::new(crate::SPEC));
        let editor = BeatEditorExtension::new(Arc::clone(&params));
        assert!(editor.click(SLOT_X + 3.0 * (SLOT_W + SLOT_GAP) + 5.0,
            TIME_GRID_Y + 8.0 * (SLOT_H + SLOT_GAP) + 4.0, None));
        assert_eq!(params.v(0), 35);
        assert!(editor.click(SLOT_X + 5.0, VOLUME_GRID_Y + 6.0, None));
        assert_eq!(params.v(1), 0);
        assert_eq!(SNAP_BEATS[UiState::default().snap], 0.125);
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later

//! The Studio Channel Rack. Project data and editing actions are shared with the vintage view.

use super::*;

const FACE: u32 = 0x232326;
const BODY: u32 = 0x141416;
const LABEL: u32 = 0xa2a2ab;
const INK: u32 = 0xe1e1e7;
const RADIUS: f32 = 5.0;

struct Grid {
    area: Rect,
    offset: f32,
    count: u32,
    beat: u32,
}

impl Grid {
    fn first(&self) -> u32 {
        (self.offset / STEP_W).floor() as u32
    }

    fn x(&self, step: u32) -> f32 {
        self.area.x + step as f32 * STEP_W - self.offset
    }

    fn steps(&self) -> std::ops::Range<u32> {
        self.first()..self.first() + self.count + 1
    }

    fn alternate(&self, step: u32) -> bool {
        (step / self.beat) % 2 == 1
    }
}

pub(super) fn paint(ui: &mut Ui, s: &State, frame: &FrameInfo) {
    let window = rect(s);
    let columns = cols(s);
    let rows = row_view(s);
    let count = visible_steps(s);
    let grid = Grid {
        area: Rect::new(
            window.x + columns.step_x,
            rows.y,
            count as f32 * STEP_W,
            rows.h,
        ),
        offset: sx(s),
        count,
        beat: rk::beat_steps(s).max(1),
    };
    let length = rk::pattern_steps(s);
    let per_step = rk::per_step(s).max(1);
    let play_step = (frame.playing && !frame.song)
        .then(|| (frame.tick / f64::from(per_step)).floor() as u32 % length.max(1));
    let notes = s
        .project
        .pattern(s.pattern)
        .map_or(&[][..], |p| p.notes.as_slice());
    let bars = rk::pattern_bars(s, length * per_step);

    ui.c.fill_rrect(window, 10.0, ui.t.s(BODY));
    ui.c.stroke_rrect(window.inset(0.5), 10.0, 1.0, ui.t.s(0x28282b));
    ui.hit(window, Action::RackArea, "Channel rack");
    header(ui, s, window, frame);
    ruler(ui, window, columns, &grid);

    let scroll_y = scroll(s);
    ui.c.push_clip(rows);
    ui.clip_hit_rect(rows, |ui| {
        for row in layout(s) {
            let y = rows.y + row.top - scroll_y - 2.0;
            if y + row.pitch() > rows.y && y < rows.bottom() {
                controls(ui, s, window.x, columns, &row, y, frame);
                if !row.zipped {
                    sequence(ui, s, row.row, y, &grid, notes, &bars, per_step, play_step);
                }
            }
        }
        if let (Some(lane), Some(graph)) = (graph_rect(s), graph_bars(s)) {
            if lane.intersect(&rows).h > 0.0 {
                graph_lane(ui, s, lane, graph, &grid, per_step);
            }
        }
    });
    draw_row_drag(ui, rows, frame);
    ui.c.pop_clip();

    let height = content_h(s);
    if height > rows.h {
        let track = Rect::new(rows.right() + 4.0, rows.y, 6.0, rows.h);
        super::super::scrollbar(ui, track, true, scroll(s), rows.h, height, Scroll::RackY);
    }
    let add = Rect::new(
        window.x + columns.name_x,
        window.bottom() - 39.0,
        columns.name_w,
        27.0,
    );
    let hot = face(ui, add, Action::RackAdd, "Add a channel", false);
    glyph(ui, Icon::Plus, add, 15.0, text(ui, hot));
    let track = Rect::new(grid.area.x, window.bottom() - 15.0, grid.area.w, 6.0);
    let total = (length.max(count) + 256) as f32 * STEP_W;
    super::super::scrollbar(
        ui,
        track,
        false,
        grid.offset,
        grid.area.w,
        total,
        Scroll::RackX,
    );

    let (right, bottom) = (window.right(), window.bottom());
    for (region, edges) in [
        (
            Rect::span(
                right - 3.0,
                right + 4.0,
                window.y + STUDIO_TITLE_H,
                bottom - 8.0,
            ),
            EDGE_RIGHT,
        ),
        (
            Rect::span(window.x, right - 8.0, bottom - 3.0, bottom + 4.0),
            EDGE_BOTTOM,
        ),
        (
            Rect::span(right - 10.0, right + 4.0, bottom - 10.0, bottom + 4.0),
            EDGE_RIGHT | EDGE_BOTTOM,
        ),
    ] {
        ui.hit(region, Action::RackResize(edges), "Drag to resize");
    }
}

fn face(ui: &mut Ui, rect: Rect, action: Action, hint: &str, active: bool) -> bool {
    let hot = ui.hit(rect, action, hint.to_owned());
    let color = if active { ui.t.accent } else { ui.t.s(FACE) };
    ui.c.fill_rrect(rect, RADIUS, if hot { color.shade(0.10) } else { color });
    hot
}

fn text(ui: &Ui, bright: bool) -> Color {
    ui.t.tx(if bright || crate::high_visibility::on() {
        INK
    } else {
        LABEL
    })
}

fn glyph(ui: &mut Ui, icon: Icon, rect: Rect, size: f32, color: Color) {
    icons::draw(
        ui.c,
        icon,
        Rect::new(rect.cx() - size / 2.0, rect.cy() - size / 2.0, size, size),
        color,
    );
}

fn label(ui: &mut Ui, rect: Rect, label: &str, color: Color, left: bool) {
    let font = Font::light(13, 0).natural();
    if left {
        let y = (rect.cy() - ui.c.line_height(font) / 2.0).round();
        ui.c.text_clip(
            font,
            rect.x + 10.0,
            y,
            (rect.w - 15.0).max(0.0),
            color,
            label,
        );
    } else {
        ui.c.push_clip(rect);
        ui.c.text_center(font, rect, color, label);
        ui.c.pop_clip();
    }
}

fn header(ui: &mut Ui, s: &State, window: Rect, frame: &FrameInfo) {
    let top = Rect::new(
        window.x + 1.0,
        window.y + 1.0,
        window.w - 2.0,
        STUDIO_TITLE_H,
    );
    ui.c.push_clip(top);
    ui.c.fill_rrect(
        Rect::new(top.x, top.y, top.w, top.h + 10.0),
        9.0,
        ui.t.s(0x1a1a1c),
    );
    ui.c.pop_clip();
    ui.c.line(
        top.x,
        top.bottom(),
        top.right(),
        top.bottom(),
        1.0,
        ui.t.s(0x28282b),
    );
    ui.hit(
        top,
        Action::WinDrag(Win::ChannelRack),
        "Channel rack - drag to move",
    );

    let left = |x, w| Rect::new(window.x + x, window.y + 12.0, w, 32.0);
    let right = |x, w| Rect::new(window.right() - x, window.y + 12.0, w, 32.0);
    let playing = frame.playing && !frame.paused && !frame.song;
    for (rect, icon, action, hint, active, shown) in [
        (
            left(14.0, 27.0),
            Icon::Gear,
            Action::RackOptions,
            "Channel options",
            false,
            true,
        ),
        (
            left(47.0, 28.0),
            Icon::Repeat,
            Action::RackLoop,
            "Loop mode",
            s.rack.loop_mode,
            crate::workspace_visibility::shown("visible_rack-loop"),
        ),
        (
            left(78.0, 28.0),
            if playing { Icon::Stop } else { Icon::Play },
            Action::RackSpeaker,
            if playing { "Stop pattern" } else { "Play pattern" },
            playing,
            true,
        ),
        (
            right(95.0, 26.0),
            Icon::Mixer,
            Action::RackGraph,
            "Graph editor (Ctrl+K)",
            s.rack.graph_open,
            crate::workspace_visibility::shown("visible_rack-graph"),
        ),
        (
            right(65.0, 26.0),
            Icon::ChannelRack,
            Action::RackRollView,
            "Step sequencer / piano roll overview",
            !s.rack_roll_view,
            true,
        ),
        (
            right(34.0, 24.0),
            Icon::Close,
            Action::CloseWin(Win::ChannelRack),
            "Close",
            false,
            true,
        ),
    ] {
        if shown {
            let hot = ui.hit(rect, action.clone(), hint);
            if hot {
                ui.c.fill_rrect(rect, RADIUS, ui.t.s(FACE));
            }
            if action == Action::RackOptions {
                for offset in [-5.0, 0.0, 5.0] {
                    ui.c.fill_circle(rect.cx() + offset, rect.cy(), 1.5, text(ui, hot));
                }
            } else {
                glyph(
                    ui,
                    icon,
                    rect,
                    15.0,
                    if active { ui.t.accent } else { text(ui, hot) },
                );
            }
        }
    }

    let gap = ((window.w - 750.0) * 0.20).clamp(0.0, 56.0);
    label(
        ui,
        left(108.0 + gap, 70.0),
        "Global pitch",
        text(ui, false),
        false,
    );
    let follow = left(183.0 + gap, 28.0);
    let following = crate::rack_controls::all_follow(s);
    face(
        ui,
        follow,
        Action::RackPitchFollow(usize::MAX),
        "Global Pitch Follow for all channels: preserve current pitch",
        following,
    );
    label(
        ui,
        follow,
        "G",
        if following {
            Color::rgb(0x101820)
        } else {
            text(ui, true)
        },
        false,
    );
    let pitch = left(218.0 + gap, 91.0);
    face(
        ui,
        pitch,
        Action::Knob(KnobId::MasterPitch),
        &s.knob_text(KnobId::MasterPitch),
        false,
    );
    label(
        ui,
        pitch,
        &format!(
            "{:+.2} st",
            s.project.main_pitch.unwrap_or(0) as f32 / 100.0
        ),
        text(ui, true),
        false,
    );

    let filter = left(317.0 + gap, 101.0);
    let group = match s.rack.filter {
        rk::Filter::All => "All",
        rk::Filter::Unsorted => "Unsorted",
        rk::Filter::Group(g) => s.rack.groups.get(g).map_or("All", |g| g.name.as_str()),
    };
    face(
        ui,
        filter,
        Action::RackFilter,
        "Channel display filter (PgUp/PgDn)",
        false,
    );
    label(ui, filter, group, text(ui, true), true);
    let pattern = left(425.0 + gap, 102.0);
    let name = crate::patterns::name_of(s, s.pattern.max(1));
    face(
        ui,
        pattern,
        Action::PatternNext,
        "Pattern selector: drag or scroll",
        false,
    );
    let menu = Rect::new(pattern.right() - 19.0, pattern.y, 19.0, pattern.h);
    let hot = ui.hit(menu, Action::PatternPrev, "Pattern options");
    label(
        ui,
        Rect::new(pattern.x, pattern.y, pattern.w - menu.w, pattern.h),
        &name,
        text(ui, true),
        true,
    );
    glyph(ui, Icon::ArrowDown, menu, 10.0, text(ui, hot));

    let captions_fit = pattern.right() + 7.0 <= window.right() - 260.0;
    if captions_fit {
        label(ui, right(260.0, 42.0), "Swing", text(ui, false), false);
        label(ui, right(166.0, 37.0), "Steps", text(ui, false), false);
    }
    let swing = right(if captions_fit { 215.0 } else { 175.0 }, 44.0);
    dial(ui, s, swing.cx(), swing.cy(), KnobId::Swing, false);
    let steps = right(128.0, 31.0);
    face(
        ui,
        steps,
        Action::RackLen,
        "Pattern length: drag or right-click",
        false,
    );
    label(
        ui,
        steps,
        &rk::explicit_len(s).map_or("---".into(), |n| n.to_string()),
        text(ui, true),
        false,
    );
}

fn ruler(ui: &mut Ui, window: Rect, columns: Cols, grid: &Grid) {
    let y = window.y + STUDIO_TITLE_H + 1.0;
    let name = Rect::new(
        window.x + columns.name_x,
        y,
        columns.name_w,
        STUDIO_RULER_H - 2.0,
    );
    label(ui, name, "Tracks", text(ui, false), false);
    label(
        ui,
        Rect::new(name.right() + 40.0, y, 34.0, name.h),
        "Pitch",
        text(ui, false),
        false,
    );
    let grip = Rect::new(name.right() - 3.0, y + 2.0, 9.0, name.h - 4.0);
    let hot = ui.hit(grip, Action::RackDots, "Drag to resize channel buttons");
    for index in 0..3 {
        ui.c.fill_circle(
            grip.cx(),
            grip.cy() + index as f32 * 4.0 - 4.0,
            0.85,
            text(ui, hot),
        );
    }
    let bounds = Rect::new(grid.area.x, y, grid.area.w, name.h);
    ui.c.push_clip(bounds);
    for step in grid.steps() {
        let rect = Rect::new(grid.x(step), y, STEP_W - 2.0, name.h);
        label(ui, rect, &(step + 1).to_string(), text(ui, false), false);
    }
    ui.c.pop_clip();
}

fn controls(
    ui: &mut Ui,
    s: &State,
    x: f32,
    columns: Cols,
    row: &RowPos,
    y: f32,
    frame: &FrameInfo,
) {
    let index = row.row;
    let channel = &s.project.channels[index];
    let name = Rect::new(x + columns.name_x, y, columns.name_w, ROW_H - 2.0);
    let selected = s.rack.sel.contains(&index);
    let focused = index == s.selected_channel;
    if row.zipped {
        let strip = Rect::new(name.x, y + 2.0, name.w, rk::ZIP_H - 4.0);
        face(
            ui,
            strip,
            Action::ChannelSelect(index),
            "Zipped channel - Alt+U unzips all",
            selected,
        );
        if !selected {
            if let Some(color) = channel.color {
                ui.c.fill_rrect(strip, 2.0, rgb(color).mix(ui.t.s(FACE), 0.45));
            }
        }
        super::midi_glow(ui, s, channel, strip);
        return;
    }

    let select = Rect::new(x + columns.slot_x - 10.0, y, 22.0, name.h);
    let hot = ui.hit(
        select,
        Action::RackSelect(index),
        "Graph editor for this channel (Shift to select; Shift-double-click to select all)",
    );
    ui.live(Live::Sounding, select);
    let sounding = channel.enabled && frame.sounding.iter().any(|&(n, _)| n == channel.index);
    let graph_open = s.rack.graph_open && focused;
    glyph(
        ui,
        if graph_open {
            Icon::ArrowUp
        } else {
            Icon::ArrowDown
        },
        select,
        11.0,
        if sounding || selected || graph_open {
            ui.t.accent
        } else {
            text(ui, hot)
        },
    );
    for (center, knob, bipolar) in [
        (x + 55.0, KnobId::ChanPan(index), true),
        (x + 83.0, KnobId::ChanVol(index), false),
        (name.right() + 57.0, KnobId::ChanPitch(index), true),
    ] {
        dial(ui, s, center, name.cy(), knob, bipolar);
    }

    let hot = ui.hit(
        name,
        Action::ChannelSelect(index),
        "Click: plug-in window; drag: select channels; Ctrl+drag: move rows",
    );
    let base = ui.t.s(FACE);
    let base = channel
        .color
        .map_or(base, |color| base.mix(rgb(color), 0.27));
    ui.c.fill_rrect(name, RADIUS, if hot { base.shade(0.10) } else { base });
    if focused || selected {
        ui.c.stroke_rrect(name.inset(0.5), RADIUS, 1.0, ui.t.accent);
    }
    super::midi_glow(ui, s, channel, name);
    let route_w = if s.rack.show_mixer { 24.0 } else { 0.0 };
    let label_rect = Rect::new(name.x, name.y, name.w - route_w, name.h);
    let name_text = channel
        .name
        .clone()
        .unwrap_or_else(|| format!("Channel {}", channel.index + 1));
    let name_text = shorten(
        ui,
        Font::light(13, 0).natural(),
        &name_text,
        (label_rect.w - 15.0).max(0.0),
    );
    label(
        ui,
        label_rect,
        &name_text,
        text(ui, focused || hot),
        s.rack.align_left.unwrap_or(true),
    );
    if s.rack.show_mixer {
        let route = Rect::new(name.right() - 22.0, name.y + 8.0, 19.0, name.h - 16.0);
        let hot = face(
            ui,
            route,
            Action::RackMixer(index),
            &format!(
                "Target mixer track: {} (click to choose; drag to mixer; wheel to change)",
                insert_name(channel.mixer_track)
            ),
            false,
        );
        if let Some(color) = channel.color.filter(|_| s.rack.colorful) {
            let fill = ui.t.s(FACE).mix(rgb(color), if hot { 0.55 } else { 0.4 });
            ui.c.fill_rrect(route, RADIUS, fill);
        }
        let number = if channel.mixer_track == 0 {
            "M".into()
        } else {
            channel.mixer_track.to_string()
        };
        label(ui, route, &number, text(ui, hot), false);
    }

    let follow = Rect::new(name.right() + 6.0, y + 3.0, 28.0, name.h - 6.0);
    face(
        ui,
        follow,
        Action::RackPitchFollow(index),
        "Global Pitch Follow: toggle without changing the current pitch",
        channel.misc.main_pitch,
    );
    label(
        ui,
        follow,
        "G",
        if channel.misc.main_pitch {
            Color::rgb(0x101820)
        } else {
            text(ui, false)
        },
        false,
    );
    let mute = Rect::new(name.right() + 80.0, y, 29.0, name.h);
    let hot = face(
        ui,
        mute,
        Action::ChannelEnable(index),
        "Mute selected channels when this row is selected (Shift: lock, Ctrl/Alt: solo)",
        false,
    );
    if !channel.enabled {
        ui.c.fill_rrect(mute, RADIUS, Color::rgb(0xd6635d));
    }
    if s.rack.locked.contains(&index) {
        padlock(ui, mute.cx(), mute.cy(), channel.enabled);
    } else {
        label(
            ui,
            mute,
            "M",
            if channel.enabled {
                text(ui, hot)
            } else {
                Color::rgb(0x191417)
            },
            false,
        );
    }
    let solo = Rect::new(name.right() + 115.0, y, 29.0, name.h);
    let active = rk::solo_active(s, index);
    let hot = face(ui, solo, Action::RackSolo(index), "Solo selected channels when this row is selected", active);
    label(
        ui,
        solo,
        "S",
        if active {
            Color::rgb(0x101820)
        } else {
            text(ui, hot)
        },
        false,
    );
}

fn dial(ui: &mut Ui, s: &State, x: f32, y: f32, id: KnobId, bipolar: bool) {
    let hot = ui.hit(
        Rect::new(x - 13.0, y - 13.0, 26.0, 26.0),
        Action::Knob(id),
        s.knob_text(id),
    );
    let value = s.knob(id).clamp(0.0, 1.0);
    let start = std::f32::consts::PI * 0.75;
    let sweep = std::f32::consts::PI * 1.5;
    let angle = start + value * sweep;
    let origin = if bipolar { 0.5 } else { 0.0 };
    let stroke = (value - origin).abs() * sweep;
    ui.c.fill_circle(x, y, 9.5, ui.t.s(if hot { 0x343438 } else { FACE }));
    ui.c.stroke_circle(x, y, 9.5, 1.0, ui.t.s(0x46464b));
    if stroke > 0.02 {
        ui.c.arc(
            x,
            y,
            9.5,
            start + origin.min(value) * sweep,
            stroke,
            1.8,
            ui.t.accent,
        );
    }
    ui.c.line(
        x + angle.cos() * 3.4,
        y + angle.sin() * 3.4,
        x + angle.cos() * 7.5,
        y + angle.sin() * 7.5,
        1.6,
        text(ui, true),
    );
}

fn sequence(
    ui: &mut Ui,
    s: &State,
    index: usize,
    y: f32,
    grid: &Grid,
    notes: &[os_model::Note],
    bars: &[(u32, u32)],
    per_step: u32,
    play_step: Option<u32>,
) {
    let channel = &s.project.channels[index];
    let row = Rect::new(grid.area.x, y, grid.area.w, ROW_H - 2.0);
    ui.c.push_clip(row);
    if channel.kind == os_model::ChannelKind::AudioClip && channel.sample_path.is_some() {
        ui.c.fill_rrect(row, 3.0, ui.t.s(0x232326));
        if let Some(wave) = channel
            .sample_path
            .as_deref()
            .and_then(|p| s.playlist.wave(p))
        {
            let width = row.w.max(1.0) as usize;
            for pixel in 0..width {
                let start = pixel * wave.peaks.len() / width;
                let end = ((pixel + 1) * wave.peaks.len() / width)
                    .max(start + 1)
                    .min(wave.peaks.len());
                let (lo, hi) = wave.peaks[start..end]
                    .iter()
                    .fold((0.0f32, 0.0f32), |(lo, hi), &(a, b)| (lo.min(a), hi.max(b)));
                ui.c.line(
                    row.x + pixel as f32,
                    row.cy() - hi * row.h * 0.45,
                    row.x + pixel as f32,
                    row.cy() - lo * row.h * 0.45,
                    1.0,
                    ui.t.accent,
                );
            }
        }
        ui.hit(
            row,
            Action::ChannelSelect(index),
            "Audio waveform: click for sample settings",
        );
    } else if s.rack_roll_view || !plain_steps(notes, channel.index) {
        let end = bars.last().map_or(1, |bar| bar.1).max(1);
        let scale = row.w / end as f32;
        for (i, &(start, end)) in bars.iter().enumerate() {
            let bar = Rect::new(
                row.x + start as f32 * scale,
                row.y,
                (end - start) as f32 * scale,
                row.h,
            );
            ui.c.fill_rect(bar, ui.t.s(if i % 2 == 0 { 0x1b1b1e } else { 0x242427 }));
        }
        let channel_notes: Vec<_> = notes
            .iter()
            .filter(|n| n.channel == channel.index)
            .collect();
        let lowest = channel_notes.iter().map(|n| n.key).min().unwrap_or(60);
        let highest = channel_notes.iter().map(|n| n.key).max().unwrap_or(60);
        let center = f32::from(lowest + highest) / 2.0;
        let pitch_span = f32::from(highest - lowest).max(12.0);
        for note in channel_notes {
            let length = if note.length == 0 {
                per_step
            } else {
                note.length
            };
            let y = row.cy() - (f32::from(note.key) - center) / pitch_span * (row.h - 8.0);
            let mark = Rect::new(
                row.x + note.position as f32 * scale,
                y,
                (length as f32 * scale - 1.0).max(2.0),
                2.0,
            );
            ui.c.fill_rrect(mark, 1.0, ui.t.accent);
        }
        ui.hit(
            row,
            Action::RackRoll(index),
            "Piano roll preview - click to open the Piano roll",
        );
    } else {
        ui.hit(row, Action::RackStepGap, "Drag between steps to select channels");
        let length = rk::row_len(s, index);
        for step in grid.steps() {
            let cell = Rect::new(grid.x(step), y, STEP_W - 2.0, row.h);
            let hit = cell.intersect(&row);
            if hit.w <= 0.0 {
                continue;
            }
            let on = rk::step_on(notes, channel.index, step, per_step);
            let hot = ui.hit(
                hit,
                Action::Step(index, step),
                "Step - click or drag to paint (left / right)",
            );
            let tone = if on {
                0xf2f2f3
            } else if grid.alternate(step) {
                0x303034
            } else {
                0x262629
            };
            let color = if on { Color::rgb(tone) } else { ui.t.s(tone) };
            let color = if hot {
                color.mix(ui.t.accent, 0.30)
            } else if !on && step >= length {
                color.shade(-0.22)
            } else {
                color
            };
            ui.c.fill_rrect(cell, 3.0, color);
            if on {
                let cap = Rect::new(cell.x + 4.0, cell.y + 3.0, cell.w - 8.0, 1.6);
                ui.c.fill_rrect(
                    cap,
                    0.8,
                    if play_step == Some(step) {
                        ui.t.accent
                    } else {
                        Color::rgb(0x98989f)
                    },
                );
            } else if play_step == Some(step) {
                ui.c.stroke_rrect(cell.inset(0.5), 3.0, 1.0, ui.t.accent);
            }
        }
    }
    ui.c.pop_clip();
    if s.rack.loop_mode {
        let loop_box = Rect::new(row.right() + 6.0, y, 27.0, row.h);
        face(
            ui,
            loop_box,
            Action::RackLoopBox(index),
            "Set loop point",
            false,
        );
        label(
            ui,
            loop_box,
            &rk::row_len(s, index).to_string(),
            text(ui, true),
            false,
        );
    }
}

fn graph_lane(ui: &mut Ui, s: &State, lane: Rect, bars: Rect, grid: &Grid, per_step: u32) {
    ui.c.fill_rrect(lane, 6.0, ui.t.s(0x111113));
    ui.c.stroke_rrect(lane.inset(0.5), 6.0, 1.0, ui.t.s(0x27272b));
    ui.hit(lane, Action::Swallow, "Graph editor");
    let tab = s.rack.graph_tab;
    let menu = Rect::new(
        lane.x + 32.0,
        lane.y + 6.0,
        bars.x - lane.x - 41.0,
        lane.h - 12.0,
    );
    let hot = face(
        ui,
        menu,
        Action::RackGraphMenu,
        "Choose the graph editor parameter",
        false,
    );
    label(
        ui,
        menu,
        if tab == Tab::Vel {
            "Velocity"
        } else {
            tab.label()
        },
        text(ui, true),
        true,
    );
    glyph(
        ui,
        Icon::ArrowDown,
        Rect::new(menu.right() - 23.0, menu.y, 22.0, menu.h),
        11.0,
        text(ui, hot),
    );
    ui.c.polyline(
        &[
            (lane.x + 11.0, lane.y),
            (lane.x + 11.0, menu.cy()),
            (menu.x - 4.0, menu.cy()),
        ],
        1.3,
        ui.t.accent,
    );
    ui.c.push_clip(bars);
    let notes = rk::row_notes(s, s.selected_channel);
    for step in grid.steps() {
        let cell = Rect::new(grid.x(step), bars.y, STEP_W - 2.0, bars.h);
        ui.c.fill_rect(
            cell,
            ui.t.s(if grid.alternate(step) {
                0x242427
            } else {
                0x1e1e21
            }),
        );
        if let Some(note) = notes.iter().find(|n| n.position / per_step == step) {
            let value_y = bars.bottom() - tab.get(note, per_step) * bars.h;
            let base = if tab.centred() {
                bars.cy()
            } else {
                bars.bottom()
            };
            let color = if tab == Tab::Vel {
                Color::rgb(0xab6299)
            } else {
                ui.t.a(tab.color())
            };
            if tab != Tab::Note {
                ui.c.fill_rect(
                    Rect::span(cell.x, cell.right(), value_y.min(base), value_y.max(base)),
                    color,
                );
            }
            ui.c.fill_rect(Rect::new(cell.x, value_y, cell.w, 1.5), color.shade(0.25));
        }
    }
    ui.c.pop_clip();
    ui.hit(
        bars,
        Action::RackGraphArea,
        format!("Graph editor: {} - click or drag to set", tab.label()),
    );
}

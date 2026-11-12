// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use os_ui::Rect;

use crate::app::{Action, App, State};
use crate::{history, rack, views};

pub const OVERLAY_VISIBILITY: &str = "visible_rack-selection-overlay";

#[derive(Clone, Debug)]
pub struct RowDrag {
    origin: (f32, f32),
    scroll: f32,
    selection: BTreeSet<usize>,
    move_row: Option<usize>,
    preset: Option<usize>,
    pub started: bool,
}

#[derive(Clone, Copy)]
pub enum Overlay {
    Selection(Rect),
    Insertion(f32),
}

impl RowDrag {
    pub fn begin(app: &App, action: &Action) -> Option<Self> {
        let s = &app.state;
        let (x, y) = app.mouse;
        if app.shift || app.alt || !views::rack::row_view(s).contains(x, y) {
            return None;
        }
        let row = match *action {
            Action::ChannelSelect(row) | Action::RackSelect(row) | Action::RackRoll(row) => Some(row),
            Action::Step(row, _) if app.ctrl => Some(row),
            Action::RackArea | Action::RackStepGap => views::rack::row_at(s, y),
            _ => return None,
        };
        let move_row = if app.ctrl { Some(row?) } else { None };
        let preset = match *action {
            Action::ChannelSelect(row) if !app.ctrl => Some(row),
            _ => None,
        };
        Some(Self {
            origin: (x, y),
            scroll: views::rack::scroll(s),
            selection: s.rack.sel.clone(),
            move_row,
            preset,
            started: false,
        })
    }

    pub fn motion(&mut self, s: &mut State, mouse: (f32, f32)) {
        self.started |= (mouse.0 - self.origin.0).hypot(mouse.1 - self.origin.1) >= 5.0;
        if !self.started || self.move_row.is_some() { return; }
        let view = views::rack::row_view(s);
        if view.is_empty() { return; }
        let start = self.start_y(s);
        let end = mouse.1.clamp(view.y, view.bottom());
        let (top, bottom) = (start.min(end), start.max(end));
        s.rack.sel = views::rack::layout(s).into_iter().filter_map(|row| {
            let y = views::rack::row_y(s, &row);
            (y <= bottom && y + row.pitch() > top).then_some(row.row)
        }).collect();
    }

    pub fn overlay(&self, s: &State, mouse: (f32, f32)) -> Option<Overlay> {
        if !self.started { return None; }
        if self.move_row.is_some() {
            return insertion(s, mouse).map(|(_, _, y)| Overlay::Insertion(y));
        }
        if !crate::workspace_visibility::shown(OVERLAY_VISIBILITY) { return None; }
        let start = self.start_y(s);
        Some(Overlay::Selection(Rect::new(
            self.origin.0.min(mouse.0), start.min(mouse.1),
            (self.origin.0 - mouse.0).abs().max(6.0), (start - mouse.1).abs().max(2.0),
        )))
    }

    pub fn cancel(self, s: &mut State) {
        if self.started && self.move_row.is_none() {
            s.rack.sel = self.selection;
        }
    }

    pub fn finish(self, app: &mut App) {
        if let Some(from) = self.move_row {
            if let Some((target, after, _)) = insertion(&app.state, app.mouse) {
                move_rows(app, from, target, after);
            }
        } else if let Some(row) = self.preset.filter(|_| over_browser(app)) {
            app.state.rack.sel = self.selection;
            app.state.message = crate::presets::save_channel_drop(app, row);
        } else if !app.state.rack.sel.contains(&app.state.selected_channel)
            && let Some(&row) = app.state.rack.sel.first()
        {
            app.focus_channel(row);
        }
        app.invalidate();
    }

    fn start_y(&self, s: &State) -> f32 {
        self.origin.1 + self.scroll - views::rack::scroll(s)
    }
}

fn over_browser(app: &App) -> bool {
    let s = &app.state;
    let (x, y) = app.mouse;
    s.show_browser && !s.browser_auto_hidden && s.native_view.is_none()
        && x >= 0.0 && x < views::browser_w(s) && y >= s.work_rect().y
}

fn insertion(s: &State, (x, y): (f32, f32)) -> Option<(usize, bool, f32)> {
    let view = views::rack::row_view(s);
    if !view.contains(x, y) { return None; }
    let mut last = None;
    for row in views::rack::layout(s) {
        let top = views::rack::row_y(s, &row);
        let bottom = top + row.pitch();
        if bottom <= view.y || top >= view.bottom() { continue; }
        let center = (top.max(view.y) + bottom.min(view.bottom())) * 0.5;
        let after = (row.row, true, bottom.min(view.bottom() - 1.0));
        if y < bottom {
            return Some(if y < center { (row.row, false, top.max(view.y)) } else { after });
        }
        last = Some(after);
    }
    last
}

fn move_rows(app: &mut App, from: usize, target: usize, after: bool) {
    let s = &mut app.state;
    let selected = rack::selection_for(s, from);
    let moved: BTreeSet<_> = rack::shown(s).into_iter().filter(|row| selected.contains(row)).collect();
    if moved.is_empty() || moved.contains(&target) { return; }
    let mut order: Vec<_> = (0..s.project.channels.len()).filter(|row| !moved.contains(row)).collect();
    let Some(at) = order.iter().position(|&row| row == target) else { return };
    let at = at + usize::from(after);
    order.splice(at..at, moved.iter().copied());
    if order.iter().copied().eq(0..order.len()) { return; }
    let before = history::Snap::RackOrder(
        rack::inverse_order(&order), s.collaborating.then(|| s.project.channels.clone()),
    );
    history::record(s, "Move channels", before);
    s.rack.sel = moved;
    if !s.rack.sel.contains(&s.selected_channel) {
        app.focus_channel(from);
    }
    rack::reorder(app, &order);
}

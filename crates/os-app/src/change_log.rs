// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared, bundled release history. Reading/searching never needs a network request.
use crate::app::Ui;
use os_ui::{Font, Rect};
use serde::Deserialize;
use std::sync::OnceLock;

pub const LABEL: &str = "Change Log";
pub const VISIBILITY: &str = "visible_help-change-log";
pub const PAGE: usize = crate::help::PAGE + 1;
#[derive(Deserialize)]
struct History {
    intro: String,
    sections: Vec<Section>,
}
#[derive(Deserialize)]
struct Section {
    date: String,
    title: String,
    changes: Vec<String>,
}
fn history() -> &'static History {
    static DATA: OnceLock<History> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../../../packaging/changelog.json"))
            .expect("validated bundled changelog")
    })
}
fn lines(ui: &mut Ui, query: &str, width: f32) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let mut add = |text: &str, heading: bool| {
        // Measure every line with the exact same font used for painting.
        out.extend(
            crate::help::wrap_lines(ui, font(heading), text, width)
                .into_iter()
                .map(|l| (l, heading)),
        );
        out.push((String::new(), false));
    };
    add(&history().intro, false);
    let query = query.trim().to_lowercase();
    let mut found = false;
    let mut days = std::collections::BTreeMap::<&str, Vec<&str>>::new();
    for section in &history().sections {
        let changes = days
            .entry(section.date.get(..10).unwrap_or(&section.date))
            .or_default();
        for change in &section.changes {
            if !changes.contains(&change.as_str()) {
                changes.push(change);
            }
        }
    }
    for (date, changes) in days.iter().rev() {
        let changes: Vec<_> = changes
            .iter()
            .filter(|c| {
                query.is_empty() || date.contains(&query) || c.to_lowercase().contains(&query)
            })
            .collect();
        if changes.is_empty() {
            continue;
        }
        found = true;
        add(date, true);
        for change in changes {
            add(&format!("• {change}"), false);
        }
    }
    if !found {
        add("No changes match your search.", false);
    }
    out
}
fn font(heading: bool) -> Font {
    if heading {
        Font::new(12).bold()
    } else {
        Font::new(12)
    }
}

pub fn paint_rows(ui: &mut Ui, body: Rect, offset: f32, query: &str) -> f32 {
    const LINE: f32 = 18.;
    let rows = lines(ui, query, body.w);
    for (i, (line, heading)) in rows.iter().enumerate() {
        let y = body.y + i as f32 * LINE - offset;
        if y + LINE >= body.y && y < body.bottom() {
            let ink = if *heading {
                ui.t.h(0xb5d98b)
            } else {
                ui.t.text
            };
            ui.c.text_clip(font(*heading), body.x, y, body.w, ink, line);
        }
    }
    rows.len() as f32 * LINE
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{Action, App, Scroll},
        help::Hit,
        views::{menus, settings::SAct},
    };
    use winit::keyboard::{Key, NamedKey};

    #[test]
    fn changelog_opens_from_help_and_searches_offline() {
        let mut app = App::new(None);
        assert_eq!(
            menus::item_at(menus::HELP_MENU, menus::HELP_CHANGE_LOG)
                .unwrap()
                .label,
            LABEL
        );
        for (w, h) in [(1000, 600), (1600, 900)] {
            app.state.screen = (w as f32, h as f32);
            app.menu_item(menus::HELP_MENU, menus::HELP_CHANGE_LOG);
            assert_eq!(app.state.settings.tab, PAGE);
            assert!(app.state.settings.open);
            let mut canvas = os_ui::Canvas::new(w, h, 1.).unwrap();
            app.paint(&mut canvas, None);
            assert!(
                app.hits
                    .iter()
                    .any(|hit| hit.action == Action::Settings(SAct::Help(Hit::Search)))
            );
            assert!(
                app.hits
                    .iter()
                    .any(|hit| hit.action == Action::Scrollbar(Scroll::Settings))
            );
            let full = app.state.settings.help.content_height.get();
            assert!(full > 1000.);
            app.key(&Key::Named(NamedKey::PageDown), false, false);
            assert!(app.state.settings.scroll > 0.);
            app.key(&Key::Character("no-such-change-12345".into()), false, false);
            app.paint(&mut canvas, None);
            assert_eq!(app.state.settings.scroll, 0.);
            assert!(app.state.settings.help.content_height.get() < full);
            assert!(
                !app.hits
                    .iter()
                    .any(|hit| hit.action == Action::Scrollbar(Scroll::Settings))
            );
            app.key(&Key::Named(NamedKey::Escape), false, false);
            app.key(&Key::Named(NamedKey::Escape), false, false);
            assert!(!app.state.settings.open);
        }
        assert_eq!(history().sections.last().unwrap().date, "2026-10-28");
        assert!(
            history()
                .sections
                .iter()
                .map(|s| s.changes.len())
                .sum::<usize>()
                > 100
        );
    }
}

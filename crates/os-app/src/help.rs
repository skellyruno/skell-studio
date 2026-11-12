// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline workflow articles, hosted in the same overlay as Settings.
use crate::app::{Action, App, Scroll, State, Ui};
use crate::views::settings::{self, SAct};
use os_ui::{Color, Font, Rect};
use std::{cell::Cell, sync::OnceLock};
use winit::keyboard::{Key, NamedKey};

mod spotlight;
pub(crate) use spotlight::{active, paint as paint_spotlight};

pub const LABEL: &str = "Search User Guide...";
pub const VISIBILITY: &str = "visible_help-guide";
pub const SPOTLIGHT_VISIBILITY: &str = "visible_help-show-where";
pub const PAGE: usize = settings::TABS.len();

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Hit {
    Search,
    Clear,
    Topic(usize),
    Back,
    ShowWhere,
    ReturnToGuide,
    DismissSpotlight,
    Close,
}

#[derive(Debug, Default)]
pub struct View {
    search: crate::dialog::Field,
    focused: bool,
    topic: Option<usize>,
    results_scroll: f32,
    spotlight: Option<spotlight::Visit>,
    pub content_height: Cell<f32>,
}

struct Topic {
    category: &'static str,
    title: &'static str,
    keywords: &'static str,
    paragraphs: Vec<&'static str>,
    destination: Option<spotlight::Target>,
    index: String,
}

fn topics() -> &'static [Topic] {
    static TOPICS: OnceLock<Vec<Topic>> = OnceLock::new();
    TOPICS.get_or_init(|| {
        let mut category = "";
        let mut topics: Vec<Topic> = Vec::new();
        for line in include_str!("../../../docs/user-guide.md")
            .lines()
            .map(str::trim)
        {
            if let Some(name) = line.strip_prefix("## ") {
                category = name;
            } else if let Some(title) = line.strip_prefix("### ") {
                topics.push(Topic {
                    category,
                    title,
                    keywords: "",
                    paragraphs: Vec::new(),
                    destination: None,
                    index: String::new(),
                });
            } else if !line.is_empty()
                && let Some(topic) = topics.last_mut()
            {
                if topic.keywords.is_empty() {
                    topic.keywords = line;
                } else if let Some(key) = line.strip_prefix("Guide: ") {
                    topic.destination = spotlight::Target::from_key(key);
                } else {
                    topic.paragraphs.push(line);
                }
            }
        }
        for topic in &mut topics {
            topic.index = format!(
                "{} {} {} {}",
                topic.category,
                topic.title,
                topic.keywords,
                topic.paragraphs.join(" ")
            )
            .to_lowercase();
        }
        topics
    })
}

fn results(query: &str) -> Vec<usize> {
    let query = query.to_lowercase();
    let terms: Vec<_> = query
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| {
            !word.is_empty()
                && !matches!(
                    *word,
                    "how" | "do" | "i" | "can" | "to" | "the" | "a" | "an" | "my" | "in" | "and"
                )
        })
        .collect();
    let mut matches: Vec<_> = topics()
        .iter()
        .enumerate()
        .filter_map(|(index, topic)| {
            if !terms.iter().all(|term| topic.index.contains(term)) {
                return None;
            }
            let title = topic.title.to_lowercase();
            let rank = terms
                .iter()
                .map(|term| {
                    if title.contains(term) {
                        3
                    } else if topic.keywords.contains(term) {
                        2
                    } else {
                        0
                    }
                })
                .sum::<u32>();
            Some((index, rank))
        })
        .collect();
    matches.sort_by_key(|&(index, rank)| (std::cmp::Reverse(rank), index));
    matches.into_iter().map(|(index, _)| index).collect()
}

pub fn is_open(s: &State) -> bool {
    s.settings.open && matches!(s.settings.tab, PAGE | crate::change_log::PAGE)
}

pub fn open(app: &mut App) {
    if crate::workspace_visibility::shown(VISIBILITY) {
        app.open_settings(PAGE);
    }
}

pub fn reset(view: &mut View) {
    *view = View {
        focused: true,
        ..Default::default()
    };
}

pub fn act(app: &mut App, hit: Hit) {
    match hit {
        Hit::ShowWhere => return spotlight::show(app),
        Hit::ReturnToGuide => return spotlight::return_to_guide(app),
        Hit::DismissSpotlight => return spotlight::dismiss(app),
        _ => {}
    }
    if !is_open(&app.state) {
        return;
    }
    let st = &mut app.state.settings;
    let view = &mut st.help;
    match hit {
        Hit::Search => {
            view.focused = true;
            view.search.select_all();
        }
        Hit::Clear => {
            view.search.set("");
            view.topic = None;
            view.focused = true;
            st.scroll = 0.;
        }
        Hit::Topic(index) if index < topics().len() => {
            view.results_scroll = st.scroll;
            view.topic = Some(index);
            view.focused = false;
            st.scroll = 0.;
        }
        Hit::Back => {
            view.topic = None;
            view.focused = false;
            st.scroll = view.results_scroll;
        }
        Hit::Close => st.open = false,
        Hit::Topic(_) | Hit::ShowWhere | Hit::ReturnToGuide | Hit::DismissSpotlight => {}
    }
    app.invalidate();
}

pub fn key(app: &mut App, key: &Key, ctrl: bool, alt: bool) -> bool {
    if !is_open(&app.state)
        && active(&app.state)
        && *key == Key::Named(NamedKey::Escape)
        && app.state.menu_open.is_none()
        && app.state.playlist.popup.is_none()
        && app.drag.is_none()
    {
        spotlight::dismiss(app);
        return true;
    }
    if !is_open(&app.state) {
        return false;
    }
    let page_height = body(&app.state).h;
    let st = &mut app.state.settings;
    let view = &mut st.help;
    match key {
        Key::Named(NamedKey::Escape) if view.topic.is_some() => act(app, Hit::Back),
        Key::Named(NamedKey::Escape) if !view.search.text.is_empty() => act(app, Hit::Clear),
        Key::Named(NamedKey::Escape) => act(app, Hit::Close),
        Key::Character(c) if ctrl && !alt && c.eq_ignore_ascii_case("f") => act(app, Hit::Search),
        Key::Named(NamedKey::Enter) if st.tab == PAGE && view.topic.is_none() => {
            if let Some(index) = results(&view.search.text).first().copied() {
                act(app, Hit::Topic(index));
            }
        }
        Key::Named(NamedKey::PageDown | NamedKey::PageUp) => {
            let direction = if *key == Key::Named(NamedKey::PageDown) {
                1.
            } else {
                -1.
            };
            let max = (view.content_height.get() - page_height).max(0.);
            st.scroll = (st.scroll + direction * page_height * 0.9).clamp(0., max);
        }
        _ if view.focused => {
            let previous = view.search.text.clone();
            if view.search.key(key, ctrl, alt) && view.search.text != previous {
                view.topic = None;
                st.scroll = 0.;
            }
        }
        _ => {}
    }
    true
}

fn rect(s: &State) -> Rect {
    let (width, height) = s.screen;
    let w = (width - 24.).clamp(220., 840.);
    let h = (height - 36.).clamp(180., 740.);
    Rect::new(
        ((width - w) / 2.).max(0.).round(),
        ((height - h) / 2.).max(0.).round(),
        w,
        h,
    )
}

pub fn body(s: &State) -> Rect {
    let r = rect(s);
    Rect::new(r.x + 16., r.y + 117., r.w - 38., r.h - 130.)
}

fn hit(hit: Hit) -> Action {
    Action::Settings(SAct::Help(hit))
}

fn button(ui: &mut Ui, r: Rect, label: &str, action: Hit) {
    let hover = ui.hit(r, hit(action), label.to_owned());
    ui.c.fill_rrect(r, 4., if hover { ui.t.button_hi } else { ui.t.button });
    ui.c.text_center(Font::new(12), r, ui.t.text, label);
}

pub(crate) fn wrap_lines(ui:&mut Ui,font:Font,value:&str,width:f32)->Vec<String>{
 let mut out=Vec::new();
 for paragraph in value.lines(){
  let mut line=String::new();
  for word in paragraph.split_whitespace(){
   let candidate=if line.is_empty(){word.to_string()}else{format!("{line} {word}")};
   if ui.c.text_width(font,&candidate)<=width {line=candidate;continue;}
   if !line.is_empty(){out.push(std::mem::take(&mut line));}
   for ch in word.chars(){let candidate=format!("{line}{ch}");if !line.is_empty()&&ui.c.text_width(font,&candidate)>width {out.push(std::mem::take(&mut line));}line.push(ch);}
  }
  out.push(line);
 }
 out
}

pub fn paint(ui: &mut Ui, s: &State) {
    let r = rect(s);
    let body = body(s);
    let st = &s.settings;
    let view = &st.help;
    let change_log = st.tab == crate::change_log::PAGE;
    let t = ui.t;

    ui.hit(r, Action::Settings(SAct::Body), "");
    ui.c.fill_rrect(r.outset(4.), 10., Color::rgb(0x000000).with_alpha(80));
    ui.c.fill_rrect(r, 8., t.window_title);
    ui.c.fill_rrect(r.inset(1.), 7., t.s(0x25282b));

    ui.c.text_clip(
        Font::new(21).bold(),
        r.x + 16.,
        r.y + 12.,
        r.w - 98.,
        t.text,
        if change_log { crate::change_log::LABEL } else { "Skell Studio User Guide" },
    );
    button(
        ui,
        Rect::new(r.right() - 75., r.y + 12., 59., 26.),
        "Close",
        Hit::Close,
    );
    ui.c.text_clip(
        Font::new(12),
        r.x + 16.,
        r.y + 41.,
        r.w - 32.,
        t.text_dim,
        if change_log { "Skell Studio release history · available offline" }
        else { "Browse topics or search by feature, task, or shortcut. Available offline." },
    );

    let search = Rect::new(r.x + 16., r.y + 61., r.w - 100., 30.);
    let hover = ui.hit(search, hit(Hit::Search), if change_log { "Search the Change Log" } else { "Search the User Guide" });
    crate::views::sunken(ui, search, t.sunken);
    settings::cell_text(
        ui,
        search,
        &view.search.text,
        view.focused.then_some(&view.search),
        hover,
    );
    if view.search.text.is_empty() {
        ui.c.text_clip(
            Font::new(12),
            search.x + 7.,
            search.y + 7.,
            search.w - 14.,
            t.text_dim,
            if change_log { "Search by date or change" } else { "Try audio timing, no sound, or Ctrl wheel" },
        );
    }
    button(
        ui,
        Rect::new(search.right() + 7., search.y, 61., search.h),
        "Clear",
        Hit::Clear,
    );

    let matches = if change_log { Vec::new() } else { results(&view.search.text) };
    let offset = st
        .scroll
        .clamp(0., (view.content_height.get() - body.h).max(0.));
    ui.c.push_clip(body);
    ui.clip_hit_rect(body, |ui| {
        let mut y = body.y - offset;
        if change_log {
            y += crate::change_log::paint_rows(ui, body, offset, &view.search.text);
        } else if let Some(topic) = view.topic.and_then(|i| topics().get(i)) {
            for line in wrap_lines(ui, Font::new(12).bold(), topic.title, body.w) {
                ui.c.text_clip(Font::new(12).bold(), body.x, y, body.w, t.text, &line);
                y += 19.;
            }
            y += 9.;
            for paragraph in &topic.paragraphs {
                for line in wrap_lines(ui, Font::new(12), paragraph, body.w - 8.) {
                    ui.c.text_clip(Font::new(12), body.x, y, body.w, t.text, &line);
                    y += 19.;
                }
                y += 9.;
            }
        } else {
            let browse = view.search.text.trim().is_empty();
            let mut previous_category = "";
            for index in matches.iter().copied() {
                let topic = &topics()[index];
                if browse && topic.category != previous_category {
                    ui.c.text_clip(
                        Font::new(13).bold(),
                        body.x,
                        y + 3.,
                        body.w,
                        t.text,
                        topic.category,
                    );
                    y += 25.;
                    previous_category = topic.category;
                }
                let card = Rect::new(body.x, y, body.w, 42.);
                if card.bottom() > body.y && card.y < body.bottom() {
                    let hover = ui.hit(card, hit(Hit::Topic(index)), topic.title);
                    ui.c.fill_rrect(card, 5., if hover { t.button_hi } else { t.s(0x303438) });
                    ui.c.text_clip(
                        Font::new(14).bold(),
                        card.x + 10.,
                        card.y + 4.,
                        card.w - 20.,
                        t.text,
                        topic.title,
                    );
                    ui.c.text_clip(
                        Font::new(11),
                        card.x + 10.,
                        card.y + 23.,
                        card.w - 20.,
                        t.text_dim,
                        if browse {
                            topic.keywords
                        } else {
                            topic.category
                        },
                    );
                }
                y += 47.;
            }
            if matches.is_empty() {
                ui.c.text_clip(
                    Font::new(13),
                    body.x,
                    y + 5.,
                    body.w,
                    t.text,
                    "No matching topics. Try fewer words or a feature name.",
                );
                y += 28.;
            }
        }
        view.content_height.set(y - body.y + offset);
    });
    ui.c.pop_clip();

    if change_log {
        ui.c.text_clip(Font::new(12), body.x, r.y + 98., body.w, t.text_dim, "Newest changes first");
    } else if let Some(topic) = view.topic.and_then(|i| topics().get(i)) {
        button(
            ui,
            Rect::new(body.x, r.y + 94., 61., 21.),
            "‹ Back",
            Hit::Back,
        );
        ui.c.text_clip(
            Font::new(12),
            body.x + 70.,
            r.y + 98.,
            (body.w
                - if topic.destination.is_some() && spotlight::enabled() {
                    200.
                } else {
                    70.
                })
            .max(0.),
            t.text_dim,
            topic.category,
        );
        if topic.destination.is_some() && spotlight::enabled() {
            let show = Rect::new(body.right() - 122., r.y + 93., 122., 22.);
            button(ui, show, "Show me where", Hit::ShowWhere);
        }
    } else {
        let count = matches.len();
        ui.c.text_clip(
            Font::new(12),
            body.x,
            r.y + 98.,
            body.w,
            t.text_dim,
            &format!("{count} topics  ·  Enter opens the first match"),
        );
    }
    let total = view.content_height.get();
    if total > body.h {
        crate::views::scrollbar(
            ui,
            Rect::new(body.right() + 7., body.y, 8., body.h),
            true,
            offset.min(total - body.h),
            body.h,
            total,
            Scroll::Settings,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::menus::{self, HELP_GUIDE, HELP_MENU};

    #[test]
    fn guide_menu_search_read_back_and_visibility() {
        assert!(topics().len() >= 50);
        assert!(topics().iter().all(|topic| !topic.category.is_empty()
            && !topic.keywords.is_empty()
            && topic.paragraphs.len() >= 2));
        let warp = results("How do I warp audio?")[0];
        assert_eq!(topics()[warp].title, "Edit audio timing with warp markers");
        assert!(!results("ctrl wheel").is_empty());
        assert!(results("zzzznoresult").is_empty());

        let mut app = App::new(None);
        let item = menus::item_at(HELP_MENU, HELP_GUIDE).unwrap();
        assert!(menus::in_menu(&app.state, HELP_MENU, item));
        for (w, h) in [(700, 460), (1280, 800)] {
            app.state.screen = (w as f32, h as f32);
            let mut canvas = os_ui::Canvas::new(w, h, 1.).unwrap();
            app.menu_item(HELP_MENU, HELP_GUIDE);
            assert!(is_open(&app.state));
            app.paint(&mut canvas, None);
            assert!(settings::content_h(&app.state) > settings::page_h(&app.state));
            app.key(&Key::Character("warp audio".into()), false, false);
            app.paint(&mut canvas, None);
            let row = app
                .hits
                .iter()
                .find(|target| target.action == hit(Hit::Topic(warp)))
                .unwrap()
                .rect;
            app.mouse_move(row.cx(), row.cy());
            app.mouse_down();
            app.mouse_up();
            assert_eq!(app.state.settings.help.topic, Some(warp));
            app.paint(&mut canvas, None);
            if let Some(dir) = std::env::var_os("OS_SHOT_DIR") {
                canvas
                    .pixmap
                    .save_png(std::path::PathBuf::from(dir).join(format!("user-guide-{w}.png")))
                    .unwrap();
            }
            app.key(&Key::Named(NamedKey::Escape), false, false);
            assert!(app.state.settings.help.topic.is_none());
            assert_eq!(app.state.settings.help.search.text, "warp audio");
            app.key(&Key::Named(NamedKey::Enter), false, false);
            assert_eq!(app.state.settings.help.topic, Some(warp));
        }
        act(&mut app, Hit::Close);
        crate::workspace_visibility::toggle(VISIBILITY);
        assert!(!menus::in_menu(&app.state, HELP_MENU, item));
        app.menu_item(HELP_MENU, HELP_GUIDE);
        assert!(!is_open(&app.state));
    }

    #[test]
    fn spotlight_reveals_control_and_returns_to_article() {
        let mut app = App::new(None);
        app.state.project = crate::app::test_project();
        app.state.z.clear();
        let index = results("warp audio")[0];
        let mut canvas = os_ui::Canvas::new(1280, 800, 1.).unwrap();
        let before = app.state.project.clone();
        let click = |app: &mut App, action: Hit| {
            let rect = app
                .hits
                .iter()
                .find(|target| target.action == hit(action))
                .unwrap()
                .rect;
            app.mouse_move(rect.cx(), rect.cy());
            app.mouse_down();
            app.mouse_up();
        };

        for vintage in [false, true] {
            app.state.vintage_2005 = vintage;
            open(&mut app);
            act(&mut app, Hit::Topic(index));
            app.paint(&mut canvas, None);
            click(&mut app, Hit::ShowWhere);
            assert!(!is_open(&app.state));
            assert!(active(&app.state));
            assert_eq!(app.state.z.last(), Some(&crate::app::Win::Playlist));
            app.paint(&mut canvas, None);
            assert!(canvas.pixmap.pixels().iter().any(|pixel| (
                pixel.red(),
                pixel.green(),
                pixel.blue()
            ) == (255, 218, 80)));
            let anchor = app
                .hits
                .iter()
                .find(|h| h.action == Action::Warp(crate::warp::Command::Toggle(0)))
                .unwrap()
                .rect;
            assert!(matches!(
                app.top_hit(anchor.cx(), anchor.cy()).map(|h| &h.action),
                Some(Action::Warp(crate::warp::Command::Toggle(0)))
            ));
            if let Some(dir) = std::env::var_os("OS_SHOT_DIR") {
                canvas
                    .pixmap
                    .save_png(
                        std::path::PathBuf::from(dir)
                            .join(format!("guide-spotlight-{vintage}.png")),
                    )
                    .unwrap();
            }
            click(&mut app, Hit::ReturnToGuide);
            assert!(is_open(&app.state));
            assert_eq!(app.state.settings.help.topic, Some(index));
            assert!(!active(&app.state));
            assert!(app.state.project == before);
        }

        let scan = results("find installed plugins")[0];
        act(&mut app, Hit::Topic(scan));
        app.paint(&mut canvas, None);
        click(&mut app, Hit::ShowWhere);
        assert_eq!(app.state.settings.tab, settings::PLUGINS);
        assert!(active(&app.state));
        app.paint(&mut canvas, None);
        click(&mut app, Hit::ReturnToGuide);
        assert_eq!(app.state.settings.help.topic, Some(scan));
        crate::workspace_visibility::toggle(SPOTLIGHT_VISIBILITY);
        app.paint(&mut canvas, None);
        assert!(
            !app.hits
                .iter()
                .any(|target| target.action == hit(Hit::ShowWhere))
        );
    }
}

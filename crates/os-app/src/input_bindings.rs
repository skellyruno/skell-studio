//! Per-profile bindings. Input is resolved before native editor gestures; unmapped input falls through.
use crate::{
    app::{Action, App, State, Ui, Win},
    menu_layout::Menu,
    playlist::{Cmd, PItem, Popup},
    views::settings::{CONTROLS, SAct},
};
use os_ui::{Font, Rect};
use serde::{Deserialize, Serialize};
use std::{
    cell::{RefCell, RefMut},
    collections::HashSet,
    rc::Rc,
};
use winit::keyboard::{Key, NamedKey};

pub const VISIBILITY: &str = "visible_input_bindings";
const STORE: &str = "input_bindings_v1";
const PAGE_SIZE: usize = 4;
const KEYBOARD: usize = 8;
const CONTEXTS: &[&str] = &[
    "Global",
    "Piano Roll",
    "Playlist",
    "Mixer",
    "Channel Rack",
    "Browser",
    "Piano Roll: empty grid",
    "Piano Roll: notes",
];
const EVENTS: &[&str] = &[
    "Left click",
    "Left drag",
    "Right click",
    "Right drag",
    "Middle click",
    "Middle drag",
    "Wheel up",
    "Wheel down",
    "Keyboard",
    "Left double click",
    "Left triple click",
    "Hover",
    "Activate window",
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct Binding {
    context: usize,
    event: String,
    modifiers: u8,
    actions: Vec<String>,
    #[serde(default)]
    name: String,
    #[serde(default)]
    pinned: bool,
}
#[derive(Debug)]
pub struct Editor {
    context: usize,
    event: usize,
    modifiers: u8,
    key: String,
    capture: bool,
    search: crate::dialog::Field,
    searching: bool,
    page: usize,
    notice: String,
    previous: Vec<String>,
    name: crate::dialog::Field,
    naming: bool,
    pub(crate) advanced: bool,
    browser: RefCell<Option<ActionBrowser>>,
}
impl Default for Editor {
    fn default() -> Self {
        Self {
            context: 0,
            event: KEYBOARD,
            modifiers: 0,
            key: String::new(),
            capture: false,
            search: Default::default(),
            searching: false,
            page: 0,
            notice: String::new(),
            previous: Vec::new(),
            name: Default::default(),
            naming: false,
            advanced: false,
            browser: RefCell::new(None),
        }
    }
}
impl Editor {
    pub(crate) fn open(&mut self) {
        self.capture = false;
        self.searching = false;
        self.naming = false;
        self.browser.get_mut().take();
        sync_name(self);
    }
}

#[derive(Debug)]
struct Choice {
    id: String,
    label: String,
    search: String,
    sequence: bool,
}

#[derive(Debug)]
struct ActionBrowser {
    context: usize,
    bindings: Rc<Vec<Binding>>,
    items: Vec<Choice>,
    query: String,
    pointers: bool,
    filtered: Vec<usize>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    Context,
    Event,
    Modifiers,
    Capture,
    Advanced,
    Assign(String),
    Append(String),
    Remove,
    ClearChain,
    SelectContext(usize),
    SelectEvent(usize),
    SelectModifiers(u8),
    Search,
    ClearSearch,
    Page(i32),
    Saved,
    Load(usize),
    Undo,
    Import,
    Export,
    Name,
    SaveName,
    Pin,
}
#[derive(Clone)]
struct Pending {
    button: usize,
    origin: (f32, f32),
    click: Option<Vec<String>>,
    drag: Option<Vec<String>>,
    context: usize,
    modifiers: u8,
}
#[derive(Clone)]
struct Click {
    pending: Pending,
    count: u8,
    until: std::time::Instant,
}
#[derive(Default)]
pub struct Runtime {
    replay: bool,
    pending: Option<Pending>,
    owner: Option<usize>,
    gesture: bool,
    click: Option<Click>,
    hover: Option<(usize, Action, (f32, f32), std::time::Instant, bool)>,
    focused: Option<Win>,
}

#[derive(Default)]
struct BindingCache {
    text: String,
    parsed: Rc<Vec<Binding>>,
}

thread_local! {
    static BINDINGS: RefCell<BindingCache> = RefCell::new(BindingCache::default());
}

fn bindings() -> Rc<Vec<Binding>> {
    crate::settings_file::read(|settings| {
        let text = settings.get(STORE).unwrap_or("");
        BINDINGS.with(|cache| {
            let mut cache = cache.borrow_mut();
            if cache.text != text {
                let parsed = serde_json::from_str::<Vec<Binding>>(text)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|b| {
                        b.context < CONTEXTS.len()
                            && b.modifiers < 8
                            && valid_actions(&b.event, &b.actions)
                    })
                    .take(2048)
                    .collect();
                cache.parsed = Rc::new(parsed);
                cache.text.clear();
                cache.text.push_str(text);
            }
            cache.parsed.clone()
        })
    })
}

fn current<'a>(e: &Editor, all: &'a [Binding]) -> Option<&'a Binding> {
    let input = event(e);
    all.iter()
        .find(|b| b.context == e.context && b.event == input && b.modifiers == e.modifiers)
}

fn sync_name(e: &mut Editor) {
    let all = bindings();
    e.name.set(current(e, &all).map_or("", |b| b.name.as_str()));
}

fn binding_id(b: &Binding) -> String {
    format!("binding:{}:{}:{}", b.context, b.modifiers, b.event)
}
fn saved_binding<'a>(all: &'a [Binding], id: &str) -> Option<&'a Binding> {
    let mut fields = id.strip_prefix("binding:")?.splitn(3, ':');
    let context = fields.next()?.parse::<usize>().ok()?;
    let modifiers = fields.next()?.parse::<u8>().ok()?;
    let event = fields.next()?;
    all.iter()
        .find(|b| b.context == context && b.modifiers == modifiers && b.event == event)
}

pub fn has_pinned() -> bool {
    bindings().iter().any(|b| b.pinned && !b.name.is_empty())
}
fn expand(actions: &[String], c: usize, visiting: &mut Vec<String>) -> Result<Vec<String>, String> {
    expand_from(&bindings(), actions, c, visiting)
}
fn expand_from(
    all: &[Binding],
    actions: &[String],
    c: usize,
    visiting: &mut Vec<String>,
) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for id in actions {
        if id.starts_with("binding:") {
            if visiting.len() >= 8 || visiting.contains(id) {
                return Err("Saved action contains a circular reference.".into());
            }
            let b = saved_binding(all, id).ok_or("Saved action no longer exists.")?;
            if b.context != 0 && b.context != c && !(matches!(c, 6 | 7) && b.context == 1) {
                return Err("Saved action requires its original editor context.".into());
            }
            visiting.push(id.clone());
            out.extend(expand_from(all, &b.actions, c, visiting)?);
            visiting.pop();
        } else {
            out.push(id.clone());
        }
        if out.len() > 32 {
            return Err("A sequence can contain at most 32 actions.".into());
        }
    }
    Ok(out)
}
pub fn run_saved(app: &mut App, id: &str) {
    if !available(app) || app.drag.is_some() {
        return;
    }
    if saved_binding(&bindings(), id).is_some() {
        let c = context(app.state.z.last().copied());
        match expand(&[id.into()], c, &mut Vec::new()) {
            Ok(actions) if !actions.iter().any(|id| is_pointer(id)) => execute(app, actions, c),
            Ok(_) => {
                app.state.message = Some("Mouse gestures need a click or drag binding.".into())
            }
            Err(e) => app.state.message = Some(e),
        }
    }
}
pub fn toolbar(ui: &mut Ui, r: Rect, edge: f32) -> bool {
    for (i, b) in bindings()
        .iter()
        .filter(|b| b.pinned && !b.name.is_empty())
        .take(4)
        .enumerate()
    {
        let rect = Rect::new(r.x + i as f32 * 36., r.y, 32., 29.);
        if rect.right() > edge {
            return false;
        }
        let hover = ui.hit(rect, Action::InputAction(binding_id(b)), b.name.clone());
        os_ui::kit::bordered(
            ui.c,
            rect,
            1.,
            ui.t.toolbar_edge,
            os_ui::kit::face(ui.t.toolbar, os_ui::kit::State::hover(hover)),
        );
        let short: String = b.name.chars().take(2).collect();
        label(ui, rect, &short);
    }
    true
}
fn event(e: &Editor) -> String {
    if e.event == KEYBOARD {
        format!("Key {}", e.key)
    } else {
        EVENTS[e.event].into()
    }
}
fn modifiers(ctrl: bool, alt: bool, shift: bool) -> u8 {
    u8::from(ctrl) | (u8::from(alt) << 1) | (u8::from(shift) << 2)
}
fn context(win: Option<Win>) -> usize {
    match win {
        Some(Win::PianoRoll) => 1,
        Some(Win::Playlist) => 2,
        Some(Win::Mixer) => 3,
        Some(Win::ChannelRack) => 4,
        Some(Win::Browser) => 5,
        _ => 0,
    }
}
fn available(app: &App) -> bool {
    !app.input_bindings.replay
        && !app.modal_open()
        && !app.state.settings.open
        && !app.state.plugin_picker.open
        && app.state.playlist.popup.is_none()
        && app.state.menu_open.is_none()
        && app.state.playlist.edit.is_none()
        && !crate::typein::is_open(&app.state)
        && !crate::remote::is_open(&app.state)
        && app.state.rack.fill.is_none()
        && !app.state.perf.finding
        && !app.state.plugdb.finding
        && !app.state.browser_search_focus
        && app.state.browser_tag_edit.is_none()
        && app.state.pat_ui.name_box.is_none()
        && !app.state.pat_ui.confirm
}
fn effective_binding<'a>(
    all: &'a [Binding],
    context: usize,
    event: &str,
    mods: u8,
) -> Option<&'a Binding> {
    let find = |scope| {
        all.iter()
            .find(|b| b.context == scope && b.event == event && b.modifiers == mods)
    };
    find(context)
        .or_else(|| {
            if matches!(context, 6 | 7) {
                find(1)
            } else {
                None
            }
        })
        .or_else(|| if context != 0 { find(0) } else { None })
}

fn lookup(context: usize, event: &str, mods: u8) -> Option<Vec<String>> {
    effective_binding(&bindings(), context, event, mods).map(|b| b.actions.clone())
}
fn hovered(app: &App) -> usize {
    match app.top_hit(app.mouse.0, app.mouse.1) {
        Some(h) if matches!(h.action, Action::PianoGrid) => 6,
        Some(h) if matches!(h.action, Action::PianoNote(_)) => 7,
        h => {
            if app.over_browser() {
                5
            } else {
                context(h.and_then(|h| h.win))
            }
        }
    }
}
fn catalog(s: &State, c: usize, all: &[Binding]) -> Vec<(String, String, Option<Cmd>)> {
    let mut out = vec![
        ("ignore".into(), "Do nothing".into(), None),
        (
            "native-left".into(),
            "Normal left-button action".into(),
            None,
        ),
        (
            "native-right".into(),
            "Normal right-button action".into(),
            None,
        ),
        (
            "select".into(),
            "Marquee select (Ctrl + left drag)".into(),
            None,
        ),
    ];
    for &(id, label, scope) in BUILTINS {
        if c == scope || (scope == 1 && matches!(c, 6 | 7)) {
            out.push((id.into(), label.into(), None));
        }
    }
    for b in all.iter().filter(|b| {
        !b.name.is_empty()
            && (b.context == c || b.context == 0 || matches!(c, 6 | 7) && b.context == 1)
    }) {
        if !b.actions.iter().any(|id| is_pointer(id)) {
            out.push((binding_id(b), format!("Saved actions / {}", b.name), None));
        }
    }
    let menus = match c {
        1 | 6 | 7 => vec![Menu::Piano, Menu::PianoTools, Menu::PianoSelection],
        2 => vec![Menu::Playlist],
        3 => vec![Menu::Mixer, Menu::MixerTrack],
        4 => vec![Menu::Rack, Menu::RackChannel],
        5 => vec![Menu::Browser],
        _ => (0..crate::views::menus::MENUS.len())
            .map(Menu::Main)
            .collect(),
    };
    fn walk(items: Vec<PItem>, path: String, out: &mut Vec<(String, String, Option<Cmd>)>) {
        for item in items {
            if item.sep || item.head || item.col {
                continue;
            }
            let name = format!("{path}/{}", item.label);
            if !item.children.is_empty() {
                walk(item.children, name, out);
            } else if item.enabled && !matches!(item.cmd, Cmd::Nothing | Cmd::NotYet(_)) {
                out.push((name.clone(), name, Some(item.cmd)));
            }
        }
    }
    for menu in menus {
        walk(menu.catalog(s), menu.label().into(), &mut out);
    }
    out
}
fn native_press(app: &mut App, button: usize) {
    match button {
        0 => app.mouse_down(),
        1 => app.mouse_down_right(),
        _ => app.mouse_down_middle(),
    }
}
const BUILTINS: &[(&str, &str, usize)] = &[
    ("piano.insert", "Notes / Insert note at pointer", 1),
    ("piano.up", "Notes / Move up one semitone", 1),
    ("piano.down", "Notes / Move down one semitone", 1),
    ("piano.left", "Notes / Move left one snap step", 1),
    ("piano.right", "Notes / Move right one snap step", 1),
    ("piano.velocity-up", "Notes / Increase velocity", 1),
    ("piano.velocity-down", "Notes / Decrease velocity", 1),
    ("piano.zoom-in", "View / Zoom in horizontally", 1),
    ("piano.zoom-out", "View / Zoom out horizontally", 1),
    ("piano.taller", "View / Zoom in vertically", 1),
    ("piano.shorter", "View / Zoom out vertically", 1),
    (
        "mixer.volume-up",
        "Mixer / Raise selected channel volume",
        3,
    ),
    (
        "mixer.volume-down",
        "Mixer / Lower selected channel volume",
        3,
    ),
];
fn builtin(app: &mut App, id: &str, c: usize) -> bool {
    let Some(&(_, _, scope)) = BUILTINS.iter().find(|(key, _, _)| *key == id) else {
        return false;
    };
    if c != scope && !(scope == 1 && matches!(c, 6 | 7)) {
        app.state.message = Some("This action requires its editor context.".into());
        return true;
    }
    let direction = if id.ends_with("down")
        || id.ends_with("left")
        || id.ends_with("out")
        || id.ends_with("shorter")
    {
        -1
    } else {
        1
    };
    if scope == 3 {
        let t = app.state.selected_mixer;
        if crate::mixer::locked(&app.state, t) {
            app.state.message = Some(crate::mixer::LOCK_MESSAGE.into());
            return true;
        }
        if let Some(track) = crate::mixer::track(&app.state, t) {
            let start = track.volume.unwrap_or(0.8);
            let delta =
                (start + direction as f64 * crate::views::mixer::FADER_NOTCH).clamp(0., 1.) - start;
            let group = crate::mixer::fader_group(&app.state, t);
            if delta != 0. && !group.is_empty() {
                crate::mixer::snapshot_faders(&mut app.state, &group);
                crate::mixer::move_faders(&mut app.state, &group, t, start, delta);
                app.state.piano.dirty = true;
            }
        }
        return true;
    }
    use crate::piano;
    match id {
        "piano.insert" => {
            if let Some(g) = app.piano_geom() {
                if g.grid.contains(app.mouse.0, app.mouse.1) {
                    let tick =
                        crate::piano_snap::place(&app.state, &g, g.tick_at(app.mouse.0), false);
                    let key = g.key_at(app.mouse.1);
                    if let Some(ch) = piano::channel(&app.state) {
                        piano::snapshot(&mut app.state);
                        let i = piano::add_note(&mut app.state, ch, tick, key);
                        app.state.piano.select(app.state.pattern, ch, vec![i]);
                        piano::finish_edit(&mut app.state);
                    }
                } else {
                    app.state.message =
                        Some("Move the pointer over the note grid to insert a note.".into());
                }
            }
        }
        "piano.up" | "piano.down" | "piano.left" | "piano.right" => {
            let targets = piano::targets(&app.state);
            let horizontal = id.ends_with("left") || id.ends_with("right");
            let step = crate::piano_snap::unit(&app.state).unwrap_or(1) as i64;
            piano::shift(
                &mut app.state,
                &targets,
                if horizontal {
                    step * i64::from(direction)
                } else {
                    0
                },
                if horizontal { 0 } else { direction },
            );
        }
        "piano.velocity-up" | "piano.velocity-down" => {
            let targets = piano::targets(&app.state);
            if targets.iter().any(|&i| {
                piano::notes(&app.state).get(i).is_some_and(|n| {
                    (i32::from(n.velocity) + direction).clamp(0, 127) != i32::from(n.velocity)
                })
            }) {
                piano::snapshot(&mut app.state);
                for i in targets {
                    if let Some(n) = piano::pattern_mut(&mut app.state).notes.get_mut(i) {
                        n.velocity = (i32::from(n.velocity) + direction).clamp(0, 127) as u8;
                    }
                }
                piano::finish_edit(&mut app.state);
            }
        }
        "piano.zoom-in" | "piano.zoom-out" => {
            if let Some(g) = app.piano_geom() {
                let x = app.mouse.0.clamp(g.grid.x, g.grid.right());
                let from = app.state.piano.px_per_beat;
                piano::zoom_time(&mut app.state, &g, x, from, direction as f32);
            }
        }
        "piano.taller" | "piano.shorter" => {
            if let Some(g) = app.piano_geom() {
                let row = app.state.piano.row_h + direction as f32 * 2.;
                piano::set_rows(&mut app.state, g.grid, app.mouse.1, row);
            }
        }
        _ => {}
    }
    app.state.piano.dirty = true;
    true
}
fn command(s: &State, c: usize, id: &str) -> Option<Cmd> {
    let all = bindings();
    let find = |scope| {
        catalog(s, scope, &all)
            .into_iter()
            .find(|(key, _, _)| key == id)
            .and_then(|(_, _, cmd)| cmd)
    };
    find(c).or_else(|| if c == 0 { None } else { find(0) })
}

fn direct_action(c: usize, id: &str) -> bool {
    id == "ignore"
        || BUILTINS
            .iter()
            .any(|(key, _, scope)| *key == id && (c == *scope || *scope == 1 && matches!(c, 6 | 7)))
}

fn direct_command(cmd: Option<&Cmd>) -> bool {
    use crate::piano_tools::NCmd;
    matches!(
        cmd,
        Some(Cmd::Piano(
            NCmd::Quantize { .. }
                | NCmd::Legato
                | NCmd::QuickChop
                | NCmd::Mute(_)
                | NCmd::Glue
                | NCmd::DiscardLengths
                | NCmd::Duplicate
                | NCmd::ApplyNoteColor
                | NCmd::SelectedNoteType(_)
                | NCmd::Tool(_)
        ))
    )
}

fn sequence_safe(s: &State, c: usize, id: &str) -> bool {
    if id.starts_with("binding:") {
        return expand(&[id.into()], c, &mut Vec::new())
            .is_ok_and(|ids| ids.iter().all(|id| sequence_safe(s, c, id)));
    }
    direct_action(c, id) || direct_command(command(s, c, id).as_ref())
}
fn execute(app: &mut App, actions: Vec<String>, c: usize) {
    let actions = match expand(&actions, c, &mut Vec::new()) {
        Ok(a) => a,
        Err(e) => {
            app.state.message = Some(e);
            return;
        }
    };
    if actions.iter().any(|id| is_pointer(id))
        && (!app.input_bindings.gesture || actions.len() != 1)
    {
        app.state.message =
            Some("Mouse actions require a standalone click or drag binding.".into());
        return;
    }
    // Never group undo/redo, project replacement, dialogs or asynchronous work into a synchronous edit sequence.
    if actions.len() > 1 && !actions.iter().all(|id| sequence_safe(&app.state, c, id)) {
        app.state.message = Some("Sequence not run: use direct editing actions; dialogs and project commands must run alone.".into());
        return;
    }
    let history = (actions.len() > 1).then(|| app.state.history.begin_sequence());
    app.input_bindings.replay = true;
    for id in actions {
        match id.as_str() {
            "ignore" => {}
            "native-left" => native_press(app, 0),
            "native-right" => native_press(app, 1),
            "select" => {
                let ctrl = app.ctrl;
                app.ctrl = true;
                native_press(app, 0);
                app.ctrl = ctrl;
            }
            _ => {
                if builtin(app, &id, c) {
                    continue;
                }
                if let Some(cmd) = command(&app.state, c, &id) {
                    let msg = crate::playlist::run(app, cmd);
                    if msg.is_some() {
                        app.state.message = msg;
                    }
                } else {
                    app.state.message = Some(format!("Binding action unavailable: {id}"));
                    break;
                }
            }
        }
        if app.modal_open()
            || app.state.settings.open
            || app.state.playlist.popup.is_some()
            || app.want_exit
        {
            break;
        }
    }
    if let Some(previous) = history {
        app.state.history.end_sequence(previous);
    }
    app.input_bindings.replay = false;
    app.invalidate();
}
pub fn press(app: &mut App, button: usize) -> bool {
    if !app.input_bindings.replay && app.input_bindings.owner.is_some() {
        return true;
    }
    if !available(app) || app.drag.is_some() {
        return false;
    }
    let c = hovered(app);
    if c == 0 {
        return false;
    }
    if !app.top_hit(app.mouse.0, app.mouse.1).is_some_and(|h| {
        matches!(
            h.action,
            Action::PianoGrid
                | Action::PianoNote(_)
                | Action::PlaylistArea
                | Action::MixerArea
                | Action::RackArea
                | Action::RackStepGap
                | Action::Step(..)
                | Action::RackSelect(_)
                | Action::BrowserRow(_)
                | Action::BrowserList
                | Action::Fader(_)
                | Action::Knob(_)
                | Action::Mix(_)
        )
    }) {
        return false;
    }
    let mods = modifiers(app.ctrl, app.alt, app.shift);
    let click = lookup(c, EVENTS[button * 2], mods);
    let drag = lookup(c, EVENTS[button * 2 + 1], mods);
    let multi = button == 0
        && ["Left double click", "Left triple click"]
            .iter()
            .any(|event| lookup(c, event, mods).is_some());
    if click.is_none() && drag.is_none() && !multi {
        return false;
    }
    if let Some(win) = app.top_hit(app.mouse.0, app.mouse.1).and_then(|h| h.win) {
        app.state.raise(win);
    }
    app.clear_binding_press();
    app.input_bindings.owner = Some(button);
    app.input_bindings.pending = Some(Pending {
        button,
        origin: app.mouse,
        click,
        drag,
        context: c,
        modifiers: mods,
    });
    true
}
pub fn motion(app: &mut App, x: f32, y: f32) -> bool {
    if app.input_bindings.replay {
        return false;
    }
    let Some(pending) = app.input_bindings.pending.clone() else {
        return false;
    };
    if (x - pending.origin.0).hypot(y - pending.origin.1) < 5. {
        app.mouse = (x, y);
        return true;
    }
    app.input_bindings.pending = None;
    flush_click(app);
    app.mouse = pending.origin;
    let c = pending.context;
    let old_mods = (app.ctrl, app.alt, app.shift);
    (app.ctrl, app.alt, app.shift) = (
        pending.modifiers & 1 != 0,
        pending.modifiers & 2 != 0,
        pending.modifiers & 4 != 0,
    );
    if let Some(actions) = pending.drag {
        app.input_bindings.gesture = true;
        execute(app, actions, c);
        app.input_bindings.gesture = false;
    } else {
        app.input_bindings.replay = true;
        native_press(app, pending.button);
        app.input_bindings.replay = false;
    }
    app.input_bindings.replay = true;
    app.mouse_move(x, y);
    app.input_bindings.replay = false;
    (app.ctrl, app.alt, app.shift) = old_mods;
    true
}
pub fn release(app: &mut App) -> bool {
    if app.input_bindings.replay {
        return false;
    }
    app.input_bindings.owner = None;
    let Some(pending) = app.input_bindings.pending.take() else {
        return false;
    };
    if pending.button == 0
        && ["Left double click", "Left triple click"]
            .iter()
            .any(|event| lookup(pending.context, event, pending.modifiers).is_some())
    {
        let now = std::time::Instant::now();
        let matches = app.input_bindings.click.as_ref().is_some_and(|last| {
            last.until >= now
                && last.pending.context == pending.context
                && last.pending.modifiers == pending.modifiers
                && (last.pending.origin.0 - pending.origin.0)
                    .hypot(last.pending.origin.1 - pending.origin.1)
                    < 5.
        });
        if !matches {
            flush_click(app);
        }
        let count = app
            .input_bindings
            .click
            .take()
            .map_or(1, |last| last.count + 1);
        let triple = lookup(pending.context, "Left triple click", pending.modifiers).is_some();
        app.input_bindings.click = Some(Click {
            pending,
            count,
            until: now + std::time::Duration::from_millis(350),
        });
        if count == 3 || count == 2 && !triple {
            flush_click(app);
        }
        return true;
    }
    finish_click(app, pending);
    true
}
fn finish_click(app: &mut App, pending: Pending) {
    let position = app.mouse;
    app.mouse = pending.origin;
    let c = pending.context;
    let old_mods = (app.ctrl, app.alt, app.shift);
    (app.ctrl, app.alt, app.shift) = (
        pending.modifiers & 1 != 0,
        pending.modifiers & 2 != 0,
        pending.modifiers & 4 != 0,
    );
    if let Some(actions) = pending.click {
        app.input_bindings.gesture = true;
        execute(app, actions, c);
        app.input_bindings.gesture = false;
    } else {
        app.input_bindings.replay = true;
        native_press(app, pending.button);
        app.input_bindings.replay = false;
    }
    app.input_bindings.replay = true;
    app.mouse_up();
    app.input_bindings.replay = false;
    app.mouse = position;
    (app.ctrl, app.alt, app.shift) = old_mods;
}
fn flush_click(app: &mut App) {
    let Some(mut click) = app.input_bindings.click.take() else {
        return;
    };
    if click.count > 1 {
        click.pending.click = lookup(
            click.pending.context,
            if click.count == 3 {
                "Left triple click"
            } else {
                "Left double click"
            },
            click.pending.modifiers,
        )
        .or(click.pending.click);
    }
    finish_click(app, click.pending);
}
pub fn tick(app: &mut App) {
    if !available(app) {
        app.input_bindings.click = None;
        app.input_bindings.hover = None;
        return;
    }
    let now = std::time::Instant::now();
    if app
        .input_bindings
        .click
        .as_ref()
        .is_some_and(|c| c.until <= now)
        && app.input_bindings.pending.is_none()
    {
        flush_click(app);
    }
    if app.input_bindings.owner.is_some() || app.drag.is_some() || app.state.no_focus {
        return;
    }
    let focus = if app.over_browser() {
        Some(Win::Browser)
    } else {
        app.state.z.last().copied()
    };
    if app.input_bindings.focused != focus {
        let initial = app.input_bindings.focused.is_none();
        app.input_bindings.focused = focus;
        if !initial
            && let Some(actions) = lookup(
                context(focus),
                "Activate window",
                modifiers(app.ctrl, app.alt, app.shift),
            )
        {
            execute(app, actions, context(focus));
            app.input_bindings.focused = app.state.z.last().copied();
            return;
        }
    }
    let Some(action) = app
        .top_hit(app.mouse.0, app.mouse.1)
        .map(|h| h.action.clone())
    else {
        app.input_bindings.hover = None;
        return;
    };
    let c = hovered(app);
    if c == 0 {
        app.input_bindings.hover = None;
        return;
    }
    let same = app
        .input_bindings
        .hover
        .as_ref()
        .is_some_and(|(old, previous, pos, _, _)| {
            *old == c
                && *previous == action
                && (app.mouse.0 - pos.0).hypot(app.mouse.1 - pos.1) < 5.
        });
    if !same {
        app.input_bindings.hover = Some((c, action, app.mouse, now, false));
        return;
    }
    if app
        .input_bindings
        .hover
        .as_ref()
        .is_some_and(|(_, _, _, since, fired)| {
            !fired && now.duration_since(*since) >= std::time::Duration::from_millis(500)
        })
    {
        app.input_bindings.hover.as_mut().unwrap().4 = true;
        if let Some(actions) = lookup(c, "Hover", modifiers(app.ctrl, app.alt, app.shift)) {
            execute(app, actions, c);
        }
    }
}
pub fn accepts_release(app: &App, button: usize) -> bool {
    app.input_bindings.owner.is_none_or(|owner| owner == button)
}
fn is_pointer(id: &str) -> bool {
    matches!(id, "native-left" | "native-right" | "select")
}
pub fn wheel(app: &mut App, _dx: f32, dy: f32, shift: bool, ctrl: bool) -> bool {
    if !available(app) || app.input_bindings.owner.is_some() || dy == 0. {
        return false;
    }
    let c = hovered(app);
    let Some(actions) = lookup(
        c,
        if dy > 0. { "Wheel up" } else { "Wheel down" },
        modifiers(ctrl, app.alt, shift),
    ) else {
        return false;
    };
    if actions.iter().any(|id| is_pointer(id)) {
        return false;
    }
    execute(app, actions, c);
    true
}
fn key_name(key: &Key) -> Option<String> {
    match key {
        Key::Named(
            NamedKey::Control | NamedKey::Alt | NamedKey::Shift | NamedKey::Meta | NamedKey::Super,
        ) => None,
        Key::Character(s) if s.chars().count() == 1 => Some(s.to_uppercase()),
        Key::Named(k) => Some(format!("{k:?}")),
        _ => None,
    }
}
pub fn capture(app: &mut App, key: &Key, ctrl: bool, alt: bool) -> bool {
    let editor = &mut app.state.settings.input_editor;
    if !app.state.settings.open || app.state.settings.tab != CONTROLS {
        return false;
    }
    if editor.naming {
        match key {
            Key::Named(NamedKey::Escape) => editor.naming = false,
            Key::Named(NamedKey::Enter | NamedKey::Tab) => edit(app, Edit::SaveName),
            _ => {
                editor.name.key(key, ctrl, alt);
            }
        }
        app.invalidate();
        return true;
    }
    if editor.searching {
        match key {
            Key::Named(NamedKey::Escape | NamedKey::Enter | NamedKey::Tab) => {
                editor.searching = false
            }
            _ => {
                editor.search.key(key, ctrl, alt);
                editor.page = 0;
            }
        }
        app.invalidate();
        return true;
    }
    if !editor.capture {
        return false;
    }
    if matches!(key, Key::Named(NamedKey::Escape)) {
        editor.capture = false;
        return true;
    }
    if let Some(name) = key_name(key) {
        editor.key = name;
        editor.modifiers = modifiers(ctrl, alt, app.shift);
        editor.capture = false;
        sync_name(editor);
        app.invalidate();
    }
    true
}
pub fn key(app: &mut App, key: &Key, ctrl: bool, alt: bool) -> bool {
    if matches!(key, Key::Named(NamedKey::Escape)) && app.input_bindings.pending.take().is_some() {
        return true;
    }
    if app.input_bindings.owner.is_some() {
        return true;
    }
    if !available(app)
        || app.input_bindings.owner.is_some()
        || app.drag.is_some()
        || app.state.no_focus && !app.over_browser()
    {
        return false;
    }
    let Some(name) = key_name(key) else {
        return false;
    };
    if !ctrl && !alt && matches!(key, Key::Character(_)) && app.browser_takes_typing() {
        return false;
    }
    let c = if app.over_browser() {
        5
    } else {
        context(app.state.z.last().copied())
    };
    let Some(actions) = lookup(c, &format!("Key {name}"), modifiers(ctrl, alt, app.shift)) else {
        return false;
    };
    if actions.iter().any(|id| is_pointer(id)) {
        return false;
    }
    if !app.repeat
        || actions
            .iter()
            .all(|id| BUILTINS.iter().any(|(key, _, _)| key == id) && id != "piano.insert")
    {
        execute(app, actions, c);
    }
    true
}

fn popup(app: &mut App, items: Vec<PItem>) {
    app.state.playlist.popup = Some(Popup {
        x: app.mouse.0,
        y: app.mouse.1,
        items,
        sub: None,
        scroll: [0; 2],
        cascade: Vec::new(),
    });
}
fn pick(label: impl Into<String>, edit: Edit) -> PItem {
    PItem::new(label.into(), Cmd::InputBinding(edit))
}
fn remember(editor: &mut Editor) {
    editor
        .previous
        .push(crate::prefs::text(STORE).unwrap_or_else(|| "[]".into()));
    if editor.previous.len() > 32 {
        editor.previous.remove(0);
    }
}
fn pointer_event(event: &str) -> bool {
    EVENTS[..6].contains(&event) || matches!(event, "Left double click" | "Left triple click")
}
fn valid_actions(event: &str, actions: &[String]) -> bool {
    !actions.is_empty()
        && actions.len() <= 32
        && actions.iter().all(|id| !id.is_empty() && id.len() < 1024)
        && (!actions.iter().any(|id| is_pointer(id))
            || (pointer_event(event) && actions.len() == 1))
}
fn mod_label(m: u8) -> String {
    if m == 0 {
        return "None".into();
    }
    [(1, "Ctrl / Cmd"), (2, "Alt / Option"), (4, "Shift")]
        .into_iter()
        .filter_map(|(bit, name)| (m & bit != 0).then_some(name))
        .collect::<Vec<_>>()
        .join(" + ")
}
impl ActionBrowser {
    fn new(s: &State, context: usize, bindings: Rc<Vec<Binding>>) -> Self {
        let catalog = catalog(s, context, &bindings);
        let global = if context != 0 && catalog.iter().any(|(id, _, _)| id.starts_with("binding:"))
        {
            self::catalog(s, 0, &bindings)
        } else {
            Vec::new()
        };
        let direct: HashSet<&str> = catalog
            .iter()
            .chain(&global)
            .filter(|(id, _, cmd)| direct_action(context, id) || direct_command(cmd.as_ref()))
            .map(|(id, _, _)| id.as_str())
            .collect();
        let items = catalog
            .iter()
            .map(|(id, label, _)| {
                let sequence = direct.contains(id.as_str())
                    || id.starts_with("binding:")
                        && expand_from(
                            &bindings,
                            std::slice::from_ref(id),
                            context,
                            &mut Vec::new(),
                        )
                        .is_ok_and(|ids| ids.iter().all(|id| direct.contains(id.as_str())));
                Choice {
                    id: id.clone(),
                    label: label.clone(),
                    search: label.to_lowercase(),
                    sequence,
                }
            })
            .collect();
        Self {
            context,
            bindings,
            items,
            query: String::new(),
            pointers: false,
            filtered: Vec::new(),
        }
    }
}

fn action_browser(s: &State) -> RefMut<'_, ActionBrowser> {
    let e = &s.settings.input_editor;
    let all = bindings();
    let mut slot = e.browser.borrow_mut();
    let rebuild = slot
        .as_ref()
        .is_none_or(|b| b.context != e.context || !Rc::ptr_eq(&b.bindings, &all));
    if rebuild {
        *slot = Some(ActionBrowser::new(s, e.context, all));
    }
    let b = slot.as_mut().unwrap();
    let pointers = e.event != KEYBOARD && pointer_event(EVENTS[e.event]);
    if rebuild || b.query != e.search.text || b.pointers != pointers {
        b.query.clone_from(&e.search.text);
        b.pointers = pointers;
        let query = b.query.to_lowercase();
        b.filtered.clear();
        b.filtered
            .extend(b.items.iter().enumerate().filter_map(|(i, choice)| {
                ((pointers || !is_pointer(&choice.id))
                    && query
                        .split_whitespace()
                        .all(|term| choice.search.contains(term)))
                .then_some(i)
            }));
    }
    RefMut::map(slot, |slot| slot.as_mut().unwrap())
}

pub fn edit(app: &mut App, action: Edit) {
    if !app.state.settings.open || app.state.settings.tab != CONTROLS {
        return;
    }
    let pinning = matches!(action, Edit::Pin);
    if let Edit::Append(id) = &action {
        let e = &app.state.settings.input_editor;
        let all = bindings();
        let old = current(e, &all);
        if !sequence_safe(&app.state, e.context, id)
            || old.is_some_and(|b| {
                b.actions
                    .iter()
                    .any(|id| !sequence_safe(&app.state, e.context, id))
            })
        {
            app.state.settings.input_editor.notice =
                "Sequences accept direct editing actions. Other actions run alone.".into();
            app.invalidate();
            return;
        }
    }
    if !matches!(action, Edit::Search | Edit::ClearSearch) {
        app.state.settings.input_editor.searching = false;
    }
    if !matches!(action, Edit::Capture) {
        app.state.settings.input_editor.capture = false;
    }
    if !matches!(action, Edit::Name | Edit::SaveName) {
        app.state.settings.input_editor.naming = false;
    }
    let refresh_name = matches!(
        action,
        Edit::SelectContext(_)
            | Edit::SelectEvent(_)
            | Edit::SelectModifiers(_)
            | Edit::Capture
            | Edit::Undo
            | Edit::Remove
    );
    match action {
        Edit::Name => {
            let e = &mut app.state.settings.input_editor;
            e.naming = true;
            e.searching = false;
            e.name.select_all();
        }
        Edit::SaveName | Edit::Pin => {
            let e = &mut app.state.settings.input_editor;
            e.naming = false;
            let mut all = bindings().as_ref().clone();
            let pin_count = all.iter().filter(|b| b.pinned).count();
            if let Some(b) = all.iter_mut().find(|b| {
                b.context == e.context && b.event == event(e) && b.modifiers == e.modifiers
            }) {
                if matches!(action, Edit::Pin) {
                    if !b.pinned && pin_count >= 4 {
                        e.notice =
                            "Unpin a button first; four toolbar buttons are available.".into();
                        app.invalidate();
                        return;
                    }
                    if b.name.is_empty() || b.actions.iter().any(|id| is_pointer(id)) {
                        e.notice = "Name a non-gesture action before pinning it.".into();
                        app.invalidate();
                        return;
                    }
                    b.pinned = !b.pinned;
                } else {
                    b.name = e.name.text.trim().chars().take(80).collect();
                    if b.name.is_empty() {
                        b.pinned = false;
                    }
                }
                remember(e);
                crate::prefs::set_text(STORE, &serde_json::to_string(&all).unwrap());
                e.notice =
                    "Saved. Toolbar buttons can be arranged in the toolbar layout editor.".into();
            } else {
                e.notice = "Assign an action before naming it.".into();
            }
        }
        Edit::Context => popup(
            app,
            CONTEXTS
                .iter()
                .enumerate()
                .map(|(i, s)| pick(*s, Edit::SelectContext(i)))
                .collect(),
        ),
        Edit::Event => popup(
            app,
            EVENTS
                .iter()
                .enumerate()
                .map(|(i, s)| pick(*s, Edit::SelectEvent(i)))
                .collect(),
        ),
        Edit::Modifiers => popup(
            app,
            (0..8)
                .map(|i| pick(mod_label(i), Edit::SelectModifiers(i)))
                .collect(),
        ),
        Edit::SelectContext(c) if c < CONTEXTS.len() => {
            let e = &mut app.state.settings.input_editor;
            e.context = c;
            e.page = 0;
        }
        Edit::SelectEvent(v) if v < EVENTS.len() => {
            let e = &mut app.state.settings.input_editor;
            e.event = v;
            e.page = 0;
        }
        Edit::SelectModifiers(m) if m < 8 => app.state.settings.input_editor.modifiers = m,
        Edit::Capture => {
            let e = &mut app.state.settings.input_editor;
            e.event = KEYBOARD;
            e.capture = true;
            e.page = 0;
        }
        Edit::Search => {
            let e = &mut app.state.settings.input_editor;
            e.searching = true;
            e.search.select_all();
        }
        Edit::ClearSearch => {
            let e = &mut app.state.settings.input_editor;
            e.search.set("");
            e.page = 0;
            e.searching = true;
        }
        Edit::Advanced => app.state.settings.input_editor.advanced ^= true,
        Edit::Page(step) => {
            let max = action_browser(&app.state).filtered.len().saturating_sub(1) / PAGE_SIZE;
            let e = &mut app.state.settings.input_editor;
            e.page = (e.page.min(max) as i32 + step).clamp(0, max as i32) as usize;
        }
        Edit::Saved => {
            let mut items: Vec<_> = bindings()
                .iter()
                .enumerate()
                .map(|(i, b)| {
                    pick(
                        {
                            let trigger = format!(
                                "{} · {} · {}",
                                CONTEXTS[b.context],
                                b.event,
                                mod_label(b.modifiers)
                            );
                            if b.name.is_empty() {
                                trigger
                            } else {
                                format!("{} · {trigger}", b.name)
                            }
                        },
                        Edit::Load(i),
                    )
                })
                .collect();
            if items.is_empty() {
                items.push(PItem::head("No custom bindings saved"));
            }
            popup(app, items);
        }
        Edit::Load(i) => {
            if let Some(b) = bindings().get(i) {
                let e = &mut app.state.settings.input_editor;
                e.context = b.context;
                e.modifiers = b.modifiers;
                e.page = 0;
                e.search.set("");
                e.name.set(&b.name);
                if let Some(key) = b.event.strip_prefix("Key ") {
                    e.event = KEYBOARD;
                    e.key = key.into();
                } else {
                    e.event = EVENTS.iter().position(|v| *v == b.event).unwrap_or(0);
                }
            }
        }
        Edit::Undo => {
            let e = &mut app.state.settings.input_editor;
            if let Some(previous) = e.previous.pop() {
                crate::prefs::set_text(STORE, &previous);
                e.notice = "Previous binding settings restored.".into();
            }
        }
        Edit::Import | Edit::Export => {
            let export = matches!(action, Edit::Export);
            let mode = if export {
                crate::filedlg::Mode::Save
            } else {
                crate::filedlg::Mode::Open
            };
            let mut dlg = crate::filedlg::FileDlg::new(
                mode,
                if export {
                    "Export controls"
                } else {
                    "Import controls (replaces bindings)"
                },
                "input-bindings",
                crate::dialog::Then::InputBindings(export),
            )
            .filter("AQUILA controls", &["json"])
            .name(if export {
                "AQUILA Controls.json"
            } else {
                ""
            });
            if !export {
                dlg = dlg.filter("Lemon action script", &["lsa"]);
            }
            app.file_dialog(dlg);
        }
        change => {
            let editor = &mut app.state.settings.input_editor;
            if editor.event == KEYBOARD && editor.key.is_empty() {
                editor.notice = "Capture a keyboard key first.".into();
                app.invalidate();
                return;
            }
            let mut all = bindings().as_ref().clone();
            let name = event(editor);
            let old = all.iter().position(|b| {
                b.context == editor.context && b.event == name && b.modifiers == editor.modifiers
            });
            let prior = old.map(|i| all.remove(i));
            let mut actions = prior
                .as_ref()
                .map(|b| b.actions.clone())
                .unwrap_or_default();
            match change {
                Edit::Assign(id) => actions = vec![id],
                Edit::Append(id) => actions.push(id),
                Edit::ClearChain => {
                    actions.pop();
                }
                Edit::Remove => actions.clear(),
                _ => return,
            }
            if !actions.is_empty() && !valid_actions(&name, &actions) {
                editor.notice =
                    "Mouse gestures must be used alone, on click or drag events.".into();
                app.invalidate();
                return;
            }
            if !actions.is_empty() {
                all.push(Binding {
                    context: editor.context,
                    event: name.clone(),
                    modifiers: editor.modifiers,
                    actions,
                    name: prior.as_ref().map(|b| b.name.clone()).unwrap_or_default(),
                    pinned: prior.as_ref().is_some_and(|b| b.pinned),
                });
                if !expand_from(
                    &all,
                    &all.last().unwrap().actions,
                    editor.context,
                    &mut vec![binding_id(all.last().unwrap())],
                )
                .is_ok_and(|a| valid_actions(&name, &a))
                {
                    editor.notice="Saved action references must exist, match this context and have no cycles.".into();
                    app.invalidate();
                    return;
                }
            }
            remember(editor);
            crate::prefs::set_text(STORE, &serde_json::to_string(&all).unwrap());
            editor.notice = "Saved for this profile. Restore default removes this override.".into();
        }
    }
    if refresh_name {
        sync_name(&mut app.state.settings.input_editor);
    }
    if pinning
        && has_pinned()
        && !app
            .state
            .bar
            .layout
            .rows
            .iter()
            .flatten()
            .any(|p| *p == crate::toolbar::Panel::CustomActions)
    {
        if let Some(row) = app.state.bar.layout.rows.last_mut() {
            row.push(crate::toolbar::Panel::CustomActions);
            app.state.bar.layout.save();
        }
    }
    app.invalidate();
}
fn label(ui: &mut Ui, rect: Rect, text: &str) {
    let font = Font::new(13);
    ui.c.text_clip(
        font,
        rect.x + 8.,
        rect.y + 7.,
        (rect.w - 16.).max(0.),
        ui.t.text,
        text,
    );
}

fn button(ui: &mut Ui, r: Rect, text: &str, action: Edit) {
    control(ui, r, text, action, true, false);
}

fn control(ui: &mut Ui, r: Rect, text: &str, action: Edit, enabled: bool, focused: bool) {
    let hover = enabled
        && ui.hit(
            r,
            Action::Settings(SAct::InputBindings(action)),
            text.to_string(),
        );
    os_ui::kit::bordered(
        ui.c,
        r,
        1.,
        if focused {
            ui.t.accent
        } else {
            ui.t.toolbar_edge
        },
        os_ui::kit::face(ui.t.toolbar, os_ui::kit::State::hover(hover)),
    );
    let font = Font::new(13);
    let ink = if enabled { ui.t.text } else { ui.t.text_dim };
    ui.c.text_clip(font, r.x + 8., r.y + 7., (r.w - 16.).max(0.), ink, text);
}

pub(crate) fn editor_height(advanced: bool) -> f32 {
    if advanced { 514. } else { 404. }
}

pub fn paint(ui: &mut Ui, s: &State, r: Rect) {
    let e = &s.settings.input_editor;
    let gap = 8.;
    let w = r.w;
    let half = (w - gap) / 2.;
    let rect = |col: f32, row: f32, width: f32| Rect::new(r.x + col, r.y + row, width, 29.);
    let right = half + gap;
    let ready = e.event != KEYBOARD || !e.key.is_empty();
    let browser = action_browser(s);
    let active = current(e, &browser.bindings);

    label(
        ui,
        rect(0., 0., w - 146.),
        "1. Choose a shortcut or gesture",
    );
    button(ui, rect(w - 138., 0., 138.), "Saved bindings…", Edit::Saved);
    button(
        ui,
        rect(0., 33., half),
        &format!("Scope: {}  ▾", CONTEXTS[e.context]),
        Edit::Context,
    );
    button(
        ui,
        rect(right, 33., half),
        &format!("Input: {}  ▾", EVENTS[e.event]),
        Edit::Event,
    );
    button(
        ui,
        rect(0., 66., half),
        &format!("Modifiers: {}  ▾", mod_label(e.modifiers)),
        Edit::Modifiers,
    );
    let capture_label = if e.capture {
        "Press a key… (Esc cancels)".into()
    } else if e.event == KEYBOARD && !e.key.is_empty() {
        format!("Key: {}  ·  Change…", e.key)
    } else {
        "Capture keyboard shortcut…".into()
    };
    control(
        ui,
        rect(right, 66., half),
        &capture_label,
        Edit::Capture,
        true,
        e.capture,
    );

    let effective =
        active.or_else(|| effective_binding(&browser.bindings, e.context, &event(e), e.modifiers));
    let assigned = effective.map_or_else(
        || "Default behavior".into(),
        |b| {
            let friendly = |id: &str| {
                browser
                    .items
                    .iter()
                    .find(|item| item.id == id)
                    .map(|item| {
                        item.label
                            .as_str()
                            .rsplit('/')
                            .next()
                            .unwrap_or(&item.label)
                            .trim()
                    })
                    .unwrap_or_else(|| id.rsplit('/').next().unwrap_or(id))
                    .to_owned()
            };
            let mut summary = b
                .actions
                .iter()
                .take(2)
                .map(|id| friendly(id))
                .collect::<Vec<_>>()
                .join(" → ");
            if b.actions.len() > 2 {
                summary.push_str(&format!("  +{} steps", b.actions.len() - 2));
            }
            if b.name.is_empty() {
                summary
            } else {
                format!("{} · {summary}", b.name)
            }
        },
    );
    let summary = if ready {
        if let Some(inherited) = effective.filter(|b| b.context != e.context) {
            format!("Inherited from {}: {assigned}", CONTEXTS[inherited.context])
        } else {
            format!("Current assignment: {assigned}")
        }
    } else {
        "Current assignment: Choose a key first".into()
    };
    label(ui, rect(0., 104., w - 142.), &summary);
    control(
        ui,
        rect(w - 134., 104., 134.),
        "Restore default",
        Edit::Remove,
        active.is_some(),
        false,
    );

    let clear = !e.search.text.is_empty();
    let search_w = w - if clear { 64. } else { 0. };
    let search = if e.search.text.is_empty() {
        "2. Search actions…".into()
    } else {
        format!("Search: {}", e.search.text)
    };
    control(
        ui,
        rect(0., 139., search_w),
        &search,
        Edit::Search,
        true,
        e.searching,
    );
    if clear {
        button(ui, rect(w - 56., 139., 56.), "Clear", Edit::ClearSearch);
    }

    let count = browser.filtered.len();
    let pages = count.div_ceil(PAGE_SIZE).max(1);
    let page = e.page.min(pages - 1);
    let can_append = e.advanced
        && active.is_some_and(|b| {
            b.actions.len() < 32
                && b.actions.iter().all(|id| {
                    direct_action(e.context, id)
                        || browser
                            .items
                            .iter()
                            .any(|item| &item.id == id && item.sequence)
                })
        });
    for (row, &i) in browser
        .filtered
        .iter()
        .skip(page * PAGE_SIZE)
        .take(PAGE_SIZE)
        .enumerate()
    {
        let choice = &browser.items[i];
        let y = 172. + row as f32 * 33.;
        let append = choice.sequence && can_append;
        let verb = if active.is_some() {
            "Replace"
        } else {
            "Assign"
        };
        control(
            ui,
            rect(0., y, w - if append { 94. } else { 0. }),
            &format!("{verb} · {}", choice.label),
            Edit::Assign(choice.id.clone()),
            ready,
            false,
        );
        if append {
            control(
                ui,
                rect(w - 86., y, 86.),
                "Add step",
                Edit::Append(choice.id.clone()),
                ready,
                false,
            );
        }
    }
    if count == 0 {
        label(
            ui,
            rect(0., 172., w),
            "No matching actions. Try a different search or scope.",
        );
    }
    control(
        ui,
        rect(0., 306., 85.),
        "Previous",
        Edit::Page(-1),
        page > 0,
        false,
    );
    label(
        ui,
        rect(93., 306., w - 186.),
        &format!("Page {} of {pages}  ·  {count} actions", page + 1),
    );
    control(
        ui,
        rect(w - 85., 306., 85.),
        "Next",
        Edit::Page(1),
        page + 1 < pages,
        false,
    );

    button(
        ui,
        rect(0., 339., half),
        if e.advanced {
            "Hide sequence & profile tools"
        } else {
            "Sequence & profile tools  ▸"
        },
        Edit::Advanced,
    );
    control(
        ui,
        rect(right, 339., half),
        "Undo last binding change",
        Edit::Undo,
        !e.previous.is_empty(),
        false,
    );
    label(
        ui,
        rect(0., 371., w),
        if e.notice.is_empty() {
            "Changes save to this profile. Use Undo or Restore default to revert."
        } else {
            &e.notice
        },
    );

    if e.advanced {
        let named = if e.name.text.is_empty() {
            "Name this saved action…".into()
        } else {
            format!("Action name: {}", e.name.text)
        };
        control(
            ui,
            rect(0., 406., w - 100.),
            &named,
            Edit::Name,
            active.is_some(),
            e.naming,
        );
        control(
            ui,
            rect(w - 92., 406., 92.),
            "Save name",
            Edit::SaveName,
            active.is_some(),
            false,
        );
        control(
            ui,
            rect(0., 439., half),
            if active.is_some_and(|b| b.pinned) {
                "Unpin toolbar button"
            } else {
                "Pin as toolbar button"
            },
            Edit::Pin,
            active
                .is_some_and(|b| !b.name.is_empty() && !b.actions.iter().any(|id| is_pointer(id))),
            false,
        );
        control(
            ui,
            rect(right, 439., half),
            "Remove last step",
            Edit::ClearChain,
            active.is_some_and(|b| b.actions.len() > 1),
            false,
        );
        button(
            ui,
            rect(0., 472., half),
            "Import controls or action script…",
            Edit::Import,
        );
        button(
            ui,
            rect(right, 472., half),
            "Export controls…",
            Edit::Export,
        );
    }
}

pub fn file_done(app: &mut App, path: &std::path::Path, export: bool) {
    let result = (|| -> Result<String, String> {
        if export {
            let json = serde_json::to_vec_pretty(bindings().as_ref()).map_err(|e| e.to_string())?;
            std::fs::write(path, json).map_err(|e| e.to_string())?;
            return Ok("Controls exported.".into());
        }
        if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 1_048_576 {
            return Err("Controls file exceeds 1 MB.".into());
        }
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("lsa"))
        {
            let script =
                std::str::from_utf8(&bytes).map_err(|_| "Action scripts must be UTF-8 text.")?;
            let (name, actions) = parse_script(script)?;
            let e = &app.state.settings.input_editor;
            if e.event == KEYBOARD && e.key.is_empty() {
                return Err("Capture the shortcut for this script before importing it.".into());
            }
            if !actions
                .iter()
                .all(|id| sequence_safe(&app.state, e.context, id))
            {
                return Err(
                    "Script uses an unavailable action. Select its editor context first.".into(),
                );
            }
            let b = Binding {
                context: e.context,
                event: event(e),
                modifiers: e.modifiers,
                actions,
                name: name.clone(),
                pinned: false,
            };
            let mut all = bindings().as_ref().clone();
            all.retain(|old| binding_id(old) != binding_id(&b));
            all.push(b);
            remember(&mut app.state.settings.input_editor);
            crate::prefs::set_text(STORE, &serde_json::to_string(&all).unwrap());
            app.state.settings.input_editor.name.set(&name);
            return Ok("Action script imported for the selected event.".into());
        }
        let all: Vec<Binding> =
            serde_json::from_slice(&bytes).map_err(|e| format!("Invalid controls file: {e}"))?;
        let mut seen = std::collections::HashSet::new();
        if all.len() > 2048
            || all.iter().any(|b| {
                b.context >= CONTEXTS.len()
                    || b.modifiers >= 8
                    || !valid_actions(&b.event, &b.actions)
                    || b.name.chars().count() > 80
                    || !(EVENTS.iter().any(|e| *e != "Keyboard" && *e == b.event)
                        || b.event
                            .strip_prefix("Key ")
                            .is_some_and(|k| !k.is_empty() && k.len() < 64))
                    || !seen.insert((b.context, b.event.clone(), b.modifiers))
            })
        {
            return Err("Invalid or duplicate bindings; current controls were kept.".into());
        }
        if all.iter().any(|b| {
            !expand_from(&all, &b.actions, b.context, &mut vec![binding_id(b)])
                .is_ok_and(|a| valid_actions(&b.event, &a))
        }) {
            return Err("Invalid saved-action references; current controls were kept.".into());
        }
        remember(&mut app.state.settings.input_editor);
        crate::prefs::set_text(STORE, &serde_json::to_string(&all).unwrap());
        Ok(format!(
            "Imported {} bindings. Undo change restores your previous controls.",
            all.len()
        ))
    })();
    app.state.settings.input_editor.notice = result.unwrap_or_else(|e| e);
    app.invalidate();
}
/// A finite, portable action script. No shell commands, network or arbitrary code runs on the audio/UI thread.
fn parse_script(text: &str) -> Result<(String, Vec<String>), String> {
    let mut name = "Imported action".to_string();
    let mut actions = Vec::new();
    for (line_no, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(value) = line.strip_prefix("name:") {
            name = value.trim().chars().take(80).collect();
            continue;
        }
        let (count, id) = if let Some(rest) = line.strip_prefix("repeat ") {
            let (count, id) = rest
                .split_once(' ')
                .ok_or_else(|| format!("Line {}: expected repeat COUNT ACTION.", line_no + 1))?;
            let count = count
                .parse::<usize>()
                .map_err(|_| format!("Line {}: invalid repeat count.", line_no + 1))?;
            (count, id.trim())
        } else {
            (1, line)
        };
        if count == 0
            || count > 32
            || actions.len() + count > 32
            || id.is_empty()
            || id.len() > 1024
            || is_pointer(id)
            || id.starts_with("binding:")
        {
            return Err(format!(
                "Line {}: use up to 32 direct editing actions.",
                line_no + 1
            ));
        }
        actions.extend(std::iter::repeat_n(id.to_string(), count));
    }
    if actions.is_empty() {
        return Err("The script has no actions.".into());
    }
    Ok((name, actions))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::piano;
    use os_ui::Canvas;

    fn bind(context: usize, name: &str, actions: &[&str]) {
        let mut all = bindings().as_ref().clone();
        all.push(Binding {
            context,
            event: name.into(),
            modifiers: 0,
            actions: actions.iter().map(|s| s.to_string()).collect(),
            name: String::new(),
            pinned: false,
        });
        crate::prefs::set_text(STORE, &serde_json::to_string(&all).unwrap());
    }
    #[test]
    fn binding_click_drag_cancel_and_undo() {
        let mut app = App::new(None);
        app.state.project = crate::app::test_project();
        app.state.z = vec![Win::PianoRoll];
        app.state.piano.choose_tool(piano::Tool::Draw);
        let mut canvas = Canvas::new(1600, 900, 1.).unwrap();
        app.paint(&mut canvas, None);
        let grid = crate::views::piano::grid_rect(&app.state);
        let at = (grid.x + 100., grid.y + 90.);
        bind(1, "Left drag", &["select"]);
        app.mouse_move(at.0, at.1);
        assert_eq!(app.top_hit(at.0, at.1).unwrap().action, Action::PianoGrid);
        let notes = piano::notes(&app.state).to_vec();
        let history = app.state.history.done();
        app.mouse_down();
        assert_eq!(piano::notes(&app.state), notes);
        app.mouse_move(at.0 + 40., at.1 + 40.);
        app.mouse_up();
        assert_eq!(piano::notes(&app.state), notes);
        assert_eq!(app.state.history.done(), history);
        app.paint(&mut canvas, None);
        app.mouse_move(at.0, at.1);
        app.mouse_down();
        app.key(&Key::Named(NamedKey::Escape), false, false);
        app.mouse_up();
        assert_eq!(piano::notes(&app.state), notes);
        app.mouse_down();
        app.mouse_up();
        assert_eq!(piano::notes(&app.state).len(), notes.len() + 1);
        crate::history::undo(&mut app.state).unwrap();
        assert_eq!(piano::notes(&app.state), notes);
    }
    #[test]
    fn binding_context_and_settings_workflow() {
        let mut app = App::new(None);
        app.state.project = crate::app::test_project();
        bind(0, "Key F3", &["ignore"]);
        bind(1, "Key F3", &["piano.up"]);
        assert_eq!(lookup(1, "Key F3", 0), Some(vec!["piano.up".into()]));
        assert_eq!(lookup(3, "Key F3", 0), Some(vec!["ignore".into()]));
        assert!(lookup(3, "Key F3", 1).is_none());
        app.open_settings(CONTROLS);
        edit(&mut app, Edit::Capture);
        assert!(capture(
            &mut app,
            &Key::Named(NamedKey::Enter),
            false,
            false
        ));
        edit(&mut app, Edit::Assign("ignore".into()));
        assert_eq!(lookup(0, "Key Enter", 0), Some(vec!["ignore".into()]));
        edit(&mut app, Edit::Remove);
        assert!(lookup(0, "Key Enter", 0).is_none());
        let mut canvas = Canvas::new(1280, 900, 1.).unwrap();
        app.paint(&mut canvas, None);
        assert!(
            app.hits
                .iter()
                .any(|h| h.action == Action::Settings(SAct::InputBindings(Edit::Search)))
        );
        if let Some(dir) = std::env::var_os("OS_SHOT_DIR") {
            canvas
                .pixmap
                .save_png(std::path::PathBuf::from(dir).join("input-bindings.png"))
                .unwrap();
        }
    }
    #[test]
    fn binding_owner_modifiers_text_and_mixer_lock() {
        let mut app = App::new(None);
        app.state.project = crate::app::test_project();
        app.state.z = vec![Win::PianoRoll];
        let mut canvas = Canvas::new(1600, 900, 1.).unwrap();
        app.paint(&mut canvas, None);
        let grid = crate::views::piano::grid_rect(&app.state);
        bind(6, "Left click", &["ignore"]);
        bind(1, "Left drag", &["select"]);
        app.mouse_move(grid.x + 100., grid.y + 90.);
        app.mouse_down();
        assert!(accepts_release(&app, 0));
        assert!(!accepts_release(&app, 1));
        app.mouse_down_right();
        assert_eq!(app.input_bindings.pending.as_ref().unwrap().button, 0);
        app.ctrl = true;
        app.mouse_move(grid.x + 140., grid.y + 130.);
        assert!(app.ctrl, "dispatch restores live modifier state");
        app.mouse_up();
        assert!(app.input_bindings.owner.is_none());
        bind(1, "Key F3", &["ignore"]);
        app.ctrl = false;
        assert!(key(&mut app, &Key::Named(NamedKey::F3), false, false));
        app.state.browser_search_focus = true;
        assert!(!key(&mut app, &Key::Named(NamedKey::F3), false, false));
        app.state.browser_search_focus = false;
        app.state.pat_ui.confirm = true;
        assert!(!key(&mut app, &Key::Named(NamedKey::F3), false, false));
        app.state.pat_ui.confirm = false;
        app.state.selected_mixer = 0;
        crate::mixer::track_mut(&mut app.state, 0)
            .unwrap()
            .edit_locked = true;
        let volume = crate::mixer::track(&app.state, 0).unwrap().volume;
        let steps = app.state.history.done();
        execute(&mut app, vec!["mixer.volume-down".into()], 3);
        assert_eq!(crate::mixer::track(&app.state, 0).unwrap().volume, volume);
        assert_eq!(app.state.history.done(), steps);
        crate::mixer::track_mut(&mut app.state, 0)
            .unwrap()
            .edit_locked = false;
        execute(&mut app, vec!["mixer.volume-down".into()], 3);
        assert_ne!(crate::mixer::track(&app.state, 0).unwrap().volume, volume);
        crate::history::undo(&mut app.state).unwrap();
        assert_eq!(crate::mixer::track(&app.state, 0).unwrap().volume, volume);
    }
    #[test]
    fn binding_import_export_restore_and_search() {
        let mut app = App::new(None);
        app.open_settings(CONTROLS);
        edit(&mut app, Edit::SelectContext(1));
        edit(&mut app, Edit::Capture);
        capture(&mut app, &Key::Named(NamedKey::Enter), false, false);
        edit(&mut app, Edit::Assign("select".into()));
        assert!(
            lookup(1, "Key Enter", 0).is_none(),
            "keyboard must not synthesize a held mouse"
        );
        edit(&mut app, Edit::Assign("piano.up".into()));
        edit(&mut app, Edit::Append("piano.right".into()));
        let saved = bindings();
        assert_eq!(saved[0].actions.len(), 2);
        edit(&mut app, Edit::Remove);
        assert!(bindings().is_empty());
        edit(&mut app, Edit::Undo);
        assert_eq!(bindings(), saved);
        let path =
            std::env::temp_dir().join(format!("lemon-controls-test-{}.json", std::process::id()));
        file_done(&mut app, &path, true);
        edit(&mut app, Edit::Remove);
        file_done(&mut app, &path, false);
        assert_eq!(bindings(), saved);
        std::fs::write(
            &path,
            br#"[{"context":0,"event":"Key A","modifiers":0,"actions":["native-left"]}]"#,
        )
        .unwrap();
        file_done(&mut app, &path, false);
        assert_eq!(bindings(), saved);
        std::fs::remove_file(&path).unwrap();
        let items = action_browser(&app.state).items.as_ptr();
        edit(&mut app, Edit::Search);
        capture(
            &mut app,
            &Key::Character("velocity decrease".into()),
            false,
            false,
        );
        {
            let browser = action_browser(&app.state);
            assert_eq!(
                browser.items.as_ptr(),
                items,
                "search reuses the action catalog"
            );
            assert_eq!(browser.filtered.len(), 1);
            assert_eq!(browser.items[browser.filtered[0]].id, "piano.velocity-down");
        }
        let mut canvas = Canvas::new(800, 600, 1.).unwrap();
        app.paint(&mut canvas, None);
        app.paint(&mut canvas, None);
        assert_eq!(
            action_browser(&app.state).items.as_ptr(),
            items,
            "repaints reuse the catalog"
        );
        edit(&mut app, Edit::ClearSearch);
        assert!(app.state.settings.input_editor.search.text.is_empty());
        assert_eq!(action_browser(&app.state).items.as_ptr(), items);
        edit(&mut app, Edit::Advanced);
        assert!(app.state.settings.input_editor.advanced);
        assert!(editor_height(true) > editor_height(false));
        assert!(!app.typing_key(winit::keyboard::KeyCode::KeyA, true, false, false));
        edit(&mut app, Edit::Load(0));
        assert_eq!(app.state.settings.input_editor.key, "Enter");
        assert_eq!(lookup(6, "Key Enter", 0), Some(saved[0].actions.clone()));
    }
    #[test]
    fn binding_multiclick_hover_and_activation_are_deferred_and_cancellable() {
        let mut app = App::new(None);
        app.state.project = crate::app::test_project();
        app.state.z = vec![Win::PianoRoll];
        let mut canvas = Canvas::new(1600, 900, 1.).unwrap();
        app.paint(&mut canvas, None);
        let g = crate::views::piano::grid_rect(&app.state);
        let at = (g.x + 100., g.y + 90.);
        app.mouse_move(at.0, at.1);
        let before = piano::notes(&app.state).len();
        bind(1, "Left double click", &["piano.insert"]);
        app.mouse_down();
        app.mouse_up();
        assert_eq!(piano::notes(&app.state).len(), before);
        app.mouse_down();
        app.mouse_up();
        assert_eq!(piano::notes(&app.state).len(), before + 1);
        assert!(app.input_bindings.click.is_none());
        app.mouse_move(at.0 + 150., at.1);
        app.mouse_down();
        app.mouse_up();
        assert!(cancel(&mut app));
        tick(&mut app);
        assert_eq!(piano::notes(&app.state).len(), before + 1);
        bind(1, "Left triple click", &["ignore"]);
        for _ in 0..3 {
            app.mouse_down();
            app.mouse_up();
        }
        assert_eq!(piano::notes(&app.state).len(), before + 1);
        bind(1, "Hover", &["piano.up"]);
        tick(&mut app);
        let old = piano::notes(&app.state).to_vec();
        app.input_bindings.hover.as_mut().unwrap().3 -= std::time::Duration::from_secs(1);
        tick(&mut app);
        assert_ne!(piano::notes(&app.state), old);
        let hovered = piano::notes(&app.state).to_vec();
        tick(&mut app);
        assert_eq!(piano::notes(&app.state), hovered);
        bind(1, "Activate window", &["piano.down"]);
        app.input_bindings.focused = Some(Win::Playlist);
        tick(&mut app);
        assert_eq!(piano::notes(&app.state), old);
    }
    #[test]
    fn binding_named_actions_scripts_and_visibility() {
        let mut app = App::new(None);
        app.state.project = crate::app::test_project();
        app.state.z = vec![Win::PianoRoll];
        let ch = piano::channel(&app.state).unwrap();
        let i = piano::add_note(&mut app.state, ch, 192, 60);
        app.state.piano.select(app.state.pattern, ch, vec![i]);
        app.open_settings(CONTROLS);
        edit(&mut app, Edit::SelectContext(1));
        edit(&mut app, Edit::Capture);
        capture(&mut app, &Key::Named(NamedKey::F4), false, false);
        edit(&mut app, Edit::Assign("piano.up".into()));
        edit(&mut app, Edit::Append("piano.right".into()));
        app.state.settings.input_editor.name.set("Lift notes");
        edit(&mut app, Edit::SaveName);
        edit(&mut app, Edit::Pin);
        let id = binding_id(&bindings()[0]);
        assert!(has_pinned());
        edit(&mut app, Edit::Assign(id.clone()));
        assert_eq!(bindings()[0].actions.len(), 2, "self-reference rejected");
        app.state.settings.open = false;
        let before = piano::notes(&app.state).to_vec();
        run_saved(&mut app, &id);
        assert_ne!(piano::notes(&app.state), before);
        crate::history::undo(&mut app.state).unwrap();
        assert_eq!(piano::notes(&app.state), before);
        crate::settings_file::set_flag("visible_custom-actions", false);
        app.state.bar.layout.rows[0].push(crate::toolbar::Panel::CustomActions);
        assert!(
            !crate::views::toolbar::visible_layout(&app.state)
                .rows
                .iter()
                .flatten()
                .any(|p| *p == crate::toolbar::Panel::CustomActions)
        );
        run_saved(&mut app, &id);
        assert_ne!(
            piano::notes(&app.state),
            before,
            "hiding buttons preserves actions"
        );
        let script = "# Transpose a fifth\nname: Fifth\nrepeat 7 piano.up\npiano.velocity-down\n";
        let (name, actions) = parse_script(script).unwrap();
        assert_eq!(name, "Fifth");
        assert_eq!(actions.len(), 8);
        assert!(parse_script("repeat 999999 piano.up").is_err());
        assert!(parse_script("native-left").is_err());
        let path =
            std::env::temp_dir().join(format!("lemon-script-test-{}.lsa", std::process::id()));
        std::fs::write(&path, script).unwrap();
        file_done(&mut app, &path, false);
        assert_eq!(bindings()[0].actions, actions);
        assert_eq!(bindings()[0].name, "Fifth");
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn binding_editor_surfaces_and_small_window() {
        let mut app = App::new(None);
        app.state.project = crate::app::test_project();
        let mut canvas = Canvas::new(1600, 900, 1.).unwrap();
        for (c, win) in [
            (2, Win::Playlist),
            (3, Win::Mixer),
            (4, Win::ChannelRack),
            (5, Win::Browser),
        ] {
            app.state.z = vec![win];
            app.state.browser_tab = 0;
            app.state.show_browser = true;
            app.state.no_focus = false;
            app.paint(&mut canvas, None);
            let hit = app
                .hits
                .iter()
                .rev()
                .find(|h| {
                    (c == 5 || h.win == Some(win))
                        && match c {
                            2 => matches!(h.action, Action::PlaylistArea),
                            3 => matches!(h.action, Action::Fader(_) | Action::MixerArea),
                            4 => matches!(h.action, Action::RackSelect(_) | Action::RackArea),
                            _ => matches!(h.action, Action::BrowserRow(_) | Action::BrowserList),
                        }
                })
                .map(|h| h.rect)
                .unwrap_or_else(|| panic!("editor {c} has bindable surface"));
            bind(c, "Left click", &["ignore"]);
            bind(c, "Key F6", &["ignore"]);
            app.mouse_move(hit.cx(), hit.cy());
            assert_eq!(hovered(&app), c);
            assert!(press(&mut app, 0));
            assert!(release(&mut app));
            assert!(key(&mut app, &Key::Named(NamedKey::F6), false, false));
        }
        app.open_settings(CONTROLS);
        let mut small = Canvas::new(800, 600, 1.).unwrap();
        app.paint(&mut small, None);
        assert!(
            app.hits
                .iter()
                .any(|h| h.action == Action::Settings(SAct::InputBindings(Edit::Search)))
        );
        if let Some(dir) = std::env::var_os("OS_SHOT_DIR") {
            small
                .pixmap
                .save_png(std::path::PathBuf::from(dir).join("input-controls-small.png"))
                .unwrap();
        }
    }
}

pub fn cancel(app: &mut App) -> bool {
    let pending =
        app.input_bindings.pending.take().is_some() | app.input_bindings.click.take().is_some();
    app.input_bindings.hover = None;
    let active = app.input_bindings.owner.take().is_some();
    if active && !pending {
        app.input_bindings.replay = true;
        app.mouse_up();
        app.input_bindings.replay = false;
    }
    pending || active
}

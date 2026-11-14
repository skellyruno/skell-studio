//! Reusable, ordered mixer effect recipes for the visual plugin picker.
use super::{Hit, fallback};
use crate::{app::{Action, App, State, Ui}, mixer, plugin_menu::Target, plugins::{self, Plugins}};
use os_ui::{Color, Font, Rect};
use serde::{Deserialize, Serialize};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::path::Path;

pub const FILTER: u8 = 6;
pub const BATCH_VISIBILITY: &str = "visible_effect-chain-batch";
const GROUPS: [&str; 8] = ["All Chains", "Vocals", "Drums", "Bass", "Guitar", "Mix Bus", "Mastering", "My Chains"];
const MAX_CHAIN_BYTES: usize = 32 * 1024 * 1024;

fn wet() -> f64 { 1. }
fn enabled() -> bool { true }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stage {
    name: String,
    uid: Option<String>,
    preset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    state: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default = "enabled")]
    on: bool,
    #[serde(default = "wet")]
    mix: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chain {
    name: String,
    group: usize,
    stages: Vec<Stage>,
    #[serde(default)]
    favorite: bool,
}

#[derive(Clone, Debug, Default)]
pub struct View {
    chains: Vec<Chain>,
    factory_count: usize,
    pub group: usize,
    pub selected: usize,
    selection: Option<Vec<usize>>,
    anchor: Option<usize>,
    pub first: usize,
    draft: Option<(Option<usize>, Chain)>,
    pub rect: Option<Rect>,
    pub focused: bool,
    pub drag_start: Option<Rect>,
}

impl View {
    pub fn load() -> Self {
        use plugins::{LEMON_EQ as EQ, PRESS};
        let factory = [
            ("Vocal Chain – Modern", 1, &[(EQ, "Instruments/Vocal presence"), ("ML Limiter", ""), ("ML Reverb", "Small studio")][..]),
            ("Vocal Chain – Clean", 1, &[(EQ, "Clean-up/Quick low roll-off"), (PRESS, "Mastering/Transparent")]),
            ("Drum Bus – Punchy", 2, &[(EQ, "Instruments/Tight drum sizzle"), ("ML Transient Processor", "Subtle attack"), ("ML Limiter", "Drums/Drum compression")]),
            ("Bass Chain – Warm", 3, &[(EQ, "Clean-up/Cut below 20 Hz and above 18 kHz"), ("ML Fast Distortion", "Warmth")]),
            ("Guitar Chain – Clean", 4, &[("ML Chorus", "Fat clean"), ("ML Reverb", "Small studio")]),
            ("Mix Bus – Glue", 5, &[(PRESS, "Mastering/Slight warmness"), ("ML Soft Clipper", "Default")]),
            ("Mastering Chain – Balanced", 6, &[(EQ, "Clean-up/Cut below 20 Hz and above 18 kHz"), (PRESS, "Mastering/Clear master")]),
        ];
        let favorites: Vec<usize> = crate::prefs::text("effect_chain_favorites")
            .and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default();
        let mut chains: Vec<_> = factory.iter().enumerate().map(|(i, (name, group, stages))| Chain {
            name: (*name).into(),
            group: *group,
            stages: stages.iter().map(|&(name, preset)| Stage {
                name: name.into(), uid: None, preset: (!preset.is_empty()).then(|| preset.into()),
                state: None, label: None, on: true, mix: 1.,
            }).collect(),
            favorite: favorites.contains(&i),
        }).collect();
        let factory_count = chains.len();
        let personal: Vec<Chain> = crate::plugin_db::read_json("effect_chains.json").unwrap_or_default();
        chains.extend(personal.into_iter().filter(valid));
        let rect = crate::prefs::text("effect_chain_panel")
            .and_then(|text| serde_json::from_str::<[f32; 4]>(&text).ok())
            .map(|[x, y, w, h]| Rect::new(x, y, w, h));
        Self { chains, factory_count, rect, ..Self::default() }
    }

    fn save(&self) {
        let favorites: Vec<_> = self.chains[..self.factory_count].iter().enumerate()
            .filter_map(|(i, c)| c.favorite.then_some(i)).collect();
        crate::prefs::set_text("effect_chain_favorites", &serde_json::to_string(&favorites).unwrap());
        crate::plugin_db::write_json("effect_chains.json", &self.chains[self.factory_count..]);
    }

    pub fn matches(&self, query: &str, sort: u8) -> Vec<usize> {
        let query = query.to_lowercase();
        let mut rows: Vec<_> = self.chains.iter().enumerate().filter_map(|(i, c)| {
            let text = format!("{} {} {}", c.name, GROUPS[c.group],
                c.stages.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(" ")).to_lowercase();
            (self.in_group(i, self.group) && query.split_whitespace().all(|q| text.contains(q))).then_some(i)
        }).collect();
        rows.sort_by_cached_key(|&i| {
            let c = &self.chains[i];
            (match sort { 1 => usize::from(!c.favorite), 2 => c.group, _ => 0 }, c.name.to_lowercase())
        });
        rows
    }

    pub fn reset_selection(&mut self) {
        self.selected = 0;
        self.selection = None;
        self.anchor = None;
    }

    fn picked(&self, matches: &[usize]) -> Vec<usize> {
        match &self.selection {
            Some(selection) if crate::workspace_visibility::shown(BATCH_VISIBILITY) =>
                matches.iter().copied().filter(|id| selection.contains(id)).collect(),
            _ => matches.get(self.selected).copied().into_iter().collect(),
        }
    }

    fn select(&mut self, matches: &[usize], row: usize, ctrl: bool, shift: bool) {
        let Some(&id) = matches.get(row) else { return };
        let enabled = crate::workspace_visibility::shown(BATCH_VISIBILITY);
        if shift && enabled {
            let anchor = self.anchor.unwrap_or(self.selected).min(matches.len() - 1);
            self.anchor = Some(anchor);
            let range = &matches[anchor.min(row)..=anchor.max(row)];
            let mut selected = if ctrl { self.picked(matches) } else { Vec::new() };
            for &id in range { if !selected.contains(&id) { selected.push(id); } }
            self.selection = Some(selected);
        } else if ctrl && enabled {
            let mut selected = self.picked(matches);
            if let Some(i) = selected.iter().position(|&i| i == id) { selected.remove(i); }
            else { selected.push(id); }
            self.selection = Some(selected);
            self.anchor = Some(row);
        } else {
            self.selection = None;
            self.anchor = Some(row);
        }
        self.selected = row;
    }

    fn in_group(&self, i: usize, group: usize) -> bool {
        match group {
            0 => true,
            7 => i >= self.factory_count,
            _ => self.chains[i].group == group,
        }
    }
}

fn valid(chain: &Chain) -> bool {
    (1..GROUPS.len()).contains(&chain.group) && !chain.name.trim().is_empty() && chain.name.chars().count() <= 80
        && (1..=63).contains(&chain.stages.len())
        && chain.stages.iter().all(|s| !s.name.trim().is_empty() && s.name.len() <= 512
            && s.uid.as_ref().is_none_or(|id| !id.is_empty() && id.len() <= 1024)
            && s.preset.as_ref().is_none_or(|name| name.len() <= 256)
            && s.mix.is_finite() && (0. ..=1.).contains(&s.mix)
            && s.label.as_ref().is_none_or(|name| name.len() <= 512))
        && chain.stages.iter().filter_map(|s| s.state.as_ref()).map(String::len).sum::<usize>() <= MAX_CHAIN_BYTES - 128 * 1024
}

pub fn import(app: &mut App, path: &Path) -> String {
    let load = || -> Result<Chain, String> {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(path).map_err(|e| e.to_string())?.take(MAX_CHAIN_BYTES as u64 + 1)
            .read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_CHAIN_BYTES { return Err("The chain file is too large".into()); }
        let mut chain: Chain = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        chain.favorite = false;
        if !valid(&chain) || chain.stages.iter().filter_map(|s| s.state.as_ref()).any(|s| STANDARD.decode(s).is_err()) {
            return Err("The chain has an invalid name, effect list, or plugin state".into());
        }
        Ok(chain)
    };
    match load() {
        Ok(chain) => {
            let name = chain.name.clone();
            let v = &mut app.state.plugin_picker;
            let index = v.chains.chains.len();
            v.chains.chains.push(chain);
            v.chains.save();
            v.chains.group = 7;
            v.query.clear();
            v.chains.reset_selection();
            v.chains.selected = v.chains.matches("", v.sort).iter().position(|&i| i == index).unwrap_or(0);
            v.chains.first = 0;
            format!("Imported {name}")
        }
        Err(e) => format!("Could not import effect chain: {e}"),
    }
}

pub fn export(app: &App, index: usize, path: &Path) -> String {
    let Some(chain) = app.state.plugin_picker.chains.chains.get(index) else { return "Select an effect chain".into() };
    let bytes = serde_json::to_vec_pretty(chain).unwrap();
    match std::fs::write(path, bytes) {
        Ok(()) => format!("Exported {}", chain.name),
        Err(e) => format!("Could not export effect chain: {e}"),
    }
}

pub fn delete(app: &mut App, index: usize) {
    let chains = &mut app.state.plugin_picker.chains;
    if (chains.factory_count..chains.chains.len()).contains(&index) {
        chains.chains.remove(index);
        chains.save();
        chains.reset_selection();
        chains.first = 0;
    }
}

pub fn allowed(target: Target) -> bool {
    matches!(target, Target::All | Target::Fx(_))
}

fn resolve(app: &App, stage: &Stage) -> Option<usize> {
    let index = match &stage.uid {
        Some(uid) => app.plugins.effect_index_of_uid(uid),
        None => app.plugins.effect_index(&stage.name),
    }?;
    (!app.state.plugdb.prefs.is_hidden(&app.plugins.effects[index].desc.uid())).then_some(index)
}

fn insert(app: &mut App, chains: &[Chain], track: u16, requested: Option<u8>) -> Result<(), String> {
    if mixer::locked(&app.state, track) { return Err(mixer::LOCK_MESSAGE.into()); }
    let stages: Vec<_> = chains.iter().flat_map(|chain| &chain.stages).collect();
    if stages.is_empty() { return Err("The chain is empty".into()); }
    app.mixer_tick();
    let mixer_track = mixer::track(&app.state, track).ok_or("Select a mixer insert")?;
    let order: Vec<_> = (0..mixer::user_slot_capacity(&app.state, track))
        .map(|i| mixer::visible_user_slot(&app.state, track, i))
        .filter(|&slot| slot >= requested.unwrap_or(0))
        .collect();
    let slots = order.windows(stages.len())
        .find(|slots| slots.iter().all(|&slot| mixer_track.slots[usize::from(slot)].is_none()))
        .ok_or_else(|| format!("Need {} consecutive free effect slots at or after the selected slot", stages.len()))?;
    let mut prepared = Vec::with_capacity(slots.len());
    for &stage in &stages {
        let index = resolve(app, stage).ok_or_else(|| format!("{} is unavailable", stage.name))?;
        let desc = app.plugins.effects[index].desc.clone();
        let preset = stage.preset.as_ref().map(|name| {
            let file = crate::presets::factory(&stage.name).into_iter().find(|p| &p.name == name)
                .ok_or_else(|| format!("Preset {name} is unavailable for {}", stage.name))?;
            crate::presets::load(&file.path, &desc.uid()).map(|(_, body)| body)
        }).transpose()?;
        let saved = stage.state.as_ref().map(|state| STANDARD.decode(state)).transpose()
            .map_err(|_| format!("{} has invalid saved settings", stage.name))?;
        let state = saved.as_deref().or(match &preset {
            Some(crate::presets::Body::State(bytes)) => Some(bytes.as_slice()), _ => None,
        });
        let hosted = app.plugins.prepare_effect(index, state, true)?;
        let mut plugin = hosted.plugin.lock().map_err(|_| "Plugin busy")?;
        if let Some(crate::presets::Body::Params(params)) = preset {
            let missing = Plugins::apply_plugin_params(&mut *plugin, &params);
            if !missing.is_empty() {
                return Err(format!("{} is missing preset parameters: {}", stage.name, missing.join(", ")));
            }
        }
        let state = plugin.save_state().ok_or_else(|| format!("{} could not save its state", stage.name))?;
        drop(plugin);
        let effect = if desc.format == os_host::Format::Native {
            os_model::SlotPlugin { id: os_model::Id::NONE, name: desc.id, state }
        } else {
            let reference = crate::project_plugins::plugin_ref(&desc, state);
            os_model::SlotPlugin { id: os_model::Id::NONE, name: os_flp::wrapper::WRAPPER.into(), state: os_flp::wrapper::encode(None, &reference) }
        };
        prepared.push((hosted, effect));
    }

    app.store_builtin_states();
    crate::project_plugins::store_hosted(&mut app.state.project, &app.plugins)?;
    let before = mixer::channel_copy(&app.state, track).ok_or("Mixer insert is unavailable")?;
    let action = if chains.len() == 1 { "Insert effect chain" } else { "Insert effect chains" };
    crate::history::record(&mut app.state, action, crate::history::Snap::MixerChannel(Box::new(before)));
    for ((&slot, (hosted, effect)), stage) in slots.iter().zip(prepared).zip(stages) {
        let key = plugins::fx_key(track, slot);
        app.plugins.hosted.insert(key, hosted);
        app.plugins.added(key);
        let target = mixer::track_mut(&mut app.state, track).unwrap();
        target.slots[usize::from(slot)] = Some(effect);
        target.slot_on[usize::from(slot)] = stage.on;
        target.slot_mix[usize::from(slot)] = stage.mix;
        target.slots_enabled = true;
        if let Some(label) = &stage.label {
            app.state.mixer.slot_names.insert((track, slot), label.clone());
        } else {
            app.state.mixer.slot_names.remove(&(track, slot));
        }
    }
    app.transport.set_effects(app.plugins.effects());
    app.state.piano.dirty = true;
    Ok(())
}

pub fn load_into(app: &mut App, chains: &[Chain], track: u16, slot: Option<u8>) {
    app.state.message = Some(match insert(app, chains, track, slot) {
        Ok(()) if chains.len() == 1 => format!("Added {}", chains[0].name),
        Ok(()) => format!("Added {} effect chains", chains.len()),
        Err(error) => error,
    });
}

pub fn choose(app: &mut App) {
    let v = &app.state.plugin_picker;
    let matches = v.chains.matches(&v.query, v.sort);
    let chains: Vec<_> = v.chains.picked(&matches).into_iter().map(|i| v.chains.chains[i].clone()).collect();
    if chains.is_empty() { return; }
    let track = app.state.selected_mixer;
    let target = match v.target { Target::Fx(slot) => slot, Target::All => None, _ => return };
    load_into(app, &chains, track, target);
}

pub fn dragged(v: &super::View, row: usize) -> Vec<Chain> {
    let matches = v.chains.matches(&v.query, v.sort);
    let Some(&id) = matches.get(row) else { return Vec::new() };
    let picked = v.chains.picked(&matches);
    let ids = if picked.contains(&id) { picked } else { vec![id] };
    ids.into_iter().map(|i| v.chains.chains[i].clone()).collect()
}

pub fn drag_label(chains: &[Chain]) -> String {
    if let [chain] = chains { chain.name.clone() } else { format!("{} effect chains", chains.len()) }
}

pub fn select_all(v: &mut super::View) {
    if crate::workspace_visibility::shown(BATCH_VISIBILITY) {
        v.chains.selection = Some(v.chains.matches(&v.query, v.sort));
    }
}

pub fn create(app: &mut App, indices: Vec<usize>) {
    let stages: Vec<_> = indices.into_iter().filter_map(|i| app.plugins.effects.get(i)).map(|a| Stage {
        name: a.desc.name.clone(), uid: Some(a.desc.uid()), preset: None,
        state: None, label: None, on: true, mix: 1.,
    }).collect();
    if stages.is_empty() {
        app.state.message = Some("Select one or more effects in the plugin list, then choose Save as chain".into());
        return;
    }
    let v = &mut app.state.plugin_picker;
    v.filter = FILTER;
    v.query.clear();
    v.chains.first = 0;
    v.chains.focused = true;
    v.chains.draft = Some((None, Chain { name: "New Chain".into(), group: 7, stages, favorite: false }));
}

pub fn capture(app: &mut App, track: u16) -> Option<String> {
    app.mixer_tick();
    app.sync_effects();
    let result = (|| -> Result<Chain, String> {
        let channel = mixer::track(&app.state, track).ok_or("Mixer channel is unavailable")?;
        let mut stages = Vec::new();
        for i in 0..mixer::user_slot_capacity(&app.state, track) {
            let slot = mixer::visible_user_slot(&app.state, track, i);
            let k = usize::from(slot);
            let Some(effect) = &channel.slots[k] else { continue };
            let key = plugins::fx_key(track, slot);
            let (desc, bytes) = if let Some(desc) = app.plugins.descriptor(key) {
                let bytes = app.plugins.state(key).ok_or_else(|| format!("{} could not save its settings", desc.name))?;
                (desc, bytes)
            } else if let Some(saved) = crate::project_plugins::wrapped_slot(effect) {
                let plugin = app.plugins.effects.iter().find(|p| {
                    p.desc.name == saved.name && p.desc.format.as_str() == saved.format
                        && (saved.id.is_empty() || p.desc.id.eq_ignore_ascii_case(&saved.id))
                }).ok_or_else(|| format!("{} is unavailable", saved.name))?;
                (plugin.desc.clone(), saved.state)
            } else if let Some(index) = crate::project_plugins::find_native(&app.plugins.effects, &effect.name) {
                (app.plugins.effects[index].desc.clone(), effect.state.clone())
            } else {
                return Err(format!("{} must finish loading before its settings can be saved", effect.name));
            };
            stages.push(Stage { uid: Some(desc.uid()), name: desc.name, preset: None, state: Some(STANDARD.encode(bytes)),
                label: app.state.mixer.slot_names.get(&(track, slot)).cloned(), on: channel.slot_on[k], mix: channel.slot_mix[k] });
        }
        if stages.is_empty() { return Err("This channel has no effects in its plugin slots".into()); }
        let name = mixer::name(&app.state, track).chars().take(80).collect();
        let chain = Chain { name, group: 7, stages, favorite: false };
        if !valid(&chain) { return Err("The effect chain is too large or contains invalid settings".into()); }
        Ok(chain)
    })();
    match result {
        Ok(chain) => {
            if !app.state.plugin_picker.open || !allowed(app.state.plugin_picker.target) {
                super::open(app, Target::Fx(None));
            }
            let v = &mut app.state.plugin_picker;
            v.open = true;
            v.minimized = false;
            v.filter = FILTER;
            v.query.clear();
            v.chains.first = 0;
            v.chains.focused = true;
            v.chains.draft = Some((None, chain));
            app.invalidate();
            None
        }
        Err(e) => Some(e),
    }
}

pub fn act(app: &mut App, hit: Hit) {
    let v = &mut app.state.plugin_picker;
    let c = &mut v.chains;
    let matches = c.matches(&v.query, v.sort);
    let picked = c.picked(&matches);
    let current = (picked.len() == 1).then(|| picked[0]);
    match hit {
        Hit::ChainGroup(group) => { c.group = group; c.reset_selection(); c.first = 0; }
        Hit::ChainTile(row) => c.select(&matches, row, app.ctrl, app.shift),
        Hit::ChainStar(i) => {
            c.reset_selection();
            c.chains[i].favorite = !c.chains[i].favorite;
            c.save();
            c.selected = c.matches(&v.query, v.sort).iter().position(|&id| id == i).unwrap_or(0);
        }
        Hit::ChainNew => {
            v.filter = 0;
            v.query.clear();
            v.first = 0;
            v.selected = 0;
            app.state.message = Some("Select effects with Ctrl/Cmd-click, then choose Save as chain".into());
        }
        Hit::ChainEdit | Hit::ChainDuplicate => {
            let Some(i) = current else { return };
            let mut draft = c.chains[i].clone();
            let original = (hit == Hit::ChainEdit && i >= c.factory_count).then_some(i);
            if original.is_none() {
                draft.name = format!("{} Copy", draft.name).chars().take(80).collect();
                draft.favorite = false;
            }
            c.draft = Some((original, draft));
            c.first = 0;
        }
        Hit::ChainCategory => {
            if let Some((_, draft)) = &mut c.draft { draft.group = draft.group % (GROUPS.len() - 1) + 1; }
        }
        Hit::ChainDelete => {
            if let Some(i) = current.filter(|&i| i >= c.factory_count) {
                use crate::dialog::{Answer, Kind, Msg, Then};
                let name = &c.chains[i].name;
                let msg = Msg::new(Kind::Question, "Delete effect chain", vec![(format!("Delete “{name}” from My Chains?"), false)],
                    &[Answer::Yes, Answer::No], Then::DeleteEffectChain(i));
                app.show_msg(msg);
            }
        }
        Hit::ChainImport | Hit::ChainExport => {
            use crate::{dialog::Then, filedlg::{FileDlg, Mode}};
            let dlg = if hit == Hit::ChainImport {
                FileDlg::new(Mode::Open, "Import effect chain", "effect chain import", Then::ImportEffectChain)
            } else {
                let Some(i) = current else { return };
                FileDlg::new(Mode::Save, "Export effect chain", "effect chain export", Then::ExportEffectChain(i))
                    .name("Effect Chain.lemonchain")
            };
            app.file_dialog(dlg.filter("Skell Studio effect chain", &["lemonchain"]));
        }
        Hit::ChainCancel => { c.draft = None; }
        Hit::ChainSave => {
            let Some((index, chain)) = c.draft.take() else { return };
            if !valid(&chain) {
                c.draft = Some((index, chain));
                app.state.message = Some("Enter a chain name and keep at least one effect".into());
                return;
            }
            let i = index.unwrap_or(c.chains.len());
            if let Some(index) = index { c.chains[index] = chain; } else { c.chains.push(chain); }
            c.save();
            c.group = 7;
            v.query.clear();
            c.reset_selection();
            c.selected = c.matches("", v.sort).iter().position(|&id| id == i).unwrap_or(0);
            c.first = 0;
        }
        Hit::ChainMove(i, direction) => {
            if let Some((_, draft)) = &mut c.draft {
                let to = i.saturating_add_signed(direction as isize);
                if i < draft.stages.len() && to < draft.stages.len() { draft.stages.swap(i, to); }
            }
        }
        Hit::ChainRemove(i) => {
            if let Some((_, draft)) = &mut c.draft && draft.stages.len() > 1 && i < draft.stages.len() {
                draft.stages.remove(i);
            }
        }
        _ => {}
    }
}

pub fn editing(v: &super::View) -> bool { v.chains.draft.is_some() }

pub fn clear_name(v: &mut super::View) {
    if let Some((_, draft)) = &mut v.chains.draft { draft.name.clear(); }
}

pub fn scroll(v: &mut super::View, delta: isize, cols: usize, rows: usize) {
    let c = &mut v.chains;
    let limit = if let Some((_, draft)) = &c.draft {
        draft.stages.len().saturating_sub(1)
    } else {
        c.matches(&v.query, v.sort).len().div_ceil(cols).saturating_sub(rows)
    };
    c.first = c.first.saturating_add_signed(delta).min(limit);
}

pub fn navigate(v: &mut super::View, delta: isize, cols: usize, rows: usize) {
    let c = &mut v.chains;
    let matches = c.matches(&v.query, v.sort);
    c.select(&matches, c.selected.saturating_add_signed(delta).min(matches.len().saturating_sub(1)), false, false);
    let row = c.selected / cols;
    c.first = c.first.clamp(row.saturating_sub(rows - 1), row);
}

pub fn name_input(v: &mut super::View, text: Option<&str>) {
    if let Some((_, draft)) = &mut v.chains.draft {
        if let Some(text) = text {
            let room = 80usize.saturating_sub(draft.name.chars().count());
            draft.name.extend(text.chars().filter(|c| !c.is_control()).take(room));
        } else {
            draft.name.pop();
        }
    }
}

const ACCENT: Color = Color::rgb(0x2096f3);

fn button(ui: &mut Ui, r: Rect, label: &str, hit: Hit, active: bool) {
    let hover = ui.hit(r, Action::PluginPicker(hit), label.to_owned());
    let background = if active { ACCENT } else if hover { Color::rgb(0x383a3e) } else { Color::rgb(0x292b2e) };
    ui.c.fill_rrect(r, r.h * 0.42, background);
    ui.c.text_center(Font::light(12, 0).natural(), r, if active { Color::rgb(0x171817) } else { ui.t.text }, label);
}

pub fn layout(r: Rect) -> (f32, usize, usize) {
    let sidebar = if r.w >= 740. { 186. } else { 114. };
    let cols = if r.w >= 880. { 2 } else { 1 };
    let rows = ((r.h - 205.) / 138.).floor().max(1.) as usize;
    (sidebar, cols, rows)
}

pub fn panel_rect(s: &State, r: Rect) -> Rect {
    let (w, h) = s.screen;
    let w = w.max(1.);
    let h = h.max(1.);
    let width = r.w.clamp(680_f32.min(w), w);
    let height = r.h.clamp(540_f32.min(h), h);
    Rect::new(r.x.clamp(0., w - width), r.y.clamp(0., h - height), width, height)
}

pub fn paint(ui: &mut Ui, s: &State, r: Rect) {
    let v = &s.plugin_picker;
    let c = &v.chains;
    let (sidebar, cols, rows) = layout(r);
    let x = r.x + 30. + sidebar;
    let w = (r.right() - 18. - x).max(1.);
    let y = r.y + 105.;
    let bottom = r.bottom() - 83.;
    let matches = c.matches(&v.query, v.sort);
    let picked = c.picked(&matches);
    let selected = (picked.len() == 1).then(|| picked[0]);
    ui.c.push_clip(Rect::new(r.x + 16., y, r.w - 32., (bottom - y).max(0.)));
    for (group, label) in GROUPS.into_iter().enumerate() {
        if y + group as f32 * 32. + 28. > bottom { break; }
        let count = (0..c.chains.len()).filter(|&i| c.in_group(i, group)).count();
        let row = Rect::new(r.x + 18., y + group as f32 * 32., sidebar, 28.);
        let hover = ui.hit(row, Action::PluginPicker(Hit::ChainGroup(group)), label);
        if group == c.group || hover {
            ui.c.fill_rrect(row, 7., if group == c.group { Color::rgb(0x3a3520) } else { Color::rgb(0x28292c) });
        }
        if group == c.group { ui.c.stroke_rrect(row, 7., 1., ACCENT.with_alpha(140)); }
        ui.c.text_clip(Font::light(13, 0).natural(), row.x + 10., row.y + 6., row.w - 35., ui.t.text, label);
        ui.c.text_center(Font::light(11, 0).natural(), Rect::new(row.right() - 28., row.y, 25., row.h),
            ui.t.text_dim, &count.to_string());
    }
    if let Some((_, draft)) = &c.draft {
        ui.c.text_clip(Font::new(13).bold(), x, y, w, ui.t.text_dim, "CHAIN NAME · TYPE TO EDIT");
        let name = Rect::new(x, y + 23., (w - 125.).max(1.), 32.);
        ui.c.stroke_rrect(name, 6., 1., ACCENT);
        ui.c.text_clip(Font::new(15), x + 10., y + 31., name.w - 20., ui.t.text, &draft.name);
        button(ui, Rect::new(x + w - 119., y + 23., 119., 32.), GROUPS[draft.group], Hit::ChainCategory, false);
        for (i, stage) in draft.stages.iter().enumerate().skip(c.first) {
            let sy = y + 62. + (i - c.first) as f32 * 30.;
            if sy + 28. > bottom { break; }
            ui.c.text_clip(Font::new(13), x, sy + 6., (w - 107.).max(0.), ui.t.text, &stage.name);
            for (offset, label, hit) in [(102., "↑", Hit::ChainMove(i, -1)), (68., "↓", Hit::ChainMove(i, 1)), (34., "×", Hit::ChainRemove(i))] {
                button(ui, Rect::new(x + w - offset, sy, 29., 25.), label, hit, false);
            }
        }
    } else {
        ui.c.text_clip(Font::new(16).bold(), x, y, w, ui.t.text, &format!("Effect Chains  ·  {}", matches.len()));
        if matches.is_empty() { ui.c.text_clip(Font::new(13), x, y + 47., w, ui.t.text_dim, "No matching chains"); }
        let width = (w - (cols - 1) as f32 * 12.) / cols as f32;
        let first = c.first.min(matches.len().div_ceil(cols).saturating_sub(rows));
        for (n, &index) in matches.iter().skip(first * cols).take(rows * cols).enumerate() {
            let chain = &c.chains[index];
            let card = Rect::new(x + (n % cols) as f32 * (width + 12.), y + 29. + (n / cols) as f32 * 138., width, 126.);
            if card.bottom() > bottom { break; }
            let row = first * cols + n;
            let chosen = picked.contains(&index);
            let hover = ui.hit(card, Action::PluginPicker(Hit::ChainTile(row)),
                if crate::workspace_visibility::shown(BATCH_VISIBILITY) {
                    format!("{} · Ctrl/Cmd-click to toggle, Shift-click for a range, drag to Mixer", chain.name)
                } else { chain.name.clone() });
            ui.c.fill_rect_gradient(card, Color::rgb(0x252629), Color::rgb(0x1b1c1e), 9.);
            if hover || chosen { ui.c.stroke_rrect(card, 9., if chosen { 2. } else { 1. }, ACCENT); }
            ui.c.text_clip(Font::light(16, 0).natural().bold(), card.x + 12., card.y + 10., width - 58., ui.t.text, &chain.name);
            button(ui, Rect::new(card.right() - 38., card.y + 5., 28., 26.),
                if chain.favorite { "★" } else { "☆" }, Hit::ChainStar(index), chain.favorite);
            let count = chain.stages.len().min(4);
            let tile = (width - 24.) / count as f32;
            for (i, stage) in chain.stages.iter().take(count).enumerate() {
                let px = card.x + 12. + i as f32 * tile;
                let preview = Rect::new(px, card.y + 34., tile - 16., 58.);
                let entry = s.plugdb.lists.fx.iter().find(|e| match &stage.uid {
                    Some(uid) => &e.uid == uid, None => e.name == stage.name,
                });
                ui.c.push_clip(preview);
                if let Some(image) = entry.and_then(|e| v.images.get(&e.uid)) {
                    crate::plugin_artwork::draw(ui, preview, image);
                } else { fallback(ui, preview, true); }
                ui.c.pop_clip();
                if i + 1 < count {
                    ui.c.text_center(Font::new(15), Rect::new(preview.right(), preview.y, 16., preview.h), ui.t.text_dim, "›");
                }
                let pill = Rect::new(px + 2., card.y + 99., (tile - 20.).min(124.), 18.);
                ui.c.fill_rrect(pill, 9., Color::rgb(0x303135));
                ui.c.text_clip(Font::light(10, 0).natural(), pill.x + 7., pill.y + 3., pill.w - 12.,
                    ui.t.text, stage.name.trim_start_matches("Lemon "));
            }
        }
    }
    ui.c.pop_clip();
    let footer = Rect::new(x, bottom + 8., w, 60.);
    ui.c.fill_rrect(footer, 8., Color::rgb(0x1d1e20));
    if c.draft.is_some() {
        button(ui, Rect::new(footer.right() - 154., footer.y + 15., 70., 30.), "Save", Hit::ChainSave, true);
        button(ui, Rect::new(footer.right() - 77., footer.y + 15., 70., 30.), "Cancel", Hit::ChainCancel, false);
    } else {
        button(ui, Rect::new(r.x + 18., footer.y - 26., sidebar, 27.), "+ Create New Chain", Hit::ChainNew, true);
        button(ui, Rect::new(r.x + 18., footer.y + 5., sidebar, 27.), "Import Chain…", Hit::ChainImport, false);
        if selected.is_some() {
            button(ui, Rect::new(r.x + 18., footer.y + 36., sidebar, 27.), "Export Selected…", Hit::ChainExport, false);
        }
        if picked.len() > 1 {
            let effects: usize = picked.iter().map(|&i| c.chains[i].stages.len()).sum();
            ui.c.text_clip(Font::new(14).bold(), x + 10., footer.y + 8., (w - 100.).max(0.), ui.t.text,
                &format!("{} chains selected", picked.len()));
            ui.c.text_clip(Font::new(12), x + 10., footer.y + 31., (w - 100.).max(0.), ui.t.text_dim,
                &format!("{effects} effects · {}", mixer::name(s, s.selected_mixer)));
            button(ui, Rect::new(footer.right() - 80., footer.y + 15., 72., 30.), "Choose", Hit::Choose, true);
        }
        if let Some(index) = selected {
            let chain = &c.chains[index];
            let personal = index >= c.factory_count;
            let buttons = if personal { 311. } else { 235. };
            ui.c.text_clip(Font::light(14, 0).natural().bold(), x + 10., footer.y + 8., (w - buttons - 16.).max(0.), ui.t.text, &chain.name);
            ui.c.text_clip(Font::new(12), x + 10., footer.y + 31., (w - buttons - 16.).max(0.), ui.t.text_dim,
                &format!("{}  ·  {} effects  ·  {}", GROUPS[chain.group], chain.stages.len(), mixer::name(s, s.selected_mixer)));
            let right = footer.right();
            button(ui, Rect::new(right - 80., footer.y + 15., 72., 30.), "Choose", Hit::Choose, true);
            button(ui, Rect::new(right - 162., footer.y + 15., 74., 30.), "Duplicate", Hit::ChainDuplicate, false);
            button(ui, Rect::new(right - 235., footer.y + 15., 65., 30.), if personal {"Edit"} else {"Customize"}, Hit::ChainEdit, false);
            if personal {
                button(ui, Rect::new(right - 311., footer.y + 15., 68., 30.), "Delete", Hit::ChainDelete, false);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_chains_picker_batch_choose_and_drop() {
        use super::super as picker;
        use crate::app::{Drag, Win};
        use crate::views::browser::Item;

        let mut app = App::new(None);
        app.state.project = crate::app::test_project();
        app.plugins = Plugins::builtins();
        app.state.plugdb.rebuild(&app.plugins);
        app.state.z = vec![Win::Mixer];
        app.state.mixer.layout = mixer::WIDE;
        app.state.selected_mixer = 2;
        let mut canvas = os_ui::Canvas::new(1500, 910, 1.).unwrap();
        let hit = |app: &App, action| app.hits.iter().find(|h| h.action == action).unwrap().rect;
        let click = |app: &mut App, which| {
            let r = hit(app, Action::PluginPicker(which));
            app.mouse_move(r.cx(), r.cy());
            app.mouse_down();
            app.mouse_up();
        };
        let slot_names = |app: &App, track| {
            mixer::track(&app.state, track).unwrap().slots.iter().flatten()
                .filter_map(crate::project_plugins::wrapped_slot).map(|p| p.name).collect::<Vec<_>>()
        };

        picker::open(&mut app, Target::Fx(None));
        picker::act(&mut app, Hit::Filter(FILTER));
        app.paint(&mut canvas, None);
        click(&mut app, Hit::ChainTile(0));
        app.shift = true;
        click(&mut app, Hit::ChainTile(2));
        app.shift = false;
        app.ctrl = true;
        click(&mut app, Hit::ChainTile(1));
        app.ctrl = false;
        let v = &app.state.plugin_picker;
        let matches = v.chains.matches(&v.query, v.sort);
        assert_eq!(v.chains.picked(&matches), vec![matches[0], matches[2]]);
        let names: Vec<_> = [matches[0], matches[2]].into_iter()
            .flat_map(|i| v.chains.chains[i].stages.iter().map(|s| s.name.clone())).collect();
        app.paint(&mut canvas, None);
        click(&mut app, Hit::Choose);
        assert_eq!(slot_names(&app, 2), names);
        assert_eq!(crate::history::undo(&mut app.state).as_deref(), Some("Insert effect chains"));

        // Grabbing either selected card carries the whole batch in the displayed order.
        let card = hit(&app, Action::PluginPicker(Hit::ChainTile(2)));
        app.mouse_move(card.cx(), card.cy());
        app.mouse_down();
        app.mouse_move(card.cx() + 12., card.cy());
        assert!(matches!(&app.drag, Some(Drag::Browser { item: Item::EffectChains(chains), .. }) if chains.len() == 2));
        app.paint(&mut canvas, None);
        let strip = hit(&app, Action::Mix(mixer::MHit::Strip(4)));
        app.mouse_move(strip.cx(), strip.cy());
        app.mouse_up();
        assert_eq!(slot_names(&app, 4), names);
        assert!(slot_names(&app, 2).is_empty());

        picker::open(&mut app, Target::All);
        picker::act(&mut app, Hit::Filter(FILTER));
        app.paint(&mut canvas, None);
        let card = hit(&app, Action::PluginPicker(Hit::ChainTile(1)));
        app.mouse_move(card.cx(), card.cy());
        app.mouse_down();
        app.mouse_move(card.cx() + 12., card.cy());
        assert!(matches!(&app.drag, Some(Drag::Browser { item: Item::EffectChains(chains), .. }) if chains.len() == 1));
        app.paint(&mut canvas, None);
        let strip = hit(&app, Action::Mix(mixer::MHit::Strip(3)));
        app.mouse_move(strip.cx(), strip.cy());
        app.mouse_up();
        assert!(!slot_names(&app, 3).is_empty());
    }

    #[test]
    fn effect_chains_picker_inserts_factory_recipes_and_undoes_the_whole_chain() {
        let mut app = App::new(None);
        app.state.project = crate::app::test_project();
        app.plugins = Plugins::builtins();
        app.state.plugdb.rebuild(&app.plugins);
        app.state.selected_mixer = 2;
        let existing = plugins::new_slot("Keep this effect");
        app.state.project.mixer[2].slots[1] = Some(existing.clone());
        super::super::open(&mut app, Target::Fx(Some(0)));
        super::super::act(&mut app, Hit::Filter(FILTER));
        let mut canvas = os_ui::Canvas::new(1500, 910, 1.).unwrap();
        app.paint(&mut canvas, None);
        assert!(app.hits.iter().any(|h| h.action == Action::PluginPicker(Hit::ChainTile(0))));
        if let Some(path) = std::env::var_os("LEMON_CHAIN_SHOT") {
            canvas.pixmap.save_png(path).unwrap();
        }
        let chains = View::load();
        for chain in &chains.chains[..chains.factory_count] {
            insert(&mut app, std::slice::from_ref(chain), 2, Some(0)).unwrap_or_else(|e| panic!("{}: {e}", chain.name));
            let slots = &app.state.project.mixer[2].slots;
            assert_eq!(slots[1].as_ref(), Some(&existing));
            assert!(slots[0].is_none());
            let names: Vec<_> = slots.iter().enumerate().filter(|(i, _)| *i != 1)
                .filter_map(|(_, effect)| effect.as_ref().and_then(crate::project_plugins::wrapped_slot))
                .map(|effect| effect.name).collect();
            assert_eq!(names, chain.stages.iter().map(|s| s.name.clone()).collect::<Vec<_>>());
            assert_eq!(crate::history::undo(&mut app.state).as_deref(), Some("Insert effect chain"));
            app.mixer_tick();
            assert_eq!(app.state.project.mixer[2].slots.iter().flatten().count(), 1);
        }
        let mut unavailable = chains.chains[0].clone();
        unavailable.stages[1].name = "Missing effect".into();
        assert!(insert(&mut app, &[unavailable], 2, Some(0)).is_err());
        assert_eq!(app.state.project.mixer[2].slots.iter().flatten().count(), 1);
    }

    #[test]
    fn effect_chains_picker_captures_mixer_state_and_floats() {
        use super::super as picker;
        let mut app = App::new(None);
        app.state.project = crate::app::test_project();
        app.plugins = Plugins::builtins();
        app.state.plugdb.rebuild(&app.plugins);
        app.state.selected_mixer = 2;
        let factory = View::load();
        insert(&mut app, &factory.chains[..1], 2, None).unwrap();
        let source = plugins::fx_key(2, 0);
        let original = app.plugins.state(source).unwrap();
        app.state.project.mixer[2].slot_on[0] = false;
        app.state.project.mixer[2].slot_mix[0] = 0.37;
        app.state.mixer.slot_names.insert((2, 0), "Lead EQ".into());
        assert_eq!(mixer::run(&mut app, mixer::MCmd::SaveEffectChain(2)), None);
        assert!(editing(&app.state.plugin_picker));
        clear_name(&mut app.state.plugin_picker);
        name_input(&mut app.state.plugin_picker, Some("Lead vocal"));
        picker::act(&mut app, Hit::ChainCategory);
        picker::act(&mut app, Hit::ChainSave);
        let c = &app.state.plugin_picker.chains;
        assert_eq!(c.chains.last().unwrap().group, 1);
        assert_eq!(c.matches("lead vocal", 0).len(), 1);
        let path = std::env::temp_dir().join(format!("lemon-effect-chain-{}.lemonchain", std::process::id()));
        assert_eq!(export(&app, c.chains.len() - 1, &path), "Exported Lead vocal");
        assert_eq!(import(&mut app, &path), "Imported Lead vocal");
        std::fs::remove_file(path).unwrap();
        app.state.selected_mixer = 3;
        picker::act(&mut app, Hit::Choose);
        assert!(app.state.plugin_picker.open);
        assert_eq!(app.plugins.state(plugins::fx_key(3, 0)), Some(original));
        assert!(!app.state.project.mixer[3].slot_on[0]);
        assert_eq!(app.state.project.mixer[3].slot_mix[0], 0.37);
        assert_eq!(app.state.mixer.slot_names.get(&(3, 0)).map(String::as_str), Some("Lead EQ"));

        let mut canvas = os_ui::Canvas::new(1200, 800, 1.).unwrap();
        app.state.plugin_picker.chains.rect = Some(Rect::new(70., 70., 810., 620.));
        app.paint(&mut canvas, None);
        let hit = |app: &App, which| app.hits.iter().find(|h| h.action == Action::PluginPicker(which)).unwrap().rect;
        let title = hit(&app, Hit::ChainMovePanel);
        app.mouse = (title.cx(), title.cy());
        assert!(picker::mouse_down(&mut app));
        assert!(picker::mouse_move(&mut app, title.cx() + 50., title.cy() + 20.));
        assert!(picker::mouse_up(&mut app));
        assert_eq!(app.state.plugin_picker.chains.rect.unwrap().x, 120.);
        app.paint(&mut canvas, None);
        let grip = hit(&app, Hit::ChainResizePanel);
        app.mouse = (grip.cx(), grip.cy());
        assert!(picker::mouse_down(&mut app));
        assert!(picker::mouse_move(&mut app, grip.cx() - 100., grip.cy() - 70.));
        assert!(picker::mouse_up(&mut app));
        assert_eq!(app.state.plugin_picker.chains.rect.unwrap().w, 710.);
        app.mouse = (20., 760.);
        assert!(!picker::mouse_down(&mut app));
        assert!(!picker::mouse_up(&mut app));
        assert!(!app.state.plugin_picker.chains.focused);
    }
}

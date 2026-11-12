// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-input control-surface modes. MCU note numbers follow the Control Surface
//! library's published MCU constants. Select MCU on the controller's DAW port.
use crate::app::{Action, App};
use crate::midi_in::{Ev, Msg};
use crate::playlist::{Cmd, Mark, PItem, Popup};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    Play,
    Stop,
    Record,
    LoopRecord,
    Metronome,
    Previous,
    Next,
    Mute,
    Solo,
    Arm,
}

impl Transport {
    pub const ALL: [Self; 10] = [
        Self::Play, Self::Stop, Self::Record, Self::LoopRecord, Self::Metronome,
        Self::Previous, Self::Next, Self::Mute, Self::Solo, Self::Arm,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Play => "Play",
            Self::Stop => "Stop",
            Self::Record => "Record",
            Self::LoopRecord => "Loop record",
            Self::Metronome => "Metronome",
            Self::Previous => "Prev. insert",
            Self::Next => "Next insert",
            Self::Mute => "Mute insert",
            Self::Solo => "Solo insert",
            Self::Arm => "Arm insert",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Binding {
    Note { ch: u8, key: u8 },
    Cc { ch: u8, n: u8 },
}

impl Binding {
    fn from_message(msg: Msg) -> Option<(Self, bool)> {
        Some(match msg {
            Msg::On { ch, key, vel } => (Self::Note { ch, key }, vel > 0),
            Msg::Off { ch, key, .. } => (Self::Note { ch, key }, false),
            Msg::Cc { ch, n, v } => (Self::Cc { ch, n }, v >= 64),
            _ => return None,
        })
    }

    fn valid(self) -> bool {
        match self {
            Self::Note { ch, key } => ch < 16 && key < 128,
            Self::Cc { ch, n } => ch < 16 && n < 128,
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::Note { ch, key } => format!("Ch {} Note {key}", ch + 1),
            Self::Cc { ch, n } => format!("Ch {} CC {n}", ch + 1),
        }
    }
}

pub(crate) type Bindings = [Option<Binding>; Transport::ALL.len()];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    #[default]
    Both,
    Instrument,
    Control,
}

impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Self::Both => "Instruments + controls",
            Self::Instrument => "Instruments only",
            Self::Control => "DAW controls only",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Both => Self::Instrument,
            Self::Instrument => Self::Control,
            Self::Control => Self::Both,
        }
    }
}

pub fn role(device: &str) -> Role {
    match crate::settings_file::get(&device_key("midi_role", device)).as_deref() {
        Some("instrument") => Role::Instrument,
        Some("control") => Role::Control,
        _ => Role::Both,
    }
}

pub fn set_role(device: &str, role: Role) {
    crate::settings_file::write(&device_key("midi_role", device), match role {
        Role::Both => "",
        Role::Instrument => "instrument",
        Role::Control => "control",
    });
}

pub fn one_shot(device: &str) -> bool {
    crate::settings_file::flag(&device_key("midi_one_shot", device), false)
}

pub fn set_one_shot(device: &str, on: bool) {
    crate::settings_file::set_flag(&device_key("midi_one_shot", device), on);
}

pub fn feedback_output(device: &str) -> Option<String> {
    crate::settings_file::get(&device_key("midi_feedback", device))
        .and_then(|v| serde_json::from_str(&v).ok())
}

pub fn set_feedback_output(device: &str, output: Option<&str>) {
    let value = output.map(|o| serde_json::to_string(o).expect("MIDI port name")).unwrap_or_default();
    crate::settings_file::write(&device_key("midi_feedback", device), &value);
}

#[derive(Debug, Clone)]
pub struct Trace {
    pub msg: Msg,
    pub route: &'static str,
}

impl Trace {
    pub fn label(&self) -> String {
        let message = match self.msg {
            Msg::On { ch, key, vel } => format!("Ch {}   Note on {key}   Velocity {vel}", ch + 1),
            Msg::Off { ch, key, vel } => format!("Ch {}   Note off {key}   Velocity {vel}", ch + 1),
            Msg::Cc { ch, n, v } => format!("Ch {}   CC {n}   Value {v}", ch + 1),
            Msg::Bend { ch, v } => format!("Ch {}   Pitch bend {v}", ch + 1),
            Msg::Pressure { ch, v } => format!("Ch {}   Pressure {v}", ch + 1),
            Msg::Poly { ch, key, v } => format!("Ch {}   Note {key} pressure {v}", ch + 1),
            Msg::Program { ch, p } => format!("Ch {}   Program {p}", ch + 1),
            Msg::Start => "MIDI Start".into(),
            Msg::Continue => "MIDI Continue".into(),
            Msg::Stop => "MIDI Stop".into(),
            Msg::Spp(pos) => format!("Song position {pos}"),
            Msg::Machine(cmd) => format!("MIDI Machine Control {cmd:02X}"),
        };
        format!("{message}   →   {}", self.route)
    }
}

fn device_key(kind: &str, device: &str) -> String {
    // Escape the settings delimiter, escape marker and line breaks without merging device names.
    let name = device.replace('%', "%25").replace('=', "%3D").replace('\r', "%0D").replace('\n', "%0A");
    format!("{kind}:[{name}]")
}

pub fn bindings(device: &str) -> Bindings {
    let stored = crate::settings_file::get(&device_key("midi_transport", device))
        .and_then(|value| serde_json::from_str::<Vec<Option<Binding>>>(&value).ok());
    let mut mapped = Bindings::default();
    if let Some(stored) = stored {
        for (slot, binding) in mapped.iter_mut().zip(stored) {
            *slot = binding.filter(|b| b.valid());
        }
    }
    mapped
}

pub const VISIBILITY: &str = "visible_midi-controller-setup";
pub const MODES: &[&str] = &[
    "Generic MIDI / MIDI Learn",
    "Mackie Control (MCU)",
    "Komplete Kontrol DAW (MCU)",
    "Arturia KeyLab DAW (MCU)",
    "Behringer X-Touch (MCU)",
    "Korg nanoKONTROL2 (factory CC)",
    "M-Audio Oxygen Pro (Mackie mode)",
    "Korg nanoKONTROL Studio (Mackie mode)",
    "Novation Launchkey MK4 (native DAW)",
    "SSL UF1 / UF8 (Mackie mode)",
    "iCON QCon (Mackie mode)",
    "Akai / other MIDI Machine Control",
];

pub const LAUNCHKEY: usize = 8;
const MMC: usize = 11;

pub fn is_mcu(profile: usize) -> bool {
    matches!(profile, 1..=4 | 6 | 7 | 9 | 10)
}
fn key(device: &str) -> String {
    format!("midi_surface:{}", device.replace('=', "_"))
}
pub fn mode(device: &str) -> usize {
    crate::settings_file::get(&key(device)).and_then(|v| v.parse().ok())
        .filter(|&i| i < MODES.len()).unwrap_or(0)
}
pub fn set_mode(device: &str, value: usize) {
    if value < MODES.len() { crate::settings_file::write(&key(device), &value.to_string()); }
}
pub fn label(device: &str) -> &'static str { MODES[mode(device)] }

impl App {
    pub(crate) fn controller_menu(&mut self, x: f32, y: f32) {
        let Some(device) = self.state.midi.selected_input().map(|d| d.name.clone()) else { return };
        let current = mode(&device);
        let items = MODES.iter().enumerate().map(|(i, label)|
            PItem::new(*label, Cmd::Controller(device.clone(), i)).mark(Mark::Radio(i == current))).collect();
        self.state.playlist.popup = Some(Popup { x, y, items, sub: None, scroll: [0; 2], cascade: Vec::new() });
        self.invalidate();
    }
    pub(crate) fn controller_preset_dialog(&mut self, device: String, save: bool) {
        use crate::filedlg::{FileDlg, Mode};
        let (mode, title) = if save {
            (Mode::Save, "Save MIDI Learn preset")
        } else {
            (Mode::Open, "Load MIDI Learn preset into the selected input")
        };
        let dialog = FileDlg::new(mode, title, "midi-controller", crate::dialog::Then::MidiControllerPreset(device, save))
            .filter("MIDI Learn preset", &["json"]);
        self.file_dialog(dialog);
    }

    pub(crate) fn controller_preset_done(&mut self, device: &str, path: &std::path::Path, save: bool) {
        let result = if save {
            let preset = Preset { version: 1, bindings: bindings(device), one_shot: one_shot(device) };
            serde_json::to_vec_pretty(&preset).map_err(|e| e.to_string())
                .and_then(|bytes| std::fs::write(path, bytes).map_err(|e| e.to_string()))
        } else {
            read_preset(path).map(|preset| {
                self.state.midi.learning = None;
                self.midi_play.transport_held.retain(|(dev, _)| dev.as_ref() != device);
                self.save_transport_bindings(device, preset.bindings);
                set_one_shot(device, preset.one_shot);
            })
        };
        self.state.message = Some(match result {
            Ok(()) if save => "MIDI Learn preset saved".into(),
            Ok(()) => format!("MIDI Learn preset loaded for {device}"),
            Err(error) => format!("MIDI Learn preset: {error}"),
        });
        self.invalidate();
    }

    pub(crate) fn clear_transport_binding(&mut self, device: &str, action: Transport) {
        let mut mapped = bindings(device);
        mapped[action as usize] = None;
        self.save_transport_bindings(device, mapped);
    }

    fn save_transport_bindings(&mut self, device: &str, mapped: Bindings) {
        let value = if mapped.iter().any(Option::is_some) {
            serde_json::to_string(&mapped).expect("serializing MIDI transport bindings")
        } else {
            String::new()
        };
        crate::settings_file::write(&device_key("midi_transport", device), &value);
        self.midi_play.transport_bindings.insert(device.into(), mapped);
        self.invalidate();
    }

    fn learned_transport_event(&mut self, ev: &Ev) -> bool {
        let Some((binding, pressed)) = Binding::from_message(ev.msg) else { return false };
        let key = (ev.dev.clone(), binding);
        if pressed && self.state.settings.open && self.state.settings.tab == crate::views::settings::MIDI
            && self.state.midi.learning.as_ref().is_some_and(|(device, _)| device == ev.dev.as_ref())
        {
            let (_, action) = self.state.midi.learning.take().expect("MIDI Learn is armed");
            let mut mapped = bindings(&ev.dev);
            // Reassign a button instead of triggering two commands from one press.
            for slot in &mut mapped {
                if *slot == Some(binding) {
                    *slot = None;
                }
            }
            mapped[action as usize] = Some(binding);
            self.save_transport_bindings(&ev.dev, mapped);
            self.midi_play.transport_held.insert(key);
            self.state.message = Some(format!("{} learned from {} ({})", action.label(), ev.dev, binding.label()));
            return true;
        }

        let mapped = self.midi_play.transport_bindings.entry(ev.dev.clone()).or_insert_with(|| bindings(&ev.dev));
        let action = Transport::ALL.into_iter().find(|&action| mapped[action as usize] == Some(binding));
        if !pressed {
            // Consume the release even if the mapping was removed while the button was down.
            return self.midi_play.transport_held.remove(&key) || action.is_some();
        }
        let Some(action) = action else { return self.midi_play.transport_held.contains(&key) };
        if (matches!(binding, Binding::Cc { .. }) && one_shot(&ev.dev)) || self.midi_play.transport_held.insert(key) {
            self.surface_action(action);
        }
        true
    }

    pub(crate) fn controller_event(&mut self, ev: &Ev) -> bool {
        if self.learned_transport_event(ev) { return true; }
        let profile = mode(&ev.dev);
        if profile == MMC {
            let action = match ev.msg {
                Msg::Machine(0x01) => Transport::Stop,
                Msg::Machine(0x02 | 0x03) => Transport::Play,
                Msg::Machine(0x06) if !self.state.rec.armed => Transport::Record,
                Msg::Machine(0x07) if self.state.rec.armed => Transport::Record,
                Msg::Machine(_) => return true,
                _ => return false,
            };
            self.surface_action(action);
            return true;
        }
        if profile == 0 { return false; }
        let bank = *self.midi_play.surface_banks.entry(ev.dev.to_string()).or_default();
        let track = |i: u8| bank + u16::from(i) + 1;
        if profile == LAUNCHKEY {
            self.launchkey_event(ev, bank);
            return true;
        }
        if profile == 5 {
            if let Msg::Cc { n, v, .. } = ev.msg {
                match n {
                    0..=7 => self.surface_fader(track(n), f32::from(v) / 127.0),
                    16..=23 => self.surface_pan(track(n - 16), f32::from(v) / 127.0),
                    32..=39 if v >= 64 => { self.surface_solo(track(n - 32)); }
                    48..=55 if v >= 64 => self.surface_button(track(n - 48), false),
                    64..=71 if v >= 64 => self.surface_button(track(n - 64), true),
                    41 if v >= 64 => self.surface_play(),
                    42 if v >= 64 => self.act(Action::Stop),
                    45 if v >= 64 => self.act(Action::Record),
                    58 if v >= 64 => self.surface_bank(&ev.dev, -8),
                    59 if v >= 64 => self.surface_bank(&ev.dev, 8),
                    32..=39 | 48..=55 | 64..=71 | 41 | 42 | 45 | 58 | 59 => {},
                    _ => return false,
                }
                return true;
            }
            return false;
        }
        match ev.msg {
            Msg::On { ch: 0, key: key @ 0x68..=0x70, .. } | Msg::Off { ch: 0, key: key @ 0x68..=0x70, .. } => {
                let touched = self.midi_play.surface_touched.entry(ev.dev.clone()).or_default();
                let bit = 1 << (key - 0x68);
                if matches!(ev.msg, Msg::On { vel: 1..=127, .. }) {
                    *touched |= bit;
                } else {
                    *touched &= !bit;
                }
            }
            Msg::On { ch: 0, key, vel: 1..=127 } => match key {
                0..=7 => self.surface_button(track(key), true),
                8..=15 => self.surface_solo(track(key - 8)),
                16..=23 => self.surface_button(track(key - 16), false),
                24..=31 => {
                    let t = track(key - 24);
                    if t <= crate::mixer::inserts(&self.state) { crate::mixer::select(&mut self.state, t, false); }
                }
                0x2e => self.surface_bank(&ev.dev, -8),
                0x2f => self.surface_bank(&ev.dev, 8),
                0x30 => self.surface_bank(&ev.dev, -1),
                0x31 => self.surface_bank(&ev.dev, 1),
                0x56 => self.surface_action(Transport::LoopRecord),
                0x59 => self.surface_action(Transport::Metronome),
                0x5d => self.act(Action::Stop),
                0x5e => self.surface_play(),
                0x5f => self.act(Action::Record),
                _ => {}
            },
            Msg::Bend { ch: ch @ 0..=8, v } => {
                self.surface_fader(if ch == 8 { 0 } else { track(ch) }, f32::from(v) / 16383.0);
            }
            Msg::Cc { ch: 0, n: n @ 16..=23, v } => {
                let t = track(n - 16);
                let delta = f32::from(v & 63) / 127.0 * if v & 64 != 0 { -1.0 } else { 1.0 };
                let current = crate::history::knob_value(&self.state, crate::knobs::KnobId::MixPan(t)) as f32;
                self.surface_pan(t, current + delta);
            }
            _ => {}
        }
        // A dedicated MCU port carries control messages, including releases and
        // unsupported buttons. Never send these to the selected instrument.
        self.invalidate();
        true
    }
    pub(crate) fn controller_frames(&self) -> Vec<crate::midi_out::SurfaceFrame> {
        let mut frames = Vec::new();
        for input in self.state.midi.inputs.iter().filter(|input| input.open) {
            let device = input.name.as_str();
            let profile = mode(device);
            if role(device) == Role::Instrument || !(profile == LAUNCHKEY || is_mcu(profile)) { continue; }
            let Some(port) = feedback_output(device) else { continue };
            if !self.state.midi.outputs.contains(&port)
                || frames.iter().any(|f: &crate::midi_out::SurfaceFrame| f.port == port)
            {
                continue;
            }
            let bank = self.midi_play.surface_banks.get(device).copied().unwrap_or(0);
            let touched = self.midi_play.surface_touched.get(device).copied().unwrap_or(0);
            let launchkey = profile == LAUNCHKEY;
            let messages = if launchkey {
                let mut messages = vec![
                    vec![0xb3, 115, u8::from(self.transport.playing()) * 127],
                    vec![0xb3, 117, u8::from(self.state.rec.armed) * 127],
                ];
                for strip in 0..8u8 {
                    let index = bank + u16::from(strip) + 1;
                    let track = crate::mixer::track(&self.state, index);
                    let selected = self.state.selected_mixer == index;
                    messages.push(vec![0x90, 0x60 + strip, if track.is_none() { 0 } else if selected { 21 } else { 5 }]);
                    messages.push(vec![0x90, 0x70 + strip, if track.is_some_and(|t| !t.enabled) { 5 } else { 0 }]);
                }
                messages
            } else {
                mcu_messages(&self.state, bank, touched, self.transport.playing())
            };
            frames.push(crate::midi_out::SurfaceFrame { port, launchkey, messages });
        }
        frames
    }

    fn surface_action(&mut self, action: Transport) {
        let track = self.state.selected_mixer;
        match action {
            Transport::Play => self.surface_play(),
            Transport::Stop => self.act(Action::Stop),
            Transport::Record => self.act(Action::Record),
            Transport::LoopRecord => self.toggle_record_option(crate::record::Opt::Loop),
            Transport::Metronome => self.act(Action::Metronome),
            Transport::Previous | Transport::Next => {
                let next = if action == Transport::Previous { track.saturating_sub(1) } else { (track + 1).min(crate::mixer::inserts(&self.state)) };
                crate::mixer::select(&mut self.state, next, false);
                self.invalidate();
            }
            Transport::Mute => self.surface_button(track, false),
            Transport::Solo => self.surface_solo(track),
            Transport::Arm => self.surface_button(track, true),
        }
    }

    fn launchkey_event(&mut self, ev: &Ev, bank: u16) {
        if let Msg::On { ch: 0, key, vel: 1..=127 } = ev.msg {
            match key {
                0x60..=0x67 => {
                    let index = bank + u16::from(key - 0x60) + 1;
                    if index <= crate::mixer::inserts(&self.state) {
                        crate::mixer::select(&mut self.state, index, false);
                        self.invalidate();
                    }
                }
                0x70..=0x77 => self.surface_button(bank + u16::from(key - 0x70) + 1, false),
                _ => {}
            }
        }
        if let Msg::Cc { ch, n, v } = ev.msg {
            match (ch, n) {
                (6, 30) => { self.midi_play.launchkey_encoder_mode.insert(ev.dev.clone(), v); }
                (15, 5..=13) => self.surface_fader(if n == 13 { 0 } else { bank + u16::from(n - 5) + 1 }, f32::from(v) / 127.0),
                (15, 21..=28) if self.midi_play.launchkey_encoder_mode.get(&ev.dev) == Some(&1) =>
                    self.surface_pan(bank + u16::from(n - 21) + 1, f32::from(v) / 127.0),
                // The DAW endpoint is dedicated. Accept transport buttons on its button channel and Ch. 16.
                (0 | 15, 115) if v >= 64 => self.surface_action(Transport::Play),
                (0 | 15, 116) if v >= 64 => self.surface_action(Transport::Stop),
                (0 | 15, 117) if v >= 64 => self.surface_action(Transport::Record),
                (0 | 15, 118) if v >= 64 => self.surface_action(Transport::LoopRecord),
                (0 | 15, 103 | 106) if v >= 64 => self.surface_action(Transport::Previous),
                (0 | 15, 102 | 107) if v >= 64 => self.surface_action(Transport::Next),
                (0 | 15, 51) if v >= 64 => self.surface_bank(&ev.dev, -8),
                (0 | 15, 52) if v >= 64 => self.surface_bank(&ev.dev, 8),
                _ => {}
            }
        }
    }

    fn surface_play(&mut self) {
        if self.transport.state() != crate::transport::State::Playing { self.act(Action::Play); }
    }
    fn surface_bank(&mut self, device: &str, delta: i32) {
        let bank = self.midi_play.surface_banks.entry(device.to_string()).or_default();
        *bank = (i32::from(*bank) + delta).clamp(0, i32::from(crate::mixer::inserts(&self.state).saturating_sub(1))) as u16;
        self.state.message = Some(format!("Controller mixer bank starts at Insert {}", *bank + 1));
    }
    fn surface_fader(&mut self, track: u16, value: f32) {
        if track > crate::mixer::inserts(&self.state) || crate::mixer::locked(&self.state, track) { return; }
        crate::history::fader(&mut self.state, track, true);
        if let Some(t) = crate::mixer::track_mut(&mut self.state, track) { t.volume = Some(f64::from(value.clamp(0.0, 1.0))); }
        self.sync_live();
        self.invalidate();
    }
    fn surface_pan(&mut self, track: u16, value: f32) {
        if track > crate::mixer::inserts(&self.state) || crate::mixer::locked(&self.state, track) { return; }
        let knob = crate::knobs::KnobId::MixPan(track);
        crate::history::knob(&mut self.state, knob, true);
        self.state.set_knob(knob, value.clamp(0.0, 1.0));
        self.record_knob(knob);
        self.sync_live();
        self.invalidate();
    }
    fn surface_button(&mut self, track: u16, arm: bool) {
        if track > crate::mixer::inserts(&self.state) || crate::mixer::locked(&self.state, track) { return; }
        self.act(Action::Mix(if arm { crate::mixer::MHit::Disk(track) } else { crate::mixer::MHit::Led(track) }));
    }
    fn surface_solo(&mut self, track: u16) {
        if track > crate::mixer::inserts(&self.state) || crate::mixer::locked(&self.state, track) { return; }
        crate::mixer::run(self, crate::mixer::MCmd::Solo(track));
        self.sync_live();
        self.invalidate();
    }
}

#[derive(Serialize, Deserialize)]
struct Preset {
    version: u8,
    bindings: Bindings,
    one_shot: bool,
}

fn read_preset(path: &std::path::Path) -> Result<Preset, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(8193).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() > 8192 { return Err("File exceeds 8 KB".into()); }
    let preset: Preset = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let mut used = std::collections::HashSet::new();
    if preset.version != 1 || preset.bindings.iter().flatten().any(|&b| !b.valid() || !used.insert(b)) {
        return Err("Invalid or unsupported controller preset".into());
    }
    Ok(preset)
}

fn mcu_messages(s: &crate::app::State, bank: u16, touched: u16, playing: bool) -> Vec<Vec<u8>> {
    let mut messages = Vec::with_capacity(54);
    let mut labels = vec![0xf0, 0x00, 0x00, 0x66, 0x14, 0x12, 0x00];
    let led = |id, on: bool| vec![0x90, id, if on { 127 } else { 0 }];
    for strip in 0..9u8 {
        let index = if strip == 8 { 0 } else { bank + u16::from(strip) + 1 };
        let track = crate::mixer::track(s, index);
        let volume = track.map_or(0.0, |t| t.volume.unwrap_or(0.8)).clamp(0.0, 1.0);
        let value = (volume * 16383.0).round() as u16;
        messages.push(if touched & (1 << strip) != 0 { Vec::new() } else {
            vec![0xe0 | strip, (value & 127) as u8, (value >> 7) as u8]
        });
        if strip == 8 { continue; }
        messages.push(led(strip, s.mixer.recording.armed.contains(&index)));
        messages.push(led(8 + strip, s.mixer.solo.as_ref().is_some_and(|(tracks, _)| tracks.contains(&index))));
        messages.push(led(16 + strip, track.is_some_and(|t| !t.enabled)));
        messages.push(led(24 + strip, track.is_some() && s.selected_mixer == index));
        let pan = track.and_then(|t| t.pan).unwrap_or(0.0).clamp(-1.0, 1.0);
        messages.push(vec![0xb0, 0x30 + strip, (6.0 + pan * 5.0).round() as u8]);
        let name = track.map_or_else(String::new, |t| t.name.clone().unwrap_or_else(|| format!("Ins {index}")));
        labels.extend(name.chars().map(|c| if c.is_ascii_graphic() { c as u8 } else { b' ' })
            .chain(std::iter::repeat(b' ')).take(7));
    }
    labels.push(0xf7);
    messages.push(labels);
    messages.extend([
        led(0x59, crate::metronome::on()),
        led(0x56, crate::record::Opt::Loop.on()),
        led(0x5d, !playing),
        led(0x5e, playing),
        led(0x5f, s.rec.armed),
    ]);
    messages
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(device: &str, msg: Msg) -> Ev {
        Ev { dev: std::sync::Arc::from(device), msg }
    }
    #[test]
    fn controller_ports_keep_keys_separate_and_mcu_releases_do_not_play_notes() {
        let mut app = App::new(None);
        app.state.project = crate::app::test_project();
        set_mode("Controller DAW", 1);
        let note = Msg::On { ch: 0, key: 60, vel: 100 };
        assert!(!app.controller_event(&event("Controller MIDI", note)));
        assert!(app.controller_event(&event("Controller DAW", note)));
        app.midi_events(vec![event("Controller DAW", note)]);
        assert_eq!(app.midi_play.lit().count(), 0);
        app.midi_events(vec![event("Controller MIDI", note)]);
        assert_eq!(app.midi_play.lit().count(), 1);
        app.midi_events(vec![event("Controller MIDI", Msg::Off { ch: 0, key: 60, vel: 0 })]);
        assert_eq!(app.midi_play.lit().count(), 0);
        assert_eq!(mode("Controller DAW"), 1);
        assert_eq!(mode("Controller MIDI"), 0);
    }
    #[test]
    fn controller_learned_transport_is_per_input_and_channel_and_survives_restart() {
        use crate::views::midi_page::MAct;
        use crate::views::settings::MIDI;

        let mut app = App::new(None);
        app.state.project = crate::app::test_project();
        app.state.settings.open = true;
        app.state.settings.tab = MIDI;
        let surface = "Test = DAW";
        app.state.midi.inputs.push(crate::midi_in::InPort { name: surface.into(), open: true, failed: false });
        let note = Msg::On { ch: 2, key: 60, vel: 100 };
        let release = Msg::Off { ch: 2, key: 60, vel: 0 };
        let stop = Msg::Cc { ch: 2, n: 34, v: 127 };
        let stop_up = Msg::Cc { ch: 2, n: 34, v: 0 };
        let rec = Msg::Cc { ch: 2, n: 35, v: 127 };
        let rec_up = Msg::Cc { ch: 2, n: 35, v: 0 };

        app.midi_page_act(MAct::Learn(Transport::Play));
        app.midi_events(vec![event("Test Keys", note), event("Test Keys", release), event(surface, stop_up)]);
        assert!(app.state.midi.learning.is_some(), "a different device and a release cannot teach");
        app.midi_events(vec![event(surface, note), event(surface, release)]);
        assert!(app.state.midi.learning.is_none());
        assert_eq!(app.midi_play.lit().count(), 0);
        assert!(!app.transport.playing(), "the teaching press cannot start playback");
        for (action, press, up) in [(Transport::Stop, stop, stop_up), (Transport::Record, rec, rec_up)] {
            app.midi_page_act(MAct::Learn(action));
            app.midi_events(vec![event(surface, press), event(surface, up)]);
        }
        assert!(!app.state.rec.armed);
        assert_eq!(bindings(surface).into_iter().flatten().count(), 3);
        assert_eq!(bindings("Test %3D DAW"), Bindings::default());

        let mut app = App::new(None);
        app.state.project = crate::app::test_project();
        crate::settings_file::set_flag(VISIBILITY, false);
        app.midi_events(vec![event("Test Keys", note)]);
        assert_eq!(app.midi_play.lit().count(), 1);
        app.midi_events(vec![event("Test Keys", release)]);
        let other_channel = Msg::On { ch: 1, key: 60, vel: 100 };
        assert!(!app.controller_event(&event(surface, other_channel)));
        assert!(!app.transport.playing());
        app.midi_events(vec![event(surface, note), event(surface, release)]);
        assert!(app.transport.playing());
        assert_eq!(app.midi_play.lit().count(), 0);

        app.midi_events(vec![event(surface, stop), event(surface, stop_up)]);
        assert!(!app.transport.playing());
        app.midi_events(vec![event(surface, rec), event(surface, rec)]);
        assert!(app.state.rec.armed, "repeating a held press cannot toggle Record twice");
        app.midi_events(vec![event(surface, rec_up), event(surface, rec), event(surface, rec_up)]);
        assert!(!app.state.rec.armed);

        app.clear_transport_binding(surface, Transport::Play);
        assert!(!app.controller_event(&event(surface, note)));
        assert_eq!(bindings(surface)[Transport::Play as usize], None);
    }

    #[test]
    fn controller_monitor_and_roles_keep_unmapped_buttons_out_of_instruments() {
        let mut app = App::new(None);
        app.state.project = crate::app::test_project();
        app.state.settings.open = true;
        app.state.settings.tab = crate::views::settings::MIDI;
        app.state.midi.monitor = true;
        let input = "Role test";
        app.state.midi.inputs.push(crate::midi_in::InPort { name: input.into(), open: true, failed: false });
        set_role(input, Role::Control);
        let button = Msg::On { ch: 2, key: 60, vel: 100 };
        app.midi_events(vec![event(input, button)]);
        assert_eq!(app.state.midi.trace.back().unwrap().route, "Unmapped DAW input");
        assert_eq!(app.midi_play.lit().count(), 0);
        set_mode(input, 1);
        set_role(input, Role::Instrument);
        app.midi_events(vec![event(input, button)]);
        assert_eq!(app.state.midi.trace.back().unwrap().route, "Instrument path");
        assert_eq!(app.midi_play.lit().count(), 1);
    }

    #[test]
    fn controller_legacy_learn_and_single_press_preset_round_trip() {
        let mut app = App::new(None);
        app.state.project = crate::app::test_project();
        let input = "Learn controller";
        let binding = Binding::Cc { ch: 2, n: 42 };
        crate::settings_file::write(&device_key("midi_transport", input),
            &serde_json::to_string(&[None, None, Some(binding)]).unwrap());
        assert_eq!(bindings(input)[Transport::Record as usize], Some(binding));
        set_one_shot(input, true);
        let file = std::env::temp_dir().join(format!("ml-midi-controller-test-{}.json", std::process::id()));
        app.controller_preset_done(input, &file, true);
        app.controller_preset_done("Imported", &file, false);
        assert!(one_shot("Imported"));
        let press = event("Imported", Msg::Cc { ch: 2, n: 42, v: 127 });
        app.midi_events(vec![press.clone()]);
        assert!(app.state.rec.armed);
        app.midi_events(vec![press]);
        assert!(!app.state.rec.armed);
        std::fs::write(&file, b"{\"version\":99}").unwrap();
        app.controller_preset_done("Imported", &file, false);
        assert_eq!(bindings("Imported")[Transport::Record as usize], Some(binding));
        std::fs::remove_file(file).unwrap();
    }

    #[test]
    fn controller_mcu_feedback_and_launchkey_frames_follow_device_and_touch() {
        let mut app = App::new(None);
        app.state.project = crate::app::test_project();
        let launchkey = "Launchkey DAW input";
        let mcu = "X-Touch DAW input";
        for (input, output, profile) in [(launchkey, "Launchkey DAW output", LAUNCHKEY), (mcu, "X-Touch DAW output", 4)] {
            app.state.midi.inputs.push(crate::midi_in::InPort { name: input.into(), open: true, failed: false });
            app.state.midi.outputs.push(output.into());
            set_mode(input, profile);
            set_feedback_output(input, Some(output));
        }
        app.midi_events(vec![event(launchkey, Msg::On { ch: 0, key: 0x61, vel: 127 })]);
        assert_eq!(app.state.selected_mixer, 2);
        assert_eq!(app.midi_play.lit().count(), 0);
        let frames = app.controller_frames();
        assert!(frames[0].launchkey);
        assert!(frames[0].messages.contains(&vec![0x90, 0x61, 21]));
        assert_eq!(frames[1].messages[0][0], 0xe0);
        assert!(frames[1].messages.iter().any(|m| m.starts_with(&[0xf0, 0, 0, 0x66, 0x14, 0x12])));
        app.midi_events(vec![event(mcu, Msg::On { ch: 0, key: 0x68, vel: 127 })]);
        assert!(app.controller_frames()[1].messages[0].is_empty());
        app.midi_events(vec![event(mcu, Msg::Off { ch: 0, key: 0x68, vel: 0 })]);
        assert_eq!(app.controller_frames()[1].messages[0][0], 0xe0);
    }

    #[test]
    fn controller_mmc_is_opt_in_and_keeps_akai_notes() {
        let mut app = App::new(None);
        app.state.project = crate::app::test_project();
        let input = "Akai MPK";
        let play = crate::midi_in::parse(&[0xf0, 0x7f, 0x7f, 0x06, 0x02, 0xf7]).unwrap();
        app.midi_events(vec![event(input, play)]);
        assert!(!app.transport.playing());
        set_mode(input, MMC);
        app.midi_events(vec![event(input, play), event(input, Msg::On { ch: 0, key: 60, vel: 100 })]);
        assert!(app.transport.playing());
        assert_eq!(app.midi_play.lit().count(), 1);
        assert!(crate::midi_in::parse(&[0xf0, 0x7d, 0x7f, 0x06, 0x02, 0xf7]).is_none());
    }

    #[test]
    fn controller_korg_factory_fader_and_button_release() {
        let mut app = App::new(None);
        app.state.project = crate::app::test_project();
        set_mode("Korg test", 5);
        app.midi_events(vec![event("Korg test", Msg::Cc { ch: 0, n: 0, v: 127 })]);
        assert_eq!(crate::mixer::track(&app.state, 1).unwrap().volume, Some(1.0));
        let enabled = crate::mixer::track(&app.state, 1).unwrap().enabled;
        app.midi_events(vec![event("Korg test", Msg::Cc { ch: 0, n: 48, v: 127 }),
                            event("Korg test", Msg::Cc { ch: 0, n: 48, v: 0 })]);
        assert_eq!(crate::mixer::track(&app.state, 1).unwrap().enabled, !enabled);
    }
    #[test]
    fn controller_faders_respect_locks_and_banks_are_per_device() {
        let mut app = App::new(None);
        app.state.project = crate::app::test_project();
        set_mode("Surface A", 1);
        set_mode("Surface B", 1);
        app.controller_event(&event("Surface A", Msg::Bend { ch: 0, v: 16383 }));
        assert_eq!(crate::mixer::track(&app.state, 1).unwrap().volume, Some(1.0));
        crate::mixer::run(&mut app, crate::mixer::MCmd::ToggleLock(1));
        app.controller_event(&event("Surface A", Msg::Bend { ch: 0, v: 0 }));
        assert_eq!(crate::mixer::track(&app.state, 1).unwrap().volume, Some(1.0));
        app.controller_event(&event("Surface A", Msg::On { ch: 0, key: 0x2f, vel: 127 }));
        app.controller_event(&event("Surface B", Msg::Off { ch: 0, key: 0x2f, vel: 0 }));
        assert!(app.midi_play.surface_banks["Surface A"] > 0);
        assert_eq!(app.midi_play.surface_banks["Surface B"], 0);
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
//! Resolve guide destinations against the controls painted for the current workspace.
use super::{Hit, PAGE, hit, is_open, topics};
use crate::app::{Action, App, State, Ui, Win};
use crate::mixer::MHit;
use crate::views::settings::{self, SAct};
use os_ui::{Color, Font, Rect};

#[derive(Clone, Copy, Debug)]
enum Destination {
    Toolbar,
    Editor(Win),
    Settings(usize),
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Target {
    Transport,
    Record,
    Typing,
    Step,
    AudioDevice,
    AudioBuffer,
    Midi,
    RackSteps,
    RackRoute,
    PianoNotes,
    PianoTools,
    PianoExpression,
    PianoMpe,
    Chords,
    PianoOverview,
    PlacePattern,
    ClipPicker,
    Stretch,
    Warp,
    Vocal,
    Comp,
    TrackMenu,
    Arrangement,
    PlaylistOverview,
    MixerLevel,
    MixerSend,
    MixerLayout,
    MixerLock,
    MixerInput,
    EffectSlot,
    EffectChain,
    Phones,
    MasterAssistant,
    PluginScan,
    Controls,
    Visibility,
    Menus,
    Theme,
    Browser,
    Export,
}

#[derive(Debug)]
pub(super) struct Visit {
    target: Target,
    article_scroll: f32,
}

impl Target {
    pub(super) fn from_key(key: &str) -> Option<Self> {
        use Target::*;
        Some(match key {
            "transport" => Transport,
            "record" => Record,
            "typing" => Typing,
            "step" => Step,
            "audio-device" => AudioDevice,
            "audio-buffer" => AudioBuffer,
            "midi" => Midi,
            "rack-steps" => RackSteps,
            "rack-route" => RackRoute,
            "piano-notes" => PianoNotes,
            "piano-tools" => PianoTools,
            "piano-expression" => PianoExpression,
            "piano-mpe" => PianoMpe,
            "chords" => Chords,
            "piano-overview" => PianoOverview,
            "place-pattern" => PlacePattern,
            "clip-picker" => ClipPicker,
            "stretch" => Stretch,
            "warp" => Warp,
            "vocal" => Vocal,
            "comp" => Comp,
            "track-menu" => TrackMenu,
            "arrangement" => Arrangement,
            "playlist-overview" => PlaylistOverview,
            "mixer-level" => MixerLevel,
            "mixer-send" => MixerSend,
            "mixer-layout" => MixerLayout,
            "mixer-lock" => MixerLock,
            "mixer-input" => MixerInput,
            "effect-slot" => EffectSlot,
            "effect-chain" => EffectChain,
            "phones" => Phones,
            "master-assistant" => MasterAssistant,
            "plugin-scan" => PluginScan,
            "controls" => Controls,
            "visibility" => Visibility,
            "menus" => Menus,
            "theme" => Theme,
            "browser" => Browser,
            "export" => Export,
            _ => return None,
        })
    }

    fn destination(self) -> Destination {
        use Destination::*;
        use Target::*;
        match self {
            Transport | Record | Typing | Step | Export => Toolbar,
            AudioDevice | AudioBuffer => Settings(settings::AUDIO),
            Midi => Settings(settings::MIDI),
            PluginScan => Settings(settings::PLUGINS),
            Controls => Settings(settings::CONTROLS),
            Visibility => Settings(settings::VISIBILITY),
            Menus => Settings(settings::MENUS),
            Theme => Settings(settings::THEME),
            RackSteps | RackRoute => Editor(Win::ChannelRack),
            PianoNotes | PianoTools | PianoExpression | PianoMpe | Chords | PianoOverview
            | PlacePattern => Editor(Win::PianoRoll),
            ClipPicker | Stretch | Warp | Vocal | Comp | TrackMenu | Arrangement
            | PlaylistOverview => Editor(Win::Playlist),
            MixerLevel | MixerSend | MixerLayout | MixerLock | MixerInput | EffectSlot
            | EffectChain | Phones | MasterAssistant => Editor(Win::Mixer),
            Browser => Editor(Win::Browser),
        }
    }

    /// A higher priority favors the relevant track or the actual control over its entry point.
    fn match_control(
        self,
        s: &State,
        action: &Action,
        audio_row: Option<u16>,
    ) -> Option<(u8, &'static str)> {
        use crate::app::Scroll;
        use crate::knobs::KnobId;
        use Target::*;
        let selected = s.selected_mixer;
        let mixer_track = |track| if track == selected { 3 } else { 1 };

        Some(match (self, action) {
            (Transport, Action::Play) => (1, "Start playback. Check PAT or SONG before playing."),
            (Record, Action::Record) => {
                (1, "Record controls. Choose what to record before starting.")
            }
            (Typing, Action::Typing) => (1, "Typing keyboard to piano"),
            (Step, Action::StepEdit) => (1, "Step Edit"),
            (Export, Action::Menu(0)) => (1, "Open File and choose Export."),
            (AudioDevice, Action::Settings(SAct::Combo(settings::OUTPUT_DEVICE, ..))) => {
                (1, "Choose the audio output device here.")
            }
            (AudioBuffer, Action::Settings(SAct::Combo(settings::BUFFER_LENGTH, ..))) => {
                (2, "Adjust the audio buffer here.")
            }
            (AudioBuffer, Action::Settings(SAct::Button(settings::SHOW_ASIO_PANEL))) => {
                (1, "Adjust the ASIO buffer in the driver's panel.")
            }
            (Midi, Action::Settings(SAct::Tab(settings::MIDI))) => {
                (1, "MIDI settings. Choose your input device here.")
            }
            (PluginScan, Action::Settings(SAct::Button("Find new plugins"))) => {
                (2, "Find new plugins starts a scan.")
            }
            (PluginScan, Action::Settings(SAct::Tab(settings::PLUGINS))) => {
                (1, "Scroll down to Find new plugins.")
            }
            (Controls, Action::Settings(SAct::InputBindings(_))) => {
                (2, "Choose a context, event and action here.")
            }
            (Controls, Action::Settings(SAct::Tab(settings::CONTROLS))) => {
                (1, "The Controls tab contains mouse and keyboard bindings.")
            }
            (Visibility, Action::Settings(SAct::VisibilitySearch)) => (
                1,
                "Search here for the button or feature you want to show or hide.",
            ),
            (Menus, Action::Settings(SAct::Tab(settings::MENUS))) => {
                (1, "Choose a menu to customize in this tab.")
            }
            (Theme, Action::Settings(SAct::Tab(settings::THEME))) => {
                (1, "Choose and customize themes in this tab.")
            }
            (Browser, Action::BrowserTab(0)) => {
                (2, "Open the Browser's files tab to find samples.")
            }
            (Browser, Action::BrowserList) => (1, "Browse files and other sources here."),
            (RackSteps, Action::Step(..)) => (1, "Click a step to turn it on or off."),
            (RackRoute, Action::RackMixer(_)) => (
                1,
                "Click a channel's Mixer track number to choose its destination.",
            ),
            (PianoNotes, Action::PianoGrid) => (1, "Draw and edit notes on this grid."),
            (PianoTools, Action::PianoToolsMenu) => (1, "Open the Piano Roll Tools menu."),
            (PianoMpe, Action::PianoOptions) => (
                1,
                "Open Piano Roll options and choose MPE / Note expression.",
            ),
            (Chords, Action::Harmony(crate::harmony::HCmd::Open)) => {
                (1, "Open Chord Workspace here.")
            }
            (PianoExpression, Action::PianoLaneMenu) => {
                (1, "Choose a note property to edit in the lower lane.")
            }
            (PianoOverview, Action::Scrollbar(Scroll::PianoX)) => {
                (1, "Drag the Piano Roll overview to navigate.")
            }
            (PlacePattern, Action::PianoCommand(crate::piano_tools::NCmd::PlacePattern)) => {
                (1, "Place Pattern to Playlist")
            }
            (ClipPicker, Action::PlPickPreview(_)) => (
                3,
                "Click Play to loop a source, or Stop to end the preview.",
            ),
            (ClipPicker, Action::PlPickTab(_)) => (2, "Choose a source type in the clips picker."),
            (ClipPicker, Action::PlOptions) => (
                1,
                "Open Playlist options and choose Show clip picker panel.",
            ),
            (Stretch, Action::PlStretch) => (1, "Toggle Stretch before dragging a clip edge."),
            (Warp, Action::Warp(crate::warp::Command::Toggle(row))) => (
                if Some(*row) == audio_row { 3 } else { 1 },
                "Audio timing. Use this button on the track with your audio clip.",
            ),
            (Vocal, Action::Vocal(crate::vocal::Command::Toggle(row))) => (
                if Some(*row) == audio_row { 3 } else { 1 },
                "Vocal tuning. Use the microphone icon on the track with your vocal.",
            ),
            (Comp, Action::PlTool(crate::playlist::PlTool::Comp)) => {
                (2, "Turn on the Comping tool.")
            }
            (Comp | TrackMenu, Action::PlHeader(row)) => (
                if *row == s.playlist.track { 2 } else { 1 },
                "Right-click a track header for track commands.",
            ),
            (Arrangement, Action::PlArrangement) => (1, "Open the Arrangement selector."),
            (PlaylistOverview, Action::Scrollbar(Scroll::PlaylistX)) => {
                (1, "Drag the Playlist overview to navigate.")
            }
            (MixerLevel, Action::Fader(track)) => (
                mixer_track(*track),
                "Drag the channel fader to change its level.",
            ),
            (MixerSend, Action::Mix(MHit::Route(track))) if *track != selected => (
                3,
                "Routing triangle. Choose the destination beneath its Mixer strip.",
            ),
            (MixerSend, Action::Knob(KnobId::Send(source, _))) if *source == selected => (
                2,
                "Send level. Select the source channel before adjusting it.",
            ),
            (MixerSend, Action::Mix(MHit::Strip(track))) if *track != 0 => {
                (1, "Select a source insert to see its routing controls.")
            }
            (MixerLayout, Action::Mix(MHit::View(_))) => (2, "Choose a Mixer view here."),
            (MixerLayout | EffectChain, Action::Mix(MHit::Strip(track))) if *track != 0 => {
                let label = if matches!(self, EffectChain) {
                    "Right-click an insert and choose Save effect chain."
                } else {
                    "Right-click an insert and choose Channel layout."
                };
                (mixer_track(*track), label)
            }
            (MixerLock, Action::Mix(MHit::Lock(track))) => (
                mixer_track(*track),
                "Click the padlock to lock or unlock the channel.",
            ),
            (MixerInput, Action::Mix(MHit::Input(track))) => (
                mixer_track(*track),
                "Choose the recording input for this Mixer track.",
            ),
            (EffectSlot, Action::Mix(MHit::SlotName(slot))) => {
                if crate::mixer::slot_name(s, selected, *slot).is_none() {
                    (3, "Click this empty effect slot to open the effect picker.")
                } else {
                    (
                        2,
                        "This slot contains an effect. Click an empty slot to add another.",
                    )
                }
            }
            (EffectSlot, Action::Mix(MHit::Effects(track))) => (
                mixer_track(*track),
                "Open this track's effects panel to find its slots.",
            ),
            (Phones, Action::Mix(MHit::Headphones)) => {
                (1, "Open the headphone correction catalog.")
            }
            (MasterAssistant, Action::Mix(MHit::MasterAssistant)) => {
                (1, "Open Master Assistant on the Master strip.")
            }
            _ => return None,
        })
    }
}

pub(super) fn enabled() -> bool {
    crate::workspace_visibility::shown(super::VISIBILITY)
        && crate::workspace_visibility::shown(super::SPOTLIGHT_VISIBILITY)
}

pub(crate) fn active(s: &State) -> bool {
    s.settings.help.spotlight.is_some() && enabled()
}

pub(super) fn show(app: &mut App) {
    if !is_open(&app.state) || !enabled() {
        return;
    }
    let s = &mut app.state;
    let Some(target) = s
        .settings
        .help
        .topic
        .and_then(|index| topics().get(index))
        .and_then(|topic| topic.destination)
    else {
        return;
    };
    s.settings.help.spotlight = Some(Visit {
        target,
        article_scroll: s.settings.scroll,
    });
    s.settings.open = false;
    match target.destination() {
        Destination::Toolbar => {}
        Destination::Editor(win) => {
            if let Some(slot) = win.slot() {
                s.wins[slot].mini = false;
            }
            s.raise(win);
            if matches!(target, Target::Warp | Target::Vocal)
                && let Some(row) = preferred_audio_row(s)
            {
                s.playlist_scroll.1 = crate::playlist::Rows::new(s).top(row);
            }
        }
        Destination::Settings(tab) => app.open_settings(tab),
    }
    app.invalidate();
}

pub(super) fn return_to_guide(app: &mut App) {
    if !enabled() {
        return;
    }
    let st = &mut app.state.settings;
    if let Some(visit) = st.help.spotlight.take() {
        st.open = true;
        st.tab = PAGE;
        st.scroll = visit.article_scroll;
        app.invalidate();
    }
}

pub(super) fn dismiss(app: &mut App) {
    if app.state.settings.help.spotlight.take().is_some() {
        app.invalidate();
    }
}

fn preferred_audio_row(s: &State) -> Option<u16> {
    let clips = crate::playlist::clips(s);
    let is_audio = |clip: &&os_model::Clip| {
        matches!(clip.source, os_model::ClipSource::Channel(id) if s.project.channels.iter()
            .any(|channel| channel.index == id && channel.kind.is_sampler()))
    };
    s.playlist
        .sel
        .iter()
        .filter_map(|&i| clips.get(i))
        .find(is_audio)
        .or_else(|| clips.iter().find(is_audio))
        .map(|clip| clip.track)
}

fn control(ui: &Ui, s: &State, target: Target, screen: Rect) -> Option<(Rect, &'static str)> {
    let destination = target.destination();
    let hits = ui.hits();
    let audio_row = if matches!(target, Target::Warp | Target::Vocal) {
        preferred_audio_row(s)
    } else {
        None
    };
    let mut best = None;
    for (index, candidate) in hits.iter().enumerate() {
        if matches!(destination, Destination::Editor(win) if win != Win::Browser && candidate.win != Some(win))
        {
            continue;
        }
        let Some((rank, label)) = target.match_control(s, &candidate.action, audio_row) else {
            continue;
        };
        if best.is_some_and(|(previous, _, _)| previous >= rank) {
            continue;
        }
        let rect = candidate.rect.intersect(&screen);
        if rect.is_empty()
            || hits[index + 1..]
                .iter()
                .any(|hit| hit.rect.contains(rect.cx(), rect.cy()))
        {
            continue;
        }
        best = Some((rank, rect, label));
    }
    best.map(|(_, rect, label)| (rect, label))
}

pub(crate) fn paint(ui: &mut Ui, s: &State) {
    if !active(s)
        || is_open(s)
        || s.menu_open.is_some()
        || s.playlist.popup.is_some()
        || s.plugin_picker.open
        || s.native_view.is_some()
    {
        return;
    }
    let Some(visit) = &s.settings.help.spotlight else {
        return;
    };
    let (w, h) = ui.c.size();
    let screen = Rect::new(0., 0., w, h);
    let found = control(ui, s, visit.target, screen);
    let width = (w - 16.).clamp(1., 525.);
    let top = crate::views::work_y(s) + 8.;
    let mut banner = Rect::new(((w - width) / 2.).round(), (h - 62.).max(0.), width, 54.);
    if found.is_some_and(|(rect, _)| !banner.intersect(&rect.outset(5.)).is_empty()) {
        banner.y = top;
    }
    let label = if let Some((rect, label)) = found {
        let outline = rect.outset(3.).intersect(&screen.inset(1.));
        ui.c.stroke_rrect(outline, 3., 4., Color::rgb(0x121416));
        ui.c.stroke_rrect(outline, 3., 2., Color::rgb(0x2096f3));
        label
    } else if matches!(visit.target.destination(), Destination::Editor(win) if s.native_detached.contains(&win))
    {
        "The target window is detached. Dock it to show the highlight here."
    } else {
        "The control is out of view or hidden. Check the article for its location."
    };
    ui.hit(banner, Action::Swallow, "");
    ui.c.fill_rrect(banner, 7., Color::rgb(0x1b2026));
    ui.c.stroke_rrect(banner, 7., 1., Color::rgb(0x77818a));
    ui.c.text_clip(
        Font::new(12).bold(),
        banner.x + 10.,
        banner.y + 7.,
        banner.w - 20.,
        Color::rgb(0xffffff),
        label,
    );
    super::button(
        ui,
        Rect::new(banner.x + 9., banner.y + 27., 102., 21.),
        "Back to guide",
        Hit::ReturnToGuide,
    );
    super::button(
        ui,
        Rect::new(banner.x + 118., banner.y + 27., 65., 21.),
        "Dismiss",
        Hit::DismissSpotlight,
    );
}

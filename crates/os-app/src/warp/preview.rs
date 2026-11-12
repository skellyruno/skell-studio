// SPDX-License-Identifier: GPL-3.0-or-later
//! Non-destructive warp audition through the normal Playlist transport.
use super::*;
use crate::app::Win;
use os_engine::Sample;
use os_model::Project;
use std::sync::Weak;

pub(super) const VISIBILITY: &str = "visible_playlist-warp-preview";
pub(crate) const PREVIEW_PATH: &str = "lemon-internal://audio-timing-preview";

type Points = Vec<(f64, f64)>;
type Audio = Result<Arc<Sample>, String>;

#[derive(Clone, Copy)]
pub(crate) enum Play {
    Start(u32),
    Seek(u32),
}

#[derive(Default)]
pub(super) struct Preview {
    ready: Option<(Points, Audio)>,
    worker: Option<(Points, mpsc::Receiver<Audio>)>,
    waiting: Option<Play>,
    in_transport: bool,
    last_started: Option<Weak<Sample>>,
}

pub(super) fn enabled(s: &State) -> bool {
    s.shown(Win::Playlist)
        && crate::workspace_visibility::shown("visible_playlist-warp")
        && crate::workspace_visibility::shown(VISIBILITY)
}

fn usable(s: &State) -> bool {
    enabled(s) && !s.rec.armed
}

pub(super) fn render(sample: &Sample, points: &[(usize, usize)]) -> Audio {
    let (left, right) = os_dsp::stretch::warp(&sample.left, &sample.right, sample.rate, points)?;
    Ok(Arc::new(Sample {
        rate: sample.rate,
        left,
        right,
    }))
}

fn valid(s: &State, f: &Form) -> bool {
    f.arrangement == s.project.arrangement_index()
        && pl::clips(s).get(f.clip) == Some(&f.original)
        && s.project.channel(f.channel.index) == Some(&f.channel)
}

impl Preview {
    pub(super) fn busy(&self) -> bool {
        self.worker.is_some() || self.waiting.is_some()
    }

    fn result(&self, points: &Points) -> Option<&Audio> {
        self.ready
            .as_ref()
            .filter(|(key, _)| key == points)
            .map(|(_, result)| result)
    }

    pub(super) fn sample(&self, points: &Points) -> Option<Arc<Sample>> {
        self.result(points)?.as_ref().ok().cloned()
    }

    pub(super) fn status(&self, f: &Form) -> Option<&'static str> {
        if self.waiting.is_some() {
            Some("Preparing timing preview before playback...")
        } else if self.worker.is_some() {
            Some("Preparing timing preview...")
        } else if matches!(self.result(&f.edit.points), Some(Err(_))) {
            Some("Timing preview unavailable. Playback uses the committed audio.")
        } else if self.result(&f.edit.points).is_some_and(Result::is_ok) {
            Some("Preview ready. Use the Playlist ruler and global Play in Song mode.")
        } else {
            None
        }
    }
}

pub(crate) fn cancel_play(app: &mut App) -> bool {
    let cancelled = app
        .state
        .playlist
        .warp
        .as_mut()
        .is_some_and(|f| f.preview.waiting.take().is_some());
    if cancelled {
        app.invalidate();
    }
    cancelled
}

/// A fresh playback request waits for the latest edit, so Play never quietly
/// auditions the previous warp while a new version is being rendered.
pub(crate) fn wait_for_play(app: &mut App, play: Play) -> bool {
    if !usable(&app.state) || !app.state.song_mode {
        return false;
    }
    let Some(f) = app.state.playlist.warp.as_ref() else {
        return false;
    };
    if f.pending.is_some() || !valid(&app.state, f) || !f.has_edits() {
        return false;
    }
    let f = app.state.playlist.warp.as_mut().unwrap();
    if f.preview.result(&f.edit.points).is_some() {
        return false;
    }
    f.preview.waiting = Some(play);
    app.invalidate();
    true
}

pub(crate) fn mark_started(app: &mut App, sample: Option<&Arc<Sample>>) {
    if let Some(f) = app.state.playlist.warp.as_mut() {
        f.preview.last_started = sample.map(Arc::downgrade);
    }
}

pub(crate) fn resume(app: &mut App) {
    let restart = app.state.song_mode
        && app.transport.song()
        && app.state.playlist.warp.as_ref().is_some_and(|f| {
            let needed =
                usable(&app.state) && f.pending.is_none() && valid(&app.state, f) && f.has_edits();
            let result = needed.then(|| f.preview.result(&f.edit.points)).flatten();
            if needed && result.is_none() {
                return true;
            }
            let current = result
                .and_then(|result| result.as_ref().ok())
                .map(Arc::as_ptr);
            current != f.preview.last_started.as_ref().map(Weak::as_ptr)
        });
    if restart {
        app.play_song_from(app.transport.tick().round().max(0.) as u32);
    } else {
        app.transport.resume();
    }
}

pub(super) fn poll(app: &mut App) {
    let usable = usable(&app.state);
    let song = app.state.song_mode;
    let dragging = matches!(app.drag, Some(crate::app::Drag::WarpMarker { .. }));
    let Some(f) = app.state.playlist.warp.as_mut() else {
        return;
    };
    let finished = f
        .preview
        .worker
        .as_ref()
        .and_then(|(_, worker)| match worker.try_recv() {
            Ok(audio) => Some(audio),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err("The timing preview worker stopped".into()))
            }
        });
    let completed = finished.is_some();
    if let Some(result) = finished {
        let (key, _) = f.preview.worker.take().unwrap();
        if key == f.edit.points {
            f.preview.ready = Some((key, result));
        }
        app.state.piano.dirty = true;
    }

    let needed = usable && f.pending.is_none() && f.has_edits();
    let result = f.preview.result(&f.edit.points);
    let ready = result.is_some();
    let in_transport = needed && song && result.is_some_and(Result::is_ok);
    if in_transport != f.preview.in_transport {
        f.preview.in_transport = in_transport;
        app.state.piano.dirty = true;
    }
    if needed && !ready && f.preview.worker.is_none() && !dragging {
        let (send, receive) = mpsc::channel();
        let sample = f.sample.clone();
        let frames = f.frame_points();
        f.preview.worker = Some((f.edit.points.clone(), receive));
        std::thread::spawn(move || {
            let _ = send.send(render(&sample, &frames));
        });
        app.invalidate();
        return;
    }
    let request = if !usable || !song || f.pending.is_some() {
        f.preview.waiting = None;
        None
    } else if !needed || ready {
        f.preview.waiting.take()
    } else {
        None
    };
    if completed {
        app.invalidate();
    }
    if let Some(request) = request {
        match request {
            Play::Start(bars) => app.start_playback_counted(bars),
            Play::Seek(tick) => app.play_song_from(tick),
        }
    }
}

/// Substitute only the edited clip in the playback copy. The real channel, all
/// other clips sharing its source, and the saved/exported project stay intact.
pub(crate) fn playback(s: &State, project: &mut Project) -> Option<Arc<Sample>> {
    if !s.song_mode || !usable(s) {
        return None;
    }
    let f = s
        .playlist
        .warp
        .as_ref()
        .filter(|f| f.pending.is_none() && valid(s, f) && f.has_edits())?;
    let sample = f.preview.sample(&f.edit.points)?;
    let index = project
        .channels
        .iter()
        .map(|ch| ch.index)
        .max()?
        .checked_add(1)?;
    let mut channel = project.channel(f.channel.index)?.clone();
    channel.index = index;
    channel.sample_path = Some(PREVIEW_PATH.into());
    channel.plugin_state = None;
    channel.plugin = None;
    channel.kind = os_model::ChannelKind::AudioClip;
    channel.sound.stretch[0] = 0;
    channel.sound.stretch[2] = 0;
    crate::loop_fit::set_fitted_time(&mut channel.sound, f.end());
    let clip = project.arrangement_mut()?.clips.get_mut(f.clip)?;
    clip.source = ClipSource::Channel(index);
    clip.window = None;
    project.channels.push(channel);
    Some(sample)
}

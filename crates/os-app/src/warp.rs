// SPDX-License-Identifier: GPL-3.0-or-later
//! Editable audio timing anchors, rendered off the audio/UI threads with the source kept intact.
use crate::{
    app::{Action, App, State, Ui},
    icons::{self, Icon},
    playlist as pl,
    vocal::view as editor_style,
};
use os_model::{AudioWarp, ClipSource};
use os_ui::{Color, Rect};
use std::{
    cell::Cell,
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
};
mod preview;
pub(crate) use preview::{
    PREVIEW_PATH, Play, cancel_play, mark_started, playback, resume, wait_for_play,
};
pub(crate) const PREVIEW_VISIBILITY: &str = preview::VISIBILITY;

pub struct Loading {
    pub original: os_model::Clip,
    arrangement: usize,
    job: Arc<Mutex<Option<Result<Form, String>>>>,
}
type Job = mpsc::Receiver<Result<PathBuf, String>>;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Source,
    Select,
    Marker(usize),
    Delete,
    Reset,
    Fit,
    Snap,
    Apply,
    Toggle(u16),
    Close,
    Analyze,
    Quantize,
    Grid,
    Strength,
    Undo,
    Redo,
    Next,
}
pub struct Form {
    pub arrangement: usize,
    pub transients: Vec<f64>,
    pub analysis: Option<Arc<Mutex<Option<Vec<f64>>>>>,
    pub grid: usize,
    pub strength: usize,
    pub offset: f64,
    pub undo: Vec<Vec<(f64, f64)>>,
    pub redo: Vec<Vec<(f64, f64)>>,
    pub clip: usize,
    pub original: os_model::Clip,
    pub channel: os_model::Channel,
    pub edit: AudioWarp,
    initial_points: Vec<(f64, f64)>,
    preview: preview::Preview,
    pub sample: Arc<os_engine::Sample>,
    pub selected: Option<usize>,
    pub select_mode: bool,
    pub selection: Vec<f64>,
    selection_start: f64,
    selection_base: Vec<f64>,
    pub amplitude: f32,
    pub wave: Cell<Rect>,
    pub snap: bool,
    pub tempo: f64,
    pub pending: Option<Job>,
    pub message: String,
}
pub const EXTRA: f32 = crate::vocal::EXTRA;
const GRIDS: [f64; 6] = [1.0, 0.5, 0.25, 0.125, 1.0 / 3.0, 1.0 / 6.0];
const GRID_NAMES: [&str; 6] = ["1/4", "1/8", "1/16", "1/32", "1/8 triplet", "1/16 triplet"];
pub fn expanded(s: &State, row: u16) -> bool {
    if !crate::workspace_visibility::shown("visible_playlist-warp") {
        return false;
    }
    s.playlist
        .warp
        .as_ref()
        .is_some_and(|f| f.original.track == row)
        || s.playlist
            .warp_loading
            .as_ref()
            .is_some_and(|f| f.original.track == row)
}
fn interpolate(points: &[(f64, f64)], x: f64, inverse: bool) -> f64 {
    let axis = |p: (f64, f64)| if inverse { (p.1, p.0) } else { p };
    let i = points
        .partition_point(|&p| axis(p).0 <= x)
        .clamp(1, points.len() - 1);
    let (a, b) = axis(points[i - 1]);
    let (c, d) = axis(points[i]);
    b + (x - a) / (c - a) * (d - b)
}
// Use the renderer's integer frame limits so neighboring anchors can't round
// to the same destination or request an unsupported stretch ratio.
fn move_bounds(points: &[(f64, f64)], i: usize, rate: u32, tempo: f64) -> Option<(f64, f64)> {
    let per_beat = rate as f64 * 60. / tempo;
    let frame = |seconds: f64| (seconds * rate as f64).round() as i64;
    let left_src = frame(points[i].0) - frame(points[i - 1].0);
    let right_src = frame(points[i + 1].0) - frame(points[i].0);
    if left_src <= 0 || right_src <= 0 {
        return None;
    }
    let left_dst = (points[i - 1].1 * per_beat).round() as i64;
    let right_dst = (points[i + 1].1 * per_beat).round() as i64;
    let lo = (left_dst + (left_src + 15) / 16).max(right_dst - right_src * 16);
    let hi = (right_dst - (right_src + 15) / 16).min(left_dst + left_src * 16);
    (lo <= hi).then_some((lo as f64 / per_beat, hi as f64 / per_beat))
}

pub fn busy(s: &State) -> bool {
    s.playlist.warp_loading.is_some()
        || s.playlist.warp.as_ref().is_some_and(|form| {
            form.analysis.is_some() || form.pending.is_some() || form.preview.busy()
        })
}

pub(crate) fn applying(s: &State) -> bool {
    s.playlist
        .warp
        .as_ref()
        .is_some_and(|form| form.pending.is_some())
}

impl Form {
    fn has_edits(&self) -> bool {
        if self.edit.points == self.initial_points {
            return false;
        }
        let tolerance = self.tempo / (120. * self.sample.rate as f64);
        let matches = |a: &[(f64, f64)], b: &[(f64, f64)]| {
            a.iter()
                .all(|&(src, beat)| (interpolate(b, src, false) - beat).abs() < tolerance)
        };
        !matches(&self.edit.points, &self.initial_points)
            || !matches(&self.initial_points, &self.edit.points)
    }

    fn frame_points(&self) -> Vec<(usize, usize)> {
        let rate = self.sample.rate as f64;
        self.edit
            .points
            .iter()
            .map(|&(src, beat)| {
                (
                    (src * rate).round() as usize,
                    (beat * 60. / self.tempo * rate).round() as usize,
                )
            })
            .collect()
    }

    fn select_to(&mut self, x: f32) {
        let r = self.wave.get();
        let beat = ((x - r.x) / r.w).clamp(0., 1.) as f64 * self.end();
        let tolerance = if (beat - self.selection_start).abs() * r.w as f64 / self.end() < 6. {
            6. * self.end() / r.w as f64
        } else {
            0.
        };
        let lo = beat.min(self.selection_start) - tolerance;
        let hi = beat.max(self.selection_start) + tolerance;
        self.selection = self.selection_base.clone();
        for src in self
            .transients
            .iter()
            .copied()
            .chain(self.edit.points.iter().map(|p| p.0))
        {
            let dst = interpolate(&self.edit.points, src, false);
            if dst >= lo && dst <= hi && !self.selection.contains(&src) {
                self.selection.push(src);
            }
        }
        self.message = format!(
            "{} points selected  -  Quantize applies only to these points. Shift-drag adds more.",
            self.selection.len()
        );
    }
    fn checkpoint(&mut self) {
        if self.undo.last() != Some(&self.edit.points) {
            self.undo.push(self.edit.points.clone());
        }
        if self.undo.len() > 64 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn end(&self) -> f64 {
        self.edit.points.last().unwrap().1
    }
    fn range(&self) -> (f64, f64) {
        (self.edit.points[0].0, self.edit.points.last().unwrap().0)
    }
    fn moved(&mut self, i: usize, x: f32) {
        if self.pending.is_some() || i == 0 || i + 1 >= self.edit.points.len() {
            return;
        }
        let r = self.wave.get();
        let mut beat = ((x - r.x) / r.w).clamp(0., 1.) as f64 * self.end();
        if self.snap {
            let step = GRIDS[self.grid];
            beat = ((beat + self.offset) / step).round() * step - self.offset;
        }
        if let Some((lo, hi)) = move_bounds(&self.edit.points, i, self.sample.rate, self.tempo) {
            self.edit.points[i].1 = beat.clamp(lo, hi);
        }
    }
}
pub fn open(app: &mut App, i: usize) -> Result<(), String> {
    if applying(&app.state) {
        return Err("Wait for audio timing to finish applying".into());
    }
    app.drag = None;
    let clip = pl::clips(&app.state)
        .get(i)
        .cloned()
        .ok_or("Select an audio clip")?;
    let ClipSource::Channel(id) = clip.source else {
        return Err("Select an audio clip".into());
    };
    let ch = app
        .state
        .project
        .channel(id)
        .cloned()
        .ok_or("Missing channel")?;
    if crate::ara::is(&ch) {
        return Err("Use the ARA editor for this clip".into());
    }
    let path = ch.sample_path.as_deref().ok_or("The clip has no sample")?;
    if ch.sound.start_offset() != 0.
        || ch.sound.fx.start() != 0.
        || ch.sound.fx.length() != 1.
        || ch.sound.fx.trim != 0
        || ch.sound.fx.flags & os_model::SampleFx::REVERSE != 0
    {
        return Err("Render this clip to audio before warping its sampler trim, offset or reverse processing".into());
    }
    // A trimmed render is a new source window; keep that audio intact rather than
    // incorrectly mapping its milliseconds onto the earlier unwarped source.
    let resolved = crate::plugins::resolve_sample(path);
    let saved = app
        .state
        .project
        .audio_warps
        .iter()
        .find(|w| {
            clip.window.is_none()
                && (w.rendered == path || crate::plugins::resolve_sample(&w.rendered) == resolved)
        })
        .cloned();
    let tempo = app.state.project.tick_tempo(app.state.tempo);
    let ppq = app.state.project.ppq;
    let arrangement = app.state.project.arrangement_index();
    let done = Arc::new(Mutex::new(None));
    app.state.playlist.vocal.row = None;
    clear_form(app);
    app.state.playlist.warp_loading = Some(Loading {
        original: clip.clone(),
        arrangement,
        job: done.clone(),
    });
    std::thread::spawn(move || {
        let result = prepare(i, clip, ch, saved, tempo, ppq, arrangement);
        if let Ok(mut out) = done.lock() {
            *out = Some(result);
        }
    });
    app.invalidate();
    Ok(())
}
fn prepare(
    i: usize,
    clip: os_model::Clip,
    ch: os_model::Channel,
    saved: Option<AudioWarp>,
    tempo: f64,
    ppq: u16,
    arrangement: usize,
) -> Result<Form, String> {
    let source = saved
        .as_ref()
        .map_or(ch.sample_path.as_deref().unwrap(), |w| w.source.as_str());
    let source = crate::plugins::resolve_sample(source);
    if std::fs::metadata(&source).is_ok_and(|m| m.len() > 128 * 1024 * 1024) {
        return Err("Split this large recording into smaller clips before warping".into());
    }
    let sample =
        os_engine::sample::load_shared(std::path::Path::new(&source)).map_err(|e| e.to_string())?;
    let secs = sample.frames() as f64 / sample.rate.max(1) as f64;
    let (a, b) = clip
        .window
        .map(|(a, b)| (a / 1000., b / 1000.))
        .unwrap_or((0., secs));
    let end = clip.length as f64 / ppq.max(1) as f64;
    if end <= 0. || b.min(secs) - a.max(0.) < 0.01 {
        return Err("The clip is too short to warp".into());
    }
    let mut edit = saved.unwrap_or(AudioWarp {
        source: source.clone(),
        rendered: String::new(),
        points: vec![(a.max(0.), 0.), (b.min(secs), end)],
    });
    if edit.points.len() < 2
        || edit.points.len() > 1024
        || edit
            .points
            .iter()
            .any(|p| !p.0.is_finite() || !p.1.is_finite() || p.0 < 0.0 || p.0 > secs)
        || edit.points.last().is_some_and(|p| p.1 <= 0.0)
        || edit.points.windows(2).any(|p| {
            !p[0].0.is_finite() || !p[0].1.is_finite() || p[1].0 <= p[0].0 || p[1].1 <= p[0].1
        })
    {
        return Err("This saved warp has invalid anchors".into());
    }
    edit.source = source;
    let scale = end / edit.points.last().unwrap().1;
    for p in &mut edit.points {
        p.1 *= scale;
    }
    Ok(Form {
        arrangement,
        transients: vec![],
        analysis: None,
        grid: 2,
        strength: 4,
        offset: clip.position as f64 / ppq.max(1) as f64,
        undo: vec![],
        redo: vec![],
        clip: i,
        original: clip,
        channel: ch,
        initial_points: edit.points.clone(),
        preview: preview::Preview::default(),
        edit,
        sample,
        selected: None,
        select_mode: false,
        selection: vec![],
        selection_start: 0.,
        selection_base: vec![],
        amplitude: 1.,
        wave: Cell::new(Rect::new(0., 0., 1., 1.)),
        snap: true,
        tempo,
        pending: None,
        message: "Detecting transients...".into(),
    })
}
pub fn command(app: &mut App, c: Command) {
    run(app, c, false);
}

pub(crate) fn confirm_apply(app: &mut App) {
    if app.state.playlist.warp.as_ref().is_some_and(|form| {
        form.arrangement == app.state.project.arrangement_index()
            && pl::clips(&app.state).get(form.clip) == Some(&form.original)
            && app.state.project.channel(form.channel.index) == Some(&form.channel)
    }) {
        run(app, Command::Apply, true);
    }
}

fn run(app: &mut App, c: Command, confirmed: bool) {
    if applying(&app.state) {
        return;
    }
    if c == Command::Fit {
        let Some(f) = app.state.playlist.warp.as_ref() else {
            return;
        };
        let width = app.pl_grid().map_or(800.0, |r| r.w);
        let beats = f.end();
        let offset = f.original.position as f32 / app.state.project.ppq.max(1) as f32;
        app.state.playlist_bar_w = ((width - 32.).max(80.) * 4. / beats as f32).clamp(4., 48000.);
        app.state.playlist_scroll.0 = (offset * app.state.playlist_bar_w / 4. - 16.).max(0.);
        app.invalidate();
        return;
    }

    if let Command::Toggle(row) = c {
        if expanded(&app.state, row) {
            clear_form(app);
            app.invalidate();
            return;
        }
        let clips = pl::clips(&app.state);
        let audio = |i: usize| {
            clips.get(i).is_some_and(|clip|clip.track==row && matches!(clip.source,ClipSource::Channel(id) if app.state.project.channel(id).is_some_and(|ch|ch.sample_path.is_some())))
        };
        let i = app
            .state
            .playlist
            .sel
            .iter()
            .copied()
            .find(|&i| audio(i))
            .or_else(|| (0..clips.len()).find(|&i| audio(i)));
        app.state.message = match i {
            Some(i) => open(app, i).err(),
            None => Some("Choose a track containing audio to warp".into()),
        };
        app.invalidate();
        return;
    }
    if c == Command::Close {
        app.drag = None;
        clear_form(app);
        app.invalidate();
        return;
    }
    if c == Command::Next {
        let Some(f) = app.state.playlist.warp.as_ref() else {
            return;
        };
        let clips = pl::clips(&app.state);
        let row = f.original.track;
        let current = f.clip;
        let next=(1..=clips.len()).map(|n|(current+n)%clips.len()).find(|&i|clips[i].track==row && matches!(clips[i].source,ClipSource::Channel(id) if app.state.project.channel(id).is_some_and(|ch|ch.sample_path.is_some())));
        if let Some(i) = next {
            app.state.message = open(app, i).err();
        }
        app.invalidate();
        return;
    }
    let x = app.mouse.0;
    let Some(f) = app.state.playlist.warp.as_mut() else {
        return;
    };
    if f.pending.is_some() {
        return;
    }
    if matches!(
        c,
        Command::Source | Command::Marker(_) | Command::Delete | Command::Reset | Command::Quantize
    ) {
        f.checkpoint();
    }
    match c {
        Command::Toggle(_) | Command::Close | Command::Next | Command::Fit => {}
        Command::Select => {
            f.select_mode = !f.select_mode;
            f.selection.clear();
            f.selected = None;
            f.message = if f.select_mode {"Drag across points to select; Shift-drag adds. Then Quantize."} else {"Click to add a marker; drag its line. Ctrl+wheel zooms time; Alt+wheel enlarges waveform."}.into();
        }
        Command::Grid => f.grid = (f.grid + 1) % GRIDS.len(),
        Command::Strength => f.strength = (f.strength + 1) % 5,
        Command::Undo => {
            if let Some(points) = f.undo.pop() {
                f.redo.push(std::mem::replace(&mut f.edit.points, points));
                f.selected = None;
            }
        }
        Command::Redo => {
            if let Some(points) = f.redo.pop() {
                f.undo.push(std::mem::replace(&mut f.edit.points, points));
                f.selected = None;
            }
        }
        Command::Analyze => {
            if f.analysis.is_some() {
                return;
            }
            let done = Arc::new(Mutex::new(None));
            f.analysis = Some(done.clone());
            let sample = f.sample.clone();
            let (a, b) = f.range();
            f.message = "Detecting transients...".into();
            std::thread::spawn(move || {
                let start = (a * sample.rate as f64).round() as usize;
                let end = ((b * sample.rate as f64).round() as usize).min(sample.frames());
                let times = crate::vocal_align::transient_times(sample, start, end)
                    .into_iter()
                    .take(1022)
                    .map(|t| a + t)
                    .collect();
                if let Ok(mut result) = done.lock() {
                    *result = Some(times);
                }
            });
        }
        Command::Quantize => {
            if f.analysis.is_some() {
                f.message = "Wait for transient detection to finish".into();
                return;
            }
            if f.select_mode && f.selection.is_empty() {
                f.message = "Select points first, or switch to Edit to quantize all.".into();
                return;
            }
            let (a, b) = f.range();
            let mut points = f.edit.points.clone();
            let source_frame = |src: f64| (src * f.sample.rate as f64).round() as usize;
            let per_beat = f.sample.rate as f64 * 60. / f.tempo;
            let destination_frame = |beat: f64| (beat * per_beat).round() as usize;
            for &src in &f.transients {
                if points.len() >= 1024 {
                    break;
                }
                if src <= a || src >= b || (f.select_mode && !f.selection.contains(&src)) {
                    continue;
                }
                let i = points.partition_point(|point| point.0 < src);
                let beat = interpolate(&f.edit.points, src, false);
                if source_frame(src) <= source_frame(points[i - 1].0)
                    || source_frame(src) >= source_frame(points[i].0)
                    || destination_frame(beat) <= destination_frame(points[i - 1].1)
                    || destination_frame(beat) >= destination_frame(points[i].1)
                {
                    continue;
                }
                points.insert(i, (src, beat));
            }
            let step = GRIDS[f.grid];
            let offset = f.offset;
            let mut count = 0;
            for i in 1..points.len() - 1 {
                if f.select_mode && !f.selection.contains(&points[i].0) {
                    continue;
                }
                let beat = points[i].1;
                let snapped = ((beat + offset) / step).round() * step - offset;
                let dst = beat + (snapped - beat) * f.strength as f64 / 4.;
                // Leave unselected anchors intact and keep both segments renderable.
                if let Some((lo, hi)) = move_bounds(&points, i, f.sample.rate, f.tempo) {
                    points[i].1 = dst.clamp(lo, hi);
                    count += 1;
                }
            }
            f.edit.points = points;
            f.selected = None;
            f.message = format!("{count} points quantized. Apply to hear, or Undo to compare.");
        }
        Command::Source => {
            if f.select_mode {
                let r = f.wave.get();
                f.selection_start = ((x - r.x) / r.w).clamp(0., 1.) as f64 * f.end();
                f.selection_base = if app.shift {
                    f.selection.clone()
                } else {
                    vec![]
                };
                f.select_to(x);
                app.drag = Some(crate::app::Drag::WarpSelection);
                app.invalidate();
                return;
            }
            let r = f.wave.get();
            let beat = ((x - r.x) / r.w).clamp(0., 1.) as f64 * f.end();
            let mut src = interpolate(&f.edit.points, beat, true);
            if let Some(&near) = f.transients.iter().min_by(|a, b| {
                (interpolate(&f.edit.points, **a, false) - beat)
                    .abs()
                    .total_cmp(&(interpolate(&f.edit.points, **b, false) - beat).abs())
            }) {
                if ((interpolate(&f.edit.points, near, false) - beat) / f.end() * r.w as f64).abs()
                    < 8.0
                {
                    src = near;
                }
            }
            if f.edit.points.len() >= 1024 {
                return;
            }
            let i = f.edit.points.partition_point(|p| p.0 < src);
            if i == 0
                || i == f.edit.points.len()
                || src - f.edit.points[i - 1].0 < 0.005
                || f.edit.points[i].0 - src < 0.005
            {
                return;
            }
            let (l, r) = (f.edit.points[i - 1], f.edit.points[i]);
            let dst = l.1 + (src - l.0) / (r.0 - l.0) * (r.1 - l.1);
            f.edit.points.insert(i, (src, dst));
            f.selected = Some(i);
            app.drag = Some(crate::app::Drag::WarpMarker {
                index: i,
                grab_offset: 0.,
            });
        }
        Command::Marker(i) => {
            if f.select_mode {
                let src = f.edit.points[i].0;
                if !app.shift {
                    f.selection.clear();
                }
                if let Some(at) = f.selection.iter().position(|p| *p == src) {
                    f.selection.remove(at);
                } else {
                    f.selection.push(src);
                }
                app.invalidate();
                return;
            }
            f.selected = Some(i);
            if i > 0 && i + 1 < f.edit.points.len() {
                let wave = f.wave.get();
                let marker_x = wave.x + (f.edit.points[i].1 / f.end()) as f32 * wave.w;
                app.drag = Some(crate::app::Drag::WarpMarker {
                    index: i,
                    grab_offset: x - marker_x,
                });
            }
        }
        Command::Delete => {
            if f.select_mode {
                let (a, b) = f.range();
                f.edit
                    .points
                    .retain(|p| p.0 == a || p.0 == b || !f.selection.contains(&p.0));
                f.selection.clear();
            }
            if let Some(i) = f
                .selected
                .take()
                .filter(|&i| i > 0 && i + 1 < f.edit.points.len())
            {
                f.edit.points.remove(i);
            }
        }
        Command::Reset => {
            let (a, b) = f.range();
            let end = f.end();
            f.edit.points = vec![(a, 0.), (b, end)];
            f.selected = None;
        }
        Command::Snap => f.snap = !f.snap,
        Command::Apply => {
            if !f.has_edits() {
                f.message =
                    "No timing changes to apply. Drag a marker or use Quantize first.".into();
                app.invalidate();
                return;
            }
            let warning = &crate::messages::WARP_BAKES_VOCAL_TUNING;
            let bakes_vocal_tuning = app.state.project.vocal_edits.iter().any(|edit| {
                edit.channel == f.channel.index
                    && !edit.rendered.is_empty()
                    && edit.rendered != edit.source
                    && crate::plugins::resolve_sample(&edit.rendered) == f.edit.source
            });
            if !confirmed && crate::messages::on(warning) && bakes_vocal_tuning {
                use crate::dialog::{Answer, Kind, Msg, Then, spans};
                let mut message = Msg::new(
                    Kind::Warning,
                    "Apply audio timing?",
                    spans(&["Applying audio timing will bake the clip's vocal tuning into the new audio. You can still undo the warp to return to the editable tuned clip.\n\nCheck Don't show again to skip this warning next time."]),
                    &[Answer::Yes, Answer::Cancel],
                    Then::WarpApply,
                ).labelled(&["Apply", "Cancel"]);
                message.focus = 1;
                message.remember = Some((warning, false));
                app.show_msg(message);
                return;
            }
            let points = f.frame_points();
            let sample = f.sample.clone();
            let previewed = f.preview.sample(&f.edit.points);
            let (send, receive) = mpsc::channel();
            f.pending = Some(receive);
            f.message = "Applying audio timing...".into();
            let dir = crate::pattern_render::folder(&app.state).join("Warped");
            std::thread::spawn(move || {
                let result = (|| {
                    let rendered = match previewed {
                        Some(rendered) => rendered,
                        None => preview::render(&sample, &points)?,
                    };
                    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                    let stamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos();
                    let path = dir.join(format!("Warp-{stamp}.wav"));
                    let mut file = std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&path)
                        .map_err(|e| e.to_string())?;
                    let stereo: Vec<_> = rendered
                        .left
                        .iter()
                        .zip(&rendered.right)
                        .flat_map(|(&l, &r)| [l, r])
                        .collect();
                    os_engine::wav::write_f32_stereo(&mut file, sample.rate, &stereo)
                        .map_err(|e| e.to_string())?;
                    Ok(path)
                })();
                let _ = send.send(result);
            });
        }
    }
    app.invalidate();
}
pub fn drag(app: &mut App, i: usize, x: f32) {
    if let Some(f) = app.state.playlist.warp.as_mut() {
        f.moved(i, x);
        app.invalidate();
    }
}
pub fn drag_selection(app: &mut App, x: f32) {
    if let Some(f) = app.state.playlist.warp.as_mut() {
        f.select_to(x);
        app.invalidate();
    }
}
pub fn wheel(app: &mut App, dx: f32, dy: f32, ctrl: bool) -> bool {
    if !crate::workspace_visibility::shown("visible_playlist-warp") {
        return false;
    }
    if !matches!(
        app.top_hit(app.mouse.0, app.mouse.1).map(|h| &h.action),
        Some(Action::Warp(_))
    ) {
        return false;
    }
    let Some(f) = app.state.playlist.warp.as_mut() else {
        return false;
    };
    if !f.wave.get().contains(app.mouse.0, app.mouse.1) || (!ctrl && !app.alt) {
        return false;
    }
    if app.drag.is_some() {
        return true;
    }
    let factor = 1.18f32.powf((dx + dy).clamp(-12., 12.));
    if app.alt {
        f.amplitude = (f.amplitude * factor).clamp(0.25, 16.);
    } else {
        crate::vocal::navigation::zoom_x(app, factor, app.mouse.0);
    }
    app.invalidate();
    true
}
fn clear_form(app: &mut App) {
    if app.state.playlist.warp.take().is_some() {
        app.state.piano.dirty = true;
    }
    app.state.playlist.warp_loading = None;
}

pub fn poll(app: &mut App) {
    if let Some(load) = app.state.playlist.warp_loading.as_ref() {
        let valid = load.arrangement == app.state.project.arrangement_index()
            && pl::clips(&app.state).contains(&load.original);
        let result = load.job.lock().ok().and_then(|mut job| job.take());
        if !valid {
            app.state.playlist.warp_loading = None;
            app.invalidate();
        } else if let Some(result) = result {
            app.state.playlist.warp_loading = None;
            match result {
                Ok(form) => {
                    app.state.playlist.warp = Some(form);
                    command(app, Command::Analyze);
                    command(app, Command::Fit);
                }
                Err(error) => app.state.message = Some(error),
            }
            app.invalidate();
        }
    }
    if app.state.playlist.warp.as_ref().is_some_and(|f| {
        f.arrangement != app.state.project.arrangement_index()
            || pl::clips(&app.state).get(f.clip) != Some(&f.original)
            || app.state.project.channel(f.channel.index) != Some(&f.channel)
    }) {
        if applying(&app.state) {
            app.state.message = Some(
                "The clip or its source changed during rendering. Audio timing was not applied."
                    .into(),
            );
        }
        clear_form(app);
        app.invalidate();
        return;
    }
    preview::poll(app);
    let Some(f) = app.state.playlist.warp.as_mut() else {
        return;
    };
    let detected = f.analysis.as_ref().and_then(|job| job.lock().ok()?.take());
    if let Some(times) = detected {
        f.transients = times;
        f.analysis = None;
        if f.pending.is_none() {
            f.message = format!(
                "{} transients detected. Drag markers or Quantize. Apply to keep your edits.",
                f.transients.len()
            );
        }
        app.invalidate();
    }
    let Some(f) = app.state.playlist.warp.as_mut() else {
        return;
    };
    let result = f.pending.as_ref().and_then(|job| match job.try_recv() {
        Ok(result) => Some(result),
        Err(mpsc::TryRecvError::Empty) => None,
        Err(mpsc::TryRecvError::Disconnected) => {
            Some(Err("The render worker stopped. Try Apply again.".into()))
        }
    });
    let Some(result) = result else { return };
    f.pending = None;
    let path = match result {
        Ok(p) => p,
        Err(e) => {
            f.message = format!("Could not apply audio timing: {e}");
            app.state.message = Some(f.message.clone());
            app.invalidate();
            return;
        }
    };
    let Some(mut f) = app.state.playlist.warp.take() else {
        return;
    };
    if app.state.project.arrangement_index() != f.arrangement
        || pl::clips(&app.state).get(f.clip) != Some(&f.original)
    {
        app.state.message = Some("The clip changed; warp was rendered but not applied".into());
        return;
    }
    let Some(index) = app
        .state
        .project
        .channels
        .iter()
        .map(|c| c.index)
        .max()
        .unwrap_or(0)
        .checked_add(1)
    else {
        app.state.message = Some("No channel IDs available".into());
        return;
    };
    let row = app.state.project.channels.len();
    let before = crate::history::Snap::Several(vec![
        if usize::from(index) == row
            && app
                .state
                .project
                .channels
                .iter()
                .enumerate()
                .all(|(row, channel)| usize::from(channel.index) == row)
        {
            crate::history::Snap::Rack(Box::new(crate::history::RackSide::now(&app.state, row)))
        } else {
            crate::history::Snap::Rows(Box::new(crate::history::Rows::now(
                &app.state,
                false,
                vec![row],
                vec![],
            )))
        },
        crate::history::Snap::AudioWarps(app.state.project.audio_warps.clone()),
    ]);
    pl::snapshot_with(&mut app.state, "Warp audio", before);
    f.channel.index = index;
    f.channel.sample_path = Some(path.to_string_lossy().into_owned());
    f.channel.plugin_state = None;
    f.channel.plugin = None;
    f.channel.kind = os_model::ChannelKind::AudioClip;
    let end = f.end();
    f.channel.sound.stretch[0] = 0;
    f.channel.sound.stretch[2] = 0;
    crate::loop_fit::set_fitted_time(&mut f.channel.sound, end);
    f.channel.name = Some(format!(
        "{} (warped)",
        f.channel.name.as_deref().unwrap_or("Audio")
    ));
    // Imported projects may carry sparse IDs. Keep the ID written into the clip.
    app.state.project.channels.push(f.channel);
    crate::rack::autosize(&mut app.state);
    f.edit.rendered = path.to_string_lossy().into_owned();
    app.state.project.audio_warps.push(f.edit.clone());
    let ppq = app.state.project.ppq.max(1);
    if let Some(c) = pl::arr_mut(&mut app.state).clips.get_mut(f.clip) {
        c.source = ClipSource::Channel(index);
        c.length = (end * ppq as f64).round().max(1.) as u32;
        c.window = None;
    }
    app.state.piano.dirty = true;
    app.state.message = Some(
        "Warp applied. Reopen Audio timing to edit; Ctrl+Z restores the original clip.".into(),
    );
    app.invalidate();
}
pub fn key_input(app: &mut App, key: &winit::keyboard::Key, ctrl: bool, alt: bool) -> bool {
    if !crate::workspace_visibility::shown("visible_playlist-warp") {
        return false;
    }
    use winit::keyboard::{Key, NamedKey};
    if alt
        || app.state.playlist.warp.is_none()
        || app.state.z.last() != Some(&crate::app::Win::Playlist)
        || app.state.playlist.popup.is_some()
    {
        return false;
    }
    let action = match key {
        Key::Named(NamedKey::Escape) => Some(Command::Close),
        Key::Named(NamedKey::Delete | NamedKey::Backspace) if !ctrl => Some(Command::Delete),
        Key::Character(c) if ctrl && c.eq_ignore_ascii_case("z") => Some(if app.shift {
            Command::Redo
        } else {
            Command::Undo
        }),
        Key::Character(c) if ctrl && c.eq_ignore_ascii_case("y") => Some(Command::Redo),
        _ => None,
    };
    if let Some(action) = action {
        command(app, action);
        true
    } else {
        false
    }
}

pub fn head(ui: &mut Ui, s: &State, row: u16, r: Rect) {
    if !crate::workspace_visibility::shown("visible_playlist-warp") {
        return;
    }
    let on = expanded(s, row);
    let hot = ui.hit(
        r,
        Action::Warp(Command::Toggle(row)),
        "Audio timing: detect transients and warp this track",
    );
    editor_style::track_button_face(ui, r, on, hot);
    let color = if on { ui.t.glyph } else { ui.t.text_faint };
    let r = if ui.t.style.vintage_2005 {
        r.inset(2.)
    } else {
        r
    };
    // A movable timing pin with outward arrows.
    let x = r.cx();
    let y = r.cy();
    ui.c.line(x, r.y + 2., x, r.bottom() - 2., 1.5, color);
    ui.c.fill_polygon(
        &[(x - 3., r.y + 2.), (x + 3., r.y + 2.), (x, r.y + 5.)],
        color,
    );
    ui.c.line(r.x + 2., y, r.right() - 2., y, 1., color);
    for (tip, dir) in [(r.x + 1., 1.), (r.right() - 1., -1.)] {
        ui.c.line(tip, y, tip + dir * 3., y - 3., 1., color);
        ui.c.line(tip, y, tip + dir * 3., y + 3., 1., color);
    }
}
pub fn inline(ui: &mut Ui, s: &State, g: &pl::PGeom) {
    if !crate::workspace_visibility::shown("visible_playlist-warp") {
        return;
    }
    let form = s.playlist.warp.as_ref();
    let loading = s.playlist.warp_loading.as_ref();
    let Some(row) = loading
        .map(|f| f.original.track)
        .or_else(|| form.map(|f| f.original.track))
    else {
        return;
    };
    if g.rows.strip(row) || g.rows.height(row) == 0. {
        return;
    }
    let y = g.y_of(row) + g.rows.base_height(row);
    let panel = Rect::new(g.grid.x, y, g.grid.w, EXTRA);
    let area = Rect::new(panel.x, panel.y, panel.w, panel.h - 3.);
    let visible = panel.intersect(&g.grid);
    // A partial Playlist paint still rebuilds its hit map, even outside the damaged pixels.
    if visible.is_empty() {
        return;
    }
    ui.c.push_clip(visible);
    ui.clip_hit_rect(visible, |ui| {
        editor_style::editor_face(ui, area);
        ui.hit(panel, Action::Swallow, "Audio timing editor");
        if loading.is_some() {
            crate::loading::activity_icon(ui.c, area.x + 12., area.y + 17., ui.t.accent);
            ui.c.text_clip(
                editor_style::font(ui.t.style.vintage_2005, 11),
                area.x + 23.,
                area.y + 10.,
                area.w - 29.,
                ui.t.text,
                "Loading audio for transient detection...",
            );
        } else if let Some(f) = form {
            inline_editor(ui, s, g, area, f);
        }
    });
    ui.c.pop_clip();
}

fn inline_editor(ui: &mut Ui, s: &State, g: &pl::PGeom, area: Rect, f: &Form) {
    let vintage = ui.t.style.vintage_2005;
    let applying = f.pending.is_some();
    let mut x = area.x + 5.;
    let mut top = area.y + 4.;
    let tools = [
        (Command::Analyze, Icon::Wave, "Detect transients", false),
        (
            Command::Next,
            Icon::ArrowRight,
            "Edit next audio clip on this track",
            false,
        ),
        (
            Command::Select,
            Icon::Select,
            "Select transients and timing markers",
            f.select_mode,
        ),
        (
            Command::Quantize,
            Icon::Tick,
            "Quantize to the timing grid",
            false,
        ),
        (
            Command::Snap,
            Icon::Magnet,
            "Snap markers to the timing grid",
            f.snap,
        ),
        (
            Command::Delete,
            Icon::Delete,
            "Remove selected timing markers",
            false,
        ),
        (Command::Reset, Icon::Repeat, "Reset timing markers", false),
        (Command::Undo, Icon::Undo, "Undo timing edit", false),
        (Command::Redo, Icon::Redo, "Redo timing edit", false),
    ];
    for (cmd, icon, hint, on) in tools {
        let r = editor_style::toolbar_slot(area, &mut x, &mut top, 28.);
        let hot = !applying && ui.hit(r, Action::Warp(cmd), hint);
        editor_style::control_face(ui, r, on, hot);
        let ink = if applying {
            ui.t.text_faint
        } else if on {
            ui.t.glyph
        } else {
            ui.t.text
        };
        icons::draw(ui.c, icon, r.inset(5.), ink);
        if on {
            editor_style::control_mark(ui, r);
        }
    }

    let apply =
        editor_style::toolbar_slot(area, &mut x, &mut top, if applying { 88. } else { 51. });
    let apply_hot = !applying && ui.hit(apply, Action::Warp(Command::Apply), "Apply timing edits");
    let face = if apply_hot {
        ui.t.accent.mix(Color::rgb(0xffffff), 0.18)
    } else {
        ui.t.accent
    };
    if vintage {
        crate::vintage::metal(ui.c, apply, face);
    } else {
        ui.c.fill_rrect(apply, 4., face);
    }
    let ink = Color::rgb(0x000000);
    let (label_rect, label) = if applying {
        crate::loading::activity_icon(ui.c, apply.x + 11., apply.cy(), ink);
        (
            Rect::new(apply.x + 16., apply.y, apply.w - 16., apply.h),
            "Applying...",
        )
    } else {
        (apply, "Apply")
    };
    ui.c.text_center(editor_style::font(vintage, 11), label_rect, ink, label);
    let close = editor_style::toolbar_slot(area, &mut x, &mut top, 28.);
    let hot = !applying && ui.hit(close, Action::Warp(Command::Close), "Close timing editor");
    editor_style::control_face(ui, close, false, hot);
    icons::draw(
        ui.c,
        Icon::Close,
        close.inset(5.),
        if applying { ui.t.text_faint } else { ui.t.text },
    );

    let card_width = ((area.w - 10.) / 3.).min(180.);
    let strength = format!("{}%", f.strength * 25);
    let selected = f.selection.len().to_string();
    let inspector_y = top + 34.;
    for (i, (label, value, control)) in [
        (
            "GRID",
            GRID_NAMES[f.grid],
            Some((Command::Grid, "Click to cycle timing grid divisions")),
        ),
        (
            "STRENGTH",
            strength.as_str(),
            Some((Command::Strength, "Click to cycle quantize strength")),
        ),
        ("SELECTED", selected.as_str(), None),
    ]
    .into_iter()
    .enumerate()
    {
        let card = Rect::new(
            area.x + 5. + i as f32 * card_width,
            inspector_y,
            card_width - 4.,
            30.,
        );
        editor_style::inspector_face(ui, card);
        if let Some((cmd, hint)) = control
            && !applying
            && ui.hit(card, Action::Warp(cmd), hint)
        {
            ui.c.stroke_rrect(
                card.inset(1.),
                if vintage { 0. } else { 3. },
                1.,
                ui.t.text_dim.with_alpha(85),
            );
        }
        ui.c.text_clip(
            editor_style::font(vintage, 9),
            card.x + 6.,
            card.y + 2.,
            card.w - 10.,
            ui.t.text_dim,
            label,
        );
        ui.c.text_clip(
            editor_style::font(vintage, 12),
            card.x + 6.,
            card.y + 13.,
            card.w - 10.,
            ui.t.text,
            value,
        );
    }

    let surface_y = inspector_y + 43.;
    let surface = Rect::new(
        area.x + 1.,
        surface_y,
        (area.w - 16.).max(1.),
        (area.bottom() - surface_y - 44.).max(1.),
    );
    let wave = Rect::new(
        g.x_of(f.original.position as f32),
        surface.y,
        (f.end() as f32 * s.project.ppq as f32 * g.ppt).max(1.),
        surface.h,
    );
    f.wave.set(wave);
    let visible = wave.intersect(&surface).intersect(&g.grid);
    ui.c.fill_rect(surface, ui.t.s(0x202b31));
    if ui.c.visible(surface) {
        ui.c.push_clip(surface);
        editor_style::time_grid(ui, s, g, surface);
        ui.c.pop_clip();
    }
    ui.hit(
        visible,
        Action::Warp(Command::Source),
        if f.select_mode {"Drag to select points; Shift-drag adds. Ctrl+wheel: time zoom; Alt+wheel: waveform zoom"} else {"Click to add a marker; drag its line. Ctrl+wheel: time zoom; Alt+wheel: waveform zoom"},
    );
    ui.c.push_clip(visible);
    ui.c.line(
        visible.x,
        wave.cy(),
        visible.right(),
        wave.cy(),
        0.6,
        ui.t.s(0x50616a),
    );
    let signal = ui.t.s(0x91b4bd);
    let rate = f.sample.rate as f64;
    let columns = if ui.c.visible(visible) { visible.w.ceil() as usize } else { 0 };
    for col in 0..columns {
        let x = visible.x + col as f32;
        let dst = (x - wave.x) as f64 / wave.w as f64 * f.end();
        let src = interpolate(&f.edit.points, dst, true);
        let next = interpolate(&f.edit.points, dst + f.end() / wave.w as f64, true);
        let lo = (src * rate).max(0.) as usize;
        let hi = ((next * rate) as usize)
            .max(lo.saturating_add(1))
            .min(f.sample.frames());
        let peak = f
            .sample
            .left
            .get(lo..hi)
            .unwrap_or(&[])
            .iter()
            .chain(f.sample.right.get(lo..hi).unwrap_or(&[]).iter())
            .fold(0f32, |p, v| p.max(v.abs()))
            .min(1.);
        ui.c.line(
            x,
            wave.cy() - (peak * f.amplitude).min(1.) * wave.h * 0.42,
            x,
            wave.cy() + (peak * f.amplitude).min(1.) * wave.h * 0.42,
            1.,
            signal,
        );
    }
    for &src in &f.transients {
        let x = wave.x + (interpolate(&f.edit.points, src, false) / f.end()) as f32 * wave.w;
        ui.c.line(
            x,
            wave.y,
            x,
            wave.bottom(),
            1.,
            if f.selection.contains(&src) {
                Color::rgb(0x70dfff)
            } else {
                Color::rgb(0x79949b).with_alpha(120)
            },
        );
    }
    for (i, &(src, beat)) in f.edit.points.iter().enumerate() {
        let x = wave.x + (beat / f.end()) as f32 * wave.w;
        let color = if f.selection.contains(&src) {
            Color::rgb(0x70dfff)
        } else if f.selected == Some(i) {
            Color::rgb(0x7ac4ff)
        } else {
            Color::rgb(0xdfac50)
        };
        ui.c.line(x, wave.y, x, wave.bottom(), 2., color);
        ui.c.fill_polygon(
            &[(x - 5., wave.y), (x + 5., wave.y), (x, wave.y + 10.)],
            color,
        );
        if f.pending.is_none() && i > 0 && i + 1 < f.edit.points.len() {
            // Split close hit targets halfway between anchors, so the nearer line wins.
            let previous_x = wave.x + (f.edit.points[i - 1].1 / f.end()) as f32 * wave.w;
            let next_x = wave.x + (f.edit.points[i + 1].1 / f.end()) as f32 * wave.w;
            let left = (x - 5.).max((previous_x + x) * 0.5);
            let right = (x + 5.).min((x + next_x) * 0.5);
            // Keep drag-to-select available in the waveform while the Select tool is active.
            let height = if f.select_mode { 15. } else { wave.h };
            let target = Rect::new(left, wave.y, right - left, height).intersect(&visible);
            if !target.is_empty() {
                ui.hit(
                    target,
                    Action::Warp(Command::Marker(i)),
                    if f.select_mode {
                        "Select timing marker"
                    } else {
                        "Drag timing marker"
                    },
                );
            }
        }
    }
    ui.c.pop_clip();
    let preview_status = if f.pending.is_none() && preview::enabled(s) {
        f.preview.status(f)
    } else {
        None
    };
    let working = f.pending.is_some()
        || f.analysis.is_some()
        || (preview_status.is_some() && f.preview.busy());
    let fit = Rect::new(surface.right() - 46., surface.bottom() + 3., 41., 19.);
    let hot = !applying
        && ui.hit(
            fit,
            Action::Warp(Command::Fit),
            "Fit waveform to the timing editor",
        );
    editor_style::control_face(ui, fit, false, hot);
    ui.c.text_center(
        editor_style::font(vintage, 11),
        fit,
        if applying { ui.t.text_faint } else { ui.t.text },
        "Fit",
    );
    if working {
        crate::loading::activity_icon(ui.c, area.x + 12., area.bottom() - 9., ui.t.accent);
    }
    let inset = if working { 23. } else { 6. };
    ui.c.text_clip(
        editor_style::font(vintage, 10),
        area.x + inset,
        area.bottom() - 14.,
        area.w - inset - 6.,
        ui.t.text_dim,
        preview_status.unwrap_or(&f.message),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wait_loaded(app: &mut App) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while app.state.playlist.warp_loading.is_some() && std::time::Instant::now() < deadline {
            poll(app);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(app.state.playlist.warp.is_some(), "{:?}", app.state.message);
    }

    #[test]
    fn inline_warp_transients_quantize_cancel_and_visibility() {
        let dir = std::env::temp_dir().join(format!("lemon-inline-warp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("attacks.wav");
        let rate = 16000u32;
        let mut audio = vec![0f32; rate as usize * 4 * 2];
        let attacks = [0.13, 0.61, 1.12, 1.64, 2.13, 2.62, 3.14, 3.62];
        for time in attacks {
            let start = (time * rate as f64) as usize;
            for i in 0..1600 {
                let value = 0.7 * (-(i as f32) / 350.).exp() * (i as f32 * 0.43).cos();
                audio[(start + i) * 2] = value;
                audio[(start + i) * 2 + 1] = -value;
            }
        }
        os_engine::wav::write_f32_stereo(
            &mut std::fs::File::create(&source).unwrap(),
            rate,
            &audio,
        )
        .unwrap();
        let mut app = App::new(None);
        app.state.project = crate::app::test_project();
        app.state.tempo = 120.;
        let ppq = app.state.project.ppq;
        let ch = &mut app.state.project.channels[0];
        ch.sample_path = Some(source.to_string_lossy().into_owned());
        let id = ch.index;
        pl::arr_mut(&mut app.state).clips = vec![os_model::Clip::new(
            ppq as u32 / 8,
            ppq as u32 * 8,
            0,
            ClipSource::Channel(id),
        )];
        let before = app.state.project.clone();
        command(&mut app, Command::Toggle(0));
        wait_loaded(&mut app);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.state.playlist.warp.as_ref().unwrap().analysis.is_some()
            && std::time::Instant::now() < deadline
        {
            poll(&mut app);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(
            app.state.playlist.warp.as_ref().unwrap().transients.len(),
            attacks.len()
        );
        command(&mut app, Command::Quantize);
        let points = app
            .state
            .playlist
            .warp
            .as_ref()
            .unwrap()
            .edit
            .points
            .clone();
        assert!(points.len() > 4);
        for &(_, beat) in &points[1..points.len() - 1] {
            assert!(((beat + 0.125) * 4. - ((beat + 0.125) * 4.).round()).abs() < 0.00001);
        }
        command(&mut app, Command::Undo);
        assert_eq!(
            app.state.playlist.warp.as_ref().unwrap().edit.points.len(),
            2
        );
        command(&mut app, Command::Redo);
        assert_eq!(
            app.state.playlist.warp.as_ref().unwrap().edit.points,
            points
        );
        // Selection-only quantize must leave every unselected anchor unchanged.
        command(&mut app, Command::Select);
        {
            let f = app.state.playlist.warp.as_mut().unwrap();
            f.edit.points[2].1 += 0.04;
            f.selection = vec![f.edit.points[2].0];
        }
        let selected_before = app
            .state
            .playlist
            .warp
            .as_ref()
            .unwrap()
            .edit
            .points
            .clone();
        command(&mut app, Command::Quantize);
        let selected_after = &app.state.playlist.warp.as_ref().unwrap().edit.points;
        assert_ne!(selected_before[2], selected_after[2]);
        for i in 0..selected_before.len() {
            if i != 2 {
                assert_eq!(selected_before[i], selected_after[i]);
            }
        }
        command(&mut app, Command::Undo);
        assert_eq!(
            app.state.playlist.warp.as_ref().unwrap().edit.points,
            selected_before
        );
        {
            let f = app.state.playlist.warp.as_mut().unwrap();
            f.wave.set(Rect::new(0., 0., 800., 100.));
            f.selection_start = 0.;
            f.selection_base.clear();
            f.select_to(400.);
            assert!(f.selection.len() > 2);
            f.edit.points = points.clone();
        }
        command(&mut app, Command::Select);
        for vintage in [false, true] {
            app.state.vintage_2005 = vintage;
            app.state.z = vec![crate::app::Win::Playlist];
            let mut canvas = os_ui::Canvas::new(1280, 800, 1.).unwrap();
            app.paint(&mut canvas, None);
            assert!(
                app.hits
                    .iter()
                    .any(|h| h.action == Action::Warp(Command::Toggle(0)))
            );
            assert!(
                app.hits
                    .iter()
                    .any(|h| h.action == Action::Warp(Command::Quantize))
            );
            let f = app.state.playlist.warp.as_ref().unwrap();
            let wave = f.wave.get();
            let index = 3;
            let marker_x = wave.x + (f.edit.points[index].1 / f.end()) as f32 * wave.w;
            let press_x = marker_x + 2.;
            let press_y = wave.cy();
            let hand = crate::app::Cursor::Playlist(pl::PlTool::Draw, crate::playlist_tools::GAIN);
            // A repaint of the Playlist above the editor leaves the editor's pixels on screen.
            // Its controls must still take the next click instead of the underlying Draw tool.
            let grid = app.pl_grid().unwrap();
            let damage = Rect::new(grid.x + 1., grid.y, 30., 3.);
            assert!(damage.bottom() < wave.y);
            app.paint(&mut canvas, Some(damage));
            let clip_count = pl::clips(&app.state).len();
            app.mouse_move(press_x, press_y);
            assert_eq!(
                app.top_hit(press_x, press_y).map(|hit| &hit.action),
                Some(&Action::Warp(Command::Marker(index))),
                "vintage={vintage} wave={wave:?}"
            );
            assert_eq!(
                app.cursor(),
                hand,
                "vintage={vintage} hover={:?}",
                app.hover
            );
            app.mouse_down();
            app.mouse_up();
            assert_eq!(
                app.state.playlist.warp.as_ref().unwrap().edit.points,
                points
            );
            app.mouse_down();
            app.mouse_move(press_x + 32., press_y);
            app.mouse_up();
            assert_eq!(pl::clips(&app.state).len(), clip_count);
            assert_ne!(
                app.state.playlist.warp.as_ref().unwrap().edit.points[index],
                points[index]
            );
            command(&mut app, Command::Undo);
            assert_eq!(
                app.state.playlist.warp.as_ref().unwrap().edit.points,
                points
            );
            let close = app.hits.iter().find(|h| h.action == Action::Warp(Command::Close)).unwrap().rect;
            let _ = app.take_damage();
            let before_scroll = app.state.playlist_scroll;
            app.state.playlist_scroll.0 += 12.;
            app.playlist_scrolled(before_scroll);
            let damage = app.take_damage().expect("horizontal scroll needs a repaint");
            app.paint_region(&mut canvas, damage.as_deref());
            assert!(app.take_moved().is_empty(), "the editor's toolbar stays anchored while scrolling");
            assert_eq!(app.top_hit(close.cx(), close.cy()).map(|hit| &hit.action), Some(&Action::Warp(Command::Close)));
            let wave_hit = app
                .hits
                .iter()
                .find(|h| h.action == Action::Warp(Command::Source))
                .unwrap()
                .rect;
            app.mouse = (wave_hit.x + wave_hit.w * 0.5, wave_hit.cy());
            app.alt = true;
            assert!(wheel(&mut app, 0., 1., false));
            assert!(app.state.playlist.warp.as_ref().unwrap().amplitude > 1.);
            app.alt = false;
            let old_zoom = app.state.playlist_bar_w;
            assert!(wheel(&mut app, 0., 1., true));
            assert!(app.state.playlist_bar_w > old_zoom);
            if let Some(dir) = std::env::var_os("OS_SHOT_DIR") {
                canvas
                    .pixmap
                    .save_png(PathBuf::from(dir).join(format!("inline-warp-{vintage}.png")))
                    .unwrap();
            }
        }
        crate::settings_file::write("visible_playlist-warp", "0");
        assert!(!expanded(&app.state, 0));
        assert_eq!(
            app.state.playlist.warp.as_ref().unwrap().edit.points,
            points
        );
        crate::settings_file::write("visible_playlist-warp", "1");
        assert!(expanded(&app.state, 0));
        command(&mut app, Command::Close);
        assert_eq!(app.state.project, before);
        assert!(app.state.playlist.warp.is_none());
    }
    #[test]
    fn community_warp_roundtrip_undo_and_render() {
        let dir = std::env::temp_dir().join(format!("lemon-warp-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("source.wav");
        let rate = 8000u32;
        let audio: Vec<f32> = (0..16000)
            .flat_map(|i| {
                let x = (i as f32 * 440. * std::f32::consts::TAU / rate as f32).sin() * 0.2;
                [x, x]
            })
            .collect();
        os_engine::wav::write_f32_stereo(
            &mut std::fs::File::create(&source).unwrap(),
            rate,
            &audio,
        )
        .unwrap();
        let original_bytes = std::fs::read(&source).unwrap();
        let mut app = App::new(None);
        app.state.project = crate::app::test_project();
        app.state
            .project
            .arrangements
            .push(os_flp::project::default_arrangement());
        app.state.tempo = 120.;
        app.state.project.data_folder = Some(os_model::DataFolder {
            choice: 2,
            folder: dir.to_string_lossy().into_owned(),
        });
        let ch = &mut app.state.project.channels[0];
        ch.sample_path = Some(source.to_string_lossy().into_owned());
        ch.index = 42;
        let id = ch.index;
        let ppq = app.state.project.ppq;
        pl::arr_mut(&mut app.state).clips.push(os_model::Clip::new(
            0,
            ppq as u32 * 4,
            0,
            ClipSource::Channel(id),
        ));
        let original_vocal = dir.join("original-vocal.wav");
        std::fs::copy(&source, &original_vocal).unwrap();
        app.state.project.vocal_edits.push(os_model::vocal::Edit {
            channel: id,
            source: original_vocal.to_string_lossy().into_owned(),
            source_hash: String::new(),
            rendered: source.to_string_lossy().into_owned(),
            render_key: String::new(),
            rate: 16000,
            frames: 32000,
            f0: vec![440.; 401],
            notes: vec![],
            curve: vec![(1, 0.5)],
            timing: vec![(0., 0.), (2., 2.)],
            enabled: true,
            strength: 1.,
            formant: 0.,
        });
        let before = app.state.project.clone();
        app.state.z = vec![crate::app::Win::Playlist];
        app.state.song_mode = true;
        crate::settings_file::set_flag(PREVIEW_VISIBILITY, true);
        open(&mut app, 0).unwrap();
        wait_loaded(&mut app);
        command(&mut app, Command::Apply);
        assert!(
            app.state
                .playlist
                .warp
                .as_ref()
                .unwrap()
                .message
                .contains("No timing changes")
        );
        assert!(app.state.playlist.warp.as_ref().unwrap().pending.is_none());
        let Some(f) = app.state.playlist.warp.as_mut() else {
            panic!()
        };
        f.edit.points.insert(1, (1., 1.5));
        f.wave.set(Rect::new(0., 0., 400., 100.));
        f.moved(1, 200.);
        assert_eq!(f.edit.points[1].1, 2.);
        f.edit.points[1].1 = 1.5;
        if let Some(dir) = std::env::var_os("OS_SHOT_DIR") {
            let mut c = os_ui::Canvas::new(1280, 900, 1.).unwrap();
            app.paint(&mut c, None);
            c.pixmap
                .save_png(std::path::PathBuf::from(dir).join("warp-editor.png"))
                .unwrap();
        }
        // Several attacks snap to the same grid line. The result must still render.
        let f = app.state.playlist.warp.as_mut().unwrap();
        f.analysis = None;
        f.transients = vec![0.49, 0.5, 0.51, 0.510001];
        command(&mut app, Command::Quantize);

        // A normal global Play waits for the latest warp, including when starting inside the clip.
        app.state.playlist.pos = u32::from(ppq);
        app.act(Action::Play);
        assert!(!app.transport.playing());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !app.transport.playing() && std::time::Instant::now() < deadline {
            poll(&mut app);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(app.transport.playing() && app.transport.song());
        app.act(Action::Stop);
        assert_eq!(
            app.state.project, before,
            "audition must preserve the project"
        );
        assert!(
            !dir.join("Warped").exists(),
            "preview creates no render file"
        );
        let mut p = crate::rack::play_project(&app.state);
        let preview = playback(&app.state, &mut p).unwrap();
        let clip = &p.arrangement().unwrap().clips[0];
        let ClipSource::Channel(preview_id) = clip.source else {
            panic!()
        };
        assert_ne!(preview_id, id);
        assert_eq!(p.channel(id), before.channel(id));
        assert_eq!(
            p.channel(preview_id).unwrap().sample_path.as_deref(),
            Some(PREVIEW_PATH)
        );
        let mut library = os_engine::SampleLibrary::new(None);
        library.replace_buffer(PREVIEW_PATH, Some(preview.clone()));
        let mut engine = os_engine::Engine::new(&p, &mut library, rate, os_engine::PlayMode::Song);
        engine.seek_tick(u64::from(ppq));
        let mut audible = [0.; 1024];
        engine.process(&mut audible);
        assert!(
            audible.iter().any(|v| v.abs() > 0.005),
            "seek into the warped buffer must sound"
        );

        crate::settings_file::set_flag(PREVIEW_VISIBILITY, false);
        let mut original = crate::rack::play_project(&app.state);
        assert!(playback(&app.state, &mut original).is_none());
        assert_eq!(
            original.arrangement().unwrap().clips[0].source,
            ClipSource::Channel(id)
        );
        crate::settings_file::set_flag(PREVIEW_VISIBILITY, true);

        use crate::dialog::DHit;
        let warning = &crate::messages::WARP_BAKES_VOCAL_TUNING;
        command(&mut app, Command::Apply);
        assert!(
            app.top_message()
                .unwrap()
                .1
                .contains("bake the clip's vocal tuning")
        );
        assert!(app.state.playlist.warp.as_ref().unwrap().pending.is_none());
        app.dialog_click(DHit::Remember);
        app.dialog_click(DHit::Button(1));
        assert!(crate::messages::on(warning));
        assert!(app.state.playlist.warp.as_ref().unwrap().pending.is_none());

        command(&mut app, Command::Apply);
        assert!(app.modal_open());
        app.dialog_click(DHit::Remember);
        app.dialog_click(DHit::Button(0));
        assert!(!crate::messages::on(warning));
        assert!(app.state.playlist.warp.as_ref().unwrap().pending.is_some());
        assert!(app.animating());
        command(&mut app, Command::Close);
        assert!(app.state.playlist.warp.as_ref().unwrap().pending.is_some());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while app.state.playlist.warp.is_some() && std::time::Instant::now() < deadline {
            poll(&mut app);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(app.state.playlist.warp.is_none());
        assert_eq!(std::fs::read(&source).unwrap(), original_bytes);
        let after = app.state.project.clone();
        let warped = &after.audio_warps[0];
        let ClipSource::Channel(rendered_id) = pl::clips(&app.state)[0].source else {
            panic!()
        };
        assert_eq!(rendered_id, 43);
        assert_eq!(
            after.channel(rendered_id).unwrap().sample_path.as_deref(),
            Some(warped.rendered.as_str())
        );
        let rendered =
            os_engine::sample::load_shared(std::path::Path::new(&warped.rendered)).unwrap();
        assert_eq!(rendered.frames(), 16000);
        assert_eq!(preview.left, rendered.left);
        assert_eq!(preview.right, rendered.right);
        let file = os_flp::project::create(&after).unwrap();
        let loaded = os_flp::project::load(&file).unwrap();
        assert_eq!(loaded.audio_warps, after.audio_warps);
        crate::history::undo(&mut app.state);
        assert_eq!(app.state.project, before);
        crate::history::redo(&mut app.state);
        assert_eq!(app.state.project, after);
        open(&mut app, 0).unwrap();
        wait_loaded(&mut app);
        let Some(f) = app.state.playlist.warp.as_ref() else {
            panic!()
        };
        assert_eq!(f.edit.points, warped.points);
    }
}

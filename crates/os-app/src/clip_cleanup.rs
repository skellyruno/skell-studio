// SPDX-License-Identifier: GPL-3.0-or-later
//! Playlist -> Tesla -> same clip. Return links are session-local and one-use.
use crate::{
    app::{App, State},
    playlist as pl,
};
use os_model::{Channel, Clip, ClipSource};
use std::{
    path::PathBuf,
    sync::{atomic::Ordering, mpsc, Arc},
};
struct Target {
    index: usize,
    clips: Vec<Clip>,
    channel: Channel,
    arrangement: usize,
    path: String,
    stamp: (u64, Option<std::time::SystemTime>),
}
impl Target {
    fn current(&self, s: &State) -> bool {
        s.project.arrangement_index() == self.arrangement
            && pl::clips(s) == self.clips.as_slice()
            && s.project
                .channels
                .iter()
                .find(|c| c.index == self.channel.index)
                == Some(&self.channel)
            && std::fs::metadata(&self.path)
                .ok()
                .map(|m| (m.len(), m.modified().ok()))
                == Some(self.stamp)
    }
}
pub struct Session {
    target: Target,
    link: Arc<tesla::cleanup_link::Session>,
    id: u64,
    loading: Option<mpsc::Receiver<Result<tesla::sample::Sample, String>>>,
    writing: Option<mpsc::Receiver<Result<PathBuf, String>>>,
    shape: Option<(u32, usize)>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.link.active.store(false, Ordering::Release);
        if self.link.pending.load(Ordering::Acquire) {
            self.link
                .finish("The project or return session closed. Original clip preserved.");
        }
    }
}
pub fn request(app: &mut App, index: usize) {
    let result = (|| -> Result<Session, String> {
        if !crate::workspace_visibility::shown(tesla::cleanup::VISIBILITY) {
            return Err("Enable Tesla audio cleanup in Workspace Visibility.".into());
        }
        let clip = pl::clips(&app.state)
            .get(index)
            .ok_or("Choose an audio clip first")?.clone();
        let ClipSource::Channel(id) = clip.source else {
            return Err("Choose an audio clip first".into());
        };
        let channel = app
            .state
            .project
            .channels
            .iter()
            .find(|c| c.index == id && c.kind == os_model::ChannelKind::AudioClip)
            .ok_or("Choose an audio clip first")?
            .clone();
        if crate::ara::is(&channel) {
            return Err("Render the ARA edits to an audio clip before opening Cleanup.".into());
        }
        if crate::mixer::locked(&app.state, channel.mixer_track) {
            return Err("Unlock this clip's Mixer channel before cleaning it.".into());
        }
        if app.clip_cleanups.len() >= 8 {
            return Err(
                "Apply a cleanup or remove an unused Tesla instance before opening another.".into(),
            );
        }
        let path = crate::plugins::resolve_sample(
            channel
                .sample_path
                .as_deref()
                .ok_or("This clip has no audio file")?,
        );
        let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
        if metadata.len() > 1_000_000_000 {
            return Err("This audio file is too large for Cleanup.".into());
        }
        let target = Target {
            index,
            clips: pl::clips(&app.state).to_vec(),
            channel,
            arrangement: app.state.project.arrangement_index(),
            path: path.clone(),
            stamp: (metadata.len(), metadata.modified().ok()),
        };
        let (id, link) = tesla::cleanup_link::create();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(|| tesla::editor::load_file(&path))
                .unwrap_or_else(|_| Err("Could not load audio for Cleanup.".into()));
            let _ = tx.send(result);
        });
        Ok(Session {
            target,
            link,
            id,
            loading: Some(rx),
            writing: None,
            shape: None,
        })
    })();
    match result {
        Ok(session) => {
            app.clip_cleanups.push(session);
            app.state.message = Some("Opening clip in Tesla Cleanup…".into());
        }
        Err(e) => app.state.message = Some(e),
    }
    app.invalidate();
}
fn apply(s: &mut State, target: &Target, path: &std::path::Path) -> Result<(), String> {
    if !target.current(s) {
        return Err(
            "The Playlist or source changed. Reopen Cleanup from the clip; nothing was replaced."
                .into(),
        );
    }
    if crate::mixer::locked(s, target.channel.mixer_track) {
        return Err("The Mixer channel is locked. Nothing was replaced.".into());
    }
    let id = s
        .project
        .channels
        .iter()
        .map(|c| c.index)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or("No channel IDs available")?;
    let row = s.project.channels.len();
    let before = crate::history::Snap::Tail(Box::new(crate::history::Tail::now(s, row)));
    pl::snapshot_with(s, "Clean up audio clip", before);
    let mut channel = target.channel.clone();
    channel.index = id;
    channel.sample_path = Some(path.to_string_lossy().into_owned());
    channel.name = Some(format!(
        "{} (cleaned)",
        channel.name.as_deref().unwrap_or("Audio")
    ));
    // Only the selected occurrence changes. Same frame count/rate preserves every trim, fade and stretch setting.
    // Current source may already contain rendered tuning/warp: retain that sound, without reattaching stale editors.
    s.project.channels.push(channel);
    pl::arr_mut(s).clips[target.index].source = ClipSource::Channel(id);
    s.selected_channel = row;
    s.playlist.sel = vec![target.index];
    s.piano.dirty = true;
    crate::rack::autosize(s);
    Ok(())
}
pub fn tick(app: &mut App) {
    let mut keep = Vec::new();
    for mut session in std::mem::take(&mut app.clip_cleanups) {
        if !session.link.active.load(Ordering::Acquire) {
            continue;
        }
        if let Some(rx) = session.loading.as_ref() {
            let result = match rx.try_recv() {
                Ok(r) => Some(r),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(_) => Some(Err("Audio loading stopped.".into())),
            };
            if let Some(result) = result {
                session.loading = None;
                match result {
                    Ok(sample) if session.target.current(&app.state) => {
                        session.shape = Some((sample.rate, sample.frames()));
                        if sample.is_empty()
                            || sample.frames() > sample.rate as usize * 60 * 20
                            || sample.frames() * sample.channels() > 120_000_000
                        {
                            session.link.active.store(false, Ordering::Release);
                            app.state.message=Some("Cleanup supports nonempty audio up to 20 minutes / 120 million samples.".into());
                            app.invalidate();
                            continue;
                        }
                        *session.link.source.lock().unwrap() =
                            Some((sample, session.target.path.clone()));
                        let saved = tesla::state::Saved::default();
                        if !app.add_tesla(
                            0,
                            Some(tesla::cleanup_link::plugin_state(&saved, session.id)),
                        ) {
                            session.link.active.store(false, Ordering::Release);
                        } else {
                            app.state.message=Some("Clean in Tesla, then Apply to track. Current rendered tuning/timing is baked into the source.".into());
                        }
                    }
                    Ok(_) => {
                        session.link.active.store(false, Ordering::Release);
                        app.state.message =
                            Some("The clip changed while loading. Reopen Cleanup.".into());
                    }
                    Err(e) => {
                        session.link.active.store(false, Ordering::Release);
                        app.state.message = Some(e);
                    }
                }
                app.invalidate();
            }
        }
        let request = session.link.request.lock().unwrap().take();
        if let Some(sample) = request {
            if !session.target.current(&app.state) {
                session
                    .link
                    .finish("The Playlist changed. Reopen Cleanup from the source clip.");
                session.link.active.store(false, Ordering::Release);
            } else if session.shape != Some((sample.rate, sample.frames())) {
                session.link.finish(
                    "Sample length or rate changed. Undo those edits before Apply to track.",
                );
            } else {
                let dir = crate::pattern_render::folder(&app.state).join("Cleaned");
                let (tx, rx) = mpsc::channel();
                session.writing = Some(rx);
                let id = session.id;
                std::thread::spawn(move || {
                    let result = (|| {
                        use std::io::Write;
                        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                        let stamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos();
                        let path =
                            dir.join(format!("Cleanup-{}-{id}-{stamp}.wav", std::process::id()));
                        let mut f = std::fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(&path)
                            .map_err(|e| e.to_string())?;
                        f.write_all(&tesla::wav::write(&sample, tesla::wav::Depth::Own))
                            .map_err(|e| e.to_string())?;
                        f.flush().map_err(|e| e.to_string())?;
                        Ok(path)
                    })();
                    let _ = tx.send(result);
                });
            }
        }
        if let Some(rx) = session.writing.as_ref() {
            let result = match rx.try_recv() {
                Ok(r) => Some(r),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(_) => Some(Err("Writing stopped. Original clip preserved.".into())),
            };
            if let Some(result) = result {
                session.writing = None;
                match result.and_then(|path| apply(&mut app.state, &session.target, &path)) {
                    Ok(()) => {
                        session.link.applied.store(true, Ordering::Release);
                        session.link.active.store(false, Ordering::Release);
                        session
                            .link
                            .finish("Applied to the original clip. Playlist Undo restores it.");
                    }
                    Err(e) => session.link.finish(&e),
                }
                app.state.message = Some(session.link.message());
                app.invalidate();
            }
        }
        if session.link.active.load(Ordering::Acquire) {
            keep.push(session);
        }
    }
    app.clip_cleanups = keep;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_clip_return_isolated_undo_stale_and_saved() {
        let dir = std::env::temp_dir().join(format!("lemon-cleanup-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let original = dir.join("Original.wav");
        let cleaned = dir.join("Cleaned.wav");
        let input = tesla::sample::Sample::from_channels(
            48000,
            vec![vec![0.2; 48000]],
            tesla::sample::Format::Float32,
        );
        let raw = tesla::wav::write(&input, tesla::wav::Depth::Own);
        std::fs::write(&original, &raw).unwrap();
        let mut output = input.clone();
        output.ch[0].fill(0.05);
        std::fs::write(&cleaned, tesla::wav::write(&output, tesla::wav::Depth::Own)).unwrap();
        let mut a = App::new(None);
        a.state.project = crate::app::default_project();
        let i = pl::add_audio_clip(&mut a.state, original.to_str().unwrap(), 2, 384).unwrap();
        let mut clip = pl::clips(&a.state)[i].clone();
        clip.window = Some((125., 750.));
        clip.length = 100;
        clip.set_gain(0.7);
        pl::arr_mut(&mut a.state).clips = vec![clip.clone(), clip.clone()];
        let ClipSource::Channel(id) = clip.source else {
            panic!()
        };
        let ch = a
            .state
            .project
            .channels
            .iter_mut()
            .find(|c| c.index == id)
            .unwrap();
        ch.mixer_track = 4;
        let ch = ch.clone();
        let meta = std::fs::metadata(&original).unwrap();
        let target = Target {
            index: 1,
            clips: pl::clips(&a.state).to_vec(),
            channel: ch.clone(),
            arrangement: a.state.project.arrangement_index(),
            path: original.to_str().unwrap().into(),
            stamp: (meta.len(), meta.modified().ok()),
        };
        let before = a.state.project.clone();
        apply(&mut a.state, &target, &cleaned).unwrap();
        let after = a.state.project.clone();
        assert_eq!(pl::clips(&a.state)[0], clip);
        let mut expected = clip;
        expected.source = pl::clips(&a.state)[1].source;
        assert_eq!(pl::clips(&a.state)[1], expected);
        assert_eq!(a.state.project.channels.last().unwrap().mixer_track, 4);
        assert_eq!(
            a.state.project.channels.iter().find(|c| c.index == id),
            Some(&ch)
        );
        assert_eq!(std::fs::read(&original).unwrap(), raw);
        crate::history::undo(&mut a.state);
        assert_eq!(a.state.project.channels, before.channels);
        assert_eq!(a.state.project.arrangements, before.arrangements);
        crate::history::redo(&mut a.state);
        assert_eq!(a.state.project.channels, after.channels);
        assert_eq!(a.state.project.arrangements, after.arrangements);
        assert!(apply(&mut a.state, &target, &cleaned).is_err());
        let bytes = os_flp::project::create(&a.state.project)
            .unwrap()
            .to_bytes()
            .unwrap();
        let reopened = os_flp::project::load(&os_flp::FlpFile::parse(&bytes).unwrap()).unwrap();
        assert_eq!(
            reopened.channels.last().unwrap().sample_path,
            a.state.project.channels.last().unwrap().sample_path
        );
        crate::history::undo(&mut a.state);
        pl::arr_mut(&mut a.state).clips.remove(0);
        assert!(
            apply(&mut a.state, &target, &cleaned).is_err(),
            "identical remaining clip must not be mistaken for the target"
        );
        for path in [original, cleaned] {
            std::fs::remove_file(path).unwrap();
        }
        let _ = std::fs::remove_dir(dir);
    }
    #[test]
    fn cleanup_loading_return_link_and_visibility() {
        let path =
            std::env::temp_dir().join(format!("lemon-cleanup-link-{}.wav", std::process::id()));
        let sample = tesla::sample::Sample::from_channels(
            48000,
            vec![vec![0.; 4800]],
            tesla::sample::Format::Float32,
        );
        std::fs::write(&path, tesla::wav::write(&sample, tesla::wav::Depth::Own)).unwrap();
        let mut a = App::new(None);
        a.state.project = crate::app::default_project();
        a.plugins.effects = crate::plugins::builtin_effects();
        let i = pl::add_audio_clip(&mut a.state, path.to_str().unwrap(), 0, 0).unwrap();
        crate::settings_file::set_flag(tesla::cleanup::VISIBILITY, false);
        let menu = pl::clip_menu(&a.state, i, 0., 0.);
        assert!(!menu
            .items
            .iter()
            .flat_map(|x| &x.children)
            .any(|x| x.cmd == pl::Cmd::CleanupAudio(i)));
        crate::settings_file::set_flag(tesla::cleanup::VISIBILITY, true);
        let menu = pl::clip_menu(&a.state, i, 0., 0.);
        assert!(menu
            .items
            .iter()
            .flat_map(|x| &x.children)
            .any(|x| x.cmd == pl::Cmd::CleanupAudio(i)));
        request(&mut a, i);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while a.clip_cleanups.iter().any(|s| s.loading.is_some())
            && std::time::Instant::now() < deadline
        {
            tick(&mut a);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(a.clip_cleanups.len(), 1);
        assert_eq!(a.clip_cleanups[0].shape, Some((48000, 4800)));
        assert!(a.clip_cleanups[0].link.source.lock().unwrap().is_some());
        let link = a.clip_cleanups[0].link.clone();
        a.reset_hosted();
        assert!(!link.active.load(Ordering::Acquire));
        assert!(!link.submit(Arc::new(sample)));
        assert!(a.clip_cleanups.is_empty());
        std::fs::remove_file(path).unwrap();
    }
}

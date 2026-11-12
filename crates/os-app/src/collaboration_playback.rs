//! Optional follow transport. Each computer renders locally; this is not an internet audio jam.
use super::*;
pub fn tick(app: &mut App) {
    let loading = app.loading();
    let rt = &mut app.collaboration;
    let Some(w) = &rt.worker else { return };
    if rt.hosting {
        if let Ok(mut slot) = w.playback.try_lock() {
            *slot = Some((
                os_collab::Playback {
                    playing: app.transport.playing() && !app.transport.paused(),
                    song: app.state.song_mode,
                    tick: if app.transport.playing() {
                        app.transport.tick()
                    } else {
                        f64::from(app.state.playlist.pos)
                    },
                    tempo: app.state.tempo,
                    ppq: app.state.project.ppq,
                    pattern: app.state.pattern,
                },
                Instant::now(),
            ));
        }
        return;
    }
    if !rt.follow || !rt.ready || rt.conflict.is_some() || loading {
        return;
    }
    if rt
        .follow_check
        .is_some_and(|t| t.elapsed() < Duration::from_millis(500))
    {
        return;
    }
    rt.follow_check = Some(Instant::now());
    let Some((mut p, at)) = w.received_playback.try_lock().ok().and_then(|s| s.clone()) else {
        return;
    };
    // A stale connection never starts/keeps correcting playback from an old clock.
    if at.elapsed() > Duration::from_secs(3) {
        return;
    }
    p.advance(at.elapsed().as_secs_f64());
    if !p.playing {
        if app.transport.playing() {
            app.transport.stop();
            app.invalidate();
        }
        return;
    }
    let was = app.transport.playing() && !app.transport.paused();
    let wrong_mode = app.state.song_mode != p.song;
    let drift = (app.transport.tick() - p.tick).abs();
    // Avoid repeated tiny seeks and audible jumps from packet jitter.
    if was && !wrong_mode && drift < f64::from(p.ppq) * 2. {
        return;
    }
    app.state.song_mode = p.song;
    if p.song {
        app.play_song_from(p.tick.min(u32::MAX as f64) as u32);
    } else {
        if !app
            .state
            .project
            .patterns
            .iter()
            .any(|pat| pat.number == p.pattern)
        {
            return;
        }
        app.state.pattern = p.pattern;
        app.state.project.current_pattern = Some(p.pattern);
        let project = crate::rack::play_project(&app.state);
        app.transport
            .play(&project, os_engine::PlayMode::Pattern(p.pattern), p.tick);
        app.invalidate();
    }
}

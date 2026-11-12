// SPDX-License-Identifier: GPL-3.0-or-later
//! ARA audio-source editors. One document per audio channel, shared by its Playlist occurrences.
use crate::{
    app::App,
    playlist::{self as pl, Cmd, PItem, Popup},
};
use os_model::ClipSource;
pub const MARKER: &str = "Lemon ARA";
pub fn is(ch: &os_model::Channel) -> bool {
    ch.plugin.as_deref() == Some(MARKER)
        || ch
            .plugin_state
            .as_deref()
            .is_some_and(os_vst3::ara::State::is)
}
fn row(app: &App, i: usize) -> Option<usize> {
    let ClipSource::Channel(id) = pl::clips(&app.state).get(i)?.source else {
        return None;
    };
    app.state
        .project
        .channels
        .iter()
        .position(|c| c.index == id && c.kind == os_model::ChannelKind::AudioClip)
}
pub(crate) fn choices(app: &App) -> Vec<os_host::Descriptor> {
    let mut out: Vec<_> = app
        .plugins
        .effects
        .iter()
        .chain(app.plugins.available.iter())
        .filter(|a| {
            a.desc.format == os_host::Format::Vst3 && a.desc.features.iter().any(|f| f == "ARA")
        })
        .map(|a| a.desc.clone())
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    out.dedup_by(|a, b| a.uid() == b.uid());
    out
}
pub fn menu(app: &mut App, i: usize) -> Option<String> {
    let r = row(app, i)?;
    let attached = is(&app.state.project.channels[r]);
    let mut items = vec![PItem::head("ARA audio editor")];
    if attached {
        items.push(PItem::new(
            "Open current editor",
            Cmd::AraChoose(i, usize::MAX),
        ));
        items.push(PItem::new(
            "Remove ARA edits (use original audio)",
            Cmd::AraRemove(i),
        ));
        items.push(PItem::sep());
    }
    let editors = if attached { Vec::new() } else { choices(app) };
    for (k, d) in editors.iter().enumerate() {
        items.push(PItem::new(d.name.clone(), Cmd::AraChoose(i, k)));
    }
    if editors.is_empty() && !attached {
        items.push(PItem::head("No ARA editors found in the plugin list"));
    }
    items.push(PItem::sep());
    items.push(PItem::new("Scan for ARA editors...", Cmd::AraScan));
    app.state.playlist.popup = Some(Popup {
        x: app.mouse.0,
        y: app.mouse.1,
        items,
        sub: None,
        scroll: [0; 2],
        cascade: Vec::new(),
    });
    None
}
pub fn choose(app: &mut App, i: usize, k: usize) -> Option<String> {
    let r = row(app, i)?;
    if is(&app.state.project.channels[r]) {
        if !app.state.hosted_rows.contains(&r) {
            if let Err(e) = restore(app, r) {
                return Some(format!("ARA: {e}"));
            }
        }
        app.state.pending_open = Some(r);
        return Some(
            "Opening the source's ARA editor; all copies of this source share its edits".into(),
        );
    }
    let d = choices(app).get(k)?.clone();
    let ch = &app.state.project.channels[r];
    let source = ch.sample_path.clone()?;
    // The document owns unprocessed source audio. Sampler time/pitch transformations are not silently discarded.
    if ch.sound.fx != os_model::SampleFx::default()
        || ch.sound.start_offset() != 0.
        || ch.envelopes.iter().any(|e| e.enabled)
    {
        return Some("Render this clip's sampler processing to audio first, then open the rendered clip with ARA".into());
    }
    if (ch.misc.main_pitch && app.state.project.main_pitch.unwrap_or(0) != 0)
        || ch.pitch_cents != 0
        || ch.misc.fine != 0
        || ch.misc.root != 60
        || ch.sound.stretch_time().is_some()
        || (ch.sound.stretch_multiplier() - 1.).abs() > 1e-6
        || ch.sound.stretch_cents() != 0.
    {
        return Some("Reset this clip's time stretch and pitch first, then edit its timing and pitch in the ARA editor".into());
    }
    let before = crate::history::Snap::ara(&app.state, r)?;
    match app.plugins.create_ara(r, &d.path, &d.id, &source, None) {
        Err(e) => Some(format!("ARA: {e}")),
        Ok(name) => {
            crate::history::record(&mut app.state, "Add ARA editor", before);
            let blob = app.plugins.hosted[&r].ara_saved.clone().unwrap();
            let ch = &mut app.state.project.channels[r];
            ch.plugin = Some(MARKER.into());
            ch.plugin_state = Some(blob);
            app.sampler_failed.retain(|&x| x != r);
            if !app.state.hosted_rows.contains(&r) {
                app.state.hosted_rows.push(r);
            }
            app.plugins.added(r);
            app.transport.set_instruments(app.plugins.instruments());
            app.state.piano.dirty = true;
            app.state.pending_open = Some(r);
            app.invalidate();
            Some(format!("{name} ARA: edits apply to every copy of this source; use Make Unique for independent edits"))
        }
    }
}
pub fn remove(app: &mut App, i: usize) -> Option<String> {
    let r = row(app, i)?;
    if !is(&app.state.project.channels[r]) {
        return None;
    }
    if let Err(e) = store(app) {
        return Some(e);
    }
    let before = crate::history::Snap::ara(&app.state, r)?;
    crate::history::record(&mut app.state, "Remove ARA editor", before);
    let ch = &mut app.state.project.channels[r];
    ch.plugin = None;
    ch.plugin_state = None;
    reconcile(app);
    app.invalidate();
    Some("ARA removed; original audio restored. Undo restores the edits.".into())
}
pub fn restore(app: &mut App, r: usize) -> Result<(), String> {
    let ch = &app.state.project.channels[r];
    let saved =
        os_vst3::ara::State::decode(ch.plugin_state.as_deref().ok_or("Missing ARA archive")?)?;
    let source = ch
        .sample_path
        .as_deref()
        .ok_or("ARA source audio is missing")?;
    app.plugins.create_ara(
        r,
        &saved.plugin_path,
        &saved.class_id,
        source,
        ch.plugin_state.as_deref(),
    )?;
    if !app.state.hosted_rows.contains(&r) {
        app.state.hosted_rows.push(r);
    }
    app.sampler_failed.retain(|&x| x != r);
    app.transport.set_instruments(app.plugins.instruments());
    Ok(())
}
/// Archive changes on the UI thread, outside the audio callback. Archive failure never clears existing edits.
pub fn edited(app: &mut App, r: usize, blob: Option<Vec<u8>>) {
    if !app.state.project.channels.get(r).is_some_and(is) {
        return;
    }
    if let Some(h) = app.plugins.hosted.get_mut(&r) {
        h.ara_dirty = true;
    }
    let Some(blob) = blob else {
        app.state.message =
            Some("ARA edits could not be archived. Keep the editor open and retry saving.".into());
        return;
    };
    if Some(&blob) == app.state.project.channels[r].plugin_state.as_ref() {
        if let Some(h) = app.plugins.hosted.get_mut(&r) {
            h.ara_dirty = false;
        }
        return;
    }
    if let Some(before) = crate::history::Snap::ara(&app.state, r) {
        crate::history::record(&mut app.state, "ARA edit", before);
    }
    app.state.project.channels[r].plugin_state = Some(blob.clone());
    if let Some(h) = app.plugins.hosted.get_mut(&r) {
        h.ara_saved = Some(blob);
        h.ara_dirty = false;
    }
}
pub fn store(app: &mut App) -> Result<(), String> {
    // A save immediately after Undo must not overwrite its archive with the old running document.
    reconcile(app);
    let rows: Vec<_> = app
        .plugins
        .hosted
        .iter()
        .filter(|(_, h)| h.ara_saved.is_some())
        .map(|(&r, _)| r)
        .collect();
    let mut blobs = Vec::new();
    for r in rows {
        blobs.push((
            r,
            app.plugins.state(r).ok_or(
                "An ARA editor could not save its edits. The project was not overwritten.",
            )?,
        ));
    }
    for (r, blob) in blobs {
        if let Some(ch) = app.state.project.channels.get_mut(r).filter(|c| is(c)) {
            ch.plugin_state = Some(blob.clone());
        }
        if let Some(h) = app.plugins.hosted.get_mut(&r) {
            h.ara_saved = Some(blob);
            h.ara_dirty = false;
        }
    }
    Ok(())
}
/// History restores model archives; replace only affected instances. No model mutation while processing.
pub fn reconcile(app: &mut App) {
    let rows: Vec<_> = app
        .plugins
        .hosted
        .iter()
        .filter_map(|(&r, h)| {
            let Some(ch) = app.state.project.channels.get(r) else {
                return h.ara_saved.as_ref().map(|_| r);
            };
            let needs = match (&h.ara_saved, is(ch)) {
                (Some(st), true) => {
                    Some(st) != ch.plugin_state.as_ref()
                        || ch
                            .sample_path
                            .as_deref()
                            .map(crate::plugins::resolve_sample)
                            != h.ara_source
                }
                (Some(_), false) | (None, true) => true,
                _ => false,
            };
            needs.then_some(r)
        })
        .collect();
    if rows.is_empty() {
        return;
    }
    for r in rows {
        app.plugins.drop_key(r);
        app.state.hosted_rows.retain(|&x| x != r);
        app.sampler_failed.retain(|&x| x != r);
    }
    app.transport.set_instruments(app.plugins.instruments());
    app.state.piano.dirty = true;
}

/// A source document may appear more than once: use the selected occurrence,
/// then the playing occurrence, then the first one. Never route requests to another channel.
pub fn playback_request(app:&mut App,row:usize,request:os_host::PlaybackRequest) {
    use os_host::PlaybackRequest as R;
    let Some(channel)=app.state.project.channels.get(row).filter(|c|is(c)) else {return};
    let matches=|c:&&os_model::Clip| c.source==ClipSource::Channel(channel.index)&&!c.muted;
    let all=pl::clips(&app.state);
    let at=if app.transport.playing()&&app.transport.song(){app.transport.tick().max(0.) as u32}else{app.state.playlist.pos};
    let selected=app.state.playlist.sel.iter().filter_map(|&i|all.get(i)).find(matches);
    let Some(clip)=selected.or_else(||all.iter().filter(matches).find(|c|c.position<=at&&at<c.end())).or_else(||all.iter().find(matches)).cloned() else {return};
    let tempo=app.state.project.tick_tempo(app.state.tempo);
    let tick=|seconds| source_tick(&clip,seconds,app.state.project.ppq,tempo);
    // Resolve before mutating app to avoid holding references across a restart.
    let seek=if let R::Seek(seconds)=request {tick(seconds)} else {None};
    let cycle=if let R::Cycle(Some((a,b)))=request {tick(a).zip(tick(b)).filter(|(a,b)|b>a)}else{None};
    match request {
        R::Start=>{
            app.state.song_mode=true;
            if !app.transport.playing()||!app.transport.song(){
                let pos=app.state.playlist.pos.clamp(clip.position,clip.end().saturating_sub(1));
                app.play_song_from(pos);
            }else if app.transport.paused(){app.transport.resume();}
        }
        R::Stop=>app.act(crate::app::Action::Stop),
        R::Seek(_)=>if let Some(t)=seek {app.state.song_mode=true;app.set_song_pos(t,true);},
        R::Cycle(range)=>{
            if range.is_none()||cycle.is_some(){
                app.state.song_mode=true;app.state.playlist.range=cycle;
                if app.transport.playing()&&!app.transport.paused(){app.play_song_from(cycle.map_or(at,|r|r.0));}
            }
        }
    }
    app.invalidate();
}
fn source_tick(clip:&os_model::Clip,seconds:f64,ppq:u16,tempo:f64)->Option<u32> {
    if !seconds.is_finite()||!tempo.is_finite()||tempo<=0. {return None;}
    let start=clip.window.map_or(0.,|w|w.0.max(0.)/1000.);
    let position=f64::from(clip.position)+(seconds-start)*tempo/60.*f64::from(ppq.max(1));
    Some(position.round().clamp(f64::from(clip.position),f64::from(clip.end())) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn playback_fix_ara_seek_maps_trimmed_source_to_occurrence() {
        let mut clip=os_model::Clip::new(960,384,2,ClipSource::Channel(0));
        clip.window=Some((2000.,4000.));
        assert_eq!(source_tick(&clip,2.,96,90.),Some(960));
        assert_eq!(source_tick(&clip,3.,96,90.),Some(1104));
        assert_eq!(source_tick(&clip,-10.,96,90.),Some(960));
        assert_eq!(source_tick(&clip,100.,96,90.),Some(1344));
        assert_eq!(source_tick(&clip,f64::NAN,96,90.),None);
    }
}

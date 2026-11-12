// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared musical state. Device settings, editor positions and local file bookkeeping never travel.
use os_collab::Document;
use os_model::{Arrangement, Pattern, Project};
use serde_json::{Value, json};
type Result<T> = std::result::Result<T, String>;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
const LOCAL: &[&str] = &[
    "saved_by",
    "current_pattern",
    "current_arrangement",
    "current_group",
    "song_mode",
    "playlist_view",
    "work_time",
    "data_folder",
    "show_info",
];

pub fn capture(p: &Project) -> Result<Document> {
    let mut d = Document::new();
    d.insert("schema".into(), json!({"version":2}));
    let Value::Object(mut root) = serde_json::to_value(p).map_err(err)? else {
        unreachable!()
    };
    for k in LOCAL {
        root.remove(*k);
    }
    root.remove("patterns");
    root.remove("arrangements");
    for (kind, field, id) in [
        ("channel", "channels", "index"),
        ("mixer", "mixer", "index"),
        ("automation", "automations", "channel"),
    ] {
        let values = root.remove(field).unwrap();
        let mut order = vec![];
        for item in values.as_array().ok_or("Invalid project collection")? {
            let n = item[id].as_u64().ok_or("Missing object identity")?;
            order.push(n);
            // Independent knobs on the same strip merge; structure remains one atomic operation.
            for (key, value) in item.as_object().unwrap() {
                d.insert(format!("{kind}/{n}/{key}"), value.clone());
            }
        }
        d.insert(format!("order/{kind}"), json!(order));
    }
    for (k, v) in root {
        d.insert(format!("project/{k}"), v);
    }
    for p in &p.patterns {
        let mut meta = p.clone();
        meta.notes.clear();
        meta.controls.clear();
        d.insert(
            format!("pattern/{}", p.number),
            serde_json::to_value(meta).map_err(err)?,
        );
        let ids: std::collections::BTreeSet<_> = p
            .notes
            .iter()
            .map(|n| n.channel)
            .chain(p.controls.iter().map(|n| n.channel()))
            .collect();
        for id in ids {
            d.insert(format!("notes/{}/{id}",p.number),json!({"notes":p.notes.iter().filter(|n|n.channel==id).collect::<Vec<_>>(),"controls":p.controls.iter().filter(|n|n.channel()==id).collect::<Vec<_>>()}));
        }
    }
    for a in &p.arrangements {
        let mut meta = a.clone();
        meta.clips.clear();
        meta.tracks.clear();
        d.insert(
            format!("arrangement/{}", a.number),
            serde_json::to_value(meta).map_err(err)?,
        );
        let ids: std::collections::BTreeSet<_> = a
            .clips
            .iter()
            .map(|c| u32::from(c.track))
            .chain(a.tracks.iter().filter_map(|t| t.number.checked_sub(1)))
            .collect();
        for id in ids {
            let track = a.tracks.iter().find(|t| t.number == id + 1).map(|t| {
                let mut t = t.clone();
                t.height = None;
                t.collapsed = false;
                t
            });
            d.insert(format!("lane/{}/{id}",a.number),json!({"track":track,"clips":a.clips.iter().filter(|c|u32::from(c.track)==id).collect::<Vec<_>>()}));
        }
    }
    Ok(d)
}

pub fn decode(d: &Document) -> Result<Project> {
    if d.get("schema") != Some(&json!({"version":2})) {
        return Err("This session needs the same collaboration version on both computers".into());
    }
    let mut root = serde_json::to_value(Project::default()).map_err(err)?;
    for (k, v) in d {
        if let Some(k) = k.strip_prefix("project/") {
            if LOCAL.contains(&k) || root.get(k).is_none() {
                return Err("Unknown project setting".into());
            }
            root[k] = v.clone();
        }
    }
    for (kind, field, id) in [
        ("channel", "channels", "index"),
        ("mixer", "mixer", "index"),
        ("automation", "automations", "channel"),
    ] {
        let order = d
            .get(&format!("order/{kind}"))
            .and_then(Value::as_array)
            .ok_or("Missing object order")?;
        if order.len() > 4096 {
            return Err("Too many project objects".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut out = vec![];
        for n in order {
            let n = n
                .as_u64()
                .filter(|n| *n <= u16::MAX as u64)
                .ok_or("Invalid identity")?;
            if !seen.insert(n) {
                return Err("Duplicate project identity".into());
            }
            let prefix = format!("{kind}/{n}/");
            let mut item = serde_json::Map::new();
            for (k, v) in d.range(prefix.clone()..) {
                let Some(k) = k.strip_prefix(&prefix) else {
                    break;
                };
                item.insert(k.into(), v.clone());
            }
            if item.get(id) != Some(&json!(n)) {
                return Err("Object identity mismatch".into());
            }
            out.push(Value::Object(item));
        }
        root[field] = Value::Array(out);
    }
    let mut patterns = vec![];
    let mut arrangements = vec![];
    for (k, v) in d {
        if let Some(id) = k.strip_prefix("pattern/") {
            let mut p: Pattern = serde_json::from_value(v.clone()).map_err(err)?;
            if id != p.number.to_string()
                || p.number == 0
                || !p.notes.is_empty()
                || !p.controls.is_empty()
            {
                return Err("Invalid pattern".into());
            }
            let prefix = format!("notes/{id}/");
            for (k, v) in d.range(prefix.clone()..) {
                let Some(id) = k.strip_prefix(&prefix) else {
                    break;
                };
                let id = id.parse::<u16>().map_err(err)?;
                let notes: Vec<os_model::Note> =
                    serde_json::from_value(v["notes"].clone()).map_err(err)?;
                let controls: Vec<os_model::ControlPoint> =
                    serde_json::from_value(v["controls"].clone()).map_err(err)?;
                if notes.iter().any(|n| n.channel != id)
                    || controls.iter().any(|n| n.channel() != id)
                {
                    return Err("Invalid note channel".into());
                }
                p.notes.extend(notes);
                p.controls.extend(controls);
            }
            p.notes.sort_by_key(|n| n.position);
            p.controls.sort_by_key(|c| c.position);
            patterns.push(p);
        } else if let Some(id) = k.strip_prefix("arrangement/") {
            let mut a: Arrangement = serde_json::from_value(v.clone()).map_err(err)?;
            if id != a.number.to_string() || !a.clips.is_empty() || !a.tracks.is_empty() {
                return Err("Invalid arrangement".into());
            }
            let prefix = format!("lane/{id}/");
            for (k, v) in d.range(prefix.clone()..) {
                let Some(id) = k.strip_prefix(&prefix) else {
                    break;
                };
                let id = id.parse::<u16>().map_err(err)?;
                let clips: Vec<os_model::Clip> =
                    serde_json::from_value(v["clips"].clone()).map_err(err)?;
                let track: Option<os_model::PlaylistTrack> =
                    serde_json::from_value(v["track"].clone()).map_err(err)?;
                if clips.iter().any(|c| c.track != id)
                    || track
                        .as_ref()
                        .is_some_and(|t| t.number != u32::from(id) + 1)
                {
                    return Err("Invalid Playlist lane".into());
                }
                a.clips.extend(clips);
                a.tracks.extend(track);
            }
            a.clips.sort_by_key(|c| c.position);
            a.tracks.sort_by_key(|t| t.number);
            arrangements.push(a);
        }
    }
    patterns.sort_by_key(|p| p.number);
    arrangements.sort_by_key(|a| a.number);
    root["patterns"] = serde_json::to_value(patterns).map_err(err)?;
    root["arrangements"] = serde_json::to_value(arrangements).map_err(err)?;
    let p: Project = serde_json::from_value(root).map_err(err)?;
    check(&p)?;
    // Reject orphan shards, unknown fields and noncanonical duplicate identities.
    for (k, v) in d.iter().filter(|(k, _)| k.starts_with("parameter/")) {
        if !parameter_valid(&p, k, v) {
            return Err("Invalid plugin parameter".into());
        }
    }
    if capture(&p)?
        != d.iter()
            .filter(|(k, _)| !k.starts_with("parameter/"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    {
        return Err("Inconsistent shared project; no changes applied".into());
    }
    Ok(p)
}
pub fn validate(d: &Document) -> Result<()> {
    decode(d).map(|_| ())
}
fn check(p: &Project) -> Result<()> {
    if p.ppq == 0
        || p.ppq > 9600
        || p.tempo
            .is_some_and(|v| !v.is_finite() || !(10. ..=522.).contains(&v))
        || p.bar
            .is_some_and(|(a, b)| a == 0 || b == 0 || a > 64 || b > 64)
    {
        return Err("Invalid musical timebase".into());
    }
    if p.patterns.len() > 4096
        || p.arrangements.len() > 256
        || p.patterns
            .iter()
            .map(|p| p.notes.len() + p.controls.len())
            .sum::<usize>()
            > 1000000
    {
        return Err("Project exceeds session limit".into());
    }
    let channels: std::collections::BTreeSet<_> = p.channels.iter().map(|c| c.index).collect();
    let mixer: std::collections::BTreeSet<_> = p.mixer.iter().map(|c| c.index).collect();
    if p.mixer.iter().any(|m| {
        m.index > 4095
            || m.routes.iter().any(|r| !mixer.contains(r) || *r == m.index)
            || m.send_levels
                .iter()
                .any(|(r, v)| !mixer.contains(r) || !v.is_finite() || !(0. ..=1.).contains(v))
            || m.volume
                .is_some_and(|v| !v.is_finite() || !(0. ..=1.).contains(&v))
            || m.pan
                .is_some_and(|v| !v.is_finite() || !(-1. ..=1.).contains(&v))
            || m.stereo_separation
                .is_some_and(|v| !v.is_finite() || !(-1. ..=1.).contains(&v))
            || m.slot_mix
                .iter()
                .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
    }) {
        return Err("Invalid Mixer settings".into());
    }
    if p.channels.iter().any(|c| {
        !c.volume.is_finite()
            || !(0. ..=1.).contains(&c.volume)
            || !c.pan.is_finite()
            || !(-1. ..=1.).contains(&c.pan)
            || c.layer_children.iter().any(|id| !channels.contains(id))
    }) {
        return Err("Invalid channel settings".into());
    }
    for pat in &p.patterns {
        if pat.notes.iter().any(|n| {
            n.key > 131
                || n.position.checked_add(n.length).is_none()
                || !channels.contains(&n.channel)
        }) || pat
            .controls
            .iter()
            .any(|c| !channels.contains(&c.channel()))
        {
            return Err("Invalid shared note".into());
        }
    }
    for a in &p.arrangements {
        if a.clips.len() > 50000
            || a.tracks.len() > 4096
            || a.clips.iter().any(|c| {
                c.position.checked_add(c.length).is_none()
                    || c.track > 4095
                    || !c.pattern_speed.is_finite()
                    || !(1. / 64. ..=64.).contains(&c.pattern_speed)
                    || c.window
                        .is_some_and(|(a, b)| !a.is_finite() || !b.is_finite() || a < 0. || b < a)
                    || match c.source {
                        os_model::ClipSource::Channel(id) => !channels.contains(&id),
                        os_model::ClipSource::Pattern(id) => {
                            !p.patterns.iter().any(|p| p.number == id)
                        }
                    }
            })
        {
            return Err("Invalid shared clip".into());
        }
    }
    for a in &p.automations {
        if !channels.contains(&a.channel)
            || a.points.len() > 100000
            || a.points.iter().any(|v| {
                !v.beat.is_finite()
                    || v.beat < 0.
                    || !v.value.is_finite()
                    || !(0. ..=1.).contains(&v.value)
                    || !v.tension.is_finite()
                    || !(-1. ..=1.).contains(&v.tension)
            })
            || a.points.windows(2).any(|w| w[0].beat > w[1].beat)
        {
            return Err("Invalid automation".into());
        }
    }
    Ok(())
}
pub fn put(s: &mut crate::app::State, d: &Document) -> Result<()> {
    let mut p = decode(d)?;
    let old = &s.project;
    p.saved_by = old.saved_by.clone();
    p.current_pattern = old
        .current_pattern
        .filter(|n| p.patterns.iter().any(|p| p.number == *n))
        .or_else(|| p.patterns.first().map(|p| p.number));
    p.current_arrangement = old
        .current_arrangement
        .filter(|n| p.arrangements.iter().any(|a| a.number == *n));
    p.current_group = old.current_group;
    p.song_mode = old.song_mode;
    p.playlist_view = old.playlist_view;
    p.work_time = old.work_time;
    p.data_folder = old.data_folder.clone();
    p.show_info = old.show_info;
    for m in &mut p.mixer {
        if let Some(o) = old.mixer.iter().find(|o| o.index == m.index) {
            m.runtime_id = o.runtime_id;
            m.origin = o.origin;
            m.tune_learn = o.tune_learn;
        } else {
            m.runtime_id = os_model::MixerTrack::new(m.index).runtime_id;
        }
    }
    for a in &mut p.arrangements {
        if let Some(o) = old.arrangements.iter().find(|o| o.number == a.number) {
            for t in &mut a.tracks {
                if let Some(o) = o.tracks.iter().find(|o| o.number == t.number) {
                    t.height = o.height;
                    t.collapsed = o.collapsed;
                }
            }
        }
    }
    if p != s.project {
        let notes_changed = old.patterns.iter().find(|v| v.number == s.pattern)
            .map(|v| (&v.notes, &v.controls))
            != p.patterns.iter().find(|v| v.number == s.pattern)
                .map(|v| (&v.notes, &v.controls))
            || !old.channels.iter().map(|c| c.index).eq(p.channels.iter().map(|c| c.index));
        let clips_changed = old.arrangements.get(old.arrangement_index()).map(|a| &a.clips)
            != p.arrangements.get(p.arrangement_index()).map(|a| &a.clips);
        let automation_changed = old.automations != p.automations;
        s.tempo = p.tempo.unwrap_or(s.tempo);
        s.project = p;
        s.selected_channel = s
            .selected_channel
            .min(s.project.channels.len().saturating_sub(1));
        s.plugin_channel = s
            .plugin_channel
            .min(s.project.channels.len().saturating_sub(1));
        s.rack.sel.retain(|r| *r < s.project.channels.len());
        s.mixer
            .sel
            .retain(|id| s.project.mixer.iter().any(|m| m.index == *id));
        if !s.project.mixer.iter().any(|m| m.index == s.selected_mixer) {
            s.selected_mixer = 0;
        }
        if !s.project.patterns.iter().any(|p| p.number == s.pattern) {
            s.pattern = s.project.patterns.first().map_or(1, |p| p.number);
        }
        if clips_changed {
            s.playlist.sel.clear();
            s.piano.clip_session = None;
        }
        if notes_changed || clips_changed {
            s.piano.clear_selection();
            s.piano.properties_click = None;
            s.piano.edit_length = None;
        }
        if automation_changed {
            s.playlist.selected_points = None;
        }
        s.piano.dirty = true;
    }
    Ok(())
}

pub fn parameter_id(p: &Project, key: usize, id: u32) -> Option<String> {
    if let Some((t, s)) = crate::plugins::fx_of(key) {
        p.mixer.iter().find(|m| m.index == t)?.slots[s as usize].as_ref()?;
        Some(format!("parameter/fx/{t}/{s}/{id}"))
    } else {
        Some(format!(
            "parameter/channel/{}/{id}",
            p.channels.get(key)?.index
        ))
    }
}
pub fn parameter_key(p: &Project, k: &str) -> Option<(usize, u32)> {
    let a: Vec<_> = k.split('/').collect();
    match a.as_slice() {
        ["parameter", "channel", c, id] => Some((
            p.channels
                .iter()
                .position(|n| Some(n.index) == c.parse().ok())?,
            id.parse().ok()?,
        )),
        ["parameter", "fx", t, s, id] => {
            let t = t.parse().ok()?;
            let s = s.parse::<u8>().ok()?;
            if s >= 64 {
                return None;
            }
            p.mixer.iter().find(|m| m.index == t)?.slots[s as usize].as_ref()?;
            Some((crate::plugins::fx_key(t, s), id.parse().ok()?))
        }
        _ => None,
    }
}

pub fn parameter_identity(p: &Project, key: usize) -> Option<String> {
    let (name, wrapped) = if let Some((t, s)) = crate::plugins::fx_of(key) {
        let slot = p.mixer.iter().find(|m| m.index == t)?.slots[s as usize].as_ref()?;
        (
            slot.name.clone(),
            crate::project_plugins::wrapped_slot(slot),
        )
    } else {
        let c = p.channels.get(key)?;
        (
            c.plugin.clone().unwrap_or_else(|| "Sampler".into()),
            crate::project_plugins::wrapped_channel(c),
        )
    };
    Some(wrapped.map_or(name, |r| format!("{}:{}:{}", r.format, r.name, r.id)))
}
pub fn parameter_valid(p: &Project, key: &str, v: &Value) -> bool {
    parameter_key(p, key)
        .and_then(|(k, _)| parameter_identity(p, k))
        .is_some_and(|id| v["identity"] == id)
        && v["value"]
            .as_f64()
            .is_some_and(|v| v.is_finite() && (0. ..=1.).contains(&v))
}

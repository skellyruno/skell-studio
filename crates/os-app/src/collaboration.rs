// SPDX-License-Identifier: GPL-3.0-or-later
//! Opt-in direct-host arrangement sessions. All network and package work is off the UI/audio threads.
use crate::{
    app::{Action, App, State, Ui, Win},
    dialog::{Body, DHit, Field},
};
use os_collab::{Client, Document, Host, Invitation, Proposal};
use os_model::{Arrangement, Pattern};
use os_ui::{Font, Rect};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    time::{Duration, Instant},
};
use winit::keyboard::{Key, NamedKey};
pub const LABEL: &str = "Collaborate...";
pub const VISIBLE: &str = "visible_collaboration";
type Result<T> = std::result::Result<T, String>;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[path = "collaboration_model.rs"]
mod model;
#[path = "collaboration_playback.rs"]
mod playback;
pub use model::{capture, validate};
use playback::tick as playback_tick;
fn local_doc_changed(s: &State, d: &Document) -> bool {
    capture(&s.project).map_or(true, |v| {
        v != d
            .iter()
            .filter(|(k, _)| !k.starts_with("parameter/"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    })
}
fn current(app: &App, rt: &Runtime) -> Result<Document> {
    let mut d = capture(&app.state.project)?;
    d.extend(
        rt.parameters
            .iter()
            .filter(|(k, v)| model::parameter_valid(&app.state.project, k, v))
            .map(|(k, v)| (k.clone(), v.clone())),
    );
    Ok(d)
}
fn flush_parameters(app: &mut App) {
    let pending = std::mem::take(&mut app.collaboration.remote_parameters);
    for (key, v) in pending {
        let Some((k, id)) = model::parameter_key(&app.state.project, &key) else {
            continue;
        };
        let Some(h) = app.plugins.hosted.get(&k) else {
            app.collaboration.remote_parameters.insert(key, v);
            continue;
        };
        let Ok(mut p) = h.plugin.try_lock() else {
            app.collaboration.remote_parameters.insert(key, v);
            continue;
        };
        if p.set_param(id, v["value"].as_f64().unwrap_or(0.)) {
            if let Some(l) = app.state.params.lists.get_mut(&k) {
                l.values
                    .insert(id, v["value"].as_f64().unwrap_or(0.));
            }
        }
    }
}
fn put(s: &mut State, d: &Document) -> Result<()> {
    model::put(s, d)
}

fn capture_plugin_states(app: &mut App) -> Result<()> {
    app.store_builtin_states();
    crate::project_plugins::store_hosted(&mut app.state.project, &app.plugins)
}
fn retire_plugin(app: &mut App, key: usize) {
    app.plugins.drop_key(key);
    app.state.hosted_rows.retain(|r| *r != key);
    app.sampler_failed.retain(|r| *r != key);
    app.fx_failed.retain(|r| *r != key);
    app.wrap_failed.retain(|r| *r != key);
    app.native_failed.retain(|r| *r != key);
    app.state.params.lists.remove(&key);
}
fn flush_plugins(app: &mut App) {
    if app.transport.playing() || app.collaboration.deferred_plugins.is_empty() {
        return;
    }
    for key in std::mem::take(&mut app.collaboration.deferred_plugins) {
        retire_plugin(app, key);
    }
    app.transport.set_instruments(app.plugins.instruments());
    app.transport.set_effects(app.plugins.effects());
    app.state.piano.dirty = true;
}
fn apply_shared(app: &mut App, d: &Document) -> Result<()> {
    let before = app.state.project.clone();
    put(&mut app.state, d)?;
    if before == app.state.project {
        return Ok(());
    }
    let next = &app.state.project;
    let mut changed = std::collections::BTreeSet::new();
    for (row, ch) in before.channels.iter().enumerate() {
        if next.channels.get(row).is_none_or(|n| {
            n.index != ch.index
                || n.kind != ch.kind
                || n.plugin != ch.plugin
                || n.plugin_state != ch.plugin_state
                || n.sample_path != ch.sample_path
        }) {
            changed.insert(row);
        }
    }
    for m in &before.mixer {
        for (slot, plugin) in m.slots.iter().enumerate() {
            if next
                .mixer
                .iter()
                .find(|n| n.index == m.index)
                .and_then(|n| n.slots[slot].as_ref())
                != plugin.as_ref()
            {
                changed.insert(crate::plugins::fx_key(m.index, slot as u8));
            }
        }
    }
    app.state.retimed |= before.ppq != next.ppq;
    // Loading an opaque third-party preset can take arbitrarily long. Keep it off live audio:
    // normal controls/parameter gestures still synchronize while the song plays.
    if app.transport.playing() {
        app.collaboration.deferred_plugins.extend(changed);
    } else {
        for key in changed {
            retire_plugin(app, key);
        }
    }
    app.transport.set_instruments(app.plugins.instruments());
    app.transport.set_effects(app.plugins.effects());
    app.invalidate();
    Ok(())
}

/// Merge host updates with edits made locally while waiting for an acknowledgement.
fn merge(sent: &Document, local: &Document, remote: &Document) -> Result<Document> {
    let pending = os_collab::diff(sent, local);
    let mut merged = remote.clone();
    os_collab::apply(&mut merged, &pending)?;
    validate(&merged)?;
    Ok(merged)
}

enum Event {
    Ready {
        invitation: String,
        project: Option<PathBuf>,
        document: Document,
    },
    Update {
        document: Document,
        accepted: Option<bool>,
        peers: Vec<String>,
    },
    Status(String),
    Failed(String),
}
struct Worker {
    send: SyncSender<Proposal>,
    events: Receiver<Event>,
    latest: Arc<std::sync::Mutex<Option<Event>>>,
    stop: Arc<AtomicBool>,
    playback: Arc<std::sync::Mutex<Option<(os_collab::Playback, Instant)>>>,
    received_playback: Arc<std::sync::Mutex<Option<(os_collab::Playback, Instant)>>>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
#[derive(Default)]
pub struct Runtime {
    join_baseline: Option<(os_model::Project, f64, u64)>,
    worker: Option<Worker>,
    pub status: String,
    pub invitation: String,
    last_local: Option<os_model::Project>,
    conflict: Option<Document>,
    plugin_revision: u64,
    plugin_wait: Option<Instant>,
    deferred_plugins: std::collections::BTreeSet<usize>,
    parameters: Document,
    remote_parameters: Document,
    pub follow: bool,
    hosting: bool,
    follow_check: Option<Instant>,
    shared: Document,
    inflight: Option<Document>,
    seq: u64,
    last: Option<Instant>,
    ready: bool,
    pending_open: Option<(PathBuf, Document)>,
    folder: Option<PathBuf>,
}
impl Runtime {
    pub fn active(&self) -> bool {
        self.worker.is_some()
    }
}
pub struct Form {
    pub name: Field,
    pub address: Field,
    pub invite: Field,
    pub focus: usize,
    pub status: String,
    pub active: bool,
    pub conflict: bool,
    pub follow: bool,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    Field(usize),
    Host,
    Join,
    Copy,
    Leave,
    KeepLocal,
    UseShared,
    Follow,
    Help,
    Recovery,
}
pub fn open(app: &mut App) {
    let mut name = Field::default();
    name.set(&crate::settings_file::get("collaboration_name").unwrap_or("Producer".into()));
    let mut address = Field::default();
    address.set(
        &crate::settings_file::get("collaboration_address")
            .unwrap_or_else(|| format!("{}:32190", local_address())),
    );
    let mut invite = Field::default();
    invite.set(&app.collaboration.invitation);
    app.show_form(Body::Collaboration(Form {
        name,
        address,
        invite,
        focus: 0,
        status: status(app),
        active: app.collaboration.worker.is_some(),
        conflict: app.collaboration.conflict.is_some(),
        follow: app.collaboration.follow,
    }));
}
fn local_address() -> String {
    // UDP connect selects a local route; it sends no data.
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("192.0.2.1:9")?;
            s.local_addr()
        })
        .map(|a| a.ip().to_string())
        .unwrap_or("127.0.0.1".into())
}
fn status(app: &App) -> String {
    if app.collaboration.status.is_empty() {
        "Host a private session or paste an invitation to join.".into()
    } else {
        app.collaboration.status.clone()
    }
}
fn form(app: &mut App) -> Option<&mut Form> {
    match &mut app.state.dialogs.last_mut()?.body {
        Body::Collaboration(f) => Some(f),
        _ => None,
    }
}
fn error(app: &mut App, message: String) {
    app.collaboration.status = message.clone();
    app.state.message = Some(message.clone());
    if let Some(f) = form(app) {
        f.status = message;
    }
    app.invalidate();
}
pub fn leave(app: &mut App) {
    app.collaboration = Runtime::default();
    app.state.collaborating = false;
    // Old native snapshots must not restore a pre-session project after leaving.
    app.state.history.reset();
}
fn folder() -> Result<PathBuf> {
    let root = os_paths::user_dir()
        .ok_or("User folder unavailable")?
        .join("Collaboration");
    let p = root.join(os_collab::random_id()?);
    std::fs::create_dir_all(&p).map_err(err)?;
    Ok(p)
}
pub fn command(app: &mut App, c: Command) {
    match c {
        Command::Follow => {
            app.collaboration.follow = !app.collaboration.follow;
            if let Some(f) = form(app) {
                f.follow = !f.follow;
            }
        }
        Command::Recovery => {
            if let Some(dir) = &app.collaboration.folder {
                #[cfg(not(test))]
                {
                    let program = if cfg!(windows) {
                        "explorer"
                    } else if cfg!(target_os = "macos") {
                        "open"
                    } else {
                        "xdg-open"
                    };
                    let _ = std::process::Command::new(program).arg(dir).spawn();
                }
                #[cfg(test)]
                let _ = dir;
            }
        }
        Command::Help => {
            error(app,"Same Wi-Fi: use the host LAN address. Internet: use a shared VPN address, or forward TCP 32190 on the host router. Invitations are private access keys.".into());
        }
        Command::KeepLocal | Command::UseShared => {
            let Some(remote) = app.collaboration.conflict.take() else {
                return;
            };
            if let Some(dir) = app.collaboration.folder.clone() {
                match app
                    .save_copy(dir.join(format!("Before conflict {}.lmn", app.collaboration.seq)))
                {
                    Some(Ok(())) => {}
                    _ => {
                        app.collaboration.conflict = Some(remote);
                        return error(
                            app,
                            "Could not save the conflict copy. Save your project before resolving."
                                .into(),
                        );
                    }
                }
            }
            let chosen = if c == Command::UseShared {
                Ok(remote.clone())
            } else {
                current(app, &app.collaboration).and_then(|local| {
                    let mut merged = remote.clone();
                    for change in os_collab::diff(&app.collaboration.shared, &local) {
                        if let Some(v) = change.after {
                            merged.insert(change.key, v);
                        } else {
                            merged.remove(&change.key);
                        }
                    }
                    validate(&merged)?;
                    Ok(merged)
                })
            };
            let chosen = match chosen {
                Ok(d) => d,
                Err(e) => {
                    app.collaboration.conflict = Some(remote);
                    return error(
                        app,
                        format!(
                            "Cannot merge this structural edit: {e}. Save a copy, then use the shared version."
                        ),
                    );
                }
            };
            if let Err(e) = apply_shared(app, &chosen) {
                app.collaboration.conflict = Some(remote);
                return error(app, e);
            }
            {
                app.collaboration.parameters = chosen
                    .iter()
                    .filter(|(k, _)| k.starts_with("parameter/"))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                app.collaboration.remote_parameters = app.collaboration.parameters.clone();
            }
            app.collaboration.shared = remote;
            app.collaboration.inflight = None;
            app.collaboration.last_local = None;
            app.collaboration.status="Conflict resolved; synchronizing. Your earlier version was saved in the session folder.".into();
            if let Some(f) = form(app) {
                f.conflict = false;
            }
        }
        Command::Field(i) => {
            if let Some(f) = form(app) {
                f.focus = i;
                match i {
                    0 => f.name.select_all(),
                    1 => f.address.select_all(),
                    _ => f.invite.select_all(),
                }
            }
        }
        Command::Leave => {
            leave(app);
            if let Some(f) = form(app) {
                f.active = false;
                f.status = "Session ended. Your project remains open.".into();
                f.invite.set("");
            }
        }
        Command::Copy => {
            match arboard::Clipboard::new()
                .and_then(|mut c| c.set_text(app.collaboration.invitation.clone()))
            {
                Ok(()) => error(
                    app,
                    "Invitation copied. Share it privately with your collaborator.".into(),
                ),
                Err(e) => error(app, format!("Could not copy invitation: {e}")),
            }
        }
        Command::Host | Command::Join => {
            if app.collaboration.worker.is_some() {
                return;
            }
            let Some(f) = form(app) else { return };
            let (name, address, invite) = (
                f.name.text.trim().to_string(),
                f.address.text.trim().to_string(),
                f.invite.text.trim().to_string(),
            );
            if name.is_empty() || name.len() > 64 {
                return error(app, "Enter a name (up to 64 bytes).".into());
            }
            crate::settings_file::write("collaboration_name", &name);
            if c == Command::Join {
                if let Err(e) = Invitation::parse(&invite) {
                    return error(app, e);
                }
                app.state.dialogs.pop();
                app.modal_changed();
                app.ask_discard(
                    "joining the shared project",
                    crate::dialog::Next::CollaborationJoin(invite, name),
                );
                return;
            }
            if app.transport.playing() {
                return error(app,"Stop playback before preparing a hosted session. Playback is independent once connected.".into());
            }
            crate::settings_file::write("collaboration_address", &address);
            let Ok(port) = address.rsplit(':').next().unwrap_or("").parse::<u16>() else {
                return error(
                    app,
                    "Use a host address and port, for example 192.168.1.20:32190.".into(),
                );
            };
            if port == 0 {
                return error(app, "Choose a port between 1 and 65535.".into());
            }
            let result = (|| {
                let dir = folder()?;
                capture_plugin_states(app)?;
                let doc = capture(&app.state.project)?;
                validate(&doc)?;
                let zip = dir.join("Shared project.zip");
                let job = crate::zipped::collaboration_pack(app, &zip)?;
                let initial = doc.clone();
                let advertised = address.clone();
                let worker = spawn_worker(dir.clone(), name, move |stop, media| {
                    job()?;
                    if stop.load(Ordering::Acquire) {
                        return Err("Cancelled".into());
                    }
                    let host = Host::start(
                        ([0, 0, 0, 0], port).into(),
                        advertised,
                        zip,
                        media.encode(&initial, None, stop)?,
                        validate,
                    )?;
                    let local = host.invitation.with_address(format!("127.0.0.1:{port}"));
                    Ok((Some(host), local, None))
                })?;
                app.collaboration = Runtime {
                    worker: Some(worker),
                    folder: Some(dir),
                    shared: doc,
                    plugin_revision: app.plugins.edit_revision,
                    hosting: true,
                    ..Default::default()
                };
                app.state.collaborating = true;
                app.state.history.reset();
                Ok(())
            })();
            if let Err(e) = result {
                return error(app, e);
            }
            error(
                app,
                "Preparing project and starting the session server...".into(),
            );
            if let Some(f) = form(app) {
                f.active = true;
            }
        }
    }
    app.invalidate();
}
pub fn join(app: &mut App, text: String, name: String) {
    let result = (|| {
        let invitation = Invitation::parse(&text)?;
        let dir = folder()?;
        // Save a checkpoint even if the user selected Don't save.
        match app.save_copy(dir.join("Before joining.lmn")) {
            Some(Ok(())) => {}
            Some(Err(e)) => return Err(e),
            None => return Err("Wait for the project to finish loading".into()),
        }
        let worker = spawn_worker(dir.clone(), name, move |_, _| {
            Ok((None, invitation, Some(())))
        })?;
        app.collaboration = Runtime {
            worker: Some(worker),
            folder: Some(dir),
            join_baseline: Some((
                app.state.project.clone(),
                app.state.tempo,
                app.plugins.edit_revision,
            )),
            ..Default::default()
        };
        app.state.collaborating = true;
        Ok(())
    })();
    match result {
        Ok(()) => {
            error(app, "Connecting to the host...".into());
            open(app);
        }
        Err(e) => error(app, e),
    }
}
fn spawn_worker(
    dir: PathBuf,
    name: String,
    prepare: impl FnOnce(
        &AtomicBool,
        &mut os_collab::assets::Media,
    ) -> Result<(Option<Host>, Invitation, Option<()>)>
    + Send
    + 'static,
) -> Result<Worker> {
    let (tx, rx) = mpsc::sync_channel::<Proposal>(1);
    let (events, out) = mpsc::sync_channel::<Event>(8);
    let stop = Arc::new(AtomicBool::new(false));
    let halted = stop.clone();
    let latest = Arc::new(std::sync::Mutex::new(None));
    let updates = latest.clone();
    let playback = Arc::new(std::sync::Mutex::new(
        None::<(os_collab::Playback, Instant)>,
    ));
    let play = playback.clone();
    let received_playback = Arc::new(std::sync::Mutex::new(None));
    let received = received_playback.clone();
    std::thread::Builder::new()
        .name("collaboration".into())
        .spawn(move || {
            let run = || -> Result<()> {
                let mut media = os_collab::assets::Media::new(dir.join("assets"))?;
                let (host, invite, download) = prepare(&halted, &mut media)?;
                let peer = os_collab::random_id()?;
                let mut client = Client::connect(&invite, &peer, &name)?;
                validate(&client.document)?;
                let project = if download.is_some() {
                    let _ = events.try_send(Event::Status(
                        "Downloading shared project and samples...".into(),
                    ));
                    let zip = dir.join("Shared project.zip");
                    let mut retries = 0;
                    loop {
                        match client.download(&zip, &halted) {
                            Ok(()) => break,
                            Err(e) => {
                                if halted.load(Ordering::Acquire) || retries >= 20 {
                                    return Err(e);
                                }
                                retries += 1;
                                std::thread::sleep(Duration::from_millis(250));
                                client = reconnect(&invite, &peer, &name, &halted)?;
                            }
                        }
                    }
                    Some(crate::zipped::collaboration_unpack(&zip)?)
                } else {
                    None
                };
                // Refresh after a long initial transfer; the package remains a fixed baseline.
                if client.poll(None).is_err() {
                    client = reconnect(&invite, &peer, &name, &halted)?;
                    client.poll(None)?;
                }
                let text = host
                    .as_ref()
                    .map_or_else(|| invite.encode(), |h| h.invitation.encode())?;
                events
                    .send(Event::Ready {
                        invitation: text,
                        project,
                        document: media.decode(&client.document.clone(), &mut client, &halted)?,
                    })
                    .map_err(err)?;
                let mut pending = None;
                let mut failures = 0;
                let mut checkpoint = None;
                let mut last_report: Option<(u64, Vec<String>)> = None;
                while !halted.load(Ordering::Acquire) {
                    if pending.is_none() {
                        if let Some(doc) = checkpoint.take() {
                            let _ = write_recovery(&dir, &doc);
                        }
                        match rx.recv_timeout(Duration::from_millis(200)) {
                            Ok(p) => pending = Some(p),
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                            Err(_) => {}
                        }
                    }
                    // Persist the intent before sending; a disconnect or crash cannot erase it.
                    if let Some(p) = &pending {
                        write_journal(&dir, "pending.json", p)?;
                    }
                    let network = (|| -> Result<(os_collab::Update, Option<Document>)> {
                        let wire = if let Some(p) = &pending {
                            let mut p: Proposal = p.clone();
                            let before = p
                                .changes
                                .iter()
                                .filter_map(|c| Some((c.key.clone(), c.before.clone()?)))
                                .collect();
                            let after = p
                                .changes
                                .iter()
                                .filter_map(|c| Some((c.key.clone(), c.after.clone()?)))
                                .collect();
                            let before = media.encode(&before, Some(&mut client), &halted)?;
                            let after = media.encode(&after, Some(&mut client), &halted)?;
                            for c in &mut p.changes {
                                c.before = before.get(&c.key).cloned();
                                c.after = after.get(&c.key).cloned();
                            }
                            Some(p)
                        } else {
                            None
                        };
                        let clock = play.lock().map_err(err)?.as_ref().map(|(p, t)| {
                            let mut p = p.clone();
                            p.advance(t.elapsed().as_secs_f64());
                            p
                        });
                        let u = client.poll_playback(wire, clock)?;
                        let changed = u.accepted.is_some()
                            || last_report.as_ref() != Some(&(client.revision, u.peers.clone()));
                        let doc = if changed {
                            Some(media.decode(&client.document.clone(), &mut client, &halted)?)
                        } else {
                            None
                        };
                        Ok((u, doc))
                    })();
                    match network {
                        Ok((u, localized)) => {
                            failures = 0;
                            if let Some(p) = u.playback.clone() {
                                *received.lock().map_err(err)? = Some((p, Instant::now()));
                            }
                            if !u.changes.is_empty() || u.accepted.is_some() {
                                write_journal(&dir, "arrangement.json", &client.document)?;
                            }

                            // One bounded queue; never wait for the UI to repaint.
                            if u.accepted.is_some()
                                || last_report.as_ref() != Some(&(client.revision, u.peers.clone()))
                            {
                                let localized = localized.ok_or("Missing shared update")?;
                                checkpoint = Some(localized.clone());
                                if u.accepted == Some(false) {
                                    if let Some(p) = &pending {
                                        write_journal(&dir, "conflicting edit.json", p)?;
                                    }
                                }
                                let mut slot = updates.lock().map_err(err)?;
                                let old = match slot.take() {
                                    Some(Event::Update { accepted, .. }) => accepted,
                                    _ => None,
                                };
                                *slot = Some(Event::Update {
                                    document: localized,
                                    accepted: u.accepted.or(old),
                                    peers: u.peers.clone(),
                                });
                                last_report = Some((client.revision, u.peers));
                            }
                            if u.accepted.is_some() {
                                pending = None;
                                let _ = std::fs::remove_file(dir.join("pending.json"));
                            }
                        }
                        Err(e) => {
                            failures += 1;
                            if failures >= 8 {
                                return Err(format!(
                                    "Synchronization paused: {e}. Your local project is retained."
                                ));
                            }
                            let _ = events.try_send(Event::Status(
                                "Reconnecting — local work is retained...".into(),
                            ));
                            client = reconnect(&invite, &peer, &name, &halted)?;
                        }
                    }
                }
                drop(host);
                Ok(())
            };
            if let Err(e) = run() {
                let _ = events.try_send(Event::Failed(e));
            }
        })
        .map_err(err)?;
    Ok(Worker {
        send: tx,
        events: out,
        latest,
        stop,
        playback,
        received_playback,
    })
}
fn write_recovery(dir: &std::path::Path, doc: &Document) -> Result<()> {
    let project = model::decode(doc)?;
    let bytes = os_flp::project::create(&project)
        .map_err(err)?
        .to_bytes()
        .map_err(err)?;
    let temp = dir.join("Latest shared project.part");
    let dest = dir.join("Latest shared project.lmn");
    let previous = dir.join("Previous shared project.lmn");
    std::fs::write(&temp, bytes).map_err(err)?;
    if dest.exists() {
        let _ = std::fs::remove_file(&previous);
        std::fs::rename(&dest, &previous).map_err(err)?;
    }
    std::fs::rename(temp, dest).map_err(err)
}
fn reconnect(invite: &Invitation, peer: &str, name: &str, stop: &AtomicBool) -> Result<Client> {
    for _ in 0..20 {
        if stop.load(Ordering::Acquire) {
            return Err("Cancelled".into());
        }
        std::thread::sleep(Duration::from_millis(250));
        if let Ok(c) = Client::connect(invite, peer, name) {
            return Ok(c);
        }
    }
    Err("Host unavailable. Your project and pending changes are saved locally.".into())
}
fn write_journal(dir: &std::path::Path, name: &str, v: &impl serde::Serialize) -> Result<()> {
    let path = dir.join(name);
    let temp = dir.join(format!("{name}.tmp"));
    let bytes = serde_json::to_vec(v).map_err(err)?;
    std::fs::write(&temp, bytes).map_err(err)?;
    // Windows rename does not replace an existing file. Keep the previous checkpoint until the new file is complete.
    let previous = dir.join(format!("{name}.previous"));
    if path.exists() {
        let _ = std::fs::remove_file(&previous);
        std::fs::rename(&path, &previous).map_err(err)?;
    }
    std::fs::rename(temp, path).map_err(err)
}
pub fn tick(app: &mut App) {
    if app.collaboration.worker.is_none() {
        return;
    }
    // Never replace arrays while a gesture or a properties dialog refers to their indices.
    let modal_other = app
        .state
        .dialogs
        .iter()
        .any(|m| !matches!(m.body, Body::Collaboration(_)));
    if app.drag.is_some() || app.pressed.is_some() || modal_other {
        return;
    }
    if app
        .collaboration
        .last
        .is_some_and(|t| t.elapsed() < Duration::from_millis(150))
    {
        return;
    }
    app.collaboration.last = Some(Instant::now());
    if app.plugins.edit_revision != app.collaboration.plugin_revision {
        app.collaboration.plugin_revision = app.plugins.edit_revision;
        app.collaboration.plugin_wait = Some(Instant::now());
    }
    if !app.transport.playing()
        && app
            .collaboration
            .plugin_wait
            .is_some_and(|t| t.elapsed() > Duration::from_millis(750))
        && app
            .state
            .params
            .lists
            .values()
            .all(|l| l.gestures.is_empty())
    {
        match capture_plugin_states(app) {
            Ok(()) => app.collaboration.plugin_wait = None,
            Err(e) => {
                error(app, e);
                return;
            }
        }
    }
    flush_plugins(app);
    flush_parameters(app);
    playback_tick(app);
    let edits = std::mem::take(&mut *app.state.params.collaboration.borrow_mut());
    for ((key, id), v) in edits {
        if let (Some(key), Some(identity)) = (
            model::parameter_id(&app.state.project, key, id),
            model::parameter_identity(&app.state.project, key),
        ) {
            app.collaboration
                .parameters
                .insert(key, serde_json::json!({"identity":identity,"value":v}));
            app.collaboration.last_local = None;
        }
    }
    let mut rt = std::mem::take(&mut app.collaboration);
    let mut failure = None;
    if let Some((path, doc)) = rt.pending_open.take() {
        let changed = rt.join_baseline.as_ref().is_some_and(|(p, t, r)| {
            *p != app.state.project || *t != app.state.tempo || *r != app.plugins.edit_revision
        });
        if changed {
            failure=Some("Joining cancelled because you edited the current project during the download. Your work is untouched; save it and join again.".into());
        } else {
            app.open(path.clone());
            if app.state.path.as_ref() != Some(&path) {
                failure = Some("Could not open the shared project".into());
            } else {
                rt.shared = doc;
                rt.ready = false;
            }
        }
    }
    if !rt.ready
        && !app.loading()
        && !rt.shared.is_empty()
        && rt.worker.is_some()
        && !rt.invitation.is_empty()
    {
        match apply_shared(app, &rt.shared) {
            Ok(()) => {
                rt.parameters = rt
                    .shared
                    .iter()
                    .filter(|(k, _)| k.starts_with("parameter/"))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                rt.remote_parameters = rt.parameters.clone();
                rt.ready = true;
                app.state.collaborating = true;
                app.state.history.reset();
            }
            Err(e) => failure = Some(e),
        }
    }
    if rt.ready {
        let local_changed = rt.last_local.as_ref() != Some(&app.state.project);
        if rt.inflight.is_none() && rt.conflict.is_none() && failure.is_none() && local_changed {
            match current(app, &rt) {
                Ok(current) => {
                    let changes = os_collab::diff(&rt.shared, &current);
                    if changes.is_empty() {
                        // Local lane height/folding changes are not shared. Cache
                        // them too, so idle ticks do not keep serializing them.
                        rt.last_local = Some(app.state.project.clone());
                    }
                    if !changes.is_empty() {
                        if changes.iter().any(|c| c.key == "schema") {
                            failure = Some("Timebase changed. Start a new session.".into());
                        } else if validate(&current).is_ok() {
                            rt.seq += 1;
                            if rt
                                .worker
                                .as_ref()
                                .unwrap()
                                .send
                                .try_send(Proposal {
                                    seq: rt.seq,
                                    changes,
                                })
                                .is_ok()
                            {
                                rt.last_local = Some(app.state.project.clone());
                                rt.inflight = Some(current);
                                rt.status = "Syncing changes...".into();
                            } else {
                                rt.seq -= 1;
                            }
                        } else {
                            failure=Some("This edit cannot be shared in the current session. Local work is preserved.".into());
                        }
                    }
                }
                Err(e) => failure = Some(e),
            }
        }
    }
    for _ in 0..8 {
        let event = match rt.worker.as_ref().map(|w| w.events.try_recv()) {
            Some(Ok(event)) => event,
            Some(Err(mpsc::TryRecvError::Empty)) => {
                let next = rt
                    .worker
                    .as_ref()
                    .and_then(|w| w.latest.lock().ok()?.take());
                if let Some(e) = next {
                    e
                } else {
                    break;
                }
            }
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                failure.get_or_insert("Session connection closed. Local work is preserved.".into());
                break;
            }
            _ => break,
        };
        match event {
            Event::Ready {
                invitation,
                project,
                document,
            } => {
                rt.invitation = invitation;
                rt.status = "Connected — shared project".into();
                if let Some(path) = project {
                    rt.pending_open = Some((path, document));
                } else {
                    // Host may have edited while its package was being prepared.
                    rt.shared = document;
                    rt.ready = true;
                }
            }
            Event::Update {
                document,
                accepted,
                peers,
            } => {
                if accepted == Some(false) || rt.conflict.is_some() {
                    rt.inflight = None;
                    rt.conflict = Some(document);
                    rt.status="Conflicting edit: choose Keep my version or Use shared below. Both versions are preserved.".into();
                    continue;
                }
                if !rt.ready {
                    if let Some((_, d)) = &mut rt.pending_open {
                        *d = document;
                    } else {
                        rt.shared = document;
                    }
                    continue;
                }
                let sent = if accepted == Some(true) {
                    rt.inflight.as_ref().unwrap_or(&rt.shared)
                } else {
                    &rt.shared
                };
                if rt.inflight.is_some() && accepted.is_none() {
                    continue;
                }
                let local = current(app, &rt);
                match local.and_then(|local| merge(sent, &local, &document)) {
                    Ok(merged) => {
                        let changed = local_doc_changed(&app.state, &merged);
                        if changed {
                            crate::history::collaboration_received(&mut app.state);
                        }
                        if let Err(e) = apply_shared(app, &merged) {
                            failure = Some(e);
                            break;
                        }
                        rt.parameters = merged
                            .iter()
                            .filter(|(k, _)| k.starts_with("parameter/"))
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        for (k, v) in &rt.parameters {
                            if rt.shared.get(k) != Some(v) {
                                rt.remote_parameters.insert(k.clone(), v.clone());
                            }
                        }
                        rt.shared = document;
                        rt.last_local = None;
                        if accepted == Some(true) {
                            rt.inflight = None;
                        }
                        rt.status =
                            format!("Synced · {} connected · {}", peers.len(), peers.join(", "));
                        if changed {
                            app.damage_win(Win::Playlist);
                            app.damage_win(Win::PianoRoll);
                            app.damage_win(Win::ChannelRack);
                        }
                    }
                    Err(e) => {
                        rt.inflight = None;
                        rt.conflict = Some(document);
                        rt.status =
                            format!("{e}. Choose which version to keep in File > Collaborate.");
                    }
                }
            }
            Event::Status(s) => rt.status = s,
            Event::Failed(e) => {
                failure = Some(e);
                break;
            }
        }
    }
    if let Some(e) = failure {
        if let Some(dir) = &rt.folder {
            let _ = app.save_copy(dir.join("Recovered local project.lmn"));
        }
        rt.worker = None;
        rt.ready = false;
        rt.status = e.clone();
        app.state.collaborating = false;
        app.state.message = Some(e);
    }
    rt.deferred_plugins
        .append(&mut app.collaboration.deferred_plugins);
    app.collaboration = rt;
    let conflict = app.collaboration.conflict.is_some();
    let (status, invite, active) = (
        status(app),
        app.collaboration.invitation.clone(),
        app.collaboration.worker.is_some(),
    );
    if let Some(f) = form(app) {
        if f.status != status || f.active != active || f.conflict != conflict {
            f.status = status;
            f.active = active;
            f.conflict = conflict;
            if !invite.is_empty() {
                f.invite.set(&invite);
            }
            app.invalidate();
        }
    }
}
pub fn key(app: &mut App, key: &Key, ctrl: bool, alt: bool) {
    if matches!(key, Key::Named(NamedKey::Escape)) {
        app.state.dialogs.pop();
        app.modal_changed();
        return;
    }
    if let Some(f) = form(app) {
        if matches!(key, Key::Named(NamedKey::Tab)) {
            f.focus = (f.focus + 1) % 3;
        } else {
            let field = match f.focus {
                0 => &mut f.name,
                1 => &mut f.address,
                _ => &mut f.invite,
            };
            if ctrl && matches!(key,Key::Character(c) if c.eq_ignore_ascii_case("v")) {
                if let Ok(text) = arboard::Clipboard::new().and_then(|mut c| c.get_text()) {
                    if text.len() <= 8192 {
                        field.insert(text.trim());
                    }
                }
            } else if field.len() < 8192 || !matches!(key, Key::Character(_)) {
                field.key(key, ctrl, alt);
            }
        }
    }
    app.invalidate();
}
pub fn paint(ui: &mut Ui, r: Rect, f: &Form) {
    for (i, label) in [
        "Your name",
        "Host address and port (LAN, VPN or forwarded internet address)",
        "Private invitation (paste here to join)",
    ]
    .iter()
    .enumerate()
    {
        let y = r.y + 42. + i as f32 * 66.;
        ui.c.text(Font::new(12), r.x + 18., y, ui.t.text_dim, label);
        let b = Rect::new(r.x + 18., y + 19., r.w - 36., 30.);
        ui.c.fill_rrect(b, 3., ui.t.sunken);
        ui.c.stroke_rrect(
            b,
            3.,
            1.,
            if f.focus == i {
                ui.t.accent
            } else {
                ui.t.button
            },
        );
        let field = match i {
            0 => &f.name,
            1 => &f.address,
            _ => &f.invite,
        };
        ui.c.text_clip(
            Font::new(12),
            b.x + 8.,
            b.y + 8.,
            b.w - 16.,
            ui.t.text,
            &field.text,
        );
        ui.hit(
            b,
            Action::Dlg(DHit::Collaboration(Command::Field(i))),
            *label,
        );
    }
    for (i, (label, c, enabled)) in [
        ("Host session", Command::Host, !f.active),
        ("Join session", Command::Join, !f.active),
        (
            "Copy invitation",
            Command::Copy,
            f.active && !f.invite.text.is_empty(),
        ),
        ("End / leave", Command::Leave, f.active),
    ]
    .iter()
    .enumerate()
    {
        let b = Rect::new(r.x + 18. + i as f32 * 139., r.y + 250., 130., 32.);
        ui.c.fill_rrect(b, 4., ui.t.button);
        ui.c.text_clip(
            Font::new(12),
            b.x + 10.,
            b.y + 9.,
            b.w - 20.,
            if *enabled { ui.t.text } else { ui.t.text_dim },
            label,
        );
        if *enabled {
            ui.hit(b, Action::Dlg(DHit::Collaboration(*c)), *label);
        }
    }
    for (i, (label, c)) in (if f.conflict {
        [
            ("Keep my version", Command::KeepLocal),
            ("Use shared version", Command::UseShared),
        ]
    } else {
        [
            (
                if f.follow {
                    "Following host"
                } else {
                    "Follow host playback"
                },
                Command::Follow,
            ),
            ("Connection help", Command::Help),
        ]
    })
    .iter()
    .enumerate()
    {
        let b = Rect::new(r.x + 18. + i as f32 * 278., r.y + 286., 269., 28.);
        ui.c.fill_rrect(b, 3., ui.t.button);
        ui.c.text_clip(
            Font::new(12),
            b.x + 8.,
            b.y + 7.,
            b.w - 16.,
            ui.t.text,
            label,
        );
        ui.hit(b, Action::Dlg(DHit::Collaboration(*c)), *label);
    }
    let split = f
        .status
        .char_indices()
        .filter(|(i, c)| *i <= 80 && c.is_whitespace())
        .map(|(i, _)| i)
        .last()
        .filter(|_| f.status.len() > 80)
        .unwrap_or(f.status.len());
    ui.c.text_clip(
        Font::new(12),
        r.x + 18.,
        r.y + 327.,
        r.w - 36.,
        ui.t.text,
        &f.status[..split],
    );
    ui.c.text_clip(
        Font::new(12),
        r.x + 18.,
        r.y + 344.,
        r.w - 36.,
        ui.t.text,
        f.status[split..].trim(),
    );
    let recovery = Rect::new(r.x + r.w - 168., r.y + 322., 150., 22.);
    if f.active {
        ui.c.text_clip(
            Font::new(11),
            recovery.x,
            recovery.y,
            150.,
            ui.t.accent,
            "Open recovery folder",
        );
        ui.hit(
            recovery,
            Action::Dlg(DHit::Collaboration(Command::Recovery)),
            "Open session checkpoints and preserved conflict versions",
        );
    }
    for (i, line) in [
        "Shared project: notes, clips, Mixer, automation, tempo and instruments.",
        "New audio transfers securely in the background. Plugin licences stay local.",
        "Plugin presets synchronize when stopped; local audio keeps running during transfers.",
        "Internet hosts may need TCP port forwarding or a private VPN. No website server.",
        "Keep invitations private. The host must keep AQUILA open.",
    ]
    .iter()
    .enumerate()
    {
        ui.c.text_clip(
            Font::new(11),
            r.x + 18.,
            r.y + 366. + i as f32 * 16.,
            r.w - 36.,
            ui.t.text_dim,
            line,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collaboration_mixer_updates_preserve_editor_selections() {
        let mut app = App::new(None);
        let channel = app.state.project.channels[0].index;
        app.state.pattern = 1;
        app.state.project.patterns = vec![Pattern {
            number: 1,
            notes: vec![os_model::Note::step(channel, 0)],
            ..Default::default()
        }];
        app.state.project.arrangements = vec![Arrangement {
            number: 0,
            clips: vec![os_model::Clip::new(0, 96, 0, os_model::ClipSource::Pattern(1))],
            ..Default::default()
        }];
        app.state.piano.select(1, channel, vec![0]);
        app.state.playlist.sel = vec![0];
        let mut remote = app.state.project.clone();
        remote.mixer[1].pan = Some(0.3);
        model::put(&mut app.state, &capture(&remote).unwrap()).unwrap();
        assert_eq!(app.state.piano.selection(1, Some(channel)), vec![0]);
        assert_eq!(app.state.playlist.sel, vec![0]);
        remote.patterns[0].notes.clear();
        model::put(&mut app.state, &capture(&remote).unwrap()).unwrap();
        assert!(app.state.piano.selection(1, Some(channel)).is_empty());
        assert_eq!(app.state.playlist.sel, vec![0]);
        remote.arrangements[0].clips.clear();
        model::put(&mut app.state, &capture(&remote).unwrap()).unwrap();
        assert!(app.state.playlist.sel.is_empty());
    }
    #[test]
    fn collaboration_dialog_visibility_and_cancel() {
        let mut a = App::new(None);
        open(&mut a);
        a.state.screen = (1000., 760.);
        let mut canvas = os_ui::Canvas::new(1000, 760, 1.).unwrap();
        a.paint(&mut canvas, None);
        assert!(
            a.hits
                .iter()
                .any(|h| h.action == Action::Dlg(DHit::Collaboration(Command::Host)))
        );
        if let Some(dir) = std::env::var_os("OS_SHOT_DIR") {
            canvas
                .pixmap
                .save_png(PathBuf::from(dir).join("collaboration.png"))
                .unwrap();
        }
        key(&mut a, &Key::Named(NamedKey::Escape), false, false);
        assert!(!a.collaboration.active());
        assert!(a.state.dialogs.is_empty());
        crate::settings_file::set_flag(VISIBLE, false);
        let menu = crate::views::menus::item_at(0, 21).unwrap();
        assert_eq!(menu.label, LABEL);
        assert!(!crate::views::menus::in_menu(&a.state, 0, &menu));
        a.state.collaborating = true;
        assert!(crate::views::menus::in_menu(&a.state, 0, &menu));
        crate::settings_file::set_flag(VISIBLE, true);
    }
    fn scratch() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "ml-collaboration-{}",
            os_collab::random_id().unwrap()
        ));
        std::fs::create_dir(&p).unwrap();
        p
    }
    #[test]
    fn collaboration_package_roundtrip_and_path_rejection() {
        use std::io::Write;
        let dir = scratch();
        let mut app = App::new(None);
        let zip = dir.join("Project.zip");
        app.state.project.patterns[0]
            .notes
            .push(os_model::Note::step(app.state.project.channels[0].index, 0));
        crate::zipped::collaboration_pack(&mut app, &zip).unwrap()().unwrap();
        let project = crate::zipped::collaboration_unpack(&zip).unwrap();
        let bytes = std::fs::read(project).unwrap();
        let flp = os_flp::FlpFile::parse(&bytes).unwrap();
        let loaded = os_flp::project::load(&flp).unwrap();
        assert_eq!(loaded.patterns, app.state.project.patterns);
        let unsafe_zip = dir.join("unsafe.zip");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&unsafe_zip).unwrap());
        writer
            .start_file("../outside.lmn", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"bad").unwrap();
        writer.finish().unwrap();
        assert!(crate::zipped::collaboration_unpack(&unsafe_zip).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn collaboration_undo_refuses_to_erase_later_remote_work() {
        let mut a = App::new(None);
        a.state.collaborating = true;
        a.state.project.patterns = vec![Pattern {
            number: 1,
            ..Default::default()
        }];
        crate::history::record(
            &mut a.state,
            "Local note",
            crate::history::Snap::Pattern {
                number: 1,
                notes: vec![],
                controls: vec![],
                length: None,
            },
        );
        let note = os_model::Note::step(a.state.project.channels[0].index, 0);
        a.state.project.patterns[0].notes.push(note);
        crate::history::collaboration_received(&mut a.state);
        a.state.project.patterns[0].notes.push(os_model::Note {
            position: 96,
            ..note
        });
        assert!(crate::history::undo(&mut a.state).is_none());
        assert_eq!(a.state.project.patterns[0].notes.len(), 2);
    }
    #[test]
    fn shared_merge_preserves_independent_edits_and_rejects_conflicts() {
        let mut app = App::new(None);
        app.state.project.patterns = vec![
            Pattern {
                number: 1,
                ..Default::default()
            },
            Pattern {
                number: 2,
                ..Default::default()
            },
        ];
        let base = capture(&app.state.project).unwrap();
        validate(&base).unwrap();
        let mut a = base.clone();
        a.get_mut("pattern/1").unwrap()["name"] = serde_json::json!("Mine");
        let mut b = base.clone();
        b.get_mut("pattern/2").unwrap()["name"] = serde_json::json!("Theirs");
        let merged = merge(&base, &a, &b).unwrap();
        assert_eq!(merged["pattern/1"]["name"], "Mine");
        assert_eq!(merged["pattern/2"]["name"], "Theirs");
        b.get_mut("pattern/1").unwrap()["name"] = serde_json::json!("Conflict");
        assert!(merge(&base, &a, &b).is_err());
        b.get_mut("pattern/1").unwrap()["notes"] = serde_json::json!([{"invalid":true}]);
        assert!(validate(&b).is_err());
    }
}

// SPDX-License-Identifier: GPL-3.0-only
//! The matching Gridline-derived engine travels inside each AQUILA update.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Read},
    path::{Component, Path, PathBuf},
    sync::OnceLock,
};

pub const NAME: &str = "ML Plasma";
pub const ID: &str = "de.michaellane.ml-plasma";
pub const VISIBILITY: &str = "visible_plugin-plasma";
const FILE: &str = "ML Plasma.clap";
const BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/plasma.zip"));

#[derive(Deserialize)]
struct Manifest {
    os: String,
    arch: String,
    files: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    path: String,
    sha256: String,
    size: u64,
}

fn safe_path(name: &str) -> Result<&Path, String> {
    let path = Path::new(name);
    let allowed = name == FILE
        || ["ML Plasma.clap/", "ML Plasma-assets/", "Licences/"]
            .iter()
            .any(|root| name.starts_with(root));
    if !allowed
        || name.contains(['\\', ':'])
        || !path.components().all(|c| matches!(c, Component::Normal(_)))
    {
        return Err(format!("Invalid ML Plasma payload path: {name}"));
    }
    Ok(path)
}

fn verified(root: &Path, files: &[Entry]) -> bool {
    files.iter().all(|e| {
        safe_path(&e.path).ok().is_some_and(|p| {
            let p = root.join(p);
            fs::symlink_metadata(&p)
                .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink() && m.len() == e.size)
                && fs::read(p).is_ok_and(|data| format!("{:x}", Sha256::digest(&data)) == e.sha256)
        })
    })
}

/// Validates the complete archive before exposing a native module to the host.
fn unpack(bytes: &[u8], root: &Path) -> Result<PathBuf, String> {
    let err = |e: &dyn std::fmt::Display| e.to_string();
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| err(&e))?;
    let manifest: Manifest = {
        let file = archive.by_name("engine.json").map_err(|e| err(&e))?;
        serde_json::from_reader(file.take(1024 * 1024)).map_err(|e| err(&e))?
    };
    if manifest.os != std::env::consts::OS
        || manifest.arch != std::env::consts::ARCH
        || manifest.files.is_empty()
        || archive.len() != manifest.files.len() + 1
    {
        return Err("ML Plasma payload does not match this application".into());
    }
    let mut names = std::collections::HashSet::new();
    for entry in &manifest.files {
        safe_path(&entry.path)?;
        if !names.insert(&entry.path) || entry.size > 256 * 1024 * 1024 {
            return Err("Invalid ML Plasma manifest".into());
        }
    }
    let hash = format!("{:x}", Sha256::digest(bytes));
    let dest = root.join(&hash);
    if verified(&dest, &manifest.files) {
        return Ok(dest.join(FILE));
    }
    fs::create_dir_all(root).map_err(|e| err(&e))?;
    let serial = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let stage = root.join(format!("{hash}-{}-{serial}.tmp", std::process::id()));
    fs::create_dir(&stage).map_err(|e| err(&e))?;
    let result = (|| {
        for entry in &manifest.files {
            let mut file = archive.by_name(&entry.path).map_err(|e| err(&e))?;
            if file.size() != entry.size
                || file.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
            {
                return Err("Invalid ML Plasma archive entry".into());
            }
            let mut data = Vec::new();
            (&mut file)
                .take(entry.size + 1)
                .read_to_end(&mut data)
                .map_err(|e| err(&e))?;
            if data.len() as u64 != entry.size
                || format!("{:x}", Sha256::digest(&data)) != entry.sha256
            {
                return Err("ML Plasma payload checksum mismatch".into());
            }
            let path = stage.join(safe_path(&entry.path)?);
            fs::create_dir_all(path.parent().unwrap()).map_err(|e| err(&e))?;
            fs::write(&path, &data).map_err(|e| err(&e))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(
                    &path,
                    fs::Permissions::from_mode(file.unix_mode().unwrap_or(0o644) & 0o777),
                )
                .map_err(|e| err(&e))?;
            }
        }
        if !stage.join(FILE).exists() {
            return Err("ML Plasma native module is missing".into());
        }
        if let Err(e) = fs::rename(&stage, &dest) {
            // Another studio process can have completed the same extraction in parallel.
            if !verified(&dest, &manifest.files) {
                return Err(err(&e));
            }
        }
        Ok(dest.join(FILE))
    })();
    // Only this call's uniquely named, known staging directory is disposable.
    if stage.exists() {
        let _ = fs::remove_dir_all(&stage);
    }
    result
}

pub fn bundle_path() -> Result<PathBuf, String> {
    static PATH: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    PATH.get_or_init(|| {
        let root = os_paths::user_dir()
            .ok_or("AQUILA user directory is unavailable")?
            .join("Builtin Plugins")
            .join("ML Plasma");
        unpack(BYTES, &root)
    })
    .clone()
}

pub fn library() -> Result<std::sync::Arc<os_clap::Library>, String> {
    static LIB: OnceLock<Result<std::sync::Arc<os_clap::Library>, String>> = OnceLock::new();
    LIB.get_or_init(|| {
        let library = os_clap::Library::load(&bundle_path()?)?;
        if library.plugins.len() != 1
            || library.plugins[0].id != ID
            || library.plugins[0].name != NAME
            || library.plugins[0].vendor != "AQUILA"
        {
            return Err("ML Plasma payload has an unexpected instrument identity".into());
        }
        Ok(library)
    })
    .clone()
}

pub fn is_builtin(d: &os_host::Descriptor) -> bool {
    d.format == os_host::Format::Clap
        && d.id == ID
        && d.vendor == "AQUILA"
        && bundle_path().is_ok_and(|path| path == d.path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_reset_silence(plugin: &mut dyn os_host::Plugin, rate: u32) {
        plugin.reset();
        for block in 0..100 {
            let mut left = [0.0; 512];
            let mut right = [0.0; 512];
            plugin.process(rate, &mut left, &mut right);
            let energy: f64 = left
                .iter()
                .chain(&right)
                .map(|&x| f64::from(x).powi(2))
                .sum();
            assert!(
                energy < 1e-15,
                "reset left audio at block {block}: {energy}"
            );
        }
    }
    #[test]
    fn payload_paths_and_integrity() {
        for path in [
            "../escape",
            "/absolute",
            "ML Plasma-assets/../escape",
            "Licences/C:escape",
            "ML Plasma.clap\\outside",
        ] {
            assert!(safe_path(path).is_err(), "{path}");
        }
        let root = PathBuf::from(env!("OUT_DIR")).join("extract-test");
        let first = unpack(BYTES, &root).unwrap();
        assert!(first.exists());
        assert_eq!(unpack(BYTES, &root).unwrap(), first);
        let mut corrupt = BYTES.to_vec();
        corrupt.truncate(corrupt.len() / 2);
        assert!(unpack(&corrupt, &root).is_err());
    }

    #[test]
    fn stock_engine_audio_automation_and_recall() {
        use os_host::Library;
        let library = library().expect("bundled native instrument loads");
        assert_eq!(library.plugins.len(), 1);
        assert!(library.plugins[0].is_instrument());
        let instance = library.clone().create(ID).unwrap();
        let mut plugin = instance.lock().unwrap();
        assert!(plugin.has_editor());
        assert!(
            plugin.param_count() > 1000,
            "retain the complete Plasma parameter set"
        );
        let volume = (0..plugin.param_count())
            .filter_map(|i| plugin.param_info(i))
            .find(|p| p.name == "Main Volume")
            .expect("master level exposed for automation");
        assert!(plugin.set_param(volume.id, 0.37));
        plugin.idle();
        let saved = plugin.save_state().expect("project state");
        assert!(!saved.is_empty());
        assert!(plugin.activate(48000));
        let mut energy = 0.0f64;
        for block in 0..100 {
            let frames = [127, 256, 511, 1024][block % 4];
            let mut left = vec![0.0; frames];
            let mut right = vec![0.0; frames];
            if block == 0 {
                plugin.note_on(17, 48, 0.8);
                plugin.note_on(17, 55, 0.7);
                plugin.note_on(17, 60, 0.6);
            }
            if block == 50 {
                for note in [48, 55, 60] {
                    plugin.note_off(13, note);
                }
            }
            plugin.process(48000, &mut left, &mut right);
            assert!(
                left.iter().chain(&right).all(|x| x.is_finite()),
                "finite stereo output"
            );
            energy += left
                .iter()
                .chain(&right)
                .map(|&x| f64::from(x).powi(2))
                .sum::<f64>();
        }
        assert!(energy > 0.01, "synthetic chord produces audio: {energy}");
        assert_reset_silence(&mut *plugin, 48000);
        drop(plugin);
        // A second instance emulates closing and reopening a saved project.
        let reopened = library.create(ID).unwrap();
        let mut reopened = reopened.lock().unwrap();
        assert!(reopened.load_state(&saved));
        reopened.idle();
        assert!((reopened.param_value(volume.id).unwrap() - 0.37).abs() < 1e-5);
        assert!(
            reopened.activate(44100),
            "a project can reopen at a different sample rate"
        );
        reopened.note_on(0, 67, 0.8);
        let mut energy = 0.0f64;
        for _ in 0..16 {
            let mut left = [0.0; 256];
            let mut right = [0.0; 256];
            reopened.process(44100, &mut left, &mut right);
            energy += left
                .iter()
                .chain(&right)
                .map(|&x| f64::from(x).powi(2))
                .sum::<f64>();
        }
        assert!(energy > 0.01, "reopened patch still produces audio");
        reopened.note_off(0, 67);
        assert_reset_silence(&mut *reopened, 44100);
    }

    #[test]
    fn stock_reset_clears_held_notes_and_delay_tails() {
        use os_host::Library;
        let library = library().unwrap();
        // SAFETY: this is our checksum-verified payload built from the pinned source.
        let module =
            unsafe { os_host::load_module(&os_clap::module_file(&bundle_path().unwrap())) }
                .unwrap();
        assert!(
            unsafe {
                module.get::<unsafe extern "C" fn(i32) -> bool>(b"ml_plasma_editor_test_edit\0")
            }
            .is_err(),
            "editor test controls must not ship in the production payload"
        );
        for mode in 0..3 {
            let instance = library.clone().create(ID).unwrap();
            let mut p = instance.lock().unwrap();
            if mode != 0 {
                let mut state: serde_json::Value =
                    serde_json::from_slice(&p.save_state().unwrap()).unwrap();
                let find = |name: &str| {
                    (0..p.param_count())
                        .filter_map(|i| p.param_info(i))
                        .find(|x| x.name == name)
                        .unwrap()
                        .id as usize
                };
                let params = state["plugin"]["params"].as_array_mut().unwrap();
                params[find("FX 1 Type")] = serde_json::json!(5); // Plasma's Delay
                params[find("FX 1 On")] = serde_json::json!(1);
                let values = [
                    0.7, 0.7, 0.4, 0.0, 1.0, 0.35, 0.35, 0.0, 0.75, 0.5, 0.5, 0.5, 1.0, 0.0,
                ];
                for (i, value) in values.into_iter().enumerate() {
                    params[find(&format!("FX 1 Param {}", i + 1))] = serde_json::json!(value);
                }
                params[find("Env 1 Release")] = serde_json::json!(0);
                let mut model: serde_json::Value =
                    serde_json::from_str(state["plugin"]["data"].as_str().unwrap_or(""))
                        .unwrap_or_else(|_| serde_json::json!({"plasma": 1}));
                model["fx"] = serde_json::json!([{"slot": 0, "rack": 0}]);
                state["plugin"]["data"] = serde_json::json!(model.to_string());
                assert!(p.load_state(&serde_json::to_vec(&state).unwrap()));
            }
            assert!(p.activate(48000));
            p.reset(); // reset before the first processed block must allow fresh notes
            let before: serde_json::Value =
                serde_json::from_slice(&p.save_state().unwrap()).unwrap();
            p.note_on(0, 60, 0.8);
            let mut energy = 0.0f64;
            for _ in 0..100 {
                let mut left = [0.0; 512];
                let mut right = [0.0; 512];
                p.process(48000, &mut left, &mut right);
                energy += left
                    .iter()
                    .chain(&right)
                    .map(|&x| f64::from(x).powi(2))
                    .sum::<f64>();
            }
            assert!(energy > 0.01, "fixture {mode} must sound before reset");
            if mode == 2 {
                p.note_off(0, 60);
                let mut tail = 0.0f64;
                for _ in 0..8 {
                    let mut left = [0.0; 512];
                    let mut right = [0.0; 512];
                    p.process(48000, &mut left, &mut right);
                    tail += left
                        .iter()
                        .chain(&right)
                        .map(|&x| f64::from(x).powi(2))
                        .sum::<f64>();
                }
                assert!(tail > 1e-5, "delay must have a tail to exercise its reset");
            }
            assert_reset_silence(&mut *p, 48000);
            let after: serde_json::Value =
                serde_json::from_slice(&p.save_state().unwrap()).unwrap();
            assert_eq!(
                before["plugin"], after["plugin"],
                "reset preserves the patch"
            );
            p.note_on(0, 67, 0.8);
            let mut energy = 0.0f64;
            for _ in 0..100 {
                let mut left = [0.0; 512];
                let mut right = [0.0; 512];
                p.process(48000, &mut left, &mut right);
                energy += left
                    .iter()
                    .chain(&right)
                    .map(|&x| f64::from(x).powi(2))
                    .sum::<f64>();
            }
            assert!(energy > 0.01, "fresh notes sound after reset");
            assert_reset_silence(&mut *p, 48000);
        }
    }
}

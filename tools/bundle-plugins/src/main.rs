// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Lemon Studio: https://github.com/ItsAlphaNeon/Lemon-Studio

//! `cargo run -p bundle-plugins [-- --install]` builds one VST3 per plugin into
//! `<target>/bundled/<Name>.vst3`. The --install option copies only VST3 to
//! the user's platform VST3 folder. Existing user-installed CLAP binaries are
//! deliberately untouched to preserve sessions that reference CLAP identities.
//!
// spec: plugins/plugin-formats.md (bundles and install folders)

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

/// (cargo package, library file stem, display name).
const PLUGINS: &[(&str, &str, &str)] =
    &[("lemon-hall", "lemon_hall", "ML Hall"), ("lemon-tape", "lemon_tape", "ML Tape"), ("lemon-wave", "lemon_wave", "ML Wave"), ("lemon-sampler", "lemon_sampler", "ML Sampler"), ("lemon-parametric-eq-2", "lemon_parametric_eq_2", "ML EQ"), ("lemon-3x-osc", "lemon_3x_osc", "ML Trio"), ("lemon-limiter", "lemon_limiter", "ML Limiter"), ("lemon-reverb", "lemon_reverb", "ML Reverb"), ("lemon-delay", "lemon_delay", "ML Delay"), ("lemon-soft-clipper", "lemon_soft_clipper", "ML Soft Clipper"), ("lemon-fast-distortion", "lemon_fast_distortion", "ML Fast Distortion"), ("lemon-balance", "lemon_balance", "ML Balance"), ("lemon-stereo-enhancer", "lemon_stereo_enhancer", "ML Stereo Enhancer"), ("lemon-transient-processor", "lemon_transient_processor", "ML Transient Processor"), ("lemon-chorus", "lemon_chorus", "ML Chorus"), ("lemon-stereo-shaper", "lemon_stereo_shaper", "ML Stereo Shaper"), ("lemon-filter", "lemon_filter", "Lemon Filter"), ("citrus", "citrus", "ML Citrus"), ("smith", "smith", "Smith"), ("tesla", "tesla", "ML Tesla"), ("pulp-filter", "pulp_filter", "ML Pulp Filter"), ("press", "press", "ML Press"), ("lemon-flanger", "lemon_flanger", "Lemon Flanger"), ("lemon-phaser", "lemon_phaser", "Lemon Phaser"), ("lemon-peak-controller", "lemon_peak_controller", "Lemon Peak Controller"), ("lemon-formula-controller", "lemon_formula_controller", "Lemon Formula Controller"), ("lemon-slicer", "lemon_slicer", "Lemon Slicer"), ("lemon-vocoder", "lemon_vocoder", "Lemon Vocoder"), ("lemon-drum-sampler", "lemon_drum_sampler", "Lemon Drum Sampler"), ("lemon-drop", "lemon_drop", "Lemon Drop"), ("gross-beat", "gross_beat", "Skell Beat"), ("fruity-compressor", "fruity_compressor", "Skell Compressor"), ("lynx-tune", "lynx_tune", "Skell Tune")];
/// The plugins' earlier display names: an install removes bundles left under them (same plugin ids, so a DAW would
/// list each plugin twice).
const OLD_NAMES: &[&str] = &["Lemon Sampler","Lemon Trio","Lemon Reverb","Lemon Delay","Lemon Limiter","Lemon Soft Clipper","Lemon Fast Distortion","Lemon Balance","Lemon Stereo Enhancer","Lemon Transient Processor","Lemon Chorus","Lemon Stereo Shaper","Lemon Tape","Lemon Hall","Lemon Chop","Lemon Wave","Lemon Parametric EQ","Citrus","Tesla","Pulp Filter","Press","Lemon Parametric EQ 2", "Lemon 3x Osc", "Open Sampler", "Open Parametric EQ 2", "Open 3x Osc"];

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let to = dst.join(e.file_name());
        if e.file_type()?.is_dir() { copy_dir(&e.path(), &to)? } else { std::fs::copy(e.path(), to).map(|_| ())? }
    }
    Ok(())
}


fn stage_distribution_metadata(root: &Path, out: &Path) {
    // Copies are part of the distributable archive, not an installed plugin:
    // do not alter host-visible VST3 identities, presets or existing CLAP files.
    let documents = [
        ("LICENSE", "LICENSE-GPL-3.0-or-later.txt"),
        ("THIRD-PARTY-NOTICES.txt", "THIRD-PARTY-NOTICES.txt"),
        ("CREDITS_AND_LICENSES.md", "CREDITS_AND_LICENSES.md"),
        ("plugins/gross-beat/vendor/LYNX-UI-NOTICE.md", "SKELL-BEAT-LYNX-UI-NOTICE.md"),
    ];
    for (from, to) in documents {
        std::fs::copy(root.join(from), out.join(to))
            .unwrap_or_else(|err| panic!("required distribution notice {from}: {err}"));
    }
    // LYNX-derived source is AGPL, independently of the GPL root license.
    // Each archive carries its affected plugin's complete AGPL license.
    let agpl = [
        ("gross-beat", "Skell-Beat"),
        ("fruity-compressor", "Skell-Compressor"),
        ("lemon-limiter", "ML-Limiter"),
        ("lemon-delay", "ML-Delay"),
        ("lemon-reverb", "ML-Reverb"),
    ];
    let licenses = out.join("licenses");
    std::fs::create_dir_all(&licenses).expect("create plugin license folder");
    for (pkg, display) in agpl {
        let from = root.join("plugins").join(pkg).join("LICENSE-AGPL-3.0.txt");
        let to = licenses.join(format!("{display}-AGPL-3.0.txt"));
        std::fs::copy(&from, &to)
            .unwrap_or_else(|err| panic!("required AGPL license {}: {err}", from.display()));
    }
    let revision = std::env::var("SKELL_SOURCE_SHA").ok()
        .filter(|sha| sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()))
        .or_else(|| Command::new("git").args(["rev-parse", "HEAD"]).current_dir(root)
            .output().ok().filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|o| o.trim().to_owned()));
    let revision = revision.unwrap_or_else(|| "UNKNOWN-SOURCE-REVISION".to_owned());
    let readme = format!(
        "SKELL STUDIO | WINDOWS/MAC/LINUX VST3 ENGINEERING BUNDLE\n\
         Source revision: {revision}\n\
         Repository: https://github.com/skellyruno/skell-studio\n\
         Exact matching source: https://github.com/skellyruno/skell-studio/tree/{revision}\n\
         Format: VST3 ONLY. Legacy installed CLAP plugins are not deleted; they have distinct host identities.\n\
         Scope: engineering validation, not an approved stable/proprietary release.\n\
         The top-level LICENSE-GPL-3.0-or-later.txt, THIRD-PARTY-NOTICES.txt, CREDITS_AND_LICENSES.md\n\
         and licenses/*-AGPL-3.0.txt are required records; do not remove on redistribution.\n\
         LYNX engine and editor adaptations have AGPL-3.0-only provenance.\n\
         Source availability, attribution, dependency and commercial distribution compliance\n\
         require legal verification before publication. A VST3 file does not waive these obligations.\n"
    );
    std::fs::write(out.join("SOURCE_AND_LICENSE_README.txt"), readme)
        .expect("write source and license provenance");
}

fn main() {
    let install = std::env::args().any(|a| a == "--install");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("repository root");
    let target = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|| root.join("target"));
    let target = if target.is_relative() { root.join(target) } else { target };
    let out = target.join("bundled");
    std::fs::create_dir_all(&out).expect("bundled folder");
    // A reused build directory must not silently ship stale CLAP bundles.
    for entry in std::fs::read_dir(&out).expect("read bundled folder") {
        let path = entry.expect("bundle entry").path();
        if path.extension().is_some_and(|ext| ext == "clap") {
            if path.is_dir() { std::fs::remove_dir_all(&path).expect("remove obsolete CLAP bundle"); }
            else { std::fs::remove_file(&path).expect("remove obsolete CLAP module"); }
        }
    }
    // A build folder of their own: Lemon Studio links some plugins with their `linked` feature, and a plugin library's
    // file names carry no hash, so the app's build and this one would each overwrite the other's and rebuild every time.
    let build = target.join("plugins");
    for (pkg, stem, name) in PLUGINS {
        let ok = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "--locked", "--release", "--lib", "-p", pkg])
            .current_dir(&root)
            .env("CARGO_TARGET_DIR", &build)
            .status()
            .expect("cargo")
            .success();
        assert!(ok, "build of {pkg} failed");
        let lib = build.join("release").join(library_file(stem));
        let built = std::fs::metadata(&lib).and_then(|m| m.modified()).expect("library time stamp");
        let vst3 = out.join(format!("{name}.vst3"));
        if cfg!(target_os = "macos") {
            mac_bundle(&lib, &vst3, pkg, name);
        } else {
            let arch = vst3.join("Contents").join(BUNDLE_ARCH);
            std::fs::create_dir_all(&arch).expect("vst3 bundle");
            std::fs::copy(&lib, arch.join(bundle_module(name))).expect("copy vst3");
        }
        stamp(&module(&vst3, name), built);
        println!("{}", vst3.display());
        if install {
            let vst3_dir = install_dir();
            // Only migrate legacy VST3 names. Never remove an installed CLAP that
            // an existing DAW project might still reference.
            for old in OLD_NAMES {
                let path = vst3_dir.join(format!("{old}.vst3"));
                let _ = if path.is_dir() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
            }
            let to = vst3_dir.join(format!("{name}.vst3"));
            if same_build(&module(&vst3, name), &module(&to, name)) {
                println!("{name} is installed and up to date");
                continue;
            }
            let _ = if to.is_dir() { std::fs::remove_dir_all(&to) } else { std::fs::remove_file(&to) };
            if vst3.is_dir() { copy_dir(&vst3, &to).expect("install VST3 bundle"); }
            else {
                std::fs::create_dir_all(to.parent().expect("VST3 folder")).expect("VST3 folder");
                std::fs::copy(&vst3, &to).expect("install VST3 plugin");
            }
            stamp(&module(&to, name), built);
            println!("installed VST3 to {}", vst3_dir.display());
        }
    }
    stage_distribution_metadata(&root, &out);
}

/// VST3 module inside the platform bundle.
fn module(bundle: &Path, name: &str) -> PathBuf {
    bundle.join("Contents").join(BUNDLE_ARCH).join(bundle_module(name))
}

/// Gives a module the time stamp of the library it was made from, whatever the copy gave it: copies of one build then
/// share it, which tells an install whether its copy is current.
fn stamp(module: &Path, built: SystemTime) {
    std::fs::File::options().write(true).open(module).and_then(|f| f.set_modified(built)).expect("module time stamp");
}

/// Whether the module `copy` is the same build as `module`: the same size and time stamp.
fn same_build(module: &Path, copy: &Path) -> bool {
    let meta = |p: &Path| std::fs::metadata(p).and_then(|m| Ok((m.len(), m.modified()?))).ok();
    meta(copy).is_some_and(|c| meta(module) == Some(c))
}

/// The bundle folder holding the module: `Contents/<architecture>-<platform>` (VST 3 bundle layout), `Contents/MacOS`
/// on macOS.
const BUNDLE_ARCH: &str = if cfg!(target_os = "macos") {
    "MacOS"
} else if cfg!(windows) {
    if cfg!(target_arch = "aarch64") { "arm64-win" } else { "x86_64-win" }
} else if cfg!(target_arch = "aarch64") {
    "aarch64-linux"
} else {
    "x86_64-linux"
};

/// The shared library cargo writes for a `cdylib` named `stem`.
fn library_file(stem: &str) -> String {
    format!("{}{stem}{}", std::env::consts::DLL_PREFIX, std::env::consts::DLL_SUFFIX)
}

/// The module's name inside the bundle: `<Name>.vst3` on Windows, `<Name>.so` on Linux, `<Name>` on macOS.
fn bundle_module(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.vst3")
    } else if cfg!(target_os = "macos") {
        name.to_owned()
    } else {
        format!("{name}{}", std::env::consts::DLL_SUFFIX)
    }
}

/// A macOS VST3 bundle at `bundle` (written afresh): `Contents/Info.plist`, `Contents/PkgInfo`
/// and the module in `Contents/MacOS`, ad-hoc signed (Apple silicon loads no unsigned code).
fn mac_bundle(lib: &Path, bundle: &Path, pkg: &str, name: &str) {
    if bundle.is_dir() {
        std::fs::remove_dir_all(bundle).expect("old bundle");
    } else if bundle.exists() {
        std::fs::remove_file(bundle).expect("old bundle");
    }
    let contents = bundle.join("Contents");
    let arch = contents.join(BUNDLE_ARCH);
    std::fs::create_dir_all(&arch).expect("bundle folder");
    std::fs::copy(lib, arch.join(bundle_module(name))).expect("copy module");
    let format = bundle.extension().and_then(|e| e.to_str()).unwrap_or_default();
    std::fs::write(contents.join("PkgInfo"), "BNDL????").expect("PkgInfo");
    std::fs::write(contents.join("Info.plist"), info_plist(&format!("org.lemonstudio.{pkg}.{format}"), name)).expect("Info.plist");
    let signed = Command::new("codesign").args(["--force", "--sign", "-"]).arg(bundle).status().is_ok_and(|s| s.success());
    assert!(signed, "could not sign {}", bundle.display());
}

/// A plugin bundle's `Info.plist`.
fn info_plist(id: &str, name: &str) -> String {
    let version = env!("CARGO_PKG_VERSION");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>English</string>
    <key>CFBundleExecutable</key>
    <string>{name}</string>
    <key>CFBundleIdentifier</key>
    <string>{id}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>{name}</string>
    <key>CFBundleDisplayName</key>
    <string>{name}</string>
    <key>CFBundlePackageType</key>
    <string>BNDL</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>CFBundleShortVersionString</key>
    <string>{version}</string>
    <key>CFBundleVersion</key>
    <string>{version}</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
"#
    )
}

/// VST3 only: keep existing installed CLAP copies untouched for project recall.
fn install_dir() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(std::env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA"))
            .join("Programs").join("Common").join("VST3")
    } else if cfg!(target_os = "macos") {
        PathBuf::from(std::env::var_os("HOME").expect("HOME"))
            .join("Library").join("Audio").join("Plug-Ins").join("VST3")
    } else {
        PathBuf::from(std::env::var_os("HOME").expect("HOME")).join(".vst3")
    }
}

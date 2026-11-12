// SPDX-License-Identifier: GPL-3.0-only
use std::{env, path::PathBuf, process::Command};
fn main() {
    for path in [
        "vendor/gridline",
        "build_native.py",
        "README.md",
        "UPSTREAM.json",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    for key in ["PYTHON", "CMAKE", "CMAKE_GENERATOR", "ML_PLASMA_NATIVE_DIR"] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let bundle_key = format!(
        "ML_PLASMA_BUNDLE_{}_{}",
        os.to_uppercase(),
        arch.to_uppercase()
    );
    println!("cargo:rerun-if-env-changed={bundle_key}");
    let bundle = env::var_os(&bundle_key);
    assert!(
        env::var("HOST").unwrap() == env::var("TARGET").unwrap() || bundle.is_some(),
        "Cross builds need a matching native ML Plasma payload in {bundle_key}; a Windows engine must never enter a Linux/Mac update"
    );
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    // Reuse one native cache per Cargo target, across build-script fingerprints.
    let native = env::var_os("ML_PLASMA_NATIVE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            out.ancestors()
                .nth(4)
                .expect("Cargo target directory")
                .join("ml-plasma-native")
        });
    let mut command = Command::new(env::var_os("PYTHON").unwrap_or_else(|| "python".into()));
    command
        .arg("build_native.py")
        .arg("--output")
        .arg(out.join("plasma.zip"))
        .arg("--build-dir")
        .arg(native)
        .arg("--target-os")
        .arg(os)
        .arg("--target-arch")
        .arg(arch);
    if let Some(bundle) = bundle {
        println!(
            "cargo:rerun-if-changed={}",
            PathBuf::from(&bundle).display()
        );
        command.arg("--bundle").arg(bundle);
    }
    let result = command
        .status()
        .expect("Python is required to build the bundled ML Plasma engine");
    assert!(
        result.success(),
        "ML Plasma native build failed; a studio update must not omit its stock instrument"
    );
}

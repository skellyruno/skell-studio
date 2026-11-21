fn main() {
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let vendor = manifest.join("vendor/lynx");
    for source in ["core/ParametricEqFl.h","core/ParametricEqFl.cpp",
        "core/ParametricEqLinear.h","core/ParametricEqLinear.cpp",
        "core/Simd.h","core/CpuFeatures.h","core/Song.h","core/bridge.cpp"] {
        println!("cargo:rerun-if-changed={}",vendor.join(source).display());
    }
    if std::env::var_os("CARGO_FEATURE_LINKED").is_some() {
        // The in-process Studio build retains its existing GPL-compatible EQ implementation.
        return;
    }
    cc::Build::new().cpp(true).std("c++20").include(&vendor)
        .file(vendor.join("core/ParametricEqFl.cpp"))
        .file(vendor.join("core/ParametricEqLinear.cpp"))
        .file(vendor.join("core/bridge.cpp"))
        .flag_if_supported("/EHsc").flag_if_supported("/fp:precise")
        .flag_if_supported("-fno-fast-math").flag_if_supported("-ffp-contract=off")
        .compile("skell_lynx_parametric_eq2");
}

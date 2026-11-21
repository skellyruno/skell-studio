fn main() {
    let vendor = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("vendor");
    println!("cargo:rerun-if-changed={}", vendor.display());
    cc::Build::new()
        .cpp(true)
        .std("c++20")
        .include(&vendor)
        .file(vendor.join("core/FruityCompressor.cpp"))
        .file(vendor.join("core/bridge.cpp"))
        .flag_if_supported("/EHsc")
        .flag_if_supported("/fp:precise")
        .flag_if_supported("-fno-fast-math")
        .flag_if_supported("-ffp-contract=off")
        .compile("skell_fruity_compressor");
}

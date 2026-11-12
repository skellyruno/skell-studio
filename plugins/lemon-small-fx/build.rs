fn main() {
    println!("cargo:rerun-if-changed=vendor");
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .file("vendor/tape_adapter.cpp")
        .warnings(false)
        .compile("lemon_tape");
    let mut hall = cc::Build::new();
    hall.cpp(true)
        .std("c++14")
        .include("vendor/dragonfly")
        .define("LIBFV3_DOUBLE", None)
        .warnings(false)
        .file("vendor/hall_adapter.cpp");
    for name in [
        "allpass",
        "biquad",
        "comb",
        "delay",
        "delayline",
        "earlyref",
        "efilter",
        "revbase",
        "slot",
        "utils",
        "zrev",
        "zrev2",
    ] {
        hall.file(format!("vendor/dragonfly/freeverb/{name}.cpp"));
    }
    hall.compile("lemon_hall");
}

LYNX SHARE 0.1.30 Fruity Delay 3 engine integration
==========================================================
Pinned upstream revision: 88ec555ea2bce9714c92707cc06a5abfd317c900
Upstream repository: https://github.com/Ka1-S/LYNX-SHARE
The vendored implementation adapts LYNX Fruity Delay 3 DSP into vendor/core/FruityDelay3Engine.cpp and exposes it through vendor/core/bridge.cpp. The DAW-specific effect wrapper is not included.
The C ABI bridge and Rust adapter are Skell modifications.
No Image-Line binaries, factory presets, samples or artwork are included.
See LICENSE-AGPL-3.0.txt for the full upstream AGPLv3 terms.

LYNX SHARE 0.1.30 Fruity Reeverb 2 engine integration
==========================================================
Pinned upstream revision: 88ec555ea2bce9714c92707cc06a5abfd317c900
Upstream repository: https://github.com/Ka1-S/LYNX-SHARE
The vendored engine-only translation unit is adapted from src/core/Reeverb2.cpp through the reeverb2 namespace. It excludes the DAW-specific effect factory and Fruity Reeverb 1 implementation.
The C ABI bridge and Rust adapter are Skell modifications.
No Image-Line binaries, factory presets, samples or artwork are included.
See LICENSE-AGPL-3.0.txt for the full upstream AGPLv3 terms.

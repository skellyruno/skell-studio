LYNX SHARE 0.1.30 Fruity Limiter engine integration
=======================================================
Pinned upstream revision: 88ec555ea2bce9714c92707cc06a5abfd317c900
Upstream repository: https://github.com/Ka1-S/LYNX-SHARE
Adapted source: src/core/FruityDynamics.cpp (LimiterEngine and the preceding DSP helpers only).
The DAW-specific effect wrapper and Fruity Compressor implementation are excluded.
The C ABI bridge and Rust host adapter are Skell modifications.
The 18 raw control fields, exact-ceiling option and sidechain selection remain in the existing ML Limiter state image.
See LICENSE-AGPL-3.0.txt for the full upstream AGPLv3 terms.

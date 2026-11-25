# Skell Studio VST3-only distribution and Skell Beat LYNX UI port

## Distribution decision

Skell Studio now **packages and installs VST3 only** via `cargo run --locked --release -p bundle-plugins`. Windows Actions package `SkellStudio-Windows-x64-VST3-Plugin-Suite` validates 33 VST3s, writes per-module SHA-256 checksums and confirms zero `.clap` modules in the release directory. CLAP host support and legacy `nih_export_clap` entry points stay in source: saved sessions referring to existing CLAP identities should remain loadable. The installer never deletes previously installed CLAP copies. The plugin formats are not interchangeable host identities.

## Skell Beat interface

New layout is a Rust native port of LYNX Gross Beat's 836×563 JUCE editor structure: separate 4×9 Time and Volume banks (all 72 slots simultaneously visible), per-slot envelope previews and names, green/blue bank colors, working snap selector, large beat/level envelope, point add/remove/curve/tension editing, MIDI toggle and the existing eight automatable host controls. Native editor now measures 1100×720.

The batched single-parse thumbnail bridge is editor-thread only. Clicking a bank cell changes the existing Time Slot / Volume Slot automatable parameter; this is not merely a visual tab. State encoding and audio engine remain unchanged. See `plugins/gross-beat/vendor/LYNX-UI-NOTICE.md`.

## Verification required

- Windows `cargo test --locked -p gross-beat` including 72 previews, grid hit tests, 192 queued state swaps, and host slot selection
- Release `cargo run --locked --release -p bundle-plugins`, all 33 VST3 modules, exact-commit checksums, and zero CLAP outputs
- Open the real updated Skell Beat native screenshot; fix clipped controls, overlay, scaling defects, and capture again
- FL Studio real project save/reopen, external DAW scanning/editor lifecycle, audio A/B, CPU/latency, AGPL review

Keep PR #22 draft and `main` unchanged until release validation.

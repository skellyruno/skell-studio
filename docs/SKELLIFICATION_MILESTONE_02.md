# Skell Studio — Skellification milestone 02

Date: 2026-11-12

## Source code delivered (commit b76fcec862976055e8a6335dec6335f5b2b09a51)

### Playlist and track headers
- Replaced the plain header fill in Skell skin with reusable machine-panel rendering.
- Added compact numbered track labels, folder indicators, selected rail and track colour markers with guarded size thresholds.
- Adjusted pattern/audio waveform/selection/mute clip palette to matte green-black; no project colour values overwritten.
- Kept original hit regions for mute, solo, automation, clip trim, groups, warp, vocal controls and resize.

### Piano roll
- Added green-highlighted octave roots in note grid.
- Added glass highlight and selected outline to MIDI notes; kept existing velocity/slide/portamento and drag handlers.
- Restyled white/black piano keyboard from blue/grey to desaturated ivory and deep green-black.
- Added a slim accent guide to event/control lane; does not alter data or MIDI operations.

### Mixer
- Skell hardware panel treatment in insert strips and FX inspector.
- Dark inset meter wells and a readable green selection system.
- Weighted two-tone metal fader caps with grip and selected signal line.
- FX slot cards with status rail for loaded effects, preserved mute/solo/mix/drag hit regions.
- Added small vector cross-head screw in inspector, no image textures.

### Original icons and art
- `assets/skell-studio-mark.svg` is an original, editable skull vector source.
- `tools/branding/generate_skell_icons.py` produces matching PNG and 16–256 px multiresolution Windows ICO images with Python standard library only.
- Windows binary resource and Linux launcher now reference first-party Skell Studio assets; original credits and third-party asset licenses remain.
- The skull is provisional; future shared SkellCore logo should replace it consistently.

### Build & test gates
- Fixed hard-pinned Visual Studio 2022 CMake generator in `plugins/ml-plasma/build_native.py` to follow installed MSVC.
- Fixed out-of-date theme tests; verified core Linux theme/profile tests passed.
- GitHub Actions workflow `skellification-v2.yml` applied the UI changes with anchor verification and passed Linux source/theme checks, then committed them.
- Windows compile and real UI/audio/host tests **must not be claimed passed** unless independently confirmed by CI/results. There is no verified `.exe` from this milestone yet.

## Remaining work (not done)

- Full Windows compilation and actual UI screenshot reviews at 100/125/150% DPI.
- Explicit mouse/keyboard interaction tests and audio routing/MIDI/VST3 smoke tests.
- Broader original Skell design: workspace navigation, plugin browser, window title system, 3D knob family, original default presets and full graphic system.
- Audit inherited logo images, icons in plugins, bundled samples and release packaging before deleting assets.
- Review 104 upstream-reported failing tests; do not confuse historical baseline with fresh results.
- GPL and third-party rights audit before proprietary release.

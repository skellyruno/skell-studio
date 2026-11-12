# Skell Studio — Skellification checkpoint 03

Date: 2026-11-12
Repo: https://github.com/skellyruno/skell-studio

## Implemented on main

- **Reusable visual system** in `crates/os-app/src/views/skell_chrome.rs`: consistent hardware-style tool/button face, selection underline, hover edge, and dark title-bar trim.
- **Unified title bands** for `Playlist`, `PianoRoll`, and `Mixer` in `crates/os-app/src/views/mod.rs`. This is paint-only; window drag handles, caption buttons and geometry are unchanged.
- **Shared tool paint** in the arrangement toolbar, piano-roll editing toolbar and mixer view selector. All use `skell_chrome::tool` for the Skell skin while retaining their existing action handlers and UI hit rectangles. Original non-Skell, vintage and custom styles are retained.
- **Labeled Skell workspace navigator** in `views/toolbar.rs`: ARR / KEYS / RACK / MIX / FILES (plus PROJ / PLUG / TAP / TOUCH). The labels and icons share the original 30×33 clickable regions; keyboard shortcuts, per-user visibility and overflow behavior are unchanged.
- Earlier v2 visual changes remain: hardware track headers, piano-roll notes/octave guides, mixer faders/meter wells/FX panels, original vector skull and generated native application icons.

Source checkpoints:
- Unified editor controls: `ade0fd3a3e499d8a5f14fbd71b850eef344ffe73`.
- Labeled workspace navigation: `cc7a0cad2c3a0f6b677cb42e798f952bb87668ba`.

## Confirmed verification

- GitHub Actions **Skell Studio UI v3 - unified controls**: passed anchored source migration, theme/path unit tests, and committed the changes.
- GitHub Actions **Skell Studio workspace navigation - verify and commit**: passed protected patch and theme/path unit tests, then committed the toolbar.
- Earlier v2 source integrity and palette tests: passed.

## Not yet verified

- The long-running Windows `cargo check -p os-app --bin skell-studio` jobs have **not** produced a confirmed successful compilation at this checkpoint.
- No checked-in Windows `.exe`, live audio device run, MIDI keyboard test, VST3 hosting smoke test, actual raster screenshot review, DPI or UI mouse-hit regression run has been confirmed.
- Some inherited ML/ADE/Lemon internal names, icons and legal notices remain; these are either compatibility-sensitive or still need dependency and license audits. Do not delete blindly.
- DSP/engine code was not rewritten for this visual checkpoint, and current inherited failing full-suite tests have not been triaged.

## Next engineering gate

1. Obtain a clean Windows compilation without discarding bundled instruments or disabling license checks.
2. Exercise the existing headless toolbar hit-testing tests at 800–2400 px and multiple UI scales, then capture real Windows UI screenshots.
3. Run audio playback/record/seek, project save/reopen, export, MIDI and VST3 tests.
4. Iterate window geometry and user workflow on the basis of verified screenshots and listening tests.
5. Replace additional inherited packaging assets only after confirming no runtime/build references remain.

## License

This code remains a GPL-derived DAW. Do not misrepresent upstream copyright or package it as closed source without an appropriate rights strategy.

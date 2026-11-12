# Skell Studio — Skellification milestone 01

Date: 2026-11-12
Status: Source changes pushed to `main`. Windows compilation/audio verification pending.

## Implemented in the real DAW

- Matte-black/neon-green design applied to centralized `os-theme::design::lemon` palette: the **display** name is now "Skell" but the serialized theme key stays **Lemon** so existing presets still load.
- Updated dozens of surface/contrast roles in the toolbar, playlist, piano roll, mixer, browser, floating windows, controls and note colors; red record/clipping status remains visually distinct.
- Native vector geometric skull replaces the old ML toolbar monogram (`crates/os-app/src/icons.rs`); it is **provisional** pending the final SkellCore official shared skull.
- Splash is now code-rendered and animated, with SKELL (white), STUDIO (green), skull, a loading meter, and "CREATE WITHOUT LIMITS"; the dead ML splash PNG was removed.
- First-run compact toolbar layout centers transport, record, navigation and core controls while retaining older layouts and saved toolbar settings.
- Window/app shell, About, user guide, stem separation, AI control, installer metadata and Windows app binary target now use "Skell Studio".
- Windows binary target is `skell-studio.exe` (source is NOT a compiled executable).
- Dedicated `Skell Studio` app preferences folder; no automatic migration or deletion of upstream settings.
- In-app Skelly website and first-party GitHub/source/bug links; upstream tutorials remain attributed and accessible until replaced with Skell documentation.
- Previous upstream third-party auto-updater remains disabled. Original GNU GPL and all source attributions retained.
- Added brand/legacy theme unit checks and automated CI to test core theme/path packages plus Windows compilation.

## What remains / known risks

- Full Windows compile, UI rendering verification on multiple DPI sizes, audio device tests, recording and export smoke tests still need to pass. The initial merge was known to have **104 upstream test failures** (not newly verified).
- Many internal modules and comments retain stable Lemon/ML identifiers; these are deliberately NOT blindly renamed because changing persistent formats and plugin IDs can break sessions.
- The executable resource still points to inherited `.ico` assets: design and verify an original Windows icon before distribution. The runtime toolbar and startup skull have already changed.
- More UI redesign is needed: improve playlist track headers, piano-roll density, mixer faders, window/chrome hierarchy and plugin-browser flow, rather than merely re-tinting everything.
- A commercial closed-source release is **not** automatically permitted: the imported DAW contains GPL-licensed code and third-party assets that need a rights audit.
- No official production release, installer, code signing or app update channel exists yet.

## Safety guardrails

Keep MIDI, VST3, project file formats, plugin IDs, disk paths and DSP APIs compatible until regression-tested. Do not remove usable audio features merely to decrease source size. Do not ship the upstream product icon, update channel or commercial branding.

## How to verify

```powershell
# Windows with Rust/Cargo, Visual Studio C++ Build Tools and LLVM installed
.\scripts\build-windows.ps1
# In GitHub: Actions -> Skell Studio — branding and engine compile gates
```

The development milestone is complete at source-code level only. The next gate is a clean Windows build and audio/UI smoke tests, then the major workflow/layout redesign.

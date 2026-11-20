# Skell Studio

**Skell Studio** is Skelly's work-in-progress native digital audio workstation, based on two user-supplied open-source DAW source snapshots and undergoing a ground-up product/visual transformation.

## Project status — November 15, 2026

- Unified DAW source imported and verified from pinned ADE Studio + ML Studio snapshots.
- ADE already integrated ML and Lemon Studio components; no duplicate audio engine was added. Nine ML-only files are in `reference/ml-only/`.
- Skell visual pass committed: matte black with neon green accent across the playlist/piano roll/mixer/browser; code-drawn skull; redrawn startup splash; compact toolbar; labeled workspace navigation; new app identity and independent preferences. Built-in themes include **Ember** (black chrome, FL-style orange), **Skell Green**, **Steel** and **Bone** (light); the **Console** skin keeps the measured FL-style look.
- Windows target is **`skell-studio.exe`**. Per the [verified checkpoint](docs/SKELL_VERIFIED_UPGRADE_CHECKPOINT.md), the Windows compile gate passed in CI on an earlier commit; **no installer or runnable executable has been tested**, and nothing here has been checked on real audio/MIDI hardware.
- Inherited updater startup and old release workflows disabled.
- GPL license, credits and large third-party notice list retained; see [credits](CREDITS_AND_LICENSES.md).

## Next engineering gates

1. Complete [GitHub Actions source checks](https://github.com/skellyruno/skell-studio/actions/workflows/skell-studio-verify.yml), including Windows compilation.
2. Test live audio, MIDI, VST3 hosting, timeline, saving/reopening, rendering/export and device changes.
3. Triage 104 upstream-reported test failures and distinguish new regressions.
4. Iterate full Skell Studio UI layout, piano roll, mixer, project browser, plugin workflow and original app icon.
5. Integrate SkellCore/SkellForge interfaces carefully, without changing persistent formats or plugin identifiers without compatibility migration.

## Build on Windows

Install Rust/Cargo, Visual Studio C++ Build Tools/SDK and LLVM, then:

```powershell
.\scripts\build-windows.ps1
```

Or use the manual [Windows GitHub Action](https://github.com/skellyruno/skell-studio/actions/workflows/windows-manual-build.yml). No app binary is claimed until compile and real hardware checks pass.

## Documentation

- [Skellification milestone 01](docs/SKELLIFICATION_MILESTONE_01.md)
- [Source merge baseline](docs/MERGE_BASELINE.md)
- [Integration roadmap](docs/SKELL_STUDIO_INTEGRATION_ROADMAP.md)
- [Verified upgrade checkpoint](docs/SKELL_VERIFIED_UPGRADE_CHECKPOINT.md)
- [Milestone 05: audit, rebrand cleanup and Ember theme](docs/SKELL_STUDIO_MILESTONE_05.md)
- [Credits and licenses](CREDITS_AND_LICENSES.md)
- [Original ADE handoff](HANDOFF.md)

**Legal:** The inherited DAW is GPL-licensed. Preserve the source history and third-party licenses; rebranding does not make the inherited code proprietary.

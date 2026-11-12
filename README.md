# Skell Studio — combined DAW source foundation (v0.1)

**Source successfully imported to GitHub on November 12, 2026.**

This repository contains a unified DAW engineering baseline reconstructed from the user's exact uploaded `ADE-Studio-michael-lane.zip` and `ML-Studio-michael-lane.zip` snapshots. The import was performed by [a SHA-verified GitHub Actions workflow](https://github.com/skellyruno/skell-studio/actions/workflows/import-baseline.yml), which checked both archives' complete Git tree fingerprints before committing the code.

## Source and merger

ADE Studio had already integrated ML Studio and newer Lemon Studio changes. Therefore, ADE is used as the **active compiled baseline** rather than duplicating both DAW engines. The nine files unique to the earlier ML snapshot are retained in `reference/ml-only/` for later analysis.

- ADE source fingerprint: `f84bf2b0475a776c90984acd3720d57b7c9a95a6` (9,641 files).
- ML source fingerprint: `36c6447167313157311e8f98c310f65750561e29` (9,364 files).
- Original inherited release workflows have been preserved as **disabled references**, not runnable releases.
- Original third-party automatic updater is disabled in the development baseline.

Read [merge baseline](docs/MERGE_BASELINE.md), [integration roadmap](docs/SKELL_STUDIO_INTEGRATION_ROADMAP.md), and [credits/licensing](CREDITS_AND_LICENSES.md).

## Current engine

- Rust Cargo workspace with native Windows desktop DAW app
- Playlist, mixer, piano roll, MIDI, recording and project handling
- Audio engine in `crates/os-engine`; plugin hosts including VST3 support
- Binary name: `lemonstudio.exe` until the next Skell branding stage
- SkellCore/SkellForge interoperability **not integrated yet**

**Important:** This is uploaded *source code*, not a newly compiled, fully verified Windows DAW. The inherited ADE handoff reported 2,968 passing tests, 104 failing tests and 119 ignored tests. Those numbers are historical, not fresh verification.

## Windows build

Install Rust/Cargo, Visual Studio C++ build tools/SDK and LLVM/Clang, then from the repository root run:

```powershell
.\scripts\build-windows.ps1
```

Or use the [manual Windows build workflow](https://github.com/skellyruno/skell-studio/actions/workflows/windows-manual-build.yml) under Actions. No binary distribution is implied until compilation and audio smoke tests pass.

## License and safety

This project derives from GPL-3.0-or-later upstream software; preserve original authors' copyright notices and third-party licenses. It cannot simply be sold as closed-source proprietary code. Conduct a complete dependency/asset review before any commercial release.

## Immediate priorities

1. Run and fix Windows compile; run engine tests.
2. Validate playback, recording, project save/load, export and VST3 hosting on real hardware.
3. Investigate baseline test failures and establish safety/performance metrics.
4. Create the distinct matte-black / neon-green Skell Studio UI with shared skull assets.
5. Integrate SkellCore, SkellForge and the wider SkellWorld ecosystem deliberately, not by copying incompatible build trees.

This repo is a **working development starting point**, not a finished product.

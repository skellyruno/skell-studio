# Skell Studio ML-first source migration

The user chose the newer **ML Studio** codebase as the primary baseline. The legacy Skell engine and UI remain preserved on `main` as a rollback reference; ML engine/DSP, mixer scheduling, recording, notation, visualizer, mix assistant, voice-assistant and related assets should be moved as one coherent codebase rather than cherry-picked into structurally incompatible modules.

## Source provenance

Source package generated from the user's **`ML-Studio-michael-lane (1)(1).zip`**, using ML's newer modules (mixer graph, device output graph, recording merge, timeline, assistant, visualizer, score/notation), with Skell product branding and neon-green theme. The full source archive was delivered in the conversation as `SkellStudio_ML_First_Full_Source_2026-11-15.zip`. The archive is not stored in this GitHub repository and this draft PR **does not claim the full migration is already on GitHub**.

## Stage the full source on a NEW local Git feature branch

Save and extract the delivered ZIP, or pass its path to the staged loader:

```powershell
.\scripts\stage_ml_first_source.ps1 -SourceZip "C:\Users\you\Downloads\SkellStudio_ML_First_Full_Source_2026-11-15.zip"
```

The script checks a clean Git checkout and source markers, fetches `main`, creates `feature/skell-ml-first` from `origin/main`, copies the complete new ML-first code tree into that branch (preserving separate existing repository-only files), and commits the result **without touching main**. Only add `-Push` when ready to publish the source branch. Do not delete or rewrite the old Skell baseline in its existing branch.

## Verification gates before any merge

1. Run `cargo test --locked -p os-model -p os-dsp -p os-engine -p os-flp`.
2. Run `cargo check --locked -p os-app --bin skell-studio` and `cargo check --locked -p os-app --bin skell-studio --features rt-audit`.
3. Build the actual Windows GUI executable with MSVC and inspect native offscreen `--screenshot` outputs for playlist, mixer, piano roll, rack and plugin palette.
4. Launch the executable and test real audio, plugin scanning, MIDI and recording on Windows. Test CPU/latency/glitches under stress, not only Rust compile.
5. Verify that user data is stored under the **Skell Studio** profile and the inherited updater cannot overwrite the fork with another DAW.
6. Check distribution notices in `THIRD-PARTY-NOTICES.txt`, `licenses/` and the upstream license trees before distributing any derivative binary.

## GitHub draft PR audit

Older draft PRs #1–#3 and #7–#10 were audited. Do not automatically merge changes from these onto the ML-first branch: they may conflict with the newer ML architecture. Audit and forward-port only product-facing improvements that aren't already present.

## Status

**Source candidate built from ML and statically checked locally; Windows Cargo compilation and UI screenshots of this exact ML-first code have not been verified.** The GitHub bootstrap PR contains the staging script and its lint checks, not the 125 MB source archive. Once a full source branch is pushed, open a *new draft PR* for the actual migration and let Windows CI run before selecting it as main.
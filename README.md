# Skell Studio — DAW development

**Status (2026-11-12):** Source merge baseline v0.1 completed locally from the two exact uploaded archives, **ML-Studio-michael-lane.zip** and **ADE-Studio-michael-lane.zip**. The complete 95.7 MB combined source ZIP is available in the ChatGPT conversation where the two inputs were provided. It has **not** been uploaded to this repository yet.

## Source merge result

- ADE Studio was already a merge of ML Studio and newer upstream Lemon Studio changes, according to its included `HANDOFF.md`.
- 8,990 shared files identical; 365 same-path files different; 286 ADE-only files; 9 ML-only files.
- Adopted ADE as the compiled baseline, retained the 9 ML-only files under `reference/ml-only/` without reintroducing obsolete audio-worker implementations.
- Disabled inherited Michael Lane update checks from startup and the Updates UI action in the prepared source.
- Moved inherited publishing workflows into a disabled reference folder; added a manual-only Windows build workflow.
- **No visual or product renaming yet**. Next stage: build and validate, then Skell Studio branding.

## Import the source

1. Download **Skell_Studio_Combined_Source_v0.1.zip** from the ChatGPT conversation.
2. Clone this repository, unzip the archive, and copy the **contents** of its `skell-studio/` folder into the cloned repository.
3. Commit and push the files. Use Git LFS only if any file requires it; never commit build outputs, secret keys, personal audio, or recordings.
4. On Windows install Rust/Cargo, Visual Studio C++ Build Tools/Windows SDK and LLVM. Run `scripts/build-windows.ps1` or the manual Windows GitHub Actions workflow after source import.

The source package contains `CREDITS_AND_LICENSES.md`, `LICENSE`, a reproducibility inventory, and integration roadmap.

## Known limitations

- The local environment had no Rust/Cargo toolchain, so **no binary or audio testing has been performed on this merged package**. Source structure and ZIP integrity checks passed.
- ADE's own handoff reports 2,968 passed, 104 failed and 119 ignored tests; these are upstream-reported baseline results, not fresh test results.
- The source declares **GPL-3.0-or-later**, with additional third-party license obligations. It cannot simply be shipped as proprietary closed-source software. License/asset clearance and a complete third-party notice inventory are release blockers.
- The current binary name and UI are inherited from the upstream project until the scheduled branding stage.

**Next milestone:** clean Windows compilation, audio smoke tests and test-baseline triage—then begin the Skell Studio redesign.

# Skell Studio release candidate — next-agent handoff

**Updated:** 2026-11-21  
**Repository:** `skellyruno/skell-studio`  
**Branch:** `feature/skell-release-candidate-20261121`  
**Pull request:** [#22 — Consolidate LYNX engine upgrades and Skell Beat/Tune into release candidate](https://github.com/skellyruno/skell-studio/pull/22)  
**PR state:** Open, draft, targeting `main`. Do not merge yet.  
**Last inspected branch head before this handoff commit:** `7160c89924cbee975a832e82254adc2918d35b39`.

`main` has not been modified. The candidate combines the #15 baseline and #16–#20 feature work on one reviewable branch. Keep the PR as draft until the remaining DAW/audio/release checks are complete.

## Current verified result

The Windows plugin-suite workflow completed successfully on commit `6a145fba1fdf8f1086cf577292640a45f822cb18`:

- [Windows plugin suite run 44913193451](https://github.com/skellyruno/skell-studio/actions/runs/44913193451) — **success**.
- [Hosted plugin editor compile run 44913193473](https://github.com/skellyruno/skell-studio/actions/runs/44913193473) — **success**.
- The plugin test step passed for ML Limiter, ML Delay, ML Reverb, ML Hall, ML EQ, ML Press, Skell Beat, Skell Compressor and Skell Tune.
- The full Release VST3/CLAP package build passed.
- The Studio's in-process VST3 host test passed for ML EQ, Skell Beat and Skell Tune.
- The Windows AMD64 PE-header check passed for both VST3 and CLAP modules for all nine named core products (18 core modules).
- The suite archive uploaded successfully with 66 files: 33 VST3 modules and 33 CLAP modules.

The current branch commits after that passing test added only a test-only unused-import cleanup, documentation updates and the workflow path trigger for `crates/os-vst3/**`; no plugin DSP or wrapper behavior changed after the passing package/host test. A follow-up exact-current-head workflow was triggered by the workflow-filter change; check its result before relying on the post-test metadata change as fully revalidated.

## Deliverables and CI evidence

**Windows VST3 + CLAP suite**

- Actions artifact ID: `14711151601`
- Name: `SkellStudio-Windows-x64-VST3-CLAP-Plugin-Suite`
- Compressed size: 73,090,014 bytes
- SHA-256: `136f0edca224238e50e5fe7d19412ac63374f1b2512e7caed58779cc7dc5e7e6`
- [Download from Actions run 44913193451](https://github.com/skellyruno/skell-studio/actions/runs/44913193451)

**Fresh native plugin UI captures**

- Actions artifact ID: `14710797300`
- Name: `SkellStudio-Native-Plugin-Editor-Captures`
- [Download from Actions run 44913193451](https://github.com/skellyruno/skell-studio/actions/runs/44913193451)
- Captured through the real native editor code paths, not generated mockups.
- Beat, EQ and Tune images were visually opened and inspected. The Beat left heading is now above the tab row; EQ routing has no apparent overlap/cropping; Tune's eight controls fit in the 1100×300 editor.

The artifacts are retained for seven days from 2026-11-21; download them before expiry if needed for local installation/review.

## Implemented work

### Consolidated engine upgrades

- ML Limiter, ML Delay and ML Reverb use the selected LYNX engine integrations in place, preserving their existing plugin IDs and state codecs by design.
- ML EQ's default stereo static path uses the pinned LYNX Parametric EQ engine. Its existing dynamic/spectral stage is retained. Non-default band routing continues through the existing routed engine as a fallback; the routed graph is not fully ported to LYNX.
- Skell Compressor is a separate plugin and does not duplicate the ML Limiter product.
- Existing host identities/automation IDs were intentionally retained for the in-place upgrades.

### Skell Beat

- Separate `plugins/gross-beat` VST3/CLAP crate, with the isolated LYNX Gross Beat engine and DAW-specific Effect wrapper excluded.
- 72 envelope slots: 36 Time and 36 Volume; 6×6 page grids.
- Point add/remove/drag, 13 curve modes, tension wheel and MIDI trigger option.
- MIDI mapping: C5–B7 selects Time slots 1–36; C2–B4 selects Volume slots 1–36.
- Binary state preserves all 72 maps and the MIDI setting; a 1 MiB state capacity is reserved before audio processing.
- Screenshot defect fixed: left section heading moved from y=225 to y=218. Fresh capture confirms the heading no longer overlaps the tabs.

### Skell Tune

- New independent AGPL-3.0-only crate: `plugins/lynx-tune`.
- Pinned source: LYNX SHARE 0.1.30, commit `88ec555ea2bce9714c92707cc06a5abfd317c900`.
- Uses LYNX's YIN pitch detector and TD-PSOLA formant-aware pitch shifter.
- DAW-specific Song/EffectSlot traversal and Send Scale To All helper excluded from the isolated adapter; a small Effect/EffectParams/NoteMask shim and C ABI bridge are used instead.
- MSVC no-inline compatibility added.
- Eight controls exposed: Key, Scale, Retune, Humanize, Formant Shift, Transpose, Mix and Voice Range.
- Accepts incoming MIDI notes from all 16 host MIDI channels, merges their held-note masks into the LYNX MIDI target mask and reports engine PDC latency.
- New identities: CLAP `com.skellstudio.skell-tune`; VST3 class ID `SkellStudioTune1`.
- The engine compiles and is packaged as Windows VST3/CLAP. Tests cover control text/parse, state fields, editor render, MIDI, finite processing/PDC and correction of an off-scale tone.
- The exposed UI is a first core-control pass, not full LYNX Tune parity. Extended vibrato, speed-link, custom keyboard/note maps, custom scale editing and reference-tone controls remain unexposed.

## VST3 / FL-state test result — read the boundary carefully

A new integration test lives in `crates/os-vst3/src/tests.rs`:

`hosts_skell_plugins_and_roundtrips_component_state_through_host_wrapper`

It loaded the actual built Windows VST3 modules for ML EQ, Skell Beat and Skell Tune inside the Studio's `os-vst3` host. For each it:
1. Created a VST3 instance and activated it at 48 kHz.
2. Edited an automatable host parameter and processed audio.
3. Saved the component state using the Studio host's FL wrapper format.
4. Restored that state into a fresh instance and checked the parameter value.
5. Reconstructed the plugin's VST3 preset container and restored it too.

Confirmed log output:

- ML EQ, ParamID 1041356077: 0.5000 → 0.2700; wrapper and VST3 preset restored.
- Skell Beat, ParamID 36636720: 0.0000 → 0.7429; wrapper and VST3 preset restored.
- Skell Tune, ParamID 106079: 0.0000 → 0.7273; wrapper and VST3 preset restored.

**This is a genuine VST3 module/host integration test. It is not a test inside the FL Studio application or a full project/playlist save-close-reopen.** No external DAW scan/automation pass or full FL Studio project round-trip has been claimed. The FL FST preset-import regression is a separate workflow check; verify its current status in the latest Windows-check run before reporting it as passed.

## Reproduction commands

From the repo root on Windows with the Rust toolchain and LLVM/libclang installed:

```powershell
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"

cargo test --locked -p lemon-limiter -p lemon-delay -p lemon-reverb -p lemon-hall -p lemon-parametric-eq-2 -p press -p gross-beat -p fruity-compressor -p lynx-tune

cargo run --locked --release -p bundle-plugins

cargo test --locked -p os-vst3 hosts_skell_plugins_and_roundtrips_component_state_through_host_wrapper -- --nocapture
```

The plugin-suite workflow also captures native plugin editors using `OS_SHOT_DIR` and validates PE machine type `0x8664` for the nine named core VST3/CLAP pairs. The bundle command packages every plugin listed in `tools/bundle-plugins/src/main.rs`.

## CI workflows and current status to refresh

- Plugin suite + VST3 host test: run [44913193451](https://github.com/skellyruno/skell-studio/actions/runs/44913193451), success on SHA `6a145f...`.
- Hosted plugin editor compile: [44913193473](https://github.com/skellyruno/skell-studio/actions/runs/44913193473), success on SHA `6a145f...`.
- Core tests and Windows-check: [44913193465](https://github.com/skellyruno/skell-studio/actions/runs/44913193465). Core-tests job passed; the Windows-check was canceled by later candidate updates before the FL FST preset-import step ran.
- Fast Windows preview + QuickView screenshots: [44913193449](https://github.com/skellyruno/skell-studio/actions/runs/44913193449). Preview compilation was canceled by later candidate updates before the preview executable/screenshot steps completed.
- The suite workflow path filters now include `crates/os-vst3/**` on both push and pull request, so future host adapter/test edits retrigger this suite.

Refresh the current run list for branch `feature/skell-release-candidate-20261121` before finishing the next pass. Newer commits can cancel older PR runs; only report results matching the commit you inspected.

## Remaining work — next priorities

1. Finish the current exact-head Windows Studio-check, FL FST preset-import regression and QuickView Windows preview jobs. Capture/inspect the app at all three requested sizes if the preview workflow completes; fix and recapture any visible defect.
2. Run an actual FL Studio application project save/close/reopen and preset recall test for ML Limiter, Delay, Reverb, EQ, Beat, Compressor and Tune. Include automation, sidechain and PDC cases; do not substitute the Studio host wrapper test above.
3. Do a third-party VST3 host scan/editor open-close/resize/automation pass for the shipped binaries.
4. Run audio A/B against the reference engine captures, especially real vocal correction for Tune. Test 44.1/48/96 kHz, odd host blocks, transport changes and latency compensation.
5. Measure CPU and latency. Audit audio-thread allocation, locking, state-pointer reclamation and retired-queue saturation for Beat; stress Tune's MIDI-mask and DSP processing paths.
6. Surface and test the desired advanced LYNX Tune controls (vibrato, speed-link, keyboard mapping/custom-scale, reference tone) without changing its stable initial parameter IDs.
7. Port the non-default EQ routing graph to LYNX or clearly retain and regression-test the current fallback.
8. Review AGPL-3.0-only obligations for distribution of the vendored LYNX-derived plugins. Full attribution/licence texts are included, but commercial distribution/legal review is not a DSP build test.

## Licensing / compatibility guardrails

- LYNX SHARE-derived Gross Beat, Fruity Compressor, EQ and Tune components are AGPL-3.0-only; their full licence text and attribution accompany their crates.
- Preserve the existing ML Limiter/Delay/Reverb/EQ CLAP IDs, VST3 class IDs, automation parameter IDs and binary state codecs during follow-up work.
- Preserve Skell Beat's `SkellStudioBeat1` VST3 identity, CLAP ID, eight parameter IDs and 72-slot state image.
- Skell Tune is a new product with its own IDs; do not replace or rename the existing SnapTune implementation while iterating on it.
- Do not merge PR #22 or push to `main` until outstanding DAW/audio/licensing checks are reviewed and the candidate is explicitly approved.

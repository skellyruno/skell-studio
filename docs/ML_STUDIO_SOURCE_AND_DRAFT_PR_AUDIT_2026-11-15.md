# Skell Studio — ML Studio source and draft-PR audit (2026-11-15)

## Inputs and preservation rules

- User-uploaded `ML-Studio-michael-lane (1)(1).zip`, compared to the earlier `ML-Studio-michael-lane.zip`.
- Repository of truth: `skellyruno/skell-studio`, `main` head at audit start `b9be47ad1f314924c535f707b72326ee1b8cbc98`.
- Follow `SkellWorld/AGENTS.md`: preserve Windows builds, FLP import, engine/DSP fidelity, project/automation IDs, Skell-branded screens and original credits/licenses.
- Do **not** copy the entire archive on top of `main`, reset Cargo.lock or replace runtime sources without a migration + test plan.

## Archive inventory

Relative to the **earlier ML archive** (not to Skell's working tree), the new upload contains **202 newly added files, 348 modified files and one removed file** (`crates/os-app/src/chords.rs`). The new ZIP is not a clean cherry-pick patch for Skell Studio: the source layout, Cargo manifests, platform integrations and bundled plugin set have diverged.

The notable *new* areas are:

| Area | Added source/examples | Migration risk |
| --- | --- | --- |
| Audio callback safety | `os-dsp/rt_audit.rs`, `spec/audio/rt-audit.md` | Low if feature-gated, higher if installed in every build |
| Mixer/host engine | `os-engine/engine/mixer_graph.rs`, `os-host/smart_disable.rs`, `os-engine/effects/peak_guard.rs`, `delay_line.rs` | **High:** latency reporting, tails, routing, default sound, automation and DSP correctness |
| Clip/record editing | `os-model/audio_edit.rs`, `os-app/recording_merge.rs`, `recording_pickup.rs`, `audio_edits.rs`, `playlist_wave.rs` | **High:** FLP roundtrips, MIDI routing, undo and project/preset state |
| Automation display | `os-app/automation_feedback.rs`, `playlist_links.rs`, `playlist_tracks.rs` | Medium–high: automation feedback must not mutate project or compete with held controls |
| Score/notation | `os-app/notation/**`, `os-model/notation.rs`, `timeline.rs` | **High:** complex new modules with UI/state serialization and font/assets |
| Visualizer | `os-app/visualizer*.rs`, `packaging/visualizer/**` | Medium–high: optional external rendering binaries and worker packaging |
| Local assistant | `os-app/voice_*.rs`, `studio_helper.rs`, `tools/ml-studio-helper/**` | **High:** new process, privacy, runtime model and packaged asset requirements |
| Platform audio | `vendor/cpal/**`, patches to workspace Cargo, devices and output routing | **High:** cross-platform/driver ABI and latency regressions |
| External assets/models | `licenses/SnapTune-AGPL-3.0.txt`, Qwen/llama.cpp notices, plugin/sample assets | **High:** licenses, distribution, model weights, source obligations |

### Integrated in draft PR #11

The first low-risk source slice is **opt-in real-time allocation/timing instrumentation**:

- The `os-dsp::rt_audit` component from the supplied ZIP was adapted with callback/feeder/worker scopes, a Rust global-allocator wrapper, nested-scope test and atomic counters.
- The `os-app/rt-audit` feature installs the allocator *only in explicit diagnostic builds*. Normal builds keep their original allocator.
- Feeder `fill` and playback callback `render_block` use role scopes (compiled to no-ops when disabled).
- This is not a claimed zero-allocation or lower-CPU result: need passing feature-enabled Rust tests, live stressed audio, real plugin restore and measured latency/CPU to make those statements.

### Not yet imported

The other new ML modules remain unimplemented in Skell Studio. They need dependency-by-dependency diffing with the existing app and tests. `SnapTune` includes an AGPL-3.0 license and other models/tools have separate license terms; treat licensing as a required review before combining/bundling binaries. The upstream project still has GPL/copyleft and attribution obligations; do not scrub the origins.

## Draft PR audit — all seven that were open at audit start

| PR | Subject | Audit finding | Decision |
| --- | --- | --- | --- |
| [#1](https://github.com/skellyruno/skell-studio/pull/1) | Windows registry isolation | **Merge conflict**. Main already contains `PROG_ID=SkellStudio.LmnProject` and avoids claiming `.lemon` files. Blind merge risks duplicating/reverting an already verified compatibility fix | Treat as likely **superseded**; compare residual tests/changes to main before closing, do not merge |
| [#2](https://github.com/skellyruno/skell-studio/pull/2) | User-facing dialog branding | Mergeable; mostly string replacements. No evidence that it was tested against the *latest* branch/Windows executable | Rebase/refresh; check tests and UI, then consider merge |
| [#3](https://github.com/skellyruno/skell-studio/pull/3) | Windows cache and sccache | **Merge conflict**. Useful failed-build and compiler cache ideas, but CI config diverged. Must protect main/release/manual runs from cancellation and measure actual cache hits | Reconcile into a fresh branch, verify cache hits and timings, avoid duplicate cache jobs |
| [#7](https://github.com/skellyruno/skell-studio/pull/7) | Windows release-mode engineering EXE | CI run `43613455231` **passed**, artifact `SkellStudio-Windows-x64-Engineering-EXE` exists (approx. 38 MB archive), not a finished release or live-audio test | Keep an accessible engineering artifact; don't label production |
| [#8](https://github.com/skellyruno/skell-studio/pull/8) | Fast preview and seven native screenshots | App compiled, but native capture step in run `43613855888` **failed**. PowerShell `$LASTEXITCODE` was empty after invoking a GUI-subsystem EXE | Block merge until actual PNGs produced and inspected |
| [#9](https://github.com/skellyruno/skell-studio/pull/9) | Local QuickView | PowerShell parser/plan CI passed. Real GUI-subsystem invocation was still vulnerable to empty `$LASTEXITCODE` | Changed script to wait on explicit process and check actual exit code. Requires native screenshot test |
| [#10](https://github.com/skellyruno/skell-studio/pull/10) | Screenshot exit-status fix | Original change treated empty `$LASTEXITCODE` as an error, but did not wait for Windows GUI process; removed piano and rack and DPI scenes | Corrected with explicit process launch/wait and restored seven scenes; Windows capture regression pending |

An **eighth** new draft PR, [#11](https://github.com/skellyruno/skell-studio/pull/11), holds this import slice. No unverified PR has been merged.

## Confirmed verification and remaining blockers

- `main` Windows `cargo check` and FLP compatibility runs `43388824522`, `43388824548` passed on earlier verified heads. FLP parser had 123 passing tests.
- Engineering Windows `cargo build --release` run `43613455231` passed and produced an artifact. Live Win32 launch, native audio devices, VST3 scanning, recording, and screenshot visual inspection are **not** verified.
- Screenshot source compilation succeeded, but screenshot artifact creation failed. The new process wait must generate actual valid images to close this gap.
- Opt-in RT audit feature needs to pass dedicated feature compilation. The first experimental *new* workflow failed to register; removed duplicate and retained established Skell CI. Do not count this as a DSP regression pass.
- Exact CPU, memory and latency changes are unmeasured; do not present speculative improvements as data.

## Next ordered work

1. Validate screenshot capture PR #10 end-to-end, inspect actual seven PNGs (including piano and rack), then validate the local QuickView PR #9.
2. Prove opt-in `rt-audit` feature compilation and test nested scopes; add measured audio stress fixtures before shipping.
3. Triage PRs #1/#3 conflicts against current main; stop duplicate CI runs while protecting main/release/manual; retire only truly superseded changes.
4. Stage the next ML Studio import by functional area, starting with one data-compatible project/undo feature with targeted FLP regression fixtures, then mixer/host audio changes under dedicated A/B DSP and latency benchmarks.
5. Audit external license/models/dependencies before bundling assistant, score/visualizer or SnapTune features.

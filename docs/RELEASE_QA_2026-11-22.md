# Skell Studio — engineering release gate and independent DSP/DAW QA

## Release provenance (critical)
On `pull_request`, GitHub's default `GITHUB_SHA` is a synthetic merge commit, **not** the PR branch tip. The prior VST3 artifact therefore contained a legitimate 33-module Windows x64 build whose manifest said `3231dd965b55af4ddeb17470462076532b4289e5`, rather than the PR tip `f394661251b378a82402f3df75253d3da0f8f848`. Never describe that artifact as an exact-head build. It remains useful evidence for the PR merge result but is not exact source provenance.

The new workflow explicitly checks out `${{ github.event.pull_request.head.sha || github.sha }}`, verifies `git rev-parse HEAD`, and writes `SKELL_SOURCE_SHA` to `BUILD_MANIFEST.txt`. Native 3-resolution UI captures likewise stamp their source SHA. The full Windows EXE workflow is a `push` workflow, where `GITHUB_SHA` already refers to the pushed source commit. Recheck every artifact SHA independently before distribution.

## Automated checks
- Windows gross-beat / limiter / delay / reverb / EQ / compressor / tune unit tests; all 72 slot previews, 4×9 dual-bank hit tests, active host slot selection, 192 state swaps
- Deterministic real DSP runs with stereo multitone+impulse input for **44,100 / 48,000 / 96,000 Hz**, and 17 / 64 / 257 / 4096-sample host blocks; checks finite results, 15-sample published latency, and sample-wise block invariance. Report RMS, peaks and measured CI wall-clock time in `BEAT_DSP_METRICS.txt`. CI runner timing is **not** a trustworthy production CPU budget; measure locally with a controlled machine before final release.
- Windows VST3-only 33 AMD64 modules, no CLAP packaging, all per-module SHA-256 hashes, and real VST3 host component-state round-trip
- Windows full-engine compilation, FL FST preset-import unit regression and opt-in real-time DSP audit tests
- Actual Studio native UI screenshots across 1280×720, 1440×900 and 1920×1080 plus native Beat/EQ/Compressor/Tune editor snapshots, inspected visually for overlap, labels and DPI fit
- Windows x64 **release-profile** application EXE, separate from fast preview; verify exact commit, PE machine, executable size and SHA-256 before using

## Outstanding hands-on tests (must not be marked passed from CI)
1. **FL Studio:** load each VST3 in a real Windows FL Studio host; write automation to every existing parameter; save project, close FL Studio entirely, reopen and verify preset/slot/note recall, automation lanes, latency and state. Compare against previous stable version in a copy of the project; keep the original untouched.
2. **Other VST3 DAW:** scan and load; open/resize the native editor at 100%, 125% and 150% scaling; test drag, wheel, slot selection, MIDI and host automation gestures; close/reopen, undo/redo, save/reopen and bypass.
3. **Recorded-audio A/B:** use the same user-supplied WAV, align dry/wet time including 15 samples of reported Skell Beat latency, loudness-match and measure LUFS/RMS, peak/true-peak, phase/null residue and perceptual artifacts at 44.1/48/96 kHz. Synthetic CI stimuli are **not** recorded WAV A/B.
4. **Performance / real-time safety:** record mean, P95/P99 audio-callback time, max callback, allocations, locks and denormal behavior on the target Windows machine for 32/64/128/256/512 buffers while rapidly changing curves; document actual CPU model, DAW, sample rate and audio driver.
5. **Licensing:** AGPL-3.0-only LYNX portions and other GPL dependencies must have commercial distribution/attribution/legal review. A passing build is not permission to ship closed-source proprietary binaries.

Do not merge PR #22 as a verified release until those gates are completed. Protect `main`, keep external projects and presets backward-compatible, and never delete installed historical CLAP copies: their host identities are distinct from VST3.

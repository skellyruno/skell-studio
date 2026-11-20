# Skell Studio plugin coverage and expansion plan

Audit date: 2026-11-20

This inventory separates plugins already linked into Skell Studio from the missing engine work. The goal is to make the existing FL-compatible effects reliably available in Studio and as Windows VST3/CLAP bundles, then add genuinely missing processors without duplicating what is already there.

## Already implemented in the current Skell Studio source

| Requested role | Current Skell plugin | Existing FL relationship | Status |
|---|---|---|---|
| Limiter | **ML Limiter** | Fruity Limiter | Linked into Studio; separate VST3/CLAP package |
| Compressor | **ML Limiter — Comp section** | Fruity Limiter compressor section | Existing dynamics engine supports compressor threshold, ratio, knee, attack, release and RMS controls; not a separate single-purpose compressor plugin |
| Noise gate | **ML Limiter — Gate section** | Fruity Limiter gate section | Linked into Studio |
| Delay | **ML Delay** | Fruity Delay 3 | Linked into Studio; tempo-sync and stereo delay modes |
| Reverb | **ML Reverb** | Fruity Reeverb 2 | Linked into Studio |
| Additional reverb | **ML Hall** | Skell native reverb/space effect | Separate linked plugin |
| Equalizer | **ML EQ** | Fruity Parametric EQ 2 | Linked into Studio; seven-band parametric EQ |
| Multiband dynamics | **ML Press** | Maximus | Linked into Studio; multiband maximizer/compressor-style processor |
| Filter / modulation | **Lemon Filter, Chorus, Flanger, Phaser** | FL-style filter/modulation counterparts | Linked into Studio |

The application links the core effects from `crates/os-app/src/plugins.rs`; the FL project-state adapters live in `crates/os-engine/src/effects`; and the standalone VST3/CLAP packaging manifest is `tools/bundle-plugins/src/main.rs`.

The Limiter already contains a compressor section. A dedicated compressor could still improve the workflow, but it must add a distinct control model/sound rather than duplicate the same Fruity Limiter algorithm under another name. LYNX's Crest Compressor would be a candidate to evaluate once its source archive is available again.

## Real gaps

### 1. Gross Beat-style time and volume effects

There is no dedicated Gross Beat-style mixer plugin in the current linked plugin list or bundle manifest. Skell has offline sample time-stretch/warp utilities, but those are not a real-time, tempo-synced insert effect. The missing processor needs an allocation-free audio-thread design with:

- Beat/bar-synced pattern playback and host transport following.
- Volume gates, retriggers, stutters, repeats, tape-stop/slowdown and reverse/retrigger patterns.
- Per-pattern mix/bypass and smooth transitions at grid boundaries.
- Deterministic reset/seek behavior, bounded delay/latency, and no audio-thread allocation or locks.
- Preset banks that save in Skell's format while leaving imported FL assets untouched.

### 2. Pitch correction / autotune plugin

The DSP crate already contains **SnapTune** in `crates/os-dsp/src/snaptune.rs`, backed by the isolated C ABI implementation in `crates/os-dsp/vendor/snaptune`. Its existing DSP tests cover 44.1/48/96 kHz, latency reporting, correction behavior, block-size invariance, disabled/dry behavior, voice mode and key-learning behavior.

What is missing from the user-facing plugin layer is a dedicated VST3/CLAP plugin/editor and Studio registration with:
- Amount/speed, key and scale, voice/formant mode, and learn controls.
- Host automation and stable parameter IDs.
- A scale/key suggestion that does not overwrite the user's selection.
- Correct plugin latency reporting and PDC in the host.
- Windows VST3 validation and real vocal A/B tests before calling it release-ready.

The SnapTune source is marked AGPL-3.0. Its existing license and notices must be reviewed and preserved when exposing it as a distributable plugin.

### 3. Dedicated compressor and expanded EQ family

The current Limiter's Comp section and ML Press cover much of the requested dynamics work. A separate compressor or an additional EQ should be added only when it provides a distinct workflow/sound (for example: a clean channel compressor with sidechain/auto-release, or a second EQ with a different filter topology), not just a renamed duplicate.

## Build and regression plan

This branch adds a Windows VST3/CLAP bundle gate for the existing plugin suite. It will build the bundles from the repository's own plugin sources, validate the AMD64 module headers for the requested core effects, and upload a short-lived CI artifact for inspection.

Next implementation order:

1. Finish and verify the FST preset interoperability changes already in this PR.
2. Build and inspect Windows bundles for ML Limiter, ML Delay, ML Reverb, ML Hall, ML EQ and ML Press.
3. Wrap the existing SnapTune DSP in a proper VST3/CLAP plugin with a native editor and state/automation tests.
4. Implement the Gross Beat-style real-time effect as a separate processor with tempo/pattern tests and measured CPU/latency.
5. Port/evaluate any additional LYNX plugins only after their source archive is available and their licensing, DSP behavior and parameter/preset mappings are reviewed.

## Verification boundary

- Existing plugin source and in-app registration: confirmed by source inspection.
- Windows plugin bundle build: newly added to CI in this branch; not yet verified until the exact-head workflow completes.
- SnapTune DSP tests are present in the codebase; a VST3/CLAP plugin wrapper/editor is not implemented by this inventory document.
- Gross Beat-style plugin: not implemented yet.
- Real FL Studio install/preset round-trip and vocal/audio A/B tests: not yet run in this environment.

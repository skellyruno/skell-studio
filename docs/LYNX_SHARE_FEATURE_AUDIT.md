# LYNX SHARE feature audit and Skell Studio adoption plan

Audit date: 2026-11-20  
Source reviewed: user-supplied `LYNX-SHARE-main.zip`  
Status: source/documentation inspection; LYNX itself has not been built or audio-validated in this task.

## What is useful in this source

LYNX SHARE is a C++20/JUCE DAW with an FL-style workflow (Channel Rack, Piano Roll, Playlist and Mixer), FL project import/export, native instruments/effects, and VST3/CLAP hosting. The source documentation advertises:

- 12 exact and 10 near-exact FL instrument engines.
- 34 exact and 10 near-exact FL effect engines.
- 28 dedicated LYNX plugin engines that it maps to 124 named third-party plugin products or families.

Those are the archive's own claims, not independent verification of the counts, quality, latency, or compatibility. The repository contains extensive source, documentation and tests, but this supplied archive does not provide a verified Windows plugin package.

The source's `src/plugin/plugins.def` manifest has 33 entries: 28 marked as built (21 CLAP-only and 7 CLAP+VST3), and 5 synth entries marked not yet built. The seven VST3 targets are Facet EQ, FM10, Apex Limiter, Crest Compressor, Atrium, Echoform and LYNX Tune. The built catalogue spans EQ, limiting/compression, multiband/gating/leveling, de-essing, channel strips, saturation, reverb/plate/shimmer, delay/modulation, pitch shifting, doubling and denoising. These are the manifest's build flags, not proof that we have run or approved the plug-ins.

## The FL integration worth adopting

The strongest reusable product ideas are the compatibility layers, not the JUCE application shell:

1. **Read-only FL discovery.** Locate the user's own FL Studio installation, plugin database, factory content and user preset folders. Read those files; do not modify the installation or copy proprietary DLLs, sound banks or preset content into Skell Studio.
2. **Stable plugin matching.** Prefer the saved plug-in identifier (VST3 class ID, VST2 unique ID or CLAP ID); then use FL's saved/plugin-database path; finally use a conservative same-name match in the same instrument/effect role.
3. **Preset-aware built-ins.** Read FL preset formats and translate supported states into Skell's own engines, while keeping the original files untouched.
4. **Missing-effect fallback.** When the exact effect is unavailable, offer a clearly labelled role-equivalent effect. Preserve the original wrapper/state byte-for-byte so the exact plug-in can be restored when installed later. Do not silently discard the original or pretend automation has been translated when it has not.
5. **A/B and visibility.** Make substitute status explicit, provide a project-level bypass for substitutes and disclose which parameters/automation were or were not translated.

LYNX documents its substitute feature as effect-focused; its own documentation says missing instrument plug-ins are not yet substituted. This is a useful staged design, not a reason to promise universal replacement.

## What Skell Studio already has

The current Rust codebase already provides several foundations:

- A lossless FLP container/event layer and typed project reader/writer in `crates/os-flp`.
- Fruity Wrapper record parsing in `crates/os-flp/src/wrapper.rs`, including plug-in identity and saved state.
- An opt-in Windows FL native plug-in host in `crates/os-native`, which scans the user's installed Fruity plug-in folders rather than shipping those DLLs.
- A plugin preset store in `crates/os-app/src/presets.rs` that saves Skell presets and imports VST3 `.vstpreset` plus VST2 `.fxp`/`.fxb`.
- A built-in FL effect compatibility table with 16 recreated effects (Limiter, Reeverb 2, Delay 3, Soft Clipper, Fast Dist, Balance, Stereo Enhancer, Transient Processor, Chorus, Filter, Flanger, Phaser, Peak Controller, Formula Controller, Vocodex and Wave Candy), plus file-mapped recreations for Parametric EQ 2, Love Philter and Maximus.
- Existing FL factory preset state adapters for Sytrus through **Citrus** and Ogun through **Smith**, plus a clean-room 3x Osc state converter in **ML Trio**. This pass adds the missing generic `.fst` import path and connects all three installed FL preset folders to the Browser's factory categories.

The gap addressed in this branch was narrower: the generic Import preset dialog/drop extension filter did not include `.fst`, and the generic import dispatcher did not turn supported FL states into normal Skell presets. This branch adds `.fst` import for the existing Citrus/Sytrus, Smith/Ogun and ML Trio/3x Osc adapters. It also lists the user's local 3x Osc channel presets from `Data/Patches/Channel presets/3x Osc` in the plugin preset menus and Browser. It refuses to pass an FL preset to an unrelated plug-in or accept a state with the wrong 3x Osc payload size. Imported presets are saved in Skell's own `.ospreset` format; the source `.fst` is read only.

## Adoption roadmap

### Stage 1 — safe FL preset interoperability

- Complete and verify the new `.fst` import path for Sytrus/Ogun/3x Osc.
- Test user/factory discovery on a real FL installation without copying any FL files.
- Add format-specific readers only when there is a reproducible fixture, explicit plug-in identity, state round-trip test and a matching Skell engine.
- Candidates to investigate separately include FL's GMS, Morphine, Toxic Biohazard and other native preset types. LYNX documents `.gmsynth`, `.mrp` and `.tbio` in addition to `.fst`; this branch does not claim to support those formats.

### Stage 2 — FL plug-in identity and project recovery

- Build a searchable, read-only index of the user's FL plug-in database and installed plug-ins.
- Match by stable IDs before path/name heuristics; keep instrument and effect roles separate.
- Show exact match, format-converted match, native-host match or substitute as distinct states.
- Keep the original FL wrapper record and state intact through save/reopen. Add fixture-based regression coverage before enabling any automatic substitution.

### Stage 3 — role-equivalent effects

- Begin with effect categories that are useful and already represented by Skell's plug-in ecosystem: EQ, dynamics/limiting, reverb, delay, modulation, distortion/saturation, de-essing and pitch correction.
- Prefer an exact installed plug-in. Use a Skell-built equivalent only when the source state has a verified parser/parameter map.
- Never claim that a parameter or automation map is exact without measurement and A/B testing. Keep untranslatable automation attached to the original state and disclose that it does not drive the replacement.
- Make substitution opt-in, reversible and bypassable per project.

### Stage 4 — more native instruments and effects

- Evaluate engines one at a time against real FL reference renders and known presets.
- Prioritize accurate preset/state loading, MIDI behavior, envelopes, modulation, oversampling, latency and CPU before adding a large catalogue.
- Keep stable plugin IDs and project state migrations backward-compatible.

## Licensing and asset boundary

The supplied LYNX source identifies itself as **GNU AGPL version 3**. Skell Studio's current workspace identifies itself as **GPL-3.0-or-later**. Do not copy or vendor LYNX implementation files, generated mapping tables, plugin binaries, JUCE build products or any third-party/FL content into this repository as part of this pass. Any future use of LYNX code or linked components needs an explicit license review and a deliberate distribution/source-offer plan; this audit does not make that decision.

FL Studio plug-ins, DLLs, factory presets, samples, FLEX content and other licensed assets remain the user's own installation/content. Skell should discover/read supported files locally and keep test fixtures synthetic or user-authorized. Do not check proprietary FL presets or samples into Git.

## Verification boundary

- Archive inventory and source/document review: completed.
- LYNX Windows build, tests, audio A/B, latency/CPU and plugin exports: not run.
- Skell `.fst` implementation: committed to this feature branch for Sytrus/Ogun/3x Osc; CI and test results are still required.
- Real FL installation and preset round-trip: not verified in this environment.
- No change has been made to `main`; this work is isolated on a feature branch and must not be merged until its tests and compatibility checks pass.

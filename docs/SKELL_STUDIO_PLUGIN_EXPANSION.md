# Skell Studio plugin selection and expansion plan

Audit date: 2026-11-20

## Selection rule

Keep one best implementation per job. Do not install two plugins that implement the same FL effect merely because both are present in the source archive. Distinct Pro-style processors can be considered as an optional second tier later, but the core target is the most accurate compatible FL behaviour.

The parity figures below are reported by the supplied LYNX SHARE 0.1.30 source documentation and its test reports. They are source-reported comparisons, not independent parity reruns performed by Skell in this session. "Bit-identical" means exact output for the tested reference case; near-exact results remain near-exact and are not described as perfect.

## Chosen engines

| Role | Selected implementation | Source-reported evidence | Decision |
|---|---|---|---|
| Limiter | LYNX Fruity Limiter engine | 101/272 installation states bit-identical; all tested states within -129.5 dB | Upgrade the existing ML Limiter in place; retain compressor and gate sections, sidechain, state and automation mapping |
| Dedicated compressor | LYNX Fruity Compressor engine | 70/70 tested installation states bit-identical; sidechain supported | **Skell Compressor** is now a separate plug-in; do not create a duplicate copy of ML Limiter |
| Delay | LYNX Fruity Delay 3 engine | 85 installation states plus 110 randomized states; one reported case at -151 dB | Upgrade the existing ML Delay in place |
| Reverb | LYNX Fruity Reeverb 2 engine | 293 tested installation states bit-identical | Upgrade the existing ML Reverb in place; keep ML Hall as a separate additional hall/space algorithm |
| EQ | LYNX Parametric EQ 2 + linear-phase implementation | 912/1,139 tested states bit-identical; all IIR results within -166 dB; linear-phase path within -127 dB | Upgrade ML EQ in place, preserving all existing automation and stored state |
| Gross Beat / time FX | LYNX Gross Beat time/volume engine | 41 installation states bit-identical at the two primary reported test tempos; at 120 BPM 34/41 bit-identical and the rest -141 to -153 dB | New Skell Beat plug-in; this effect is genuinely missing from Skell |
| AutoTune-style pitch correction | LYNX Tune | Its documented parity assessment rates it strongly on configuration/design and includes real-time, allocation and stability tests; see source `docs/PARITY.md` | Select this engine over the small SnapTune wrapper; implement one Skell tuner, not two |

### Important accuracy caveats

- LYNX Gross Beat documents an unresolved spline curve type 10 difference. Its primary parity tests are excellent but that remaining difference is real.
- The Fruity Limiter's residual differences are documented as very small, not zero.
- The EQ's 1,139-state corpus is not 1,139 bit-identical states; 912 are exact and the remainder are within the reported error bound.
- LYNX Tune's comparison scores are the source's assessment against its available reference versions. The Auto-Tune Pro baseline is noted as expired in that report, so the score is not a fresh independent measurement by Skell.
- None of these figures substitutes for a new Skell audio A/B or real DAW validation pass.

## What Skell already has

The main branch already contains:
- **ML Limiter** — Fruity Limiter-style limiter, compressor and gate sections.
- **ML Delay** — Fruity Delay 3-style delay.
- **ML Reverb** — Fruity Reeverb 2-style reverb.
- **ML Hall** — additional hall/space reverb.
- **ML EQ** — seven-band Parametric EQ 2-style EQ.
- **ML Press** — Maximus-style multiband dynamics.

These existing plug-ins are real and registered in the Studio. The LYNX engines are selected as upgrades to the matching processors, not as duplicate plug-in entries. Existing parameter IDs, host automation, FL state, presets and session compatibility must be preserved during replacement.

## Implementation status on this branch

### Skell Beat — 72-slot editor and MIDI integration

Implemented on this branch:
- Separate AGPL-3.0-only plug-in crate at `plugins/gross-beat`, kept separate from the GPL Studio executable.
- Isolated LYNX Gross Beat engine, state parser and curve implementation. The LYNX-specific DAW Effect wrapper has been removed from the vendored code.
- C ABI bridge for factory state, all 72 slots, tempo/position/start/stop/jump transport, audio processing and 15-sample latency reporting.
- Native curve workspace with Time and Volume pages, 36 slots per page, selectable slots, point insertion/removal and dragging, 13 curve modes and wheel-adjustable tension.
- MIDI note-to-slot triggering: C5–B7 selects Time slots 1–36 and C2–B4 selects Volume slots 1–36. Event-bounded processing applies note events at their sample offsets.
- Host parameter controls remain available alongside the curve workspace.
- Host state serialization preserves the complete binary slot map, including all 72 maps and MIDI option. The 1 MiB state capacity is reserved before audio processing.
- Seven Beat tests passed on the previous validation revision, including the full 72-slot state restore and editor workspace test. The current revision moves the left section heading upward after the real screenshot exposed overlap with the tab row; a fresh capture is required to confirm the correction.
- VST3/CLAP bundle manifest and Windows AMD64 artifact-validation workflow.
- Full AGPL license text and source attribution are included.

Still unverified:
- A real VST3 host save/close/reopen round-trip and a complete FL Studio project save/reopen round-trip.
- A real DAW host scan/automation pass, audio A/B and CPU profiling.
- Factory envelopes are the LYNX clean-room factory bank based on documented behaviour; no proprietary FL factory assets were copied.

### Skell Compressor — first integration pass

Implemented on this branch:
- Separate AGPL-3.0-only **Skell Compressor** VST3/CLAP crate at `plugins/fruity-compressor`.
- The isolated LYNX Fruity Compressor engine (not the Fruity Limiter compressor subsection).
- The six original raw controls: threshold, ratio, makeup gain, attack, release and knee/release type.
- Original look-ahead/PDC state bit; the engine always has a 64-frame output delay and reports 64 samples to the host only when the original look-ahead option is enabled.
- Optional stereo sidechain aux input through the shared native wrapper. Its Auto/On mode is saved in a Skell extension byte at offset 36; Auto follows the original FL sidechain assignment at offset 32, and On forces the connected VST3/CLAP aux input. The original 36-byte FL state, including all flag/sidechain bytes, is retained.
- FL binary state layout v1–v3, including preservation of flags/sidechain bytes not directly exposed as automation.
- Shared Skell native editor and Windows VST3/CLAP bundle gate.

Local C++ verification against the uploaded LYNX SHARE 0.1.30 source: zero allocations during 100 processing blocks, bit-identical output across the tested 1/17/64/127/512/2048/4096/8192 block sizes, finite output, and a measured difference when an external sidechain is used. Exact Windows Rust/plugin build and real-host state/sidechain tests are still pending.

### AutoTune-style pitch correction — Skell Tune wrapper

LYNX Tune remains the selected engine; the duplicate SnapTune wrapper is not being added as a second tuner. A separate AGPL-3.0-only `plugins/lynx-tune` crate now wraps the pinned LYNX YIN pitch detector and TD-PSOLA formant-aware shifter. Its DAW-specific Song/EffectSlot traversal is excluded and replaced by a narrow Effect/EffectParams/NoteMask shim plus a C ABI bridge.

The first Skell UI exposes Key, Scale, Retune, Humanize, Formant Shift, Transpose, Mix and Voice Range. Basic MIDI input is accepted, note masks from all host MIDI channels are merged for the engine's selected MIDI source, and the engine's fixed PDC latency is reported to the host. The wrapper has a new, distinct CLAP ID and VST3 class ID; the existing SnapTune source and all existing plugin identities remain untouched.

Tests cover control text/parse stability, host parameter-field serialization, MIDI note forwarding, finite audio and latency reporting. The first Windows test attempt compiled the C++ Tune engine but stopped on a missing Rust `Params` trait import in the new state test. That import and the explicit `linked` feature declaration are now fixed; the corrected exact-head CI result is pending. The current wrapper exposes the core controls only; LYNX's extended vibrato, speed-link, note-map, custom-scale and reference-tone options remain engine defaults and are not yet surfaced as Skell controls.

### Existing processor upgrades

The selected in-place LYNX engine adapters are now present for ML Limiter, ML Delay, ML Reverb and the normal static stereo path of ML EQ. ML EQ retains the existing dynamic stage and routes non-default band-routing configurations through the established routed engine until that graph is ported. Skell Compressor remains a separate, distinct plug-in. The integrations preserve the existing host-facing identities and stored-state layouts by design; real host automation and full DAW state reopen tests remain required.

A separate Pro-style limiter/compressor/EQ (LYNX Apex, Crest, Facet) is not part of this core migration. They are distinct optional products, not replacements for the selected FL processors.

### In-place LYNX engine replacement — 2026-11-21

The user has prioritized direct DSP replacement to get the audio engine work moving. **ML Reverb is now ported in place to the LYNX SHARE Fruity Reeverb 2 engine.** The existing ML Reverb product remains the same plug-in: its display name, VST3 class ID, CLAP ID, 15 parameter IDs, two option IDs, 66-byte FL state codec, preset identity and UI remain unchanged. The new bridge maps the existing raw 15-knob state plus tempo-predelay and mid/side flags directly into LYNX's `reeverb2::Settings`.

The vendored engine-only translation unit is pinned to LYNX SHARE 0.1.30 commit `88ec555ea2bce9714c92707cc06a5abfd317c900`, excludes LYNX's DAW-specific effect wrapper, and carries its attribution and full AGPL-3.0-only text. The `lemon-reverb` package metadata now reflects that source. Final combined-distribution review is deferred from this implementation milestone; it remains a release follow-up, not a reason to omit the source notices.

**Local C++ verification:** the selected engine rendered finite output at 44.1, 48 and 96 kHz; host block sizes 1, 17, 64, 127, 512, 2,048 and 8,192 samples matched exactly in the tested 48 kHz render; mid/side mode changed the response; and a 100-block 8,192-frame C ABI bridge stress test recorded zero heap allocations after initialize/reserve. These are engine/bridge smoke checks, not a Windows VST3 host or FL Studio project round-trip.

ML Limiter, ML Delay, ML Reverb and ML EQ now have selected in-place engine integrations on this branch; the final state, automation, sidechain, PDC and audio regressions still need exact-head verification. No duplicate FL-style products are planned.

## Build and test gates

The branch adds a Windows VST3/CLAP build-and-bundle gate for the plugin suite. It tests ML Limiter, ML Delay, ML Reverb, ML Hall, ML EQ, ML Press, Skell Beat, Skell Compressor and Skell Tune, packages VST3 and CLAP modules, validates AMD64 PE headers and uploads the artifact for inspection.

Required before any release:
1. Exact-head Windows Rust tests, plug-in compilation and VST3/CLAP bundle validation.
2. Gross Beat transport, block-size, state restore, reset/seek and parameter automation tests; add audio-thread allocation audit and measured CPU.
3. Real Windows VST3 host scan, editor open/close/resize and host state save/restore.
4. FL Studio tests with real project/preset state round-trips.
5. Audio A/B against reference captures and regression tests at 44.1/48/96 kHz, odd block sizes and tempo/transport jumps.
6. Finish LYNX Tune's extended controls and run vocal A/B, CPU, allocation and host MIDI checks.
7. Complete real host and project/preset state save/reopen validation for each affected plugin, including the remaining compressor, routing and sidechain cases.

## Verification boundary

- Existing Skell plugin inventory: confirmed by source inspection.
- LYNX selection and parity figures: transcribed from the supplied LYNX SHARE source documentation; not independently rerun during this integration.
- Gross Beat engine C++ bridge: compiled and smoke-tested locally against the supplied LYNX 0.1.30 archive; factory state was 27,046 bytes with 72 slots, the 15-frame dry delay is confirmed in engine source, a selected-slot render produced finite output, and 100 × 4,096-frame processing blocks performed zero measured heap allocations. Variable host blocks (1, 17, 64, 127, 512, 2,048 and 8,192 frames) differed by less than 6×10⁻⁸ in the local render comparison. This is not a Windows plug-in or in-host test.
- Skell Beat's full 72-slot curve editor, MIDI mapping and host-field serialization tests are implemented; its seven tests passed on the prior plugin-suite revision. A refreshed post-layout-fix capture and latest exact-head suite are pending.
- LYNX Tune's separate Windows VST3/CLAP wrapper and C++ engine source are implemented; the first MSVC C++ compile succeeded, while the first Rust test compile found a missing trait import. The trait import and feature declaration fix are in the latest candidate; exact-head CI is pending.
- Native VST3 host scan/save/reopen and FL Studio project save/reopen remain unverified. Current coverage is plugin parameter-state field serialization, Beat's complete 72-slot state serialization test, and the separate FL FST preset-import regression.

### In-place LYNX Fruity Delay 3 replacement — 2026-11-21

ML Delay is being ported in place to the pinned LYNX SHARE 0.1.30 Fruity Delay 3 engine. Its existing wrapper, plugin identity, parameter IDs, parameter ordering, 108-byte version-1 state format, FL preset mappings and editor remain unchanged. The adapter passes the current 26 raw stored controls through LYNX's `delay3EngineValue` mapping, applies changed values only (so its smoothing vector does not churn every host block), and sends host tempo at each block. The C++ engine's constructor reserves its internal 26-entry parameter smoothing list and creates the delay/filter buffers during plugin initialization.

Pinned upstream revision: `88ec555ea2bce9714c92707cc06a5abfd317c900`. The engine-only translation unit keeps the LYNX Fruity Delay 3 engine and excludes its DAW-specific effect-slot wrapper and Fruity Delay 2 implementation. Build/test and real host state/preset validation are still required.

### Skell Beat MIDI slot triggering — 2026-11-21

Skell Beat accepts Basic MIDI note input. At each note's sample offset, C5–B7 selects Time slots 1–36 and C2–B4 selects Volume slots 1–36, matching LYNX's Gross Beat keyboard map. New Skell Beat factory states enable MIDI triggering; when an existing FL/Gross Beat state contains an explicit MIDI-trigger option, that saved setting remains authoritative. Note and channel held-state tracking is delegated to LYNX's fixed-capacity `MidiSlots` helper. The plugin dispatches audio in event-bounded slices so a note changes the slot before the corresponding sample. MIDI selection is allocation-free in the callback.

The curve/point workspace and MIDI trigger editing controls are now implemented in `plugins/gross-beat/src/editor.rs`; the left section title was moved from y=225 to y=218 after inspection of the previous native capture. The latest capture and exact-head tests must confirm the visual fix.

### In-place LYNX Fruity Limiter replacement — 2026-11-21

ML Limiter is being ported in place to the pinned LYNX SHARE 0.1.30 Fruity Limiter engine. The existing plugin name, VST3 class ID, CLAP ID, 18 parameter IDs, parameter order, 169-byte version-7 state image, spare bank, sidechain index, exact-ceiling option byte, attack-hold gesture, editor pages and history display are retained. LYNX uses the same 18 raw control ranges/defaults and maps the existing exact-ceiling flag directly.

The engine allocates its peak/RMS windows, look-ahead delay and stereo scratch buffers during initialize. The bridge chunks every host block into LYNX's 512-frame engine blocks, passes the optional stereo sidechain through preallocated interleaved scratch, and reports the source look-ahead latency. The editor's history feed remains live using a preallocated input-peak copy and nonblocking publication; its detector-level rail is an input-peak approximation until the exact engine tap is exposed. Windows tests and real FL/VST3 state and sidechain round-trips remain required.

### LYNX Fruity Parametric EQ 2 — standalone engine integration

The standalone ML EQ VST3/CLAP build routes its normal stereo static path through the pinned LYNX SHARE 0.1.30
Parametric EQ 2 engine. The existing ML EQ plugin name, VST3 class ID, CLAP ID, 36 host parameter IDs, 354-byte
saved-state codec, editor and A/B state remain unchanged. The existing dynamic/spectral stage is retained, and
non-default band routing continues through the established routed engine until that routing graph is mapped into
LYNX. Linear-phase FFT scratch and magnitude buffers are reserved at initialization; selected FFT quality is
fixed at initialization to prevent realtime resizing. Windows compilation and real FL/VST3 state/audio validation remain required. The previous native editor capture was inspected and had a clean EQ layout; the new exact-head screenshot will be checked after the plugin suite tests complete.

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

### Skell Beat — first integration pass

Implemented on this branch:
- Separate AGPL-3.0-only plug-in crate at `plugins/gross-beat`, kept separate from the GPL Studio executable.
- Isolated LYNX Gross Beat engine, state parser and curve implementation. The LYNX-specific DAW Effect wrapper has been removed from the vendored code.
- C ABI bridge for factory state, loading the 72-slot state, eight parameters, tempo/position/start/stop/jump transport, audio processing and 15-sample latency reporting.
- Skell's shared native plug-in editor framework with eight controls: time slot, volume slot, time mix, volume mix, attack, release, tension and hold.
- VST3/CLAP bundle manifest and Windows AMD64 artifact-validation workflow.
- Full AGPL license text copied beside the plug-in source and source attribution added to `THIRD-PARTY-NOTICES.txt`.

Known first-pass gaps:
- The current editor exposes the eight host parameters, not Gross Beat's full curve/point editor for all 72 slots.
- MIDI note-to-slot triggering is not yet wired to the plug-in.
- Factory envelopes are the LYNX clean-room factory bank based on documented behaviour; no proprietary FL factory assets were copied.
- Full binary slot state is retained through the plug-in state field, but round-tripping through a real VST3 host and FL project remains to be verified.

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

### AutoTune-style pitch correction

LYNX Tune is the selected target. It offers a broader vocal-correction control set than the small SnapTune wrapper I initially prototyped. That duplicate SnapTune wrapper has been removed from this branch; the pre-existing SnapTune DSP source remains part of the repository baseline, but is not being added as a second tuner.

The selected LYNX Tune engine still needs its own Skell VST3/CLAP wrapper, native editor and tests before it can be called implemented. Required controls include retune/speed, key, scale/note mask, voice range, humanize, formant, transpose, mix and learn/MIDI-target behavior. Preserve its audio-thread allocation guarantees and latency behavior.

### Existing processor upgrades

Limiter, dedicated compressor, Delay, Reverb and EQ have been selected from the source comparison but have not yet been ported into their Skell plug-ins. The next port must retain:
- Existing Skell parameter/automation identifiers wherever the control meaning is unchanged.
- Existing FL state import/export and unknown stored bytes.
- Preset and session backward compatibility.
- Sidechain behavior and plug-in delay compensation.
- Existing UI identity/layout unless an intentional Skell UI improvement is tested.

A separate Pro-style limiter/compressor/EQ (LYNX Apex, Crest, Facet) is not part of this core migration. They are distinct optional products, not replacements for the selected FL processors.

### In-place upgrade license boundary — 2026-11-21

The selected LYNX Delay 3, Reeverb 2, Fruity Limiter and Parametric EQ 2 implementations are AGPL-3.0 source. The corresponding existing Skell crates (`lemon-delay`, `lemon-reverb`, `lemon-limiter` and `lemon-parametric-eq-2`) inherit the workspace's `GPL-3.0-or-later` declaration, and `crates/os-app` links all four with the `linked` feature. Directly copying the LYNX engines into these shared crates would cross the current application/plugin licensing boundary and could change the obligations for the combined Studio distribution.

**Do not vendor those AGPL engines into the linked crates until the distribution architecture is deliberately resolved.** Viable paths to evaluate are: (1) an explicit decision to distribute the combined Studio under the applicable AGPL terms; (2) a standalone AGPL plug-in module that preserves the existing plug-in identity/state contract without linking its engine into the Studio executable; or (3) a separately authored clean-room implementation that does not copy the AGPL implementation source. Path 2 still needs a careful duplicate-module/host-discovery plan and does not by itself upgrade the in-app linked effect. This is an engineering/licensing gate, not a legal opinion.

Until that decision is made, the existing in-app effects keep their current Rust DSP and saved-state behavior. The 2026-11-21 Skelly Audio vendor-metadata pass is cosmetic only: ML Delay, ML Reverb, ML Limiter and ML EQ retain their current display names, CLAP IDs, VST3 class IDs, parameter IDs and state formats.

## Build and test gates

The branch adds a Windows VST3/CLAP build-and-bundle gate for the plugin suite. It tests ML Limiter, ML Delay, ML Reverb, ML Hall, ML EQ, ML Press and Skell Beat, packages VST3 and CLAP modules, validates AMD64 PE headers and uploads the artifact for inspection.

Required before any release:
1. Exact-head Windows Rust tests, plug-in compilation and VST3/CLAP bundle validation.
2. Gross Beat transport, block-size, state restore, reset/seek and parameter automation tests; add audio-thread allocation audit and measured CPU.
3. Real Windows VST3 host scan, editor open/close/resize and host state save/restore.
4. FL Studio tests with real project/preset state round-trips.
5. Audio A/B against reference captures and regression tests at 44.1/48/96 kHz, odd block sizes and tempo/transport jumps.
6. LYNX Tune port and the same Windows/host/DSP checks.
7. Port the selected Limiter, Compressor, Delay, Reverb and EQ engines one at a time, with their state mappings and regressions.

## Verification boundary

- Existing Skell plugin inventory: confirmed by source inspection.
- LYNX selection and parity figures: transcribed from the supplied LYNX SHARE source documentation; not independently rerun during this integration.
- Gross Beat engine C++ bridge: compiled and smoke-tested locally against the supplied LYNX 0.1.30 archive; factory state was 27,046 bytes with 72 slots, the 15-frame dry delay is confirmed in engine source, a selected-slot render produced finite output, and 100 × 4,096-frame processing blocks performed zero measured heap allocations. Variable host blocks (1, 17, 64, 127, 512, 2,048 and 8,192 frames) differed by less than 6×10⁻⁸ in the local render comparison. This is not a Windows plug-in or in-host test.
- Skell Beat Rust wrapper and Windows VST3/CLAP artifact: pending exact-head CI.
- LYNX Tune wrapper: selected, not implemented.
- FL Studio project round-trip, DAW automation, full Gross Beat curve editor, MIDI slot triggering and vocal A/B: not yet verified.

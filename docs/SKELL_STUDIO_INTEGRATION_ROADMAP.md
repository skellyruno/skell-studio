# Skell Studio integration roadmap

## Current stage — source baseline
- Verify exact user source snapshots with pinned archive Git tree SHA-1 fingerprints.
- ADE is the compiled base because it already integrates ML + Lemon; preserve ML-only files as reference.
- Disable inherited update server and upstream publishing.
- Do not rebrand or replace existing DSP until playback, recording, import/export, and project save/load have a tested baseline.

## Next engineering gates
1. Clean Windows build, artifact signing plan and dependency audit.
2. Audio playback, recording, click-free seek/scrub, save/reopen and export tests.
3. Review ADE's prior 104 failing tests against original baseline; label new regressions separately.
4. Plugin hosting (VST3) and MIDI smoke testing; real Windows hardware tests at multiple buffer sizes.
5. License strategy: remain GPL/compatible or pursue independently licensed replacement for proprietary distribution.
6. Skell Studio UI pass: matte black, neon green, consistent shared skull asset and scalable layout.
7. SkellCore + SkellForge integration via documented Rust/C++ boundaries or native VST3 hosting, not copied internal build trees.

## Reliability contract
Preserve original imported source for rollback; do not auto-update from another product; keep original author licenses and third-party notices. Do not say Windows build or audio is verified until CI and hands-on tests prove it.

## FL presets and native plug-in expansion (2026-11-20)
- See [LYNX SHARE feature audit and adoption plan](LYNX_SHARE_FEATURE_AUDIT.md) for the user-supplied source review, licensing boundary, FL discovery ideas, plugin fallback design and staged engine roadmap.
- Import preset and the Browser now expose local FL `.fst` presets for the existing Citrus/Sytrus, Smith/Ogun and ML Trio/3x Osc state adapters; unsupported plugin/state combinations are rejected rather than passed to unrelated plugins.
- Next gates: synthetic parser tests in CI, real Windows FL installation discovery, preset load/save/reopen, then additional native preset formats only with tested codecs.

## Core plugin suite and new-effect gaps (2026-11-20)
- See [Skell Studio plugin coverage and expansion plan](SKELL_STUDIO_PLUGIN_EXPANSION.md) for the current Limiter/Compressor, Delay, Reverb, EQ and Press inventory, plus the Gross Beat-style and autotune gaps.
- The Windows VST3/CLAP bundler now gets its nested Cargo builds with `--locked`.
- A dedicated Windows workflow tests and packages the current plugin suite, validates AMD64 modules for ML Limiter, ML Delay, ML Reverb, ML Hall, ML EQ and ML Press, and uploads the suite as a CI artifact.

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

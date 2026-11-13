# Skell Studio verified-upgrade checkpoint — 2026-11-13 (UTC)

## Gate results before the latest visual pass

- Windows app compile gate: **PASS** [run 43360086220](https://github.com/skellyruno/skell-studio/actions/runs/43360086220), commit `001e61f1611b6946148a79fc9232fc2821acf9f8`.
- Windows file-association tests: **PASS** [run 43360107437](https://github.com/skellyruno/skell-studio/actions/runs/43360107437), commit `61d39349ff9becc6f4a4c6f9e02b75eaed4c0de1`. The only change since the compilation commit was an association-workflow path trigger, not a source change.
- FLP parser compatibility: **123 passed** [run 43319605610](https://github.com/skellyruno/skell-studio/actions/runs/43319605610), an earlier commit. A current-head FLP rerun is still needed before release.
- **Current visual-pass head**: `a46d67b39a6a7dfc7f537e7775dbfe12faf4faac`. Windows compile and association tests have been requested by workflow triggers, but are **not yet verified** on this commit.

## Latest source-drawn visual upgrade

The new shared `skell_chrome` primitives provide:
- Playlist: compact color-coded identity rail in track headers, dimmed when tracks are off, while preserving selection and drag targets.
- Piano roll: subtle dual-stroke note highlights for readable note boundaries at supported zooms; original note editing, velocity, portamento and resize handling remain unchanged.
- Mixer: five evenly spaced calibration marks alongside the original meter graphics; the existing live meter engine, peak/VU logic and clipping status are not modified.
- Browser/plugin database: selected/hovered row underscoring consistent with mixer FX slots and arrangement cards. Folder expansion, search and activation remain intact.
- Existing Skell UI foundation includes panel bevels, accent tools, FX slot rails and meter wells; the new additions deepen the same dark/green identity rather than replacing the DAW's underlying widgets.

No audio DSP, project serialization, FLP parser, MIDI/VST3 host, or plugin binaries were changed in this visual pass. Existing GPL notices and upstream attributions remain in the original source files. The shared visual helper is GPL-3.0-or-later.

## Verification and release policy

- Current-head Windows compile: [run 43368744646](https://github.com/skellyruno/skell-studio/actions/runs/43368744646) — queued at checkpoint.
- Current-head file-association tests: [run 43368744838](https://github.com/skellyruno/skell-studio/actions/runs/43368744838) — queued at checkpoint.
- Earlier Windows compile/association success is a **baseline**, not proof of current-head correctness.
- Do not merge the UI upgrade until both current-head Windows gates pass. Confirm relevant source checks and Rust tests in the jobs; rerun FLP parser regression before final release.
- No installer or runnable Windows executable has been built or tested.
- PR #1 and #3 remain conflicting against main and must not be merged blindly. PR #2 is mergeable but still needs verified relevant tests and a scope review.

## Next

1. Inspect both current-head CI logs and fix source/compiler/test failures if any.
2. Recheck core Rust unit tests and source-merge safety gate, and rerun FLP compatibility against the current head.
3. Review remaining PRs for overlap and license preservation, then merge only verified non-conflicting changes.
4. Expand and validate visual responsiveness at narrow sizes and interaction hit regions before final merge.

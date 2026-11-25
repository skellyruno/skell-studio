# Skell Studio PR #12 and #21 reconciliation (2026-11-21)

Target: draft PR #22, branch `feature/skell-release-candidate-20261121`. Source baseline inspected: `9ff105830d5a8ae5302dcb794422c8db00f1ad5d`.

## PR #21 — functionality already present in #22

PR #21 head `ad2071cfcd25f4b751bc59ab20965ee2a8231275` diverges from #22 in commit history. The 26 paths changed by its unique commit are all present in #22, including the Delay 3 vendored engine, license material, build and editor changes, and MIDI input support for Skell Beat. Sampled content hashes match exactly for `crates/os-ui/src/kit/lemon.rs`, `plugins/lemon-delay/src/lib.rs`, `plugins/lemon-delay/vendor/LYNX-DELAY3-NOTICE.md`, and `plugins/gross-beat/vendor/core/GrossBeat.cpp`. Other paths have newer revisions on #22 (notably Beat's state handling). Preserve those revisions; do not cherry-pick the stale commit wholesale.

## PR #12 — unique legacy migration tooling retained, not run

PR #12 head `f12faeca147ce7a0d301271329cb003bf21d3746` contains three files absent from #22: the ML-first ZIP staging script, its source-migration guide and a PowerShell parse/guard workflow. These are retained on #22 for historical recovery, with a warning that the active candidate already integrates the newer ML-first code. The checker is enabled for PR changes to these paths. **No historical ZIP has been extracted or overlaid onto the current candidate.**

## Other reconciliation fix

Corrected `plugins/lemon-delay/vendor/LYNX-DELAY3-NOTICE.md`, which had incorrectly labeled Fruity Delay 3's source as Reeverb 2. AGPL obligations remain open for legal review.

## Merge gate

This consolidation does not authorize a merge: verify exact-head Windows builds, plugin suite and native captures; inspect refreshed UI; complete DAW save/reopen, real audio A/B, CPU/latency/realtime checks, and license review. Keep PR #22 draft until verified. No published automation IDs, preset formats or plugin state schema should be changed by this reconciliation.

# Skell Studio branch reconciliation — 2026-11-22

## Scope and preservation

Audited all 24 repository branches and all 22 pull requests against draft PR #22, starting at commit `6614498230e14d4b1ce98d9a6d5bf1d8c3886b51`. The branch inventory includes 23 non-main branches (including the release candidate) plus `main`. PR #22 is the sole open PR, remains draft, and targets `main`.

**No branches were deleted, no historical pull requests reopened or merged, and no changes were pushed to `main`.** A diverged history does not prove an independent feature is absent: it may be equivalent code, an earlier implementation, or a subsequent refinement. For each diverged branch, the files changed by that branch relative to the candidate's merge base were compared by Git blob SHA against the candidate; the mismatches are not automatically safe to cherry-pick.

## Included by ancestry (ahead=0 vs release candidate)

- `feature/skell-beat-curves-midi-20261121`
- `feature/skell-fl-fst-presets-lynx-audit-20261120`
- `feature/skell-lynx-delay3-in-place-20261121`
- `feature/skell-lynx-limiter-in-place-20261121`
- `feature/skell-lynx-reeverb2-in-place-20261121`
- `feature/skell-small-fx-skull-branding-20261121`

These branch commits already occur in the release candidate's ancestry; no repeat cherry-pick is necessary.

## Diverged historical branches (compared by file content)

| Branch | Assessment and disposition |
| --- | --- |
| `dev/skell-fast-ui-preview` | Preview workflow, Cargo profile and capture script differ; candidate has later integrated versions. Preserve history. |
| `dev/skell-fast-windows-cache` | Original cache proposal not wholly applied; archive its previously missing technical memo. CI cache tuning remains unverified. |
| `dev/skell-local-fast-loop` | QuickView cmd, PowerShell and guide match candidate; separate old smoke workflow has equivalent parse/plan checks in current full preview workflow. Do not duplicate. |
| `dev/skell-ml-audit-rt-20261115` | RT audit instrumentation, manifest, transport and audit memo match candidate; changed app entry point and workflow are later revisions. |
| `dev/skell-ml-first-bootstrap-20261115` | Workflow, script and migration memo are later revisions in candidate; retain old branch, no wholesale replay. |
| `dev/skell-remaining-dialog-branding` | Nine of eleven touched files match candidate exactly; advanced theme and presets differ after later changes. No regression-safe reason to replace them. |
| `dev/skell-verified-upgrade` | Seven of ten touched files match; file-association CI, Studio verification and playlist have later revisions. |
| `dev/skell-windows-association-isolation` | App association/recent/splash and Chop editor match exactly; association workflow and Drop/Slicer editors differ with newer implementation. |
| `dev/skell-windows-exe-build` | Manual Windows EXE workflow changed subsequently; retain integrated current workflow. |
| `feature/skell-studio-integrated-candidate-20261121` | Twenty of twenty-six touched files match byte-identically, including Delay 3 sources and editor identity; remaining workflow, lockfile, license-notice and expansion-doc differences reflect later work and need careful licensing review. |
| `feature/skell-studio-m05-20261120` | Thirty-one of thirty-three touched files match; preset and starter-pack files have newer revisions. |
| `feature/skell-studio-milestone-05-20261120` | Thirty of thirty-four touched files match; old staging workflow and retry-trigger are not production features, and presets/starter pack differ. |
| `feature/windows-release-exe-trigger-20261120` | Its sole changed workflow matches the candidate exactly. |
| `fix/host-link-command-exhaustiveness` | All three touched files match candidate exactly. |
| `fix/quickview-exit-status-four-scenes-20261115` | Six of ten touched files match, including QuickView commands/scripts; app and workflow have newer revisions. |
| `fix/recent-project-error-string-and-pr-build-check` | Recent/splash source files match exactly; verify workflow has newer revisions. |
| `main` | Independent base; one ahead commit/file relating to Fruity Compressor AGPL license. Do **not** overwrite or merge licensing differences without review. |

## Additive consolidation in this audit

1. Preserve historical PR #3's `docs/WINDOWS_BUILD_CACHE.md` as `docs/archive/WINDOWS_BUILD_CACHE_PR3_HISTORICAL.md` with an explicit archival header. The old optimization results remain unverified.
2. Strengthen the existing Windows VST3/CLAP packaging workflow with a SHA-256 manifest of **every** packaged VST3 and CLAP module, an exact `GITHUB_SHA`, and a module-count parity check. The manifest goes inside the existing bundle artifact; plugin code and identities are unchanged.
3. Keep the standalone, now-redundant QuickView smoke and milestone staging workflows on their historical branches rather than create duplicate workflows.

## Existing exact-head CI evidence (pre-audit commit)

At `6614498230e14d4b1ce98d9a6d5bf1d8c3886b51`, the following workflows completed successfully on 2026-11-22: Windows plugin suite `44969256344`; Windows Studio compile / FL FST import and core tests `44969256343`; native UI/QuickView preview `44969256348`; hosted editors `44969256395`; ML staging lint `44969256446`. This proves only their defined CI operations.

Warnings remain on the plugin job: MSVC C compiler-family probe failures in `os-dsp` and EQ/limiter build scripts, deprecated Rust VST3 macro expansions and atomic API usage, and Node `punycode` deprecation. Do not suppress them; address root causes in a separate tested source change.

## Outstanding release gates

Keep PR #22 draft and unmerged. Do not distribute this as a validated stable release without: full FL Studio project save/close/reopen; third-party DAW scanning/editor/automation tests; real audio A/B at 44.1/48/96 kHz; sample-rate/buffer stress and CPU/latency/realtime-allocation audit; and legal review of LYNX-derived AGPL/GPL and source attribution. Latest-head Windows CI success is not a substitute for those checks.

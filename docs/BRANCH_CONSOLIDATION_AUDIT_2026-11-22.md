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
| `main` | Independent base; its one ahead commit refers to the Fruity Compressor AGPL license, but the current license file blob matches the candidate byte-for-byte (`be3f7b28e564e7dd05eaf59d64adba1a4065ac0e`). Preserve history; no license replacement needed. |

## Additive consolidation in this audit

1. Preserve historical PR #3's `docs/WINDOWS_BUILD_CACHE.md` as `docs/archive/WINDOWS_BUILD_CACHE_PR3_HISTORICAL.md` with an explicit archival header. The old optimization results remain unverified.
2. Strengthen the existing Windows VST3/CLAP packaging workflow with a SHA-256 manifest of **every** packaged VST3 and CLAP module, an exact `GITHUB_SHA`, and a module-count parity check. The manifest goes inside the existing bundle artifact; plugin code and identities are unchanged.
3. Keep the standalone, now-redundant QuickView smoke and milestone staging workflows on their historical branches rather than create duplicate workflows.

## Existing exact-head CI evidence (pre-audit commit)

At `6614498230e14d4b1ce98d9a6d5bf1d8c3886b51`, the following workflows completed successfully on 2026-11-22: Windows plugin suite `44969256344`; Windows Studio compile / FL FST import and core tests `44969256343`; native UI/QuickView preview `44969256348`; hosted editors `44969256395`; ML staging lint `44969256446`. This proves only their defined CI operations.

Warnings remain on the plugin job: MSVC C compiler-family probe failures in `os-dsp` and EQ/limiter build scripts, deprecated Rust VST3 macro expansions and atomic API usage, and Node `punycode` deprecation. Do not suppress them; address root causes in a separate tested source change.

## Outstanding release gates

Keep PR #22 draft and unmerged. Do not distribute this as a validated stable release without: full FL Studio project save/close/reopen; third-party DAW scanning/editor/automation tests; real audio A/B at 44.1/48/96 kHz; sample-rate/buffer stress and CPU/latency/realtime-allocation audit; and legal review of LYNX-derived AGPL/GPL and source attribution. Latest-head Windows CI success is not a substitute for those checks.

## Follow-up: latest candidate, VST3-only packaging, and Windows application EXE

The release-candidate branch advanced after the original audit to `aa912eada4bfb85975f786eca9631219887c1455` with VST3-only packaging and a LYNX-style two-bank Skell Beat UI. This is distinct from the previously audited `66144982` source snapshot. The VST3/CLAP verification text in this historical audit describes the earlier state. **The current build packages VST3 only**, without deleting legacy CLAP installations, as detailed in `docs/SKELL_STUDIO_VST3_ONLY_BEAT_LYNX_UI.md`.

Windows executable request: this follow-up enables the **existing** `windows-manual-build.yml` on pushes to draft PR #22's candidate branch. Its command is `cargo build --locked --release -p os-app --bin skell-studio` with `OPENSTUDIO_NO_PLUGIN_BUILD=1`. It is a full Windows x64 Release-mode **engineering app EXE**, not the VST3 bundle, not the `skell-preview` QuickView binary, and not a bundled-plugin installer. The existing PE x64 verification, SHA-256 and artifact manifest stay in place; the workflow now also checks `git rev-parse HEAD` against `GITHUB_SHA` and uses safe concurrency for superseded feature pushes only.

Conflict dispositions stay conservative: older preset/starter-pack and theme variants, different build workflows, and old Skell Beat/LYNX license notices are *not* copied over the newer candidate without compatibility testing. Full FL Studio project reopen, external DAW test, CPU/latency, real audio A/B, and AGPL/legal review remain unverified release gates. A successful GitHub Actions build is engineering evidence, not final release approval.

## Reconciliation update: smooth-editor stage ancestry and release provenance
The two unique commits on `dev/skell-smooth-ui-branding-20261122` were a clean fast-forward from `f394661`; their source is retained as parent history by the next PR #22 commit. Adopted reusable Skell Beat preview cache (72 precomputed thumbnail labels, graph and MIDI previews), stable host-visible vendor branding, and Windows executable regression tests. Removed stage-only branch triggers to avoid duplicate CI. Added host-slot automation-to-graph sync, independent synthetic DSP tests, and exact-head PR build manifests. See `docs/RELEASE_QA_2026-11-22.md` and `docs/BRANCH_RETIREMENT_LEDGER_2026-11-22.md`.

Do not use the earlier synthetic PR merge SHA in build manifests as though it were PR branch head. The actual VST3 baseline manifest had `3231dd965b55af4ddeb17470462076532b4289e5`. New CI asserts `git rev-parse HEAD` against the PR source SHA.

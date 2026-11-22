# Skell Studio branch retirement / preservation ledger — 2026-11-22

Inventory read from GitHub REST: 25 refs (1 main + 24 others). SHAs below are the original branch tips **before** final reconciliation of stage branch into PR #22. They are permanent historical evidence, not instructions to overwrite branches.

**Policy:** Retain `main` and `feature/skell-release-candidate-20261121`. The six prior feature branches listed as `contained` are ancestry-verifiable; `dev/skell-smooth-ui-branding-20261122` becomes ancestry-contained after the planned fast-forward. Other branches have divergent historical content and must **not** be bulk-deleted without file-by-file verification and a durable archival reference.

**This document does not delete any branch.** The current connector does not provide a branch/tag deletion or creation action. A count reduction requires a future authorized maintenance operation after all unique work has been preserved.

| Branch | Pinned historical tip | Disposition before reconciliation |
|---|---|---|
| `dev/skell-fast-ui-preview` | `26f3dca79157c1f691fb072ba4f04f527d3e57c1` | retain for divergent-work review |
| `dev/skell-fast-windows-cache` | `ac2df607416be044e7a92227446b0b805bf365d1` | retain for divergent-work review |
| `dev/skell-local-fast-loop` | `792ac82a280d78bd3d84b4f6ac4c064564b8beba` | retain for divergent-work review |
| `dev/skell-ml-audit-rt-20261115` | `422636885926521ebf7b0f6f89f596c16d6372ab` | retain for divergent-work review |
| `dev/skell-ml-first-bootstrap-20261115` | `f12faeca147ce7a0d301271329cb003bf21d3746` | retain for divergent-work review |
| `dev/skell-remaining-dialog-branding` | `248a0d06cdbe709d0c6e59c7c2d51ef87daf3dcc` | retain for divergent-work review |
| `dev/skell-smooth-ui-branding-20261122` | `bace2eeaeff2520932d61ca59a6ec39c6289c2e9` | contained / ancestry-verified |
| `dev/skell-verified-upgrade` | `e50cd6af0bba9bd4e1fae0462bae6e052abdc549` | retain for divergent-work review |
| `dev/skell-windows-association-isolation` | `dc26af94e21bd4a6fd6fd1f6849d14cdfb421a8e` | retain for divergent-work review |
| `dev/skell-windows-exe-build` | `65b1ea58cda4b6a81cd3e8fc90904b3882022d47` | retain for divergent-work review |
| `feature/skell-beat-curves-midi-20261121` | `ec933330aae4bb8a074b163e975fdc730ffda3c5` | contained / ancestry-verified |
| `feature/skell-fl-fst-presets-lynx-audit-20261120` | `0f56c7ae4f0488b73649a1c0ecd6efd6ad965bc9` | contained / ancestry-verified |
| `feature/skell-lynx-delay3-in-place-20261121` | `5b7592638e575edffd7aa13269e9033659e7c0da` | contained / ancestry-verified |
| `feature/skell-lynx-limiter-in-place-20261121` | `2b6d436b7c0d1a3014aaf881b6ea598a747f0ae5` | contained / ancestry-verified |
| `feature/skell-lynx-reeverb2-in-place-20261121` | `64f9e34feb423bf8132577459cf6f5bb0710727a` | contained / ancestry-verified |
| `feature/skell-release-candidate-20261121` | `f394661251b378a82402f3df75253d3da0f8f848` | active draft PR #22 |
| `feature/skell-small-fx-skull-branding-20261121` | `3f8c5268c48a7005a26311b898bfd45dd5d49d82` | contained / ancestry-verified |
| `feature/skell-studio-integrated-candidate-20261121` | `ad2071cfcd25f4b751bc59ab20965ee2a8231275` | retain for divergent-work review |
| `feature/skell-studio-m05-20261120` | `832fde14c169ad669fc3c0fa1efd2fc56001cc6f` | retain for divergent-work review |
| `feature/skell-studio-milestone-05-20261120` | `92fd3e985b7415841c5288d273f463e4c0220914` | retain for divergent-work review |
| `feature/windows-release-exe-trigger-20261120` | `99d606d69b7503e1b4ede4e59ce823843650c377` | retain for divergent-work review |
| `fix/host-link-command-exhaustiveness` | `5bcff72d150692fc1a6eee3102bacdfb319b499e` | retain for divergent-work review |
| `fix/quickview-exit-status-four-scenes-20261115` | `63f5aaef84464ab2caf960e06f72d181b9ba0c06` | retain for divergent-work review |
| `fix/recent-project-error-string-and-pr-build-check` | `08a280763e5db2547a9535b0618757dc1db1ec95` | retain for divergent-work review |
| `main` | `bc857c1808378410420cdd40d8585b0be6fbfbb4` | protect |

Source comparison details: `docs/BRANCH_CONSOLIDATION_AUDIT_2026-11-22.md`. The original 22 PRs had exactly one open draft (PR #22); closed PRs and historical branches are independent concepts.

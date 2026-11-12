# Skell Studio v0.1 merger checkpoint — 2026-11-12

**Inputs:** two user-supplied source archives (exact files analyzed locally).

| Archive | SHA-256 |
| --- | --- |
| ML-Studio-michael-lane.zip | *(See `docs/merge_provenance.json` inside the combined source package for SHA-256.)* |
| ADE-Studio-michael-lane.zip | *(See `docs/merge_provenance.json` inside the combined source package for SHA-256.)* |

## Strategy

ADE's own `HANDOFF.md` states that it already integrated ML Studio plus newer Lemon Studio code. A byte-level comparison of the uploaded sources found:

- ML ZIP: **9,364** files
- ADE ZIP: **9,641** files
- Same content: **8,990** shared paths
- Differing content: **365** shared paths
- ADE-only: **286** files
- ML-only: **9** files

**Integrated baseline:** ADE source adopted for compiled code and assets, while the 9 ML-only files are preserved in `reference/ml-only/` for manual reconciliation. Blindly overwriting newer code with older mixer/engine sources was deliberately avoided.

The combined package adds the missing root `LICENSE`, `CREDITS_AND_LICENSES.md`, `README.md`, `docs/merge_provenance.json`, a static merge check, and Windows build helpers. The updater startup and UI update trigger are disabled to prevent contact with an external product's update channel. The inherited release automation is disabled and kept only as reference.

## Verification status

- ZIP internal integrity check: **passed**
- Static file structure / merge provenance checks: **passed**
- Rust build and Windows binary: **not run / not built here**
- Real audio / device / DAW workflow tests: **not run here**
- ADE's inherited HANDOFF reported **2,968 passed, 104 failed, 119 ignored** in its own final test run. These are NOT new test results.

## Licensing

The workspace declares `GPL-3.0-or-later`; upstream credit belongs to ItsAlphaNeon/Lemon Studio, Dorfhaus/ML Studio and ADE Studio contributors. Missing third-party notices, AGPL integration, trademarks and other vendored licensing must be audited before commercial distribution. This **cannot automatically be used as a proprietary closed-source DAW**.

## Next steps

1. Import complete source ZIP into this repository with user's local Git client (not uploaded to GitHub by this checkpoint).
2. Run manual Windows build with Rust, MSVC/SDK and LLVM, investigate errors at their root.
3. Test audio playback, microphone recording, save/reopen, MIDI, plugins and export on real Windows hardware.
4. Triage 104 known upstream failures and review license/asset rights.
5. Begin a separate rebranding stage for **Skell Studio**, including black/neon-green UI and SkellCore/SkellForge integration.

**Source package SHA-256:** `1b34ce6228f6a8e71b3cb9854220d35e0afebaddb5b750fecc906e3ee0037779`

**Important:** The repository currently tracks this documentation only. The full baseline source exists in the downloadable artifact provided in the same conversation, not in GitHub yet.

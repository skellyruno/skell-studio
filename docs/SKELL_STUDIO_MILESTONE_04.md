# Skell Studio — Milestone 04: reliability, plugin vault and project protection

Date: November 12, 2026.

## Completed on main

- Fixed a **real Windows compiler error** in bundled `smith` instrument: the host-link `Cmd` enum gained `BrowseSample` and `SampleFit`, which Smith had not handled. These unsupported requests now explicitly return zero instead of removing the instrument or changing the common protocol.
- The Windows build workflow now triggers when Smith or the host-link protocol changes, rather than only on visual edits.
- Rebranded the **plugin picker** code with a Skell skull, dark-green hardware panels, consistent hover/selected states and Skell Studio name. Kept existing plugin search, filter, actions and drag targets.
- Replaced remaining visible AQUILA references in detached editor title bars, recent-files dialog, app version label and file association descriptions.
- Added a dedicated **FLP parser/compatibility CI workflow** that runs after FLP parser/model changes.

## Verified

- GitHub Actions FLP compatibility run `43319605610`: **123 unit tests passed, 0 failed**, and integration tests compiled successfully.
- Theme/profile tests from current Skell verification workflow completed successfully.
- Source files on GitHub were checked after commits.

## Pending validation

- The Windows application `cargo check --locked -p os-app --bin skell-studio` job is still pending at the latest checked point. The old failure from Smith is fixed in source, but this does **not** prove the app builds or works.
- No real runtime FLP import, recording, mixer playback, GUI rendering, MIDI, VST3 or export testing has been completed for the latest fork.
- The starter splash/icon and DSP safety need a real compiled executable for visual/audio verification.

## Separate compatibility safety fix

Pull request [#1](https://github.com/skellyruno/skell-studio/pull/1) holds a **draft**, Windows-registry isolation fix:

- Changes Windows `PROG_ID` from inherited `LemonStudio.LmnProject` to `SkellStudio.LmnProject`.
- Stops claiming/unregistering the inherited `.lemon` extension.
- Preserves the previous `.lmn` association on opt-in install/uninstall.
- Adds a fake Windows registry test to detect tampering with the original DAW's class or extension.

**Not merged while its targeted Windows test is in progress**, so the ongoing mainline Windows build can complete without disruption.

## Next steps

1. Review the remaining Windows compile output; fix each concrete error at the source, without removing bundled instruments.
2. Complete PR #1 test and merge once it is verified.
3. Run a release Windows build and upload a dev artifact (not a commercial release).
4. Validate UI on real Windows + varied DPI, audio recording and playback, live VST3 plugins, and imported FLP copies.
5. Continue replacing inherited UI graphics, maintaining upstream attribution, project compatibility and GPL obligations.

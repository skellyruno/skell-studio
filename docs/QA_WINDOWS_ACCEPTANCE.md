# Skell Studio — Windows DAW acceptance gates

Status: Engineering checklist, not a declaration of passing tests. Updated 2026-11-12.

## Verified by GitHub Actions

- [x] Core Skell theme and settings-profile unit tests (most recent Windows-check workflow's core-tests job).
- [x] Shared playlist/piano-roll/mixer toolbar migration is committed and source-tested (Skellification v3 workflow).
- [x] Skell labeled workspace navigation is committed and source-tested.
- [x] `os-flp` unit tests passed **123/123**, including container round-trip behavior; integration tests compiled (FLP compatibility workflow).

## Build gate — required before a Windows alpha is shared

- [ ] `cargo check --locked -p os-app --bin skell-studio` succeeds in Windows CI.
- [ ] `cargo build --locked --release -p os-app --bin skell-studio` succeeds with all required bundled instruments.
- [ ] Standalone `skell-studio.exe` opens and renders the original Skell icon, splash and workspace.
- [ ] GPL / third-party notices bundled, with source and license links.

The first confirmed compilation blocker was a non-exhaustive host command match in `plugins/smith/src/editor.rs` (BrowseSample / SampleFit). Fixed by explicitly returning 'unsupported' rather than disabling the instrument. The Windows check is being rerun; additional blockers may remain.

## Workflow and UI smoke checks

| Area | Expected acceptance behavior |
| --- | --- |
| UI resize | 800, 1280, 1920, and 2560 px window widths; 100, 125, 150, 200% DPI; no clipped labels or overlapping click targets |
| Toolbar | ARR / KEYS / RACK / MIX / FILES and F5/F7/F6/F9 shortcuts still act on the right windows |
| Playlist | Drag/trim/stretch clips, scroll, zoom, folder tracks, mute/solo, automation, and track-header renaming |
| Piano roll | Draw, paint, move, resize, duplicate and delete notes; velocity lane, scale highlight and keyboard audition |
| Mixer | Select tracks, move faders, pan, mute/solo, audio meter peak/hold, insert/reorder/remove FX |
| Plug-ins | Load a known VST3, save/reopen, automate parameter, and compare rendered audio with a bypass reference |
| FLP import | Open a **copy** of an actual FL Studio project, inspect MIDI/audio/arrangement, resolve missing assets, save to `.lmn`, reopen |
| Recording | Select device and input, record vocal/audio, inspect resulting clip, monitor latency and audible glitches |
| Export | Render 10 s silence, 10 s sine test and real project; measure peak, channels, sample count and duration |
| Startup safety | Fresh profile, existing upstream profile, missing device, missing plugin, corrupt project: no silent data deletion or upstream update prompt |

## Audio metrics (not yet measured)

- PCM is finite for every processed sample (no NaNs/infinities), and no unexplained clipping or denormals under test.
- Transport seek/loop and buffer change must avoid pops/glitches.
- For playback/recording, report actual configured sample rate, buffer length, total reported latency, and observed CPU.
- Test 44.1 kHz / 48 kHz and multiple buffer sizes on Windows hardware; do not promise 1–4 ms latency without measurement.

## Project safety

- Never overwrite an original `.flp` when importing; save Skell work to a separate `.lmn` path.
- Preserve project model, plug-in IDs, and inherited file-format identifiers until migration is deliberately tested.
- Do not reassign old built-in vendor IDs or rename project extensions solely as cosmetic branding.
- Every commercial release must resolve inherited GPL and third-party obligations first.

## Next implementation order

1. Resolve Windows compiler diagnostics without dropping bundled instruments.
2. Produce a dev executable and collect actual screenshots at multiple sizes.
3. Fix layout defects based on real rendering and interaction.
4. Run audio/FLP/VST3 regression matrix and log baseline failures.
5. Replace remaining inherited identity where not compatibility-sensitive, then ship an internal alpha.

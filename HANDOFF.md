# AQUILA — Handoff

Status as of 2026-11-12. This document covers what this project is, what was done, where everything lives, how to build and test it, and what is left.

---

## 1. What AQUILA is

AQUILA is a Rust digital audio workstation made by merging two lines of development:

| Source | Repository | Role in AQUILA |
| --- | --- | --- |
| **ML Studio** (Michael Lane / Dorfhaus) | https://github.com/Dorfhaus/ML-Studio | The base. Its interface, mixer, browser, transport, menus and file format win wherever the two differ. |
| **Lemon Studio** (ItsAlphaNeon and contributors) | https://github.com/ItsAlphaNeon/Lemon-Studio | ML Studio's ancestor. Its newer commits (through November 2026) were merged in, including multi-core mixing and new plugins. |

The merged result was renamed **AQUILA** and published to **https://github.com/ajdohaxhia/ADE-Studio**.

---

## 2. Repository and branches

**Local checkout:** this folder (the one containing this file).

**Git remotes:**

| Remote | URL | Use |
| --- | --- | --- |
| `origin` | https://github.com/Dorfhaus/ML-Studio.git | ML Studio (the fork with 48 own commits) |
| `upstream` | https://github.com/ItsAlphaNeon/Lemon-Studio.git | Lemon Studio |
| `ade` | https://github.com/ajdohaxhia/ADE-Studio.git | **AQUILA — where work is pushed** |

**Branches:**
- The local branch is `michael-lane`.
- Pushes go to both `ade/michael-lane` and `ade/main`. `main` was force-replaced once with the merge, at the owner's request; every push since has been a normal fast-forward.

**Key commits (newest first):**

| Commit | What |
| --- | --- |
| `4b00b476` | Retire Lemon Studio UI tests that AQUILA's ML Studio interface replaces |
| `15e1542e` | Fix more merge regressions: undo, rack, menus, looping |
| `aa976690` | Fix merge regressions in the engine, sampler and mixer |
| `fb9f17aa` | **The merge:** Lemon Studio upstream into ML Studio, rename to AQUILA |
| `0e9cc091` | Last ML Studio commit before the merge (the fork's tip) |

- `fb9f17aa^1` is ML Studio and `fb9f17aa^2` is Lemon Studio, so `git show fb9f17aa^2:<path>` gives upstream's version of any file.
- All 48 Dorfhaus commits and Lemon Studio's history are preserved.

To publish:

```bash
git push ade HEAD:michael-lane HEAD:main
```

---

## 3. Layout of the codebase

| Path | Contents |
| --- | --- |
| `crates/os-app` | The application: windows, editors, browser, mixer, rack, transport, history (undo), menus. Binary target `lemonstudio`. |
| `crates/os-engine` | Audio engine: voices, mixing, multi-core rendering (`engine/multicore.rs`), plugin delay compensation |
| `crates/os-model` | Project model shared by everything (`MixerTrack`, `Channel`, targets in `target.rs`) |
| `crates/os-flp` | FLP / `.lmn` / `.lemon` reading and writing, legacy conversion |
| `crates/os-ui`, `os-theme` | Drawing, widgets, themes |
| `crates/os-host`, `os-clap`, `os-vst2`, `os-vst3` | Third-party plugin hosting |
| `plugins/*` | Built-in instruments and effects (sampler, ML Plasma, Tesla, EQ…) |
| `tools/bundle-app` | macOS `.app` bundler |
| `tools/logo` | Icon generation from `crates/os-app/assets/logo/aquila.png` |
| `docs/assets/aquila/` | README logo and screenshots |
| `README.md` | Rewritten for AQUILA (features, building, shortcuts, credits) |

**Naming notes:**
- `version::NAME` is `"AQUILA"`.
- Built-in plugins are mostly named "ML …"; a few keep their "Lemon …" names (Flanger, Phaser, Vocoder, Drop, the controllers).
- Mixer tracks are named "Bus N" everywhere.
- **Project files:** AQUILA saves `.lmn` (`project_files::EXT`) and opens `.lmn`, `.lemon` and `.flp`.

---

## 4. Building and testing (this machine)

### Why there is a special setup
1. **Paths with spaces break the build.** The original checkout lived under `~/Library/Application Support/...`; the active checkout is now `/Users/ade/Downloads/AQUILA/ML-Studio`. The bundled LAME (autotools/make) and ML Plasma native builds fail on paths with spaces.
2. **ML Plasma's build needs 20 GiB free** before it will compile.
3. **The selected Xcode is on an external drive whose path has spaces.**

### The setup in use
- An external-SSD **sparse disk image mounted at `/Volumes/aquilabuild`** (about 28 GB, no spaces in the path).
- **`/Volumes/aquilabuild/chk.sh`** is the helper used for every build and test. It:
  1. rsyncs `/Users/ade/Downloads/AQUILA/ML-Studio/` (corrected from the previous scratch checkout during continuation) to `/Volumes/aquilabuild/tree` (excluding `.git` and `target`), and
  2. runs cargo there with:
     ```
     DEVELOPER_DIR=/Library/Developer/CommandLineTools PYTHON=python3 CARGO_INCREMENTAL=0
     LEMON_STORAGE_CONFIG=/Volumes/aquilabuild/work/storage.json CARGO_TARGET_DIR=/Volumes/aquilabuild/target
     ```
- **`/Volumes/aquilabuild/work/storage.json`** = `{"minimum_free_gib": 6, "maximum_cache_gib": 0}` lowers ML Plasma's free-space minimum. This is for local runs only and must not go into release builds.
- **`CARGO_INCREMENTAL=0`** was added after the incremental cache (15 GB) filled the image. If space runs out, delete `/Volumes/aquilabuild/target/debug/incremental`.

Examples:

```bash
/Volumes/aquilabuild/chk.sh test -p os-app --bins -- mixer::
```

```bash
/Volumes/aquilabuild/chk.sh test --workspace --lib --bins --tests --no-fail-fast
```

**Gotcha:** the os-app test binary is `--bins` (named `lemonstudio`), not `--bin os-app`.

### Test-comparison helpers (in `/Volumes/aquilabuild/work/`)

| File | Purpose |
| --- | --- |
| `base-sorted.txt` | **Baseline:** the 110 tests that already failed in ML Studio before the merge |
| `newfail.sh <cargo test args>` | Runs tests, prints pass/fail totals and **only failures not in the baseline** |
| `ignore.py "reason" mod::tests::name ...` | Adds `#[ignore = "reason"]` to the named tests (run from the repo root) |
| `last.log` | Output of the most recent `newfail.sh` run |
| `up-mixer.rs`, `fork-mixer.rs` | Upstream / fork copies of `mixer.rs` used during the merge |

---

## 5. What was done

### 5.1 The merge (`fb9f17aa`)
- Lemon Studio's new commits were merged into ML Studio, and conflicts were resolved in favour of ML Studio's interface and behaviour.
- Duplicate definitions from both sides were reconciled (`Sprite`, `MHit::Input`, `Separator`, `DETACHABLE`, `snap_edge`…).
- Upstream's transport and browser were adapted to ML Studio's with shims.
- Rebranded to AQUILA:
  - names and packaging
  - the icon set (`.png`, `.ico`, `.icns`, nightly variants)
  - the splash `ml-splash.png`, generated from `crates/os-app/assets/logo/aquila.png`
- README rewritten with screenshots.

### 5.2 Regression fixes after the merge

**Engine and sampler** (`aa976690`)
- Restored ML Studio's Playlist audio-voice reconciliation when editing during playback. Upstream's block now skips audio-clip voices (`clip_origin`).
- Instruments' extra outputs route to their mixer tracks (`InstOut::aux`, `instruments_out`).
- Lemon Sampler: the `on` switch keeps its live value; per-note drive is implemented (`Voice::drive`, `note_drive`).
- Result: all 426 engine tests pass, and 80/80 sampler tests pass.

**Mixer** (`aa976690`)
- `slot_cpu` was 10 entries long while slots are 64, which crashed on slots past 9. Fixed in `os-model`, `os-flp` and the engine's `slot_smart`.
- Track moves remapped automation twice (ML Studio's `retarget` plus upstream's `follow_places`), which swapped targets back. Only the upstream pass and ML Studio's peak-link remap remain.
- Restored upstream track-menu commands inside ML Studio's menu: rename, random/gradient colour, icons, audio-track assign, routing, select, groups, move, insert one/multiple/from state, File ▸ states, audio editor.
- Help pages for "ML …" plugins (`help_links::plugin_page`).

**Undo, rack, menus** (`15e1542e`)
- **Plugin-window gestures:** ML Studio's per-value undo and Lemon Studio's per-gesture undo both ran. The per-gesture one is kept (`params.rs`).
- **Duplicate steps removed:** mixer colour and track moves recorded twice. Single moves are now named "Move mixer track".
- **Channel delete via the D key** now goes through `rack::delete_step`, so it is undoable.
- **`State::rows_moved` is consumed again** in `plugin_tick`. Undo/redo brings removed instruments back, including instrument-stack children.
- **Remote-control links follow moved channels** (`rack::reorder` calls `remote::follow`).
- **Advanced looping:** loop points play, save and burn (`row_loops`, `write_loops`).
- **"Color selected ▸ All"** works (`patterns::apply_swatch`).
- **Rack menus:** the channel menu gained Change icon, waveform, Wrap in container and Assign to new instrument track. The options menu gained "Selected channels ▸" and "Options ▸" submenus, keeping ML Studio's compact top level of 10 entries or fewer.
- **Playlist options:** Quantize song jumps and the performance-mode entries are placed again. The "Reset empty tracks" label is fixed.
- **Collaborate** got a real `MenuCmd`. The stray "MIDI chords" placeholder is removed.
- **Other:**
  - Saved-by text is "AQUILA x.y.z".
  - Vocoder plugin category.
  - `LINKED_EFFECTS` order matches the built-ins.
  - "Bus N" naming in the rack and mixer views.

**Tests adapted rather than code changed** (where ML Studio's design deliberately differs):
- the insert-multiple form
- the reset confirmation
- the typing keyboard being off by default
- the TRACK box doing free-drag routing
- the categorised plugin menus
- the in-app Change Log instead of "What's new"

### 5.3 Retired tests
About 50 Lemon Studio tests check UI that AQUILA replaces with ML Studio's. They are marked `#[ignore = "<reason>"]`, so they are easy to find and re-enable.

| Reason | Tests |
| --- | --- |
| Group fader semantics | 7 tests in `history.rs` |
| Animations | ripples, fades, captions, popup timing |
| Browser and drag tag | drag-tag tests, browser rows, channel-states browse, init browser |
| Windows | `win_clamp`, detach tests, detached editor layout, designer fonts |
| Plugin picker and palette | picker previews, palette, plugin manager |
| Rack view layout | 2 tests in `rack.rs` |
| Containers | 2 tests: ML built-ins saved as CLAP references need installed bundles; ML Studio's browser rows |
| ML Studio test replaced | `params::forum_editor_gesture_undo_redo`, covered by `control::tests::edits_in_a_plugin_s_window_are_undo_steps` |

To list them all:

```bash
grep -rn '#\[ignore = ' crates plugins tools
```

---

## 6. Test status before continuation

**Full workspace** (`--workspace --lib --bins --tests`):
- 2941 passed, 129 failed, 119 ignored.
- Of the 129 failures, **104 were already failing in ML Studio** (the baseline) and **25 are new** since the merge, down from 134.

### 6.1 The 25 remaining new failures

**Recording (likely a real bug, investigate first)**
- `smart_recording::tests::smart_mode_arms_without_a_question_and_writes_automation_clips`
- `record::tests::records_mixer_controls_the_tempo_and_plugin_parameters`
- `record::tests::takes_the_clip_just_before_in_song_mode`
- `record::tests::option_buttons_menus_and_keys` (expects Count-in length / Pre-roll entries)
- `track_rec::tests::an_armed_track_records_into_the_playlist`

**What's known about the recording failures:**
- `record.rs`, `smart_recording.rs`, `mixer_recording.rs`, `track_rec.rs` and `prefs.rs` are **identical to ML Studio's**, and so is the `Cmd::Record` handler in `playlist.rs`.
- Test preferences are thread-local (`settings_file.rs`), so this is not leakage between tests.
- `Cmd::Record(Opt::Automation)` toggles automation **off** (it defaults to on), yet ML Studio's test expects it on after `toggle_record`. In ML Studio's baseline this test passed, so something reached through `App::new` or `toggle_record` in merged code (app.rs or the transport) must behave differently.
- `records_mixer_…` also shows a recording making two undo steps: "Recorded items" plus "Create automation clip" from smart recording.

**App (os-app)**
- `app::tests::tempo_and_time_settings_are_undo_steps`: dragging the tempo display doesn't change the tempo in ML Studio's transport layout (`tempo_digits` hit area). This could be a UI difference or a bug.
- `app::tests::alt_click_resets_controls_without_dragging`: Alt+click gives 0.8, expected 0.23 (reset value).
- `app::tests::selection_fixes_mixer_reset_and_group_solo`: one value 0.8 vs 0.95.
- `app::tests::piano_ctrl_drag_selects_in_draw`: Ctrl+drag in Draw mode.
- `app::tests::main_menu_titles_switch_on_first_press`: expects an enabled "Undo history" entry.
- `app::tests::settings_tabs_can_be_hidden_and_restored_from_visibility`: an extra tab index (2) in the tab order.
- `app::tests::channel_rack_window`, `app::tests::rack_preview_mute_button_and_preview_under_the_steps`: rack window layout; probably UI differences.
- `init::tests::create_automation_clip_lists_the_control_while_the_check_is_on`: the "Automation" rack group or filter.
- `playlist::tests::renaming_a_channel_clip_renames_its_channel`: the rename box targets `AudioClip(0)` instead of `Rack(Channel(1))`.
- `playlist::tests::slot_window_menu_entries`: a plugin-window menu entry (`Plugin(Picker(Fx(Some(2))))`).
- `remote::tests::view_remote_control_lists_the_links`: the VIEW menu's Remote control entry.

**Other crates**
- `editor::tests::drum_loop_stretch_and_noise_gating`, `editor::tests::time_stretch_and_pitch_shift` (`plugins/tesla`)
- `editor::tests::tab_wheel_moves_a_step_per_notch_and_adds_up_fractions`, `editor::tests::the_knob_captions_stay_clear_of_band_1s_knobs` (`plugins/lemon-parametric-eq-2`)
- `editor::tests::the_newer_pages_wordmarks_end_6_px_from_the_right_edge` (`plugins/lemon-small-fx`; the wordmark is still "Lemon Filter")
- `project::legacy::tests::an_old_file_opens_and_saves_converted` (`crates/os-flp`): legacy file conversion; worth checking, because it affects opening old projects.
- `tests::a_nightly_is_an_app_of_its_own` (`tools/bundle-app`): Info.plist / `CFBundleIconName` after the rename.
- `tests::paints_text_and_shapes` (`crates/os-ui`): text width assertion (`w > 40.0`).

### 6.2 Suggested order
1. Recording (section 6.1): the most likely user-facing bug.
2. `project::legacy::...` (opening old projects).
3. `tempo_and_time_settings_are_undo_steps`, `alt_click_resets...` and `renaming_a_channel_clip...`, which may be real.
4. Bundle-app nightly test (packaging).
5. The rest: decide case by case whether to fix the code, adapt the test to ML Studio's design, or retire it with `ignore.py` and a reason.

---

## 7. Merge patterns to watch for

These caused most of the regressions, so check for them when touching merged code:

- **Double handling:** ML Studio and Lemon Studio each added their own mechanism for the same thing (undo recording, target remapping, gesture handling). Look for a fork-side `history::record` or remap inside code that upstream already wraps with `control::edit_app` or `follow_places`.
- **Lost glue:** functions that exist but are never called after the merge, such as `rack::moved_link` and the consumer of `State::rows_moved`. A quick check is `grep` for definitions with no callers.
- **Menus:** the merge usually kept ML Studio's menu builders and dropped upstream entries. Compare with `git show fb9f17aa^2:<file>`.
- **Array sizes:** ML Studio has 64 mixer slots; some upstream code assumed 10.
- **Renames:** tests and lookups still using "Lemon X" names, `.lemon` extensions, or "Insert N" instead of "Bus N".

---

## 8. Open decisions for the owner

- Whether to keep or re-enable any retired Lemon Studio UI tests, which would mean porting those UI features into ML Studio's interface.
- Whether the "Bus N" naming (now everywhere) is wanted, or ML Studio's earlier "Insert N" in the rack and mixer views.
- `HANDOFF.md` itself is **not committed yet**. Commit it, or keep it local.


## 9. Continuation on 2026-11-12

This section supersedes the remaining-work list in section 6. Changes below are local and uncommitted; nothing has been pushed.

### Fixes and test updates

- Corrected `/Volumes/aquilabuild/chk.sh` to sync the active Downloads checkout instead of the previous scratch checkout.
- Recording: audio clips honor their explicit/inherited mixer route; smart capture no longer also writes pattern controls. Remote faders, plugin parameters and container levels use smart capture, including Start on input. Clip extension preserves ML Studio's gap behavior but avoids occupied rows, including assigned instrument lanes.
- Grouped mixer fader resets preserve every changed fader in one undo step; resetting default values adds no step.
- Restored EDIT → Undo history and Current project → Remote control. Project data remains immediately before Samples.
- Legacy imports that are unchanged after conversion preserve the event stream rather than append default-only native chunks.
- Nightly packaging uses AQUILA Nightly as the display name and has one microphone permission key; its test checks the actual ICNS icon setup.
- Restored Tesla's right-click Drum Stretch action and fractional wheel stepping over the modern EQ type selector. Fixed effect wordmark right alignment.
- Adapted tests for intentional ML Studio behavior: tempo drag speed/scaling, individual audio-clip names, instrument palette, rack preview/LED arrangement, additive piano selection, Recording settings tab, categorized effect menus, template rack groups, recording filters/gap semantics, and modern plugin geometry. No new ignored tests were added.
- Made Chop host-audio tests initialize at their explicit 48 kHz render rate instead of inheriting the machine's 44.1 kHz device rate.
- New integration tests cover smart plugin automation without duplicate pattern controls and remote fader/container recording, including Start on input and disabled filters.

### Verification

The first full workspace run reported 2964 passed, 107 failed, 119 ignored. All original 25 listed merge failures were resolved. The three additional nonbaseline failures were a browser section ordering assertion (corrected) and two Chop host tests being investigated for a device-rate/fixture-rate mismatch. Final verification is recorded below.


Final full workspace command:

```bash
/Volumes/aquilabuild/work/newfail.sh --workspace --lib --bins --tests --no-fail-fast
```

**2968 passed, 104 failed, 119 ignored. Zero failures outside the pre-merge baseline.** All 25 original remaining merge failures are resolved. The final run includes the new remote/container/plugin recording tests, assigned-lane collision coverage, and explicit-rate Chop tests. No new ignores were added. The 104 pre-existing failures remain; coverage percentage was not measured. `git diff --check` passes. Code review found no remaining actionable issues after the recording follow-ups.

Final log: `/Volumes/aquilabuild/work/last.log`. First continuation full run: `/Volumes/aquilabuild/work/continuation-first.log`. Chop rate diagnostic: `/Volumes/aquilabuild/work/chop-red.log`.

Next work, if requested: investigate the 104 baseline failures; do not classify them as unresolved merge regressions. Review/commit the local changes and this handoff when desired. No commit or push was performed during continuation.

# Skell Studio — Milestone 05: source audit, rebrand cleanup and Ember theme

Date: November 15, 2026.

**Verification status: static only.** This pass was done by reading and editing source in an environment with no Rust
toolchain and no network. Nothing here was compiled, no tests were run, and no screenshots were taken. Every change is
deliberately low risk (display strings, comments, colour constants, one theme data file, docs), and a mechanical diff
check confirmed the Rust edits changed only string contents or comments, plus one intended test expression. The next
Windows CI run and `cargo test` are the real gate.

## Fixed

- **Test regression from the rebrand.** `app::tests::flps_migrate_to_lemon_files` asserted the saved-by text
  `"AQUILA {version}"`, but the app writes `"{version::NAME} {version}"` (now `Skell Studio …`). The test now builds its
  expectation from `version::NAME`. This is a rebrand regression, not part of the 104-test upstream baseline: ADE's run
  passed it while `NAME` was still "AQUILA".
- **User-visible leftover names** (Milestone 04 said these were replaced; many were not). Changed to "Skell Studio" in
  file-dialog filters (projects, presets, toolbar presets, themes, controls, effect chains, channel states), the
  "Project not supported" dialog (which also said "Lemon Studio"), preset/zip/import error messages, the import report
  header, plugin-preview report header, bug-report diagnostics header, starter-pack button, vintage toolbar title and
  exit hint, mixer/container/collaboration/vocal/plugin-stopped messages, WASAPI/ASIO format errors, the macOS menu
  titles, and the default `Skell Studio Controls.json` file name. "a AQUILA preset" also had a grammar error.
- **Stale theme documentation.** `os-theme/src/design.rs` still described a blue brand colour and old grey values; the
  comments now describe the green palette. Stable identifiers (`Lemon` key, `lemon` module) are unchanged.
- **Broken doc link** in `docs/releasing.md` (the release workflow was moved to `docs/upstream-workflows-disabled/`).
- **README status** now matches the verified-checkpoint doc instead of saying nothing had been compiled.

## Visual pass

- **Ember theme** (`crates/os-theme/themes/Ember.flstheme`, registered in `presets.rs`, preset count 23 → 24): neutral
  black chrome with FL-style orange selection/highlight and warm neutral step pads. It is Graphite's tested
  surface/text settings with only the key colours and step pads changed, so it follows the same paths the existing
  theme tests already exercise (`themes_colour_lemon`, `built_in_themes_read`, `presets_parse_and_differ`).
- **Three more built-in themes** (preset count now 27), each derived from an existing theme that passes the theme
  tests, changing only key colours and step pads: **Skell Green** (exact brand colours as a theme, so the Skell accent
  is available on the Console skin), **Steel** (cool slate-blue) and **Bone** (light, green accent).
  `skell_themes_keep_their_key_colours` pins them.
- **Step pads:** the Skell `step_b` group is a little lighter (`#3a4e3a`) for clearer 4-step grouping; still under the
  0.09 chroma threshold, so it keeps the same retint path.
- **Ember guard test** (`presets::tests::ember_is_orange`) pins Ember's decoded key colours, so a byte-order slip
  (selection/text are RGB, highlight/mute/steps are COLORREF) fails loudly instead of silently turning it blue.
- **Palette consistency.** Five leftover neutral or blue-grey tokens in the Skell skin were tinted to match the green
  palette at the same lightness: `menu_drop_disabled`, `menu_pop_dim`, `menu_pop_head_text`, `menu_pop_shortcut`,
  `key_c`.
- The **Console** skin (the measured FL-style default) and Skell chrome are separate: every `skell_chrome` call is gated
  on the Skell skin and not on vintage mode, so choosing Console or a theme does not pick up Skell painting.

## Feature request status (FL-style list)

Done: more built-in themes; step-pad contrast. Already present in the code (found, not seen running): Pattern/Song
transport (`transport.rs`), scale and chord tools (`harmony.rs`), browser favorites and preview (`browser.rs`,
`browser_preview.rs`), clip mute handling (`playlist.rs`), recessed tempo box (`tempo_box` token). Not done because
they need new logic or UI and a compiler/screenshots: theme-picker grouping, per-type clip colours (the palette has a
single `clip` token), meter peak-hold (no peak-hold state exists), selected-strip outline changes, ghost notes from
other channels, chord-stamp tool changes.

## Found but deliberately NOT changed (needs an owner decision)

- **Still wired to the upstream author.** The Donate button opens `paypal.me/michaellanemusic` (`donate.rs`); the bug
  report button opens `michaellanemusic.de/lemon-studio/features/feedback.html` (`feedback.rs`, with diagnostics in the
  URL fragment); the optional vocal-tuning engine downloads from `michaellanemusic.de` (`vocal/engine.rs`); in-app help
  links go to `getlemonstudio.app/wiki`. The updater is confirmed not started (`main.rs`), but its manifests still point
  upstream. Decide the Skell destinations before any public build. `feedback.rs` has a test that forbids `?` in the URL,
  so a GitHub issue-form prefill URL would need that test adapted.
- **Linux desktop entry:** only its display `Name=` was changed. `Exec=`, `Icon=` and `StartupWMClass=` still say
  `lemonstudio` because `main.rs` and the packaging scripts match that app id; `scripts/apply_skell_asset_links.py`
  is the planned rename. `packaging/github-release.json` is a stale upstream release manifest (upstream download
  URLs) consumed only by the disabled publish workflow.
- **Identifiers still say AQUILA/Lemon on purpose:** plugin vendor `AQUILA` (`plugin_db::BUILTIN_VENDOR`, matched by
  `master_assistant.rs`), template folder `AQUILA/Lemon Starter`, temp/skin/update folder names, the control-plane `app`
  field, the MIDI client name, and every updater message. Renaming these changes saved projects, paths or protocols and
  needs a compatibility migration first.
- **`app::tests::settings_window`** asserts hints `"AQUILA has no MIDI scripts"` and `"… performance mode"`. The code says
  `"Skell Studio has no MIDI scripts yet"`, and `NO_PERFORMANCE` in `views/settings.rs` is never attached to a control,
  so this test cannot pass as written and was not touched. It most likely predates the rebrand (baseline).

## CI coverage gap

Workflows run `cargo test` only for `os-theme`, `os-paths`, `os-flp` and `os-app`'s `file_assoc` tests; every other
`os-app` test is never run on push. That is how the saved-by test regression above went unnoticed, and it means
Ember's app-level readability check (`theme_library::tests::built_in_themes_read`, both skins) is not CI-gated even
though its `os-theme` tests are. Run it locally, and once the 104-test baseline is triaged consider adding targeted
`cargo test -p os-app --bin skell-studio <test path>` steps for the tests that guard the rebrand and themes.

## Next

1. Push and run the Windows compile gate and `cargo test -p os-theme -p os-app` (theme tests matter most: Ember).
2. Look at Ember on the Skell skin and on Console at 100/125/150% DPI.
3. Decide donate / feedback / help / vocal-engine destinations.
4. Triage the 104 baseline failures, starting with `settings_window`.

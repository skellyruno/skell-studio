# Mouse and keyboard controls

Settings > Controls stores bindings in the current profile. No custom bindings are enabled by default. Select the editor context, input event and modifiers, then search and click an action. Capture records a keyboard key; Escape cancels capture. A binding replaces that exact context/event/modifier combination. More specific Piano Roll note/grid contexts inherit Piano Roll bindings, then Global. Restore event removes the override and restores this inheritance. Do nothing explicitly suppresses an event.

Supported editors are Piano Roll, Playlist, Mixer, Channel Rack and Browser. Mouse bindings apply to editor surfaces and controls, not window close buttons or text fields. Keyboard bindings use the active editor; the docked Browser follows its existing pointer-based key routing. Dialogs and focused text fields keep their input. Typing-keyboard MIDI retains precedence over plain note letters; turn it off to bind those letters to editing commands. Ctrl means Command on macOS; Alt means Option.

Clicks and drags are independent. A 5-pixel travel threshold starts a drag. Unassigned gestures use native behavior. The initiating button owns its gesture; another button cannot release it. Escape cancels a deferred click; after a native drag starts, Escape finishes it without closing the editor, and normal Undo remains available. The normal pointer position is preserved. Left double/triple-click bindings defer a single click for 350 ms; without such bindings, ordinary click/drag dispatch has no multi-click delay. Hover fires once after 500 ms of dwell on a target. Activate window fires on a subsequent editor activation, not initial application startup. These triggers only operate when assigned.

Actions use the current selection. Explicit note actions cover insertion at the pointer, pitch/time movement, velocity and horizontal/vertical zoom. Existing menu commands are resolved against current application state when invoked; unavailable commands report that they cannot run. Mixer volume respects locked channels and existing grouped fader behavior. Repeated keys repeat direct movement/velocity/zoom actions; toggle and menu commands fire once per press.

Append creates an ordered sequence of up to 32 direct editing actions. These sequences undo/redo as one action, including with a one-step undo-history limit. Dialog, file, project-management and undo/redo commands run alone. Mouse-down gestures also run alone and cannot be assigned to keyboard/wheel/hover triggers. This avoids stuck mouse buttons and partial asynchronous sequences.

Saved opens the binding list for editing. Give a binding an action name and Save name to reuse it from Saved actions in the action search. Reuse retains the source event/context identity when renamed. Circular or missing references are rejected; restoring a referenced source binding makes its consumers unavailable until repaired. Pin as toolbar button adds Custom actions to the toolbar layout, with up to four named buttons. The buttons use the action's first two characters and full-name tooltips. The vintage FL theme includes them in its toolbar. Workspace Visibility can hide Controls settings and Custom action buttons independently without deleting bindings or project data. Existing toolbar layout/visibility choices still apply.

Undo change restores up to 32 previous binding-settings edits in the current session. Export writes the profile's controls as JSON. Import replaces bindings only after validating the complete file; Undo change can reverse an import. JSON exports preserve names, sequences and pinned state. Unknown action IDs from another version remain stored but report unavailable when invoked. Native menu action IDs use their original menu paths, independent of user menu customization; future menu renames require migration.

## Portable action scripts

Import also accepts UTF-8 `.lsa` action scripts. Select the target context/event (and capture a key if using Keyboard) before importing. The script becomes a named, editable action sequence assigned to that event. Syntax:

```text
# Raise selected notes by a fifth, then reduce velocity one step.
name: Fifth softer
repeat 7 piano.up
piano.velocity-down
```

Each nonempty line is an action ID, a `name:` directive, a comment beginning with `#`, or `repeat COUNT ACTION_ID`. The expanded script is limited to 32 direct editing actions. Action IDs are visible in a JSON export. Scripts cannot execute shell commands, access files, run arbitrary Lua/JavaScript, wait asynchronously or recurse into named actions. This is a finite action scripting format, not a general-purpose scripting API or a claim of complete REAPER compatibility. Native gamepad/HID mapping is not included; controllers exposing keyboard events can use the keyboard bindings.

## Verification

Focused synthetic app tests cover click/drag fallback and cancellation, button ownership and modifier restoration, text input guards, per-context inheritance, Mixer lock protection, note sequence undo/redo under a one-step limit, settings capture/search/restore, import/export validation, named actions/cycle rejection, visibility, finite script parsing/import, multi-click suppression, hover/activation and editor surfaces. Canvas renders check the Controls layout at full and small window sizes. Live mouse/audio behavior remains a hands-on user check.

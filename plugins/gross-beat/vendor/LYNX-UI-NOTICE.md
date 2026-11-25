# Skell Beat native editor — LYNX UI adaptation

Upstream: [LYNX SHARE](https://github.com/Ka1-S/LYNX-SHARE), release 0.1.30, pinned revision `88ec555ea2bce9714c92707cc06a5abfd317c900`.

Original references: `src/core/PluginLayout.cpp::grossBeat()`, `src/ui/GrossBeatEditor.inc` (4×9 Time/Volume slot grids, individual envelope thumbnails, tab strip, beat/level graph, snap selector, selected point and slot controls) and `src/core/GrossBeat.{h,cpp}` (the already integrated DSP, state parser and 72 slot maps).

Adaptation: `plugins/gross-beat/src/editor.rs` reimplements these LYNX editor structures in Skell's native Rust Canvas / nih-plug view instead of JUCE classes, and `plugins/gross-beat/vendor/core/bridge.cpp` exports a batched editor-thread thumbnail/name bridge. The existing VST3 class ID, eight automation parameter IDs, 72-slot preset encoding and audio engine remain unchanged. Time/Volume 4×9 banks, previews, tabs, snap and point controls are ported; some LYNX advanced context menus, keyboard shortcuts and transport/playhead animation are not yet implemented.

Source remains AGPL-3.0-only. See `plugins/gross-beat/LICENSE-AGPL-3.0.txt` and repository third-party notices before redistribution. No Image-Line DLL, proprietary artwork, factory preset assets or audio were copied.

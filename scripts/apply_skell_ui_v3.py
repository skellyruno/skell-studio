#!/usr/bin/env python3
"""Unify tools across Skell Studio's arranger, keys and mixer.

This script changes painting only; tool IDs, action targets and DSP remain untouched.
It deliberately fails on unknown upstream versions to avoid partial edits.
"""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

PATCHES = {
"crates/os-app/src/views/playlist.rs": (
"""    if super::lemon(ui) {
        if on {
            lk::lit_in(ui.c, ui.t, b.outset(1.0), lk::r_btn(t), hov);
        } else if hov {
            ui.c.fill_rrect(b.outset(1.0), lk::r_btn(t), t.button_hi);
        }
        return if on { lk::ink_on_lit(ui.t) } else { t.glyph };
    }""",
"""    if super::lemon(ui) && !t.style.vintage_2005 {
        return super::skell_chrome::tool(ui, b, on, hov);
    }""",
),
"crates/os-app/src/views/piano.rs": (
"""        } else if super::lemon(ui) {
            // Lemon: the active tool is a lit rounded square with a dark glyph.
            let bg = b.outset(1.0);
            if on {
                lk::lit_in(ui.c, ui.t, bg, lk::r_btn(t), hov);
            } else if hov {
                ui.c.fill_rrect(bg, lk::r_btn(t), t.button_hi);
            }
            if on { lk::ink_on_lit(ui.t) } else { t.glyph }
        } else if on {""",
"""        } else if super::lemon(ui) && !t.style.vintage_2005 {
            // The same hardware button is used by the playlist and mixer.
            super::skell_chrome::tool(ui, b, on, hov)
        } else if on {""",
),
"crates/os-app/src/views/mixer.rs": (
"""        ui.c.fill_rrect(b,3.0,if active {ui.t.accent} else if hover {ui.t.button_hi} else {ui.t.s(0x292f32)});
        let ink=if active {ui.t.s(0x182024)} else {ui.t.text};""",
"""        let ink = if super::lemon(ui) && !s.vintage_2005 {
            super::skell_chrome::tool(ui, b, active, hover)
        } else {
            ui.c.fill_rrect(b, 3.0,
                if active { ui.t.accent }
                else if hover { ui.t.button_hi }
                else { ui.t.s(0x292f32) });
            if active { ui.t.s(0x182024) } else { ui.t.text }
        };""",
),
}

def main():
    proposed = {}
    for rel, (before, after) in PATCHES.items():
        file = ROOT / rel
        data = file.read_text(encoding="utf-8")
        if data.count(before) != 1:
            raise SystemExit(f"Refusing to patch {rel}: found {data.count(before)} source anchors")
        if data.count(after) != 0:
            raise SystemExit(f"Refusing to patch {rel}: target code already present")
        proposed[file] = data.replace(before, after, 1)
    # Write only after ALL source files pass verification.
    for file, body in proposed.items():
        file.write_text(body, encoding="utf-8")
        print(f"Skell tools unified: {file.relative_to(ROOT)}")
    print("Actions, keyboard shortcuts and serialized projects left unchanged.")

if __name__ == "__main__":
    main()

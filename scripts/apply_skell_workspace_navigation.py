#!/usr/bin/env python3
"""Make the Skell Studio top-band workflow navigation distinct and readable.
All existing F-key mappings, Action handlers and clickable rectangles stay unchanged.
"""
from pathlib import Path
p = Path(__file__).resolve().parents[1] / "crates/os-app/src/views/toolbar.rs"
s = p.read_text(encoding="utf-8")
changes = [
("""        let ink = if super::lemon(ui) && !c.s.vintage_2005 {
            if hov || !c.s.toolbar.flat {
                studio_tile(ui, r.inset(1.0), if hov { ui.t.button_hi } else { ui.t.toolbar.mix(ui.t.button, 0.45) });
            }
            if hov { ui.t.accent } else { ui.t.glyph }
        } else {
            super::tool_face(ui, r, if hov { super::Look::Hot } else { super::Look::Off })
        };""",
"""        let ink = if super::lemon(ui) && !c.s.vintage_2005 {
            super::skell_chrome::tool(ui, r.inset(1.0), false, hov)
        } else {
            super::tool_face(ui, r, if hov { super::Look::Hot } else { super::Look::Off })
        };"""),
("""        } else {icons::draw(ui.c, icon, Rect::new(r.x + 5.0, r.y + 6.5, 20.0, 20.0), ink);}""",
"""        } else if super::lemon(ui) {
            // This is the first-party Skell Studio workspace switcher.
            // The icon and text share the original 30x33 click target.
            let names = ["ARR", "KEYS", "RACK", "MIX", "FILES",
                         "PROJ", "PLUG", "TAP", "TOUCH"];
            icons::draw(ui.c, icon,
                Rect::new(r.x + 7.0, r.y + 3.5, 16.0, 16.0), ink);
            ui.c.text_center(
                Font::condensed(8, 4),
                Rect::new(r.x + 1.0, r.y + 21.0, r.w - 2.0, 10.0),
                if hov { ui.t.accent } else { ui.t.text_dim },
                names[i],
            );
        } else {
            icons::draw(ui.c, icon,
                Rect::new(r.x + 5.0, r.y + 6.5, 20.0, 20.0), ink);
        }"""),
]
for a,b in changes:
    if s.count(a) != 1:
        raise RuntimeError(f"Expected one original anchor, found {s.count(a)}: {a[:60]}")
    s=s.replace(a,b,1)
p.write_text(s,encoding="utf-8")
print("Skell Studio workspace buttons: ARR / KEYS / RACK / MIX / FILES")

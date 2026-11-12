#!/usr/bin/env python3
"""Idempotent, fail-closed Skell Studio v2 visual refactor.

Only paint and layout code changes. No model edits, DSP, audio routing or session serialization.
Requires exact known upstream anchors; refuses a partially applied or diverged source tree.
"""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
changes = {}

def patch(rel, pairs):
    path = ROOT / rel
    source = path.read_text(encoding="utf-8")
    result = source
    for needle, replacement in pairs:
        if result.count(needle) != 1:
            raise RuntimeError(f"{rel}: expected exactly one anchor, found {result.count(needle)}: {needle[:70]}")
        result = result.replace(needle, replacement, 1)
    changes[rel] = (source, result)

def main():
    patch("crates/os-theme/src/design.rs", [
        ('("note_bottom", c(0x1685dc))', '("note_bottom", c(0x3e9b3c))'),
        ('("key_black_lit_left", c(0xa88f25))', '("key_black_lit_left", c(0x357c21))'),
        ('("key_white", c(0xededed))', '("key_white", c(0xcddaca))'),
        ('("key_white_left", c(0xd2d2d2))', '("key_white_left", c(0xa5b4a3))'),
        ('("key_black", c(0x414141))', '("key_black", c(0x152017))'),
        ('("key_black_left", c(0x2c2c2c))', '("key_black_left", c(0x0b120d))'),
    ])
    patch("crates/os-app/src/views/playlist.rs", [
        ('CLIP_BORDER => Color::rgb(0x1e1e1e),', 'CLIP_BORDER => Color::rgb(0x1e3425),'),
        ('TITLE_TOP | TITLE_BOTTOM => Color::rgb(0x5c5c5c),', 'TITLE_TOP | TITLE_BOTTOM => Color::rgb(0x30513a),'),
        ('SEL_BODY => Color::rgb(0x2e2810),', 'SEL_BODY => Color::rgb(0x112818),'),
        ('MUTED_TITLE => Color::rgb(0x2b2b2b),', 'MUTED_TITLE => Color::rgb(0x1c2a20),'),
        ('MUTED_TEXT => Color::rgb(0x767676),', 'MUTED_TEXT => Color::rgb(0x819181),'),
        ('DEFAULT_BODY => Color::rgb(0x1c1c1c),', 'DEFAULT_BODY => Color::rgb(0x101a12),'),
        ('NOTE => Color::rgb(0xd6d6d6),', 'NOTE => Color::rgb(0xb5ebb0),'),
        ('WAVE => Color::rgb(0xe6e6e6),', 'WAVE => Color::rgb(0xc6efc0),'),
        ('''    ui.c.fill_rect_gradient(hr, top, bottom, k);
    if t.style.vintage_2005 {''',
         '''    // Skell track cards are machined, low-glare panels with a selection rail.
    // All hit regions, drag handles and group geometry remain unchanged.
    if super::lemon(ui) && !s.vintage_2005 {
        super::skell_chrome::panel(ui, hr, s.playlist.header_selection.contains(&row), colored);
    } else {
        ui.c.fill_rect_gradient(hr, top, bottom, k);
    }
    if t.style.vintage_2005 {'''),
        ('''        if lead {icons::draw(ui.c,Icon::Folder,Rect::new(hr.x+2.+if short{15.}else{0.},hr.y+3.,13.,13.),text);}
        ui.c.text_clip(Font::new(16), hr.x + 2.0+indent, hr.y + 1.0, name_w-indent, text, &pl::track_name(s, row));''',
         '''        if super::lemon(ui) && !s.vintage_2005 && !short && hr.w > 86.0 {
            // Clear track numbering; leave the lower controls and folder indicator clickable.
            let number = Rect::new(hr.x + 5.0, hr.y + 4.0, 23.0, 16.0);
            ui.c.fill_rrect(number, 3.0, if lead { t.button_hi } else { t.sunken });
            ui.c.text_center(Font::new(10), number,
                if s.playlist.header_selection.contains(&row) { t.accent } else { t.text_dim },
                &format!("{:02}", u32::from(row) + 1));
            if lead {
                icons::draw(ui.c, Icon::Folder,
                    Rect::new(hr.x + 31.0, hr.y + 5.0, 12.0, 12.0), text);
            }
            let label_x = hr.x + if lead { 46.0 } else { 33.0 };
            ui.c.text_clip(Font::new(13), label_x, hr.y + 5.0,
                (hr.right() - label_x - 24.0).max(0.0),
                text, &pl::track_name(s, row));
        } else {
            if lead {icons::draw(ui.c,Icon::Folder,Rect::new(hr.x+2.+if short{15.}else{0.},hr.y+3.,13.,13.),text);}
            ui.c.text_clip(Font::new(16), hr.x + 2.0+indent, hr.y + 1.0, name_w-indent, text, &pl::track_name(s, row));
        }'''),
        ('''    crate::vocal::keys(ui,s,g,row,hr);
    ui.hit(Rect::new(hr.x,hr.bottom()-3.,hr.w,4.),Action::PlRowResize(row),"Drag to resize this Playlist track");''',
         '''    crate::vocal::keys(ui,s,g,row,hr);
    if super::lemon(ui) && !s.vintage_2005 && hr.w >= 100.0 && hr.h >= 40.0 {
        // The track colour gets a compact identity marker, never a giant colour slab.
        ui.c.fill_rrect(Rect::new(hr.x + 6.0, hr.y + 26.0, 3.0, 8.0),
            1.5, colored.unwrap_or(t.accent).with_alpha(if on { 220 } else { 70 }));
    }
    ui.hit(Rect::new(hr.x,hr.bottom()-3.,hr.w,4.),Action::PlRowResize(row),"Drag to resize this Playlist track");'''),
    ])
    patch("crates/os-app/src/views/piano.rs", [
        ('''            ui.c.fill_rect_gradient(body, top, bottom, (rad - 1.0).max(0.0));
        } else {''',
         '''            ui.c.fill_rect_gradient(body, top, bottom, (rad - 1.0).max(0.0));
            if body.w > 12.0 && body.h > 7.0 {
                // Narrow glass reflection and clear selected rim at any zoom.
                ui.c.fill_rect(Rect::new(body.x + 3.0, body.y + 1.0,
                    body.w - 6.0, 1.0), top.mix(Color::rgb(0xf0fff1), 0.32));
                if selected {
                    ui.c.stroke_rrect(nr.inset(0.5), rad, 1.0, t.accent);
                }
            }
        } else {'''),
        ('''        ui.c.fill_rect(Rect::new(gr.x, line_y, gr.w, pixel), sub(t, white, 0x0a));
        bands.push((y, (line_y - y).max(0.0), fill));''',
         '''        ui.c.fill_rect(Rect::new(gr.x, line_y, gr.w, pixel), sub(t, white, 0x0a));
        // Octave roots are visible but subdued even with bright custom note colours.
        if super::lemon(ui) && !selecting && key.rem_euclid(12) == 0 {
            ui.c.fill_rect(Rect::new(gr.x, y, gr.w, pixel), t.accent.with_alpha(52));
        }
        bands.push((y, (line_y - y).max(0.0), fill));'''),
        ('''    ui.c.fill_rect(l.label, t.window_title);
    let btn = Rect::new(l.label.x, l.label.y, l.label.w, 22.0);''',
         '''    ui.c.fill_rect(l.label, t.window_title);
    if super::lemon(ui) && l.label.h > 10.0 {
        ui.c.fill_rect(Rect::new(l.label.right() - 2.0, l.label.y + 3.0,
            2.0, (l.label.h - 6.0).max(0.0)), t.accent.with_alpha(125));
    }
    let btn = Rect::new(l.label.x, l.label.y, l.label.w, 22.0);'''),
        ('''    let bands = if lemon { 1 } else { 6 };
    for i in 0..bands {''',
         '''    let bands = if lemon { 4 } else { 6 };
    for i in 0..bands {'''),
        ('''let c = if lemon { t.key_white } else { t.key_white_left.mix(t.key_white, i as f32 / (bands - 1) as f32) };''',
         '''let c = t.key_white_left.mix(t.key_white, i as f32 / (bands - 1) as f32);'''),
    ])
    patch("crates/os-app/src/views/mixer.rs", [
        ('''    if console || t.style.vintage_2005 {
        console_skin::panel(ui,Rect::new(left,ly.y(HEAD_Y1)+1.,width,bottom-ly.y(HEAD_Y1)-1.),body,selected);
    } else if lemon {''',
         '''    if lemon && !t.style.vintage_2005 {
        // One native Skell panel across narrow, wide and rack mixers.
        super::skell_chrome::panel(ui,
            Rect::new(left,ly.y(HEAD_Y1)+1.,width,bottom-ly.y(HEAD_Y1)-1.),
            selected || also, tint);
    } else if console || t.style.vintage_2005 {
        console_skin::panel(ui,Rect::new(left,ly.y(HEAD_Y1)+1.,width,bottom-ly.y(HEAD_Y1)-1.),body,selected);
    } else if lemon {'''),
        ('''    if show_meters {
        ui.live(Live::Meter(n), meter.outset(if full {4.0}else{2.0}));''',
         '''    if show_meters {
        if lemon && !s.vintage_2005 {
            super::skell_chrome::meter_well(ui, meter.outset(2.0), selected);
        }
        ui.live(Live::Meter(n), meter.outset(if full {4.0}else{2.0}));'''),
        ('''fn fader_handle(ui: &mut Ui, cx: f32, cy: f32, selected: bool, hov: bool) {
    os_ui::kit::fader_handle(ui.c, ui.t, cx, cy, selected, os_ui::kit::State::hover(hov));
}''',
         '''fn fader_handle(ui: &mut Ui, cx: f32, cy: f32, selected: bool, hov: bool) {
    if super::lemon(ui) && !ui.t.style.vintage_2005 {
        // Weighted metal fader cap; hit/automation area belongs to the caller.
        let face = Rect::new(cx - 8.0, cy - 12.0, 16.0, 24.0);
        ui.c.fill_rrect(face.outset(1.5), 3.5, ui.t.sunken);
        ui.c.fill_rect_gradient(face,
            if hov { ui.t.text_dim } else { ui.t.s(0x728374) },
            ui.t.s(0x253327), 3.0);
        ui.c.stroke_rrect(face, 3.0, 1.0,
            if selected { ui.t.accent } else { ui.t.window_border });
        ui.c.fill_rect(Rect::new(face.x + 2.0, cy - 2.0, 12.0, 4.0), ui.t.s(0x0a110c));
        ui.c.fill_rect(Rect::new(face.x + 3.0, cy, 10.0, 1.0),
            if selected { ui.t.accent } else { ui.t.text });
    } else {
        os_ui::kit::fader_handle(ui.c, ui.t, cx, cy, selected, os_ui::kit::State::hover(hov));
    }
}'''),
        ('''    let themed = ui.mixer_finish == Finish::Theme;
    if themed {''',
         '''    let themed = ui.mixer_finish == Finish::Theme;
    if super::lemon(ui) && !s.vintage_2005 {
        super::skell_chrome::panel(ui, r, false, None);
    } else if themed {'''),
        ('''        if themed {
            let fill = if hov { t.slot.shade(0.10) } else { t.slot };''',
         '''        if super::lemon(ui) && !s.vintage_2005 {
            // FX slots have separated edges, a filled insert indicator and
            // an accent on hover, without altering knob/LED interaction.
            super::skell_chrome::panel(ui, row, hov, plugin.is_some().then_some(t.accent));
            if plugin.is_some() && row.h >= 11.0 {
                ui.c.fill_rrect(Rect::new(row.x + 2.0, row.y + 3.0,
                    2.0, (row.h - 6.0).max(1.0)), 1.0, t.accent);
            }
        } else if themed {
            let fill = if hov { t.slot.shade(0.10) } else { t.slot };'''),
        ('''    ui.c.fill_rect(Rect::new(r.x, r.y, r.w, 23.0), t.window_title);
    if !mx::traditional(s) || mx::inspector_left() {''',
         '''    ui.c.fill_rect(Rect::new(r.x, r.y, r.w, 23.0), t.window_title);
    if super::lemon(ui) && !s.vintage_2005 && r.w > 80.0 {
        ui.c.fill_rect(Rect::new(r.x + 6.0, r.y + 21.0, r.w - 12.0, 1.0),
            t.accent.with_alpha(112));
        super::skell_chrome::screw(ui, r.right() - 9.0, r.y + 10.0, 3.0);
    }
    if !mx::traditional(s) || mx::inspector_left() {'''),
    ])
    # Strictly validate EVERY anchor before changing ANY tracked file.
    for rel, (_old, new) in changes.items():
        print(f"{rel}: patched {len(new)} bytes")
    for rel, (_, new) in changes.items():
        (ROOT / rel).write_text(new, encoding="utf-8")
    print("Skell Studio v2 UI patch complete: 4 source modules.")

if __name__ == "__main__":
    main()

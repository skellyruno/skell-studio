// SPDX-License-Identifier: GPL-3.0-or-later
//! Lemon's built-in artwork for the classic layout. Uses the same module sizes
//! and hit regions as the optional local skin, with Lemon's own drawn faces.
use super::{Skin, Sprite};
use crate::icons::{self, Icon};
use crate::vintage::{metal, recess};
use os_ui::{Canvas, Color, Font, Rect};

const STEEL: Color = Color::rgb(0x616b71);
const PANEL: Color = Color::rgb(0x49555b);
const INK: Color = Color::rgb(0xc5d2d0);
const GLASS: Color = Color::rgb(0x263c40);
const LCD: Color = Color::rgb(0x8ca7a3);

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::new(x, y, w, h)
}

fn sprite(c: &mut Canvas, w: usize, h: usize, paint: impl FnOnce(&mut Canvas, Rect)) -> Sprite {
    c.clear(PANEL);
    paint(c, rect(0., 0., w as f32, h as f32));
    let pixels = c
        .pixmap
        .data()
        .chunks_exact(c.pixmap.width() as usize * 4)
        .take(h)
        .flat_map(|row| row.chunks_exact(4).take(w))
        .map(|p| u32::from(p[0]) << 16 | u32::from(p[1]) << 8 | u32::from(p[2]))
        .collect();
    Sprite { w, h, pixels }
}

fn panel(c: &mut Canvas, r: Rect) {
    metal(c, r, STEEL);
    for y in (4..r.h as usize - 3).step_by(4) {
        c.fill_rect(rect(3., y as f32, 3., 1.), Color::rgb(0x29363b));
        c.fill_rect(
            rect(r.right() - 6., y as f32 + 1., 3., 1.),
            Color::rgb(0x849397),
        );
    }
}

fn label(c: &mut Canvas, r: Rect, text: &str, size: u16) {
    c.text_center(Font::vintage(size).bold(), r, INK, text);
}

fn icon(c: &mut Canvas, r: Rect, glyph: Icon, ink: Color) {
    let pack = os_theme::appearance::VINTAGE_ICON_PACK;
    c.art_icon(&icons::artwork(glyph, pack).shape, r, ink);
}

fn lamp(c: &mut Canvas, r: Rect, on: bool) {
    recess(c, r, Color::rgb(if on { 0xdf6e24 } else { 0x473934 }));
    if on {
        c.fill_rect(rect(r.x + 2., r.y + 2., r.w - 4., 1.), Color::rgb(0xffcf80));
    }
}

pub(super) fn build() -> Skin {
    let mut skin = Skin {
        sprites: Default::default(),
        alpha: Default::default(),
    };
    let Ok(mut c) = Canvas::new(376, 80, 1.) else {
        return skin;
    };
    let mut add = |name: &str, w, h, count, paint: &mut dyn FnMut(&mut Canvas, Rect, usize)| {
        let frames = (0..count)
            .map(|i| sprite(&mut c, w, h, |c, r| paint(c, r, i)))
            .collect();
        skin.sprites.insert(name.to_owned(), frames);
    };
    add("ControlToolBar", 376, 80, 1, &mut |c, r, _| {
        panel(c, r);
        recess(c, rect(8., 27., 289., 24.), PANEL);
        recess(c, rect(8., 53., 305., 22.), GLASS);
        for (i, glyph) in [Icon::Minimize, Icon::Restore, Icon::Close]
            .into_iter()
            .enumerate()
        {
            let button = rect(256. + i as f32 * 14., 4., 14., 15.);
            metal(c, button, STEEL);
            icon(c, button.inset(3.), glyph, INK);
        }
        for x in [342., 361.] {
            recess(c, rect(x - 3., 5., 6., 63.), PANEL);
        }
    });
    add("TransportToolBar", 197, 40, 1, &mut |c, r, _| {
        panel(c, r);
        recess(c, rect(9., 2., 116., 26.), GLASS);
        recess(c, rect(128., 3., 59., 23.), LCD);
        label(c, rect(130., 25., 55., 13.), "TEMPO", 8);
        recess(c, rect(12., 31., 110., 5.), GLASS);
    });
    add("TransportBtns", 28, 22, 9, &mut |c, r, frame| {
        if frame < 2 {
            recess(c, r, PANEL);
            label(c, rect(0., 0., 21., 11.), "PAT", 8);
            label(c, rect(0., 11., 21., 11.), "SONG", 8);
            lamp(c, rect(21., 3., 6., 6.), frame == 0);
            lamp(c, rect(21., 13., 6., 6.), frame == 1);
            return;
        }
        let glyph = match frame {
            2 => Icon::Play,
            3 | 4 => Icon::Pause,
            5 | 6 => Icon::Stop,
            _ => Icon::Record,
        };
        metal(
            c,
            r,
            Color::rgb(if matches!(frame, 3 | 6 | 8) {
                0x89938f
            } else {
                0x98a5aa
            }),
        );
        let ink = Color::rgb(if frame == 8 { 0xa84a37 } else { 0x1c272c });
        icon(c, r.inset(5.), glyph, ink);
    });
    for (name, w) in [("RecordToolBar", 202), ("RecordSwitches", 127)] {
        add(name, w, 40, 1, &mut |c, r, _| {
            panel(c, r);
            for (i, glyph) in [
                Icon::TypingKeyboard,
                Icon::Metronome,
                Icon::Countdown,
                Icon::WaitInput,
                Icon::Blend,
                Icon::StepEdit,
            ]
            .into_iter()
            .enumerate()
            {
                let x = 9. + (i / 2) as f32 * 40.;
                let y = 5. + (i % 2) as f32 * 15.;
                recess(c, rect(x, y, 13., 13.), GLASS);
                icon(c, rect(x + 16., y - 1., 18., 15.), glyph, INK);
            }
            if w == 202 {
                recess(c, rect(128., 2., 69., 22.), Color::rgb(0x10191d));
                label(c, rect(162., 24., 33., 13.), "SNAP", 8);
            }
        });
    }
    add("RecordRail", 1, 40, 1, &mut |c, r, _| {
        c.fill_rect_gradient(r, STEEL.shade(0.12), STEEL.shade(-0.17), 0.);
    });
    add("RecordLed", 9, 9, 2, &mut |c, r, i| lamp(c, r, i == 1));
    add("RecordSocket", 14, 14, 1, &mut |c, r, _| {
        recess(c, r, GLASS)
    });
    add("TimeToolBar", 183, 40, 1, &mut |c, r, _| {
        panel(c, r);
        recess(c, rect(8., 3., 169., 33.), GLASS);
        c.fill_rect_gradient(rect(10., 5., 165., 29.), GLASS, Color::rgb(0x354e51), 0.);
    });
    add("MonitorToolBar", 145, 40, 1, &mut |c, r, _| {
        panel(c, r);
        recess(c, rect(9., 3., 69., 33.), GLASS);
        recess(c, rect(79., 3., 58., 24.), Color::rgb(0x18272a));
        for x in (81..135).step_by(3) {
            c.fill_rect(rect(x as f32, 5., 2., 10.), Color::rgb(0x374d4c));
            c.fill_rect(rect(x as f32, 16., 2., 9.), Color::rgb(0x374d4c));
        }
        label(c, rect(79., 27., 58., 12.), "MONITOR", 8);
    });
    add("PeakLit", 54, 10, 1, &mut |c, r, _| {
        c.fill_rect(r, Color::rgb(0x18272a));
        for x in (0..54).step_by(3) {
            let color = if x < 39 {
                0x9bbc81
            } else if x < 48 {
                0xe8b662
            } else {
                0xe57655
            };
            c.fill_rect_gradient(
                rect(x as f32, 0., 2., 10.),
                Color::rgb(color),
                Color::rgb(color).shade(-0.38),
                0.,
            );
        }
    });
    add("CPUToolBar", 107, 40, 1, &mut |c, r, _| {
        panel(c, r);
        recess(c, rect(9., 3., 66., 16.), GLASS);
        for y in [3., 20.] {
            recess(c, rect(75., y, 24., 16.), Color::rgb(0x593b35));
        }
        label(c, rect(9., 18., 25., 13.), "CPU", 8);
        label(c, rect(45., 25., 30., 12.), "POLY", 8);
    });
    add("PatternToolBar", 113, 40, 1, &mut |c, r, _| {
        panel(c, r);
        recess(c, rect(9., 2., 52., 34.), GLASS);
        recess(c, rect(66., 2., 38., 25.), LCD);
        label(c, rect(65., 26., 41., 13.), "PATTERN", 8);
    });
    add("PatternKey", 16, 10, 18, &mut |c, r, i| {
        metal(c, r, Color::rgb(if i < 9 { 0x79878c } else { 0xb57647 }));
        label(c, r, &((i % 9) + 1).to_string(), 8);
    });
    add("ShortcutToolBar", 204, 40, 1, &mut |c, r, _| panel(c, r));
    add("ShortcutBank", 154, 40, 1, &mut |c, r, _| panel(c, r));
    let shortcuts = [
        Icon::Playlist,
        Icon::ChannelRack,
        Icon::PianoRoll,
        Icon::Browser,
        Icon::Mixer,
    ];
    add("SCBtns", 29, 25, 10, &mut |c, r, i| {
        c.fill_rect(r, STEEL);
        let face = r.inset(1.);
        c.fill_rrect(face, 8., Color::rgb(0x233037));
        c.fill_rect_gradient(
            face.inset(1.),
            Color::rgb(0x99afb0),
            Color::rgb(0x34474d),
            7.,
        );
        c.fill_rect_gradient(
            face.inset(2.),
            Color::rgb(if i % 2 == 0 { 0x334a50 } else { 0x567477 }),
            GLASS,
            6.,
        );
        icon(c, rect(7., 2., 15., 14.), shortcuts[i / 2], INK);
        label(
            c,
            rect(5., 15., 19., 8.),
            ["PL", "SS", "PR", "SB", "FX"][i / 2],
            7,
        );
    });
    add("DiskBtns", 18, 15, 8, &mut |c, r, i| {
        metal(c, r, if i % 2 == 0 { PANEL } else { STEEL });
        icon(
            c,
            r.inset(3.),
            [Icon::FolderOpen, Icon::Save, Icon::SaveRender, Icon::Page][i / 2],
            INK,
        );
    });
    for (name, w, h) in [("MinClose", 14, 15), ("WindowButtonFaces", 13, 11)] {
        add(name, w, h, 6, &mut |c, r, i| {
            metal(
                c,
                r,
                Color::rgb(if i % 2 == 0 { 0x87969c } else { 0x53656b }),
            );
            icon(
                c,
                r.inset(2.),
                [Icon::Minimize, Icon::Restore, Icon::Close][i / 2],
                Color::rgb(0x1e292e),
            );
        });
    }
    for (name, w) in [
        ("RackMiddle", 1),
        ("RackLeft", 90),
        ("RackSwing", 75),
        ("RackEnd", 27),
        ("FooterLeft", 133),
        ("FooterMiddle", 1),
        ("FooterRight", 2),
    ] {
        add(name, w, 22, 1, &mut |c, r, _| {
            c.fill_rect_gradient(r, STEEL, PANEL, 0.);
            c.fill_rect(rect(0., 0., r.w, 1.), Color::rgb(0x8d9a9e));
            c.fill_rect(rect(0., 20., r.w, 2.), Color::rgb(0x28353b));
            if name == "RackLeft" {
                recess(c, rect(16., 2., 23., 17.), LCD);
                for (x, glyph) in [
                    (2., Icon::Exchange),
                    (43., Icon::Speaker),
                    (56., Icon::Repeat),
                ] {
                    icon(c, rect(x, 4., 12., 12.), glyph, INK);
                }
            } else if name == "RackSwing" {
                label(c, rect(0., 2., 29., 15.), "SWING", 7);
                recess(c, rect(29., 6., 40., 6.), GLASS);
            }
        });
    }
    add("SSFXBtns", 25, 22, 4, &mut |c, r, i| {
        metal(c, r.inset(1.), if i % 2 == 0 { PANEL } else { STEEL });
        icon(
            c,
            r.inset(4.),
            if i < 2 { Icon::Mixer } else { Icon::PianoRoll },
            INK,
        );
    });
    add("FXMeterIdle", 4, 19, 1, &mut |c, r, _| {
        c.fill_rect_gradient(r, Color::rgb(0x77582a), Color::rgb(0x162d1a), 0.);
    });
    add("FXMeterColumn", 4, 19, 1, &mut |c, r, _| {
        c.fill_rect_gradient(r, Color::rgb(0xe8b659), Color::rgb(0x426f2d), 0.);
    });
    for letter in 'A'..='Z' {
        add(&letter.to_string(), 5, 20, 1, &mut |c, r, _| {
            c.fill_rect(r, PANEL);
            c.text_center(Font::condensed(12, 4).bold(), r, INK, &letter.to_string());
        });
    }
    skin
}

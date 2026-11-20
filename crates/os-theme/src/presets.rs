// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Lemon Studio: https://github.com/ItsAlphaNeon/Lemon-Studio

//! Built-in themes, `.flstheme` files in `themes/` (parsed by `FlTheme::parse`). They colour either skin.
// spec: ui/theme-presets.md
//
// Settings > Theme > Themes lists the Studio and Console themes (the skins' own colours), these, then the user's own
// `.flstheme` files from the `themes` folder in Lemon Studio's user folder (`os_paths::user_dir`).

macro_rules! presets {
    ($($name:literal),* $(,)?) => {
        /// (name, theme text), dark themes first.
        pub const PRESETS: &[(&str, &str)] = &[$(($name, include_str!(concat!("../themes/", $name, ".flstheme"))),)*];
    };
}

presets!(
    "Graphite",
    "Ember",
    "Skell Green",
    "Steel",
    "Bone",
    "Frost",
    "Copper",
    "Amber",
    "Sea Glass",
    "Moss",
    "Umber",
    "Neon Magenta",
    "Afterlight",
    "Blueshift",
    "Low Profile",
    "Charcoal",
    "Salamander",
    "Velour",
    "Lagoon",
    "Cobalt",
    "Orchid",
    "Jade",
    "Moss Light",
    "Umber Light",
    "Rose Quartz",
    "Peridot",
    "Sandstone",
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FlTheme;

    /// Every built-in theme parses, none is the stock Dark theme (each is its own colours), and all differ.
    #[test]
    fn presets_parse_and_differ() {
        assert_eq!(PRESETS.len(), 27);
        let themes: Vec<FlTheme> = PRESETS.iter().map(|(_, t)| FlTheme::parse(t)).collect();
        for (th, (name, _)) in themes.iter().zip(PRESETS) {
            assert!(th.selected.is_some() && th.highlight.is_some() && th.text_color.is_some(), "{name}");
            assert!(!th.is_stock(), "{name}");
        }
        for i in 0..themes.len() {
            for j in i + 1..themes.len() {
                assert_ne!(themes[i], themes[j], "{} {}", PRESETS[i].0, PRESETS[j].0);
            }
        }
    }

    /// Ember is the orange-on-black theme: its key colours decode to orange (selection and text are RGB values,
    /// highlight and mute are COLORREF values), so a byte-order slip would show up as blue here.
    #[test]
    fn ember_is_orange() {
        let (_, text) = PRESETS.iter().find(|(name, _)| *name == "Ember").expect("Ember is built in");
        let th = FlTheme::parse(text);
        assert_eq!(th.selected, Some(crate::Color::rgb(0xff8a1f)));
        assert_eq!(th.highlight, Some(crate::Color::rgb(0xffb347)));
        assert_eq!(th.mute, Some(crate::Color::rgb(0xff6b2c)));
        assert_eq!(th.text_color, Some(crate::Color::rgb(0xddd8d2)));
    }

    /// Skell Green carries the exact brand colours (lemon::MAIN / HIGHLIGHT / DEEP) as a theme, so the Skell accent is
    /// available on the Console skin too. Steel is cool blue and Bone is the light theme with a green accent.
    #[test]
    fn skell_themes_keep_their_key_colours() {
        let theme = |wanted: &str| {
            let (_, text) = PRESETS.iter().find(|(name, _)| *name == wanted).expect("built in");
            FlTheme::parse(text)
        };
        let green = theme("Skell Green");
        assert_eq!(green.selected, Some(crate::design::lemon::MAIN));
        assert_eq!(green.highlight, Some(crate::design::lemon::HIGHLIGHT));
        assert_eq!(green.mute, Some(crate::design::lemon::DEEP));
        assert_eq!(theme("Steel").selected, Some(crate::Color::rgb(0x5cc8ff)));
        assert_eq!(theme("Bone").selected, Some(crate::Color::rgb(0x22a559)));
    }
}

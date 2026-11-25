// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: AGPL-3.0-only
//! Skell Compressor: standalone VST3/CLAP Fruity Compressor-compatible dynamics.
//! The clean-room LYNX engine is kept separate from the GPL Studio executable.

use lemon_small_fx::{Card, Control, Kind, Mark, Setting, SmallFx, Sound, Spec};
use lemon_small_fx::params;
use nih_plug::prelude::{nih_export_clap, nih_export_vst3, AudioIOLayout, ClapFeature, PortNames, Vst3SubCategory, new_nonzero_u32};
use std::{ffi::c_void, num::NonZeroU32, ptr};

unsafe extern "C" {
    fn lynx_fc_new(rate: i32) -> *mut c_void;
    fn lynx_fc_delete(instance: *mut c_void);
    fn lynx_fc_prepare(instance: *mut c_void, rate: i32);
    fn lynx_fc_set(instance: *mut c_void, values: *const i32, lookahead: bool);
    fn lynx_fc_reset(instance: *mut c_void);
    fn lynx_fc_process(instance: *mut c_void, input: *const f32, sidechain: *const f32, output: *mut f32, frames: i32);
    fn lynx_fc_latency(instance: *mut c_void) -> i32;
}

const TYPES: &[&str] = &["Hard", "Medium", "Vintage", "Soft", "Hard/R", "Medium/R", "Vintage/R", "Soft/R"];
const LOOKAHEAD: &[&str] = &["PDC Off", "PDC On"];
const SIDECHAIN: &[&str] = &["Auto", "On"];

const SETTINGS: [Setting; 8] = [
    Setting { id: "threshold", name: "Threshold", range: (-600, 0), start: 0, magnet: None },
    Setting { id: "ratio", name: "Ratio", range: (4, 300), start: 10, magnet: None },
    Setting { id: "gain", name: "Gain", range: (-300, 300), start: 0, magnet: Some(0) },
    Setting { id: "attack", name: "Attack", range: (0, 4000), start: 150, magnet: None },
    Setting { id: "release", name: "Release", range: (1, 4000), start: 200, magnet: None },
    Setting { id: "type", name: "Knee / release type", range: (0, 7), start: 0, magnet: None },
    Setting { id: "lookahead", name: "Look-ahead PDC", range: (0, 1), start: 1, magnet: None },
    Setting { id: "sidechain", name: "Sidechain Mode", range: (0, 1), start: 0, magnet: None },
];

const CONTROLS: [Control; 8] = [
    Control::KnobWithValue { id: "threshold", cx: 68.0, cy: 150.0, r: 23.0, caption: "THRESHOLD", bipolar: false, width: 98.0 },
    Control::KnobWithValue { id: "ratio", cx: 174.0, cy: 150.0, r: 23.0, caption: "RATIO", bipolar: false, width: 98.0 },
    Control::KnobWithValue { id: "gain", cx: 280.0, cy: 150.0, r: 23.0, caption: "GAIN", bipolar: true, width: 98.0 },
    Control::KnobWithValue { id: "attack", cx: 386.0, cy: 150.0, r: 23.0, caption: "ATTACK", bipolar: false, width: 98.0 },
    Control::KnobWithValue { id: "release", cx: 492.0, cy: 150.0, r: 23.0, caption: "RELEASE", bipolar: false, width: 98.0 },
    Control::Choice { id: "type", x: 570.0, top: 76.0, names: TYPES },
    Control::Choice { id: "lookahead", x: 684.0, top: 76.0, names: LOOKAHEAD },
    Control::Choice { id: "sidechain", x: 684.0, top: 132.0, names: SIDECHAIN },
];

fn text(k: usize, raw: i32) -> String {
    match k {
        0 | 2 => format!("{:+.1} dB", raw as f32 / 10.0),
        1 => format!("{:.1}:1", raw as f32 / 10.0),
        3 => format!("{:.1} ms", raw as f32 / 10.0),
        4 => format!("{raw} ms"),
        5 => TYPES[raw.clamp(0, 7) as usize].to_string(),
        6 => if raw == 0 { "Off".into() } else { "On".into() },
        7 => SIDECHAIN[raw.clamp(0, 1) as usize].to_string(),
        _ => String::new(),
    }
}

fn parse(k: usize, input: &str) -> Option<i32> {
    let s = input.trim();
    match k {
        5 => TYPES.iter().position(|name| name.eq_ignore_ascii_case(s)).map(|i| i as i32),
        6 if s.eq_ignore_ascii_case("off") || s.eq_ignore_ascii_case("pdc off") => Some(0),
        6 if s.eq_ignore_ascii_case("on") || s.eq_ignore_ascii_case("pdc on") => Some(1),
        7 => SIDECHAIN.iter().position(|name| name.eq_ignore_ascii_case(s)).map(|i| i as i32),
        0 | 2 => {
            let v = s.trim_end_matches("dB").trim().parse::<f64>().ok()?;
            let (lo, hi) = if k == 0 { (-600.0, 0.0) } else { (-300.0, 300.0) };
            (v.is_finite()).then(|| (v * 10.0).round().clamp(lo, hi) as i32)
        }
        1 => {
            let v = s.trim_end_matches(":1").trim().parse::<f64>().ok()?;
            (v.is_finite()).then(|| (v * 10.0).round().clamp(4.0, 300.0) as i32)
        }
        3 => {
            let v = s.trim_end_matches("ms").trim().parse::<f64>().ok()?;
            (v.is_finite()).then(|| (v * 10.0).round().clamp(0.0, 4000.0) as i32)
        }
        4 => {
            let v = s.trim_end_matches("ms").trim().parse::<f64>().ok()?;
            (v.is_finite()).then(|| v.round().clamp(1.0, 4000.0) as i32)
        }
        _ => None,
    }
}

struct CompressorSound {
    instance: ptr::NonNull<c_void>,
    rate: u32,
    controls: [i32; 6],
    lookahead: bool,
    sidechain_mode: i32,
    fl_sidechain_enabled: bool,
    input: Vec<f32>,
    output: Vec<f32>,
    sidechain: Vec<f32>,
}

unsafe impl Send for CompressorSound {}

impl CompressorSound {
    fn new(rate: u32, values: &[i32]) -> Self {
        let rate = rate.clamp(8000, 384000);
        let mut raw = [0, 10, 0, 150, 200, 0, 1, 0];
        for (dst, src) in raw.iter_mut().zip(values.iter().copied()) { *dst = src; }
        let instance = ptr::NonNull::new(unsafe { lynx_fc_new(rate as i32) })
            .expect("Fruity Compressor engine allocation");
        let mut sound = Self {
            instance,
            rate,
            controls: raw[..6].try_into().expect("six compressor controls"),
            lookahead: raw[6] != 0,
            sidechain_mode: raw[7].clamp(0, 1),
            fl_sidechain_enabled: false,
            input: vec![0.0; 8192],
            output: vec![0.0; 8192],
            sidechain: vec![0.0; 8192],
        };
        sound.configure();
        sound
    }

    fn configure(&mut self) {
        unsafe { lynx_fc_set(self.instance.as_ptr(), self.controls.as_ptr(), self.lookahead); }
    }
}

impl Sound for CompressorSound {
    fn set(&mut self, values: &[i32]) {
        for (dst, src) in self.controls.iter_mut().zip(values.iter().copied()) { *dst = src; }
        self.lookahead = values.get(6).copied().unwrap_or(1) != 0;
        self.sidechain_mode = values.get(7).copied().unwrap_or(0).clamp(0, 1);
        self.configure();
    }

    fn set_sample_rate(&mut self, rate: u32) {
        self.rate = rate.clamp(8000, 384000);
        unsafe { lynx_fc_prepare(self.instance.as_ptr(), self.rate as i32); }
    }

    fn reset(&mut self) {
        unsafe { lynx_fc_reset(self.instance.as_ptr()); }
    }

    fn set_saved_state(&mut self, state: &[u8]) {
        self.fl_sidechain_enabled = state.get(32..36)
            .map(|bytes| i32::from_le_bytes(bytes.try_into().unwrap()) != 0)
            .unwrap_or(false);
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.process_with_sidechain(left, right, None);
    }

    fn process_with_sidechain(&mut self, left: &mut [f32], right: &mut [f32], sidechain: Option<(&[f32], &[f32])>) {
        let frames = left.len().min(right.len());
        let capacity = self.input.len() / 2;
        let mut offset = 0;
        while offset < frames {
            let count = (frames - offset).min(capacity);
            let n = count * 2;
            let input = &mut self.input[..n];
            let output = &mut self.output[..n];
            for i in 0..count {
                input[i * 2] = left[offset + i];
                input[i * 2 + 1] = right[offset + i];
            }
            // Auto uses the original FL sidechain routing assignment; On forces a connected
            // host sidechain for VST3/CLAP projects that do not have FL state.
            let use_sidechain = self.sidechain_mode != 0 || self.fl_sidechain_enabled;
            let side_ptr = if use_sidechain {
                if let Some((side_l, side_r)) = sidechain {
                let side = &mut self.sidechain[..n];
                for i in 0..count {
                    side[i * 2] = side_l.get(offset + i).copied().unwrap_or(0.0);
                    side[i * 2 + 1] = side_r.get(offset + i).copied().unwrap_or(0.0);
                }
                side.as_ptr()
                } else {
                    ptr::null()
                }
            } else {
                ptr::null()
            };
            unsafe {
                lynx_fc_process(self.instance.as_ptr(), input.as_ptr(), side_ptr, output.as_mut_ptr(), count as i32);
            }
            for i in 0..count {
                left[offset + i] = output[i * 2];
                right[offset + i] = output[i * 2 + 1];
            }
            offset += count;
        }
    }

    fn latency(&self) -> u32 {
        unsafe { lynx_fc_latency(self.instance.as_ptr()).max(0) as u32 }
    }
}

impl Drop for CompressorSound {
    fn drop(&mut self) {
        unsafe { lynx_fc_delete(self.instance.as_ptr()); }
    }
}

fn sound(rate: u32, values: &[i32]) -> Box<dyn Sound> {
    Box::new(CompressorSound::new(rate, values))
}

const LAYOUTS: &[AudioIOLayout] = &[
    AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        aux_input_ports: &[new_nonzero_u32(2)],
        names: PortNames { aux_inputs: &["Sidechain"], ..PortNames::const_default() },
        ..AudioIOLayout::const_default()
    },
    AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    },
];

pub const SPEC: &Spec = &Spec {
    name: "Skell Compressor",
    word: "COMPRESSOR",
    about: "Fruity Compressor-style dynamics with external sidechain and host look-ahead compensation",
    settings: &SETTINGS,
    version: Some(3),
    any_version: false,
    fields: &[0, 1, 2, 3, 4, 5, params::BYTE, params::byte(8)],
    tail: &[1, 0, 0, 0, 0, 0, 0, 0],
    blank: None,
    locate: None,
    size: (780, 300),
    cards: &[Card { x0: 8.0, x1: 772.0, y0: 22.0, y1: 226.0, heading: "DYNAMICS" }],
    controls: &CONTROLS,
    display: None,
    mark: Mark { x: 768.0, y: 276.0, anchor: 1.0, stacked: false },
    hint: Some(os_ui::Rect::new(12.0, 244.0, 450.0, 18.0)),
    labels: &[],
    text,
    parse,
    sound,
    rings: false,
};

pub struct CompressorKind;

impl Kind for CompressorKind {
    const VENDOR: &'static str = "Skell Audio";
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = LAYOUTS;
    const SPEC: &'static Spec = SPEC;
    const CLAP_ID: &'static str = "com.skellstudio.compressor";
    const CLAP_DESCRIPTION: &'static str = "Fruity Compressor-compatible clean dynamics with optional stereo sidechain";
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect, ClapFeature::Compressor, ClapFeature::Stereo];
    const VST3_CLASS_ID: [u8; 16] = *b"SkellStudioComp1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Fx, Vst3SubCategory::Dynamics];
}

pub type SkellCompressor = SmallFx<CompressorKind>;

#[cfg(not(feature = "linked"))]
nih_export_clap!(SkellCompressor);
#[cfg(not(feature = "linked"))]
nih_export_vst3!(SkellCompressor);

#[cfg(test)]
mod tests {
    use super::*;
    use lemon_small_fx::editor;
    use nih_plug::prelude::Params;
    use std::sync::Arc;

    fn tone(rate: u32, seconds: f32, amplitude: f32) -> Vec<f32> {
        (0..(rate as f32 * seconds) as usize)
            .map(|i| amplitude * (std::f32::consts::TAU * 220.0 * i as f32 / rate as f32).sin())
            .collect()
    }

    #[test]
    fn raw_values_text_and_state_are_preserved() {
        for k in 0..6 {
            for v in SETTINGS[k].range.0..=SETTINGS[k].range.1 {
                assert_eq!(parse(k, &text(k, v)), Some(v), "parameter {k}, value {v}");
            }
        }
        assert_eq!(parse(6, "PDC On"), Some(1));
        assert_eq!(parse(5, "Vintage/R"), Some(6));
        assert_eq!(parse(7, "Auto"), Some(0));
        assert_eq!(parse(7, "On"), Some(1));

        let state = lemon_small_fx::params::default_state(SPEC);
        assert_eq!(state.len(), 36, "version + six controls + flags + sidechain");
        let p = lemon_small_fx::params::FxParams::new(SPEC);
        p.set_raw(state.clone());
        let fields = p.serialize_fields();
        let restored = lemon_small_fx::params::FxParams::new(SPEC);
        restored.deserialize_fields(&fields);
        assert_eq!(restored.raw_copy(), state);
    }

    #[test]
    fn lookahead_latency_and_block_size_are_stable() {
        fn render(block: usize, lookahead: i32) -> Vec<f32> {
            let mut sound = CompressorSound::new(48000, &[-300, 40, 0, 150, 200, 0, lookahead]);
            let input = tone(48000, 1.0, 0.8);
            let mut left = input.clone();
            let mut right = input;
            let mut offset = 0;
            while offset < left.len() {
                let end = (offset + block).min(left.len());
                sound.process(&mut left[offset..end], &mut right[offset..end]);
                offset = end;
            }
            left
        }
        let ref_out = render(4096, 1);
        for block in [1, 17, 64, 127, 512, 2048, 8192] {
            let candidate = render(block, 1);
            assert!(ref_out.iter().zip(candidate).all(|(a,b)| (a-b).abs() < 2.0e-6), "block {block}");
        }
        let off = CompressorSound::new(48000, &[-300, 40, 0, 150, 200, 0, 0]);
        let on = CompressorSound::new(48000, &[-300, 40, 0, 150, 200, 0, 1]);
        assert_eq!(off.latency(), 0);
        assert_eq!(on.latency(), 64);
    }

    #[test]
    fn sidechain_changes_compression_and_output_stays_finite() {
        let mut own = CompressorSound::new(48000, &[-300, 40, 0, 150, 200, 0, 1, 0]);
        let mut keyed = CompressorSound::new(48000, &[-300, 40, 0, 150, 200, 0, 1, 1]);
        let dry = tone(48000, 1.0, 0.2);
        let side = tone(48000, 1.0, 0.9);
        let mut own_l = dry.clone();
        let mut own_r = dry.clone();
        own.process(&mut own_l, &mut own_r);
        let mut key_l = dry.clone();
        let mut key_r = dry;
        keyed.process_with_sidechain(&mut key_l, &mut key_r, Some((&side, &side)));
        assert!(own_l.iter().chain(&own_r).chain(&key_l).chain(&key_r).all(|v| v.is_finite()));
        assert!(own_l.iter().zip(&key_l).any(|(a,b)| (a-b).abs() > 1.0e-3));
    }

    #[test]
    fn auto_sidechain_follows_imported_fruity_compressor_state() {
        let mut auto = CompressorSound::new(48000, &[-300, 40, 0, 150, 200, 0, 1, 0]);
        let mut dry = CompressorSound::new(48000, &[-300, 40, 0, 150, 200, 0, 1, 0]);

        let mut state = lemon_small_fx::params::default_state(SPEC);
        // Fruity Compressor v3's sidechain assignment is the int at byte 32.
        state[32..36].copy_from_slice(&1i32.to_le_bytes());
        auto.set_saved_state(&state);

        let input = tone(48000, 0.25, 0.2);
        let side = tone(48000, 0.25, 0.9);
        let mut auto_l = input.clone();
        let mut auto_r = input.clone();
        let mut dry_l = input.clone();
        let mut dry_r = input;
        auto.process_with_sidechain(&mut auto_l, &mut auto_r, Some((&side, &side)));
        dry.process_with_sidechain(&mut dry_l, &mut dry_r, Some((&side, &side)));
        assert!(auto_l.iter().chain(&auto_r).all(|v| v.is_finite()));
        assert!(auto_l.iter().zip(&dry_l).any(|(a, b)| (a - b).abs() > 1.0e-3));

        // Explicit On also enables an external host sidechain for a new non-FL project.
        let mut manual = CompressorSound::new(48000, &[-300, 40, 0, 150, 200, 0, 1, 1]);
        let mut manual_l = tone(48000, 0.25, 0.2);
        let mut manual_r = manual_l.clone();
        manual.process_with_sidechain(&mut manual_l, &mut manual_r, Some((&side, &side)));
        assert!(manual_l.iter().zip(&dry_l).any(|(a, b)| (a - b).abs() > 1.0e-3));
    }

    #[test]
    fn editor_exposes_all_controls_and_can_capture() {
        let params = Arc::new(lemon_small_fx::params::FxParams::new(SPEC));
        let mut logic = editor::Logic::new(
            SPEC,
            params,
            Arc::new(lemon_small_fx::Shared::default()),
            None,
        );
        let mut canvas = os_ui::Canvas::new(SPEC.size.0, SPEC.size.1, 1.0).unwrap();
        logic.paint(&mut canvas, &os_plugkit::palette());
        assert_eq!(logic.hits.iter().filter(|h| matches!(h.t, lemon_small_fx::view::Target::Knob(_))).count(), 5);
        assert_eq!(logic.hits.iter().filter(|h| matches!(h.t, lemon_small_fx::view::Target::Choice(_, _))).count(), 12);
        if let Some(dir) = std::env::var_os("OS_SHOT_DIR") {
            let dir = std::path::PathBuf::from(dir);
            canvas.pixmap.save_png(dir.join("skell-compressor.png")).unwrap();
            logic.typed("threshold", "-24 dB");
            logic.typed("ratio", "4:1");
            assert_eq!(logic.value_text("threshold"), "-24.0 dB");
            assert_eq!(logic.value_text("ratio"), "4.0:1");
            let mut scaled = os_ui::Canvas::new(SPEC.size.0 * 3 / 2, SPEC.size.1 * 3 / 2, 1.5).unwrap();
            logic.paint(&mut scaled, &os_plugkit::palette());
            scaled.pixmap.save_png(dir.join("skell-compressor-150pct-edited.png")).unwrap();
        }
    }
}

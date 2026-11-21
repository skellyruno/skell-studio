// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: AGPL-3.0-only
//! Standalone Skell Beat VST3/CLAP effect. The processing core is the clean-room
//! LYNX Gross Beat engine, isolated from LYNX's DAW-specific Effect wrapper.

use lemon_small_fx::{Card, Control, Kind, Mark, Setting, SmallFx, Sound, Spec};
use nih_plug::prelude::{nih_export_clap, nih_export_vst3, ClapFeature, Vst3SubCategory};
use std::{ffi::c_void, ptr};

unsafe extern "C" {
    fn lynx_gb_factory_state(out: *mut u8, capacity: i32) -> i32;
    fn lynx_gb_new(sample_rate: i32, max_block: i32) -> *mut c_void;
    fn lynx_gb_delete(instance: *mut c_void);
    fn lynx_gb_load(instance: *mut c_void, data: *const u8, size: usize) -> bool;
    fn lynx_gb_prepare(instance: *mut c_void, sample_rate: i32, max_block: i32);
    fn lynx_gb_set_param(instance: *mut c_void, index: i32, raw: i32);
    fn lynx_gb_set_transport(instance: *mut c_void, tempo: f64, position_beats: f64, has_position: bool, playing: bool);
    fn lynx_gb_process(instance: *mut c_void, left: *mut f32, right: *mut f32, frames: i32);
    fn lynx_gb_reset(instance: *mut c_void);
    fn lynx_gb_latency() -> i32;
}

const SETTINGS: [Setting; 8] = [
    Setting { id: "time_slot", name: "Time Slot", range: (0, 35), start: 0, magnet: None },
    Setting { id: "volume_slot", name: "Volume Slot", range: (0, 35), start: 0, magnet: None },
    Setting { id: "time_mix", name: "Time Mix", range: (0, 256), start: 256, magnet: Some(256) },
    Setting { id: "volume_mix", name: "Volume Mix", range: (0, 256), start: 256, magnet: Some(256) },
    Setting { id: "attack", name: "Attack", range: (0, 10000), start: 1765, magnet: None },
    Setting { id: "release", name: "Release", range: (0, 10000), start: 5000, magnet: None },
    Setting { id: "tension", name: "Volume Tension", range: (-256, 256), start: 0, magnet: Some(0) },
    Setting { id: "hold", name: "Hold", range: (0, 1), start: 0, magnet: None },
];

const CONTROLS: [Control; 8] = [
    Control::Knob { id: "time_slot", cx: 52.0, cy: 128.0, r: 21.0, caption: "TIME SLOT", bipolar: false },
    Control::Knob { id: "volume_slot", cx: 149.0, cy: 128.0, r: 21.0, caption: "VOLUME SLOT", bipolar: false },
    Control::Knob { id: "time_mix", cx: 246.0, cy: 128.0, r: 21.0, caption: "TIME MIX", bipolar: false },
    Control::Knob { id: "volume_mix", cx: 343.0, cy: 128.0, r: 21.0, caption: "VOLUME MIX", bipolar: false },
    Control::Knob { id: "attack", cx: 440.0, cy: 128.0, r: 21.0, caption: "ATTACK", bipolar: false },
    Control::Knob { id: "release", cx: 537.0, cy: 128.0, r: 21.0, caption: "RELEASE", bipolar: false },
    Control::Knob { id: "tension", cx: 634.0, cy: 128.0, r: 21.0, caption: "TENSION", bipolar: true },
    Control::Knob { id: "hold", cx: 731.0, cy: 128.0, r: 21.0, caption: "HOLD", bipolar: false },
];

fn factory_state() -> Vec<u8> {
    let needed = unsafe { lynx_gb_factory_state(ptr::null_mut(), 0) };
    if needed <= 0 { return Vec::new(); }
    let mut state = vec![0u8; needed as usize];
    let written = unsafe { lynx_gb_factory_state(state.as_mut_ptr(), needed) };
    if written != needed { Vec::new() } else { state }
}

/// Gross Beat's versioned state starts with a u32 version and then its eight
/// automatable parameters. The remaining bytes contain the 72 envelope slots.
fn state_parameter_offset(state: &[u8]) -> Option<usize> {
    (state.len() >= 4 + SETTINGS.len() * 4).then_some(4)
}

fn text(k: usize, raw: i32) -> String {
    match k {
        0 | 1 => format!("Slot {:02}", raw.clamp(0, 35) + 1),
        2 | 3 => format!("{:.1}%", raw.clamp(0, 256) as f32 * 100.0 / 256.0),
        4 | 5 => format!("{:.0}", raw),
        6 => format!("{:+.1}%", raw as f32 * 100.0 / 256.0),
        7 => if raw == 0 { "Off".into() } else { "On".into() },
        _ => String::new(),
    }
}

fn parse(k: usize, value: &str) -> Option<i32> {
    let s = value.trim().trim_end_matches('%').trim();
    if k == 0 || k == 1 {
        let n = s.strip_prefix("Slot").unwrap_or(s).trim().parse::<i32>().ok()?;
        return Some((n - 1).clamp(0, 35));
    }
    if k == 7 && (s.eq_ignore_ascii_case("on") || s.eq_ignore_ascii_case("off")) {
        return Some(i32::from(s.eq_ignore_ascii_case("on")));
    }
    let n = s.parse::<f64>().ok()?;
    if !n.is_finite() { return None; }
    Some(match k {
        2 | 3 => (n * 256.0 / 100.0).round().clamp(0.0, 256.0) as i32,
        6 => (n * 256.0 / 100.0).round().clamp(-256.0, 256.0) as i32,
        4 | 5 => n.round().clamp(0.0, 10000.0) as i32,
        _ => return None,
    })
}

struct BeatSound {
    instance: ptr::NonNull<c_void>,
    rate: u32,
    params: [i32; 8],
}

unsafe impl Send for BeatSound {}

impl BeatSound {
    fn new(rate: u32, values: &[i32]) -> Self {
        let rate = rate.clamp(8000, 384000);
        let instance = ptr::NonNull::new(unsafe { lynx_gb_new(rate as i32, 4096) })
            .expect("Gross Beat engine allocation");
        let mut params = SETTINGS.map(|setting| setting.start);
        for (dst, src) in params.iter_mut().zip(values.iter().copied()) { *dst = src; }
        Self { instance, rate, params }
    }

    fn apply_params(&mut self) {
        for (index, value) in self.params.iter().copied().enumerate() {
            unsafe { lynx_gb_set_param(self.instance.as_ptr(), index as i32, value); }
        }
    }
}

impl Sound for BeatSound {
    fn set(&mut self, values: &[i32]) {
        for (dst, src) in self.params.iter_mut().zip(values.iter().copied()) { *dst = src; }
        self.apply_params();
    }

    fn set_sample_rate(&mut self, rate: u32) {
        self.rate = rate.clamp(8000, 384000);
        unsafe { lynx_gb_prepare(self.instance.as_ptr(), self.rate as i32, 4096); }
    }

    fn reset(&mut self) {
        unsafe { lynx_gb_reset(self.instance.as_ptr()); }
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let frames = left.len().min(right.len());
        if frames == 0 { return; }
        unsafe { lynx_gb_process(self.instance.as_ptr(), left.as_mut_ptr(), right.as_mut_ptr(), frames as i32); }
    }

    fn set_saved_state(&mut self, state: &[u8]) {
        if !state.is_empty() {
            let ok = unsafe { lynx_gb_load(self.instance.as_ptr(), state.as_ptr(), state.len()) };
            debug_assert!(ok, "Gross Beat saved state could not be parsed");
            if ok { self.apply_params(); }
        }
    }

    fn set_transport(&mut self, tempo: f64, position_beats: Option<f64>, playing: bool) {
        let has_position = position_beats.is_some();
        unsafe {
            lynx_gb_set_transport(
                self.instance.as_ptr(),
                tempo,
                position_beats.unwrap_or(0.0),
                has_position,
                playing,
            );
        }
    }

    fn latency(&self) -> u32 {
        unsafe { lynx_gb_latency().max(0) as u32 }
    }
}

fn sound(rate: u32, values: &[i32]) -> Box<dyn Sound> {
    Box::new(BeatSound::new(rate, values))
}

pub const SPEC: &Spec = &Spec {
    name: "Skell Beat",
    word: "BEAT",
    about: "Tempo-synced time and volume effects powered by the clean-room LYNX Gross Beat engine",
    settings: &SETTINGS,
    version: None,
    any_version: true,
    fields: &[0, 1, 2, 3, 4, 5, 6, 7],
    tail: &[],
    blank: Some(factory_state),
    locate: Some(state_parameter_offset),
    size: (784, 258),
    cards: &[Card { x0: 8.0, x1: 776.0, y0: 22.0, y1: 215.0, heading: "TIME TRAVEL & VOLUME" }],
    controls: &CONTROLS,
    display: None,
    mark: Mark { x: 770.0, y: 240.0, anchor: 1.0, stacked: false },
    hint: Some(os_ui::Rect::new(12.0, 222.0, 500.0, 18.0)),
    labels: &[],
    text,
    parse,
    sound,
    rings: false,
};

pub struct BeatKind;

impl Kind for BeatKind {
    const VENDOR: &'static str = "Skell Audio";
    const SPEC: &'static Spec = SPEC;
    const CLAP_ID: &'static str = "com.skellstudio.skellbeat";
    const CLAP_DESCRIPTION: &'static str = "Tempo-synced stutter, gating, tape-stop and reverse time effects";
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect, ClapFeature::Delay];
    const VST3_CLASS_ID: [u8; 16] = *b"SkellStudioBeat1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Fx];
}

pub type SkellBeat = SmallFx<BeatKind>;

#[cfg(not(feature = "linked"))]
nih_export_clap!(SkellBeat);
#[cfg(not(feature = "linked"))]
nih_export_vst3!(SkellBeat);

#[cfg(test)]
mod tests {
    use super::*;
    use lemon_small_fx::editor;
    use std::sync::Arc;

    fn tone(rate: u32, seconds: f32) -> Vec<f32> {
        (0..(rate as f32 * seconds) as usize)
            .map(|i| (std::f32::consts::TAU * 220.0 * i as f32 / rate as f32).sin() * 0.25)
            .collect()
    }

    #[test]
    fn factory_state_roundtrips_and_contains_72_slots() {
        let state = factory_state();
        assert!(state.len() > 512, "factory Gross Beat state should include its 72 slot maps");
        assert_eq!(state_parameter_offset(&state), Some(4));
        assert_eq!(parse(0, &text(0, 3)), Some(3));
        assert_eq!(parse(1, &text(1, 35)), Some(35));
        assert_eq!(parse(6, &text(6, -128)), Some(-128));
    }

    #[test]
    fn playback_is_finite_and_responds_to_time_slot_selection() {
        let state = factory_state();
        let mut sound = BeatSound::new(48000, &[0, 0, 256, 256, 1765, 5000, 0, 0]);
        sound.set_saved_state(&state);
        sound.set(&[1, 0, 256, 256, 1765, 5000, 0, 0]);
        let mut left = tone(48000, 2.0);
        let mut right = left.clone();
        let dry = left.clone();
        let block = 4096;
        for offset in (0..left.len()).step_by(block) {
            let end = (offset + block).min(left.len());
            let beats = offset as f64 / 48000.0 * 120.0 / 60.0;
            sound.set_transport(120.0, Some(beats), true);
            sound.process(&mut left[offset..end], &mut right[offset..end]);
        }
        assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
        assert!(left.iter().zip(&dry).any(|(a, b)| (a - b).abs() > 1.0e-5));
        assert_eq!(sound.latency(), 15);
    }

    #[test]
    fn variable_host_blocks_keep_gross_beat_transport_aligned() {
        fn render(block: usize) -> Vec<f32> {
            let state = factory_state();
            let mut sound = BeatSound::new(48000, &[0, 0, 256, 256, 1765, 5000, 0, 0]);
            sound.set_saved_state(&state);
            sound.set(&[1, 0, 256, 256, 1765, 5000, 0, 0]);
            let input = tone(48000, 1.0);
            let mut left = input;
            let mut right = left.clone();
            let mut offset = 0;
            while offset < left.len() {
                let end = (offset + block).min(left.len());
                let beats = offset as f64 / 48000.0 * 120.0 / 60.0;
                sound.set_transport(120.0, Some(beats), true);
                sound.process(&mut left[offset..end], &mut right[offset..end]);
                offset = end;
            }
            left
        }

        let reference = render(4096);
        for block in [1, 17, 64, 127, 512, 2048, 8192] {
            let candidate = render(block);
            assert!(
                reference.iter().zip(candidate).all(|(a, b)| (a - b).abs() < 2.0e-6),
                "output changed with host block size {block}"
            );
        }
    }

    #[test]
    fn all_72_slot_maps_survive_plugin_state_save_and_restore() {
        use nih_plug::prelude::Params;

        let state = factory_state();
        assert!(state.len() > 512);
        let original = lemon_small_fx::params::FxParams::new(SPEC);
        original.set_raw(state.clone());
        let host_fields = original.serialize_fields();

        let restored = lemon_small_fx::params::FxParams::new(SPEC);
        restored.deserialize_fields(&host_fields);
        assert_eq!(restored.raw_copy(), state, "the host state field must preserve every slot map and option byte");
    }

    #[test]
    fn editor_exposes_all_eight_controls_and_can_capture() {
        let params = Arc::new(lemon_small_fx::params::FxParams::new(SPEC));
        let mut logic = editor::Logic::new(
            SPEC,
            params,
            Arc::new(lemon_small_fx::Shared::default()),
            None,
        );
        let mut canvas = os_ui::Canvas::new(SPEC.size.0, SPEC.size.1, 1.0).unwrap();
        logic.paint(&mut canvas, &os_plugkit::palette());
        assert_eq!(logic.hits.iter().filter(|h| matches!(h.t, lemon_small_fx::view::Target::Knob(_))).count(), 8);
        if let Some(dir) = std::env::var_os("OS_SHOT_DIR") {
            canvas.pixmap.save_png(std::path::PathBuf::from(dir).join("skell-beat.png")).unwrap();
        }
    }
}

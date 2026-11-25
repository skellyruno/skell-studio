// Copyright (C) 2026 Skell Audio
// SPDX-License-Identifier: AGPL-3.0-only
//! Standalone Skell Tune VST3/CLAP wrapper around the pinned LYNX YIN + TD-PSOLA pitch-correction engine.

use lemon_small_fx::{Card, Control, Kind, Mark, Setting, SmallFx, Sound, Spec};
use nih_plug::prelude::{nih_export_clap, nih_export_vst3, ClapFeature, MidiConfig, Vst3SubCategory};
use std::{ffi::c_void, ptr};

unsafe extern "C" {
    fn skell_lynx_tune_new(sample_rate: i32, values: *const i32, count: i32) -> *mut c_void;
    fn skell_lynx_tune_delete(instance: *mut c_void);
    fn skell_lynx_tune_prepare(instance: *mut c_void, sample_rate: i32);
    fn skell_lynx_tune_set(instance: *mut c_void, values: *const i32, count: i32);
    fn skell_lynx_tune_note(instance: *mut c_void, on: bool, channel: i32, note: i32);
    fn skell_lynx_tune_reset(instance: *mut c_void);
    fn skell_lynx_tune_process(instance: *mut c_void, left: *mut f32, right: *mut f32, frames: i32);
    fn skell_lynx_tune_latency(instance: *const c_void) -> i32;
}

const KEYS: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
const SCALES: [&str; 14] = [
    "Chromatic", "Major", "Minor", "Harmonic Minor", "Melodic Minor", "Major Pentatonic", "Minor Pentatonic",
    "Blues", "Dorian", "Mixolydian", "Phrygian", "Lydian", "Locrian", "Custom",
];
const RANGES: [&str; 4] = ["Soprano", "Alto / Tenor", "Low male", "Instrument"];

const SETTINGS: [Setting; 8] = [
    Setting { id: "key", name: "Key", range: (0, 11), start: 0, magnet: None },
    Setting { id: "scale", name: "Scale", range: (0, 13), start: 1, magnet: None },
    Setting { id: "retune", name: "Retune", range: (0, 1000), start: 224, magnet: None },
    Setting { id: "humanize", name: "Humanize", range: (0, 1000), start: 0, magnet: Some(0) },
    Setting { id: "formant_shift", name: "Formant Shift", range: (-700, 700), start: 0, magnet: Some(0) },
    Setting { id: "transpose", name: "Transpose", range: (-12, 12), start: 0, magnet: Some(0) },
    Setting { id: "mix", name: "Mix", range: (0, 1000), start: 1000, magnet: Some(1000) },
    Setting { id: "range", name: "Voice Range", range: (0, 3), start: 1, magnet: None },
];

const CONTROLS: [Control; 8] = [
    Control::KnobWithValue { id: "key", cx: 72.0, cy: 126.0, r: 23.0, caption: "KEY", bipolar: false, width: 128.0 },
    Control::KnobWithValue { id: "scale", cx: 208.0, cy: 126.0, r: 23.0, caption: "SCALE", bipolar: false, width: 128.0 },
    Control::KnobWithValue { id: "retune", cx: 344.0, cy: 126.0, r: 23.0, caption: "RETUNE", bipolar: false, width: 128.0 },
    Control::KnobWithValue { id: "humanize", cx: 480.0, cy: 126.0, r: 23.0, caption: "HUMANIZE", bipolar: false, width: 128.0 },
    Control::KnobWithValue { id: "formant_shift", cx: 616.0, cy: 126.0, r: 23.0, caption: "FORMANT", bipolar: true, width: 128.0 },
    Control::KnobWithValue { id: "transpose", cx: 752.0, cy: 126.0, r: 23.0, caption: "TRANSPOSE", bipolar: true, width: 128.0 },
    Control::KnobWithValue { id: "mix", cx: 888.0, cy: 126.0, r: 23.0, caption: "MIX", bipolar: false, width: 128.0 },
    Control::KnobWithValue { id: "range", cx: 1024.0, cy: 126.0, r: 23.0, caption: "RANGE", bipolar: false, width: 128.0 },
];

fn text(k: usize, raw: i32) -> String {
    match k {
        0 => KEYS[raw.clamp(0, 11) as usize].to_string(),
        1 => SCALES[raw.clamp(0, 13) as usize].to_string(),
        2 => format!("{:.1} ms", 400.0 * (raw.clamp(0, 1000) as f64 / 1000.0).powi(2)),
        3 => format!("{:.1}%", raw.clamp(0, 1000) as f32 / 10.0),
        4 => format!("{:+.2} st", raw.clamp(-700, 700) as f32 / 100.0),
        5 => format!("{:+} st", raw.clamp(-12, 12)),
        6 => format!("{:.1}%", raw.clamp(0, 1000) as f32 / 10.0),
        7 => RANGES[raw.clamp(0, 3) as usize].to_string(),
        _ => String::new(),
    }
}

fn parse(k: usize, input: &str) -> Option<i32> {
    let s = input.trim();
    let choice = |values: &[&str]| values.iter().position(|name| name.eq_ignore_ascii_case(s)).map(|v| v as i32);
    match k {
        0 => choice(&KEYS),
        1 => choice(&SCALES),
        7 => choice(&RANGES),
        2 => {
            let ms = s.trim_end_matches("ms").trim().parse::<f64>().ok()?;
            (ms.is_finite() && ms >= 0.0).then(|| (1000.0 * (ms.clamp(0.0, 400.0) / 400.0).sqrt()).round() as i32)
        }
        3 | 6 => {
            let value = s.trim_end_matches('%').trim().parse::<f64>().ok()?;
            value.is_finite().then(|| (value.clamp(0.0, 100.0) * 10.0).round() as i32)
        }
        4 => {
            let value = s.trim_end_matches("st").trim().parse::<f64>().ok()?;
            value.is_finite().then(|| (value * 100.0).round().clamp(-700.0, 700.0) as i32)
        }
        5 => {
            let value = s.trim_end_matches("st").trim().parse::<f64>().ok()?;
            value.is_finite().then(|| value.round().clamp(-12.0, 12.0) as i32)
        }
        _ => None,
    }
}

struct TuneSound {
    instance: ptr::NonNull<c_void>,
    rate: u32,
}
unsafe impl Send for TuneSound {}

impl TuneSound {
    fn new(rate: u32, values: &[i32]) -> Self {
        let rate = rate.clamp(8000, 192000);
        let instance = ptr::NonNull::new(unsafe { skell_lynx_tune_new(rate as i32, values.as_ptr(), values.len() as i32) })
            .expect("LYNX Tune engine allocation");
        let mut sound = Self { instance, rate };
        sound.set(values);
        sound
    }
}

impl Sound for TuneSound {
    fn set(&mut self, values: &[i32]) {
        if values.len() >= SETTINGS.len() {
            unsafe { skell_lynx_tune_set(self.instance.as_ptr(), values.as_ptr(), SETTINGS.len() as i32); }
        }
    }
    fn midi_note(&mut self, on: bool, channel: u8, note: u8) {
        unsafe { skell_lynx_tune_note(self.instance.as_ptr(), on, i32::from(channel), i32::from(note)); }
    }
    fn set_sample_rate(&mut self, rate: u32) {
        self.rate = rate.clamp(8000, 192000);
        unsafe { skell_lynx_tune_prepare(self.instance.as_ptr(), self.rate as i32); }
    }
    fn reset(&mut self) { unsafe { skell_lynx_tune_reset(self.instance.as_ptr()); } }
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let frames = left.len().min(right.len());
        if frames > 0 {
            unsafe { skell_lynx_tune_process(self.instance.as_ptr(), left.as_mut_ptr(), right.as_mut_ptr(), frames as i32); }
        }
    }
    fn latency(&self) -> u32 { unsafe { skell_lynx_tune_latency(self.instance.as_ptr()).max(0) as u32 } }
}
impl Drop for TuneSound {
    fn drop(&mut self) { unsafe { skell_lynx_tune_delete(self.instance.as_ptr()); } }
}

fn sound(rate: u32, values: &[i32]) -> Box<dyn Sound> { Box::new(TuneSound::new(rate, values)) }

pub const SPEC: &Spec = &Spec {
    name: "Skell Tune",
    word: "TUNE",
    about: "Real-time vocal pitch correction with YIN pitch tracking and TD-PSOLA formant-aware shifting",
    settings: &SETTINGS,
    version: Some(1),
    any_version: false,
    fields: &[0, 1, 2, 3, 4, 5, 6, 7],
    tail: &[],
    blank: None,
    locate: None,
    size: (1100, 300),
    cards: &[Card { x0: 8.0, x1: 1092.0, y0: 22.0, y1: 195.0, heading: "PITCH CORRECTION" }],
    controls: &CONTROLS,
    display: None,
    mark: Mark { x: 1088.0, y: 286.0, anchor: 1.0, stacked: false },
    hint: None,
    labels: &[(550.0, 220.0, "MIDI INPUT: ALL CHANNELS  ·  MIDI NOTES BECOME THE TARGET SCALE")],
    text,
    parse,
    sound,
    rings: false,
};

pub struct TuneKind;
impl Kind for TuneKind {
    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const VENDOR: &'static str = "Skell Audio";
    const SPEC: &'static Spec = SPEC;
    const CLAP_ID: &'static str = "com.skellstudio.skell-tune";
    const CLAP_DESCRIPTION: &'static str = "Real-time pitch correction powered by the LYNX YIN and TD-PSOLA engine";
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect, ClapFeature::Stereo];
    const VST3_CLASS_ID: [u8; 16] = *b"SkellStudioTune1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Fx];
}

pub type SkellTune = SmallFx<TuneKind>;
#[cfg(not(feature = "linked"))]
nih_export_clap!(SkellTune);
#[cfg(not(feature = "linked"))]
nih_export_vst3!(SkellTune);

#[cfg(test)]
mod tests {
    use super::*;
    use lemon_small_fx::params::FxParams;
    use nih_plug::prelude::Params;

    fn tone(rate: u32, hz: f32, seconds: f32) -> Vec<f32> {
        (0..(rate as f32 * seconds) as usize)
            .map(|i| 0.25 * (std::f32::consts::TAU * hz * i as f32 / rate as f32).sin())
            .collect()
    }

    #[test]
    fn control_text_and_parse_are_stable() {
        for (k, setting) in SETTINGS.iter().enumerate() {
            for raw in setting.range.0..=setting.range.1 {
                let label = text(k, raw);
                let parsed = parse(k, &label).unwrap_or_else(|| panic!("setting {}: {raw} -> {label}", setting.id));
                assert_eq!(text(k, parsed), label, "setting {}: {raw}", setting.id);
            }
        }
    }

    #[test]
    fn host_state_fields_roundtrip_and_identity_is_new() {
        let state = lemon_small_fx::params::default_state(SPEC);
        let params = FxParams::new(SPEC);
        params.set_raw(state.clone());
        let fields = params.serialize_fields();
        let restored = FxParams::new(SPEC);
        restored.deserialize_fields(&fields);
        assert_eq!(restored.raw_copy(), state);
        assert_eq!(TuneKind::CLAP_ID, "com.skellstudio.skell-tune");
        assert_eq!(TuneKind::VST3_CLASS_ID, *b"SkellStudioTune1");
    }

    #[test]
    fn editor_exposes_all_eight_controls_and_can_capture() {
        let params = std::sync::Arc::new(lemon_small_fx::params::FxParams::new(SPEC));
        let mut logic = lemon_small_fx::editor::Logic::new(
            SPEC,
            params,
            std::sync::Arc::new(lemon_small_fx::Shared::default()),
            None,
        );
        let mut canvas = os_ui::Canvas::new(SPEC.size.0, SPEC.size.1, 1.0).unwrap();
        logic.paint(&mut canvas, &os_plugkit::palette());
        assert_eq!(
            logic.hits.iter().filter(|h| matches!(h.t, lemon_small_fx::view::Target::Knob(_))).count(),
            SETTINGS.len()
        );
        if let Some(dir) = std::env::var_os("OS_SHOT_DIR") {
            let dir = std::path::PathBuf::from(dir);
            canvas.pixmap.save_png(dir.join("skell-tune.png")).unwrap();
            logic.typed("retune", "100 ms");
            logic.typed("scale", "Harmonic Minor");
            logic.typed("formant_shift", "+3.00 st");
            assert_eq!(logic.value_text("retune"), "100.0 ms");
            assert_eq!(logic.value_text("scale"), "Harmonic Minor");
            let mut scaled = os_ui::Canvas::new(SPEC.size.0 * 3 / 2, SPEC.size.1 * 3 / 2, 1.5).unwrap();
            logic.paint(&mut scaled, &os_plugkit::palette());
            scaled.pixmap.save_png(dir.join("skell-tune-150pct-edited.png")).unwrap();
        }
    }

    #[test]
    fn corrects_off_scale_tone_to_the_nearest_c_major_note() {
        fn estimate_hz(samples: &[f32], rate: f32) -> Option<f32> {
            let mut crossings = Vec::with_capacity(512);
            for (i, pair) in samples.windows(2).enumerate() {
                if pair[0] <= 0.0 && pair[1] > 0.0 {
                    let fraction = -pair[0] / (pair[1] - pair[0]).max(f32::MIN_POSITIVE);
                    crossings.push(i as f32 + fraction);
                }
            }
            let first = *crossings.first()?;
            let last = *crossings.last()?;
            let cycles = crossings.len().saturating_sub(1);
            (cycles > 0 && last > first).then_some(rate * cycles as f32 / (last - first))
        }

        let rate = 48000u32;
        let mut sound = TuneSound::new(rate, &[0, 1, 0, 0, 0, 0, 1000, 1]);
        // 238 Hz is about 58.36 MIDI: B3 is nearer than A3 in C major, avoiding an exact tie.
        let mut left = tone(rate, 238.0f32, 1.5);
        let mut right = left.clone();
        sound.process(&mut left, &mut right);
        let measured = estimate_hz(&left[rate as usize..], rate as f32)
            .expect("corrected output should contain stable positive-going crossings");
        // A#3 is outside C major; B3 (246.94 Hz) is the closest allowed note.
        assert!((measured - 246.94).abs() < 4.0, "expected B3, got {measured:.2} Hz");
    }

    #[test]
    fn restored_voice_range_has_correct_latency_before_first_audio() {
        for rate in [44_100, 48_000, 96_000] {
            for range in 0..=3 {
                let mut values = SPEC.start();
                values[7] = range;
                let mut sound = TuneSound::new(rate, &values);
                let initial = sound.latency();
                let mut left = [0.0f32; 127];
                let mut right = left;
                sound.process(&mut left, &mut right);
                assert!(initial > 0);
                assert_eq!(sound.latency(), initial, "rate={rate}, range={range}");
            }
        }
    }

    #[test]
    fn retune_display_preserves_existing_ms_mapping() {
        assert_eq!(parse(2, "0 ms"), Some(0));
        assert_eq!(parse(2, "100 ms"), Some(500));
        assert_eq!(text(2, 500), "100.0 ms");
        assert_eq!(text(2, 1000), "400.0 ms");
    }

    #[test]
    fn engine_processes_audio_with_stable_pdc_and_midi_input() {
        let mut sound = TuneSound::new(48000, &SPEC.start());
        assert_eq!(sound.latency(), 1204);
        sound.midi_note(true, 0, 60);
        let mut left = tone(48000, 233.082f32, 1.5);
        let mut right = left.clone();
        let blocks = [1usize, 17, 64, 127, 512, 2048];
        let mut offset = 0;
        let mut index = 0;
        while offset < left.len() {
            let end = (offset + blocks[index % blocks.len()]).min(left.len());
            sound.process(&mut left[offset..end], &mut right[offset..end]);
            offset = end;
            index += 1;
        }
        sound.midi_note(false, 0, 60);
        assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
        assert!(left.iter().any(|sample| sample.abs() > 1.0e-4));
    }
}

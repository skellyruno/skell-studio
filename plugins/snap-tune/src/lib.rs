// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: AGPL-3.0
//! SnapTune pitch correction as a standalone VST3 / CLAP effect.
//!
//! The pitch engine is kept in os-dsp's isolated SnapTune C ABI. This crate is
//! built as its own plug-in and is deliberately not linked into Skell Studio,
//! preserving the AGPL boundary from the GPL Studio application.

use lemon_small_fx::{
    Card, ClapFeature, Control, Kind, Mark, Setting, SmallFx, Sound, Spec, Vst3SubCategory,
};
use nih_plug::prelude::{nih_export_clap, nih_export_vst3};
use os_dsp::snaptune::{SnapTune, KEYS, SCALES};

const VOICES: &[&str] = &["Low Male", "Soprano"];
const LEARN: &[&str] = &["Off", "Learn"];

const SETTINGS: [Setting; 5] = [
    Setting { id: "amount", name: "Correction Amount", range: (0, 1000), start: 1000, magnet: Some(1000) },
    Setting { id: "key", name: "Key", range: (0, 11), start: 0, magnet: None },
    Setting { id: "scale", name: "Scale", range: (0, 12), start: 0, magnet: None },
    Setting { id: "voice", name: "Voice Range", range: (0, 1), start: 0, magnet: None },
    Setting { id: "learn", name: "Learn", range: (0, 1), start: 0, magnet: None },
];

const CONTROLS: [Control; 5] = [
    Control::Knob { id: "amount", cx: 76.0, cy: 135.0, r: 25.0, caption: "AMOUNT", bipolar: false },
    Control::Knob { id: "key", cx: 184.0, cy: 135.0, r: 22.0, caption: "KEY", bipolar: false },
    Control::Knob { id: "scale", cx: 292.0, cy: 135.0, r: 22.0, caption: "SCALE", bipolar: false },
    Control::Choice { id: "voice", x: 395.0, top: 80.0, names: VOICES },
    Control::Choice { id: "learn", x: 520.0, top: 80.0, names: LEARN },
];

fn text(k: usize, v: i32) -> String {
    match k {
        0 => format!("{:.1}%", f64::from(v.clamp(0, 1000)) / 10.0),
        1 => KEYS[v.clamp(0, 11) as usize].to_string(),
        2 => SCALES[v.clamp(0, 12) as usize].to_string(),
        3 => VOICES[v.clamp(0, 1) as usize].to_string(),
        4 => if v == 0 { "Off".into() } else { "Learn".into() },
        _ => String::new(),
    }
}

fn parse(k: usize, value: &str) -> Option<i32> {
    let s = value.trim();
    match k {
        0 => {
            let n = s.trim_end_matches('%').trim().parse::<f64>().ok()?;
            (n.is_finite()).then(|| (n * 10.0).round().clamp(0.0, 1000.0) as i32)
        }
        1 => KEYS.iter().position(|name| name.eq_ignore_ascii_case(s)).map(|v| v as i32)
            .or_else(|| {
                KEYS.iter().position(|name| name.split('/').next().is_some_and(|part| part.trim().eq_ignore_ascii_case(s)))
                    .map(|v| v as i32)
            }),
        2 => SCALES.iter().position(|name| name.eq_ignore_ascii_case(s)).map(|v| v as i32),
        3 => VOICES.iter().position(|name| name.eq_ignore_ascii_case(s)).map(|v| v as i32),
        4 if s.eq_ignore_ascii_case("off") || s.eq_ignore_ascii_case("stop") => Some(0),
        4 if s.eq_ignore_ascii_case("learn") || s.eq_ignore_ascii_case("on") => Some(1),
        _ => None,
    }
}

struct TuneSound {
    rate: u32,
    values: [i32; 5],
    dsp: SnapTune,
    /// Interleaved stereo staging memory is allocated once per instance, never in process().
    scratch: Vec<f32>,
}

impl TuneSound {
    fn new(rate: u32, values: &[i32]) -> Self {
        let mut current = [1000, 0, 0, 0, 0];
        for (dst, src) in current.iter_mut().zip(values.iter().copied()) {
            *dst = src;
        }
        let rate = rate.clamp(8000, 384000);
        let settings = [
            ((current[0].clamp(0, 1000) as u32 * 65535 + 500) / 1000) as u16,
            current[1].clamp(0, 11) as u16,
            current[2].clamp(0, 12) as u16,
            current[3].clamp(0, 1) as u16,
        ];
        let dsp = SnapTune::new(rate, settings, current[4].clamp(0, 1) as u32);
        Self { rate, values: current, dsp, scratch: vec![0.0; 8192] }
    }

    fn update(&mut self, values: &[i32]) {
        for (dst, src) in self.values.iter_mut().zip(values.iter().copied()) {
            *dst = src;
        }
        self.dsp.configure(
            self.values[0].clamp(0, 1000) as f32 / 1000.0,
            self.values[1].clamp(0, 11) as u8,
            self.values[2].clamp(0, 12) as u8,
            self.values[3].clamp(0, 1) as u8,
            self.values[4].clamp(0, 1) as u32,
            true,
        );
    }
}

impl Sound for TuneSound {
    fn set(&mut self, values: &[i32]) {
        self.update(values);
    }

    fn set_sample_rate(&mut self, rate: u32) {
        let rate = rate.clamp(8000, 384000);
        if rate == self.rate { return; }
        self.rate = rate;
        let settings = [
            ((self.values[0].clamp(0, 1000) as u32 * 65535 + 500) / 1000) as u16,
            self.values[1].clamp(0, 11) as u16,
            self.values[2].clamp(0, 12) as u16,
            self.values[3].clamp(0, 1) as u16,
        ];
        self.dsp = SnapTune::new(rate, settings, self.values[4].clamp(0, 1) as u32);
    }

    fn reset(&mut self) {
        self.dsp.reset();
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let frames = left.len().min(right.len());
        let mut offset = 0;
        while offset < frames {
            let count = (frames - offset).min(self.scratch.len() / 2);
            let interleaved = &mut self.scratch[..count * 2];
            for i in 0..count {
                interleaved[i * 2] = left[offset + i];
                interleaved[i * 2 + 1] = right[offset + i];
            }
            self.dsp.process(interleaved);
            for i in 0..count {
                left[offset + i] = interleaved[i * 2];
                right[offset + i] = interleaved[i * 2 + 1];
            }
            offset += count;
        }
    }

    fn latency(&self) -> u32 {
        self.dsp.latency()
    }
}

fn sound(rate: u32, values: &[i32]) -> Box<dyn Sound> {
    Box::new(TuneSound::new(rate, values))
}

pub const SPEC: &Spec = &Spec {
    name: "Skell Tune",
    word: "TUNE",
    about: "Live pitch correction and key/scale-aware vocal tuning powered by SnapTune",
    settings: &SETTINGS,
    version: Some(1),
    any_version: false,
    fields: &[],
    tail: &[],
    blank: None,
    locate: None,
    size: (640, 292),
    cards: &[Card { x0: 8.0, x1: 632.0, y0: 22.0, y1: 224.0, heading: "PITCH CORRECTION" }],
    controls: &CONTROLS,
    display: None,
    mark: Mark { x: 624.0, y: 273.0, anchor: 1.0, stacked: false },
    hint: Some(os_ui::Rect::new(12.0, 246.0, 420.0, 18.0)),
    labels: &[(335.0, 246.0, "LIVE MODE • LOW-LATENCY PITCH TRACKING")],
    text,
    parse,
    sound,
    rings: false,
};

pub struct TuneKind;

impl Kind for TuneKind {
    const SPEC: &'static Spec = SPEC;
    const CLAP_ID: &'static str = "com.skellstudio.snaptune";
    const CLAP_DESCRIPTION: &'static str = "Low-latency vocal pitch correction with key and scale control";
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect, ClapFeature::Stereo];
    const VST3_CLASS_ID: [u8; 16] = *b"SkellStudioTune1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Fx];
}

pub type SkellTune = SmallFx<TuneKind>;

#[cfg(not(feature = "linked"))]
nih_export_clap!(SkellTune);
#[cfg(not(feature = "linked"))]
nih_export_vst3!(SkellTune);

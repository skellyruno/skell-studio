// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: AGPL-3.0-only
//! Standalone Skell Beat VST3/CLAP effect. The processing core is the clean-room
//! LYNX Gross Beat engine, isolated from LYNX's DAW-specific Effect wrapper.

use lemon_small_fx::{Control, Kind, Mark, Setting, SmallFx, Sound, Spec};
use nih_plug::prelude::{nih_export_clap, nih_export_vst3, ClapFeature, Vst3SubCategory};
use std::{ffi::c_void, ptr};
use std::sync::Arc;

mod editor;

unsafe extern "C" {
    fn lynx_gb_factory_state(out: *mut u8, capacity: i32) -> i32;
    fn lynx_gb_new(sample_rate: i32, max_block: i32) -> *mut c_void;
    fn lynx_gb_delete(instance: *mut c_void);
    fn lynx_gb_load(instance: *mut c_void, data: *const u8, size: usize) -> bool;
    fn lynx_gb_prepare(instance: *mut c_void, sample_rate: i32, max_block: i32);
    fn lynx_gb_set_param(instance: *mut c_void, index: i32, raw: i32);
    fn lynx_gb_note(instance: *mut c_void, on: bool, channel: i32, note: i32) -> i32;
    fn lynx_gb_set_transport(instance: *mut c_void, tempo: f64, position_beats: f64, has_position: bool, playing: bool);
    fn lynx_gb_process(instance: *mut c_void, left: *mut f32, right: *mut f32, frames: i32);
    fn lynx_gb_reset(instance: *mut c_void);
    fn lynx_gb_latency() -> i32;
    fn lynx_gb_state_point_count(data: *const u8, size: usize, slot: i32) -> i32;
    fn lynx_gb_state_get_point(data: *const u8, size: usize, slot: i32, index: i32,
        x: *mut f64, y: *mut f32, tension: *mut f32, kind: *mut i32) -> bool;
    fn lynx_gb_state_render_slot(data: *const u8, size: usize, slot: i32, out: *mut f32, count: i32) -> i32;
    fn lynx_gb_state_midi_trigger(data: *const u8, size: usize) -> i32;
    fn lynx_gb_state_thumbnails(data: *const u8, size: usize,
        out: *mut f32, samples: i32, names: *mut u8, name_bytes: i32) -> i32;
    fn lynx_gb_state_edit(data: *const u8, size: usize, operation: i32, slot: i32, index: i32,
        x: f64, y: f32, kind: i32, tension: f32, out: *mut u8, capacity: i32, result_index: *mut i32) -> i32;
    fn lynx_gb_state_set_midi_trigger(data: *const u8, size: usize, enabled: bool,
        out: *mut u8, capacity: i32) -> i32;
    fn lynx_gb_queue_state(instance: *mut c_void, data: *const u8, size: usize) -> bool;
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
    // Automation IDs remain stable. LYNX's grid selects slots; these knobs remain
    // accessible for hosts and existing project automation at the panel edges.
    Control::Knob { id: "time_slot", cx: 46.0, cy: 71.0, r: 18.0, caption: "TIME SLOT", bipolar: false },
    Control::Knob { id: "time_mix", cx: 121.0, cy: 71.0, r: 18.0, caption: "TIME MIX", bipolar: false },
    Control::Knob { id: "hold", cx: 197.0, cy: 71.0, r: 18.0, caption: "HOLD", bipolar: false },
    Control::Knob { id: "volume_slot", cx: 273.0, cy: 71.0, r: 18.0, caption: "VOL SLOT", bipolar: false },
    Control::Knob { id: "volume_mix", cx: 46.0, cy: 656.0, r: 18.0, caption: "VOL MIX", bipolar: false },
    Control::Knob { id: "attack", cx: 121.0, cy: 656.0, r: 18.0, caption: "ATTACK", bipolar: false },
    Control::Knob { id: "release", cx: 197.0, cy: 656.0, r: 18.0, caption: "RELEASE", bipolar: false },
    Control::Knob { id: "tension", cx: 273.0, cy: 656.0, r: 18.0, caption: "TENSION", bipolar: true },
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
    fn bind_editor(&mut self, extension: Arc<dyn lemon_small_fx::editor::Extension>) {
        extension.bind_engine_handle(self.instance.as_ptr() as usize);
    }

    fn state_capacity_hint(&self) -> usize { 1_048_576 }

    fn midi_note(&mut self, on: bool, channel: u8, note: u8) {
        unsafe { lynx_gb_note(self.instance.as_ptr(), on, i32::from(channel), i32::from(note)); }
    }

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
    size: (1100, 720),
    cards: &[],
    controls: &CONTROLS,
    display: None,
    mark: Mark { x: 1088.0, y: 698.0, anchor: 1.0, stacked: false },
    hint: None,
    labels: &[],
    text,
    parse,
    sound,
    rings: false,
};

pub struct BeatKind;

impl Kind for BeatKind {
    fn editor_extension(params: Arc<lemon_small_fx::params::FxParams>) -> Option<Arc<dyn lemon_small_fx::editor::Extension>> {
        Some(Arc::new(editor::BeatEditorExtension::new(params)))
    }
    const MIDI_INPUT: nih_plug::prelude::MidiConfig = nih_plug::prelude::MidiConfig::Basic;
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
    fn curve_state_editing_preserves_all_72_slots_and_midi_option() {
        let state = factory_state();
        let slot = 12;
        let count = unsafe { lynx_gb_state_point_count(state.as_ptr(), state.len(), slot) };
        assert!(count >= 2);
        let (mut x, mut y, mut tension, mut kind) = (0.0, 0.0, 0.0, 0);
        assert!(unsafe { lynx_gb_state_get_point(state.as_ptr(), state.len(), slot, 1,
            &mut x, &mut y, &mut tension, &mut kind) });
        let mut selected = -1;
        let needed = unsafe { lynx_gb_state_edit(state.as_ptr(), state.len(), 3, slot, 1,
            x, y, kind, 0.25, std::ptr::null_mut(), 0, &mut selected) };
        assert!(needed > 0);
        let mut changed = vec![0u8; needed as usize];
        assert_eq!(unsafe { lynx_gb_state_edit(state.as_ptr(), state.len(), 3, slot, 1,
            x, y, kind, 0.25, changed.as_mut_ptr(), needed, &mut selected) }, needed);
        assert!(changed.len() >= state.len() - 64);
        for other in 0..72 {
            assert!(unsafe { lynx_gb_state_point_count(changed.as_ptr(), changed.len(), other) } >= 2,
                "slot {other} was lost during state serialization");
        }
        assert_eq!(unsafe { lynx_gb_state_midi_trigger(changed.as_ptr(), changed.len()) }, 1);
        let midi_needed = unsafe { lynx_gb_state_set_midi_trigger(changed.as_ptr(), changed.len(),
            false, std::ptr::null_mut(), 0) };
        assert!(midi_needed > 0);
        let mut midi_off = vec![0u8; midi_needed as usize];
        assert_eq!(unsafe { lynx_gb_state_set_midi_trigger(changed.as_ptr(), changed.len(),
            false, midi_off.as_mut_ptr(), midi_needed) }, midi_needed);
        assert_eq!(unsafe { lynx_gb_state_midi_trigger(midi_off.as_ptr(), midi_off.len()) }, 0);
        let mut px = 0.0; let mut py = 0.0; let mut pt = 0.0; let mut pk = 0;
        assert!(unsafe { lynx_gb_state_get_point(midi_off.as_ptr(), midi_off.len(), slot, 1,
            &mut px, &mut py, &mut pt, &mut pk) });
        assert!((pt - 0.25).abs() < 1e-5);
        assert_eq!(px, x); assert_eq!(py, y); assert_eq!(pk, kind);
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
    fn midi_notes_select_the_matching_time_and_volume_slots() {
        let state = factory_state();
        let instance = ptr::NonNull::new(unsafe { lynx_gb_new(48000, 4096) })
            .expect("Gross Beat bridge allocation");
        assert!(unsafe { lynx_gb_load(instance.as_ptr(), state.as_ptr(), state.len()) });
        // Gross Beat's key map: C5..B7 => time 1..36; C2..B4 => volume 1..36.
        assert_eq!(unsafe { lynx_gb_note(instance.as_ptr(), true, 0, 60) }, 0);
        assert_eq!(unsafe { lynx_gb_note(instance.as_ptr(), true, 0, 95) }, 35);
        assert_eq!(unsafe { lynx_gb_note(instance.as_ptr(), true, 0, 24) }, 36);
        assert_eq!(unsafe { lynx_gb_note(instance.as_ptr(), true, 0, 59) }, 71);
        assert_eq!(unsafe { lynx_gb_note(instance.as_ptr(), true, 0, 23) }, -1);
        assert_eq!(unsafe { lynx_gb_note(instance.as_ptr(), true, 0, 96) }, -1);
        assert_eq!(unsafe { lynx_gb_note(instance.as_ptr(), true, 16, 60) }, -1);
        unsafe { lynx_gb_delete(instance.as_ptr()); }
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
    fn synthetic_audio_diagnostics_44100_48000_96000() {
        use std::time::Instant;
        // Host buffer-size invariance and objective dry/wet measurements.
        // This test never claims equivalence with recordings or FL Studio.
        fn render(rate: u32, block: usize) -> (Vec<f32>, Vec<f32>, u128) {
            let state = factory_state();
            let mut sound = BeatSound::new(rate, &[0, 0, 256, 256, 1765, 5000, 0, 0]);
            sound.set_saved_state(&state);
            sound.set(&[1, 0, 256, 256, 1765, 5000, 0, 0]);
            let frames = rate as usize;
            let mut left = Vec::with_capacity(frames);
            let mut right = Vec::with_capacity(frames);
            for i in 0..frames {
                let t = i as f32 / rate as f32;
                left.push(0.17 * (std::f32::consts::TAU * 220.0 * t).sin()
                    + 0.07 * (std::f32::consts::TAU * 937.0 * t).sin()
                    + if i == 0 { 0.45 } else { 0.0 });
                right.push(0.12 * (std::f32::consts::TAU * 330.0 * t).sin()
                    + if i == frames / 8 { -0.35 } else { 0.0 });
            }
            let now = Instant::now();
            for offset in (0..frames).step_by(block) {
                let end = (offset + block).min(frames);
                let position_beats = offset as f64 / rate as f64 * 2.0;
                sound.set_transport(120.0, Some(position_beats), true);
                sound.process(&mut left[offset..end], &mut right[offset..end]);
            }
            let nanos = now.elapsed().as_nanos();
            assert_eq!(sound.latency(), 15, "published 15-sample latency changed at {rate}");
            assert!(left.iter().chain(&right).all(|v| v.is_finite()),
                "NaN/Inf in Skell Beat audio output at {rate} Hz, block={block}");
            (left, right, nanos)
        }

        let mut metrics = Vec::new();
        metrics.push("Skell Beat | synthetic input diagnostics (not recorded audio A/B)".to_string());
        metrics.push("Runner wall-clock timing includes host transport calls; no hard CPU target.".to_string());
        for rate in [44_100, 48_000, 96_000] {
            let (reference_l, reference_r, elapsed_ns) = render(rate, 4096);
            let peak = reference_l.iter().chain(&reference_r)
                .fold(0.0f32, |m, &v| m.max(v.abs()));
            let rms = (reference_l.iter().chain(&reference_r)
                .map(|v| f64::from(*v) * f64::from(*v)).sum::<f64>()
                / (reference_l.len() + reference_r.len()) as f64).sqrt();
            assert!(rms.is_finite() && peak.is_finite());
            for block in [17, 64, 257] {
                let (a, b, _) = render(rate, block);
                let difference = a.iter().zip(&reference_l).chain(b.iter().zip(&reference_r))
                    .map(|(x, y)| (*x - *y).abs())
                    .fold(0.0f32, f32::max);
                assert!(difference < 2.0e-6,
                    "Skell Beat block-size dependent output: rate={rate} block={block} diff={difference}");
            }
            let line = format!("rate_hz={rate} input=stereo_multitone_impulses seconds=1.0 blocks=17/64/257/4096 latency_samples=15 wet_peak={peak:.6} wet_rms={rms:.6} wall_ms={:.3} wall_ratio={:.5}",
                elapsed_ns as f64 / 1.0e6, elapsed_ns as f64 / 1.0e9);
            println!("{line}");
            metrics.push(line);
        }
        if let Some(path) = std::env::var_os("SKELL_DSP_METRICS_PATH") {
            // Test harness thread only, never the DSP callback.
            std::fs::write(path, metrics.join("\n") + "\n").expect("write diagnostic report");
        }
    }

    #[test]
    fn all_72_slot_thumbnails_and_titles_are_batched_from_lynx_state() {
        let state = factory_state();
        let mut levels = [0.0f32; 72 * 16];
        let mut titles = [0u8; 72 * 32];
        let count = unsafe { lynx_gb_state_thumbnails(state.as_ptr(), state.len(),
            levels.as_mut_ptr(), 16, titles.as_mut_ptr(), 32) };
        assert_eq!(count, 72);
        assert!(levels.iter().all(|v| v.is_finite() && *v >= 0.0 && *v <= 1.0));
    }

    #[test]
    fn repeated_editor_state_swaps_keep_audio_finite() {
        // Exercise the same pending/retired snapshot handoff as rapid curve edits.
        // Destruction and reclamation must happen outside lynx_gb_process().
        let state = factory_state();
        let instance = ptr::NonNull::new(unsafe { lynx_gb_new(48000, 256) })
            .expect("Gross Beat bridge allocation");
        assert!(unsafe { lynx_gb_load(instance.as_ptr(), state.as_ptr(), state.len()) });
        for _ in 0..192 {
            assert!(unsafe { lynx_gb_queue_state(instance.as_ptr(), state.as_ptr(), state.len()) });
            let mut left = [0.125f32; 64];
            let mut right = [-0.125f32; 64];
            unsafe { lynx_gb_process(instance.as_ptr(), left.as_mut_ptr(), right.as_mut_ptr(), 64) };
            assert!(left.iter().chain(right.iter()).all(|value| value.is_finite()));
        }
        unsafe { lynx_gb_delete(instance.as_ptr()) };
    }

    #[test]
    fn editor_exposes_eight_host_controls_and_the_full_curve_workspace() {
        let params = Arc::new(lemon_small_fx::params::FxParams::new(SPEC));
        let mut shared = lemon_small_fx::Shared::default();
        shared.extension = BeatKind::editor_extension(params.clone());
        let shared = Arc::new(shared);
        let mut logic = editor::Logic::new(SPEC, params, shared, None);
        let mut canvas = os_ui::Canvas::new(SPEC.size.0, SPEC.size.1, 1.0).unwrap();
        logic.paint(&mut canvas, &os_plugkit::palette());
        assert_eq!(logic.hits.iter().filter(|h| matches!(h.t, lemon_small_fx::view::Target::Knob(_))).count(), 8);
        if let Some(dir) = std::env::var_os("OS_SHOT_DIR") {
            canvas.pixmap.save_png(std::path::PathBuf::from(dir).join("skell-beat.png")).unwrap();
        }
    }
}

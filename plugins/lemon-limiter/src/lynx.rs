// Copyright (C) 2026 Skelly Audio
// SPDX-License-Identifier: AGPL-3.0-only

use std::ffi::c_void;
use std::ptr::NonNull;

unsafe extern "C" {
    fn skell_limiter_new(sample_rate: i32) -> *mut c_void;
    fn skell_limiter_delete(instance: *mut c_void);
    fn skell_limiter_set(instance: *mut c_void, raw: *const i32, exact_ceiling: bool);
    fn skell_limiter_reset(instance: *mut c_void);
    fn skell_limiter_latency(instance: *mut c_void) -> i32;
    fn skell_limiter_asleep(instance: *mut c_void) -> bool;
    fn skell_limiter_process(instance: *mut c_void, left: *mut f32, right: *mut f32,
        side_left: *const f32, side_right: *const f32, frames: i32);
}

/// Pinned LYNX SHARE Fruity Limiter engine behind the unchanged ML Limiter wrapper.
pub struct LynxLimiter { instance: NonNull<c_void> }
unsafe impl Send for LynxLimiter {}

impl LynxLimiter {
    pub fn new(sample_rate: u32) -> Option<Self> {
        let rate = sample_rate.clamp(8000, 384000);
        let instance = NonNull::new(unsafe { skell_limiter_new(rate as i32) })?;
        Some(Self { instance })
    }
    #[inline]
    pub fn set(&mut self, raw: &[i32; 18], flags: u8) {
        unsafe { skell_limiter_set(self.instance.as_ptr(), raw.as_ptr(), flags & 1 != 0); }
    }
    #[inline]
    pub fn reset(&mut self) { unsafe { skell_limiter_reset(self.instance.as_ptr()) }; }
    #[inline]
    pub fn latency(&self) -> u32 {
        unsafe { skell_limiter_latency(self.instance.as_ptr()).max(0) as u32 }
    }
    #[inline]
    pub fn asleep(&self) -> bool { unsafe { skell_limiter_asleep(self.instance.as_ptr()) } }
    #[inline]
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32], sidechain: Option<(&[f32], &[f32])>) {
        let frames = left.len().min(right.len());
        if frames == 0 { return; }
        let (side_left, side_right) = sidechain
            .map(|(l, r)| (l.as_ptr(), r.as_ptr()))
            .unwrap_or((std::ptr::null(), std::ptr::null()));
        unsafe {
            skell_limiter_process(self.instance.as_ptr(), left.as_mut_ptr(), right.as_mut_ptr(),
                side_left, side_right, frames as i32);
        }
    }
}
impl Drop for LynxLimiter {
    fn drop(&mut self) { unsafe { skell_limiter_delete(self.instance.as_ptr()) }; }
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_engine::effects::dynamics::DEFAULTS;

    fn sine(rate: u32, frames: usize, amplitude: f32) -> Vec<f32> {
        (0..frames).map(|i| amplitude *
            (std::f32::consts::TAU * 220.0 * i as f32 / rate as f32).sin()).collect()
    }
    fn render(rate: u32, host_block: usize) -> Vec<f32> {
        let mut engine = LynxLimiter::new(rate).expect("Limiter engine initialization");
        engine.set(&DEFAULTS, 0);
        let mut left = sine(rate, rate as usize / 2, 1.8);
        let mut right = left.clone();
        for start in (0..left.len()).step_by(host_block) {
            let end = (start + host_block).min(left.len());
            engine.process(&mut left[start..end], &mut right[start..end], None);
        }
        assert!(left.iter().chain(&right).all(|x| x.is_finite()));
        left
    }
    #[test]
    fn default_state_reports_lookahead_and_limits_output() {
        let mut engine = LynxLimiter::new(44100).unwrap();
        engine.set(&DEFAULTS, 0);
        assert_eq!(engine.latency(), 88);
        let mut left = sine(44100, 44100, 1.8);
        let mut right = left.clone();
        engine.process(&mut left, &mut right, None);
        assert!(left.iter().chain(&right).all(|x| x.is_finite()));
        assert!(left.iter().take(88).all(|x| x.abs() < 1.0e-7));
        let peak = left[22050..].iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(peak <= 1.001 && peak > 0.9, "post-limiter peak {peak}");
    }
    #[test]
    fn variable_host_blocks_keep_output_aligned() {
        let reference = render(48000, 512);
        for block in [1, 17, 64, 127, 256, 1024, 4096] {
            let candidate = render(48000, block);
            assert!(reference.iter().zip(candidate).all(|(a,b)| (a-b).abs() < 2.0e-6),
                "output differs at host block size {block}");
        }
    }
    #[test]
    fn sidechain_controls_the_compressor_detector() {
        let mut raw = DEFAULTS;
        raw[8] = 600;
        raw[9] = 1000;
        let input = sine(44100, 8820, 0.9);
        let silent = vec![0.0f32; input.len()];
        let mut keyed = LynxLimiter::new(44100).unwrap();
        let mut self_keyed = LynxLimiter::new(44100).unwrap();
        keyed.set(&raw, 0);
        self_keyed.set(&raw, 0);
        let (mut kl, mut kr) = (input.clone(), input.clone());
        let (mut sl, mut sr) = (input.clone(), input.clone());
        keyed.process(&mut kl, &mut kr, Some((&silent, &silent)));
        self_keyed.process(&mut sl, &mut sr, None);
        let keyed_peak = kl.iter().skip(2048).fold(0.0f32, |m,x|m.max(x.abs()));
        let self_peak = sl.iter().skip(2048).fold(0.0f32, |m,x|m.max(x.abs()));
        assert!(self_peak < keyed_peak, "self-keyed={self_peak}, silent-sidechain={keyed_peak}");
    }
    #[test]
    fn exact_ceiling_flag_is_preserved() {
        let mut a = LynxLimiter::new(48000).unwrap();
        let mut b = LynxLimiter::new(48000).unwrap();
        let mut raw = DEFAULTS;
        raw[2] = 700;
        raw[3] = 0;
        a.set(&raw, 0);
        b.set(&raw, 1);
        let input = sine(48000, 24000, 1.7);
        let (mut al, mut ar) = (input.clone(), input.clone());
        let (mut bl, mut br) = (input.clone(), input.clone());
        a.process(&mut al, &mut ar, None);
        b.process(&mut bl, &mut br, None);
        let peak_a = al.iter().fold(0.0f32, |m,x|m.max(x.abs()));
        let peak_b = bl.iter().fold(0.0f32, |m,x|m.max(x.abs()));
        assert!(peak_b >= peak_a - 1.0e-4, "exact ceiling {peak_b}, safety ceiling {peak_a}");
    }
}

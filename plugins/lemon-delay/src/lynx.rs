use std::ffi::c_void;
use std::ptr::NonNull;

use os_engine::effects::delay::Settings;

unsafe extern "C" {
    fn skell_delay3_new(sample_rate: i32) -> *mut c_void;
    fn skell_delay3_delete(instance: *mut c_void);
    fn skell_delay3_set(instance: *mut c_void, raw: *const i32, bpm: f64);
    fn skell_delay3_reset(instance: *mut c_void);
    fn skell_delay3_process(instance: *mut c_void, left: *mut f32, right: *mut f32, frames: i32);
}

/// In-place adapter for the pinned LYNX SHARE Fruity Delay 3 engine.
/// Construction owns the engine's delay buffers; parameter and transport updates are allocation-free.
pub struct LynxDelay3 {
    instance: NonNull<c_void>,
}

unsafe impl Send for LynxDelay3 {}

impl LynxDelay3 {
    pub fn new(sample_rate: u32) -> Option<Self> {
        let rate = sample_rate.clamp(8000, 384000);
        let instance = NonNull::new(unsafe { skell_delay3_new(rate as i32) })?;
        Some(Self { instance })
    }

    #[inline]
    pub fn set(&mut self, settings: &Settings, bpm: f64) {
        unsafe { skell_delay3_set(self.instance.as_ptr(), settings.v.as_ptr(), bpm); }
    }

    #[inline]
    pub fn reset(&mut self) {
        unsafe { skell_delay3_reset(self.instance.as_ptr()); }
    }

    #[inline]
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let frames = left.len().min(right.len());
        if frames == 0 {
            return;
        }
        unsafe {
            skell_delay3_process(
                self.instance.as_ptr(),
                left.as_mut_ptr(),
                right.as_mut_ptr(),
                frames as i32,
            );
        }
    }
}

impl Drop for LynxDelay3 {
    fn drop(&mut self) {
        unsafe { skell_delay3_delete(self.instance.as_ptr()); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: u32, frames: usize) -> Vec<f32> {
        (0..frames)
            .map(|i| (std::f32::consts::TAU * 220.0 * i as f32 / rate as f32).sin() * 0.2)
            .collect()
    }

    fn render(rate: u32, block: usize) -> Vec<f32> {
        let mut engine = LynxDelay3::new(rate).expect("Delay 3 engine initializes off the audio thread");
        let settings = Settings { v: crate::params::START };
        engine.set(&settings, 120.0);
        let input = tone(rate, rate as usize);
        let mut left = input;
        let mut right = left.clone();
        for start in (0..left.len()).step_by(block) {
            let end = (start + block).min(left.len());
            engine.set(&settings, 120.0);
            engine.process(&mut left[start..end], &mut right[start..end]);
        }
        assert!(left.iter().chain(&right).all(|x| x.is_finite()));
        left
    }

    #[test]
    fn delay3_default_produces_finite_wet_output() {
        let mut engine = LynxDelay3::new(48000).unwrap();
        let settings = Settings { v: crate::params::START };
        engine.set(&settings, 120.0);
        let mut left = tone(48000, 48000);
        let input = left.clone();
        let mut right = left.clone();
        engine.process(&mut left, &mut right);
        assert!(left.iter().any(|x| x.abs() > 1.0e-6));
        assert!(left.iter().zip(&input).any(|(a, b)| (a - b).abs() > 1.0e-5));
        assert!(left.iter().chain(&right).all(|x| x.is_finite()));
    }

    #[test]
    fn variable_host_blocks_are_sample_aligned() {
        let reference = render(48000, 4096);
        for block in [1, 17, 64, 127, 512, 2048, 8192] {
            assert_eq!(reference, render(48000, block), "host block size {block}");
        }
    }

    #[test]
    fn reset_keeps_the_existing_state_and_clears_the_delay() {
        let mut engine = LynxDelay3::new(48000).unwrap();
        let settings = Settings { v: crate::params::START };
        engine.set(&settings, 120.0);
        let mut first = tone(48000, 24000);
        let mut right = first.clone();
        engine.process(&mut first, &mut right);
        engine.reset();
        let mut second = vec![0.0f32; 4096];
        let mut second_right = second.clone();
        engine.process(&mut second, &mut second_right);
        assert!(second.iter().chain(&second_right).all(|x| x.is_finite()));
    }

    #[test]
    fn high_sample_rates_remain_finite() {
        let audio = render(96000, 257);
        assert!(audio.iter().all(|x| x.is_finite()));
    }
}

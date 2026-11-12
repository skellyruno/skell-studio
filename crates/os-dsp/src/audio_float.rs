//! Scoped floating-point mode for audio processing. Subnormal tails are inaudible,
//! but can make recursive filters much slower. Restore the caller's mode on exit.

pub struct AudioFloat {
    saved: u64,
    // Floating-point control is local to the current thread.
    _thread: std::marker::PhantomData<*mut ()>,
}

impl AudioFloat {
    pub fn enter() -> Self {
        let saved = read();
        #[cfg(target_arch = "x86_64")]
        write(saved | (1 << 15) | (1 << 6)); // FTZ and DAZ
        #[cfg(target_arch = "aarch64")]
        write(saved | (1 << 24)); // FZ
        Self { saved, _thread: std::marker::PhantomData }
    }
}

impl Drop for AudioFloat {
    fn drop(&mut self) { write(self.saved); }
}

fn read() -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        let mut value = 0u32;
        unsafe { std::arch::asm!("stmxcsr [{p}]", p = in(reg) &mut value, options(nostack, preserves_flags)); }
        return u64::from(value);
    }
    #[cfg(target_arch = "aarch64")]
    {
        let value: u64;
        unsafe { std::arch::asm!("mrs {v}, fpcr", v = out(reg) value, options(nostack, preserves_flags)); }
        return value;
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    0
}

fn write(value: u64) {
    #[cfg(target_arch = "x86_64")]
    {
        let value = value as u32;
        unsafe { std::arch::asm!("ldmxcsr [{p}]", p = in(reg) &value, options(nostack, preserves_flags)); }
    }
    #[cfg(target_arch = "aarch64")]
    unsafe { std::arch::asm!("msr fpcr, {v}", v = in(reg) value, options(nostack, preserves_flags)); }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    let _ = value;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "manual DSP timing comparison"]
    fn channel_strip_tail_cost() {
        for protected in [false, true] {
            let original = AudioFloat { saved: read(), _thread: std::marker::PhantomData };
            #[cfg(target_arch = "x86_64")]
            write(original.saved & !((1 << 15) | (1 << 6)));
            #[cfg(target_arch = "aarch64")]
            write(original.saved & !(1 << 24));
            let _mode = protected.then(AudioFloat::enter);
            let mut strip = crate::channel_strip::ChannelStrip::new([0, 0, 0, 0, 32768, 0, 0], 48000);
            let mut block = vec![0.0; 512];
            block[0] = 0.5;
            strip.tame(&mut block);
            let start = std::time::Instant::now();
            let mut peak = 0.0f32;
            for _ in 0..1875 {
                block.fill(0.0);
                strip.tame(std::hint::black_box(&mut block));
                for &sample in &block { assert!(sample.is_finite()); peak = peak.max(sample.abs()); }
            }
            println!("Tame 10-second impulse tail, protected={protected}: {:?}, peak={peak}", start.elapsed());
        }
    }
    #[test]
    fn restores_mode_and_flushes_inaudible_tails() {
        let before = read();
        {
            let _mode = AudioFloat::enter();
            let active = read();
            { let _nested = AudioFloat::enter(); }
            assert_eq!(read(), active);
            #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
            assert_eq!(std::hint::black_box(f32::MIN_POSITIVE) * std::hint::black_box(0.25), 0.0);
            assert_eq!(std::hint::black_box(0.25f32) * 0.5, 0.125);
        }
        assert_eq!(read(), before);
    }
}

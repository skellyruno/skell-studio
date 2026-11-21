use std::ffi::c_void;
use std::ptr::NonNull;
use os_engine::effects::reverb::Settings;

unsafe extern "C" {
    fn skell_reeverb2_new(sample_rate: i32, max_block: i32) -> *mut c_void;
    fn skell_reeverb2_delete(instance: *mut c_void);
    fn skell_reeverb2_set(instance: *mut c_void, raw: *const i32, flags: i32, bpm: f64);
    fn skell_reeverb2_reset(instance: *mut c_void);
    fn skell_reeverb2_process(instance: *mut c_void, left: *mut f32, right: *mut f32, frames: i32);
}
/// Owner for the pinned LYNX SHARE Fruity Reeverb 2 engine. Buffers are reserved during initialize.
pub struct LynxReverb2 { instance: NonNull<c_void>, max_block: usize }
unsafe impl Send for LynxReverb2 {}
impl LynxReverb2 {
    pub fn new(sample_rate: u32, max_block: usize) -> Option<Self> {
        let rate = sample_rate.clamp(8000, 384000);
        let block = max_block.clamp(1, 65536);
        let instance = NonNull::new(unsafe { skell_reeverb2_new(rate as i32, block as i32) })?;
        Some(Self { instance, max_block: block })
    }
    #[inline]
    pub fn set(&mut self, settings: &Settings, bpm: f64) {
        let flags = i32::from(settings.tempo_sync) | (i32::from(settings.mid_side) << 1);
        unsafe { skell_reeverb2_set(self.instance.as_ptr(), settings.knobs.as_ptr(), flags, bpm); }
    }
    #[inline]
    pub fn reset(&mut self) { unsafe { skell_reeverb2_reset(self.instance.as_ptr()) }; }
    #[inline]
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let frames = left.len().min(right.len());
        let mut offset = 0;
        while offset < frames {
            let count = self.max_block.min(frames - offset);
            unsafe { skell_reeverb2_process(self.instance.as_ptr(), left[offset..].as_mut_ptr(), right[offset..].as_mut_ptr(), count as i32); }
            offset += count;
        }
    }
}
impl Drop for LynxReverb2 {
    fn drop(&mut self) { unsafe { skell_reeverb2_delete(self.instance.as_ptr()) }; }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn tone(rate:u32, frames:usize)->Vec<f32>{(0..frames).map(|i|(std::f32::consts::TAU*220.0*i as f32/rate as f32).sin()*0.2).collect()}
    fn render(rate:u32, block:usize, midside:bool)->Vec<f32>{
        let mut e=LynxReverb2::new(rate,8192).expect("engine initialize");
        let s=Settings{tempo_sync:false,mid_side:midside,..Settings::default()};
        e.set(&s,123.0);
        let mut l=tone(rate,rate as usize/2); let mut r=l.iter().map(|x|-*x).collect::<Vec<_>>();
        for start in (0..l.len()).step_by(block){let end=(start+block).min(l.len());e.process(&mut l[start..end],&mut r[start..end]);}
        assert!(l.iter().chain(&r).all(|x|x.is_finite())); l
    }
    #[test] fn output_finite_and_mid_side_changes_response(){
        let a=render(48000,127,false);let b=render(48000,127,true);
        assert!(a.iter().any(|x|x.abs()>1.0e-7));assert_ne!(a,b);
    }
    #[test] fn variable_blocks_are_sample_aligned(){
        let expected=render(48000,4096,false);
        for n in [1,17,64,127,512,2048,8192]{assert_eq!(expected,render(48000,n,false),"block {n}");}
    }
    #[test] fn sample_rate_96khz_remains_finite(){let x=render(96000,257,false);assert!(x.iter().all(|v|v.is_finite()));}
}

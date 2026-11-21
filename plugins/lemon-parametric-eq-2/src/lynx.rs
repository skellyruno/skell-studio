// Copyright (C) 2026 Skelly Audio
// SPDX-License-Identifier: AGPL-3.0-only
//! Pinned LYNX SHARE Fruity Parametric EQ 2 engine adapter.
//! Existing ML EQ host identity, automation IDs and saved-state image remain authoritative.
use std::ffi::c_void;
use std::ptr::NonNull;
use os_engine::effects::eq::{Options, PARAMS};

unsafe extern "C" {
    fn skell_lynx_eq_new(rate:i32,raw:*const i32,hq:i32,flags:i32,quality:i32,fade:i32,mask:i32)->*mut c_void;
    fn skell_lynx_eq_delete(instance:*mut c_void);
    fn skell_lynx_eq_set(instance:*mut c_void,raw:*const i32,hq:i32,flags:i32,quality:i32,fade:i32,mask:i32);
    fn skell_lynx_eq_reset(instance:*mut c_void);
    fn skell_lynx_eq_latency(instance:*mut c_void)->i32;
    fn skell_lynx_eq_process(instance:*mut c_void,left:*mut f32,right:*mut f32,frames:i32);
    fn skell_lynx_eq_response(instance:*mut c_void,hz:f64)->f64;
    fn skell_lynx_eq_stages_match(seed:u32)->bool;
}
fn options_values(o:Options)->(i32,i32,i32,i32,i32){
    (o.hq,i32::from(o.flags),o.lin_size,o.lin_fade,i32::from(o.mask))
}
pub struct LynxEq{ptr:NonNull<c_void>,raw:[i32;PARAMS],options:(i32,i32,i32,i32,i32)}
unsafe impl Send for LynxEq{}
impl LynxEq{
    pub fn new(rate:u32,raw:&[i32;PARAMS],options:Options)->Option<Self>{
        let o=options_values(options);
        let ptr=NonNull::new(unsafe{skell_lynx_eq_new(rate.clamp(8000,384000) as i32,raw.as_ptr(),o.0,o.1,o.2,o.3,o.4)})?;
        Some(Self{ptr,raw:*raw,options:o})
    }
    #[inline] pub fn set(&mut self,raw:&[i32;PARAMS],options:Options){
        let o=options_values(options);
        if self.raw!=*raw||self.options!=o{
            unsafe{skell_lynx_eq_set(self.ptr.as_ptr(),raw.as_ptr(),o.0,o.1,o.2,o.3,o.4);}
            self.raw=*raw;self.options=o;
        }
    }
    #[inline] pub fn reset(&mut self){unsafe{skell_lynx_eq_reset(self.ptr.as_ptr())}}
    #[inline] pub fn latency(&self)->u32{unsafe{skell_lynx_eq_latency(self.ptr.as_ptr()).max(0) as u32}}
    #[inline] pub fn process(&mut self,l:&mut[f32],r:&mut[f32]){
        let n=l.len().min(r.len());
        if n>0{unsafe{skell_lynx_eq_process(self.ptr.as_ptr(),l.as_mut_ptr(),r.as_mut_ptr(),n as i32);}}
    }
    #[cfg(test)] pub fn response_db(&self,hz:f64)->f64{unsafe{skell_lynx_eq_response(self.ptr.as_ptr(),hz)}}
}
impl Drop for LynxEq{fn drop(&mut self){unsafe{skell_lynx_eq_delete(self.ptr.as_ptr())}}}

#[cfg(test)]
mod tests{
    use super::*;
    use os_engine::effects::eq::{self,Settings};
    fn tone(rate:u32,hz:f32,n:usize)->Vec<f32>{
        (0..n).map(|i|0.1*(std::f32::consts::TAU*hz*i as f32/rate as f32).sin()).collect()
    }
    fn render(block:usize)->Vec<f32>{
        let rate=48000;let mut raw=Settings::default().params();
        raw[3]=600;raw[10]=(eq::freq_pos(1000.)*65536.).round() as i32;raw[17]=65536;raw[24]=eq::PEAKING;
        let mut fx=LynxEq::new(rate,&raw,Options::default()).expect("LYNX EQ initialization");
        let input=tone(rate,1000.,rate as usize);let mut l=input.clone();let mut r=input;
        for at in (0..l.len()).step_by(block){let end=(at+block).min(l.len());fx.process(&mut l[at..end],&mut r[at..end]);}
        assert!(l.iter().chain(&r).all(|v|v.is_finite()));l
    }
    #[test]fn output_is_finite_and_independent_of_host_block_size(){
        let reference=render(512);
        for block in [1,17,64,127,2048,8192]{
            let candidate=render(block);
            let max_delta=reference.iter().zip(&candidate)
                .map(|(a,b)|(a-b).abs()).fold(0.0f32,f32::max);
            // LYNX flushes sub-2^-24 filter histories at each process block, so callback boundaries
            // can alter a few samples near the state floor. Bound and report the worst-case error.
            assert!(max_delta<1.0e-5,"block {block}, max abs delta {max_delta:e}");
        }
    }
    #[test]fn scalar_stage_cascade_matches_reference(){assert!(unsafe{skell_lynx_eq_stages_match(0x51E11)});}
    #[test]fn hq_external_and_linear_latency_are_reported(){
        let rate=48000;let raw=Settings::default().params();
        let hq=LynxEq::new(rate,&raw,Options{hq:1,flags:2,..Options::default()}).unwrap();assert_eq!(hq.latency(),44);
        let lin=LynxEq::new(rate,&raw,Options{flags:1,lin_size:0,..Options::default()}).unwrap();assert_eq!(lin.latency(),384);
    }
    #[test]fn peaking_band_boosts_its_centre(){
        let rate=48000;let mut raw=Settings::default().params();raw[3]=600;
        raw[10]=(eq::freq_pos(1000.)*65536.).round() as i32;raw[17]=65536;raw[24]=eq::PEAKING;
        let mut fx=LynxEq::new(rate,&raw,Options::default()).unwrap();
        let input=tone(rate,1000.,rate as usize);let mut l=input.clone();let mut r=input.clone();
        fx.process(&mut l,&mut r);
        let rms=|v:&[f32]|(v.iter().skip(8192).map(|x|x*x).sum::<f32>()/(v.len()-8192) as f32).sqrt();
        assert!(rms(&l)>rms(&input)*1.35);assert!(fx.response_db(1000.)>5.);
    }
}

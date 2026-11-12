// SPDX-License-Identifier: GPL-3.0-or-later
//! Lemon Tape. MIT Airwindows ToTape6 DSP; Lemon parameter/state/UI adapter.
use super::*;
use crate::params::leading_number;
use std::ffi::c_void;
unsafe extern "C" {
    fn lemon_tape_create(rate:f64)->*mut c_void;
    fn lemon_tape_destroy(p:*mut c_void);
    fn lemon_tape_reset(p:*mut c_void,rate:f64);
    fn lemon_tape_set(p:*mut c_void,v:*const f32);
    fn lemon_tape_process(p:*mut c_void,l:*mut f32,r:*mut f32,n:i32);
}
struct Tape {p:std::ptr::NonNull<c_void>,rate:u32,target:[f32;6],current:[f32;6]}
// The native instance belongs to exactly one Sound and has no shared/static state.
unsafe impl Send for Tape {}
impl Drop for Tape {fn drop(&mut self){unsafe{lemon_tape_destroy(self.p.as_ptr())}}}
fn values(v:&[i32])->[f32;6]{std::array::from_fn(|i|v.get(i).copied().unwrap_or(SETTINGS[i].start).clamp(0,1000) as f32/1000.)}
impl Sound for Tape {
    fn set(&mut self,v:&[i32]){self.target=values(v);}
    fn set_sample_rate(&mut self,rate:u32){self.rate=rate.clamp(22050,384000);self.reset();}
    fn reset(&mut self){self.current=self.target;unsafe{lemon_tape_reset(self.p.as_ptr(),self.rate as f64);lemon_tape_set(self.p.as_ptr(),self.current.as_ptr());}}
    fn process(&mut self,l:&mut[f32],r:&mut[f32]){
        // Small, allocation-free control ramps (~10 ms) limit abrupt automation changes.
        for (l,r) in l.chunks_mut(32).zip(r.chunks_mut(32)){
            let n=l.len().min(r.len());let retain=(-(n as f32)/(self.rate as f32*0.01)).exp();
            for k in 0..6{self.current[k]=self.target[k]+(self.current[k]-self.target[k])*retain;if (self.current[k]-self.target[k]).abs()<1e-6{self.current[k]=self.target[k];}}
            if self.current[5]==0. {continue}
            // Never let a corrupt upstream sample poison the recursive filter states.
            for x in l.iter_mut().chain(r.iter_mut()){if !x.is_finite(){*x=0.;}else{*x=(*x).clamp(-64.,64.);}}
            unsafe{lemon_tape_set(self.p.as_ptr(),self.current.as_ptr());lemon_tape_process(self.p.as_ptr(),l.as_mut_ptr(),r.as_mut_ptr(),n as i32);}
            if l.iter().chain(r.iter()).any(|x|!x.is_finite()){l.fill(0.);r.fill(0.);self.reset();}
        }
    }
}
fn sound(rate:u32,v:&[i32])->Box<dyn Sound>{
    let rate=rate.clamp(22050,384000);let target=values(v);
    let p=std::ptr::NonNull::new(unsafe{lemon_tape_create(rate as f64)}).expect("Tape allocation");
    let mut s=Tape{p,rate,target,current:target};s.reset();Box::new(s)
}
const SETTINGS:[Setting;6]=[
    Setting{id:"input",name:"Input",range:(0,1000),start:500,magnet:Some(500)},
    Setting{id:"soften",name:"Soften",range:(0,1000),start:350,magnet:None},
    Setting{id:"head_bump",name:"Head bump",range:(0,1000),start:250,magnet:None},
    Setting{id:"flutter",name:"Flutter",range:(0,1000),start:100,magnet:None},
    Setting{id:"output",name:"Output",range:(0,1000),start:500,magnet:Some(500)},
    Setting{id:"mix",name:"Mix",range:(0,1000),start:1000,magnet:None},
];
const CONTROLS:[Control;6]=[
    Control::Knob{id:"input",cx:60.,cy:180.,r:24.,caption:"INPUT",bipolar:true},
    Control::Knob{id:"soften",cx:164.,cy:180.,r:24.,caption:"SOFTEN",bipolar:false},
    Control::Knob{id:"head_bump",cx:268.,cy:180.,r:24.,caption:"HEAD BUMP",bipolar:false},
    Control::Knob{id:"flutter",cx:372.,cy:180.,r:24.,caption:"FLUTTER",bipolar:false},
    Control::Knob{id:"output",cx:476.,cy:180.,r:24.,caption:"OUTPUT",bipolar:true},
    Control::Knob{id:"mix",cx:580.,cy:180.,r:24.,caption:"MIX",bipolar:false},
];
fn text(k:usize,v:i32)->String{if k==0||k==4{format!("{:+.3} dB",(v as f64/1000.-0.5)*24.)}else{format!("{:.1}%",v as f64/10.)}}
fn parse(k:usize,s:&str)->Option<i32>{let n=leading_number(s)?;if !n.is_finite(){return None}Some((if k==0||k==4{(n/24.+0.5)*1000.}else{n*10.}).round().clamp(0.,1000.) as i32)}
fn reels(c:&mut Canvas,r:Rect,v:&[i32]){
    let ink=Color::rgb(0x16191b);let rim=Color::rgb(0x8f9798);let gold=Color::rgb(0xffd94a);
    c.fill_rrect(r,6.,ink);
    let y=r.cy();let (a,b)=(r.x+100.,r.right()-100.);
    c.line(a,y+25.,b,y+25.,3.,Color::rgb(0x997b4b));
    for x in [a,b]{c.fill_circle(x,y,34.,Color::rgb(0x303638));c.stroke_circle(x,y,33.,2.,rim);c.stroke_circle(x,y,26.,1.,gold);
        for k in 0..3{let angle=k as f32*std::f32::consts::TAU/3.+v[3] as f32/1000.;c.fill_circle(x+angle.cos()*17.,y+angle.sin()*17.,8.,ink);}
        c.fill_circle(x,y,6.,rim);c.fill_circle(x,y,2.,ink);
    }
    c.text_center(os_ui::Font::new(16),Rect::new(r.cx()-100.,y-23.,200.,22.),gold,"TAPE COLOR");
    c.text_center(os_ui::Font::new(10),Rect::new(r.cx()-105.,y+1.,210.,18.),rim,"AIRWINDOWS TOTAPE6 ENGINE");
}
pub const SPEC:&Spec=&Spec{name:"ML Tape",word:"TAPE",about:"Tape color powered by Airwindows ToTape6 — Chris Johnson, MIT licence",settings:&SETTINGS,version:Some(1),any_version:false,fields:&[],tail:&[],size:(640,270),cards:&[Card{x0:8.,x1:632.,y0:22.,y1:128.,heading:"TAPE MACHINE"},Card{x0:8.,x1:632.,y0:148.,y1:230.,heading:"CHARACTER & LEVEL"}],controls:&CONTROLS,display:Some(Display{r:Rect::new(16.,30.,608.,90.),draw:reels}),mark:Mark{x:622.,y:251.,anchor:1.,stacked:false},hint:Some(Rect::new(12.,242.,470.,20.)),labels:&[],text,parse,sound,rings:true,blank:None,locate:None};
pub struct TapeKind;
impl Kind for TapeKind{
    const SPEC:&'static Spec=SPEC;
    const CLAP_ID:&'static str="org.openstudio.lemon-tape";
    const CLAP_DESCRIPTION:&'static str="Tape saturation, softness, head bump and flutter; Airwindows ToTape6";
    const CLAP_FEATURES:&'static [ClapFeature]=&[ClapFeature::AudioEffect,ClapFeature::Distortion,ClapFeature::Stereo];
    const VST3_CLASS_ID:[u8;16]=*b"LemonStudioTape1";
    const VST3_SUBCATEGORIES:&'static [Vst3SubCategory]=&[Vst3SubCategory::Fx,Vst3SubCategory::Distortion];
}
pub type LemonTape=SmallFx<TapeKind>;

#[cfg(test)]mod tests{
    use super::*;
use crate::params::leading_number;
    #[test]fn tape_rates_extremes_reset_and_dry(){
        for rate in [8000,22050,44100,48000,96000,192000,384000]{
            let input:Vec<f32>=(0..8192).map(|i|(i as f32*0.13).sin()*0.7).collect();
            for v in [SPEC.start(),vec![1000;6],vec![0,0,0,0,500,1000]]{
                let mut s=sound(rate,&v);let(mut l,mut r)=(input.clone(),input.clone());s.process(&mut l,&mut r);assert!(l.iter().chain(&r).all(|x|x.is_finite()&&x.abs()<32.));
                s.reset();let(mut again,mut ar)=(input.clone(),input.clone());s.process(&mut again,&mut ar);assert!(l==again&&r==ar,"reset reproducibility at {rate}");
            }
            let mut v=SPEC.start();v[5]=0;let mut s=sound(rate,&v);let(mut l,mut r)=(input.clone(),input.clone());s.process(&mut l,&mut r);assert!(l==input&&r==input);
        }
        let mut s=sound(48000,&SPEC.start());let mut l=[f32::NAN;64];let mut r=[f32::INFINITY;64];s.process(&mut l,&mut r);assert!(l.iter().chain(&r).all(|x|x.is_finite()));
    }
    #[test]fn tape_panel_has_clear_controls(){
        let mut logic=editor::Logic::new(SPEC,Arc::new(params::FxParams::new(SPEC)),Arc::new(Shared::default()),None);
        let mut c=Canvas::new(640,270,1.).unwrap();logic.paint(&mut c,&os_plugkit::palette());
        assert_eq!(logic.hits.iter().filter(|h|matches!(h.t,view::Target::Knob(_))).count(),6);
        if let Some(dir)=std::env::var_os("OS_SHOT_DIR"){c.pixmap.save_png(std::path::PathBuf::from(dir).join("lemon-tape.png")).unwrap();}
    }
    #[test]fn tape_parameters_roundtrip_and_color(){
        for k in 0..6{for v in 0..=1000{assert_eq!(parse(k,&text(k,v)),Some(v));}}
        let mut s=sound(48000,&SPEC.start());let input:Vec<f32>=(0..48000).map(|i|(i as f32*0.12).sin()*0.8).collect();let(mut l,mut r)=(input.clone(),input.clone());s.process(&mut l,&mut r);
        assert!(l.iter().zip(&input).any(|(a,b)|(a-b).abs()>0.01));
        let p=params::FxParams::new(SPEC);let raw=params::default_state(SPEC);p.set_raw(raw.clone());assert_eq!(*p.raw.read().unwrap(),raw);
    }
}

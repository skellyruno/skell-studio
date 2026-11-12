// SPDX-License-Identifier: GPL-3.0-or-later
//! Presentation-only boundary pressure; content and zoom hit geometry stay clamped.
use crate::app::Scroll;
use std::time::{Duration, Instant};
pub const ADAPTIVE: &str = "visible_adaptive-horizontal-scrollbars";
pub fn adaptive(t:Scroll)->bool {matches!(t,Scroll::PlaylistX|Scroll::PianoX) && crate::workspace_visibility::shown(ADAPTIVE)}
/// A viewport clipped against the content range, with an accessible minimum grab width.
pub fn horizontal(len:f32,pos:f32,view:f32,total:f32)->(f32,f32,f32) {
    let len=len.max(1.);let total=total.max(view).max(1.);
    let minimum=48.0_f32.min(len);
    let max=total*(1.-minimum/len);
    let at=pos.clamp(0.,max)/total*len;
    let width=(view/total*len).max(minimum).min(len-at);
    (at,width,max)
}
pub const VISIBILITY: &str = "visible_elastic-scrollbars";
pub fn axis(t: Scroll) -> Option<usize> { match t { Scroll::PlaylistX=>Some(0), Scroll::PlaylistY=>Some(1), Scroll::PianoX=>Some(2), Scroll::PianoY=>Some(3), _=>None } }
#[derive(Clone, Copy, Default)]
struct Spring { pressure: f32, shown: f32, last: Option<Instant> }
#[derive(Default)]
pub struct Elastic { springs: [Spring;4], tick: Option<Instant> }
impl Elastic {
    pub fn input(&mut self,t:Scroll,excess:f32,accumulate:bool,now:Instant) {
        let Some(i)=axis(t) else{return};
        let s=&mut self.springs[i];
        if excess==0. {s.pressure=0.;s.last=None;return;}
        let base=if accumulate && s.pressure.signum()==excess.signum() {s.pressure}else{0.};
        s.pressure=(base+excess).clamp(-600.,600.);s.last=Some(now);
        self.tick.get_or_insert(now);
    }
    pub fn active(&self)->bool {self.springs.iter().any(|s|s.pressure!=0. || s.shown!=0.)}
    pub fn active_axes(&self)->[bool;4] {self.springs.map(|s|s.pressure!=0. || s.shown!=0.)}
    pub fn values(&self)->[f32;4] {self.springs.map(|s|s.shown)}
    pub fn advance(&mut self,now:Instant,held:[bool;4])->bool {
        let active=self.active();
        let dt=self.tick.map_or(0.,|t|now.saturating_duration_since(t).as_secs_f32()).min(0.1);
        self.tick=Some(now);
        for (i,s) in self.springs.iter_mut().enumerate() {
            if !held[i] && s.last.is_none_or(|t|now.saturating_duration_since(t)>=Duration::from_millis(100)) {s.pressure=0.;}
            let target=s.pressure.signum()*(1.-(-s.pressure.abs()/100.).exp())*0.78;
            let rate=if target==0. {18.} else {30.};
            s.shown+=(target-s.shown)*(1.-(-rate*dt).exp());
            if target==0. && s.shown.abs()<0.001 {s.shown=0.;}
        }
        active
    }
    pub fn clear(&mut self) {*self=Self::default();}
}
pub fn thumb(values:&[f32;4],t:Scroll,at:f32,len:f32)->(f32,f32) {
    let v=axis(t).map_or(0.,|i|values[i]);
    let size=(len*(1.-v.abs())).max(6.0_f32.min(len));
    (at+if v>0. {len-size}else{0.},size)
}
#[cfg(test)] mod tests {
 use super::*;
 #[test] fn adaptive_horizontal_matches_full_shrink_replenish_cycle() {
  let (at,width,max)=horizontal(800.,0.,1000.,10.);assert_eq!((at,width),(0.,800.));
  let (at,width,_)=horizontal(800.,500.,1000.,10.);assert_eq!((at,width),(400.,400.));
  let (at,width,_)=horizontal(800.,max+999.,1000.,10.);assert_eq!(width,48.);assert_eq!(at+width,800.);
  let (new_at,new_width,new_max)=horizontal(800.,max,1000.,2000.);
  assert!(new_width>width && new_at>0. && new_max>max);
  let (end,w,_)=horizontal(800.,new_max,1000.,2000.);assert_eq!((end+w,w),(800.,48.));
 }
 #[test] fn elastic_boundary_pressure_and_release() {
  let mut e=Elastic::default();let now=Instant::now();
  e.input(Scroll::PlaylistY,-40.,true,now);e.advance(now+Duration::from_millis(32),[false;4]);
  let a=thumb(&e.values(),Scroll::PlaylistY,0.,100.);assert_eq!(a.0,0.);assert!(a.1<100.);
  e.input(Scroll::PlaylistY,-80.,true,now+Duration::from_millis(32));e.advance(now+Duration::from_millis(64),[false;4]);
  assert!(thumb(&e.values(),Scroll::PlaylistY,0.,100.).1<a.1);
  e.input(Scroll::PianoY,90.,false,now);e.advance(now+Duration::from_millis(80),[false,false,false,true]);
  let (at,len)=thumb(&e.values(),Scroll::PianoY,400.,100.);assert!((at+len-500.).abs()<0.001);
  for n in 1..100 {e.advance(now+Duration::from_millis(80+n*16),[false;4]);}
  assert!(!e.active());assert_eq!(e.values(),[0.;4]);
  e.input(Scroll::MixerX,100.,true,now);assert!(!e.active());
 }
}

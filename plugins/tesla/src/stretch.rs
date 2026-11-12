// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline Signalsmith edits. Rendering owns a snapshot, never the audio thread's document lock.
use crate::sample::Sample;
pub const VISIBILITY: &str = "visible_tesla-stretch";

pub fn visible() -> bool {
    use std::{cell::Cell, time::{Duration,Instant}};
    thread_local! {static CACHE:Cell<Option<(Instant,bool)>>=const{Cell::new(None)};}
    CACHE.with(|cache| {
        if let Some((at,value))=cache.get(){if at.elapsed()<Duration::from_millis(300){return value}}
        let value=!os_paths::user_file("settings.txt").and_then(|p|std::fs::read_to_string(p).ok()).is_some_and(|s|s.lines().rev().filter_map(|l|l.split_once('=')).find(|(k,_)|k.trim()==VISIBILITY).is_some_and(|(_,v)|v.trim()=="0"));
        cache.set(Some((Instant::now(),value)));value
    })
}

#[derive(Clone,Debug,PartialEq)]
pub struct Panel {pub percent:f64,pub semitones:f64,pub busy:bool,pub message:String}
impl Default for Panel {fn default()->Self{Self{percent:100.,semitones:0.,busy:false,message:"Selection, or whole sample. Original file stays unchanged.".into()}}}

pub fn render(source:&Sample,range:(usize,usize),percent:f64,semitones:f64)->Result<Sample,String>{
    if !percent.is_finite()||!(25.0..=400.0).contains(&percent)||!semitones.is_finite()||!(-24.0..=24.0).contains(&semitones){return Err("Duration: 25–400%. Pitch: -24 to +24 semitones.".into())}
    let (a,b)=range;
    if a>b||b>=source.frames()||source.rate==0{return Err("Select some audio first.".into())}
    let n=b-a+1;let frames=(n as f64*percent/100.).round().max(1.) as usize;
    // Bound scratch allocations; no partial edit can reach the document.
    if source.frames().max(source.frames()-n+frames)>source.rate as usize*60*20{return Err("This tool supports samples up to 20 minutes.".into())}
    if percent==100.&&semitones==0.{return Ok(source.clone())}
    let left=&source.ch[0][a..=b];let right=&source.ch[source.channels()-1][a..=b];
    let (mut l,mut r)=os_dsp::stretch::whole(left,right,source.rate,frames,2f64.powf(semitones/12.));
    if l.iter().chain(&r).any(|x|!x.is_finite()){return Err("Processing produced invalid audio. Original preserved.".into())}
    // Short transitions inside the processed range avoid clicks against untouched neighbours.
    let fade=(source.rate as usize/250).min(frames/2).min(n/2);
    for (output,input) in [(&mut l,left),(&mut r,right)]{for j in 0..fade{
        let wet=(1.-(std::f32::consts::PI*j as f32/fade.max(1) as f32).cos())*0.5;
        if a>0{output[j]=output[j]*wet+input[j]*(1.-wet);}
        if b+1<source.frames(){output[frames-1-j]=output[frames-1-j]*wet+input[n-1-j]*(1.-wet);}
    }}
    let mut out=source.clone();
    for (c,new) in out.ch.iter_mut().zip([l,r]) {c.splice(a..=b,new);}
    // Map boundaries, including cues crossing either edge of the selection.
    let map=|p:usize|->usize{if p<a{p}else if p>=b+1{p-n+frames}else{a+((p-a) as f64*frames as f64/n as f64).round() as usize}};
    for cue in &mut out.cues{let end=cue.len.map(|len|map(cue.pos+len));cue.pos=map(cue.pos).min(out.ch[0].len()-1);cue.len=end.map(|e|e.saturating_sub(cue.pos).max(1));}
    out.looped=out.looped.map(|(x,y)|{let x=map(x).min(out.ch[0].len()-1);(x,map(y+1).saturating_sub(1).max(x).min(out.ch[0].len()-1))});
    if a==0&&b+1==source.frames()&&out.tempo>0.{out.tempo/=(percent/100.) as f32;}
    out.quantize();Ok(out)
}

#[cfg(test)]mod tests{
    use super::*;
    use crate::{doc::Doc,sample::{Format,Cue}};
    #[test]fn stretch_audio_metadata_and_undo(){
        let wave:Vec<_>=(0..48000).map(|i|(i as f32*440.*std::f32::consts::TAU/48000.).sin()*0.2).collect();
        let mut sample=Sample::from_channels(48000,vec![wave.clone(),wave],Format::Float32);
        sample.cues.push(Cue{id:1,name:"inside".into(),pos:24000,len:Some(12000)});sample.looped=Some((12000,35999));
        let changed=render(&sample,(12000,35999),200.,12.).unwrap();
        assert_eq!(changed.frames(),72000);assert_eq!(&changed.ch[0][..12000],&sample.ch[0][..12000]);assert_eq!(&changed.ch[0][60000..],&sample.ch[0][36000..]);
        assert_eq!(changed.cues[0].pos,36000);assert_eq!(changed.looped,Some((12000,59999)));
        let stereo_error=changed.ch[0].iter().zip(&changed.ch[1]).map(|(l,r)|(l-r).abs()).fold(0f32,f32::max);
        assert!(stereo_error<0.001,"stereo error: {stereo_error}");
        let mid=&changed.ch[0][24000..48000];let crossings=mid.windows(2).filter(|w|w[0]<=0.&&w[1]>0.).count();assert!((430..450).contains(&crossings),"octave shifted tone: {crossings}");
        let mut d=Doc::default();d.sample=std::sync::Arc::new(sample.clone());d.sel=Some((12000,35999));
        d.apply_stretch(changed.clone(),(12000,59999));assert_eq!(d.history().len(),1);d.undo();assert_eq!(*d.sample,sample);d.redo();assert_eq!(*d.sample,changed);
        assert!(render(&sample,(0,47999),f64::NAN,0.).is_err());assert!(render(&sample,(0,47999),100.,f64::INFINITY).is_err());
    }
}

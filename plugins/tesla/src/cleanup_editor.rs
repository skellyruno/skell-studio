// SPDX-License-Identifier: GPL-3.0-or-later
impl Drop for Logic {fn drop(&mut self){self.shared.preview(None);if self.view.cleanup.is_some(){self.stop();}}}
impl Logic {
 fn cleanup_return_current(&mut self){
  if self.shared.armed.load(Ordering::Acquire){self.hint="Stop recording before applying to track.".into();return;}
  self.stop();self.shared.preview(None);
  let sample=self.doc(|d|d.sample.clone());let link=self.shared.cleanup_link.lock().unwrap().clone();
  if let Some(link)=link.filter(|s|s.submit(sample)){
   self.view.cleanup=Some(crate::cleanup::Panel{linked:true,awaiting:true,message:link.message(),..Default::default()});
  }else{self.hint="Return link expired. Reopen Cleanup from the Playlist clip.".into();}
 }
 fn cleanup_open(&mut self){
  if self.stretch_job.is_some()||self.shared.armed.load(Ordering::Acquire)||self.doc(|d|d.frames()==0){self.hint="Load audio and stop recording first.".into();return;}
  self.stop();self.drag=None;self.cleanup_ready=None;
  let linked=self.shared.cleanup_link.lock().unwrap().as_ref().is_some_and(|s|s.active.load(Ordering::Acquire));
  let mut panel=crate::cleanup::Panel{linked,..Default::default()};
  if self.cleanup_profile.is_some(){panel.message="Noise profile learned. Preview, then apply to the selection or whole sample.".into();}
  self.view.cleanup=Some(panel);
 }
 fn cleanup_close(&mut self){self.cleanup_job=None;self.shared.preview(None);self.stop();self.cleanup_ready=None;self.view.cleanup=None;}
 fn cleanup_click(&mut self,id:u8)->Vec<Act>{
  if id==9 {self.cleanup_close();return vec![Act::Repaint];}
  let Some(panel)=self.view.cleanup.as_ref()else{return vec![]};if panel.busy||panel.awaiting{return vec![];}
  if matches!(id,0|1|2|3|11){
   self.stop();self.shared.preview(None);self.cleanup_ready=None;
   let p=self.view.cleanup.as_mut().unwrap();p.ready=false;
   match id {0=>p.reduction=(p.reduction-3.).max(0.),1=>p.reduction=(p.reduction+3.).min(36.),2=>p.sensitivity=(p.sensitivity-0.25).max(0.5),3=>p.sensitivity=(p.sensitivity+0.25).min(4.),11=>{self.cleanup_profile=None;p.message="Automatic estimate selected. Use a noise-only profile for best results.".into();},_=>{}}
   return vec![Act::Repaint];
  }
  if id==6 { // Original, never committed to the document.
   self.stop();self.shared.preview(Some(self.doc(|d|d.sample.clone())));if let Some((a,b))=self.doc(|d|d.range()){self.shared.send(Play::Play{from:a,to:b,at:a as f64});self.shared.playing.store(true,Ordering::Release);}return vec![Act::Repaint];
  }
  if id==10 {self.stop();return vec![Act::Repaint];}
  if id==8 {self.cleanup_apply(false);return vec![Act::Repaint];}
  if id==12 {self.cleanup_apply(true);return vec![Act::Repaint];}
  if id==4||id==7 {
   if id==7&&self.cleanup_ready.is_some(){self.cleanup_audition();return vec![Act::Repaint];}
   let(source,version,range,selection)=self.doc(|d|(d.sample.clone(),d.version,d.range(),d.sel));let Some(range)=range else{return vec![]};
   if id==4&&selection.is_none(){self.view.cleanup.as_mut().unwrap().message="Close this panel and select noise only, then choose Learn selected noise.".into();return vec![Act::Repaint];}
   let(reduction,sensitivity)=(panel.reduction,panel.sensitivity);let profile=self.cleanup_profile.clone();let progress=Arc::new(crate::cleanup::Progress::default());
   let task_progress=progress.clone();let input=source.clone();let(tx,rx)=std::sync::mpsc::channel();
   self.stop();self.shared.preview(None);
   std::thread::spawn(move||{let result=std::panic::catch_unwind(||if id==4{crate::cleanup::learn(&input,range,false,&task_progress).map(crate::cleanup::Outcome::Profile)}else{crate::cleanup::render(&input,range,profile.as_ref(),reduction,sensitivity,&task_progress).map(crate::cleanup::Outcome::Audio)}).unwrap_or_else(|_|Err("Cleanup failed. Original preserved.".into()));let _=tx.send(result);});
   self.cleanup_job=Some(crate::cleanup::Job{rx,source,version,range,progress});let p=self.view.cleanup.as_mut().unwrap();p.busy=true;p.message=if id==4{"Learning noise…"}else{"Rendering preview… Cancel preserves the original."}.into();
  }
  vec![Act::Repaint]
 }
 fn cleanup_audition(&mut self){
  let Some((source,version,output,range))=self.cleanup_ready.clone() else{return};
  if !self.doc(|d|d.version==version&&Arc::ptr_eq(&source,&d.sample)){self.cleanup_ready=None;if let Some(p)=self.view.cleanup.as_mut(){p.ready=false;p.message="Audio changed. Create a new preview.".into();}return;}
  self.stop();self.shared.preview(Some(output.clone()));self.shared.send(Play::Play{from:range.0,to:range.1,at:range.0 as f64});self.shared.playing.store(true,Ordering::Release);
 }
 fn cleanup_apply(&mut self,to_track:bool){
  let Some((source,version,output,_))=self.cleanup_ready.clone()else{if let Some(p)=self.view.cleanup.as_mut(){p.message="Preview cleaned audio first.".into();}return;};
  if !self.doc(|d|d.version==version&&Arc::ptr_eq(&source,&d.sample))||self.shared.armed.load(Ordering::Acquire){self.cleanup_ready=None;if let Some(p)=self.view.cleanup.as_mut(){p.ready=false;p.message="Audio changed. Create a new preview.".into();}return;}
  self.stop();self.shared.preview(None);
  if to_track {
   let link=self.shared.cleanup_link.lock().unwrap().clone();
   if let Some(link)=link.filter(|s|s.submit(output.clone())){let p=self.view.cleanup.as_mut().unwrap();p.awaiting=true;p.message=link.message();}
   else{self.view.cleanup.as_mut().unwrap().message="Return link expired. Reopen Cleanup from the source clip.".into();}
  } else {
   self.doc(|d|d.apply_cleanup((*output).clone()));self.cleanup_close();self.hint="Cleanup applied. Undo restores the original.".into();
  }
 }
 fn poll_cleanup(&mut self){
  let mut committed=false;
  if let Some(p)=self.view.cleanup.as_mut(){if p.awaiting {
   let link=self.shared.cleanup_link.lock().unwrap().clone();
   if let Some(link)=link {p.message=link.message();if !link.pending.load(Ordering::Acquire){p.awaiting=false;committed=link.applied.load(Ordering::Acquire);if !link.active.load(Ordering::Acquire){p.linked=false;p.ready=false;}}}
  }}
  if committed {if let Some((_,_,output,_))=self.cleanup_ready.take(){self.doc(|d|{if !Arc::ptr_eq(&d.sample,&output){d.apply_cleanup((*output).clone());}});}}
  let Some(job)=self.cleanup_job.as_ref()else{return};if let Some(p)=self.view.cleanup.as_mut(){p.progress=job.progress.percent.load(Ordering::Relaxed);}
  let result=match job.rx.try_recv(){Ok(r)=>r,Err(std::sync::mpsc::TryRecvError::Empty)=>return,Err(_)=>Err("Cleanup worker stopped.".into())};
  let job=self.cleanup_job.take().unwrap();if self.view.cleanup.is_none(){return;}
  let result=result.and_then(|r|if self.doc(|d|d.version==job.version&&Arc::ptr_eq(&d.sample,&job.source))&&!self.shared.armed.load(Ordering::Acquire){Ok(r)}else{Err("Audio changed. Please preview again.".into())});
  self.view.cleanup.as_mut().unwrap().busy=false;
  match result {
   Ok(crate::cleanup::Outcome::Profile(profile))=>{self.cleanup_profile=Some(profile);self.doc(|d|d.sel=None);let p=self.view.cleanup.as_mut().unwrap();p.message="Noise learned. The whole sample is now selected for cleanup.".into();p.ready=false;self.cleanup_ready=None;},
   Ok(crate::cleanup::Outcome::Audio(sample))=>{self.cleanup_ready=Some((job.source.clone(),job.version,Arc::new(sample),job.range));let p=self.view.cleanup.as_mut().unwrap();p.ready=true;p.message="Compare Original / Cleaned, then Apply. Source file stays untouched.".into();self.cleanup_audition();},
   Err(e)=>{self.view.cleanup.as_mut().unwrap().message=e;},
  }
 }
}

#[cfg(test)]mod cleanup_tests{
 use super::*;
 fn logic()->Logic{let params=Arc::new(TeslaParams::default());params.shared.restore(state::Saved{sample:Sample::from_channels(48000,vec![(0..12000).map(|i|(i as f32*0.2).sin()*0.2).collect()],Format::Float32),..Default::default()});Logic::new(params)}
 fn wait(l:&mut Logic){let end=std::time::Instant::now()+std::time::Duration::from_secs(5);while l.cleanup_job.is_some()&&std::time::Instant::now()<end{l.poll();std::thread::sleep(std::time::Duration::from_millis(2));}assert!(l.cleanup_job.is_none());}
 #[test]fn cleanup_preview_cancel_apply_and_link(){
  let mut l=logic();let original=l.doc(|d|d.sample.clone());l.pick(&MenuReq::Main(MenuId::Tools),Cmd::Cleanup,0.,0.);assert!(l.view.cleanup.is_some());l.cleanup_click(7);wait(&mut l);
  assert!(l.view.cleanup.as_ref().unwrap().ready);assert!(Arc::ptr_eq(&original,&l.doc(|d|d.sample.clone())));assert!(!l.doc(|d|d.can_undo()));assert!(l.shared.preview.lock().unwrap().is_some());
  let mut player=crate::Player::default();player.sync(&l.shared);assert!(Arc::ptr_eq(player.sample.as_ref().unwrap(),l.shared.preview.lock().unwrap().as_ref().unwrap()));assert_eq!(l.shared.saved().sample,*original,"saving during preview retains original");
  l.cleanup_click(6);assert!(Arc::ptr_eq(l.shared.preview.lock().unwrap().as_ref().unwrap(),&original));
  l.key(Key::Escape,Mods::default());assert!(l.shared.preview.lock().unwrap().is_none());player.sync(&l.shared);assert!(Arc::ptr_eq(player.sample.as_ref().unwrap(),&original));assert!(Arc::ptr_eq(&original,&l.doc(|d|d.sample.clone())));
  l.cleanup_open();l.cleanup_click(7);wait(&mut l);l.cleanup_click(8);assert_eq!(l.doc(|d|d.history().len()),1);l.doc(|d|d.undo());assert_eq!(*l.doc(|d|d.sample.clone()),*original);
  let(id,link)=crate::cleanup_link::create();let saved=l.shared.saved();let blob=crate::cleanup_link::plugin_state(&saved,id);let json:serde_json::Value=serde_json::from_slice(&blob[8..]).unwrap();let fields:std::collections::BTreeMap<String,String>=serde_json::from_value(json["fields"].clone()).unwrap();
  let params=Arc::new(TeslaParams::default());params.deserialize_fields(&fields);assert!(!params.serialize_fields().contains_key(crate::cleanup_link::FIELD));let mut l=Logic::new(params);assert!(l.view.cleanup.as_ref().unwrap().linked);
  l.cleanup_click(7);wait(&mut l);l.cleanup_click(12);assert!(link.request.lock().unwrap().is_some());assert!(link.pending.load(Ordering::Acquire));assert!(l.view.cleanup.as_ref().unwrap().awaiting);
  link.applied.store(true,Ordering::Release);link.active.store(false,Ordering::Release);link.finish("Applied to track");l.poll_cleanup();assert_eq!(l.doc(|d|d.history().len()),1);assert!(!l.view.cleanup.as_ref().unwrap().awaiting);
  let(_,second)=crate::cleanup_link::create();*l.shared.cleanup_link.lock().unwrap()=Some(second.clone());l.cleanup_close();l.cleanup_return_current();assert!(second.request.lock().unwrap().as_ref().is_some_and(|s|Arc::ptr_eq(s,&l.doc(|d|d.sample.clone()))));
  assert_eq!(l.doc(|d|d.history().len()),1,"returning an edit never runs cleanup again");
 }
 #[test]fn cleanup_stale_job_and_small_panel(){
  let mut l=logic();l.cleanup_open();l.cleanup_click(7);let original=l.doc(|d|d.sample.clone());l.doc(|d|d.version+=1);wait(&mut l);assert!(!l.view.cleanup.as_ref().unwrap().ready);assert!(l.view.cleanup.as_ref().unwrap().message.contains("changed"));assert!(Arc::ptr_eq(&original,&l.doc(|d|d.sample.clone())));
  l.resize_to(680,360);let mut c=Canvas::new(680,360,1.).unwrap();l.paint(&mut c);for h in l.hits.iter().filter(|h|matches!(h.t,Target::Cleanup(_))){assert!(h.r.x>=0.&&h.r.y>=0.&&h.r.right()<=680.&&h.r.bottom()<=360.);}
  if let Some(dir)=std::env::var_os("OS_SHOT_DIR"){c.pixmap.save_png(std::path::PathBuf::from(dir).join("tesla-cleanup.png")).unwrap();}
  l.cleanup_close();l.doc(|d|d.sel=Some((0,5999)));l.cleanup_open();l.cleanup_click(4);wait(&mut l);assert!(l.cleanup_profile.is_some());assert!(l.doc(|d|d.sel.is_none()));
  l.view.cleanup.as_mut().unwrap().linked=true;l.cleanup_click(7);wait(&mut l);l.paint(&mut c);
  if let Some(dir)=std::env::var_os("OS_SHOT_DIR"){c.pixmap.save_png(std::path::PathBuf::from(dir).join("tesla-cleanup-ready.png")).unwrap();}
  l.cleanup_click(1);l.cleanup_click(7);l.cleanup_close();assert!(l.cleanup_job.is_none());assert!(l.view.cleanup.is_none());assert!(Arc::ptr_eq(&original,&l.doc(|d|d.sample.clone())));
 }
}

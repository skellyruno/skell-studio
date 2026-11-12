// SPDX-License-Identifier: GPL-3.0-or-later
impl Logic {
    fn stretch_click(&mut self,id:u8,x:f32,y:f32)->Vec<Act>{
        if id==8 {self.view.stretch=None;return vec![Act::Repaint]}
        let Some(panel)=self.view.stretch.as_mut()else{return vec![]};
        if panel.busy{return vec![]}
        match id {
            0=>panel.percent=(panel.percent-5.).max(25.),
            2=>panel.percent=(panel.percent+5.).min(400.),
            3=>panel.semitones=(panel.semitones-1.).max(-24.),
            5=>panel.semitones=(panel.semitones+1.).min(24.),
            1|4=>return vec![Act::TypeIn(if id==1{Prompt::StretchDuration}else{Prompt::StretchPitch},x,y)],
            6=>*panel=Default::default(),
            7=>{
                let (percent,semitones)=(panel.percent,panel.semitones);
                if percent==100.&&semitones==0.{panel.message="No change: adjust duration or pitch first.".into();return vec![Act::Repaint]}
                if self.shared.armed.load(Ordering::Acquire){panel.message="Stop recording before applying.".into();return vec![Act::Repaint]}
                let (source,version,range)=self.doc(|d|(d.sample.clone(),d.version,d.range()));
                let Some(range)=range else{return vec![]};
                let frames=((range.1-range.0+1) as f64*percent/100.).round().max(1.) as usize;
                let selection=(range.0,range.0+frames-1);
                let (tx,rx)=std::sync::mpsc::channel();let input=source.clone();
                std::thread::spawn(move||{let result=std::panic::catch_unwind(||crate::stretch::render(&input,range,percent,semitones)).unwrap_or_else(|_|Err("Processing failed. Original audio preserved.".into()));let _=tx.send(result);});
                self.stretch_job=Some((rx,source,version,selection));
                let panel=self.view.stretch.as_mut().unwrap();panel.busy=true;panel.message="Rendering audio… Cancel keeps the original.".into();
            },
            _=>{},
        }
        vec![Act::Repaint]
    }
    fn poll_stretch(&mut self){
        use std::sync::mpsc::TryRecvError;
        let Some((rx,_,_,_))=&self.stretch_job else{return};
        let result=match rx.try_recv(){Ok(r)=>r,Err(TryRecvError::Empty)=>return,Err(TryRecvError::Disconnected)=>Err("Processing stopped. Original preserved.".into())};
        let (_,source,version,selection)=self.stretch_job.take().unwrap();
        if self.view.stretch.is_none(){return}
        let result=result.and_then(|sample|self.doc(|d|{
            if d.version!=version||!Arc::ptr_eq(&source,&d.sample)||self.shared.armed.load(Ordering::Acquire){return Err("Audio changed while rendering. Please try again.".into())}
            d.apply_stretch(sample,selection);d.view.start=0.;d.view.len=0.;Ok(())
        }));
        match result {
            Ok(())=>{self.view.stretch=None;self.hint="Applied time stretch / pitch shift. Undo restores original audio.".into();},
            Err(e)=>if let Some(panel)=self.view.stretch.as_mut(){panel.busy=false;panel.message=e;},
        }
    }
}

#[cfg(test)]mod stretch_interaction_tests {
    use super::*;
    #[test]fn stretch_toolbar_apply_and_reopen(){
        let mut logic=Logic::new(Arc::new(TeslaParams::default()));
        logic.doc(|d|d.sample=Arc::new(Sample::from_channels(48000,vec![vec![0.1;4800]],crate::sample::Format::Float32)));
        let mut c=os_ui::Canvas::new(800,400,1.).unwrap();logic.paint(&mut c);
        let hit=logic.hits.iter().find(|h|h.t==Target::Tool(12)).unwrap().r;
        logic.down(hit.cx(),hit.cy(),Btn::Left,Mods::default(),false);assert!(logic.view.stretch.is_some());
        logic.typed(&Prompt::StretchDuration,"150");logic.paint(&mut c);
        let hit=logic.hits.iter().find(|h|h.t==Target::Stretch(7)).unwrap().r;
        logic.down(hit.cx(),hit.cy(),Btn::Left,Mods::default(),false);
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(5);
        while logic.stretch_job.is_some()&&std::time::Instant::now()<deadline{logic.poll();std::thread::sleep(std::time::Duration::from_millis(5));}
        assert!(logic.stretch_job.is_none());assert!(logic.view.stretch.is_none());
        assert_eq!(logic.doc(|d|d.frames()),7200);assert_eq!(logic.doc(|d|d.history().len()),1);
        logic.doc(|d|{d.undo();});assert_eq!(logic.doc(|d|d.frames()),4800);
        logic.pick(&MenuReq::Main(MenuId::Tools),Cmd::Stretch,0.,0.);assert!(logic.view.stretch.is_some());
        logic.key(Key::Escape,Mods::default());assert!(logic.view.stretch.is_none());
    }
    #[test]fn stretch_cancel_and_stale_result_preserve_audio(){
        let mut logic=Logic::new(Arc::new(TeslaParams::default()));
        logic.doc(|d|d.sample=Arc::new(Sample::from_channels(48000,vec![vec![0.1;4800]],crate::sample::Format::Float32)));
        let original=logic.doc(|d|d.sample.clone());
        logic.view.stretch=Some(Default::default());logic.typed(&Prompt::StretchDuration,"200");logic.stretch_click(7,0.,0.);
        assert!(logic.view.stretch.as_ref().unwrap().busy);logic.stretch_click(8,0.,0.);logic.poll_stretch();
        assert!(logic.doc(|d|Arc::ptr_eq(&d.sample,&original)));assert!(!logic.doc(|d|d.can_undo()));
        logic.view.stretch=Some(Default::default());let(tx,rx)=std::sync::mpsc::channel();
        logic.stretch_job=Some((rx,original.clone(),logic.doc(|d|d.version),(0,4799)));
        logic.doc(|d|{d.version+=1;});tx.send(Ok((*original).clone())).unwrap();logic.poll_stretch();
        assert!(logic.doc(|d|Arc::ptr_eq(&d.sample,&original)));assert!(logic.view.stretch.as_ref().unwrap().message.contains("changed"));
        let (tx,rx)=std::sync::mpsc::channel();logic.stretch_job=Some((rx,original.clone(),logic.doc(|d|d.version),(0,4799)));drop(tx);logic.poll_stretch();assert!(!logic.view.stretch.as_ref().unwrap().busy);
        let mut c=os_ui::Canvas::new(800,400,1.).unwrap();logic.paint(&mut c);
        if let Some(dir)=std::env::var_os("OS_SHOT_DIR"){c.pixmap.save_png(std::path::PathBuf::from(dir).join("tesla-stretch.png")).unwrap();}
    }
}

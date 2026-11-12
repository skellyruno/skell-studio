//! Reusable output buffers and jobs for independent instrument processing.
use super::{ChannelState, lock_busy};
use crate::{Instrument, SharedInstrument};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

pub(super) struct State {
    pub spare: Vec<Job>,
    pub outputs: Vec<Job>,
    pub cost: Duration,
    pub tx: mpsc::Sender<Job>,
    pub rx: mpsc::Receiver<Job>,
}
impl State {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { spare: Vec::new(), outputs: Vec::new(), cost: Duration::ZERO, tx, rx }
    }
    pub fn ready(&self, channels: &[ChannelState]) -> bool {
        if self.cost < Duration::from_micros(300) { return false; }
        let mut count = 0;
        for (index, channel) in channels.iter().enumerate() {
            let Some(handle) = &channel.instrument.0 else { continue };
            if channels[..index].iter().any(|c| c.instrument.0.as_ref().is_some_and(|h| Arc::ptr_eq(h, handle))) {
                return false;
            }
            let Ok(inst) = handle.try_lock() else { return false };
            if !inst.parallel_processing() { return false; }
            count += 1;
        }
        count >= 2
    }
}

#[derive(Default)]
pub(super) struct Job {
    pub channel: usize,
    pub instrument: Option<SharedInstrument>,
    pub frames: usize,
    pub rate: u32,
    pub gains: (f32, f32),
    pub voiced: bool,
    pub audio: Vec<(Vec<f32>, Vec<f32>)>,
    pub ports: usize,
    pub extra_active: u16,
    pub active: bool,
    pub delay: u32,
    pub rendered: bool,
    pub elapsed: Duration,
    pub until: Option<Instant>,
    pub wait: Duration,
    pub export: bool,
}
impl Job {
    pub fn render(&mut self, inst: &mut dyn Instrument) {
        let start = Instant::now();
        self.rendered = false;
        self.audio.resize_with(self.audio.len().max(1), Default::default);
        let (left, right) = &mut self.audio[0];
        left.resize(self.frames, 0.);
        right.resize(self.frames, 0.);
        left.fill(0.);
        right.fill(0.);
        inst.process(self.rate, left, right);
        self.delay = inst.reported_delay();
        self.active = inst.active();
        self.ports = 1 + inst.extra_outputs().min(15);
        self.audio.resize_with(self.audio.len().max(self.ports), Default::default);
        self.extra_active = 0;
        for port in 1..self.ports {
            let (left, right) = &mut self.audio[port];
            left.resize(self.frames, 0.);
            right.resize(self.frames, 0.);
            left.fill(0.);
            right.fill(0.);
            if inst.extra_output(port-1, left, right) { self.extra_active |= 1 << port; }
        }
        self.elapsed = start.elapsed();
        self.rendered = true;
    }
    pub fn run(&mut self) {
        let _float = os_dsp::audio_float::AudioFloat::enter();
        self.rendered = false;
        self.elapsed = Duration::ZERO;
        let handle = self.instrument.as_ref().expect("prepared instrument").clone();
        // A failure must return its job so the engine can account for the missed frames.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if let Some(mut inst) = lock_busy(&handle, &mut self.until, self.wait, self.export) {
                self.render(&mut *inst);
            }
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct Failure;
    impl Instrument for Failure {
        fn note_on(&mut self, _:u32, _:u8, _:f32) {}
        fn note_off(&mut self, _:u32, _:u8) {}
        fn process(&mut self, _:u32, _: &mut[f32], _: &mut[f32]) { panic!("test instrument failure"); }
    }

    #[test]
    fn busy_and_panicking_instruments_return_their_jobs() {
        let handle: SharedInstrument = Arc::new(Mutex::new(Failure));
        let mut job = Job { instrument: Some(handle.clone()), frames:64, rate:48000, ..Default::default() };
        let guard = handle.lock().unwrap();
        job.run();
        assert!(!job.rendered);
        drop(guard);
        let (tx, rx) = mpsc::channel();
        super::super::mix_workers::submit_instrument(job, tx);
        let returned = rx.recv_timeout(Duration::from_secs(2)).expect("failure returns the job");
        assert!(!returned.rendered);
        assert!(handle.is_poisoned());
    }
}

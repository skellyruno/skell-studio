// SPDX-License-Identifier: GPL-3.0-or-later
//! One-use, in-process return link. Never persisted or accepted from another process.
use crate::{sample::Sample, state};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, OnceLock, Weak,
    },
};
pub const FIELD: &str = "tesla_cleanup_return";
#[derive(Default)]
pub struct Session {
    pub active: AtomicBool,
    pub applied: AtomicBool,
    pub pending: AtomicBool,
    pub source: Mutex<Option<(Sample, String)>>,
    pub request: Mutex<Option<Arc<Sample>>>,
    pub status: Mutex<String>,
}
impl Session {
    pub fn message(&self) -> String {
        self.status.lock().unwrap().clone()
    }
    pub fn finish(&self, message: &str) {
        *self.status.lock().unwrap() = message.into();
        self.pending.store(false, Ordering::Release);
    }
    pub fn submit(&self, s: Arc<Sample>) -> bool {
        if !self.active.load(Ordering::Acquire) || self.pending.swap(true, Ordering::AcqRel) {
            return false;
        }
        *self.status.lock().unwrap() = "Applying to the source clip…".into();
        *self.request.lock().unwrap() = Some(s);
        true
    }
}
fn sessions() -> &'static Mutex<HashMap<u64, Weak<Session>>> {
    static R: OnceLock<Mutex<HashMap<u64, Weak<Session>>>> = OnceLock::new();
    R.get_or_init(Default::default)
}
pub fn create() -> (u64, Arc<Session>) {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let s = Arc::new(Session::default());
    s.active.store(true, Ordering::Release);
    let mut r = sessions().lock().unwrap();
    r.retain(|_, v| v.strong_count() > 0);
    r.insert(id, Arc::downgrade(&s));
    (id, s)
}
pub(crate) fn lookup(id: u64) -> Option<Arc<Session>> {
    sessions().lock().ok()?.get(&id)?.upgrade()
}
pub fn plugin_state(saved: &state::Saved, id: u64) -> Vec<u8> {
    let raw = state::plugin_state(saved);
    let mut json: serde_json::Value = serde_json::from_slice(&raw[8..]).unwrap();
    json["fields"][FIELD] = id.to_string().into();
    let data = serde_json::to_vec(&json).unwrap();
    let mut out = (data.len() as u64).to_le_bytes().to_vec();
    out.extend(data);
    out
}

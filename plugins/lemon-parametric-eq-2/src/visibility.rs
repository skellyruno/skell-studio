//! Profile visibility for the compact EQ band card.
pub const BAND_CARD: &str = "visible_eq-band-card";

pub const BAND_OVERVIEW: &str = "visible_eq-band-overview";
pub fn band_card() -> bool { shown(BAND_CARD) }
pub fn band_overview() -> bool { shown(BAND_OVERVIEW) }
fn shown(key: &str) -> bool {
    use std::{cell::Cell, time::{Duration, Instant}};
    thread_local! {
        static CACHED: Cell<Option<(Instant, bool, bool)>> = const { Cell::new(None) };
    }
    CACHED.with(|cache| {
        if let Some((at, card, overview)) = cache.get() {
            if at.elapsed() < Duration::from_millis(300) {
                return if key == BAND_CARD { card } else { overview };
            }
        }
        let text = os_paths::user_file("settings.txt")
            .and_then(|path| std::fs::read_to_string(path).ok())
            .unwrap_or_default();
        let visible = |name: &str| !text.lines().rev().filter_map(|line| line.split_once('='))
            .find(|(k, _)| k.trim() == name).is_some_and(|(_, v)| v.trim() == "0");
        let (card, overview) = (visible(BAND_CARD), visible(BAND_OVERVIEW));
        cache.set(Some((Instant::now(), card, overview)));
        if key == BAND_CARD { card } else { overview }
    })
}

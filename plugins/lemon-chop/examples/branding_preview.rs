// SPDX-License-Identifier: GPL-3.0-or-later
//! Render the stock editor without opening a window or audio device.
use std::sync::Arc;
fn main() {
    let out=std::env::args().nth(1).expect("output PNG");
    let mut logic=lemon_chop::editor::Logic::new(Arc::new(lemon_chop::params::ChopParams::default()),Arc::new(lemon_chop::Shared::default()),None);
    let mut canvas=os_ui::Canvas::new(lemon_chop::view::W,lemon_chop::view::H,1.).unwrap();
    logic.paint(&mut canvas,&os_plugkit::palette());
    canvas.pixmap.save_png(out).unwrap();
}

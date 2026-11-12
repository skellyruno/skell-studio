// SPDX-License-Identifier: GPL-3.0-or-later
//! Render the stock editor without opening a window or audio device.
use std::sync::Arc;
fn main() {
    let out=std::env::args().nth(1).expect("output PNG");
    let params=Arc::new(lemon_wave::assets::WaveParams::default());
    let editor=Arc::new(lemon_wave::table_editor::TableEditor::new(params.clone()));
    let shared=Arc::new(lemon_small_fx::Shared{extension:Some(editor),..Default::default()});
    let mut logic=lemon_small_fx::editor::Logic::new(lemon_wave::controls::SPEC,params.base.clone(),shared,None);
    let mut canvas=os_ui::Canvas::new(1584,1056,1.).unwrap();
    logic.paint(&mut canvas,&os_plugkit::palette());
    canvas.pixmap.save_png(out).unwrap();
}

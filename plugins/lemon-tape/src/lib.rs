// SPDX-License-Identifier: GPL-3.0-or-later
#[cfg(not(feature="linked"))]use nih_plug::prelude::*;
pub use lemon_small_fx::tape::LemonTape;
#[cfg(not(feature="linked"))]nih_export_clap!(LemonTape);
#[cfg(not(feature="linked"))]nih_export_vst3!(LemonTape);
os_plugkit::linked_clap_entry!(LemonTape);

// SPDX-License-Identifier: GPL-3.0-or-later
pub use lemon_small_fx::hall::LemonHall;
#[cfg(not(feature = "linked"))]
use nih_plug::prelude::*;
#[cfg(not(feature = "linked"))]
nih_export_clap!(LemonHall);
#[cfg(not(feature = "linked"))]
nih_export_vst3!(LemonHall);
os_plugkit::linked_clap_entry!(LemonHall);

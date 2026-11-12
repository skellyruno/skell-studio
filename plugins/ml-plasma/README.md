# ML Plasma — private development

AQUILA adaptation of Gridline Plasma by lolitemaultes, pinned to commit
5ed1fa53f52df0009d2c6294a5e0b6e6861dba35 (11 November 2026 retrieval).
Upstream: https://github.com/lolitemaultes/Gridline
Original instrument documentation: vendor/gridline/docs/plugins/plasma.md.

GPL-3.0-only. The original source, copyright headers and dependency licence
texts remain in vendor/gridline. ML modifications add a distinct plugin identity,
ML branding, charcoal surfaces, lemon highlights, cyan displays and violet
modulation. Host automation is synchronized into native preset state before saving,
including when the editor has never opened. This is a Gridline-derived instrument, with its original synthesis
engine and preset importer. It is not Serum and does not promise identical
Serum sound or preset compatibility. No Xfer code, presets or audio is included.

The studio embeds the matching native CLAP engine and editor resources. On first
use it verifies and extracts its payload into the studio user profile. Updates
carry the instrument with them; no plugin scan, separate download or registration
is required. Settings > Visibility can hide ML Plasma from plugin menus without
preventing existing projects from loading and sounding. The normal studio window
frame supplies detach/attach, close and visibility controls.

Identity: CLAP de.michaellane.ml-plasma; native Plasma state and parameter numbers
are retained. The ML identity is separate from upstream Gridline Plasma, so both
can coexist. The adapted editor uses a separate ml-plasma settings directory.

Building the ml-plasma Cargo crate invokes build_native.py and the normal storage
guard. CMake, Python and a C++17 compiler are required; SDL is fetched with its
upstream pinned checksum into the build directory. Every release architecture
must build its own payload. A missing/failed engine is a build failure, never a
silently incomplete stock-plugin update. No installation or publication occurs.

For the existing Windows-to-Linux cross build, first build this same pinned source
on native Linux and supply its ZIP through `ML_PLASMA_BUNDLE_LINUX_X86_64` to the
coordinator's environment. Other native payload overrides follow the same
`ML_PLASMA_BUNDLE_<OS>_<ARCH>` convention (MACOS/AARCH64, MACOS/X86_64,
WINDOWS/X86_64). The build validates the archive's checksums, pinned source,
platform label and actual PE/ELF/Mach-O architecture. Native Windows and Mac
builds compile their payload automatically. Mac/Linux runtime and packaging
validation are required before publishing; no platform is claimed tested here.

The native adapter also clears voices/tails/buffered audio on host reset and
reports editor-only asset changes through CLAP state.mark_dirty. The separately
built Windows `ml_plasma_editor_fixture` target is for device-free regression
checks only; it is never included in the embedded shipping payload. Run the
ignored ML host editor test explicitly with `ML_PLASMA_EDITOR_TEST_LIBRARY`
pointing at its `ML Plasma Editor Test.clap` output.

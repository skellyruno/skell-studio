# Skell Studio QuickView — local native screenshot capture

QuickView runs **Skell Studio's existing built-in renderer**, `skell-studio.exe --screenshot <file>.png`. The pixels come directly from the DAW's production `App::paint`/Canvas path, not a generated mockup or a browser approximation. A built executable is required, but **repeated screenshots do not require a rebuild**.

## One-click Windows launcher

In the source checkout, **double-click `Skell-QuickView.cmd`**. It uses an already-built executable if present, or starts the fast local build once if needed. If you have downloaded and extracted an engineering `skell-studio.exe` elsewhere, **drag the EXE onto `Skell-QuickView.cmd`**: the script will capture the actual interface directly from that executable without compiling anything.

The gallery opens from `.skell-quickview/latest/index.html`. This only displays frames rendered by the app itself.

## Fastest route when you already have an EXE

From a PowerShell terminal in the repository:

```powershell
.\scripts\skell-quickview.ps1 -Mode Render -Exe "C:\path\to\skell-studio.exe" -Open
```

Capture a specific view without opening the full GUI:

```powershell
.\scripts\skell-quickview.ps1 -Mode Render -Exe "C:\path\to\skell-studio.exe" -View piano -Size 1280x720 -Open
```

**No Cargo installation is necessary to render from an existing Windows EXE.** This is the quickest path after downloading the engineering-build artifact.

## Build locally with a fast preview profile

First use, from a source checkout that contains `[profile.skell-preview]` in the top-level `Cargo.toml`:

```powershell
.\scripts\skell-quickview.ps1 -Mode Auto -Open
```

That reuses an existing Skell Studio EXE or, if none exists, builds one using `cargo build --locked --profile skell-preview -p os-app --bin skell-studio -j 3`. You'll need Rust, the MSVC C++ build tools, and LLVM for first-time compilation.

After source changes, explicitly rebuild just the engineering executable, then capture:

```powershell
.\scripts\skell-quickview.ps1 -Mode BuildRender -Open
```

For subsequent screenshot passes with unchanged source:

```powershell
.\scripts\skell-quickview.ps1 -Mode Render -View mixer -Open
```

Build only without rendering: `-Mode Build`. Use `-Jobs 2` if compilation exhausts RAM, or `-Jobs 4` if the laptop has sufficient headroom. Avoid `cargo clean` between iterations; it throws away the compilation cache.

## What it captures

`-View all` (the default) captures five native views:

- Playlist without rack
- Playlist with channel rack
- Piano roll
- Mixer with the first insert focused
- Plugin palette overlay

All are saved under `.skell-quickview/latest/`, along with `index.html`, a dark/green gallery of **actual screenshot PNG files**. Add `-Tag before-changes` to preserve a named baseline and compare it later.

Change `-Size 1440x900` (default) to `1280x720`, `1920x1080`, or another supported size. Run with `-Plan` to list the capture command plan without compiling or launching anything.

The script forces `OPENSTUDIO_USER_DIR` to a separate `.skell-quickview/isolated-settings` folder so it does not migrate or modify the user's normal Skell Studio/Lemon Studio settings.

## Why it's faster

The inherited DAW's `[profile.dev]` and dependency overrides use `opt-level = 3`, which makes the normal Debug profile relatively expensive to compile. The preview branch introduces an explicit `skell-preview` profile with `opt-level = 0`, `debug = 0`, and local incremental builds. It skips bundled plug-in packaging via `OPENSTUDIO_NO_PLUGIN_BUILD=1` (the app's compiled-in Rust dependencies are still required). This is **an engineering UI build**, not a release-quality or audio benchmark configuration.

The first local build can still be slow because there are many native and Rust dependencies, especially on a machine with limited RAM. We have not measured the exact speedup. **Rendering without rebuilding** avoids the main recurring compile cost. An EXE from a downloaded GitHub Actions artifact avoids all local compilation.

## Verification limits

- Native renderer capture verifies actual source drawing, including layout and theme at the requested size.
- This is **not** a screenshot of an interactive Windows desktop window; actual controls, Win32 dialogs, animations, VST3 plug-in editors, sound hardware, and CPU/latency require a separately launched Windows app.
- Use `-Mode BuildRender` after any source change, or your captures may reflect a stale executable.
- Keep original FL Studio projects backed up before opening them in the actual app. QuickView does not overwrite FLPs.

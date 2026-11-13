// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Lemon Studio: https://github.com/ItsAlphaNeon/Lemon-Studio

//! Transparent AQUILA startup mark, progress and status, drawn independently of the main window.
// spec: ui/settings-choices.md "Miscellaneous: naming, detached plugins and startup"
#![cfg_attr(not(any(windows, target_os = "macos", target_os = "linux")), allow(dead_code))]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use os_ui::{Canvas, Color, Font};
use tiny_skia::Pixmap;

pub const SIZE: (f32, f32) = (500.0, 500.0);
// The brand mark and typography are drawn by Canvas, not a bitmap.

/// Where startup's steps start on the gauge (0..1). The opened project's loading fills it from `LOADING` to `LOADED`.
pub const SETTINGS: f32 = 0.06;
pub const PROJECT: f32 = 0.2;
pub const MIDI: f32 = 0.32;
pub const WINDOW: f32 = 0.4;
pub const PLUGINS: f32 = 0.5;
pub const LOADING: f32 = 0.62;
pub const LOADED: f32 = 0.96;

const FADE_IN: f32 = 0.3;
const FOLLOW: f32 = 0.18;
const TOP_UP: f32 = 0.4;
const POP: (f32, f32) = (0.4, 0.8);
const FADE_OUT: (f32, f32) = (1.0, 1.3);
/// Seconds from the splash opening: a quicker startup still sees the gauge fill until the first; one taking longer
/// than the second does not keep the splash up.
const EARLIEST: f32 = 1.5;
const LONGEST: f32 = 120.0;

/// Whether this start shows the splash.
pub fn wanted() -> bool {
    crate::prefs::check(&crate::prefs::SHOW_SPLASH)
}

/// What the UI thread tells the splash.
#[derive(Default)]
struct Feed {
    /// Startup's progress (0..=1, never going back) and the step it is on.
    progress: f32,
    step: String,
    /// Startup is done.
    done: bool,
}

/// Between the UI thread and the splash's.
#[derive(Default)]
struct Shared {
    feed: Mutex<Feed>,
    /// The splash finished and faded away (on macOS its process ends instead).
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    gone: AtomicBool,
    /// The splash was dropped: the window closes.
    closed: AtomicBool,
}

impl Shared {
    /// Startup's progress, step and whether it is done.
    fn feed(&self) -> (f32, String, bool) {
        let f = self.feed.lock().unwrap_or_else(|e| e.into_inner());
        (f.progress, f.step.clone(), f.done)
    }
}

/// The splash while it shows: dropping it closes the window.
pub struct Splash {
    shared: Arc<Shared>,
    #[cfg(any(windows, target_os = "macos", target_os = "linux"))]
    window: platform::Window,
}

impl Splash {
    /// Startup has reached `progress` (0..1) and is on the step `what`.
    pub fn step(&self, progress: f32, what: &str) {
        let mut f = self.shared.feed.lock().unwrap_or_else(|e| e.into_inner());
        let p = f.progress.max(progress.clamp(0.0, 1.0));
        if p == f.progress && f.step == what {
            return;
        }
        f.progress = p;
        if f.step != what {
            f.step = what.to_string();
        }
        #[cfg(target_os = "macos")]
        self.window.tell(&f);
    }

    /// Startup is done: the gauge tops up and the splash goes.
    pub fn finish(&self) {
        let mut f = self.shared.feed.lock().unwrap_or_else(|e| e.into_inner());
        if !f.done {
            f.done = true;
            #[cfg(target_os = "macos")]
            self.window.tell(&f);
        }
    }

    /// Whether the splash has finished and faded away: it can be dropped.
    pub fn gone(&self) -> bool {
        #[cfg(target_os = "macos")]
        return self.window.gone();
        #[cfg(not(target_os = "macos"))]
        self.shared.gone.load(Ordering::Acquire)
    }
}

/// `Splash::step`, when there is a splash.
pub fn step(splash: &Option<Splash>, progress: f32, what: &str) {
    if let Some(s) = splash {
        s.step(progress, what);
    }
}

/// Opens the splash when its check is on (`wanted`). None when it is off, or where there is no splash yet.
pub fn open() -> Option<Splash> {
    if !wanted() {
        return None;
    }
    #[cfg(any(windows, target_os = "macos", target_os = "linux"))]
    {
        let shared = Arc::new(Shared::default());
        let window = platform::open(shared.clone())?;
        Some(Splash { shared, window })
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    None
}

impl Drop for Splash {
    fn drop(&mut self) {
        self.shared.closed.store(true, Ordering::SeqCst);
        #[cfg(any(windows, target_os = "macos", target_os = "linux"))]
        self.window.close();
    }
}

#[cfg(windows)]
use win as platform;

#[cfg(target_os = "macos")]
use mac as platform;

#[cfg(target_os = "linux")]
use x11 as platform;

#[cfg(target_os = "macos")]
pub use mac::{child_main, ARG};

// ---------------------------------------------------------------------------------------------------------------
// The animation.

/// The splash's animation: what it shows at each moment, from what startup reports.
#[derive(Default)]
struct Anim {
    /// The progress shown (following startup's), and when the last frame was.
    shown: f32,
    last: f32,
    /// Finishing: when it began, and the progress shown then.
    done: Option<(f32, f32)>,
}

/// One frame of the splash.
#[derive(Clone, Debug, PartialEq)]
struct Look {
    /// The window's opacity: fading in at the start, away at the end.
    alpha: f32,
    /// The logo's scale: growing in at the start, popping at the end.
    scale: f32,
    /// The amount of progress shown, 0..=1.
    progress: f32,
    /// The status line.
    caption: String,
}

impl Anim {
    /// The splash `t` seconds after it opened, startup being at `progress` on the step `what` (and `done`); `None`
    /// once it has finished and faded away.
    fn frame(&mut self, t: f32, progress: f32, what: &str, done: bool) -> Option<Look> {
        let dt = (t - self.last).max(0.0);
        self.last = t;
        if self.done.is_none() && (done || t >= LONGEST) && t >= EARLIEST {
            self.done = Some((t, self.shown));
        }
        let k = ease_out(seg(t, 0.0, FADE_IN));
        let status = |p: f32| match what {
            "" => format!("{:.0}%", p * 100.0),
            _ => format!("{what} · {:.0}%", p * 100.0),
        };
        let Some((t0, from)) = self.done else {
            // The gauge eases toward startup's progress (all the way once it is done), never going back.
            let to = if done { 1.0 } else { progress.clamp(0.0, 1.0) };
            self.shown += (to - self.shown).max(0.0) * (1.0 - (-dt / FOLLOW).exp());
            return Some(Look { alpha: k, scale: lerp(0.94, 1.0, k), progress: self.shown, caption: status(self.shown) });
        };
        let d = t - t0;
        if d >= FADE_OUT.1 {
            return None;
        }
        let progress = lerp(from, 1.0, ease_out(seg(d, 0.0, TOP_UP)));
        Some(Look {
            alpha: k * (1.0 - ease_in_out(seg(d, FADE_OUT.0, FADE_OUT.1))),
            scale: lerp(0.94, 1.0, k) * (1.0 + 0.03 * bump(seg(d, POP.0, POP.1))),
            progress,
            caption: if d >= TOP_UP { "Ready".into() } else { status(progress) },
        })
    }
}

/// How far `t` is from `a` to `b`, 0..=1.
fn seg(t: f32, a: f32, b: f32) -> f32 {
    ((t - a) / (b - a)).clamp(0.0, 1.0)
}

/// From `a` (at 0) to `b` (at 1), exactly at either end.
fn lerp(a: f32, b: f32, k: f32) -> f32 {
    a * (1.0 - k) + b * k
}

fn ease_out(k: f32) -> f32 {
    1.0 - (1.0 - k).powi(3)
}

fn ease_in_out(k: f32) -> f32 {
    if k < 0.5 { 4.0 * k * k * k } else { 1.0 - (2.0 - 2.0 * k).powi(3) / 2.0 }
}

/// Up and back down over 0..1.
fn bump(k: f32) -> f32 {
    if k <= 0.0 || k >= 1.0 { 0.0 } else { (std::f32::consts::PI * k).sin() }
}

// ---------------------------------------------------------------------------------------------------------------
// The picture.

/// The window's size in device px at `scale` (device px per logical one).
fn device(scale: f32) -> (u32, u32) {
    ((SIZE.0 * scale).round().max(1.0) as u32, (SIZE.1 * scale).round().max(1.0) as u32)
}

struct Painter {
    frame: Canvas,
}

impl Painter {
    fn new(scale: f32) -> Option<Self> {
        let (w, h) = device(scale);
        let mut frame = Canvas::new(w, h, scale).ok()?;
        frame.set_theme_font(2, None);
        Some(Self { frame })
    }

    fn paint(&mut self, look: &Look) -> &Pixmap {
        let cv = &mut self.frame;
        cv.pixmap.fill(tiny_skia::Color::TRANSPARENT);
        // Compact hardware-inspired black surface. No inherited ML splash image.
        let panel = os_ui::Rect::new(55.0, 32.0, 390.0, 442.0);
        cv.fill_rrect(panel, 22.0, Color::rgb(0x0a100b));
        cv.stroke_rrect(panel, 22.0, 1.0, Color::rgb(0x36563b));
        let green = os_theme::design::lemon::MAIN;
        let glow = os_theme::design::lemon::DEEP;
        cv.fill_rrect(os_ui::Rect::new(204.0, 52.0, 92.0, 4.0), 2.0, glow);
        crate::icons::logo(cv, 250.0, 189.0, 89.0 * look.scale, green);

        let f = Font::new(42);
        let white = Color::rgb(0xf0f5ed);
        let title = "SKELL";
        let suffix = "STUDIO";
        let gap = 9.0;
        let a = cv.text_width(f, title);
        let b = cv.text_width(f, suffix);
        let start = (SIZE.0 - a - b - gap) / 2.0;
        cv.text(f, start, 324.0, white, title);
        cv.text(f, start + a + gap, 324.0, green, suffix);
        let caption = "CREATE WITHOUT LIMITS";
        let small = Font::new(13);
        let caption_width = cv.text_width(small, caption);
        cv.text(small, (SIZE.0 - caption_width) / 2.0, 357.0, Color::rgb(0xb1c4ac), caption);

        let span = 248.0;
        let x = (SIZE.0 - span) / 2.0;
        cv.fill_rrect(os_ui::Rect::new(x, 409.0, span, 3.0), 1.5, Color::rgb(0x263c2a));
        if look.progress > 0.0 {
            cv.fill_rrect(os_ui::Rect::new(x, 409.0, span * look.progress, 3.0), 1.5, green);
        }
        let status_font = Font::new(14);
        let w = cv.text_width(status_font, &look.caption);
        cv.text(status_font, (SIZE.0 - w) / 2.0, 439.0, white, &look.caption);
        &cv.pixmap
    }
}

#[cfg(windows)]
mod win {
    use std::cell::RefCell;
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::sync::Arc;
    use std::time::Instant;

    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetMonitorInfoW, MonitorFromPoint, SelectObject, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS, HBITMAP, HDC, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, KillTimer, PostMessageW, PostQuitMessage, RegisterClassW, SetTimer,
        ShowWindow, TranslateMessage, UpdateLayeredWindow, MA_NOACTIVATE, MSG, SW_SHOWNOACTIVATE, ULW_ALPHA, WM_CLOSE, WM_DESTROY, WM_MOUSEACTIVATE, WM_TIMER,
        WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    };

    use super::{device, Anim, Painter, Shared};

    const CLASS: &str = "SkellStudioSplash";
    /// The frame timer and its period (ms).
    const TIMER: usize = 1;
    const FRAME_MS: u32 = 15;

    /// The splash's window, as the splash thread made it (0 until then).
    pub struct Window {
        hwnd: Arc<AtomicIsize>,
    }

    impl Window {
        /// Closes the window. Not made yet: the thread sees `Shared::closed` and does not make it.
        pub fn close(&self) {
            let hwnd = self.hwnd.load(Ordering::SeqCst);
            if hwnd != 0 {
                // SAFETY: a window of the splash thread, or one already gone (the post then fails harmlessly).
                unsafe { PostMessageW(hwnd as HWND, WM_CLOSE, 0, 0) };
            }
        }
    }

    /// The splash thread's drawing: the animation, its painter, and the bitmap the window is updated from.
    struct State {
        shared: Arc<Shared>,
        anim: Anim,
        painter: Painter,
        start: Instant,
        dc: HDC,
        bitmap: HBITMAP,
        bits: *mut u32,
        size: (i32, i32),
    }

    impl Drop for State {
        fn drop(&mut self) {
            // SAFETY: the memory DC and bitmap made for this state, no longer used.
            unsafe {
                DeleteDC(self.dc);
                DeleteObject(self.bitmap);
            }
        }
    }

    thread_local! {
        static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    pub fn open(shared: Arc<Shared>) -> Option<Window> {
        let hwnd = Arc::new(AtomicIsize::new(0));
        let h = hwnd.clone();
        std::thread::Builder::new().name("splash".into()).spawn(move || run(shared, &h)).ok()?;
        Some(Window { hwnd })
    }

    /// The splash thread: makes the window, then animates it until it is closed or has gone.
    fn run(shared: Arc<Shared>, hwnd_out: &AtomicIsize) {
        // SAFETY: plain Win32 calls on this thread's own window and GDI objects; the structs are zeroed C structs.
        unsafe {
            // The primary monitor, in physical px: winit made the process per-monitor DPI aware when the event loop
            // was built, before this.
            let mon = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
            let mut mi: MONITORINFO = std::mem::zeroed();
            mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            if GetMonitorInfoW(mon, &mut mi) == 0 {
                return;
            }
            let (mut dx, mut dy) = (96, 96);
            if GetDpiForMonitor(mon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) != 0 {
                dx = 96;
            }
            // The UI scale there: the monitor's, or Main GUI scaling's (spec: ui/ui-scaling.md §1).
            let scale = crate::gui_scale(f64::from(dx) / 96.0) as f32;
            let Some(painter) = Painter::new(scale) else { return };
            let (w, h) = device(scale);
            let (w, h) = (w as i32, h as i32);
            // The bitmap the window is updated from: 32-bit premultiplied BGRA, top row first.
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            };
            let mut bits = std::ptr::null_mut();
            let bitmap = CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
            let state = State { shared: shared.clone(), anim: Anim::default(), painter, start: Instant::now(), dc, bitmap, bits: bits.cast(), size: (w, h) };
            if dc.is_null() || bitmap.is_null() || bits.is_null() {
                return;
            }
            SelectObject(dc, bitmap);
            let r = mi.rcMonitor;
            let (x, y) = (r.left + (r.right - r.left - w) / 2, r.top + (r.bottom - r.top - h) / 2);
            let inst = GetModuleHandleW(std::ptr::null());
            let class = wide(CLASS);
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.lpfnWndProc = Some(splash_proc);
            wc.hInstance = inst;
            wc.lpszClassName = class.as_ptr();
            RegisterClassW(&wc);
            let title = wide("Skell Studio");
            // Layered (per-pixel alpha) and transparent to the mouse; above every window that is not topmost too; a
            // tool window takes no taskbar button; never activated, so the main window gets the focus as before.
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class.as_ptr(),
                title.as_ptr(),
                WS_POPUP,
                x,
                y,
                w,
                h,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                inst,
                std::ptr::null(),
            );
            if hwnd.is_null() {
                return;
            }
            hwnd_out.store(hwnd as isize, Ordering::SeqCst);
            if shared.closed.load(Ordering::SeqCst) {
                DestroyWindow(hwnd);
                return;
            }
            STATE.with(|s| *s.borrow_mut() = Some(state));
            // The first frame before the window shows.
            frame(hwnd);
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            SetTimer(hwnd, TIMER, FRAME_MS, None);
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            STATE.with(|s| s.borrow_mut().take());
        }
    }

    /// Draws the next frame into the window; once the splash has gone, destroys the window.
    ///
    /// # Safety
    /// On the splash thread, for its window.
    unsafe fn frame(hwnd: HWND) {
        let gone = STATE.with(|s| {
            let mut s = s.borrow_mut();
            let Some(st) = s.as_mut() else { return false };
            let (progress, what, done) = st.shared.feed();
            let Some(look) = st.anim.frame(st.start.elapsed().as_secs_f32(), progress, &what, done) else {
                st.shared.gone.store(true, Ordering::Release);
                return true;
            };
            let px = st.painter.paint(&look);
            // SAFETY: the DIB section's pixels, `size` of them, live as long as the state.
            let out = unsafe { std::slice::from_raw_parts_mut(st.bits, (st.size.0 * st.size.1) as usize) };
            for (o, p) in out.iter_mut().zip(px.data().chunks_exact(4)) {
                *o = (u32::from(p[3]) << 24) | (u32::from(p[0]) << 16) | (u32::from(p[1]) << 8) | u32::from(p[2]);
            }
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: (look.alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let size = SIZE { cx: st.size.0, cy: st.size.1 };
            let from = POINT { x: 0, y: 0 };
            // SAFETY: our own layered window and memory DC.
            unsafe { UpdateLayeredWindow(hwnd, std::ptr::null_mut(), std::ptr::null(), &size, st.dc, &from, 0, &blend, ULW_ALPHA) };
            false
        });
        if gone {
            // SAFETY: the splash thread's own window.
            unsafe { DestroyWindow(hwnd) };
        }
    }

    unsafe extern "system" fn splash_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        // SAFETY: called by the system on the splash thread for its own window.
        unsafe {
            match msg {
                WM_TIMER => {
                    frame(hwnd);
                    0
                }
                WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
                WM_DESTROY => {
                    KillTimer(hwnd, TIMER);
                    PostQuitMessage(0);
                    0
                }
                _ => DefWindowProcW(hwnd, msg, wp, lp),
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Linux: the splash in an X11 window of its own connection (XWayland under Wayland), animated on a thread of its own. A
// 32-bit (ARGB) window, so the compositor blends it over whatever is behind; override-redirect, so the window manager
// neither frames nor places it and keeps it above the managed windows; an empty input shape, so clicks go through.

#[cfg(target_os = "linux")]
mod x11 {
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use x11rb::connection::{Connection, RequestConnection as _};
    use x11rb::protocol::randr::ConnectionExt as _;
    use x11rb::protocol::shape::{self, ConnectionExt as _};
    use x11rb::protocol::xproto::{
        AtomEnum, ClipOrdering, ColormapAlloc, ConnectionExt as _, CreateGCAux, CreateWindowAux, ImageFormat, ImageOrder, PropMode, VisualClass, WindowClass,
    };
    use x11rb::rust_connection::RustConnection;
    use x11rb::wrapper::ConnectionExt as _;

    use super::{device, Anim, Painter, Shared};

    /// Time between frames.
    const FRAME: Duration = Duration::from_millis(16);

    /// The splash thread (it closes the window once `Shared::closed` is set).
    pub struct Window;

    impl Window {
        /// Nothing to do: the thread sees `Shared::closed` at its next frame and closes the window.
        pub fn close(&self) {}
    }

    pub fn open(shared: Arc<Shared>) -> Option<Window> {
        if !std::env::var_os("DISPLAY").is_some_and(|d| !d.is_empty()) {
            return None;
        }
        std::thread::Builder::new()
            .name("splash".into())
            .spawn(move || {
                if run(&shared).is_none() {
                    // No splash after all (no X server, no 32-bit visual): startup goes on without it.
                    shared.gone.store(true, Ordering::Release);
                }
            })
            .ok()?;
        Some(Window)
    }

    /// Makes the window, then animates it until it is closed or has gone.
    fn run(shared: &Shared) -> Option<()> {
        let (c, screen_num) = RustConnection::connect(None).ok()?;
        let screen = &c.setup().roots[screen_num];
        let root = screen.root;
        // A 32-bit TrueColor visual: per-pixel alpha.
        let visual = screen
            .allowed_depths
            .iter()
            .filter(|d| d.depth == 32)
            .flat_map(|d| d.visuals.iter())
            .find(|v| v.class == VisualClass::TRUE_COLOR)?
            .visual_id;
        // The UI scale: the X resources' `Xft.dpi` / 96, or Main GUI scaling's (spec: ui/ui-scaling.md §1).
        let dpi = x11rb::resource_manager::new_from_default(&c).ok().and_then(|db| db.get_value::<u32>("Xft.dpi", "").ok().flatten());
        let scale = crate::gui_scale(dpi.map_or(1.0, |d| f64::from(d) / 96.0)) as f32;
        let mut painter = Painter::new(scale)?;
        let (w, h) = device(scale);
        // The primary monitor (RandR), else the first, else the whole screen.
        let (mx, my, mw, mh) = c
            .randr_get_monitors(root, true)
            .ok()
            .and_then(|r| r.reply().ok())
            .and_then(|r| r.monitors.iter().find(|m| m.primary).or(r.monitors.first()).map(|m| (m.x, m.y, m.width, m.height)))
            .unwrap_or((0, 0, screen.width_in_pixels, screen.height_in_pixels));
        let x = i32::from(mx) + (i32::from(mw) - w as i32) / 2;
        let y = i32::from(my) + (i32::from(mh) - h as i32) / 2;

        let colormap = c.generate_id().ok()?;
        c.create_colormap(ColormapAlloc::NONE, colormap, root, visual).ok()?;
        let win = c.generate_id().ok()?;
        c.create_window(
            32,
            win,
            root,
            x as i16,
            y as i16,
            w as u16,
            h as u16,
            0,
            WindowClass::INPUT_OUTPUT,
            visual,
            &CreateWindowAux::new().background_pixel(0).border_pixel(0).colormap(colormap).override_redirect(1),
        )
        .ok()?;
        let atom = |name: &[u8]| c.intern_atom(false, name).ok()?.reply().ok().map(|r| r.atom);
        if let (Some(ty), Some(splash)) = (atom(b"_NET_WM_WINDOW_TYPE"), atom(b"_NET_WM_WINDOW_TYPE_SPLASH")) {
            let _ = c.change_property32(PropMode::REPLACE, win, ty, AtomEnum::ATOM, &[splash]);
        }
        let _ = c.change_property8(PropMode::REPLACE, win, AtomEnum::WM_NAME, AtomEnum::STRING, b"Skell Studio");
        let _ = c.change_property8(PropMode::REPLACE, win, AtomEnum::WM_CLASS, AtomEnum::STRING, b"lemonstudio\0LemonStudio\0");
        // No input region: clicks go through to what is behind.
        let _ = c.shape_rectangles(shape::SO::SET, shape::SK::INPUT, ClipOrdering::UNSORTED, win, 0, 0, &[]);
        let gc = c.generate_id().ok()?;
        c.create_gc(gc, win, &CreateGCAux::new()).ok()?;

        // Rows per PutImage, within the server's request size.
        let stride = w as usize * 4;
        let max = c.maximum_request_bytes().saturating_sub(64);
        let band = (max / stride).clamp(1, h as usize);
        let lsb = c.setup().image_byte_order == ImageOrder::LSB_FIRST;
        let mut out = vec![0u8; stride * h as usize];

        let mut anim = Anim::default();
        let start = Instant::now();
        let mut mapped = false;
        while !shared.closed.load(Ordering::SeqCst) {
            let next = Instant::now() + FRAME;
            let (progress, what, done) = shared.feed();
            let Some(look) = anim.frame(start.elapsed().as_secs_f32(), progress, &what, done) else {
                shared.gone.store(true, Ordering::Release);
                break;
            };
            // Premultiplied RGBA to the server's 32-bit ARGB, the window's opacity multiplied in.
            let k = (look.alpha.clamp(0.0, 1.0) * 256.0) as u32;
            for (o, p) in out.chunks_exact_mut(4).zip(painter.paint(&look).data().chunks_exact(4)) {
                let [r, g, b, a] = [p[0], p[1], p[2], p[3]].map(|v| ((u32::from(v) * k) >> 8) as u8);
                o.copy_from_slice(&if lsb { [b, g, r, a] } else { [a, r, g, b] });
            }
            for (i, rows) in out.chunks(band * stride).enumerate() {
                let _ = c.put_image(ImageFormat::Z_PIXMAP, win, gc, w as u16, (rows.len() / stride) as u16, 0, (i * band) as i16, 0, 32, rows);
            }
            if !mapped {
                // The first frame before the window shows.
                let _ = c.map_window(win);
                mapped = true;
            }
            let _ = c.flush();
            // Errors (the server's) are of no consequence to the splash.
            while let Ok(Some(_)) = c.poll_for_event() {}
            std::thread::sleep(next.saturating_duration_since(Instant::now()));
        }
        let _ = c.destroy_window(win);
        let _ = c.free_colormap(colormap);
        let _ = c.flush();
        Some(())
    }
}

// ---------------------------------------------------------------------------------------------------------------
// macOS: the splash in a process of its own (`lemonstudio --splash`). AppKit puts no window of a process on the
// screen before its main thread has launched it, and the app's main thread is busy starting up until just before the
// main window shows; the splash process launches at once and animates on its own main thread, from what the app writes
// down its standard input. A borderless, clear window at the status level, whose layer it fills.

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::c_void;
    use std::io::{BufRead, Write};
    use std::process::{Child, ChildStdin, Command, Stdio};
    use std::ptr::NonNull;
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, Mutex, MutexGuard};
    use std::time::{Duration, Instant};

    use objc2::rc::Retained;
    use objc2::{MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{
        NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSColor, NSScreen, NSWindow, NSWindowAnimationBehavior, NSWindowCollectionBehavior,
        NSWindowStyleMask,
    };
    use objc2_core_foundation::{kCFRunLoopDefaultMode, CFRetained, CFRunLoop};
    use objc2_core_graphics::{
        CGBitmapInfo, CGColorRenderingIntent, CGColorSpace, CGDataProvider, CGImage, CGImageAlphaInfo, CGImageByteOrderInfo, CGImageComponentInfo, CGImagePixelFormatInfo,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    use objc2_quartz_core::{CALayer, CATransaction};

    use super::{device, Anim, Feed, Painter, Shared};

    /// The argument that starts the splash process.
    pub const ARG: &str = "--splash";
    /// Above every other window, the Dock and floating panels included (`NSStatusWindowLevel`).
    const LEVEL: isize = 25;
    /// Time between frames.
    const FRAME: Duration = Duration::from_millis(16);

    fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
        m.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The splash process, and the pipe startup's progress goes down.
    pub struct Window {
        child: Mutex<Child>,
        pipe: Mutex<Option<ChildStdin>>,
    }

    pub fn open(_shared: Arc<Shared>) -> Option<Window> {
        let mut child = Command::new(std::env::current_exe().ok()?).arg(ARG).stdin(Stdio::piped()).stdout(Stdio::null()).spawn().ok()?;
        let pipe = child.stdin.take();
        Some(Window { child: Mutex::new(child), pipe: Mutex::new(pipe) })
    }

    impl Window {
        /// Tells the splash process where startup is: a line "<progress> <step>", or "done".
        pub fn tell(&self, feed: &Feed) {
            let mut pipe = lock(&self.pipe);
            let Some(p) = pipe.as_mut() else { return };
            let line = if feed.done { "done\n".to_string() } else { format!("{} {}\n", feed.progress, feed.step) };
            if p.write_all(line.as_bytes()).is_err() {
                *pipe = None;
            }
        }

        /// Whether the splash process has ended: the splash finished and faded away.
        pub fn gone(&self) -> bool {
            matches!(lock(&self.child).try_wait(), Ok(Some(_)))
        }

        /// Ends the splash process: its input closing ends it, and it is ended in any case.
        pub fn close(&self) {
            lock(&self.pipe).take();
            let mut child = lock(&self.child);
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// The splash process (`ARG`): shows the splash until it has gone, or the app has closed the pipe (dropped the
    /// splash, or ended). Never returns.
    pub fn child_main() -> ! {
        let shared = Arc::new(Shared::default());
        let feed = shared.clone();
        std::thread::spawn(move || {
            for line in std::io::stdin().lock().lines() {
                let Ok(line) = line else { break };
                let mut f = lock(&feed.feed);
                if line == "done" {
                    f.done = true;
                } else if let Some((p, what)) = line.split_once(' ') {
                    if let Ok(p) = p.parse::<f32>() {
                        f.progress = f.progress.max(p);
                        f.step = what.to_string();
                    }
                }
            }
            feed.closed.store(true, Ordering::SeqCst);
        });
        run(&shared);
        std::process::exit(0)
    }

    /// Shows the splash and animates it on this (the main) thread.
    fn run(shared: &Shared) -> Option<()> {
        let mtm = MainThreadMarker::new()?;
        // No Dock icon or menu bar of its own; launched now, so its window shows.
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        app.finishLaunching();
        // The primary screen (the one with the menu bar), in points from its bottom left.
        let screen = NSScreen::screens(mtm).firstObject()?;
        let (frame, backing) = (screen.frame(), screen.backingScaleFactor());
        let scale = crate::gui_scale(backing) as f32;
        let (w, h) = device(scale);
        let size = NSSize::new(f64::from(w) / backing, f64::from(h) / backing);
        let at = NSPoint::new(frame.origin.x + (frame.size.width - size.width) / 2.0, frame.origin.y + (frame.size.height - size.height) / 2.0);
        // SAFETY: a new borderless window, kept until the process ends (not released when closed).
        let window: Retained<NSWindow> = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(NSWindow::alloc(mtm), NSRect::new(at, size), NSWindowStyleMask::Borderless, NSBackingStoreType::Buffered, false)
        };
        unsafe { window.setReleasedWhenClosed(false) };
        window.setOpaque(false);
        window.setBackgroundColor(Some(&NSColor::clearColor()));
        window.setHasShadow(false);
        window.setIgnoresMouseEvents(true);
        window.setLevel(LEVEL);
        window.setAnimationBehavior(NSWindowAnimationBehavior::None);
        window.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        // A layer-hosting view: the frames go into its layer.
        let layer = CALayer::new();
        layer.setContentsScale(backing);
        let view = window.contentView()?;
        view.setLayer(Some(&layer));
        view.setWantsLayer(true);
        window.orderFrontRegardless();
        let mut painter = Painter::new(scale)?;
        let mut anim = Anim::default();
        let start = Instant::now();
        while !shared.closed.load(Ordering::SeqCst) {
            let next = Instant::now() + FRAME;
            let (progress, what, done) = shared.feed();
            let Some(look) = anim.frame(start.elapsed().as_secs_f32(), progress, &what, done) else { break };
            if let Some(img) = image(painter.paint(&look)) {
                CATransaction::begin();
                CATransaction::setDisableActions(true);
                // SAFETY: a CGImage is what a layer's contents may be.
                unsafe { layer.setContents(Some(img.as_ref())) };
                layer.setOpacity(look.alpha);
                CATransaction::commit();
            }
            // AppKit puts the frame on the screen as the run loop turns, until the next one is due.
            // SAFETY: CoreFoundation's own constant.
            CFRunLoop::run_in_mode(unsafe { kCFRunLoopDefaultMode }, next.saturating_duration_since(Instant::now()).as_secs_f64(), false);
            std::thread::sleep(next.saturating_duration_since(Instant::now()));
        }
        Some(())
    }

    /// `px` (premultiplied RGBA) as an image for the layer.
    fn image(px: &tiny_skia::Pixmap) -> Option<CFRetained<CGImage>> {
        unsafe extern "C-unwind" fn release(_info: *mut c_void, data: NonNull<c_void>, size: usize) {
            let slice = std::ptr::slice_from_raw_parts_mut(data.cast::<u8>().as_ptr(), size);
            // SAFETY: the boxed slice handed out below.
            drop(unsafe { Box::from_raw(slice) });
        }
        let (w, h) = (px.width() as usize, px.height() as usize);
        let bytes = px.data().to_vec().into_boxed_slice();
        let len = bytes.len();
        let data = Box::into_raw(bytes).cast::<c_void>();
        // SAFETY: `data` holds `len` bytes until `release` frees it.
        let provider = unsafe { CGDataProvider::with_data(std::ptr::null_mut(), data, len, Some(release)) }?;
        let space = CGColorSpace::new_device_rgb()?;
        let info = CGBitmapInfo(CGImageAlphaInfo::PremultipliedLast.0 | CGImageComponentInfo::Integer.0 | CGImageByteOrderInfo::OrderDefault.0 | CGImagePixelFormatInfo::Packed.0);
        unsafe { CGImage::new(w, h, 8, 32, w * 4, Some(&space), info, Some(&provider), std::ptr::null(), false, CGColorRenderingIntent::RenderingIntentDefault) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prefs::{check, find_check, set_check, SHOW_SPLASH, SILENT_STARTUP};

    /// "Show startup splash screen" (spec: ui/settings-choices.md "Miscellaneous: naming, detached plugins and
    /// startup"): on as found, a program setting; with it off the start opens no splash.
    #[test]
    fn the_splash_follows_its_check() {
        assert_eq!(find_check("Show startup splash screen").map(|c| c.key), Some(SHOW_SPLASH.key));
        assert!(wanted(), "on by default");
        set_check(&SHOW_SPLASH, false);
        assert!(!wanted());
        assert!(open().is_none(), "off: no splash");
        set_check(&SHOW_SPLASH, true);
        assert!(wanted());
    }

    /// The gauge follows startup's progress, never going back, under a status line that names the step; it fades and
    /// grows in, a quick startup still sees it fill for a moment, and then it tops up, it says "Ready"
    /// and fades away.
    #[test]
    fn the_gauge_follows_startup_then_goes() {
        let mut a = Anim::default();
        let first = a.frame(0.0, 0.0, "Reading settings", false).unwrap();
        assert_eq!((first.alpha, first.progress), (0.0, 0.0), "it fades in from nothing");
        assert!(first.scale < 1.0);
        let mut t = 0.0;
        let mut last = 0.0;
        // Startup steps on; the gauge eases after it, never past it.
        for (to, p) in [(0.4, 0.2), (1.5, 0.5)] {
            while t < to {
                t += 1.0 / 60.0;
                let l = a.frame(t, p, "Opening project", false).unwrap();
                assert!(l.progress >= last && l.progress <= p, "at {t}: {}", l.progress);
                last = l.progress;
            }
        }
        let l = a.frame(t, 0.3, "Opening project", false).unwrap();
        assert!((l.progress - 0.5).abs() < 0.01, "caught up, and not back: {}", l.progress);
        assert_eq!((l.alpha, l.scale), (1.0, 1.0));
        assert_eq!(l.caption, "Opening project · 50%");
        // Done: it tops up then fades away.
        let t0 = t;
        let mut seen_ready = false;
        let gone_at = loop {
            t += 1.0 / 60.0;
            match a.frame(t, 0.5, "Opening project", true) {
                Some(l) => {
                    if t - t0 >= TOP_UP + 0.02 {
                        assert!((l.progress - 1.0).abs() < 1e-6);
                        assert_eq!(l.caption, "Ready");
                        seen_ready = true;
                    }
                    assert!(t - t0 < FADE_OUT.1 + 0.02);
                }
                None => break t - t0,
            }
        };
        assert!(seen_ready);
        assert!((gone_at - FADE_OUT.1).abs() < 0.05, "gone after {gone_at} s");
        // A quick startup: done at once, the gauge fills all the way, and it finishes at the earliest.
        let mut b = Anim::default();
        let (mut t, mut l) = (0.0, b.frame(0.0, 0.2, "Reading settings", true).unwrap());
        while t + 1.0 / 60.0 < EARLIEST {
            t += 1.0 / 60.0;
            l = b.frame(t, 0.2, "Reading settings", true).unwrap();
        }
        assert!(b.done.is_none() && l.progress > 0.99, "filled, not finishing yet: {}", l.progress);
        b.frame(EARLIEST, 0.2, "Reading settings", true).unwrap();
        assert_eq!(b.done.map(|d| d.0), Some(EARLIEST));
        // A start that never says it is done does not keep the splash up for ever.
        let mut c = Anim::default();
        c.frame(LONGEST, 0.3, "Stuck", false).unwrap();
        assert!(c.done.is_some());
    }

    #[test]
    fn splash_preserves_clear_background_and_shows_loading_progress() {
        for scale in [1.0, 2.0] {
            let mut painter = Painter::new(scale).unwrap();
            let mut look = Look { alpha: 1.0, scale: 1.0, progress: 0.25, caption: "Loading project".into() };
            let first = painter.paint(&look).clone();
            let at = |p: &Pixmap, x: f32, y: f32| p.pixel((x * scale) as u32, (y * scale) as u32).unwrap();
            assert_eq!(at(&first, 0.0, 0.0).alpha(), 0);
            assert!(at(&first, 250.0, 180.0).alpha() > 0);
            assert!(first.pixels().iter().any(|p| p.alpha() == 255 && p.green() > 200 && p.red() > 100));
            look.progress = 1.0;
            let last = painter.paint(&look);
            assert!(at(last, 285.0, 410.0).green() > at(&first, 285.0, 410.0).green());
            if let Some(dir) = std::env::var_os("LEMON_SPLASH_PREVIEW") {
                let dir = std::path::PathBuf::from(dir);
                std::fs::create_dir_all(&dir).unwrap();
                last.save_png(dir.join(format!("skell-splash-{scale}.png"))).unwrap();
            }
        }
    }

    /// The Windows startup sound preference is stored and defaults to off.
    #[test]
    fn silent_startup_is_stored() {
        assert_eq!(find_check("Silent startup").map(|c| c.key), Some(SILENT_STARTUP.key));
        assert!(!check(&SILENT_STARTUP), "off by default");
        set_check(&SILENT_STARTUP, true);
        assert!(check(&SILENT_STARTUP));
        set_check(&SILENT_STARTUP, false);
    }
}

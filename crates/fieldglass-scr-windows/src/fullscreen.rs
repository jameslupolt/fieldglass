//! Fullscreen screensaver mode (`/s`).
//!
//! Native Win32 implementation. Creates a borderless popup window covering the
//! primary monitor, loads cached images via the `image` crate, and displays
//! them with crossfade transitions using GDI `AlphaBlend`.
//!
//! Input handling:
//! - Ignores the first few `WM_MOUSEMOVE` events after activation (to prevent
//!   immediate dismissal from the cursor's pre-existing position).
//! - Any subsequent mouse move, click, or keypress exits the screensaver.

use std::cell::RefCell;
use std::ffi::c_void;
use std::mem;
use std::path::{Path, PathBuf};
use std::ptr;
use std::slice;

use anyhow::{Context, Result};
use windows::core::w;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::System::RemoteDesktop::{
    WTSRegisterSessionNotification, WTSUnRegisterSessionNotification,
};
use windows::Win32::UI::HiDpi::{SetProcessDpiAwareness, PROCESS_SYSTEM_DPI_AWARE};
use windows::Win32::UI::WindowsAndMessaging::*;

use fieldglass_core::config::Settings;
use fieldglass_core::types::AspectRatioMode;
use fieldglass_core::CacheManager;
use fieldglass_core::CachedPhoto;
use fieldglass_core::PhotoLicense;

thread_local! {
    static WINDOW_HWNDS: RefCell<Vec<HWND>> = const { RefCell::new(Vec::new()) };
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Timer ID for the photo display interval.
const TIMER_PHOTO: usize = 1;
/// Timer ID for the crossfade animation ticks.
const TIMER_CROSSFADE: usize = 2;

/// Milliseconds between crossfade steps (~33 FPS).
const CROSSFADE_INTERVAL_MS: u32 = 30;
/// Total number of crossfade steps. 50 × 30 ms = 1.5 s transition.
const CROSSFADE_STEPS: u32 = 50;
/// Bottom-left margin in pixels for the overlay.
const OVERLAY_MARGIN: i32 = 20;
/// Padding inside the overlay background rectangle.
const OVERLAY_PADDING: i32 = 12;
/// Background opacity for the attribution overlay (0–255).
const OVERLAY_BG_ALPHA: u32 = 180;

/// `WM_WTSSESSION_CHANGE` message (0x02B1). Sent after `WTSRegisterSessionNotification`.
const WM_WTSSESSION_CHANGE: u32 = 0x02B1;
/// Session has been locked (Ctrl+L, screen timeout, etc.).
const WTS_SESSION_LOCK: usize = 0x7;
/// Console has been disconnected (e.g. Remote Desktop switch).
const WTS_CONSOLE_DISCONNECT: usize = 0x2;

// ---------------------------------------------------------------------------
// GDI image wrapper
// ---------------------------------------------------------------------------

/// A decoded image held in a GDI memory DC, ready for `StretchBlt`/`AlphaBlend`.
struct DibImage {
    hdc: HDC,
    hbitmap: HBITMAP,
    prev_obj: HGDIOBJ,
    width: i32,
    height: i32,
}

impl Drop for DibImage {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.hdc, self.prev_obj);
            let _ = DeleteObject(self.hbitmap.into());
            let _ = DeleteDC(self.hdc);
        }
    }
}

// ---------------------------------------------------------------------------
// Attribution overlay
// ---------------------------------------------------------------------------

/// Text lines for the attribution overlay.
#[derive(Clone)]
struct OverlayInfo {
    /// "© Creator Name · CC BY-NC 4.0 · iNaturalist.org"
    line1: String,
    /// "Common Name (Scientific name)" or just "Scientific name"
    line2: String,
    /// Location, if available.
    line3: Option<String>,
}

impl OverlayInfo {
    fn from_cached_photo(photo: &CachedPhoto) -> Self {
        let line1 = format!("© {} · {} · iNaturalist.org", photo.creator_name, photo.license_display);
        let line2 = match &photo.common_name {
            Some(common) => format!("{} ({})", common, photo.scientific_name),
            None => photo.scientific_name.clone(),
        };
        OverlayInfo {
            line1,
            line2,
            line3: photo.place_name.clone(),
        }
    }
}

/// Pre-rendered attribution overlay as a 32bpp BGRA DIB, ready for `AlphaBlend`.
struct OverlayDib {
    hdc: HDC,
    hbitmap: HBITMAP,
    prev_obj: HGDIOBJ,
    width: i32,
    height: i32,
}

impl Drop for OverlayDib {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.hdc, self.prev_obj);
            let _ = DeleteObject(self.hbitmap.into());
            let _ = DeleteDC(self.hdc);
        }
    }
}

/// A decoded photo paired with its pre-rendered attribution overlay.
struct DisplayImage {
    dib: DibImage,
    overlay: OverlayDib,
    license_code: String,
}

/// An entry in the photo display queue: path + overlay text.
#[derive(Clone)]
struct PhotoEntry {
    path: PathBuf,
    overlay_info: OverlayInfo,
    license_code: String,
}

// ---------------------------------------------------------------------------
// Screensaver state
// ---------------------------------------------------------------------------

/// All mutable state for the screensaver, stored via `GWLP_USERDATA`.
struct State {
    /// Absolute paths + overlay metadata for cached photos (pre-shuffled).
    photos: Vec<PhotoEntry>,
    /// Index of the currently displayed photo.
    current_index: usize,
    screen_w: i32,
    screen_h: i32,

    // -- Images --
    /// Currently visible image.
    current: Option<DisplayImage>,
    /// Incoming image during a crossfade transition.
    next: Option<DisplayImage>,

    // -- Back buffer --
    back_dc: HDC,
    back_bmp: HBITMAP,
    back_prev: HGDIOBJ,

    // -- Transition --
    transitioning: bool,
    transition_step: u32,

    // -- Settings --
    duration_ms: u32,
    aspect_ratio_mode: AspectRatioMode,
    font: HFONT,

    // -- Input --
    /// Counts `WM_MOUSEMOVE` events; first few are ignored.
    mouse_moves: u32,
}

impl Drop for State {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.back_dc, self.back_prev);
            let _ = DeleteObject(self.back_bmp.into());
            let _ = DeleteDC(self.back_dc);
            let _ = DeleteObject(self.font.into());
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Run the fullscreen screensaver.
///
/// 1. Load settings and open the image cache.
/// 2. Create a borderless popup window covering the primary monitor.
/// 3. Load the first cached photo and begin cycling via `WM_TIMER`.
/// 4. Exit on any user input (mouse / keyboard).
pub fn run() -> Result<()> {
    let instance_mutex = unsafe { CreateMutexW(None, true, w!("FieldGlass.Screensaver.Singleton")) }
        .context("CreateMutexW failed")?;
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe {
            let _ = CloseHandle(instance_mutex);
        }
        tracing::warn!("Another FieldGlass screensaver instance is already running");
        return Ok(());
    }

    // Load settings
    let settings_path = Settings::default_path().context("Failed to determine settings path")?;
    let settings = Settings::load(&settings_path).context("Failed to load settings")?;

    // Open cache and get a shuffled display queue
    let cache = CacheManager::new().context("Failed to open cache")?;
    let display_queue = cache
        .get_display_queue()
        .context("Failed to get display queue")?;

    if display_queue.is_empty() {
        tracing::warn!("No cached photos — exiting screensaver");
        return Ok(());
    }

    // Resolve to absolute paths, keeping only files that exist, with overlay metadata
    let photos: Vec<PhotoEntry> = display_queue
        .iter()
        .filter_map(|p| {
            let path = cache.storage().absolute_path_for(&p.file_path);
            if path.exists() {
                Some(PhotoEntry {
                    path,
                    overlay_info: OverlayInfo::from_cached_photo(p),
                    license_code: p.license_code.clone(),
                })
            } else {
                None
            }
        })
        .collect();

    if photos.is_empty() {
        tracing::warn!("No cached photo files found on disk — exiting");
        return Ok(());
    }

    let duration_ms = settings.photo_duration_secs.max(1) * 1000;

    // --- Win32 setup ----------------------------------------------------------

    unsafe {
        let _ = SetProcessDpiAwareness(PROCESS_SYSTEM_DPI_AWARE);
    }

    let hinstance: HINSTANCE =
        unsafe { GetModuleHandleW(None).context("GetModuleHandleW")?.into() };

    // Register window class
    let wc = WNDCLASSEXW {
        cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wndproc),
        hInstance: hinstance,
        hbrBackground: unsafe { CreateSolidBrush(COLORREF(0)) },
        lpszClassName: w!("FieldGlass"),
        ..Default::default()
    };

    if unsafe { RegisterClassExW(&wc) } == 0 {
        anyhow::bail!("RegisterClassExW failed");
    }

    let monitors = enumerate_monitors();
    if monitors.is_empty() {
        tracing::warn!("No monitors found — exiting screensaver");
        return Ok(());
    }

    WINDOW_HWNDS.with(|windows| windows.borrow_mut().clear());

    unsafe {
        ShowCursor(false);
    }

    for (monitor_index, monitor_rect) in monitors.iter().enumerate() {
        let screen_w = (monitor_rect.right - monitor_rect.left).max(1);
        let screen_h = (monitor_rect.bottom - monitor_rect.top).max(1);
        let font = create_overlay_font(screen_h);
        let monitor_photos = monitor_photo_queue(&photos, monitor_index, monitors.len());

        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST,
                w!("FieldGlass"),
                w!("Field Glass"),
                WS_POPUP | WS_VISIBLE,
                monitor_rect.left,
                monitor_rect.top,
                screen_w,
                screen_h,
                None, // parent
                None, // menu
                Some(hinstance),
                None, // lpParam
            )
            .context("CreateWindowExW failed")?
        };

        register_window(hwnd);

        // Register for session change notifications so we exit on screen lock
        unsafe {
            let _ = WTSRegisterSessionNotification(hwnd, 0); // NOTIFY_FOR_THIS_SESSION
        }

        let screen_dc = unsafe { GetDC(Some(hwnd)) };
        let back_dc = unsafe { CreateCompatibleDC(Some(screen_dc)) };
        let back_bmp = unsafe { CreateCompatibleBitmap(screen_dc, screen_w, screen_h) };
        let back_prev = unsafe { SelectObject(back_dc, back_bmp.into()) };

        let mut first_image = None;
        let mut start_index = 0;
        for (i, entry) in monitor_photos.iter().enumerate() {
            match load_display_image(
                screen_dc,
                &entry.path,
                &entry.overlay_info,
                &entry.license_code,
                font,
                screen_w,
                screen_h,
            ) {
                Ok(img) => {
                    first_image = Some(img);
                    start_index = i;
                    break;
                }
                Err(e) => {
                    tracing::debug!(path = %entry.path.display(), error = %e, "Skipping photo");
                }
            }
        }

        unsafe {
            ReleaseDC(Some(hwnd), screen_dc);
        }

        if first_image.is_none() {
            tracing::warn!("Could not decode any cached photo — exiting");
            unsafe {
                dismiss_all_windows();
            }
            return Ok(());
        }

        let state = Box::new(State {
            photos: monitor_photos,
            current_index: start_index,
            screen_w,
            screen_h,
            current: first_image,
            next: None,
            back_dc,
            back_bmp,
            back_prev,
            transitioning: false,
            transition_step: 0,
            duration_ms,
            aspect_ratio_mode: settings.aspect_ratio_mode,
            font,
            mouse_moves: 0,
        });

        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
            let _ = InvalidateRect(Some(hwnd), None, false);
            SetTimer(Some(hwnd), TIMER_PHOTO, duration_ms, None);
        }
    }

    tracing::info!(
        monitors = monitors.len(),
        duration_secs = settings.photo_duration_secs,
        photos = display_queue.len(),
        "Screensaver started"
    );

    // --- Message loop ----------------------------------------------------------

    let mut msg = MSG::default();
    loop {
        let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        // 0 = WM_QUIT, -1 = error
        if ret.0 <= 0 {
            break;
        }
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    // Restore cursor
    unsafe {
        ShowCursor(true);
    }

    unsafe {
        let _ = CloseHandle(instance_mutex);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Window procedure
// ---------------------------------------------------------------------------

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_DESTROY {
            let ptr = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut State;
            if !ptr.is_null() {
                drop(Box::from_raw(ptr));
            }
            let _ = WTSUnRegisterSessionNotification(hwnd);
            unregister_window(hwnd);
            let has_windows = WINDOW_HWNDS.with(|windows| !windows.borrow().is_empty());
            if !has_windows {
                PostQuitMessage(0);
            }
            return LRESULT(0);
        }

        // Retrieve per-window state. Null during WM_CREATE and friends — delegate
        // those to the default handler.
        let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
        if state_ptr.is_null() {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        let state = &mut *state_ptr;

        match msg {
            // -- Painting -----------------------------------------------------------
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                paint(state, hdc);
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }

            // -- Timers --------------------------------------------------------------
            WM_TIMER => {
                match wparam.0 {
                    TIMER_PHOTO => start_transition(hwnd, state),
                    TIMER_CROSSFADE => advance_transition(hwnd, state),
                    _ => {}
                }
                LRESULT(0)
            }

            // -- Dismissal -----------------------------------------------------------
            WM_MOUSEMOVE => {
                state.mouse_moves = state.mouse_moves.saturating_add(1);
                if state.mouse_moves > 5 {
                    tracing::debug!("Screensaver dismissed (mouse move)");
                    dismiss_all_windows();
                }
                LRESULT(0)
            }

            WM_KEYDOWN | WM_SYSKEYDOWN | WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN => {
                tracing::debug!("Screensaver dismissed (key/click)");
                dismiss_all_windows();
                LRESULT(0)
            }

            WM_CLOSE => {
                tracing::debug!("Screensaver dismissed (close)");
                dismiss_all_windows();
                LRESULT(0)
            }

            WM_ACTIVATEAPP => {
                if wparam.0 == 0 {
                    tracing::debug!("Screensaver dismissed (app deactivated)");
                    dismiss_all_windows();
                }
                LRESULT(0)
            }

            WM_QUERYENDSESSION | WM_ENDSESSION => {
                dismiss_all_windows();
                LRESULT(1)
            }

            WM_WTSSESSION_CHANGE => {
                if wparam.0 == WTS_SESSION_LOCK || wparam.0 == WTS_CONSOLE_DISCONNECT {
                    tracing::debug!("Screensaver dismissed (session lock/disconnect)");
                    dismiss_all_windows();
                }
                LRESULT(0)
            }

            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

// ---------------------------------------------------------------------------
// Painting
// ---------------------------------------------------------------------------

/// Double-buffered paint: compose into `state.back_dc`, then `BitBlt` to the
/// window DC in one
///
/// During a crossfade the current image is drawn first, then the next image
/// is alpha-blended on top with increasing opacity.
unsafe fn paint(state: &State, hdc: HDC) {
    unsafe {
        let full_rect = RECT {
            left: 0,
            top: 0,
            right: state.screen_w,
            bottom: state.screen_h,
        };

        // Clear to black
        let black = CreateSolidBrush(COLORREF(0));
        FillRect(state.back_dc, &full_rect, black);
        let _ = DeleteObject(black.into());

        let _ = SetStretchBltMode(state.back_dc, HALFTONE);

        if let Some(ref current) = state.current {
            let (dest, src) = photo_rects(
                current.dib.width,
                current.dib.height,
                state.screen_w,
                state.screen_h,
                state.aspect_ratio_mode,
                &current.license_code,
            );
            let dw = dest.right - dest.left;
            let dh = dest.bottom - dest.top;
            let sw = src.right - src.left;
            let sh = src.bottom - src.top;

            // Draw current photo (full opacity)
            let _ = StretchBlt(
                state.back_dc,
                dest.left,
                dest.top,
                dw,
                dh,
                Some(current.dib.hdc),
                src.left,
                src.top,
                sw,
                sh,
                SRCCOPY,
            );

            // Crossfade: blend next photo on top with increasing alpha
            if state.transitioning
                && let Some(ref next) = state.next
            {
                    let alpha =
                        (state.transition_step * 255 / CROSSFADE_STEPS).min(255) as u8;
                    let (nd, ns) = photo_rects(
                        next.dib.width,
                        next.dib.height,
                        state.screen_w,
                        state.screen_h,
                        state.aspect_ratio_mode,
                        &next.license_code,
                    );
                    let nw = nd.right - nd.left;
                    let nh = nd.bottom - nd.top;
                    let nsw = ns.right - ns.left;
                    let nsh = ns.bottom - ns.top;

                    let blend = BLENDFUNCTION {
                        BlendOp: 0, // AC_SRC_OVER
                        BlendFlags: 0,
                        SourceConstantAlpha: alpha,
                        AlphaFormat: 0,
                    };

                    let _ = AlphaBlend(
                        state.back_dc,
                        nd.left,
                        nd.top,
                        nw,
                        nh,
                        next.dib.hdc,
                        ns.left,
                        ns.top,
                        nsw,
                        nsh,
                        blend,
                    );
            }
        }

        // -- Attribution overlay --
        if state.transitioning {
            let alpha = ((state.transition_step * 255) / CROSSFADE_STEPS).min(255) as u8;
            if let Some(ref current) = state.current {
                blit_overlay(
                    state.back_dc,
                    &current.overlay,
                    state.screen_w,
                    state.screen_h,
                    255u8.saturating_sub(alpha),
                );
            }
            if let Some(ref next) = state.next {
                blit_overlay(
                    state.back_dc,
                    &next.overlay,
                    state.screen_w,
                    state.screen_h,
                    alpha,
                );
            }
        } else if let Some(ref current) = state.current {
            blit_overlay(
                state.back_dc,
                &current.overlay,
                state.screen_w,
                state.screen_h,
                255,
            );
        }

        // Flip back buffer to screen
        let _ = BitBlt(
            hdc,
            0,
            0,
            state.screen_w,
            state.screen_h,
            Some(state.back_dc),
            0,
            0,
            SRCCOPY,
        );
    }
}

// ---------------------------------------------------------------------------
// Transition logic
// ---------------------------------------------------------------------------

/// Begin a crossfade transition to the next photo.
unsafe fn start_transition(hwnd: HWND, state: &mut State) {
    unsafe {
        let next_index = (state.current_index + 1) % state.photos.len();

        let screen_dc = GetDC(Some(hwnd));
        let entry = &state.photos[next_index];
        let result = load_display_image(
            screen_dc,
            &entry.path,
            &entry.overlay_info,
            &entry.license_code,
            state.font,
            state.screen_w,
            state.screen_h,
        );
        ReleaseDC(Some(hwnd), screen_dc);

        match result {
            Ok(next_img) => {
                state.next = Some(next_img);
                state.transition_step = 0;
                state.transitioning = true;

                let _ = KillTimer(Some(hwnd), TIMER_PHOTO);
                SetTimer(Some(hwnd), TIMER_CROSSFADE, CROSSFADE_INTERVAL_MS, None);
            }
            Err(e) => {
                // Can't decode this photo — skip it and let the next timer tick
                // try the one after.
                tracing::debug!(
                    index = next_index,
                    error = %e,
                    "Skipping undecodable photo"
                );
                state.current_index = next_index;
            }
        }
    }
}

/// Advance the crossfade one step. When done, swap current/next and restart
/// the photo timer.
unsafe fn advance_transition(hwnd: HWND, state: &mut State) {
    unsafe {
        state.transition_step += 1;

        if state.transition_step >= CROSSFADE_STEPS {
            // Transition complete — promote next → current
            state.transitioning = false;
            state.transition_step = 0;
            state.current = state.next.take();
            state.current_index = (state.current_index + 1) % state.photos.len();

            let _ = KillTimer(Some(hwnd), TIMER_CROSSFADE);
            SetTimer(Some(hwnd), TIMER_PHOTO, state.duration_ms, None);
        }

        // Repaint
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
}

// ---------------------------------------------------------------------------
// Image loading
// ---------------------------------------------------------------------------

/// Decode an image file from disk and load it into a GDI memory DC as a
/// 32-bit top-down DIB, ready for `StretchBlt` / `AlphaBlend`.
fn load_photo_dib(screen_dc: HDC, path: &Path) -> Result<DibImage> {
    let img =
        image::open(path).with_context(|| format!("Failed to open image: {}", path.display()))?;
    let rgb = img.to_rgb8();
    let w = rgb.width() as i32;
    let h = rgb.height() as i32;

    if w == 0 || h == 0 {
        anyhow::bail!("Image has zero dimensions: {w}×{h}");
    }

    unsafe {
        let mem_dc = CreateCompatibleDC(Some(screen_dc));

        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // negative = top-down scanline order
                biPlanes: 1,
                biBitCount: 32,
                biCompression: 0, // BI_RGB
                biSizeImage: (w * h * 4) as u32,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut bits: *mut c_void = ptr::null_mut();
        let hbitmap = CreateDIBSection(Some(mem_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0)
            .context("CreateDIBSection failed")?;

        // Convert RGB → BGRX (Windows native pixel order, no per-pixel alpha)
        let raw = rgb.as_raw();
        let pixel_count = (w * h) as usize;
        let dst = slice::from_raw_parts_mut(bits as *mut u8, pixel_count * 4);

        for i in 0..pixel_count {
            let s = i * 3;
            let d = i * 4;
            dst[d] = raw[s + 2]; // B
            dst[d + 1] = raw[s + 1]; // G
            dst[d + 2] = raw[s]; // R
            dst[d + 3] = 0xFF; // X (unused, opaque)
        }

        let prev_obj = SelectObject(mem_dc, hbitmap.into());

        Ok(DibImage {
            hdc: mem_dc,
            hbitmap,
            prev_obj,
            width: w,
            height: h,
        })
    }
}

/// Create a GDI font for the attribution overlay, sized relative to screen height.
fn create_overlay_font(screen_h: i32) -> HFONT {
    // ~1.5% of screen height, minimum 16px
    let font_height = -((screen_h as f64 * 0.015) as i32).max(16);
    let mut face_name = [0u16; 32];
    for (i, ch) in "Segoe UI".encode_utf16().enumerate() {
        if i >= 31 {
            break;
        }
        face_name[i] = ch;
    }
    let lf = LOGFONTW {
        lfHeight: font_height,
        lfWeight: 400, // FW_NORMAL
        lfQuality: FONT_QUALITY(5), // CLEARTYPE_QUALITY
        lfFaceName: face_name,
        ..Default::default()
    };
    unsafe { CreateFontIndirectW(&lf) }
}

/// Render the attribution overlay text into a 32bpp BGRA DIB with per-pixel alpha.
///
/// The result is a pre-multiplied-alpha surface: semi-transparent black background
/// with opaque white text, ready to be composited via `AlphaBlend` with `AC_SRC_ALPHA`.
unsafe fn render_overlay_dib(
    screen_dc: HDC,
    info: &OverlayInfo,
    font: HFONT,
    screen_w: i32,
    _screen_h: i32,
) -> OverlayDib {
    unsafe {
        // Build multi-line text
        let mut text = info.line1.clone();
        text.push('\n');
        text.push_str(&info.line2);
        if let Some(ref loc) = info.line3 {
            text.push('\n');
            text.push_str(loc);
        }
        let mut text_wide: Vec<u16> = text.encode_utf16().collect();

        // Measure text extents
        let measure_dc = CreateCompatibleDC(Some(screen_dc));
        let old_font = SelectObject(measure_dc, font.into());
        let max_width = screen_w / 2; // cap overlay width at half the screen
        let mut measure_rect = RECT {
            left: 0,
            top: 0,
            right: max_width,
            bottom: 0,
        };
        DrawTextW(
            measure_dc,
            &mut text_wide,
            &mut measure_rect,
            DT_CALCRECT | DT_LEFT | DT_WORDBREAK,
        );
        SelectObject(measure_dc, old_font);
        let _ = DeleteDC(measure_dc);

        let overlay_w = measure_rect.right + OVERLAY_PADDING * 2;
        let overlay_h = measure_rect.bottom + OVERLAY_PADDING * 2;

        // Create 32bpp top-down DIB
        let overlay_dc = CreateCompatibleDC(Some(screen_dc));
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: overlay_w,
                biHeight: -overlay_h, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: 0, // BI_RGB
                biSizeImage: (overlay_w * overlay_h * 4) as u32,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut c_void = ptr::null_mut();
        // If DIB creation fails, return a zero-size overlay (harmless no-op when blitted)
        let hbitmap =
            match CreateDIBSection(Some(overlay_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
                Ok(bmp) => bmp,
                Err(_) => {
                    let _ = DeleteDC(overlay_dc);
                    return OverlayDib {
                        hdc: HDC::default(),
                        hbitmap: HBITMAP::default(),
                        prev_obj: HGDIOBJ::default(),
                        width: 0,
                        height: 0,
                    };
                }
            };

        let prev_obj = SelectObject(overlay_dc, hbitmap.into());

        // Fill every pixel with semi-transparent black: BGRA(0, 0, 0, OVERLAY_BG_ALPHA)
        let pixel_count = (overlay_w * overlay_h) as usize;
        let pixels = slice::from_raw_parts_mut(bits as *mut u32, pixel_count);
        let bg_pixel = OVERLAY_BG_ALPHA << 24; // alpha in high byte, RGB = 0
        for p in pixels.iter_mut() {
            *p = bg_pixel;
        }

        // Draw white text (GDI writes RGB but does NOT touch alpha channel)
        let old_font2 = SelectObject(overlay_dc, font.into());
        SetTextColor(overlay_dc, COLORREF(0x00FFFFFF));
        SetBkMode(overlay_dc, TRANSPARENT);
        let mut draw_rect = RECT {
            left: OVERLAY_PADDING,
            top: OVERLAY_PADDING,
            right: overlay_w - OVERLAY_PADDING,
            bottom: overlay_h - OVERLAY_PADDING,
        };
        DrawTextW(
            overlay_dc,
            &mut text_wide,
            &mut draw_rect,
            DT_LEFT | DT_WORDBREAK,
        );

        // Fix alpha: GDI left alpha=OVERLAY_BG_ALPHA on text pixels.
        // Any pixel where RGB != 0 is text → set alpha to 255 (fully opaque).
        for p in pixels.iter_mut() {
            if *p & 0x00FFFFFF != 0 {
                *p = (*p & 0x00FFFFFF) | 0xFF000000;
            }
        }

        SelectObject(overlay_dc, old_font2);

        OverlayDib {
            hdc: overlay_dc,
            hbitmap,
            prev_obj,
            width: overlay_w,
            height: overlay_h,
        }
    }
}

/// Composite a pre-rendered overlay onto the back buffer at the bottom-left.
unsafe fn blit_overlay(
    back_dc: HDC,
    overlay: &OverlayDib,
    _screen_w: i32,
    screen_h: i32,
    alpha: u8,
) {
    unsafe {
        if alpha == 0 || overlay.width == 0 || overlay.height == 0 {
            return;
        }
        let x = OVERLAY_MARGIN;
        let y = screen_h - overlay.height - OVERLAY_MARGIN;
        let blend = BLENDFUNCTION {
            BlendOp: 0, // AC_SRC_OVER
            BlendFlags: 0,
            SourceConstantAlpha: alpha,
            AlphaFormat: 1, // AC_SRC_ALPHA — use per-pixel alpha from the DIB
        };
        let _ = AlphaBlend(
            back_dc,
            x,
            y,
            overlay.width,
            overlay.height,
            overlay.hdc,
            0,
            0,
            overlay.width,
            overlay.height,
            blend,
        );
    }
}

/// Load a photo from disk and pre-render its attribution overlay.
fn load_display_image(
    screen_dc: HDC,
    path: &Path,
    overlay_info: &OverlayInfo,
    license_code: &str,
    font: HFONT,
    screen_w: i32,
    screen_h: i32,
) -> Result<DisplayImage> {
    let dib = load_photo_dib(screen_dc, path)?;
    let overlay = unsafe { render_overlay_dib(screen_dc, overlay_info, font, screen_w, screen_h) };
    Ok(DisplayImage {
        dib,
        overlay,
        license_code: license_code.to_string(),
    })
}

fn register_window(hwnd: HWND) {
    WINDOW_HWNDS.with(|windows| windows.borrow_mut().push(hwnd));
}

fn unregister_window(hwnd: HWND) {
    WINDOW_HWNDS.with(|windows| windows.borrow_mut().retain(|h| *h != hwnd));
}

unsafe fn dismiss_all_windows() {
    unsafe {
        let hwnds = WINDOW_HWNDS.with(|windows| windows.borrow().clone());
        for hwnd in hwnds {
            if IsWindow(Some(hwnd)).as_bool() {
                let _ = DestroyWindow(hwnd);
            }
        }
        WINDOW_HWNDS.with(|windows| windows.borrow_mut().clear());
        PostQuitMessage(0);
    }
}

unsafe extern "system" fn enum_monitor_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _clip_rect: *mut RECT,
    lparam: LPARAM,
) -> windows::core::BOOL {
    unsafe {
        let monitors = &mut *(lparam.0 as *mut Vec<RECT>);
        let mut info = MONITORINFO {
            cbSize: mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(hmonitor, &mut info as *mut MONITORINFO).as_bool() {
            monitors.push(info.rcMonitor);
        }
        true.into()
    }
}

fn enumerate_monitors() -> Vec<RECT> {
    let mut monitors = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(enum_monitor_proc),
            LPARAM((&mut monitors as *mut Vec<RECT>) as isize),
        );
    }
    monitors
}

fn monitor_photo_queue(
    photos: &[PhotoEntry],
    monitor_index: usize,
    monitor_count: usize,
) -> Vec<PhotoEntry> {
    if photos.is_empty() {
        return Vec::new();
    }

    let n = photos.len();
    let offset = if monitor_count == 0 {
        0
    } else {
        monitor_index.saturating_mul(n / monitor_count)
    } % n;

    (0..n)
        .map(|idx| photos[(offset + idx) % n].clone())
        .collect()
}

fn effective_aspect_ratio_mode(mode: AspectRatioMode, license_code: &str) -> AspectRatioMode {
    if PhotoLicense::from_code(license_code).is_some_and(|license| license.is_no_derivatives()) {
        AspectRatioMode::Contain
    } else {
        mode
    }
}

fn photo_rects(
    img_w: i32,
    img_h: i32,
    screen_w: i32,
    screen_h: i32,
    mode: AspectRatioMode,
    license_code: &str,
) -> (RECT, RECT) {
    let resolved_mode = effective_aspect_ratio_mode(mode, license_code);
    let full_src = RECT {
        left: 0,
        top: 0,
        right: img_w,
        bottom: img_h,
    };

    match resolved_mode {
        AspectRatioMode::Contain => (contain_rect(img_w, img_h, screen_w, screen_h), full_src),
        AspectRatioMode::Fill => {
            let (dest, src) = fill_rect(img_w, img_h, screen_w, screen_h);
            (dest, src)
        }
    }
}

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

/// Compute a centered destination `RECT` that fits `img_w × img_h` inside
/// `screen_w × screen_h` while preserving the aspect ratio (contain / letterbox).
fn contain_rect(img_w: i32, img_h: i32, screen_w: i32, screen_h: i32) -> RECT {
    let img_ratio = img_w as f64 / img_h.max(1) as f64;
    let scr_ratio = screen_w as f64 / screen_h.max(1) as f64;

    let (dw, dh) = if img_ratio > scr_ratio {
        // Image is wider than screen — fit to width, letterbox top/bottom
        (screen_w, (screen_w as f64 / img_ratio) as i32)
    } else {
        // Image is taller — fit to height, pillarbox left/right
        ((screen_h as f64 * img_ratio) as i32, screen_h)
    };

    let x = (screen_w - dw) / 2;
    let y = (screen_h - dh) / 2;

    RECT {
        left: x,
        top: y,
        right: x + dw,
        bottom: y + dh,
    }
}

fn fill_rect(img_w: i32, img_h: i32, screen_w: i32, screen_h: i32) -> (RECT, RECT) {
    let safe_img_w = img_w.max(1);
    let safe_img_h = img_h.max(1);
    let safe_screen_w = screen_w.max(1);
    let safe_screen_h = screen_h.max(1);

    let img_ratio = safe_img_w as f64 / safe_img_h as f64;
    let scr_ratio = safe_screen_w as f64 / safe_screen_h as f64;

    let (src_x, src_y, src_w, src_h) = if img_ratio > scr_ratio {
        let crop_w = ((safe_img_h as f64) * scr_ratio).round() as i32;
        let clamped_w = crop_w.clamp(1, safe_img_w);
        ((safe_img_w - clamped_w) / 2, 0, clamped_w, safe_img_h)
    } else {
        let crop_h = ((safe_img_w as f64) / scr_ratio).round() as i32;
        let clamped_h = crop_h.clamp(1, safe_img_h);
        (0, (safe_img_h - clamped_h) / 2, safe_img_w, clamped_h)
    };

    let dest = RECT {
        left: 0,
        top: 0,
        right: safe_screen_w,
        bottom: safe_screen_h,
    };
    let src = RECT {
        left: src_x,
        top: src_y,
        right: src_x + src_w,
        bottom: src_y + src_h,
    };

    (dest, src)
}

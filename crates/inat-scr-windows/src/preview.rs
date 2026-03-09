//! Preview mode (`/p <HWND>`).
//!
//! Renders a simple branded panel into the host-provided window handle
//! (~152x112px pane in the Screen Saver Settings dialog).
//!
//! Deliberately kept minimal — the preview pane is too small for photos.
//! The actual screensaver experience lives in fullscreen `/s` mode.

use anyhow::Result;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect, SetBkMode,
    SetTextColor, DT_CENTER, DT_VCENTER, DT_WORDBREAK, PAINTSTRUCT, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DispatchMessageW, GetClientRect, GetMessageW, GetWindowLongPtrW, IsWindow,
    SetWindowLongPtrW, TranslateMessage, GWLP_USERDATA, GWLP_WNDPROC, MSG, WNDPROC,
};

const PREVIEW_LABEL: &str = "iNaturalist\nScreensaver";

/// WM_PAINT constant (windows-rs may export as u32 or WINDOW_MESSAGE).
const WM_PAINT_ID: u32 = 0x000F;
const WM_ERASEBKGND_ID: u32 = 0x0014;

/// Subclassed WndProc that handles WM_PAINT and suppresses WM_ERASEBKGND.
unsafe extern "system" fn preview_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_ERASEBKGND_ID {
        // Suppress background erase — we paint the full area ourselves.
        return LRESULT(1);
    }

    if msg == WM_PAINT_ID {
        let mut ps = PAINTSTRUCT::default();
        let hdc = unsafe { BeginPaint(hwnd, &mut ps) };

        let mut rect = RECT::default();
        let _ = unsafe { GetClientRect(hwnd, &mut rect) };

        let brush = unsafe { CreateSolidBrush(COLORREF(0x00101010)) };
        let _ = unsafe { FillRect(hdc, &rect, brush) };
        let _ = unsafe { DeleteObject(brush.into()) };

        let _ = unsafe { SetBkMode(hdc, TRANSPARENT) };
        let _ = unsafe { SetTextColor(hdc, COLORREF(0x00E0E0E0)) };
        let mut text_utf16: Vec<u16> = PREVIEW_LABEL.encode_utf16().collect();
        let mut text_rect = RECT {
            left: rect.left + 8,
            top: rect.top + 8,
            right: rect.right - 8,
            bottom: rect.bottom - 8,
        };
        let _ = unsafe {
            DrawTextW(
                hdc,
                &mut text_utf16,
                &mut text_rect,
                DT_CENTER | DT_VCENTER | DT_WORDBREAK,
            )
        };

        let _ = unsafe { EndPaint(hwnd, &ps) };
        return LRESULT(0);
    }

    let old_proc = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
    let wndproc: WNDPROC = unsafe { std::mem::transmute(old_proc) };
    unsafe { CallWindowProcW(wndproc, hwnd, msg, wparam, lparam) }
}

/// Run preview mode: subclass the host window and run a message loop.
pub fn run(hwnd_raw: u64) -> Result<()> {
    let hwnd = HWND(hwnd_raw as *mut _);

    // Subclass the window so we control WM_PAINT and WM_ERASEBKGND.
    unsafe {
        let old_proc = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, preview_wndproc as *const () as isize);
        let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, old_proc);
    }

    // Message loop — exits when the parent dialog destroys the window.
    let mut msg = MSG::default();
    while unsafe { IsWindow(Some(hwnd)).as_bool() } {
        let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if !ret.as_bool() {
            break;
        }
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}

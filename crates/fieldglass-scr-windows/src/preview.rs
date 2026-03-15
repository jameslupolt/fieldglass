//! Preview mode (`/p <HWND>`).
//!
//! Creates a child window inside the host-provided preview pane
//! (~152×112 px in the Screen Saver Settings dialog) and renders a
//! simple branded label.
//!
//! Deliberately kept minimal — the preview pane is too small for photos.
//! The actual screensaver experience lives in fullscreen `/s` mode.
//!
//! ## Lifecycle
//!
//! When the Settings dialog tears down its preview pane the child window
//! receives `WM_DESTROY`, which posts `WM_QUIT` to exit the message loop.
//! A periodic timer provides an additional safety net by verifying the
//! parent window is still alive — this guards against cross-process
//! window-destruction edge cases where `WM_DESTROY` may not be delivered.

use std::mem;

use anyhow::{Context, Result};
use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect, SetBkMode,
    SetTextColor, DT_CENTER, DT_VCENTER, DT_WORDBREAK, PAINTSTRUCT, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, GetMessageW,
    GetWindowLongPtrW, IsWindow, KillTimer, PostQuitMessage, RegisterClassExW, SetTimer,
    SetWindowLongPtrW, TranslateMessage, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, MSG, WM_DESTROY,
    WM_ERASEBKGND, WM_PAINT, WM_TIMER, WNDCLASSEXW, WS_CHILD, WS_VISIBLE,
};

const PREVIEW_LABEL: &str = "Field Glass\nScreensaver";

/// Timer ID for the periodic parent-liveness check.
const TIMER_PARENT_CHECK: usize = 1;
/// Interval (ms) between parent-liveness checks.
const PARENT_CHECK_MS: u32 = 250;

// ---------------------------------------------------------------------------
// Window procedure
// ---------------------------------------------------------------------------

unsafe extern "system" fn preview_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                let mut rect = RECT::default();
                let _ = GetClientRect(hwnd, &mut rect);

                // Dark background
                let brush = CreateSolidBrush(COLORREF(0x00101010));
                let _ = FillRect(hdc, &rect, brush);
                let _ = DeleteObject(brush.into());

                // Centered label
                let _ = SetBkMode(hdc, TRANSPARENT);
                let _ = SetTextColor(hdc, COLORREF(0x00E0E0E0));
                let mut text: Vec<u16> = PREVIEW_LABEL.encode_utf16().collect();
                let mut text_rect = RECT {
                    left: rect.left + 8,
                    top: rect.top + 8,
                    right: rect.right - 8,
                    bottom: rect.bottom - 8,
                };
                let _ = DrawTextW(
                    hdc,
                    &mut text,
                    &mut text_rect,
                    DT_CENTER | DT_VCENTER | DT_WORDBREAK,
                );

                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }

            WM_ERASEBKGND => {
                // We paint the full client area ourselves — suppress default erase.
                LRESULT(1)
            }

            WM_TIMER if wparam.0 == TIMER_PARENT_CHECK => {
                // Safety net: if the parent preview pane was destroyed without
                // our child receiving WM_DESTROY, tear down and exit.
                let parent_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
                let parent = HWND(parent_ptr as *mut _);
                if !IsWindow(Some(parent)).as_bool() {
                    // DestroyWindow triggers WM_DESTROY → PostQuitMessage.
                    if DestroyWindow(hwnd).is_err() {
                        PostQuitMessage(0);
                    }
                }
                LRESULT(0)
            }

            WM_DESTROY => {
                let _ = KillTimer(Some(hwnd), TIMER_PARENT_CHECK);
                PostQuitMessage(0);
                LRESULT(0)
            }

            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Run preview mode: create a child window inside the host-provided preview
/// pane and pump messages until the pane is destroyed.
pub fn run(hwnd_raw: u64) -> Result<()> {
    let parent = HWND(hwnd_raw as *mut _);

    let hinstance: HINSTANCE =
        unsafe { GetModuleHandleW(None).context("GetModuleHandleW")?.into() };

    // Register a window class for the preview child.
    let wc = WNDCLASSEXW {
        cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(preview_wndproc),
        hInstance: hinstance,
        lpszClassName: w!("FieldGlassPreview"),
        ..Default::default()
    };
    // The class may already be registered — ignore the error.
    unsafe {
        RegisterClassExW(&wc);
    }

    // Size the child to fill the parent's client area.
    let mut rect = RECT::default();
    unsafe {
        let _ = GetClientRect(parent, &mut rect);
    }

    let child = unsafe {
        CreateWindowExW(
            Default::default(), // no extended styles
            w!("FieldGlassPreview"),
            None,
            WS_CHILD | WS_VISIBLE,
            0,
            0,
            rect.right - rect.left,
            rect.bottom - rect.top,
            Some(parent),
            None,
            Some(hinstance),
            None,
        )
        .context("Failed to create preview child window")?
    };

    // Stash the parent HWND so the timer can verify it is still alive.
    unsafe {
        SetWindowLongPtrW(child, GWLP_USERDATA, parent.0 as isize);
    }

    // Start a periodic liveness check as a safety net.
    unsafe {
        SetTimer(Some(child), TIMER_PARENT_CHECK, PARENT_CHECK_MS, None);
    }

    // Message loop — exits on WM_QUIT (posted by WM_DESTROY or the timer).
    let mut msg = MSG::default();
    loop {
        let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        // 0 → WM_QUIT, -1 → error
        if ret.0 <= 0 {
            break;
        }
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}

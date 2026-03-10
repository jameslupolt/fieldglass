//! Configuration mode (`/c`).
//!
//! Opens or focuses the companion app's settings window. If the companion
//! is not running, launches it.

use anyhow::{Context, Result};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE,
};

use std::process::Command;

/// Open the companion app for configuration.
///
/// 1. Try to find an existing companion window and bring it to front.
/// 2. If not found, launch the companion app executable.
pub fn open_companion() -> Result<()> {
    let titles = [windows::core::w!("Field Glass")];

    for title in titles {
        let hwnd = unsafe { FindWindowW(None, title) };
        if let Ok(hwnd) = hwnd {
            if hwnd == HWND::default() {
                continue;
            }
            tracing::info!("Found companion window, bringing to front");
            unsafe {
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(hwnd);
            }
            return Ok(());
        }
    }

    // Companion not running — try to launch it
    tracing::info!("Companion not found, attempting to launch");

    // Look for the companion executable next to the .scr
    let scr_path = std::env::current_exe().context("Failed to get .scr executable path")?;
    let companion_dir = scr_path.parent().context("Failed to get .scr directory")?;

    let sidecar_path = companion_dir.join("FieldGlass.companion-path.txt");
    if let Ok(sidecar_contents) = std::fs::read_to_string(&sidecar_path) {
        let candidate = sidecar_contents.trim();
        if !candidate.is_empty() {
            let companion_path = std::path::PathBuf::from(candidate);
            if companion_path.exists() {
                Command::new(&companion_path).spawn().with_context(|| {
                    format!("Failed to launch companion at {}", companion_path.display())
                })?;
                tracing::info!(path = %companion_path.display(), "Launched companion app from sidecar path");
                return Ok(());
            }
        }
    }

    // Try common companion names
    let companion_names = [
        "Field Glass.exe",
        "field-glass.exe",
        "FieldGlass.exe",
        "fieldglass-companion.exe",
    ];

    for name in &companion_names {
        let companion_path = companion_dir.join(name);
        if companion_path.exists() {
            Command::new(&companion_path).spawn().with_context(|| {
                format!("Failed to launch companion at {}", companion_path.display())
            })?;
            tracing::info!(path = %companion_path.display(), "Launched companion app");
            return Ok(());
        }
    }

    // Also try Program Files
    if let Ok(program_files) = std::env::var("ProgramFiles") {
        let companion_path = std::path::Path::new(&program_files)
            .join("Field Glass")
            .join("Field Glass.exe");
        if companion_path.exists() {
            Command::new(&companion_path).spawn().with_context(|| {
                format!("Failed to launch companion at {}", companion_path.display())
            })?;
            tracing::info!(path = %companion_path.display(), "Launched companion app");
            return Ok(());
        }
    }

    anyhow::bail!(
        "Could not find companion app. Run Field Glass manually, or reinstall with companion integration."
    )
}

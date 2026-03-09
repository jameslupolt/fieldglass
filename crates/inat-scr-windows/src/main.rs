#![cfg_attr(windows, windows_subsystem = "windows")]

//! Windows .scr screensaver host for Field Glass.
//!
//! This binary is compiled as a `.scr` (renamed `.exe`) and handles the
//! standard Windows screensaver command-line protocol:
//!
//! - `/s`         — Run fullscreen screensaver on the primary monitor
//! - `/c`         — Open configuration (launch/focus companion app)
//! - `/p <HWND>`  — Render preview into the host-provided window handle
//!
//! The fullscreen mode uses native Win32 GDI rendering to display cached
//! photos with crossfade transitions. Preview mode renders a static branded
//! panel using native GDI.

#[cfg(windows)]
mod config;
#[cfg(windows)]
mod fullscreen;
#[cfg(windows)]
mod preview;

fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    let mode = parse_mode(&args);

    match mode {
        ScreensaverMode::Fullscreen => {
            tracing::info!("Starting fullscreen screensaver");
            #[cfg(windows)]
            {
                if let Err(e) = fullscreen::run() {
                    tracing::error!(error = %e, "Fullscreen mode failed");
                }
            }
            #[cfg(not(windows))]
            {
                eprintln!("Fullscreen mode requires Windows");
            }
        }
        ScreensaverMode::Configure => {
            tracing::info!("Opening configuration");
            #[cfg(windows)]
            {
                if let Err(e) = config::open_companion() {
                    tracing::error!(error = %e, "Configure mode failed");
                    show_error_dialog(
                        "Field Glass",
                        &format!("Could not open settings. {e}"),
                    );
                }
            }
            #[cfg(not(windows))]
            {
                eprintln!("Configure mode requires Windows");
            }
        }
        ScreensaverMode::Preview(hwnd) => {
            tracing::info!(hwnd, "Starting preview mode");
            #[cfg(windows)]
            {
                if let Err(e) = preview::run(hwnd) {
                    tracing::error!(error = %e, "Preview mode failed");
                }
            }
            #[cfg(not(windows))]
            {
                let _ = hwnd;
                eprintln!("Preview mode requires Windows");
            }
        }
        ScreensaverMode::Unknown => {
            tracing::info!("No recognized flags — opening configuration");
            #[cfg(windows)]
            {
                if let Err(e) = config::open_companion() {
                    tracing::error!(error = %e, "Configure mode failed");
                    show_error_dialog(
                        "Field Glass",
                        &format!("Could not open settings. {e}"),
                    );
                }
            }
            #[cfg(not(windows))]
            {
                eprintln!("Configure mode requires Windows");
            }
        }
    }
}

#[cfg(windows)]
fn show_error_dialog(title: &str, message: &str) {
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    let mut title_w: Vec<u16> = title.encode_utf16().collect();
    title_w.push(0);
    let mut msg_w: Vec<u16> = message.encode_utf16().collect();
    msg_w.push(0);

    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(msg_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

/// Screensaver operating mode, determined by command-line flags.
enum ScreensaverMode {
    /// `/s` — fullscreen slideshow
    Fullscreen,
    /// `/c` — open settings / companion app
    Configure,
    /// `/p <HWND>` — render preview into host window
    Preview(u64),
    /// Unrecognized arguments
    Unknown,
}

/// Parse Windows screensaver command-line flags.
///
/// Windows may pass flags as `/s`, `/S`, `-s`, or `/c:HWND`. The HWND for
/// `/p` mode may be the next argument or appended after a colon.
fn parse_mode(args: &[String]) -> ScreensaverMode {
    for (i, arg) in args.iter().enumerate().skip(1) {
        let lower = arg.to_lowercase();
        let flag = lower.trim_start_matches('/').trim_start_matches('-');

        if flag == "s" {
            return ScreensaverMode::Fullscreen;
        }

        if flag.starts_with('c') {
            return ScreensaverMode::Configure;
        }

        if flag.starts_with('p') {
            let hwnd_str = if flag.len() > 1 {
                flag[1..].trim_start_matches(':')
            } else if let Some(next) = args.get(i + 1) {
                next.as_str()
            } else {
                return ScreensaverMode::Unknown;
            };

            if let Some(hwnd) = parse_hwnd(hwnd_str) {
                return ScreensaverMode::Preview(hwnd);
            }
        }
    }

    ScreensaverMode::Unknown
}

fn parse_hwnd(input: &str) -> Option<u64> {
    let trimmed = input.trim().trim_matches('"');
    if trimmed.is_empty() {
        return None;
    }

    let hex = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"));
    if let Some(h) = hex {
        return u64::from_str_radix(h, 16).ok();
    }

    if let Ok(v) = trimmed.parse::<u64>() {
        return Some(v);
    }

    trimmed.parse::<isize>().ok().map(|v| v as u64)
}

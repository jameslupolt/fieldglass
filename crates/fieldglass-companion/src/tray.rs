//! System tray setup and menu for the companion app.

use tauri::{
    Emitter,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, Manager,
};

pub fn create_tray(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let preview = MenuItem::with_id(app, "preview", "Preview Screensaver", true, None::<&str>)?;
    let refresh = MenuItem::with_id(app, "refresh", "Refresh Cache Now", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Open Settings", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&preview, &refresh, &settings, &quit])?;

    TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Field Glass")
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "preview" => {
                tracing::info!("Tray: preview requested");
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = app.emit("navigate", "preview");
                }
            }
            "refresh" => {
                tracing::info!("Tray: manual cache refresh requested");
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    match crate::commands::cache::refresh_cache().await {
                        Ok(status) => {
                            tracing::info!(
                                cached = status.total_photos,
                                required = status.required_photos,
                                "Cache refresh completed from tray"
                            );
                            let _ = handle.emit("cache-refreshed", ());
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "Cache refresh failed from tray");
                        }
                    }
                });
            }
            "settings" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

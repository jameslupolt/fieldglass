//! Tauri companion app for the Field Glass screensaver.
//!
//! Provides a settings UI, system tray integration, cache management,
//! and background refresh. The React frontend communicates exclusively
//! via Tauri `invoke()` commands — it makes no network or filesystem
//! operations directly.

mod commands;
mod tray;

use tracing_subscriber::EnvFilter;

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("inat=info".parse().unwrap()))
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            tray::create_tray(app)?;
            tracing::info!("Field Glass started");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::cache::get_cache_status,
            commands::cache::refresh_cache,
            commands::cache::clear_cache,
            commands::taxa::search_taxa,
            commands::location::search_location,
            commands::photos::get_cached_photos,
            commands::photos::get_photo_details,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Field Glass");
}

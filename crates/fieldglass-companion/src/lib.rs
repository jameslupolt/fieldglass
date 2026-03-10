//! Tauri companion app for the Field Glass screensaver.
//!
//! Provides a settings UI, system tray integration, cache management,
//! and background refresh. The React frontend communicates exclusively
//! via Tauri `invoke()` commands — it makes no network or filesystem
//! operations directly.

mod commands;
mod tray;

use std::time::Duration;

use fieldglass_core::Settings;
use tracing_subscriber::EnvFilter;

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("fieldglass_core=info".parse().unwrap())
                .add_directive("fieldglass_companion=info".parse().unwrap()),
        )
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
            start_auto_refresh_loop();
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
            commands::photos::delete_cached_photo,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Field Glass");
}

fn start_auto_refresh_loop() {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(60));
        let mut last_refresh_attempt: Option<std::time::Instant> = None;

        loop {
            ticker.tick().await;

            let settings_path = match Settings::default_path() {
                Ok(path) => path,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to resolve settings path for auto-refresh");
                    continue;
                }
            };

            let settings = match Settings::load(&settings_path) {
                Ok(settings) => settings,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to load settings for auto-refresh");
                    continue;
                }
            };

            let interval_minutes = settings.cache_refresh_interval_minutes;
            if interval_minutes == 0 {
                continue;
            }

            let should_refresh = match last_refresh_attempt {
                Some(instant) => {
                    instant.elapsed() >= Duration::from_secs(u64::from(interval_minutes) * 60)
                }
                None => true,
            };

            if !should_refresh {
                continue;
            }

            last_refresh_attempt = Some(std::time::Instant::now());

            match crate::commands::cache::refresh_cache().await {
                Ok(status) => tracing::info!(
                    total = status.total_photos,
                    required = status.required_photos,
                    "Auto cache refresh complete"
                ),
                Err(e) => tracing::warn!(error = %e, "Auto cache refresh failed"),
            }
        }
    });
}

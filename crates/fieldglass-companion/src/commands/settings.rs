//! Tauri commands for reading and writing user settings.
//!
//! Settings are persisted as JSON in the app data directory. The companion
//! app is the sole writer; screensaver hosts read the same file.

use fieldglass_core::Settings;

/// Return the current settings (defaults if no file exists).
#[tauri::command]
pub fn get_settings() -> Result<Settings, String> {
    let path = Settings::default_path().map_err(|e| e.to_string())?;
    Settings::load(&path).map_err(|e| e.to_string())
}

/// Persist updated settings to disk (atomic write via rename).
#[tauri::command]
pub fn update_settings(settings: Settings) -> Result<(), String> {
    let path = Settings::default_path().map_err(|e| e.to_string())?;
    settings.save(&path).map_err(|e| e.to_string())
}

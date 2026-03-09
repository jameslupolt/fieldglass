//! Tauri command for geocoding (location search).
//!
//! Delegates to `fieldglass_core::api::geocoding` which supports both Photon
//! (default, autocomplete-friendly) and Nominatim (fallback) backends.
//! The geocoder backend is selected from user settings.

use fieldglass_core::api::geocoding::create_geocoder;
use fieldglass_core::types::GeocodingResult;
use fieldglass_core::{ApiClient, Settings};

/// Search for a location by name (city/region autocomplete).
///
/// Uses the geocoding backend configured in settings (Photon by default).
/// Returns up to 5 matching locations with coordinates and display names.
#[tauri::command]
pub async fn search_location(query: String) -> Result<Vec<GeocodingResult>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let settings = load_settings()?;
    let client = ApiClient::new("0.1.0").map_err(|e| e.to_string())?;
    let geocoder = create_geocoder(settings.geocoder_backend, &client);
    geocoder.search(&query).await.map_err(|e| e.to_string())
}

fn load_settings() -> Result<Settings, String> {
    let path = Settings::default_path().map_err(|e| e.to_string())?;
    Settings::load(&path).map_err(|e| e.to_string())
}

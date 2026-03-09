//! User-facing settings, persisted as JSON in the app data directory.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::types::{AspectRatioMode, Location, SearchRadius};

/// All user-configurable settings for the screensaver.
///
/// Persisted as a JSON file in the app data directory. The companion app writes
/// this file; the screensaver hosts and core library read it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    // -- Location --
    /// User's selected location.
    pub location: Option<Location>,
    /// Search radius around the location.
    #[serde(default)]
    pub search_radius: SearchRadius,

    // -- Taxa --
    /// Selected taxon IDs. Empty = all life.
    #[serde(default)]
    pub taxon_ids: Vec<u64>,

    // -- Display --
    /// Seconds each photo is displayed.
    #[serde(default = "default_photo_duration")]
    pub photo_duration_secs: u32,
    /// How to fit photos to the screen.
    #[serde(default)]
    pub aspect_ratio_mode: AspectRatioMode,
    /// Overlay opacity (0.0 = invisible, 1.0 = fully opaque).
    #[serde(default = "default_overlay_opacity")]
    pub overlay_opacity: f32,

    // -- Content filters --
    /// Only show research-grade observations.
    #[serde(default = "default_true")]
    pub research_grade_only: bool,
    /// Only show photos with CC licenses.
    #[serde(default = "default_true")]
    pub licensed_only: bool,
    /// Exclude observations annotated as "Dead".
    #[serde(default = "default_true")]
    pub exclude_dead: bool,
    /// Exclude non-organism evidence (scat, tracks, bones).
    #[serde(default = "default_true")]
    pub exclude_non_organism: bool,

    // -- Cache --
    /// Legacy no-repeat preference retained for compatibility.
    #[serde(default = "default_no_repeat_minutes")]
    pub no_repeat_minutes: u32,
    /// Legacy monitor count retained for compatibility.
    #[serde(default = "default_monitor_count")]
    pub monitor_count: u32,
    /// Target number of photos to keep in cache.
    #[serde(default = "default_cache_max_items")]
    pub cache_max_items: u32,
    /// Automatic cache refresh interval in minutes. Set to 0 to disable.
    #[serde(default = "default_cache_refresh_interval_minutes")]
    pub cache_refresh_interval_minutes: u32,

    // -- Geocoding --
    /// Which geocoding backend to use.
    #[serde(default)]
    pub geocoder_backend: GeocoderBackend,

    // -- Companion --
    /// Whether the companion app should auto-start with the OS.
    #[serde(default)]
    pub auto_start: bool,
}

/// Which geocoding service to use. Must be runtime-switchable per OSMF policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GeocoderBackend {
    #[default]
    Photon,
    Nominatim,
}

// -- Defaults --

fn default_photo_duration() -> u32 {
    15
}
fn default_overlay_opacity() -> f32 {
    0.75
}
fn default_true() -> bool {
    true
}
fn default_no_repeat_minutes() -> u32 {
    30
}
fn default_monitor_count() -> u32 {
    2
}
fn default_cache_max_items() -> u32 {
    240
}
fn default_cache_refresh_interval_minutes() -> u32 {
    60
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            location: None,
            search_radius: SearchRadius::default(),
            taxon_ids: Vec::new(),
            photo_duration_secs: default_photo_duration(),
            aspect_ratio_mode: AspectRatioMode::default(),
            overlay_opacity: default_overlay_opacity(),
            research_grade_only: true,
            licensed_only: true,
            exclude_dead: true,
            exclude_non_organism: true,
            no_repeat_minutes: default_no_repeat_minutes(),
            monitor_count: default_monitor_count(),
            cache_max_items: default_cache_max_items(),
            cache_refresh_interval_minutes: default_cache_refresh_interval_minutes(),
            geocoder_backend: GeocoderBackend::default(),
            auto_start: false,
        }
    }
}

impl Settings {
    /// Compute the required number of cached images for the current settings.
    ///
    /// Uses the explicit user-defined cache size target.
    pub fn required_cache_size(&self) -> u32 {
        self.cache_max_items.max(1)
    }

    /// Load settings from a JSON file. Returns defaults if the file doesn't exist.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read settings from {}", path.display()))?;
        let settings: Self = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse settings from {}", path.display()))?;
        Ok(settings)
    }

    /// Save settings to a JSON file (atomic write via rename).
    pub fn save(&self, path: &Path) -> Result<()> {
        let contents =
            serde_json::to_string_pretty(self).context("Failed to serialize settings")?;

        // Atomic write: write to temp file, then rename
        let dir = path
            .parent()
            .context("Settings path has no parent directory")?;
        std::fs::create_dir_all(dir)
            .with_context(|| format!("Failed to create settings directory {}", dir.display()))?;

        let tmp_path = dir.join(".settings.tmp");
        std::fs::write(&tmp_path, &contents)
            .with_context(|| format!("Failed to write temp settings to {}", tmp_path.display()))?;
        std::fs::rename(&tmp_path, path)
            .with_context(|| format!("Failed to rename settings to {}", path.display()))?;
        Ok(())
    }

    /// Returns the default settings file path for this platform.
    pub fn default_path() -> Result<PathBuf> {
        let dirs = directories::ProjectDirs::from("", "", "field-glass")
            .context("Could not determine app data directory")?;
        Ok(dirs.config_dir().join("settings.json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cache_size() {
        let settings = Settings::default();
        // 30 min × 60 × 2 monitors / 15s = 240
        assert_eq!(settings.required_cache_size(), 240);
    }

    #[test]
    fn single_monitor_cache_size() {
        let settings = Settings {
            monitor_count: 1,
            ..Settings::default()
        };
        assert_eq!(settings.required_cache_size(), 240);
    }

    #[test]
    fn explicit_cache_size() {
        let settings = Settings {
            cache_max_items: 500,
            ..Settings::default()
        };
        assert_eq!(settings.required_cache_size(), 500);
    }

    #[test]
    fn roundtrip_json() {
        let settings = Settings::default();
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.photo_duration_secs, 15);
        assert_eq!(parsed.required_cache_size(), 240);
    }
}

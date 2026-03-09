//! Tauri commands for querying cached photos.
//!
//! The companion UI displays cached photos in a preview panel and a
//! details view. These commands read from the cache (SQLite + filesystem)
//! without any network access.

use inat_core::types::CachedPhoto;
use inat_core::CacheManager;

/// Return all cached photos for the preview grid / list.
///
/// Photos are returned in random order (same shuffle as the screensaver
/// display queue).
#[tauri::command]
pub fn get_cached_photos() -> Result<Vec<CachedPhoto>, String> {
    let manager = CacheManager::new().map_err(|e| e.to_string())?;
    let mut photos = manager.get_display_queue().map_err(|e| e.to_string())?;

    // Convert relative file_path to absolute so the frontend can use convertFileSrc
    for photo in &mut photos {
        let abs = manager.storage().absolute_path_for(&photo.file_path);
        photo.file_path = abs.to_string_lossy().into_owned();
    }

    Ok(photos)
}

/// Return details for a single cached photo by its iNaturalist photo ID.
#[tauri::command]
pub fn get_photo_details(photo_id: u64) -> Result<Option<CachedPhoto>, String> {
    let manager = CacheManager::new().map_err(|e| e.to_string())?;
    let result = manager
        .get_photo_for_display(photo_id)
        .map_err(|e| e.to_string())?;
    Ok(result.map(|(mut photo, path)| {
        photo.file_path = path.to_string_lossy().into_owned();
        photo
    }))
}

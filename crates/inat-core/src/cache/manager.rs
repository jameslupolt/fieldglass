//! High-level cache lifecycle management.
//!
//! Orchestrates image storage, metadata persistence, and cache rotation.
//! All methods are synchronous — async I/O boundaries live at higher layers
//! (e.g., the companion app's cache refresh loop).

use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use tracing;

use crate::config::Settings;
use crate::types::CachedPhoto;

use super::metadata::MetadataStore;
use super::storage::CacheStorage;

/// High-level cache status reported to the companion app UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStatus {
    /// Number of active (non-pending-deletion) cached photos.
    pub total_photos: u64,
    /// Number of photos required by current settings.
    pub required_photos: u32,
    /// Total disk usage of cached images in bytes.
    pub cache_size_bytes: u64,
}

/// Manages the photo cache lifecycle: adding, rotating, and cleaning up photos.
///
/// The companion app uses this to fill and maintain the cache.
/// Screensaver hosts use `MetadataStore::open_readonly` + `CacheStorage` directly
/// for read-only access.
pub struct CacheManager {
    metadata: MetadataStore,
    storage: CacheStorage,
}

impl CacheManager {
    /// Create a new `CacheManager` using default platform paths.
    ///
    /// Opens the metadata database (with WAL mode), creates cache directories,
    /// and initializes the schema if needed.
    pub fn new() -> Result<Self> {
        let base_path =
            CacheStorage::default_base_path().context("Failed to determine cache base path")?;
        Self::with_base_path(base_path)
    }

    /// Create a `CacheManager` with a specific base path (useful for testing).
    pub fn with_base_path(base_path: PathBuf) -> Result<Self> {
        let storage = CacheStorage::new(base_path);
        storage
            .ensure_dirs()
            .context("Failed to create cache directories")?;

        let db_path = storage.db_path();
        let metadata = MetadataStore::open(&db_path).context("Failed to open metadata database")?;
        metadata
            .initialize()
            .context("Failed to initialize metadata schema")?;

        tracing::info!(
            path = %storage.base_path().display(),
            "Cache manager initialized"
        );

        Ok(Self { metadata, storage })
    }

    /// Get the current cache status.
    pub fn status(&self, settings: &Settings) -> Result<CacheStatus> {
        let total_photos = self
            .metadata
            .count()
            .context("Failed to count cached photos")?;
        let cache_size_bytes = self
            .storage
            .cache_size_bytes()
            .context("Failed to compute cache size")?;

        Ok(CacheStatus {
            total_photos,
            required_photos: settings.required_cache_size(),
            cache_size_bytes,
        })
    }

    /// Whether the cache needs more photos to meet the target.
    pub fn needs_refresh(&self, settings: &Settings) -> bool {
        match self.metadata.count() {
            Ok(count) => (count as u32) < settings.required_cache_size(),
            Err(e) => {
                tracing::warn!(error = %e, "Failed to count photos, assuming refresh needed");
                true
            }
        }
    }

    /// Add a photo to the cache.
    ///
    /// Writes the image data to disk (atomic via tmp+rename) and inserts
    /// metadata into SQLite. The `ext` is the file extension (e.g., "jpg").
    pub fn add_photo(&self, photo: CachedPhoto, image_data: &[u8], ext: &str) -> Result<()> {
        // Write image to disk atomically
        let final_path = self
            .storage
            .write_image_bytes(photo.photo_id, ext, image_data)
            .with_context(|| format!("Failed to write image for photo {}", photo.photo_id))?;

        // Store the relative path in metadata (relative to cache base)
        let relative_path = format!("images/{}.{}", photo.photo_id, ext.trim_start_matches('.'));
        let photo_with_path = CachedPhoto {
            file_path: relative_path,
            ..photo
        };

        // Insert metadata into SQLite
        self.metadata
            .insert_photo(&photo_with_path)
            .with_context(|| {
                format!(
                    "Failed to insert metadata for photo {}",
                    photo_with_path.photo_id
                )
            })?;

        tracing::debug!(
            photo_id = photo_with_path.photo_id,
            path = %final_path.display(),
            "Photo added to cache"
        );

        Ok(())
    }

    /// Mark the N lowest-scored photos as pending deletion.
    ///
    /// These photos will be cleaned up by `cleanup_pending()` after enough
    /// time has passed (to avoid deleting photos currently being displayed).
    pub fn remove_lowest_scored(&self, count: u32) -> Result<()> {
        let all_photos = self
            .metadata
            .get_all_photos()
            .context("Failed to get photos for eviction scoring")?;

        if all_photos.is_empty() {
            return Ok(());
        }

        // Sort by diversity_score ascending — lowest scores evicted first
        let mut scored: Vec<_> = all_photos.into_iter().collect();
        scored.sort_by(|a, b| {
            a.diversity_score
                .partial_cmp(&b.diversity_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let to_evict = scored.iter().take(count as usize);
        let mut evicted = 0u32;

        for photo in to_evict {
            self.metadata
                .mark_pending_deletion(photo.photo_id)
                .with_context(|| format!("Failed to mark photo {} for deletion", photo.photo_id))?;
            evicted += 1;
        }

        tracing::info!(evicted, "Marked photos as pending deletion");
        Ok(())
    }

    /// Delete photos that have been pending deletion for over 1 hour.
    ///
    /// This conservative window avoids deleting a photo that a screensaver
    /// instance is currently displaying.
    pub fn cleanup_pending(&self) -> Result<u64> {
        let one_hour_ago = Utc::now() - Duration::hours(1);

        // Get the photos to delete so we can remove their files
        let pending = self
            .metadata
            .get_pending_before(one_hour_ago)
            .context("Failed to get pending photos for cleanup")?;

        for photo in &pending {
            // Delete the image file from disk
            self.storage
                .delete_relative_path(&photo.file_path)
                .with_context(|| {
                    format!("Failed to delete image file for photo {}", photo.photo_id)
                })?;
        }

        // Delete the metadata rows
        let deleted = self
            .metadata
            .delete_pending(one_hour_ago)
            .context("Failed to delete pending photo metadata")?;

        if deleted > 0 {
            tracing::info!(deleted, "Cleaned up pending photos");
        }

        Ok(deleted)
    }

    /// Get all active photos for the screensaver display queue.
    ///
    /// Returns non-pending photos in a random order. Each screensaver instance
    /// builds its own shuffled queue from this list.
    pub fn get_display_queue(&self) -> Result<Vec<CachedPhoto>> {
        use rand::seq::SliceRandom;

        let mut photos = self
            .metadata
            .get_all_photos()
            .context("Failed to get photos for display queue")?;

        let mut rng = rand::rng();
        photos.shuffle(&mut rng);

        Ok(photos)
    }

    /// Get a single photo's metadata and its full filesystem path.
    ///
    /// Used by the screensaver to load a specific photo for display.
    pub fn get_photo_for_display(&self, photo_id: u64) -> Result<Option<(CachedPhoto, PathBuf)>> {
        let photo = self
            .metadata
            .get_photo_by_id(photo_id)
            .with_context(|| format!("Failed to get photo {photo_id}"))?;

        match photo {
            Some(p) => {
                let full_path = self.storage.absolute_path_for(&p.file_path);
                Ok(Some((p, full_path)))
            }
            None => Ok(None),
        }
    }

    pub fn remove_photo(&self, photo_id: u64) -> Result<bool> {
        let Some(photo) = self
            .metadata
            .get_photo_by_id(photo_id)
            .with_context(|| format!("Failed to get photo {photo_id} for deletion"))?
        else {
            return Ok(false);
        };

        self.storage
            .delete_relative_path(&photo.file_path)
            .with_context(|| format!("Failed to delete image for photo {photo_id}"))?;

        self.metadata
            .delete_photo_by_id(photo_id)
            .with_context(|| format!("Failed to delete metadata for photo {photo_id}"))
    }

    /// Get taxon representation counts (for diversity scoring).
    pub fn get_taxon_counts(&self) -> Result<std::collections::HashMap<u64, u32>> {
        self.metadata.get_taxon_counts()
    }

    /// Get observer representation counts (for diversity scoring).
    pub fn get_observer_counts(&self) -> Result<std::collections::HashMap<String, u32>> {
        self.metadata.get_observer_counts()
    }

    /// Get just the photo IDs (lightweight, for building display queues).
    pub fn get_photo_ids(&self) -> Result<Vec<u64>> {
        self.metadata.get_photo_ids()
    }

    /// Access the underlying storage (for path resolution).
    pub fn storage(&self) -> &CacheStorage {
        &self.storage
    }

    /// Delete all cached photos (metadata and image files).
    pub fn clear_all(&self) -> Result<u64> {
        // Get all photos so we can delete their image files
        let all_photos = self
            .metadata
            .get_all_photos()
            .context("Failed to get photos for clearing")?;

        for photo in &all_photos {
            let _ = self.storage.delete_relative_path(&photo.file_path);
        }

        let deleted = self
            .metadata
            .delete_all()
            .context("Failed to delete all photo metadata")?;

        tracing::info!(deleted, "Cleared all cached photos");
        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_settings() -> Settings {
        Settings::default()
    }

    #[test]
    fn new_cache_is_empty() {
        let tmp = TempDir::new().unwrap();
        let manager = CacheManager::with_base_path(tmp.path().to_path_buf()).unwrap();
        let status = manager.status(&test_settings()).unwrap();
        assert_eq!(status.total_photos, 0);
        assert_eq!(status.required_photos, 240);
    }

    #[test]
    fn needs_refresh_when_empty() {
        let tmp = TempDir::new().unwrap();
        let manager = CacheManager::with_base_path(tmp.path().to_path_buf()).unwrap();
        assert!(manager.needs_refresh(&test_settings()));
    }

    #[test]
    fn add_and_retrieve_photo() {
        let tmp = TempDir::new().unwrap();
        let manager = CacheManager::with_base_path(tmp.path().to_path_buf()).unwrap();

        let photo = CachedPhoto {
            photo_id: 12345,
            observation_id: 67890,
            file_path: String::new(), // will be overwritten by add_photo
            creator_name: "Test User".into(),
            license_code: "cc-by".into(),
            license_display: "CC BY 4.0".into(),
            observation_url: "https://www.inaturalist.org/observations/67890".into(),
            common_name: Some("Eastern Bluebird".into()),
            scientific_name: "Sialia sialis".into(),
            place_name: Some("Brooklyn, NY".into()),
            observed_on: Some("2025-06-15".into()),
            taxon_id: Some(42),
            iconic_taxon_name: Some("Aves".into()),
            observer_username: "testuser".into(),
            photo_width: Some(1024),
            photo_height: Some(768),
            attribution_text: "© Test User, CC BY 4.0, via iNaturalist".into(),
            diversity_score: 15.0,
            cached_at: Utc::now(),
            pending_deletion: false,
        };

        // Fake image data
        let image_data = b"fake jpeg data for testing";
        manager.add_photo(photo, image_data, "jpg").unwrap();

        let status = manager.status(&test_settings()).unwrap();
        assert_eq!(status.total_photos, 1);

        let (retrieved, path) = manager.get_photo_for_display(12345).unwrap().unwrap();
        assert_eq!(retrieved.photo_id, 12345);
        assert_eq!(retrieved.common_name.as_deref(), Some("Eastern Bluebird"));
        assert!(path.exists());
    }
}

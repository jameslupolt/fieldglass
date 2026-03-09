use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use tokio::fs;
use uuid::Uuid;

use crate::config::Settings;

pub struct CacheStorage {
    base_path: PathBuf,
}

impl CacheStorage {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    pub fn default_base_path() -> Result<PathBuf> {
        let dirs = ProjectDirs::from("", "", "field-glass")
            .context("Could not determine app data directory")?;
        Ok(dirs.data_local_dir().join("cache"))
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(self.images_dir())
            .with_context(|| format!("Failed to create images directory {}", self.images_dir().display()))?;
        std::fs::create_dir_all(self.tmp_dir())
            .with_context(|| format!("Failed to create temp directory {}", self.tmp_dir().display()))?;
        Ok(())
    }

    pub fn image_path(&self, photo_id: u64, ext: &str) -> PathBuf {
        let ext = normalize_ext(ext);
        self.images_dir().join(format!("{}.{}", photo_id, ext))
    }

    pub fn tmp_path(&self) -> PathBuf {
        self.tmp_dir().join(format!("{}.tmp", Uuid::new_v4()))
    }

    pub fn db_path(&self) -> PathBuf {
        self.base_path.join("metadata.db")
    }

    pub fn settings_path(&self) -> PathBuf {
        Settings::default_path().unwrap_or_else(|_| self.base_path.join("settings.json"))
    }

    pub async fn download_image(
        &self,
        client: &reqwest::Client,
        url: &str,
        photo_id: u64,
        ext: &str,
    ) -> Result<PathBuf> {
        let response = client
            .get(url)
            .send()
            .await
            .with_context(|| format!("Failed to download image from {}", url))?
            .error_for_status()
            .with_context(|| format!("Image download failed with non-success status from {}", url))?;

        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("Failed to read image bytes from {}", url))?;

        let tmp_path = self.tmp_path();
        let final_path = self.image_path(photo_id, ext);

        if let Some(parent) = tmp_path.parent() {
            fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Failed to create temp directory {}", parent.display()))?;
        }

        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Failed to create images directory {}", parent.display()))?;
        }

        fs::write(&tmp_path, &bytes)
            .await
            .with_context(|| format!("Failed to write temp image {}", tmp_path.display()))?;

        fs::rename(&tmp_path, &final_path).await.with_context(|| {
            format!(
                "Failed to atomically move image {} -> {}",
                tmp_path.display(),
                final_path.display()
            )
        })?;

        Ok(final_path)
    }

    pub fn write_image_bytes(&self, photo_id: u64, ext: &str, image_data: &[u8]) -> Result<PathBuf> {
        let tmp_path = self.tmp_path();
        let final_path = self.image_path(photo_id, ext);

        if let Some(parent) = tmp_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create temp directory {}", parent.display()))?;
        }

        if let Some(parent) = final_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create images directory {}", parent.display()))?;
        }

        std::fs::write(&tmp_path, image_data)
            .with_context(|| format!("Failed to write temp image {}", tmp_path.display()))?;

        std::fs::rename(&tmp_path, &final_path).with_context(|| {
            format!(
                "Failed to atomically move image {} -> {}",
                tmp_path.display(),
                final_path.display()
            )
        })?;

        Ok(final_path)
    }

    pub fn delete_image(&self, photo_id: u64, ext: &str) -> Result<()> {
        let image_path = self.image_path(photo_id, ext);
        match std::fs::remove_file(&image_path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err).with_context(|| format!("Failed to delete image {}", image_path.display())),
        }
    }

    pub fn delete_relative_path(&self, relative_path: &str) -> Result<()> {
        let image_path = self.base_path.join(relative_path);
        match std::fs::remove_file(&image_path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err).with_context(|| format!("Failed to delete image {}", image_path.display())),
        }
    }

    pub fn cache_size_bytes(&self) -> Result<u64> {
        let images_dir = self.images_dir();
        if !images_dir.exists() {
            return Ok(0);
        }

        let mut total = 0_u64;
        for entry in std::fs::read_dir(&images_dir)
            .with_context(|| format!("Failed to read images directory {}", images_dir.display()))?
        {
            let entry = entry.context("Failed to read cache file entry")?;
            let metadata = entry.metadata().with_context(|| {
                format!("Failed to read metadata for {}", entry.path().display())
            })?;
            if metadata.is_file() {
                total = total.saturating_add(metadata.len());
            }
        }

        Ok(total)
    }

    pub fn absolute_path_for(&self, relative_path: &str) -> PathBuf {
        self.base_path.join(relative_path)
    }

    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    fn images_dir(&self) -> PathBuf {
        self.base_path.join("images")
    }

    fn tmp_dir(&self) -> PathBuf {
        self.base_path.join("tmp")
    }
}

fn normalize_ext(ext: &str) -> &str {
    ext.trim_start_matches('.')
}

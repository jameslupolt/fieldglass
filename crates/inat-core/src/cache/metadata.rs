use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};

use crate::types::CachedPhoto;

pub struct MetadataStore {
    conn: Connection,
}

impl MetadataStore {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open metadata database at {}", path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .context("Failed to enable SQLite WAL mode")?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .context("Failed to set SQLite synchronous mode")?;
        Ok(Self { conn })
    }

    pub fn open_readonly(path: &Path) -> Result<Self> {
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY;
        let conn = Connection::open_with_flags(path, flags).with_context(|| {
            format!(
                "Failed to open metadata database in read-only mode at {}",
                path.display()
            )
        })?;
        Ok(Self { conn })
    }

    pub fn initialize(&self) -> Result<()> {
        self.conn
            .execute_batch(
                "
                CREATE TABLE IF NOT EXISTS cached_photos (
                    photo_id INTEGER PRIMARY KEY,
                    observation_id INTEGER NOT NULL,
                    file_path TEXT NOT NULL,
                    creator_name TEXT NOT NULL,
                    license_code TEXT NOT NULL,
                    license_display TEXT NOT NULL,
                    observation_url TEXT NOT NULL,
                    common_name TEXT,
                    scientific_name TEXT NOT NULL,
                    place_name TEXT,
                    observed_on TEXT,
                    taxon_id INTEGER,
                    iconic_taxon_name TEXT,
                    observer_username TEXT NOT NULL,
                    photo_width INTEGER,
                    photo_height INTEGER,
                    attribution_text TEXT NOT NULL,
                    diversity_score REAL NOT NULL,
                    cached_at TEXT NOT NULL,
                    pending_deletion INTEGER NOT NULL DEFAULT 0
                );

                CREATE INDEX IF NOT EXISTS idx_cached_photos_pending_deletion
                    ON cached_photos(pending_deletion);

                CREATE INDEX IF NOT EXISTS idx_cached_photos_taxon_id
                    ON cached_photos(taxon_id);

                CREATE INDEX IF NOT EXISTS idx_cached_photos_observer_username
                    ON cached_photos(observer_username);
                ",
            )
            .context("Failed to initialize metadata schema")?;

        Ok(())
    }

    pub fn insert_photo(&self, photo: &CachedPhoto) -> Result<()> {
        self.conn
            .execute(
                "
                INSERT OR REPLACE INTO cached_photos (
                    photo_id,
                    observation_id,
                    file_path,
                    creator_name,
                    license_code,
                    license_display,
                    observation_url,
                    common_name,
                    scientific_name,
                    place_name,
                    observed_on,
                    taxon_id,
                    iconic_taxon_name,
                    observer_username,
                    photo_width,
                    photo_height,
                    attribution_text,
                    diversity_score,
                    cached_at,
                    pending_deletion
                ) VALUES (
                    ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
                );
                ",
                params![
                    u64_to_i64(photo.photo_id, "photo_id")?,
                    u64_to_i64(photo.observation_id, "observation_id")?,
                    photo.file_path,
                    photo.creator_name,
                    photo.license_code,
                    photo.license_display,
                    photo.observation_url,
                    photo.common_name,
                    photo.scientific_name,
                    photo.place_name,
                    photo.observed_on,
                    optional_u64_to_i64(photo.taxon_id, "taxon_id")?,
                    photo.iconic_taxon_name,
                    photo.observer_username,
                    optional_u32_to_i64(photo.photo_width),
                    optional_u32_to_i64(photo.photo_height),
                    photo.attribution_text,
                    photo.diversity_score,
                    photo.cached_at.to_rfc3339(),
                    photo.pending_deletion,
                ],
            )
            .context("Failed to insert cached photo metadata")?;

        Ok(())
    }

    pub fn get_all_photos(&self) -> Result<Vec<CachedPhoto>> {
        let mut stmt = self
            .conn
            .prepare(
                "
                SELECT
                    photo_id,
                    observation_id,
                    file_path,
                    creator_name,
                    license_code,
                    license_display,
                    observation_url,
                    common_name,
                    scientific_name,
                    place_name,
                    observed_on,
                    taxon_id,
                    iconic_taxon_name,
                    observer_username,
                    photo_width,
                    photo_height,
                    attribution_text,
                    diversity_score,
                    cached_at,
                    pending_deletion
                FROM cached_photos
                WHERE pending_deletion = 0
                ",
            )
            .context("Failed to prepare get_all_photos query")?;

        let mut rows = stmt
            .query([])
            .context("Failed to execute get_all_photos query")?;
        let mut photos = Vec::new();

        while let Some(row) = rows.next().context("Failed reading photo row")? {
            photos.push(cached_photo_from_row(row)?);
        }

        Ok(photos)
    }

    pub fn get_photo_ids(&self) -> Result<Vec<u64>> {
        let mut stmt = self
            .conn
            .prepare(
                "
                SELECT photo_id
                FROM cached_photos
                WHERE pending_deletion = 0
                ",
            )
            .context("Failed to prepare get_photo_ids query")?;

        let rows = stmt
            .query_map([], |row| {
                let value: i64 = row.get(0)?;
                i64_to_u64_sql(value, "photo_id")
            })
            .context("Failed to execute get_photo_ids query")?;

        let mut ids = Vec::new();
        for row in rows {
            ids.push(row.context("Failed reading photo_id row")?);
        }

        Ok(ids)
    }

    pub fn get_photo_by_id(&self, photo_id: u64) -> Result<Option<CachedPhoto>> {
        let mut stmt = self
            .conn
            .prepare(
                "
                SELECT
                    photo_id,
                    observation_id,
                    file_path,
                    creator_name,
                    license_code,
                    license_display,
                    observation_url,
                    common_name,
                    scientific_name,
                    place_name,
                    observed_on,
                    taxon_id,
                    iconic_taxon_name,
                    observer_username,
                    photo_width,
                    photo_height,
                    attribution_text,
                    diversity_score,
                    cached_at,
                    pending_deletion
                FROM cached_photos
                WHERE photo_id = ?
                ",
            )
            .context("Failed to prepare get_photo_by_id query")?;

        let photo = stmt
            .query_row(params![u64_to_i64(photo_id, "photo_id")?], cached_photo_from_row)
            .optional()
            .context("Failed to execute get_photo_by_id query")?;

        Ok(photo)
    }

    pub fn mark_pending_deletion(&self, photo_id: u64) -> Result<()> {
        self.conn
            .execute(
                "
                UPDATE cached_photos
                SET pending_deletion = 1
                WHERE photo_id = ?
                ",
                params![u64_to_i64(photo_id, "photo_id")?],
            )
            .context("Failed to mark photo as pending deletion")?;

        Ok(())
    }

    pub fn delete_pending(&self, older_than: DateTime<Utc>) -> Result<u64> {
        let deleted = self
            .conn
            .execute(
                "
                DELETE FROM cached_photos
                WHERE pending_deletion = 1
                  AND cached_at < ?
                ",
                params![older_than.to_rfc3339()],
            )
            .context("Failed to delete pending photos")?;

        Ok(deleted as u64)
    }

    pub fn count(&self) -> Result<u64> {
        let count: i64 = self
            .conn
            .query_row(
                "
                SELECT COUNT(*)
                FROM cached_photos
                WHERE pending_deletion = 0
                ",
                [],
                |row| row.get(0),
            )
            .context("Failed to count active cached photos")?;

        i64_to_u64(count, "count")
    }

    pub fn get_taxon_counts(&self) -> Result<HashMap<u64, u32>> {
        let mut stmt = self
            .conn
            .prepare(
                "
                SELECT taxon_id, COUNT(*)
                FROM cached_photos
                WHERE pending_deletion = 0
                  AND taxon_id IS NOT NULL
                GROUP BY taxon_id
                ",
            )
            .context("Failed to prepare get_taxon_counts query")?;

        let rows = stmt
            .query_map([], |row| {
                let taxon_id: i64 = row.get(0)?;
                let count: i64 = row.get(1)?;
                Ok((
                    i64_to_u64_sql(taxon_id, "taxon_id")?,
                    i64_to_u32_sql(count, "taxon count")?,
                ))
            })
            .context("Failed to execute get_taxon_counts query")?;

        let mut counts = HashMap::new();
        for row in rows {
            let (taxon_id, count) = row.context("Failed reading taxon count row")?;
            counts.insert(taxon_id, count);
        }

        Ok(counts)
    }

    pub fn get_observer_counts(&self) -> Result<HashMap<String, u32>> {
        let mut stmt = self
            .conn
            .prepare(
                "
                SELECT observer_username, COUNT(*)
                FROM cached_photos
                WHERE pending_deletion = 0
                GROUP BY observer_username
                ",
            )
            .context("Failed to prepare get_observer_counts query")?;

        let rows = stmt
            .query_map([], |row| {
                let username: String = row.get(0)?;
                let count: i64 = row.get(1)?;
                Ok((username, i64_to_u32_sql(count, "observer count")?))
            })
            .context("Failed to execute get_observer_counts query")?;

        let mut counts = HashMap::new();
        for row in rows {
            let (username, count) = row.context("Failed reading observer count row")?;
            counts.insert(username, count);
        }

        Ok(counts)
    }

    pub fn get_pending_before(&self, older_than: DateTime<Utc>) -> Result<Vec<CachedPhoto>> {
        let mut stmt = self
            .conn
            .prepare(
                "
                SELECT
                    photo_id,
                    observation_id,
                    file_path,
                    creator_name,
                    license_code,
                    license_display,
                    observation_url,
                    common_name,
                    scientific_name,
                    place_name,
                    observed_on,
                    taxon_id,
                    iconic_taxon_name,
                    observer_username,
                    photo_width,
                    photo_height,
                    attribution_text,
                    diversity_score,
                    cached_at,
                    pending_deletion
                FROM cached_photos
                WHERE pending_deletion = 1
                  AND cached_at < ?
                ",
            )
            .context("Failed to prepare get_pending_before query")?;

        let mut rows = stmt
            .query(params![older_than.to_rfc3339()])
            .context("Failed to execute get_pending_before query")?;
        let mut photos = Vec::new();

        while let Some(row) = rows.next().context("Failed reading pending row")? {
            photos.push(cached_photo_from_row(row)?);
        }

        Ok(photos)
    }

    /// Delete all cached photo metadata (for cache clearing).
    pub fn delete_all(&self) -> Result<u64> {
        let deleted = self
            .conn
            .execute("DELETE FROM cached_photos", [])
            .context("Failed to delete all cached photos")?;
        Ok(deleted as u64)
    }

}

fn cached_photo_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CachedPhoto> {
    let photo_id: i64 = row.get(0)?;
    let observation_id: i64 = row.get(1)?;
    let taxon_id: Option<i64> = row.get(11)?;
    let photo_width: Option<i64> = row.get(14)?;
    let photo_height: Option<i64> = row.get(15)?;
    let cached_at: String = row.get(18)?;

    Ok(CachedPhoto {
        photo_id: i64_to_u64_sql(photo_id, "photo_id")?,
        observation_id: i64_to_u64_sql(observation_id, "observation_id")?,
        file_path: row.get(2)?,
        creator_name: row.get(3)?,
        license_code: row.get(4)?,
        license_display: row.get(5)?,
        observation_url: row.get(6)?,
        common_name: row.get(7)?,
        scientific_name: row.get(8)?,
        place_name: row.get(9)?,
        observed_on: row.get(10)?,
        taxon_id: optional_i64_to_u64_sql(taxon_id, "taxon_id")?,
        iconic_taxon_name: row.get(12)?,
        observer_username: row.get(13)?,
        photo_width: optional_i64_to_u32_sql(photo_width, "photo_width")?,
        photo_height: optional_i64_to_u32_sql(photo_height, "photo_height")?,
        attribution_text: row.get(16)?,
        diversity_score: row.get(17)?,
        cached_at: DateTime::parse_from_rfc3339(&cached_at)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
        pending_deletion: row.get(19)?,
    })
}

fn u64_to_i64(value: u64, field_name: &str) -> Result<i64> {
    i64::try_from(value).map_err(|_| anyhow!("{} is too large for SQLite INTEGER", field_name))
}

fn optional_u64_to_i64(value: Option<u64>, field_name: &str) -> Result<Option<i64>> {
    value.map(|v| u64_to_i64(v, field_name)).transpose()
}

fn i64_to_u64(value: i64, field_name: &str) -> Result<u64> {
    u64::try_from(value).map_err(|_| anyhow!("{} is negative in SQLite row", field_name))
}

fn optional_u32_to_i64(value: Option<u32>) -> Option<i64> {
    value.map(i64::from)
}

// rusqlite-compatible variants for use inside query_map closures
fn i64_to_u64_sql(value: i64, field_name: &str) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|_| {
        rusqlite::Error::ToSqlConversionFailure(
            format!("{} is negative in SQLite row", field_name).into(),
        )
    })
}

fn optional_i64_to_u64_sql(value: Option<i64>, field_name: &str) -> rusqlite::Result<Option<u64>> {
    value.map(|v| i64_to_u64_sql(v, field_name)).transpose()
}

fn i64_to_u32_sql(value: i64, field_name: &str) -> rusqlite::Result<u32> {
    u32::try_from(value).map_err(|_| {
        rusqlite::Error::ToSqlConversionFailure(
            format!("{} is out of range for u32", field_name).into(),
        )
    })
}

fn optional_i64_to_u32_sql(value: Option<i64>, field_name: &str) -> rusqlite::Result<Option<u32>> {
    value.map(|v| i64_to_u32_sql(v, field_name)).transpose()
}

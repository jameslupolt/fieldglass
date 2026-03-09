//! Tauri commands for cache status, manual refresh, and clearing.

use chrono::Utc;
use std::collections::HashSet;

use inat_core::api::observations::ObservationQuery;
use inat_core::cache::CacheStatus;
use inat_core::selection::diversity::{DiversityScorer, select_top_n};
use inat_core::selection::filter::{AnnotationFilter, filter_observations};
use inat_core::types::CachedPhoto;
use inat_core::{ApiClient, CacheManager, PhotoLicense, Settings};

/// Return the current cache status (photo count, required count, disk usage).
#[tauri::command]
pub fn get_cache_status() -> Result<CacheStatus, String> {
    let settings = load_settings()?;
    let manager = CacheManager::new().map_err(|e| e.to_string())?;
    manager.status(&settings).map_err(|e| e.to_string())
}

/// Trigger a cache refresh cycle.
///
/// Fetches observations from iNaturalist, filters them, scores for diversity,
/// downloads images, and fills the cache up to the target size.
#[tauri::command]
pub async fn refresh_cache() -> Result<CacheStatus, String> {
    const MAX_PAGES_PER_TAXON: u32 = 10;

    let settings = load_settings()?;
    let manager = CacheManager::new().map_err(|e| e.to_string())?;

    // Check if we actually need more photos
    if !manager.needs_refresh(&settings) {
        tracing::info!("Cache is already full, skipping refresh");
        return manager.status(&settings).map_err(|e| e.to_string());
    }

    // Must have a location configured
    let location = settings
        .location
        .as_ref()
        .ok_or_else(|| "No location configured. Please set a location in Settings.".to_string())?;

    let client = ApiClient::new("0.1.0").map_err(|e| e.to_string())?;
    let annotation_filter = AnnotationFilter::from_settings(&settings);

    // Build existing diversity counts from cache
    let taxon_counts = manager.get_taxon_counts().map_err(|e| e.to_string())?;
    let observer_counts = manager.get_observer_counts().map_err(|e| e.to_string())?;
    let mut scorer = DiversityScorer::new(taxon_counts, observer_counts);

    let current_count = manager
        .status(&settings)
        .map_err(|e| e.to_string())?
        .total_photos;
    let target = settings.required_cache_size() as u64;
    let needed = target.saturating_sub(current_count);

    if needed == 0 {
        return manager.status(&settings).map_err(|e| e.to_string());
    }

    tracing::info!(
        needed,
        target,
        current = current_count,
        "Starting cache refresh"
    );

    let taxon_ids = if settings.taxon_ids.is_empty() {
        vec![None] // all life
    } else {
        settings.taxon_ids.iter().map(|&id| Some(id)).collect()
    };

    let mut total_added = 0u64;
    let mut seen_photo_ids: HashSet<u64> = manager
        .get_display_queue()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|p| p.photo_id)
        .collect();
    let http_client = reqwest::Client::new();

    for taxon_id in &taxon_ids {
        let mut page = 1u32;
        while total_added < needed && page <= MAX_PAGES_PER_TAXON {
            let query = ObservationQuery {
                lat: location.lat,
                lng: location.lng,
                radius: settings.search_radius.km(),
                taxon_id: *taxon_id,
                quality_grade: if settings.research_grade_only {
                    "research".to_owned()
                } else {
                    "research,needs_id".to_owned()
                },
                photos: true,
                per_page: 200,
                page,
                ..Default::default()
            };

            tracing::info!(
                lat = location.lat,
                lng = location.lng,
                radius = settings.search_radius.km(),
                taxon_id = ?taxon_id,
                page,
                "Fetching observations"
            );

            let response = client
                .search_observations(&query)
                .await
                .map_err(|e| e.to_string())?;

            let returned = response.results.len();
            tracing::info!(
                total_results = response.total_results,
                returned,
                page,
                taxon_id = ?taxon_id,
                "Received observations"
            );

            if returned == 0 {
                break;
            }

            let filtered = filter_observations(response.results, &annotation_filter)
                .into_iter()
                .filter(|obs| {
                    obs.photos
                        .first()
                        .map(|p| !seen_photo_ids.contains(&p.id))
                        .unwrap_or(false)
                })
                .collect();

            // Score and select top candidates
            let remaining_needed = (needed - total_added) as usize;
            let selected = select_top_n(filtered, &mut scorer, remaining_needed);

            // Download each selected observation's photo
            for scored_obs in &selected {
                if total_added >= needed {
                    break;
                }

                let obs = &scored_obs.observation;
                let Some(photo) = obs.photos.first() else {
                    continue;
                };

                if seen_photo_ids.contains(&photo.id) {
                    continue;
                }

                // Build the large photo URL from the square URL
                let Some(ref url) = photo.url else {
                    continue;
                };
                let large_url = url.replace("/square.", "/large.");

                // Determine license
                let license_code = photo
                    .license_code
                    .as_deref()
                    .unwrap_or("cc-by");
                let license = PhotoLicense::from_code(license_code);
                let license_display = license
                    .map(|l| l.display_name())
                    .unwrap_or("Unknown License");

                // Skip ND-licensed photos if using fill mode
                if settings.aspect_ratio_mode == inat_core::types::AspectRatioMode::Fill
                    && license.is_some_and(|l| l.is_no_derivatives())
                {
                    continue;
                }

                // Build attribution text
                let creator = obs
                    .user
                    .as_ref()
                    .and_then(|u| u.name.as_deref())
                    .or(obs.user.as_ref().map(|u| u.login.as_str()))
                    .unwrap_or("Unknown");

                let attribution = format!(
                    "\u{00a9} {}, {}, via iNaturalist",
                    creator, license_display
                );

                let cached_photo = CachedPhoto {
                    photo_id: photo.id,
                    observation_id: obs.id,
                    file_path: String::new(), // filled by CacheManager::add_photo
                    creator_name: creator.to_string(),
                    license_code: license_code.to_string(),
                    license_display: license_display.to_string(),
                    observation_url: obs.uri.clone(),
                    common_name: obs
                        .taxon
                        .as_ref()
                        .and_then(|t| t.preferred_common_name.clone()),
                    scientific_name: obs
                        .taxon
                        .as_ref()
                        .map(|t| t.name.clone())
                        .unwrap_or_else(|| "Unknown species".to_string()),
                    place_name: obs.place_guess.clone(),
                    observed_on: obs
                        .observed_on_details
                        .as_ref()
                        .and_then(|d| d.date.clone()),
                    taxon_id: obs.taxon.as_ref().map(|t| t.id),
                    iconic_taxon_name: obs
                        .taxon
                        .as_ref()
                        .and_then(|t| t.iconic_taxon_name.clone()),
                    observer_username: obs
                        .user
                        .as_ref()
                        .map(|u| u.login.clone())
                        .unwrap_or_else(|| "unknown".to_string()),
                    photo_width: photo.original_dimensions.as_ref().map(|d| d.width),
                    photo_height: photo.original_dimensions.as_ref().map(|d| d.height),
                    attribution_text: attribution,
                    diversity_score: scored_obs.score,
                    cached_at: Utc::now(),
                    pending_deletion: false,
                };

                match manager
                    .storage()
                    .download_image(&http_client, &large_url, photo.id, "jpg")
                    .await
                {
                    Ok(path) => match tokio::fs::read(&path).await {
                        Ok(bytes) => {
                            if let Err(e) = manager.add_photo(cached_photo, &bytes, "jpg") {
                                tracing::warn!(
                                    photo_id = photo.id,
                                    error = %e,
                                    "Failed to add photo to cache"
                                );
                            } else {
                                seen_photo_ids.insert(photo.id);
                                total_added += 1;
                                tracing::debug!(
                                    photo_id = photo.id,
                                    total_added,
                                    "Photo added to cache"
                                );
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                photo_id = photo.id,
                                error = %e,
                                "Failed to read downloaded image"
                            );
                        }
                    },
                    Err(e) => {
                        tracing::warn!(
                            photo_id = photo.id,
                            url = %large_url,
                            error = %e,
                            "Failed to download image"
                        );
                    }
                }
            }

            if returned < query.per_page as usize {
                break;
            }

            page += 1;
        }
    }

    // Clean up any photos pending deletion for over an hour
    let _ = manager.cleanup_pending();

    // Evict lowest-scored if we're over the target
    let status = manager.status(&settings).map_err(|e| e.to_string())?;
    if status.total_photos > target {
        let excess = (status.total_photos - target) as u32;
        let _ = manager.remove_lowest_scored(excess);
    }

    tracing::info!(added = total_added, "Cache refresh complete");
    manager.status(&settings).map_err(|e| e.to_string())
}

/// Delete all cached photos and metadata.
#[tauri::command]
pub fn clear_cache() -> Result<(), String> {
    let manager = CacheManager::new().map_err(|e| e.to_string())?;
    manager.clear_all().map_err(|e| e.to_string())?;
    Ok(())
}

fn load_settings() -> Result<Settings, String> {
    let path = Settings::default_path().map_err(|e| e.to_string())?;
    Settings::load(&path).map_err(|e| e.to_string())
}

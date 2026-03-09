// Typed Tauri invoke wrappers. This is the ONLY interface between the React
// frontend and the Rust backend. The frontend makes no network requests or
// filesystem operations — everything goes through these commands.

import { invoke } from "@tauri-apps/api/core";
import type {
  Settings,
  CacheStatus,
  Taxon,
  GeocodingResult,
  CachedPhoto,
} from "../types/models";

export async function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

export async function updateSettings(settings: Settings): Promise<void> {
  return invoke<void>("update_settings", { settings });
}

export async function getCacheStatus(): Promise<CacheStatus> {
  return invoke<CacheStatus>("get_cache_status");
}

export async function refreshCache(): Promise<CacheStatus> {
  return invoke<CacheStatus>("refresh_cache");
}

export async function clearCache(): Promise<void> {
  return invoke<void>("clear_cache");
}

export async function searchTaxa(query: string): Promise<Taxon[]> {
  return invoke<Taxon[]>("search_taxa", { query });
}

export async function searchLocation(
  query: string,
): Promise<GeocodingResult[]> {
  return invoke<GeocodingResult[]>("search_location", { query });
}

export async function getCachedPhotos(): Promise<CachedPhoto[]> {
  return invoke<CachedPhoto[]>("get_cached_photos");
}

export async function getPhotoDetails(
  photoId: number,
): Promise<CachedPhoto | null> {
  return invoke<CachedPhoto | null>("get_photo_details", {
    photoId,
  });
}

export async function deleteCachedPhoto(photoId: number): Promise<boolean> {
  return invoke<boolean>("delete_cached_photo", { photoId });
}

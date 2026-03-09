// TypeScript interfaces mirroring Rust types from inat-core.
// The React frontend uses these types exclusively — it never constructs
// its own network types or filesystem types.

export interface Location {
  lat: number;
  lng: number;
  display_name: string | null;
}

export type SearchRadius = "Km10" | "Km25" | "Km50" | "Km100";
export type AspectRatioMode = "contain" | "fill";
export type GeocoderBackend = "photon" | "nominatim";

export interface Settings {
  location: Location | null;
  search_radius: SearchRadius;
  taxon_ids: number[];
  photo_duration_secs: number;
  aspect_ratio_mode: AspectRatioMode;
  overlay_opacity: number;
  research_grade_only: boolean;
  licensed_only: boolean;
  exclude_dead: boolean;
  exclude_non_organism: boolean;
  no_repeat_minutes: number;
  monitor_count: number;
  cache_max_items: number;
  cache_refresh_interval_minutes: number;
  geocoder_backend: GeocoderBackend;
  auto_start: boolean;
}

export interface CacheStatus {
  total_photos: number;
  required_photos: number;
  cache_size_bytes: number;
}

export interface Taxon {
  id: number;
  name: string;
  preferred_common_name: string | null;
  iconic_taxon_name: string | null;
  rank: string | null;
  rank_level: number | null;
  default_photo: { square_url: string | null } | null;
  observations_count: number | null;
}

export interface GeocodingResult {
  display_name: string;
  lat: number;
  lng: number;
  country: string | null;
  state: string | null;
  city: string | null;
}

export interface CachedPhoto {
  photo_id: number;
  observation_id: number;
  file_path: string;
  creator_name: string;
  license_code: string;
  license_display: string;
  observation_url: string;
  common_name: string | null;
  scientific_name: string;
  place_name: string | null;
  observed_on: string | null;
  taxon_id: number | null;
  iconic_taxon_name: string | null;
  observer_username: string;
  photo_width: number | null;
  photo_height: number | null;
  attribution_text: string;
  diversity_score: number;
  cached_at: string;
  pending_deletion: boolean;
}

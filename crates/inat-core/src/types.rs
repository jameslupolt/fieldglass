//! Shared domain types for Field Glass.
//!
//! These types represent the core data model: observations, photos, taxa,
//! annotations, and cached photo metadata. They are used across all components.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// iNaturalist API response types
// ---------------------------------------------------------------------------

/// Top-level paginated response from the iNaturalist observations API.
#[derive(Debug, Deserialize)]
pub struct ObservationsResponse {
    pub total_results: u64,
    pub page: u32,
    pub per_page: u32,
    pub results: Vec<Observation>,
}

/// A single iNaturalist observation.
#[derive(Debug, Clone, Deserialize)]
pub struct Observation {
    pub id: u64,
    pub quality_grade: String,
    pub observed_on_details: Option<DateDetails>,
    pub place_guess: Option<String>,
    pub uri: String,
    pub photos: Vec<Photo>,
    pub taxon: Option<Taxon>,
    pub annotations: Vec<Annotation>,
    pub user: Option<ObservationUser>,
}

/// Date details from the observation (iNaturalist returns a structured object).
#[derive(Debug, Clone, Deserialize)]
pub struct DateDetails {
    pub date: Option<String>,
    pub month: Option<u32>,
    pub year: Option<u32>,
}

/// A photo attached to an observation.
#[derive(Debug, Clone, Deserialize)]
pub struct Photo {
    pub id: u64,
    pub url: Option<String>,
    pub attribution: String,
    pub license_code: Option<String>,
    pub original_dimensions: Option<PhotoDimensions>,
}

/// Width and height of a photo.
#[derive(Debug, Clone, Deserialize)]
pub struct PhotoDimensions {
    pub width: u32,
    pub height: u32,
}

/// Taxon information from the observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Taxon {
    pub id: u64,
    pub name: String,
    pub preferred_common_name: Option<String>,
    pub iconic_taxon_name: Option<String>,
    pub rank: Option<String>,
    pub rank_level: Option<f64>,
    pub default_photo: Option<TaxonPhoto>,
    pub observations_count: Option<u64>,
}

/// Simplified photo for taxon autocomplete results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxonPhoto {
    pub square_url: Option<String>,
}

/// An annotation on an observation (e.g., "Alive or Dead", "Evidence of Organism").
#[derive(Debug, Clone, Deserialize)]
pub struct Annotation {
    pub controlled_attribute_id: u64,
    pub controlled_value_id: u64,
}

/// The user who created the observation.
#[derive(Debug, Clone, Deserialize)]
pub struct ObservationUser {
    pub id: u64,
    pub login: String,
    pub name: Option<String>,
}

/// Response from the taxa autocomplete endpoint.
#[derive(Debug, Deserialize)]
pub struct TaxaAutocompleteResponse {
    pub results: Vec<Taxon>,
}

// ---------------------------------------------------------------------------
// Annotation term/value IDs (from iNaturalist)
// ---------------------------------------------------------------------------

/// Well-known annotation term IDs.
pub mod annotation_terms {
    /// "Life Stage" (term_id = 1)
    pub const LIFE_STAGE: u64 = 1;
    /// "Alive or Dead" (term_id = 17)
    pub const ALIVE_OR_DEAD: u64 = 17;
    /// "Evidence of Organism" (term_id = 22)
    pub const EVIDENCE_OF_ORGANISM: u64 = 22;
}

/// Well-known annotation value IDs.
pub mod annotation_values {
    // Life Stage values
    pub const ADULT: u64 = 2;
    pub const LARVA: u64 = 6;
    pub const EGG: u64 = 7;
    pub const JUVENILE: u64 = 8;

    // Alive or Dead values
    pub const ALIVE: u64 = 18;
    pub const DEAD: u64 = 19;
    pub const CANNOT_BE_DETERMINED: u64 = 20;

    // Evidence of Organism values
    pub const ORGANISM: u64 = 24;
    pub const SCAT: u64 = 25;
    pub const TRACK: u64 = 26;
    pub const BONE: u64 = 27;
    pub const MOLT: u64 = 28;
    pub const GALL: u64 = 29;
    pub const EGG_EVIDENCE: u64 = 30;
    pub const HAIR: u64 = 31;
    pub const LEAFMINE: u64 = 32;
    pub const CONSTRUCTION: u64 = 35;
}

// ---------------------------------------------------------------------------
// Photo size variants
// ---------------------------------------------------------------------------

/// Available photo sizes from iNaturalist's static CDN.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PhotoSize {
    Square,
    Small,
    Medium,
    Large,
    Original,
}

impl PhotoSize {
    /// Returns the URL path segment for this size.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Square => "square",
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
            Self::Original => "original",
        }
    }
}

// ---------------------------------------------------------------------------
// Photo license handling
// ---------------------------------------------------------------------------

/// Creative Commons license types we accept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhotoLicense {
    Cc0,
    CcBy,
    CcByNc,
    CcBySa,
    CcByNcSa,
    CcByNd,
    CcByNcNd,
}

impl PhotoLicense {
    /// Parse from iNaturalist's license_code string (e.g., "cc-by-nc").
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "cc0" => Some(Self::Cc0),
            "cc-by" => Some(Self::CcBy),
            "cc-by-nc" => Some(Self::CcByNc),
            "cc-by-sa" => Some(Self::CcBySa),
            "cc-by-nc-sa" => Some(Self::CcByNcSa),
            "cc-by-nd" => Some(Self::CcByNd),
            "cc-by-nc-nd" => Some(Self::CcByNcNd),
            _ => None,
        }
    }

    /// Whether this license forbids derivatives (adaptations).
    ///
    /// ND-licensed photos cannot be cropped or blurred — only displayed unmodified.
    pub fn is_no_derivatives(self) -> bool {
        matches!(self, Self::CcByNd | Self::CcByNcNd)
    }

    /// Human-readable display string (e.g., "CC BY-NC 4.0").
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Cc0 => "CC0 1.0",
            Self::CcBy => "CC BY 4.0",
            Self::CcByNc => "CC BY-NC 4.0",
            Self::CcBySa => "CC BY-SA 4.0",
            Self::CcByNcSa => "CC BY-NC-SA 4.0",
            Self::CcByNd => "CC BY-ND 4.0",
            Self::CcByNcNd => "CC BY-NC-ND 4.0",
        }
    }

    /// The API query parameter value for this license.
    pub fn api_code(self) -> &'static str {
        match self {
            Self::Cc0 => "cc0",
            Self::CcBy => "cc-by",
            Self::CcByNc => "cc-by-nc",
            Self::CcBySa => "cc-by-sa",
            Self::CcByNcSa => "cc-by-nc-sa",
            Self::CcByNd => "cc-by-nd",
            Self::CcByNcNd => "cc-by-nc-nd",
        }
    }
}

// ---------------------------------------------------------------------------
// Cached photo metadata (stored in SQLite)
// ---------------------------------------------------------------------------

/// Metadata for a cached photo, stored in SQLite and read by screensaver hosts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedPhoto {
    /// iNaturalist photo ID.
    pub photo_id: u64,
    /// iNaturalist observation ID.
    pub observation_id: u64,
    /// Path to the cached image file, relative to the cache directory.
    pub file_path: String,
    /// Photo creator display name.
    pub creator_name: String,
    /// License code (e.g., "cc-by-nc").
    pub license_code: String,
    /// Human-readable license display (e.g., "CC BY-NC 4.0").
    pub license_display: String,
    /// URL to the observation on iNaturalist.
    pub observation_url: String,
    /// Common name of the species (if available).
    pub common_name: Option<String>,
    /// Scientific name of the species.
    pub scientific_name: String,
    /// Place name / location description.
    pub place_name: Option<String>,
    /// Date the organism was observed.
    pub observed_on: Option<String>,
    /// iNaturalist taxon ID (for diversity scoring).
    pub taxon_id: Option<u64>,
    /// Iconic taxon group (e.g., "Plantae", "Animalia").
    pub iconic_taxon_name: Option<String>,
    /// Observer username (for diversity scoring).
    pub observer_username: String,
    /// Photo width in pixels.
    pub photo_width: Option<u32>,
    /// Photo height in pixels.
    pub photo_height: Option<u32>,
    /// Pre-formatted attribution string for overlay display.
    pub attribution_text: String,
    /// Diversity score computed during cache fill.
    pub diversity_score: f64,
    /// When this photo was cached.
    pub cached_at: DateTime<Utc>,
    /// Whether this photo is marked for pending deletion.
    pub pending_deletion: bool,
}

// ---------------------------------------------------------------------------
// Geocoding types
// ---------------------------------------------------------------------------

/// A geocoding search result (from Photon or Nominatim).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeocodingResult {
    /// Display name (e.g., "Brooklyn, New York, USA").
    pub display_name: String,
    /// Latitude.
    pub lat: f64,
    /// Longitude.
    pub lng: f64,
    /// Country (if available).
    pub country: Option<String>,
    /// State/province (if available).
    pub state: Option<String>,
    /// City/town (if available).
    pub city: Option<String>,
}

/// A geographic location (lat/lng pair with optional metadata).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub lat: f64,
    pub lng: f64,
    pub display_name: Option<String>,
}

// ---------------------------------------------------------------------------
// Search radius presets
// ---------------------------------------------------------------------------

/// Search radius presets in kilometers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchRadius {
    Km10,
    Km25,
    #[default]
    Km50,
    Km100,
}

impl SearchRadius {
    pub fn km(self) -> u32 {
        match self {
            Self::Km10 => 10,
            Self::Km25 => 25,
            Self::Km50 => 50,
            Self::Km100 => 100,
        }
    }
}


// ---------------------------------------------------------------------------
// Display settings
// ---------------------------------------------------------------------------

/// How to fit photos to the screen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AspectRatioMode {
    /// Contain with letterbox (black bars). Preserves ND license compatibility.
    #[default]
    Contain,
    /// Crop to fill. Excludes ND-licensed photos.
    Fill,
}


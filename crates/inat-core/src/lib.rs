//! # inat-core
//!
//! Shared core library for Field Glass. All network access,
//! data processing, and cache management lives here. Neither the screensaver
//! hosts nor the React frontend make any network requests directly.
//!
//! ## Modules
//!
//! - [`api`] — iNaturalist API client, geocoding integration
//! - [`cache`] — Image cache manager, SQLite metadata store, filesystem ops
//! - [`selection`] — Diversity-aware photo scoring, annotation filtering
//! - [`config`] — User settings persistence
//! - [`types`] — Domain types shared across all components

pub mod api;
pub mod cache;
pub mod config;
pub mod selection;
pub mod types;

// Re-export key types at crate root for convenience.
pub use api::ApiClient;
pub use cache::{CacheManager, CacheStatus, CacheStorage};
pub use config::Settings;
pub use types::{CachedPhoto, GeocodingResult, Location, Observation, PhotoLicense, Taxon};

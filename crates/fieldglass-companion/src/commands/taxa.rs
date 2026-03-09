//! Tauri command for taxa autocomplete search.
//!
//! Delegates to `fieldglass_core::ApiClient::search_taxa`, which calls the
//! iNaturalist taxa/autocomplete endpoint. The React frontend never
//! makes network requests directly.

use fieldglass_core::types::Taxon;
use fieldglass_core::ApiClient;

/// Search iNaturalist taxa by name (autocomplete).
///
/// Returns up to 10 matching taxa with common names, scientific names,
/// and thumbnail URLs for display in the taxa picker.
#[tauri::command]
pub async fn search_taxa(query: String) -> Result<Vec<Taxon>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let client = ApiClient::new("0.1.0").map_err(|e| e.to_string())?;
    let response = client.search_taxa(&query).await.map_err(|e| e.to_string())?;
    Ok(response.results)
}

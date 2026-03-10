use anyhow::{Context, Result};
use url::Url;

use crate::api::client::{ApiClient, INAT_API_BASE_URL};
use crate::types::ObservationsResponse;

pub const DEFAULT_PHOTO_LICENSE: &str =
    "cc-by,cc-by-nc,cc-by-sa,cc-by-nd,cc-by-nc-nd,cc-by-nc-sa,cc0";
pub const DEFAULT_WITHOUT_TERM_VALUE_ID: &str = "19,25,26,27";

#[derive(Debug, Clone)]
pub struct ObservationQuery {
    pub lat: f64,
    pub lng: f64,
    pub radius: u32,
    pub taxon_id: Option<u64>,
    pub quality_grade: String,
    pub photos: bool,
    pub photo_license: String,
    pub without_term_value_id: String,
    pub per_page: u32,
    pub page: u32,
}

impl Default for ObservationQuery {
    fn default() -> Self {
        Self {
            lat: 0.0,
            lng: 0.0,
            radius: 50,
            taxon_id: None,
            quality_grade: "research".to_owned(),
            photos: true,
            photo_license: DEFAULT_PHOTO_LICENSE.to_owned(),
            without_term_value_id: DEFAULT_WITHOUT_TERM_VALUE_ID.to_owned(),
            per_page: 200,
            page: 1,
        }
    }
}

impl ApiClient {
    pub async fn search_observations(&self, query: &ObservationQuery) -> Result<ObservationsResponse> {
        let mut url = Url::parse(&format!("{INAT_API_BASE_URL}/observations"))
            .context("failed to parse observations endpoint URL")?;

        let photo_license = if query.photo_license.trim().is_empty() {
            DEFAULT_PHOTO_LICENSE
        } else {
            query.photo_license.as_str()
        };

        let without_term_value_id = if query.without_term_value_id.trim().is_empty() {
            DEFAULT_WITHOUT_TERM_VALUE_ID
        } else {
            query.without_term_value_id.as_str()
        };

        let per_page = if query.per_page == 0 { 200 } else { query.per_page };

        {
            let mut query_pairs = url.query_pairs_mut();
            query_pairs.append_pair("lat", &query.lat.to_string());
            query_pairs.append_pair("lng", &query.lng.to_string());
            query_pairs.append_pair("radius", &query.radius.to_string());
            query_pairs.append_pair("quality_grade", &query.quality_grade);
            query_pairs.append_pair("photos", &query.photos.to_string());
            query_pairs.append_pair("photo_license", photo_license);
            query_pairs.append_pair("without_term_value_id", without_term_value_id);
            query_pairs.append_pair("per_page", &per_page.to_string());
            query_pairs.append_pair("page", &query.page.to_string());

            if let Some(taxon_id) = query.taxon_id {
                query_pairs.append_pair("taxon_id", &taxon_id.to_string());
            }
        }

        let response = self
            .get(url.as_ref())
            .await?
            .json::<ObservationsResponse>()
            .await
            .context("failed to parse observations API response")?;

        Ok(response)
    }
}

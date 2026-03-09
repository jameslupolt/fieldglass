use anyhow::{Context, Result};
use url::Url;

use crate::api::client::{ApiClient, INAT_API_BASE_URL};
use crate::types::TaxaAutocompleteResponse;

impl ApiClient {
    pub async fn search_taxa(&self, query: &str) -> Result<TaxaAutocompleteResponse> {
        let mut url = Url::parse(&format!("{INAT_API_BASE_URL}/taxa/autocomplete"))
            .context("failed to parse taxa autocomplete endpoint URL")?;

        url.query_pairs_mut().append_pair("q", query);

        let response = self
            .get(url.as_ref())
            .await?
            .json::<TaxaAutocompleteResponse>()
            .await
            .context("failed to parse taxa autocomplete API response")?;

        Ok(response)
    }
}

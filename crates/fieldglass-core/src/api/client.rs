use std::num::NonZeroU32;
use std::sync::Arc;

use anyhow::{Context, Result};
use governor::clock::DefaultClock;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter};

pub const INAT_API_BASE_URL: &str = "https://api.inaturalist.org/v1";
pub const PHOTON_BASE_URL: &str = "https://photon.komoot.io";
pub const NOMINATIM_BASE_URL: &str = "https://nominatim.openstreetmap.org";

type ApiRateLimiter = RateLimiter<NotKeyed, InMemoryState, DefaultClock>;

#[derive(Clone)]
pub struct ApiClient {
    http_client: reqwest::Client,
    rate_limiter: Arc<ApiRateLimiter>,
}

impl ApiClient {
    pub fn new(app_version: &str) -> Result<Self> {
        let user_agent = format!(
            "FieldGlass/{app_version} (https://github.com/jameslupolt/fieldglass)"
        );

        let http_client = reqwest::Client::builder()
            .user_agent(user_agent)
            .build()
            .context("failed to build HTTP client")?;

        let one_request = NonZeroU32::new(1).context("rate limit must be non-zero")?;
        let rate_limiter = Arc::new(RateLimiter::direct(Quota::per_second(one_request)));

        Ok(Self {
            http_client,
            rate_limiter,
        })
    }

    pub async fn get(&self, url: &str) -> Result<reqwest::Response> {
        self.rate_limiter.until_ready().await;

        let response = self
            .http_client
            .get(url)
            .send()
            .await
            .with_context(|| format!("failed to GET {url}"))?
            .error_for_status()
            .with_context(|| format!("non-success status while GET {url}"))?;

        Ok(response)
    }
}

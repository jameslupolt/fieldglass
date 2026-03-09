use std::future::Future;
use std::pin::Pin;

use anyhow::{Context, Result};
use serde_json::Value;
use url::Url;

use crate::api::client::{ApiClient, NOMINATIM_BASE_URL, PHOTON_BASE_URL};
use crate::config::GeocoderBackend;
use crate::types::GeocodingResult;

pub trait Geocoder: Send + Sync {
    fn search<'a>(
        &'a self,
        query: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<GeocodingResult>>> + Send + 'a>>;
}

#[derive(Clone)]
pub struct PhotonGeocoder {
    client: ApiClient,
}

#[derive(Clone)]
pub struct NominatimGeocoder {
    client: ApiClient,
}

impl PhotonGeocoder {
    pub fn new(client: &ApiClient) -> Self {
        Self {
            client: client.clone(),
        }
    }
}

impl NominatimGeocoder {
    pub fn new(client: &ApiClient) -> Self {
        Self {
            client: client.clone(),
        }
    }
}

impl Geocoder for PhotonGeocoder {
    fn search<'a>(
        &'a self,
        query: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<GeocodingResult>>> + Send + 'a>> {
        Box::pin(async move {
            let mut url = Url::parse(&format!("{PHOTON_BASE_URL}/api"))
                .context("failed to parse Photon endpoint URL")?;
            url.query_pairs_mut()
                .append_pair("q", query)
                .append_pair("limit", "5");

            let payload = self
                .client
                .get(url.as_ref())
                .await?
                .json::<Value>()
                .await
                .context("failed to parse Photon response JSON")?;

            let mut results = Vec::new();
            let features = payload
                .get("features")
                .and_then(Value::as_array)
                .context("Photon response missing features array")?;

            for feature in features {
                let coordinates = match feature
                    .get("geometry")
                    .and_then(|geometry| geometry.get("coordinates"))
                    .and_then(Value::as_array)
                {
                    Some(coordinates) if coordinates.len() >= 2 => coordinates,
                    _ => continue,
                };

                let lng = match coordinates.first().and_then(Value::as_f64) {
                    Some(value) => value,
                    None => continue,
                };

                let lat = match coordinates.get(1).and_then(Value::as_f64) {
                    Some(value) => value,
                    None => continue,
                };

                let properties = feature.get("properties");
                let name = properties
                    .and_then(|p| p.get("name"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let country = properties
                    .and_then(|p| p.get("country"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let state = properties
                    .and_then(|p| p.get("state"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let city = properties
                    .and_then(|p| p.get("city"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);

                let mut display_parts: Vec<String> = Vec::new();
                for value in [&name, &city, &state, &country] {
                    if let Some(value) = value
                        && !value.trim().is_empty()
                        && !display_parts.contains(value)
                    {
                        display_parts.push(value.clone());
                    }
                }

                let display_name = if display_parts.is_empty() {
                    format!("{lat}, {lng}")
                } else {
                    display_parts.join(", ")
                };

                results.push(GeocodingResult {
                    display_name,
                    lat,
                    lng,
                    country,
                    state,
                    city,
                });
            }

            Ok(results)
        })
    }
}

impl Geocoder for NominatimGeocoder {
    fn search<'a>(
        &'a self,
        query: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<GeocodingResult>>> + Send + 'a>> {
        Box::pin(async move {
            let mut url = Url::parse(&format!("{NOMINATIM_BASE_URL}/search"))
                .context("failed to parse Nominatim endpoint URL")?;
            url.query_pairs_mut()
                .append_pair("q", query)
                .append_pair("format", "jsonv2")
                .append_pair("limit", "5");

            let payload = self
                .client
                .get(url.as_ref())
                .await?
                .json::<Value>()
                .await
                .context("failed to parse Nominatim response JSON")?;

            let entries = payload
                .as_array()
                .context("Nominatim response was not a JSON array")?;

            let mut results = Vec::new();

            for entry in entries {
                let display_name = match entry.get("display_name").and_then(Value::as_str) {
                    Some(value) if !value.trim().is_empty() => value.to_owned(),
                    _ => continue,
                };

                let lat = match entry
                    .get("lat")
                    .and_then(Value::as_str)
                    .and_then(|value| value.parse::<f64>().ok())
                {
                    Some(value) => value,
                    None => continue,
                };

                let lng = match entry
                    .get("lon")
                    .and_then(Value::as_str)
                    .and_then(|value| value.parse::<f64>().ok())
                {
                    Some(value) => value,
                    None => continue,
                };

                let address = entry.get("address");
                let country = address
                    .and_then(|a| a.get("country"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let state = address
                    .and_then(|a| a.get("state"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);

                let city = address
                    .and_then(|a| a.get("city"))
                    .or_else(|| address.and_then(|a| a.get("town")))
                    .or_else(|| address.and_then(|a| a.get("village")))
                    .and_then(Value::as_str)
                    .map(str::to_owned);

                results.push(GeocodingResult {
                    display_name,
                    lat,
                    lng,
                    country,
                    state,
                    city,
                });
            }

            Ok(results)
        })
    }
}

pub fn create_geocoder(backend: GeocoderBackend, client: &ApiClient) -> Box<dyn Geocoder> {
    match backend {
        GeocoderBackend::Photon => Box::new(PhotonGeocoder::new(client)),
        GeocoderBackend::Nominatim => Box::new(NominatimGeocoder::new(client)),
    }
}

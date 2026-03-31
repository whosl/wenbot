//! National Weather Service (NWS) API integration
//!
//! Provides gridpoint forecast lookup for US locations

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// NWS forecast data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NwsForecast {
    pub city_name: String,
    pub lat: f64,
    pub lon: f64,
    pub high_temp_f: Option<f64>,
    pub low_temp_f: Option<f64>,
    pub valid_time: String,
}

/// NWS API client
pub struct NwsClient {
    client: reqwest::Client,
}

impl NwsClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .user_agent("wenbot-rust/0.1.0")
                .build()
                .unwrap_or_default(),
        }
    }

    /// Fetch NWS forecast for a US city using coordinates
    /// Returns None if location is outside NWS coverage or API fails
    pub async fn fetch_forecast(&self, lat: f64, lon: f64, city_name: &str) -> Option<NwsForecast> {
        // NWS only covers US (approximate bounds)
        if !(24.0..=50.0).contains(&lat) || !(-125.0..=-65.0).contains(&lon) {
            return None;
        }

        // First, get the gridpoint URL for these coordinates
        let points_url = format!(
            "https://api.weather.gov/points/{:.4},{:.4}",
            lat, lon
        );

        let points_resp = self.client.get(&points_url).send().await.ok()?;
        if !points_resp.status().is_success() {
            warn!("NWS points API failed for ({}, {}): {}", lat, lon, points_resp.status());
            return None;
        }

        let points_data: serde_json::Value = points_resp.json().await.ok()?;
        let forecast_url = points_data
            .get("properties")
            .and_then(|p| p.get("forecast"))
            .and_then(|f| f.as_str())?;

        // Fetch the forecast
        let forecast_resp = self.client.get(forecast_url).send().await.ok()?;
        if !forecast_resp.status().is_success() {
            warn!("NWS forecast API failed for {}: {}", city_name, forecast_resp.status());
            return None;
        }

        let forecast_data: serde_json::Value = forecast_resp.json().await.ok()?;
        let periods = forecast_data
            .get("properties")
            .and_then(|p| p.get("periods"))
            .and_then(|p| p.as_array())?;

        // Get today's forecast (first period is usually daytime)
        if let Some(today_period) = periods.first() {
            let temp = today_period.get("temperature").and_then(|t| t.as_f64());
            let temp_unit = today_period.get("temperatureUnit").and_then(|u| u.as_str());
            let is_fahrenheit = temp_unit == Some("F");

            let high_temp_f = if is_fahrenheit { temp } else {
                temp.map(|c| c * 9.0 / 5.0 + 32.0)
            };

            // Try to get tonight's low temperature (second period is usually nighttime)
            let low_temp_f = if let Some(night_period) = periods.get(1) {
                let night_temp = night_period.get("temperature").and_then(|t| t.as_f64());
                let night_unit = night_period.get("temperatureUnit").and_then(|u| u.as_str());
                let night_is_f = night_unit == Some("F");
                if night_is_f { night_temp } else {
                    night_temp.map(|c| c * 9.0 / 5.0 + 32.0)
                }
            } else {
                None
            };

            let valid_time = today_period
                .get("startTime")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();

            info!("NWS forecast for {}: high={:?}°F, low={:?}°F", city_name, high_temp_f, low_temp_f);

            return Some(NwsForecast {
                city_name: city_name.to_string(),
                lat,
                lon,
                high_temp_f,
                low_temp_f,
                valid_time,
            });
        }

        None
    }

    /// Get NWS gridpoint info for coordinates
    pub async fn get_gridpoint_info(&self, lat: f64, lon: f64) -> Option<(String, String)> {
        if !(24.0..=50.0).contains(&lat) || !(-125.0..=-65.0).contains(&lon) {
            return None;
        }

        let points_url = format!(
            "https://api.weather.gov/points/{:.4},{:.4}",
            lat, lon
        );

        let resp = self.client.get(&points_url).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }

        let data: serde_json::Value = resp.json().await.ok()?;
        let props = data.get("properties")?;

        let forecast_url = props.get("forecast")
            .and_then(|f| f.as_str())
            .unwrap_or("")
            .to_string();

        let grid_id = props.get("cwa")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();

        Some((grid_id, forecast_url))
    }
}

impl Default for NwsClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires network access
    async fn test_nws_fetch_austin() {
        let client = NwsClient::new();
        // Austin, TX coordinates
        let forecast = client.fetch_forecast(30.2672, -97.7431, "Austin").await;
        assert!(forecast.is_some());
    }
}

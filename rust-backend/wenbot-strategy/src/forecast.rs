//! Weather forecast data types and providers

use crate::error::{Result, WeatherError};
use serde::{Deserialize, Serialize};

/// Ensemble weather forecast
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleForecast {
    /// City key (e.g. "nyc")
    pub city_key: String,
    /// Target date (YYYY-MM-DD)
    pub target_date: String,
    /// Individual member predictions for high temperature (°F)
    pub member_highs: Vec<f64>,
    /// Individual member predictions for low temperature (°F)
    pub member_lows: Vec<f64>,
    /// Number of ensemble members
    pub num_members: usize,
}

impl EnsembleForecast {
    /// Mean of ensemble member highs
    pub fn mean_high(&self) -> f64 {
        if self.member_highs.is_empty() {
            return 0.0;
        }
        self.member_highs.iter().sum::<f64>() / self.member_highs.len() as f64
    }

    /// Standard deviation of ensemble member highs
    pub fn std_high(&self) -> f64 {
        if self.member_highs.is_empty() {
            return 0.0;
        }
        let mean = self.mean_high();
        let variance = self.member_highs.iter()
            .map(|h| (h - mean).powi(2))
            .sum::<f64>()
            / self.member_highs.len() as f64;
        variance.sqrt()
    }

    /// Mean of ensemble member lows
    pub fn mean_low(&self) -> f64 {
        if self.member_lows.is_empty() {
            return 0.0;
        }
        self.member_lows.iter().sum::<f64>() / self.member_lows.len() as f64
    }

    /// Standard deviation of ensemble member lows
    pub fn std_low(&self) -> f64 {
        if self.member_lows.is_empty() {
            return 0.0;
        }
        let mean = self.mean_low();
        let variance = self.member_lows.iter()
            .map(|l| (l - mean).powi(2))
            .sum::<f64>()
            / self.member_lows.len() as f64;
        variance.sqrt()
    }

    /// Fraction of members with high above threshold (>=)
    pub fn probability_high_above(&self, threshold: f64) -> f64 {
        if self.member_highs.is_empty() {
            return 0.5;
        }
        self.member_highs.iter().filter(|&&h| h >= threshold).count() as f64
            / self.member_highs.len() as f64
    }

    /// Fraction of members with high below threshold (<=)
    pub fn probability_high_below(&self, threshold: f64) -> f64 {
        if self.member_highs.is_empty() {
            return 0.5;
        }
        self.member_highs.iter().filter(|&&h| h <= threshold).count() as f64
            / self.member_highs.len() as f64
    }

    /// Fraction of members with high between low and high
    pub fn probability_high_between(&self, low: f64, high: f64) -> f64 {
        if self.member_highs.is_empty() {
            return 0.5;
        }
        self.member_highs.iter()
            .filter(|&&h| low <= h && h <= high)
            .count() as f64
            / self.member_highs.len() as f64
    }

    /// Fraction of members with low above threshold (>=)
    pub fn probability_low_above(&self, threshold: f64) -> f64 {
        if self.member_lows.is_empty() {
            return 0.5;
        }
        self.member_lows.iter().filter(|&&l| l >= threshold).count() as f64
            / self.member_lows.len() as f64
    }

    /// Fraction of members with low below threshold (<=)
    pub fn probability_low_below(&self, threshold: f64) -> f64 {
        if self.member_lows.is_empty() {
            return 0.5;
        }
        self.member_lows.iter().filter(|&&l| l <= threshold).count() as f64
            / self.member_lows.len() as f64
    }

    /// Fraction of members with low between low and high
    pub fn probability_low_between(&self, low: f64, high: f64) -> f64 {
        if self.member_lows.is_empty() {
            return 0.5;
        }
        self.member_lows.iter()
            .filter(|&&l| low <= l && l <= high)
            .count() as f64
            / self.member_lows.len() as f64
    }

    /// Ensemble agreement: fraction of members on the majority side of the mean.
    /// Returns 0.0-1.0 where 1.0 = perfect agreement.
    pub fn agreement(&self) -> f64 {
        let mean = self.mean_high();
        if self.member_highs.is_empty() {
            return 0.0;
        }
        let above = self.member_highs.iter().filter(|&&h| h >= mean).count();
        let below = self.member_highs.len() - above;
        above.max(below) as f64 / self.member_highs.len() as f64
    }
}

/// NWS (National Weather Service) deterministic forecast
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NwsForecast {
    pub city_key: String,
    pub target_date: String,
    pub high: Option<f64>,
    pub low: Option<f64>,
}

/// Forecast provider trait
#[async_trait::async_trait]
pub trait ForecastProvider: Send + Sync {
    async fn fetch_ensemble(&self, city_key: &str, target_date: &str) -> Result<EnsembleForecast>;

    async fn fetch_nws(&self, city_key: &str, target_date: &str) -> Result<Option<NwsForecast>>;
}

/// Open-Meteo ensemble forecast provider
pub struct OpenMeteoProvider {
    client: reqwest::Client,
}

impl OpenMeteoProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_forecast() -> EnsembleForecast {
        EnsembleForecast {
            city_key: "nyc".into(),
            target_date: "2026-03-28".into(),
            member_highs: vec![70.0, 72.0, 68.0, 71.0, 69.0, 73.0, 67.0, 70.0, 71.0, 72.0],
            member_lows: vec![55.0, 57.0, 53.0, 56.0, 54.0, 58.0, 52.0, 55.0, 56.0, 57.0],
            num_members: 10,
        }
    }

    #[test]
    fn test_mean_and_std() {
        let fc = make_forecast();
        let mean = fc.mean_high();
        let std = fc.std_high();
        assert!((mean - 70.2).abs() < 0.1);
        assert!(std > 0.0);
    }

    #[test]
    fn test_probability_high_above() {
        let fc = make_forecast();
        let prob = fc.probability_high_above(70.0);
        // 6 out of 10 are >= 70
        assert!((prob - 0.6).abs() < 0.1);
    }

    #[test]
    fn test_probability_high_below() {
        let fc = make_forecast();
        let prob = fc.probability_high_below(70.0);
        // 4 out of 10 are < 70
        assert!((prob - 0.4).abs() < 0.1);
    }

    #[test]
    fn test_probability_high_between() {
        let fc = make_forecast();
        let prob = fc.probability_high_between(68.0, 72.0);
        // 7 out of 10 are in range [68, 72]
        assert!((prob - 0.7).abs() < 0.2);
    }
}

//! Error types for the wenbot strategy

use thiserror::Error;

#[derive(Error, Debug)]
pub enum WeatherError {
    #[error("Insufficient edge: {:.2} < {:.2}", edge, threshold)]
    InsufficientEdge { edge: f64, threshold: f64 },

    #[error("Entry price too high: {:.2} > {:.2}", price, max)]
    EntryPriceTooHigh { price: f64, max: f64 },

    #[error("Forecast unavailable for {} on {}", city, date)]
    ForecastUnavailable { city: String, date: String },

    #[error("No suitable markets found")]
    NoMarketsFound,

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Settlement error: {0}")]
    SettlementError(String),

    #[error("Cooldown active for {}: retry after {}s", market, retry_after_seconds)]
    CooldownActive { market: String, retry_after_seconds: u64 },

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, WeatherError>;

//! Wenbot Strategy — Weather temperature prediction trading
//!
//! Core strategy components:
//! - Weather market scanning (Polymarket Gamma API)
//! - Temperature prediction (ensemble forecast)
//! - Signal generation with edge calculation
//! - NWS cross-validation
//! - Kelly criterion position sizing
//! - GTC order lifecycle management
//! - Failure cooldown mechanism
//!
//! Depends on: virtual-wallet, polymarket-client

mod config;
mod error;
mod forecast;
mod markets;
mod signals;
mod strategy;

pub use config::WenbotConfig;
pub use error::{WeatherError, Result};
pub use forecast::{EnsembleForecast, ForecastProvider, NwsForecast};
pub use markets::WeatherMarketInfo;
pub use signals::{WeatherSignal, SignalDirection};
pub use strategy::WenbotStrategy;

/// Default list of cities to monitor
pub const DEFAULT_CITIES: &[&str] = &[
    "nyc", "chicago", "miami", "los_angeles", "denver", "seattle",
    "atlanta", "dallas", "london", "paris", "seoul", "sao_paulo",
    "wellington", "tokyo", "toronto", "ankara", "shanghai",
    "hong_kong", "tel_aviv", "munich", "buenos_aires", "lucknow",
    "madrid", "milan", "singapore", "warsaw", "austin", "san_francisco",
];

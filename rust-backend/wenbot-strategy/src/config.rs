//! Wenbot configuration

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WenbotConfig {
    pub enabled: bool,
    pub scan_interval_seconds: u64,
    pub settlement_interval_seconds: u64,
    pub min_edge_threshold: f64,
    pub max_entry_price: f64,
    pub max_trade_size: f64,
    pub kelly_fraction: f64,
    pub kelly_max_fraction: f64,
    pub kelly_lookback_trades: usize,
    pub cities: Vec<String>,
    pub settlement_source: String,
    pub simulation_mode: bool,
    pub order_management_enabled: bool,
    pub order_management_interval_seconds: u64,
    pub order_price_change_threshold: f64,
    pub daily_loss_limit: f64,
    pub total_exposure_cap_ratio: f64,
    pub low_balance_threshold: f64,
    pub api_failure_alert_threshold: usize,
}

impl Default for WenbotConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            scan_interval_seconds: 300,
            settlement_interval_seconds: 1800,
            min_edge_threshold: 0.08,
            max_entry_price: 0.70,
            max_trade_size: 100.0,
            kelly_fraction: 0.15,
            kelly_max_fraction: 0.05,
            kelly_lookback_trades: 20,
            cities: super::DEFAULT_CITIES.iter().map(|s| s.to_string()).collect(),
            settlement_source: "wunderground".into(),
            simulation_mode: true,
            order_management_enabled: true,
            order_management_interval_seconds: 300,
            order_price_change_threshold: 0.30,
            daily_loss_limit: 20.0,
            total_exposure_cap_ratio: 0.6,
            low_balance_threshold: 10.0,
            api_failure_alert_threshold: 3,
        }
    }
}

impl WenbotConfig {
    pub fn from_env() -> Self {
        let mut cfg = Self::default();

        cfg.enabled = env_bool("WEATHER_ENABLED", cfg.enabled);
        cfg.scan_interval_seconds = env_u64("WEATHER_SCAN_INTERVAL_SECONDS", cfg.scan_interval_seconds);
        cfg.settlement_interval_seconds = env_u64("WEATHER_SETTLEMENT_INTERVAL_SECONDS", cfg.settlement_interval_seconds);
        cfg.min_edge_threshold = env_f64("WEATHER_MIN_EDGE_THRESHOLD", cfg.min_edge_threshold);
        cfg.max_entry_price = env_f64("WEATHER_MAX_ENTRY_PRICE", cfg.max_entry_price);
        cfg.max_trade_size = env_f64("WEATHER_MAX_TRADE_SIZE", cfg.max_trade_size);
        cfg.kelly_fraction = env_f64("WEATHER_KELLY_FRACTION", cfg.kelly_fraction);
        cfg.kelly_max_fraction = env_f64("WEATHER_KELLY_MAX_FRACTION", cfg.kelly_max_fraction);
        cfg.kelly_lookback_trades = env_usize("WEATHER_KELLY_LOOKBACK_TRADES", cfg.kelly_lookback_trades);
        cfg.daily_loss_limit = env_f64("WEATHER_DAILY_LOSS_LIMIT", cfg.daily_loss_limit);
        cfg.total_exposure_cap_ratio = env_f64("WEATHER_TOTAL_EXPOSURE_CAP_RATIO", cfg.total_exposure_cap_ratio);
        cfg.low_balance_threshold = env_f64("WEATHER_LOW_BALANCE_THRESHOLD", cfg.low_balance_threshold);
        cfg.api_failure_alert_threshold = env_usize("WEATHER_API_FAILURE_ALERT_THRESHOLD", cfg.api_failure_alert_threshold);

        if let Ok(v) = std::env::var("WEATHER_CITIES") {
            cfg.cities = v.split(',').map(|s| s.trim().to_string()).collect();
        }

        cfg.simulation_mode = env_bool("SIMULATION_MODE", cfg.simulation_mode);
        cfg.settlement_source = std::env::var("SETTLEMENT_SOURCE")
            .unwrap_or_else(|_| "wunderground".into());

        cfg
    }
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn env_f64(name: &str, default: f64) -> f64 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(default)
}

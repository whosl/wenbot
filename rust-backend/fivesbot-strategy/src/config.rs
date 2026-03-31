//! Configuration for the fivesbot strategy

use serde::{Deserialize, Serialize};

/// Fivesbot configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FivesbotConfig {
    /// Markets to trade (e.g. ["btc", "eth"])
    pub markets: Vec<String>,

    /// Shares per side (number of token shares to buy per trade)
    pub shares_per_side: u32,

    /// Tick size for order prices
    pub tick_size: f64,

    /// Use neg-risk markets
    pub neg_risk: bool,

    /// Price buffer in cents for order execution
    pub price_buffer: f64,

    /// Fire and forget (don't wait for order confirmation)
    pub fire_and_forget: bool,

    /// Minimum USDC balance before stopping
    pub min_balance_usdc: f64,

    /// Max buy counts per side per market (0 = no cap)
    pub max_buy_counts_per_side: u32,

    /// Only trade when fewer seconds remain (0 = no restriction)
    pub min_seconds_remaining: u32,

    /// **Reverse prediction signal**: up → buy down, down → buy up
    pub invert_prediction_signal: bool,

    /// Kelly fraction (e.g. 0.15 = 15% of bankroll)
    pub kelly_fraction: f64,

    /// Kelly max fraction cap (e.g. 0.05 = max 5% per trade)
    pub kelly_max_fraction: f64,

    /// Maximum trade size in USD
    pub max_trade_size: f64,

    /// Minimum edge threshold (e.g. 0.06 = 6%)
    pub min_edge_threshold: f64,

    /// Maximum entry price (e.g. 0.55 = don't buy above 55c)
    pub max_entry_price: f64,

    /// Minimum time remaining (seconds) — don't trade near-expiry windows
    pub min_time_remaining: u32,

    /// Maximum time remaining (seconds) — don't trade fresh windows
    pub max_time_remaining: u32,

    /// Max trades per 15-min window
    pub max_trades_per_window: u32,

    /// Simulation mode (use virtual wallet instead of real CLOB)
    pub simulation_mode: bool,
}

impl Default for FivesbotConfig {
    fn default() -> Self {
        Self {
            markets: vec!["btc".into()],
            shares_per_side: 5,
            tick_size: 0.01,
            neg_risk: false,
            price_buffer: 0.0,
            fire_and_forget: true,
            min_balance_usdc: 1.0,
            max_buy_counts_per_side: 0,
            min_seconds_remaining: 0,
            invert_prediction_signal: false,
            kelly_fraction: 0.15,
            kelly_max_fraction: 0.05,
            max_trade_size: 75.0,
            min_edge_threshold: 0.06,
            max_entry_price: 0.55,
            min_time_remaining: 60,
            max_time_remaining: 1800,
            max_trades_per_window: 1,
            simulation_mode: true,
        }
    }
}

impl FivesbotConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let mut cfg = Self::default();

        if let Ok(v) = std::env::var("COPYTRADE_MARKETS") {
            cfg.markets = v.split(',').map(|s| s.trim().to_lowercase()).collect();
        }

        cfg.shares_per_side = env_u32("COPYTRADE_SHARES", cfg.shares_per_side);
        cfg.tick_size = env_f64("COPYTRADE_TICK_SIZE", cfg.tick_size);
        cfg.neg_risk = env_bool("COPYTRADE_NEG_RISK", cfg.neg_risk);
        cfg.price_buffer = env_f64("COPYTRADE_PRICE_BUFFER", cfg.price_buffer);
        cfg.fire_and_forget = env_bool("COPYTRADE_FIRE_AND_FORGET", cfg.fire_and_forget);
        cfg.min_balance_usdc = env_f64("COPYTRADE_MIN_BALANCE_USDC", cfg.min_balance_usdc);
        cfg.max_buy_counts_per_side = env_u32("COPYTRADE_MAX_BUY_COUNTS_PER_SIDE", cfg.max_buy_counts_per_side);
        cfg.min_seconds_remaining = env_u32("COPYTRADE_MIN_SECONDS_REMAINING", cfg.min_seconds_remaining);
        cfg.invert_prediction_signal = env_bool("COPYTRADE_INVERT_PREDICTION_SIGNAL", cfg.invert_prediction_signal);

        // Strategy params
        cfg.kelly_fraction = env_f64("KELLY_FRACTION", cfg.kelly_fraction);
        cfg.kelly_max_fraction = env_f64("KELLY_MAX_FRACTION", cfg.kelly_max_fraction);
        cfg.max_trade_size = env_f64("MAX_TRADE_SIZE", cfg.max_trade_size);
        cfg.min_edge_threshold = env_f64("MIN_EDGE_THRESHOLD", cfg.min_edge_threshold);
        cfg.max_entry_price = env_f64("MAX_ENTRY_PRICE", cfg.max_entry_price);

        cfg.simulation_mode = env_bool("SIMULATION_MODE", cfg.simulation_mode);

        cfg
    }

    /// Get the effective direction, applying the invert signal if configured
    pub fn effective_direction<'a>(&self, predicted: &'a str) -> &'a str {
        if self.invert_prediction_signal {
            match predicted {
                "up" => "down",
                "down" => "up",
                other => other,
            }
        } else {
            predicted
        }
    }
}

fn env_u32(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_f64(name: &str, default: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(default)
}

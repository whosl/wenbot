//! Fivesbot Strategy — BTC/ETH Up/Down 15-minute prediction trading
//!
//! Core strategy components:
//! - Price prediction (15-min window, adaptive predictor)
//! - Signal generation (BUY_UP / BUY_DOWN / HOLD)
//! - Reverse signal support (configurable invert)
//! - Kelly criterion position sizing
//! - Edge threshold filtering
//! - Slippage protection
//!
//! Depends on: virtual-wallet, polymarket-client

mod config;
mod error;
mod indicators;
mod predictor;
mod signals;
mod strategy;

pub use config::FivesbotConfig;
pub use error::{StrategyError, Result};
pub use indicators::{BtcMicrostructure, Candle, compute_microstructure};
pub use predictor::AdaptivePricePredictor;
pub use signals::{calculate_edge, calculate_kelly_size, IndicatorScores, TradingSignal, SignalAction};
pub use strategy::{FivesbotStrategy, StrategyProfile};

/// The default markets to trade
pub const DEFAULT_MARKETS: &[&str] = &["btc", "eth"];

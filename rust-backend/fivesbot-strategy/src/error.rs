//! Error types for the fivesbot strategy

use thiserror::Error;

#[derive(Error, Debug)]
pub enum StrategyError {
    #[error("Insufficient edge: {:.2} < {:.2}", edge, threshold)]
    InsufficientEdge { edge: f64, threshold: f64 },

    #[error("Entry price too high: {:.2} > {:.2}", price, max)]
    EntryPriceTooHigh { price: f64, max: f64 },

    #[error("Insufficient balance: {:.2} < {:.2}", have, need)]
    InsufficientBalance { have: f64, need: f64 },

    #[error("Market too close to expiry: {}s < {}s", remaining, min)]
    TooCloseToExpiry { remaining: u32, min: u32 },

    #[error("Market too fresh: {}s > {}s", remaining, max)]
    TooFresh { remaining: u32, max: u32 },

    #[error("Max trades reached for window")]
    MaxTradesReached,

    #[error("Market not found: {0}")]
    MarketNotFound(String),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Wallet error: {0}")]
    WalletError(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, StrategyError>;

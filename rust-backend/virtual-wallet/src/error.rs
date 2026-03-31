//! Error types for the virtual wallet

use thiserror::Error;

#[derive(Error, Debug)]
pub enum WalletError {
    #[error("Insufficient balance: need ${need:.2}, have ${have:.2}")]
    InsufficientBalance { need: f64, have: f64 },

    #[error("Invalid trade input: {0}")]
    InvalidInput(String),

    #[error("Position not found: id={0}")]
    PositionNotFound(i64),

    #[error("Wallet not initialized")]
    NotInitialized,

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, WalletError>;

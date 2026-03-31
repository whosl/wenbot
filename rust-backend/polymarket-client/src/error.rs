//! Error types for the Polymarket client

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("API error: status={status}, body={body}")]
    ApiError { status: u16, body: String },

    #[error("Authentication error: {0}")]
    AuthError(String),

    #[error("Invalid response format: {0}")]
    ParseError(String),

    #[error("Rate limited: retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: u64 },

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("WebSocket error: {0}")]
    WebSocketError(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, ClientError>;

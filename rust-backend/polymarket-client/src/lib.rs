//! Polymarket CLOB Client — Polymarket 交互层
//!
//! 提供策略层所需的所有 Polymarket 操作：
//! - CLOB API 认证 (API key + HMAC signature)
//! - 市场信息获取
//! - 下单 / 撤单
//! - 余额 / 持仓查询
//! - WebSocket orderbook 订阅
//! - Proxy wallet 支持 (signature_type=1)

mod auth;
mod client;
mod error;
mod types;

pub use auth::ApiCredentials;
pub use client::PolymarketClient;
pub use error::{ClientError, Result};
pub use types::*;

/// 客户端配置
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// CLOB API host (default: https://clob.polymarket.com)
    pub host: String,
    /// Chain ID (default: 137 = Polygon mainnet)
    pub chain_id: u64,
    /// API credentials (from env vars)
    pub credentials: ApiCredentials,
    /// Optional HTTP proxy
    pub proxy: Option<String>,
}

impl ClientConfig {
    /// Load configuration from environment variables.
    ///
    /// Required env vars:
    /// - `POLYMARKET_API_KEY`
    /// - `POLYMARKET_API_SECRET`
    /// - `POLYMARKET_API_PASSPHRASE`
    ///
    /// Optional:
    /// - `POLYMARKET_CLOB_HOST` (default: https://clob.polymarket.com)
    /// - `POLYMARKET_CLOB_CHAIN_ID` (default: 137)
    /// - `HTTPS_PROXY` / `https_proxy`
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            host: std::env::var("POLYMARKET_CLOB_HOST")
                .unwrap_or_else(|_| "https://clob.polymarket.com".into()),
            chain_id: std::env::var("POLYMARKET_CLOB_CHAIN_ID")
                .unwrap_or_else(|_| "137".into())
                .parse()?,
            credentials: ApiCredentials::from_env()?,
            proxy: std::env::var("HTTPS_PROXY")
                .or_else(|_| std::env::var("https_proxy"))
                .or_else(|_| std::env::var("HTTP_PROXY"))
                .or_else(|_| std::env::var("http_proxy"))
                .ok(),
        })
    }
}

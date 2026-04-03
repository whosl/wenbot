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
mod eip712;
mod error;
mod types;

use anyhow::Context;

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
    /// Wallet address / EOA address (checksummed, 0x-prefixed)
    pub address: String,
    /// Private key used for EIP-712 order signing
    pub private_key: Option<String>,
    /// Proxy wallet (funder) address for POLY_PROXY signature type.
    /// For MetaMask + Polymarket proxy wallets, this is the proxy contract address
    /// that holds the actual USDC balance.
    pub funder_address: Option<String>,
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
    /// - `POLYMARKET_ADDRESS` (wallet address)
    /// - `HTTPS_PROXY` / `https_proxy`
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            host: std::env::var("POLYMARKET_CLOB_HOST")
                .unwrap_or_else(|_| "https://clob.polymarket.com".into()),
            chain_id: std::env::var("POLYMARKET_CLOB_CHAIN_ID")
                .unwrap_or_else(|_| "137".into())
                .parse()?,
            credentials: ApiCredentials::from_env()?,
            address: std::env::var("POLYMARKET_ADDRESS")
                .context("POLYMARKET_ADDRESS not set")?,
            private_key: std::env::var("POLYMARKET_PRIVATE_KEY").ok(),
            funder_address: std::env::var("POLYMARKET_FUNDER_ADDRESS").ok(),
            proxy: std::env::var("HTTPS_PROXY")
                .or_else(|_| std::env::var("https_proxy"))
                .or_else(|_| std::env::var("HTTP_PROXY"))
                .or_else(|_| std::env::var("http_proxy"))
                .ok(),
        })
    }
}

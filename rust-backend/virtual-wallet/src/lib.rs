//! Virtual Wallet — 虚拟钱包层
//!
//! 包装所有虚拟钱包操作：
//! - 虚拟资金管理（充值、扣款、余额查询）
//! - 持仓管理（开仓、平仓、持仓查询、PnL 计算）
//! - 交易历史记录
//! - 自动计算 Polymarket 手续费（官方 fee curve）
//! - SQLite 持久化
//! - 线程安全

mod db;
mod error;
mod fee;
mod manager;
mod models;
mod valuation;

pub use db::WalletDatabase;
pub use error::{WalletError, Result};
pub use fee::FeeService;
pub use manager::VirtualWallet;
pub use models::*;
pub use valuation::{mark_to_market, unrealized_pnl, effective_price_for_direction, mark_to_market_from_summary, unrealized_pnl_from_summary};

use serde::{Deserialize, Serialize};

/// Wallet configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletConfig {
    /// Initial balance in USD (default: 100.0)
    pub initial_balance: f64,
    /// Database path (default: virtual_wallet.db in current dir)
    pub database_url: String,
    /// Wallet identifier used to isolate rows in shared SQLite DB (default: wallet1)
    pub wallet_id: String,
    /// Default fee parameter for Polymarket (default: 0.25)
    pub default_fee_param: f64,
}

impl Default for WalletConfig {
    fn default() -> Self {
        Self {
            initial_balance: 100.0,
            database_url: "sqlite:virtual_wallet.db".into(),
            wallet_id: "wallet1".into(),
            default_fee_param: 0.25,
        }
    }
}

impl WalletConfig {
    /// Load from environment variables
    pub fn from_env() -> Self {
        Self {
            initial_balance: std::env::var("INITIAL_BALANCE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100.0),
            database_url: std::env::var("VIRTUAL_WALLET_DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:virtual_wallet.db".into()),
            wallet_id: std::env::var("VIRTUAL_WALLET_ID")
                .unwrap_or_else(|_| "wallet1".into()),
            default_fee_param: std::env::var("DEFAULT_FEE_PARAM")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.25),
        }
    }
}

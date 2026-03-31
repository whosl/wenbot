//! Data models for the virtual wallet

use serde::{Deserialize, Serialize};
use crate::error::{Result, WalletError};
use sqlx::FromRow;

// ─── Wallet State (singleton per wallet_id) ───

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct WalletState {
    pub wallet_id: String,
    pub balance: f64,
    pub total_trades: i64,
    pub winning_trades: i64,
    pub total_pnl: f64,
    pub created_at: String,
    pub updated_at: String,
}

// ─── Position ───

/// Position status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionStatus {
    Open,
    Settled,
    Canceled,
}

impl std::fmt::Display for PositionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PositionStatus::Open => write!(f, "open"),
            PositionStatus::Settled => write!(f, "settled"),
            PositionStatus::Canceled => write!(f, "canceled"),
        }
    }
}

/// Position direction (YES or NO)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Yes,
    No,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction::Yes => write!(f, "YES"),
            Direction::No => write!(f, "NO"),
        }
    }
}

impl Direction {
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_uppercase().as_str() {
            "YES" => Ok(Direction::Yes),
            "NO" => Ok(Direction::No),
            other => Err(WalletError::InvalidInput(format!("Invalid direction: {}", other))),
        }
    }
}

/// Market category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Category {
    Weather,
    Btc,
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Category::Weather => write!(f, "weather"),
            Category::Btc => write!(f, "btc"),
        }
    }
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct VirtualPosition {
    pub id: i64,
    pub wallet_id: String,
    pub market_id: String,
    pub market_question: String,
    pub token_id: String,
    pub direction: String,      // "YES" or "NO"
    pub entry_price: f64,
    pub effective_entry: Option<f64>,
    pub size: f64,              // Invested amount in USD
    pub quantity: f64,          // Number of shares
    pub fee: f64,
    pub slippage: f64,
    pub category: String,       // "weather" or "btc"
    pub target_date: Option<String>,
    pub threshold_f: Option<f64>,
    pub city_name: Option<String>,
    pub metric: Option<String>, // "high" or "low"
    pub event_slug: Option<String>,
    pub window_end: Option<String>,
    pub btc_price: Option<f64>,
    pub status: String,         // "open", "settled", "canceled"
    pub created_at: String,
    pub settled_at: Option<String>,
    pub settlement_value: Option<f64>,
    pub actual_temperature: Option<f64>,
    pub pnl: Option<f64>,
}

// ─── Trade History ───

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TradeHistory {
    pub id: i64,
    pub wallet_id: String,
    pub position_id: i64,
    pub market_id: String,
    pub market_question: String,
    pub direction: String,
    pub entry_price: f64,
    pub size: f64,
    pub quantity: f64,
    pub settlement_value: f64,
    pub actual_temperature: Option<f64>,
    pub pnl: f64,
    pub result: String,         // "win" or "loss"
    pub opened_at: String,
    pub closed_at: String,
    pub indicator_details: Option<String>,
}

// ─── Price Snapshot ───

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PriceSnapshot {
    pub id: i64,
    pub wallet_id: String,
    pub market_id: String,
    pub token_id: String,
    pub price: f64,
    pub timestamp: String,
}

// ─── API Response types ───

/// Balance summary returned by the wallet API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceSummary {
    pub balance: f64,
    pub total_position_value: f64,
    pub total_value: f64,
    pub total_trades: i64,
    pub winning_trades: i64,
    pub win_rate: f64,
    pub total_pnl: f64,
}

/// Trade input for opening a position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeInput {
    pub market_id: String,
    pub market_question: String,
    pub token_id: String,
    pub direction: String,       // "YES" or "NO"
    pub entry_price: f64,
    pub size: f64,               // USD amount to invest
    pub category: String,
    pub slippage: f64,           // Default 0.01 (1%)
    // Optional metadata
    pub target_date: Option<String>,
    pub threshold_f: Option<f64>,
    pub city_name: Option<String>,
    pub metric: Option<String>,
    pub event_slug: Option<String>,
    pub window_end: Option<String>,
    pub btc_price: Option<f64>,
    pub fee_param: Option<f64>,
}

// ─── BTC Trade Ledger ───

/// Full-featured BTC trade ledger record with signal context
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct BtcTradeLedger {
    pub id: i64,
    /// Self-generated trade ID (format: btc-{position_id})
    pub trade_id: String,
    pub wallet_id: String,
    pub position_id: i64,
    /// Market slug (e.g. "btc-updown-15m-1774682100")
    pub market_slug: String,
    /// Market question text
    pub market_question: String,
    /// Token ID for the bought token
    pub token_id: String,
    /// Trade direction: "YES" or "NO"
    pub direction: String,
    /// Predicted direction before inversion
    pub predicted_direction: String,
    /// Effective direction after inversion
    pub effective_direction: String,
    /// When the position was opened
    pub opened_at: String,
    /// When the position was settled/closed (NULL if still open)
    pub closed_at: Option<String>,
    /// Entry price per share
    pub entry_price: f64,
    /// Number of shares
    pub quantity: f64,
    /// USD amount invested
    pub size: f64,
    /// Edge (model_prob - market_prob)
    pub edge: f64,
    /// Confidence score (0-1)
    pub confidence: f64,
    /// Model's estimated probability
    pub model_probability: f64,
    /// Market's implied probability
    pub market_probability: f64,
    /// Suggested position size by the model
    pub suggested_size: f64,
    /// Reasoning string from signal generation
    pub reasoning: String,
    /// Indicator scores as JSON string
    pub indicator_scores: String,
    /// BTC price at signal time
    pub asset_price: f64,
    /// Settlement result: "win", "loss", or NULL if unsettled
    pub result: Option<String>,
    /// Realized PnL (NULL if unsettled)
    pub pnl: Option<f64>,
    /// Settlement token value (1.0=win, 0.0=loss, NULL if unsettled)
    pub settlement_value: Option<f64>,
    /// Fee paid
    pub fee: f64,
    /// Slippage applied
    pub slippage: f64,
    /// True if backfilled from history (not captured at trade time)
    pub is_reconstructed: bool,
    /// How the reconstruction was done (e.g. "epoch_inferred,price_inferred")
    pub reconstruction_source: Option<String>,
    /// Confidence score for the reconstruction quality (0.0 - 1.0)
    pub match_score: Option<f64>,
    /// Per-indicator vote/score/weight JSON string
    pub indicator_details: Option<String>,
    /// Record creation time
    pub created_at: String,
}

/// Parameters for inserting a new BTC trade ledger entry
pub struct BtcTradeLedgerInsert {
    pub trade_id: String,
    pub wallet_id: String,
    pub position_id: i64,
    pub market_slug: String,
    pub market_question: String,
    pub token_id: String,
    pub direction: String,
    pub predicted_direction: String,
    pub effective_direction: String,
    pub opened_at: String,
    pub entry_price: f64,
    pub quantity: f64,
    pub size: f64,
    pub edge: f64,
    pub confidence: f64,
    pub model_probability: f64,
    pub market_probability: f64,
    pub suggested_size: f64,
    pub reasoning: String,
    pub indicator_scores: String,
    pub indicator_details: Option<String>,
    pub asset_price: f64,
    pub fee: f64,
    pub slippage: f64,
    pub is_reconstructed: bool,
    pub reconstruction_source: Option<String>,
    pub match_score: Option<f64>,
}

// ─── Risk Management ───

/// Daily loss tracking for risk management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyLossTracker {
    pub date: String,            // YYYY-MM-DD format
    pub daily_start_balance: f64,
    pub daily_loss: f64,
    pub daily_realized_pnl: f64,
    pub open_exposure: f64,
}

impl Default for DailyLossTracker {
    fn default() -> Self {
        Self {
            date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
            daily_start_balance: 0.0,
            daily_loss: 0.0,
            daily_realized_pnl: 0.0,
            open_exposure: 0.0,
        }
    }
}

//! Core data types for the Polymarket CLOB

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ─── Market types ───

/// A Polymarket market
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    pub condition_id: String,
    pub question: String,
    pub market_id: String,
    pub slug: String,
    pub end_date: Option<String>,
    pub outcomes: Vec<String>,
    pub clob_token_ids: Vec<String>,
    pub outcome_prices: Vec<String>,
    pub active: bool,
    pub closed: bool,
    pub archived: bool,
    /// Up price for Up/Down markets (0-1)
    #[serde(default)]
    pub up_price: f64,
    /// Down price for Up/Down markets (0-1)
    #[serde(default)]
    pub down_price: f64,
}

impl Market {
    /// Get YES price (first outcome price)
    pub fn yes_price(&self) -> f64 {
        self.outcome_prices
            .first()
            .and_then(|p| p.parse::<f64>().ok())
            .unwrap_or(0.5)
    }

    /// Get NO price (second outcome price)
    pub fn no_price(&self) -> f64 {
        self.outcome_prices
            .get(1)
            .and_then(|p| p.parse::<f64>().ok())
            .unwrap_or(0.5)
    }

    /// Check if this is an Up/Down crypto market
    pub fn is_updown_market(&self) -> bool {
        self.outcomes.len() == 2
            && self.outcomes.iter().any(|o| o.eq_ignore_ascii_case("up"))
            && self.outcomes.iter().any(|o| o.eq_ignore_ascii_case("down"))
    }
}

/// Minimal market info for Up/Down crypto markets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpDownMarket {
    pub market_id: String,
    pub condition_id: String,
    pub slug: String,
    pub question: String,
    pub up_token_id: String,
    pub down_token_id: String,
    pub up_price: f64,
    pub down_price: f64,
    pub end_date: Option<String>,
    pub event_slug: String,
}

/// Weather market metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherMarket {
    pub market_id: String,
    pub condition_id: String,
    pub question: String,
    pub slug: String,
    pub city_key: String,
    pub city_name: String,
    pub target_date: String,
    pub metric: String,        // "high" or "low"
    pub direction: String,     // "above", "below", "between"
    pub threshold_f: f64,
    pub range_low: Option<f64>,
    pub range_high: Option<f64>,
    pub yes_price: f64,
    pub no_price: f64,
    pub token_id_yes: String,
    pub token_id_no: String,
}

// ─── Order types ───

/// Order type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderType {
    Gtc,
    Gtd,
    Fok,
    Fak,
}

/// Order side
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Side {
    Buy,
    Sell,
}

impl std::fmt::Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Side::Buy => write!(f, "BUY"),
            Side::Sell => write!(f, "SELL"),
        }
    }
}

/// A trade order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub token_id: String,
    pub side: Side,
    pub order_type: OrderType,
    pub price: f64,
    pub size: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration: Option<DateTime<Utc>>,
}

impl Order {
    pub fn buy_limit(token_id: &str, price: f64, size: f64) -> Self {
        Self {
            token_id: token_id.to_string(),
            side: Side::Buy,
            order_type: OrderType::Gtc,
            price,
            size,
            expiration: None,
        }
    }

    pub fn sell_limit(token_id: &str, price: f64, size: f64) -> Self {
        Self {
            token_id: token_id.to_string(),
            side: Side::Sell,
            order_type: OrderType::Gtc,
            price,
            size,
            expiration: None,
        }
    }

    pub fn buy_fak(token_id: &str, price: f64, size: f64) -> Self {
        Self {
            token_id: token_id.to_string(),
            side: Side::Buy,
            order_type: OrderType::Fak,
            price,
            size,
            expiration: None,
        }
    }
}

/// Response from order placement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResponse {
    pub order_id: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub fills: Vec<OrderFill>,
}

/// An order fill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderFill {
    pub fill_id: String,
    pub price: f64,
    pub size: f64,
    pub side: Side,
    pub match_time: DateTime<Utc>,
}

// ─── Price / Balance ───

/// Token price pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPrice {
    pub token_id: String,
    pub price: f64,
}

/// Balance info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceInfo {
    pub usdc_balance: f64,
    pub usdc_allowance: f64,
}

/// A position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub market_id: String,
    pub token_id: String,
    pub side: Side,
    pub size: f64,
    pub average_price: f64,
    pub current_price: f64,
    pub pnl: f64,
    pub outcome: String,
}

// ─── Orderbook ───

/// Orderbook side
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderbookSide {
    pub price: f64,
    pub size: f64,
}

/// Full orderbook
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Orderbook {
    pub market: String,
    pub asset_id: String,
    pub bids: Vec<OrderbookSide>,
    pub asks: Vec<OrderbookSide>,
    pub best_bid: Option<f64>,
    pub best_ask: Option<f64>,
    pub spread: Option<f64>,
}

// ─── Fee ───

/// Fee rate response from Polymarket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeRateResponse {
    pub base_fee: f64,
    pub fee_param: f64,
}

/// Fee parameters for a token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeInfo {
    pub token_id: String,
    pub fee_rate: f64,
    pub fee_dollars: f64,
}

/// Calculate Polymarket taker fee using the standard curve.
///
/// Fee curve: `fee_param * price^2 * (1 - price)^2`
/// Where fee_param is typically 0.25 for crypto markets.
pub fn calculate_taker_fee(price: f64, fee_param: Option<f64>) -> f64 {
    let fp = fee_param.unwrap_or(0.25);
    fp * price.powi(2) * (1.0 - price).powi(2)
}

/// Inverse of calculate_taker_fee: given total cost and price, return the fee portion.
pub fn fee_from_cost(price: f64, fee_param: Option<f64>) -> f64 {
    calculate_taker_fee(price, fee_param)
}

// ─── Tick sizes ───

/// Supported tick sizes
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TickSize {
    #[serde(rename = "0.01")]
    Cent,
    #[serde(rename = "0.001")]
    Mill,
    #[serde(rename = "0.0001")]
    TenMicro,
}

impl Default for TickSize {
    fn default() -> Self {
        TickSize::Cent
    }
}

impl TickSize {
    pub fn as_f64(&self) -> f64 {
        match self {
            TickSize::Cent => 0.01,
            TickSize::Mill => 0.001,
            TickSize::TenMicro => 0.0001,
        }
    }

    /// Round a price down to the nearest tick
    pub fn round_price(&self, price: f64) -> f64 {
        let tick = self.as_f64();
        (price / tick).floor() * tick
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_taker_fee() {
        // At price=0.50, fee = 0.25 * 0.25 * 0.25 = 0.015625 (~1.56%)
        let fee = calculate_taker_fee(0.50, Some(0.25));
        assert!((fee - 0.015625).abs() < 1e-10);

        // At extremes, fee approaches 0
        assert!(calculate_taker_fee(0.01, None) < 0.001);
        assert!(calculate_taker_fee(0.99, None) < 0.001);
    }

    #[test]
    fn test_tick_size_rounding() {
        assert_eq!(TickSize::Cent.round_price(0.557), 0.55);
        assert_eq!(TickSize::Mill.round_price(0.5575), 0.557);
    }
}

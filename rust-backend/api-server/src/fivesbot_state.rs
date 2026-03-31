use std::collections::HashMap;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use polymarket_client::UpDownMarket;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FivesbotPredictorState {
    pub signal: String,
    pub confidence: f64,
    pub direction: String,
    pub trend: f64,
    pub momentum: f64,
    pub volatility: f64,
    pub rsi: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpDownMarketInfo {
    pub market_id: String,
    pub slug: String,
    pub question: String,
    pub up_token_id: String,
    pub down_token_id: String,
    pub up_price: f64,
    pub down_price: f64,
    pub end_date: Option<String>,
    pub event_slug: String,
}

impl From<UpDownMarket> for UpDownMarketInfo {
    fn from(value: UpDownMarket) -> Self {
        Self {
            market_id: value.market_id,
            slug: value.slug,
            question: value.question,
            up_token_id: value.up_token_id,
            down_token_id: value.down_token_id,
            up_price: value.up_price,
            down_price: value.down_price,
            end_date: value.end_date,
            event_slug: value.event_slug,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FivesbotLiveState {
    pub ws_connected: bool,
    pub started_at: Instant,
    pub btc_current_cycle: String,
    pub eth_current_cycle: String,
    pub wallet3_current_cycle: String,
    pub wallet4_current_cycle: String,
    pub btc_last_update: Option<DateTime<Utc>>,
    pub eth_last_update: Option<DateTime<Utc>>,
    pub wallet3_last_update: Option<DateTime<Utc>>,
    pub wallet4_last_update: Option<DateTime<Utc>>,
    pub btc_up_price: Option<f64>,
    pub btc_down_price: Option<f64>,
    pub eth_up_price: Option<f64>,
    pub eth_down_price: Option<f64>,
    pub wallet3_up_price: Option<f64>,
    pub wallet3_down_price: Option<f64>,
    pub wallet4_up_price: Option<f64>,
    pub wallet4_down_price: Option<f64>,
    pub btc_price: Option<f64>,
    pub eth_price: Option<f64>,
    pub wallet3_price: Option<f64>,
    pub wallet4_price: Option<f64>,
    pub btc_predictor: Option<FivesbotPredictorState>,
    pub eth_predictor: Option<FivesbotPredictorState>,
    pub wallet3_predictor: Option<FivesbotPredictorState>,
    pub wallet4_predictor: Option<FivesbotPredictorState>,
    pub btc_recent_signals: Vec<fivesbot_strategy::TradingSignal>,
    pub eth_recent_signals: Vec<fivesbot_strategy::TradingSignal>,
    pub wallet3_recent_signals: Vec<fivesbot_strategy::TradingSignal>,
    pub wallet4_recent_signals: Vec<fivesbot_strategy::TradingSignal>,
    pub btc_active_markets: Vec<UpDownMarketInfo>,
    pub eth_active_markets: Vec<UpDownMarketInfo>,
    pub wallet3_active_markets: Vec<UpDownMarketInfo>,
    pub wallet4_active_markets: Vec<UpDownMarketInfo>,
    /// Real-time price feed from WebSocket: token_id → latest mid price.
    /// Used for mark-to-market of wallet1 open positions without hitting DB.
    pub token_live_prices: HashMap<String, f64>,
}

impl Default for FivesbotLiveState {
    fn default() -> Self {
        Self {
            ws_connected: false,
            started_at: Instant::now(),
            btc_current_cycle: "--".to_string(),
            eth_current_cycle: "--".to_string(),
            wallet3_current_cycle: "--".to_string(),
            wallet4_current_cycle: "--".to_string(),
            btc_last_update: None,
            eth_last_update: None,
            wallet3_last_update: None,
            wallet4_last_update: None,
            btc_up_price: None,
            btc_down_price: None,
            eth_up_price: None,
            eth_down_price: None,
            wallet3_up_price: None,
            wallet3_down_price: None,
            wallet4_up_price: None,
            wallet4_down_price: None,
            btc_price: None,
            eth_price: None,
            wallet3_price: None,
            wallet4_price: None,
            btc_predictor: None,
            eth_predictor: None,
            wallet3_predictor: None,
            wallet4_predictor: None,
            btc_recent_signals: Vec::new(),
            eth_recent_signals: Vec::new(),
            wallet3_recent_signals: Vec::new(),
            wallet4_recent_signals: Vec::new(),
            btc_active_markets: Vec::new(),
            eth_active_markets: Vec::new(),
            wallet3_active_markets: Vec::new(),
            wallet4_active_markets: Vec::new(),
            token_live_prices: HashMap::new(),
        }
    }
}

impl FivesbotLiveState {
    pub fn uptime(&self) -> String {
        format_duration(self.started_at.elapsed())
    }

    pub fn push_signal(&mut self, asset: &str, signal: fivesbot_strategy::TradingSignal) {
        let bucket = if asset.eq_ignore_ascii_case("eth") {
            &mut self.eth_recent_signals
        } else if asset.eq_ignore_ascii_case("btc8") || asset.eq_ignore_ascii_case("wallet3") {
            &mut self.wallet3_recent_signals
        } else if asset.eq_ignore_ascii_case("wallet4") {
            &mut self.wallet4_recent_signals
        } else {
            &mut self.btc_recent_signals
        };
        bucket.insert(0, signal);
        if bucket.len() > 50 {
            bucket.truncate(50);
        }
    }
}

pub fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

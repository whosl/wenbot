//! Trading signal types and signal generation

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

const NEUTRAL_EPSILON: f64 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalAction {
    BuyUp,
    BuyDown,
    Hold,
}

impl std::fmt::Display for SignalAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignalAction::BuyUp => write!(f, "BUY_UP"),
            SignalAction::BuyDown => write!(f, "BUY_DOWN"),
            SignalAction::Hold => write!(f, "HOLD"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorScoreDetail {
    pub vote: String,
    pub score: f64,
    pub weight: f64,
}

impl IndicatorScoreDetail {
    pub fn from_signal(score: f64, weight: f64) -> Self {
        let vote = if score > NEUTRAL_EPSILON {
            "up"
        } else if score < -NEUTRAL_EPSILON {
            "down"
        } else {
            "neutral"
        };

        Self {
            vote: vote.to_string(),
            score: normalized_score(score),
            weight,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorDetails {
    pub rsi: IndicatorScoreDetail,
    pub momentum: IndicatorScoreDetail,
    pub vwap: IndicatorScoreDetail,
    pub sma: IndicatorScoreDetail,
    pub market_skew: IndicatorScoreDetail,
    pub volume_trend: IndicatorScoreDetail,
    pub bollinger: IndicatorScoreDetail,
    pub volatility: IndicatorScoreDetail,
}

pub fn normalized_score(signal: f64) -> f64 {
    ((signal.clamp(-1.0, 1.0) + 1.0) / 2.0).clamp(0.0, 1.0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingSignal {
    pub market_slug: String,
    pub action: SignalAction,
    pub predicted_direction: String,
    pub effective_direction: String,
    pub model_probability: f64,
    pub market_probability: f64,
    pub edge: f64,
    pub confidence: f64,
    pub suggested_size: f64,
    pub buy_token_id: String,
    pub buy_price: f64,
    pub buy_shares: u32,
    pub asset_price: f64,
    pub reasoning: String,
    pub timestamp: DateTime<Utc>,
    pub indicator_scores: IndicatorScores,
    pub indicator_details: IndicatorDetails,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorScores {
    pub rsi_signal: f64,
    pub momentum_signal: f64,
    pub vwap_signal: f64,
    pub sma_signal: f64,
    pub market_skew: f64,
    pub volume_trend_signal: f64,
    pub bollinger_signal: f64,
    pub volatility_signal: f64,
    pub composite: f64,
    pub convergence_votes: (u32, u32),
}

pub fn calculate_edge(model_prob: f64, market_price: f64) -> (f64, &'static str) {
    let up_edge = model_prob - market_price;
    let down_edge = (1.0 - model_prob) - (1.0 - market_price);

    if up_edge >= down_edge {
        (up_edge, "up")
    } else {
        (down_edge, "down")
    }
}

pub fn calculate_kelly_size(
    _edge: f64,
    probability: f64,
    market_price: f64,
    direction: &str,
    bankroll: f64,
    kelly_fraction: f64,
    kelly_max_fraction: f64,
    max_trade_size: f64,
) -> f64 {
    let (win_prob, price) = match direction {
        "up" => (probability, market_price),
        _ => (1.0 - probability, 1.0 - market_price),
    };

    if price <= 0.0 || price >= 1.0 {
        return 0.0;
    }

    let odds = (1.0 - price) / price;
    let lose_prob = 1.0 - win_prob;

    if odds == 0.0 {
        return 0.0;
    }

    let kelly = (win_prob * odds - lose_prob) / odds;
    let kelly = (kelly * kelly_fraction).min(kelly_max_fraction).max(0.0);
    (kelly * bankroll).min(max_trade_size)
}

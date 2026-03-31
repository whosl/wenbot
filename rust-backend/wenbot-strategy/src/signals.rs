//! Weather trading signal types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Signal direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalDirection {
    Yes,
    No,
}

impl std::fmt::Display for SignalDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignalDirection::Yes => write!(f, "YES"),
            SignalDirection::No => write!(f, "NO"),
        }
    }
}

/// A weather trading signal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherSignal {
    /// Market being traded
    pub market_id: String,
    pub market_question: String,

    /// Direction: buy YES or buy NO
    pub direction: SignalDirection,

    /// Model's estimated probability of YES
    pub model_probability: f64,

    /// Market's implied probability
    pub market_probability: f64,

    /// Edge (model - market)
    pub edge: f64,

    /// Confidence (0-1)
    pub confidence: f64,

    /// Suggested position size in USD
    pub suggested_size: f64,

    /// Token ID to buy
    pub buy_token_id: String,

    /// Entry price
    pub entry_price: f64,

    /// Does signal pass the minimum edge threshold?
    pub passes_threshold: bool,

    /// Reasoning string
    pub reasoning: String,

    /// Forecast context
    pub ensemble_mean: f64,
    pub ensemble_std: f64,
    pub ensemble_members: usize,

    /// NWS cross-validation
    pub nws_forecast: Option<f64>,
    pub nws_agrees: Option<bool>,

    /// City info
    pub city_key: String,
    pub city_name: String,
    pub target_date: String,

    /// Market metric context
    pub metric: String,          // "high" or "low"
    pub threshold_f: f64,        // temperature threshold in °F

    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Check if NWS forecast supports a YES outcome
pub fn nws_agrees_with_yes(nws_temp: f64, direction: &str, threshold: f64,
                           range_low: Option<f64>, range_high: Option<f64>) -> bool {
    match direction {
        "above" => nws_temp >= threshold,
        "below" => nws_temp <= threshold,
        "between" => match (range_low, range_high) {
            (Some(lo), Some(hi)) => lo <= nws_temp && nws_temp <= hi,
            _ => false,
        },
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nws_agrees_above() {
        assert!(nws_agrees_with_yes(80.0, "above", 75.0, None, None));
        assert!(!nws_agrees_with_yes(70.0, "above", 75.0, None, None));
    }

    #[test]
    fn test_nws_agrees_below() {
        assert!(nws_agrees_with_yes(28.0, "below", 30.0, None, None));
        assert!(!nws_agrees_with_yes(35.0, "below", 30.0, None, None));
    }

    #[test]
    fn test_nws_agrees_between() {
        assert!(nws_agrees_with_yes(50.0, "between", 40.0, Some(40.0), Some(60.0)));
        assert!(!nws_agrees_with_yes(30.0, "between", 40.0, Some(40.0), Some(60.0)));
        assert!(!nws_agrees_with_yes(70.0, "between", 40.0, Some(40.0), Some(60.0)));
    }
}

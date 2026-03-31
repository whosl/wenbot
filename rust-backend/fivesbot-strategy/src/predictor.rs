//! Adaptive price predictor for BTC/ETH 15-minute markets
//!
//! Maintains a sliding window of price predictions and tracks accuracy.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A price prediction made by the predictor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricePrediction {
    pub predicted_price: f64,
    pub current_price: f64,
    pub direction: String,     // "up" or "down"
    pub confidence: f64,
    pub signal: String,        // "BUY_UP", "BUY_DOWN", "HOLD"
    pub momentum: f64,
    pub volatility: f64,
    pub trend: f64,
    pub timestamp: DateTime<Utc>,
}

/// History point for accuracy tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionHistoryPoint {
    pub time: DateTime<Utc>,
    pub predicted_price: f64,
    pub actual_price: f64,
    pub confidence: f64,
    pub signal: String,
}

/// Adaptive price predictor that tracks its own accuracy
pub struct AdaptivePricePredictor {
    history: Vec<PredictionHistoryPoint>,
    max_history: usize,
    correct_predictions: u32,
    total_predictions: u32,
}

impl AdaptivePricePredictor {
    pub fn new(max_history: usize) -> Self {
        Self {
            history: Vec::with_capacity(max_history),
            max_history,
            correct_predictions: 0,
            total_predictions: 0,
        }
    }

    /// Record a prediction
    pub fn record_prediction(&mut self, prediction: PricePrediction) {
        self.history.push(PredictionHistoryPoint {
            time: prediction.timestamp,
            predicted_price: prediction.predicted_price,
            actual_price: prediction.current_price,
            confidence: prediction.confidence,
            signal: prediction.signal,
        });

        if self.history.len() > self.max_history {
            self.history.remove(0);
        }
    }

    /// Update prediction accuracy when actual result is known
    pub fn record_outcome(&mut self, actual_up: bool) {
        if let Some(last) = self.history.last_mut() {
            self.total_predictions += 1;
            let predicted_up = last.signal == "BUY_UP";
            if predicted_up == actual_up {
                self.correct_predictions += 1;
            }
        }
    }

    /// Get prediction accuracy (0-1)
    pub fn accuracy(&self) -> f64 {
        if self.total_predictions == 0 {
            return 0.5;
        }
        self.correct_predictions as f64 / self.total_predictions as f64
    }

    /// Get number of correct predictions
    pub fn correct_count(&self) -> u32 {
        self.correct_predictions
    }

    /// Get total prediction count
    pub fn total_count(&self) -> u32 {
        self.total_predictions
    }

    /// Get number of tracked predictions
    pub fn prediction_count(&self) -> usize {
        self.history.len()
    }

    /// Get history snapshot
    pub fn get_history(&self) -> &[PredictionHistoryPoint] {
        &self.history
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accuracy_tracking() {
        let mut predictor = AdaptivePricePredictor::new(100);

        // Record some predictions
        for i in 0..10 {
            predictor.record_prediction(PricePrediction {
                predicted_price: 50000.0 + i as f64,
                current_price: 50000.0,
                direction: if i % 2 == 0 { "up".to_string() } else { "down".to_string() },
                confidence: 0.6,
                signal: if i % 2 == 0 { "BUY_UP".to_string() } else { "BUY_DOWN".to_string() },
                momentum: 0.01,
                volatility: 0.02,
                trend: 0.01,
                timestamp: Utc::now(),
            });
        }

        assert_eq!(predictor.prediction_count(), 10);

        // Record outcomes — only the last prediction counts per record_outcome call
        // So we check each recorded outcome against the last signal in history
        for _i in 5..10 { // Only last 5 are in history (max_history=10 actually, all 10 remain)
            // Last prediction's signal is BUY_DOWN (i=9, odd)
            // We claim i%2==0 (true) means "up was correct"
            // But last signal was BUY_DOWN, so "up was correct" is wrong for last entry
            // The predictor compares last.signal == actual_up
        }

        // The last prediction was index 9 (BUY_DOWN), so record_outcome(true) means
        // predicted BUY_DOWN, actual up → wrong. record_outcome(false) → correct.
        // Let's just record the last one correctly:
        predictor.record_outcome(false); // last signal was BUY_DOWN, actual is not up → correct
        assert_eq!(predictor.total_count(), 1);
        assert_eq!(predictor.correct_count(), 1);
        assert!((predictor.accuracy() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_history_trimming() {
        let mut predictor = AdaptivePricePredictor::new(5);

        for _i in 0..10 {
            predictor.record_prediction(PricePrediction {
                predicted_price: 50000.0,
                current_price: 50000.0,
                direction: "up".to_string(),
                confidence: 0.5,
                signal: "HOLD".to_string(),
                momentum: 0.0,
                volatility: 0.0,
                trend: 0.0,
                timestamp: Utc::now(),
            });
        }

        assert_eq!(predictor.prediction_count(), 5);
    }
}

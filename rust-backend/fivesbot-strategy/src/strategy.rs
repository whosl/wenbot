//! Main fivesbot strategy engine

use chrono::{Timelike, Utc};
use tracing::{debug, info};

use crate::config::FivesbotConfig;
use crate::error::{Result, StrategyError};
use crate::indicators::BtcMicrostructure;
use crate::predictor::AdaptivePricePredictor;
use crate::signals::{
    IndicatorDetails, IndicatorScoreDetail, IndicatorScores, SignalAction, TradingSignal,
    calculate_edge, calculate_kelly_size,
};

#[derive(Debug, Clone)]
pub struct StrategyProfile {
    pub name: &'static str,
    pub rsi_weight: f64,
    pub momentum_weight: f64,
    pub vwap_weight: f64,
    pub sma_weight: f64,
    pub market_skew_weight: f64,
    pub volume_trend_weight: f64,
    pub bollinger_weight: f64,
    pub volatility_weight: f64,
    pub probability_scale: f64,
    pub min_convergence_votes: u32,
}

impl StrategyProfile {
    pub fn classic_four() -> Self {
        Self {
            name: "classic_four",
            rsi_weight: 0.25,
            momentum_weight: 0.25,
            vwap_weight: 0.20,
            sma_weight: 0.15,
            market_skew_weight: 0.0,
            volume_trend_weight: 0.0,
            bollinger_weight: 0.0,
            volatility_weight: 0.0,
            probability_scale: 0.15,
            min_convergence_votes: 2,
        }
    }

    pub fn btc_eight_indicator() -> Self {
        Self {
            name: "btc_eight_indicator",
            rsi_weight: 0.15,
            momentum_weight: 0.20,
            vwap_weight: 0.15,
            sma_weight: 0.10,
            market_skew_weight: 0.10,
            volume_trend_weight: 0.10,
            bollinger_weight: 0.10,
            volatility_weight: 0.10,
            probability_scale: 0.15,
            min_convergence_votes: 2,
        }
    }
}

pub struct FivesbotStrategy {
    config: FivesbotConfig,
    predictor: AdaptivePricePredictor,
    profile: StrategyProfile,
}

impl FivesbotStrategy {
    pub fn new(config: FivesbotConfig) -> Self {
        Self::with_profile(config, StrategyProfile::classic_four())
    }

    pub fn new_eight_indicator(config: FivesbotConfig) -> Self {
        Self::with_profile(config, StrategyProfile::btc_eight_indicator())
    }

    pub fn with_profile(config: FivesbotConfig, profile: StrategyProfile) -> Self {
        Self { config, predictor: AdaptivePricePredictor::new(200), profile }
    }

    pub fn config(&self) -> &FivesbotConfig { &self.config }
    pub fn profile(&self) -> &StrategyProfile { &self.profile }

    pub fn generate_signal(
        &self,
        market: &str,
        micro: &BtcMicrostructure,
        up_price: f64,
        down_price: f64,
        seconds_remaining: u32,
        bankroll: f64,
    ) -> Result<TradingSignal> {
        if seconds_remaining < self.config.min_time_remaining {
            return Err(StrategyError::TooCloseToExpiry { remaining: seconds_remaining, min: self.config.min_time_remaining });
        }
        if seconds_remaining > self.config.max_time_remaining {
            return Err(StrategyError::TooFresh { remaining: seconds_remaining, max: self.config.max_time_remaining });
        }
        if up_price < 0.02 || up_price > 0.98 {
            return Err(StrategyError::InsufficientEdge { edge: 0.0, threshold: self.config.min_edge_threshold });
        }

        // RSI inverted for 15min/5min binary options: follow momentum, not mean reversion.
        // RSI > 70 (overbought, recent uptrend) → bullish signal → positive
        // RSI < 30 (oversold, recent downtrend) → bearish signal → negative
        let rsi_signal = if micro.rsi > 70.0 {
            (0.5 + (micro.rsi - 70.0) / 30.0).clamp(-1.0, 1.0)
        } else if micro.rsi < 30.0 {
            (-0.5 - (30.0 - micro.rsi) / 30.0).clamp(-1.0, 1.0)
        } else if micro.rsi > 55.0 {
            ((micro.rsi - 55.0) / 30.0).clamp(-1.0, 1.0)
        } else if micro.rsi < 45.0 {
            (-(45.0 - micro.rsi) / 30.0).clamp(-1.0, 1.0)
        } else {
            0.0
        };

        let momentum_signal = (micro.momentum_1m * 0.5 + micro.momentum_5m * 0.35 + micro.momentum_15m * 0.15)
            .clamp(-1.0, 1.0);
        let vwap_signal = (micro.vwap_deviation / 0.05).clamp(-1.0, 1.0);
        let sma_signal = (micro.sma_crossover / 0.03).clamp(-1.0, 1.0);
        let market_skew = (up_price - 0.50) * -4.0;
        let volume_trend_signal = micro.volume_trend.clamp(-1.0, 1.0);
        let bollinger_signal = micro.bollinger_position.clamp(-1.0, 1.0);
        let volatility_signal = micro.volatility_regime.clamp(-1.0, 1.0);

        let indicators = [
            rsi_signal,
            momentum_signal,
            vwap_signal,
            sma_signal,
            market_skew,
            volume_trend_signal,
            bollinger_signal,
            volatility_signal,
        ];
        let active_indicators: Vec<f64> = indicators
            .into_iter()
            .enumerate()
            .filter_map(|(idx, value)| {
                let enabled = match idx {
                    4 => self.profile.market_skew_weight > 0.0,
                    5 => self.profile.volume_trend_weight > 0.0,
                    6 => self.profile.bollinger_weight > 0.0,
                    7 => self.profile.volatility_weight > 0.0,
                    _ => true,
                };
                enabled.then_some(value)
            })
            .collect();
        let up_votes = active_indicators.iter().filter(|&&s| s > 0.05).count() as u32;
        let down_votes = active_indicators.iter().filter(|&&s| s < -0.05).count() as u32;
        if up_votes < self.profile.min_convergence_votes && down_votes < self.profile.min_convergence_votes {
            debug!("No convergence for {}: up={}, down={}, profile={}", market, up_votes, down_votes, self.profile.name);
            return Err(StrategyError::InsufficientEdge { edge: 0.0, threshold: self.config.min_edge_threshold });
        }

        let composite = rsi_signal * self.profile.rsi_weight
            + momentum_signal * self.profile.momentum_weight
            + vwap_signal * self.profile.vwap_weight
            + sma_signal * self.profile.sma_weight
            + market_skew * self.profile.market_skew_weight
            + volume_trend_signal * self.profile.volume_trend_weight
            + bollinger_signal * self.profile.bollinger_weight
            + volatility_signal * self.profile.volatility_weight;

        let model_up_prob = (0.50 + composite * self.profile.probability_scale).clamp(0.35, 0.65);
        let (edge, raw_direction) = calculate_edge(model_up_prob, up_price);
        let entry_price = match raw_direction { "up" => up_price, _ => down_price };
        if entry_price > self.config.max_entry_price {
            return Err(StrategyError::EntryPriceTooHigh { price: entry_price, max: self.config.max_entry_price });
        }
        if edge.abs() < self.config.min_edge_threshold {
            return Err(StrategyError::InsufficientEdge { edge, threshold: self.config.min_edge_threshold });
        }

        let predicted_direction = raw_direction.to_string();
        let effective_direction = self.config.effective_direction(raw_direction).to_string();
        let action = match effective_direction.as_str() {
            "up" => SignalAction::BuyUp,
            "down" => SignalAction::BuyDown,
            _ => SignalAction::Hold,
        };

        let suggested_size = calculate_kelly_size(
            edge.abs(), model_up_prob, up_price, raw_direction, bankroll,
            self.config.kelly_fraction, self.config.kelly_max_fraction, self.config.max_trade_size,
        );

        let total_votes = up_votes + down_votes;
        let confidence_denominator = active_indicators.len().max(1) as f64;
        let indicator_details = IndicatorDetails {
            rsi: IndicatorScoreDetail::from_signal(rsi_signal, self.profile.rsi_weight),
            momentum: IndicatorScoreDetail::from_signal(momentum_signal, self.profile.momentum_weight),
            vwap: IndicatorScoreDetail::from_signal(vwap_signal, self.profile.vwap_weight),
            sma: IndicatorScoreDetail::from_signal(sma_signal, self.profile.sma_weight),
            market_skew: IndicatorScoreDetail::from_signal(market_skew, self.profile.market_skew_weight),
            volume_trend: IndicatorScoreDetail::from_signal(volume_trend_signal, self.profile.volume_trend_weight),
            bollinger: IndicatorScoreDetail::from_signal(bollinger_signal, self.profile.bollinger_weight),
            volatility: IndicatorScoreDetail::from_signal(volatility_signal, self.profile.volatility_weight),
        };
        let reasoning = format!(
            "profile={} market={} | RSI={:.2} Mom={:.2} VWAP={:.2} SMA={:.2} Skew={:.2} VolTr={:.2} Boll={:.2} VolReg={:.2} | Composite={:.3} → Model UP={:.3} vs Market={:.3} | Edge={:+.3} → {} @ {:.3} | Conv={}/{}",
            self.profile.name, market, rsi_signal, momentum_signal, vwap_signal, sma_signal, market_skew,
            volume_trend_signal, bollinger_signal, volatility_signal, composite, model_up_prob, up_price,
            edge, effective_direction, entry_price, up_votes.max(down_votes), total_votes,
        );
        info!("{}", reasoning);

        Ok(TradingSignal {
            market_slug: market.to_string(),
            action,
            predicted_direction: predicted_direction.clone(),
            effective_direction: effective_direction.clone(),
            model_probability: model_up_prob,
            market_probability: up_price,
            edge,
            confidence: up_votes.max(down_votes) as f64 / confidence_denominator,
            suggested_size,
            buy_token_id: String::new(),
            buy_price: entry_price,
            buy_shares: self.config.shares_per_side,
            asset_price: micro.price,
            reasoning,
            timestamp: Utc::now(),
            indicator_scores: IndicatorScores {
                rsi_signal,
                momentum_signal,
                vwap_signal,
                sma_signal,
                market_skew,
                volume_trend_signal,
                bollinger_signal,
                volatility_signal,
                composite,
                convergence_votes: (up_votes, down_votes),
            },
            indicator_details,
        })
    }

    pub fn slug_for_current_15m(market: &str) -> String {
        let now = Utc::now();
        let minutes = now.minute();
        let slot_min = (minutes / 15) * 15;
        let slot_time = now.with_minute(slot_min).expect("invalid minute").with_second(0).expect("invalid second").with_nanosecond(0).unwrap();
        format!("{}-updown-15m-{}", market, slot_time.timestamp())
    }

    pub fn predictor_stats(&self) -> (u32, u32, f64) {
        (self.predictor.correct_count(), self.predictor.total_count(), self.predictor.accuracy())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> FivesbotConfig {
        FivesbotConfig {
            invert_prediction_signal: false,
            min_edge_threshold: 0.06,
            max_entry_price: 0.55,
            min_time_remaining: 60,
            max_time_remaining: 1800,
            kelly_fraction: 0.15,
            kelly_max_fraction: 0.05,
            max_trade_size: 75.0,
            shares_per_side: 5,
            ..Default::default()
        }
    }

    fn make_micro() -> BtcMicrostructure {
        BtcMicrostructure {
            price: 67000.0,
            rsi: 25.0,
            momentum_1m: 0.08,
            momentum_5m: 0.12,
            momentum_15m: 0.10,
            vwap_deviation: 0.03,
            sma_crossover: 0.02,
            volume_trend: 0.25,
            bollinger_position: 0.30,
            volatility_regime: 0.15,
            change_1h: 0.5,
            change_24h: 1.0,
        }
    }

    #[test]
    fn test_signal_generation() {
        let strategy = FivesbotStrategy::new(make_config());
        let signal = strategy.generate_signal("btc-updown-15m-1745000000", &make_micro(), 0.45, 0.55, 600, 100.0);
        assert!(signal.is_ok());
    }

    #[test]
    fn test_eight_indicator_strategy_generation() {
        let strategy = FivesbotStrategy::new_eight_indicator(make_config());
        let signal = strategy.generate_signal("btc-updown-15m-1745000000", &make_micro(), 0.45, 0.55, 600, 100.0);
        assert!(signal.is_ok());
        let sig = signal.unwrap();
        assert!(sig.indicator_scores.volume_trend_signal != 0.0);
    }
}

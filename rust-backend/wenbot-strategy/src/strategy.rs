//! Main wenbot strategy engine

use chrono::Utc;
use tracing::{debug, info};

use crate::config::WenbotConfig;
use crate::error::Result;
use crate::forecast::EnsembleForecast;
use crate::markets::WeatherMarketInfo;
use crate::signals::{nws_agrees_with_yes, SignalDirection, WeatherSignal};

pub struct WenbotStrategy {
    config: WenbotConfig,
}

impl WenbotStrategy {
    pub fn new(config: WenbotConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &WenbotConfig {
        &self.config
    }

    pub fn adjusted_kelly_fraction(&self, recent_win_rate: Option<f64>) -> f64 {
        let base = self.config.kelly_fraction;
        match recent_win_rate {
            Some(rate) if rate < 0.35 => 0.0,
            Some(rate) if rate < 0.45 => base * 0.7,
            Some(rate) if rate > 0.60 => base * 1.2,
            _ => base,
        }
    }

    pub fn generate_signal(
        &self,
        market: &WeatherMarketInfo,
        forecast: &EnsembleForecast,
        nws_temp: Option<f64>,
        bankroll: f64,
    ) -> Result<WeatherSignal> {
        self.generate_signal_with_win_rate(market, forecast, nws_temp, bankroll, None)
    }

    pub fn generate_signal_with_win_rate(
        &self,
        market: &WeatherMarketInfo,
        forecast: &EnsembleForecast,
        nws_temp: Option<f64>,
        bankroll: f64,
        recent_win_rate: Option<f64>,
    ) -> Result<WeatherSignal> {
        let model_yes_prob = self.calculate_model_probability(market, forecast).clamp(0.05, 0.95);
        let market_yes_prob = market.yes_price;
        let (edge, raw_direction) = self.calculate_edge(model_yes_prob, market_yes_prob);
        let direction = match raw_direction {
            "up" => SignalDirection::Yes,
            _ => SignalDirection::No,
        };

        let entry_price = match direction {
            SignalDirection::Yes => market.yes_price,
            SignalDirection::No => market.no_price,
        };

        if entry_price > self.config.max_entry_price {
            debug!(
                "Entry price filter: {:.0} > {:.0} for {}",
                entry_price, self.config.max_entry_price, market.city_key
            );
            return Ok(WeatherSignal {
                market_id: market.market_id.clone(),
                market_question: market.question.clone(),
                direction,
                model_probability: model_yes_prob,
                market_probability: market_yes_prob,
                edge: 0.0,
                confidence: 0.0,
                suggested_size: 0.0,
                buy_token_id: String::new(),
                entry_price,
                passes_threshold: false,
                reasoning: format!("FILTERED: entry {:.0} > max {:.0}", entry_price, self.config.max_entry_price),
                ensemble_mean: if market.metric == "low" { forecast.mean_low() } else { forecast.mean_high() },
                ensemble_std: if market.metric == "low" { forecast.std_low() } else { forecast.std_high() },
                ensemble_members: forecast.num_members,
                nws_forecast: nws_temp,
                nws_agrees: None,
                city_key: market.city_key.clone(),
                city_name: market.city_name.clone(),
                target_date: market.target_date.clone(),
                metric: market.metric.clone(),
                threshold_f: market.threshold_f,
                timestamp: Utc::now(),
            });
        }

        let members = match market.metric.as_str() {
            "low" => &forecast.member_lows,
            _ => &forecast.member_highs,
        };

        let confidence = if members.is_empty() {
            0.1
        } else if market.direction == "between" {
            let between_count = members
                .iter()
                .filter(|&&m| {
                    market.range_low.map_or(false, |lo| lo <= m)
                        && market.range_high.map_or(false, |hi| m <= hi)
                })
                .count();
            let agreement = between_count.max(members.len() - between_count) as f64 / members.len() as f64;
            (agreement * 0.9).min(0.9)
        } else {
            let above_count = members
                .iter()
                .filter(|&&m| if market.direction == "above" { m >= market.threshold_f } else { m <= market.threshold_f })
                .count();
            let agreement = above_count.max(members.len() - above_count) as f64 / members.len() as f64;
            (agreement * 0.9).min(0.9)
        };

        let (nws_agrees, nws_note) = match nws_temp {
            Some(temp) => {
                let agrees = nws_agrees_with_yes(
                    temp,
                    &market.direction,
                    market.threshold_f,
                    market.range_low,
                    market.range_high,
                );
                let note = match direction {
                    SignalDirection::Yes if agrees => " | NWS AGREES ✓",
                    SignalDirection::Yes => " | NWS DISAGREES ✗",
                    SignalDirection::No if !agrees => " | NWS AGREES ✓",
                    SignalDirection::No => " | NWS DISAGREES ✗",
                };
                let direction_agrees = match direction {
                    SignalDirection::Yes => agrees,
                    SignalDirection::No => !agrees,
                };
                (Some(direction_agrees), note)
            }
            None => (None, ""),
        };

        let mut suggested_size = self.calculate_kelly_size(
            model_yes_prob,
            market_yes_prob,
            raw_direction,
            bankroll,
            recent_win_rate,
        );

        suggested_size *= 0.5 + 0.5 * confidence;

        if let Some(true) = nws_agrees {
            suggested_size *= 1.2;
        }
        if let Some(false) = nws_agrees {
            suggested_size *= 0.6;
        }

        suggested_size = suggested_size.min(self.config.max_trade_size);
        let passes_threshold = edge.abs() >= self.config.min_edge_threshold;

        let (ens_mean, ens_std) = if market.metric == "low" {
            (forecast.mean_low(), forecast.std_low())
        } else {
            (forecast.mean_high(), forecast.std_high())
        };

        let filter_status = if passes_threshold { "ACTIONABLE" } else { "FILTERED" };
        let reasoning = format!(
            "[{}] {} {} {} {:.0}°F | Ensemble: {:.1}±{:.1} ({}m) | Model {:.0} vs Market {:.0} | Edge {:+.1} → {} @ {:.0}{}",
            filter_status,
            market.city_name,
            market.metric,
            market.direction,
            market.threshold_f,
            ens_mean,
            ens_std,
            forecast.num_members,
            model_yes_prob,
            market_yes_prob,
            edge,
            direction,
            entry_price,
            nws_note,
        );

        info!("{}", reasoning);

        Ok(WeatherSignal {
            market_id: market.market_id.clone(),
            market_question: market.question.clone(),
            direction,
            model_probability: model_yes_prob,
            market_probability: market_yes_prob,
            edge,
            confidence,
            suggested_size,
            buy_token_id: String::new(),
            entry_price,
            passes_threshold,
            reasoning,
            ensemble_mean: ens_mean,
            ensemble_std: ens_std,
            ensemble_members: forecast.num_members,
            nws_forecast: nws_temp,
            nws_agrees,
            city_key: market.city_key.clone(),
            city_name: market.city_name.clone(),
            target_date: market.target_date.clone(),
            metric: market.metric.clone(),
            threshold_f: market.threshold_f,
            timestamp: Utc::now(),
        })
    }

    fn calculate_model_probability(&self, market: &WeatherMarketInfo, forecast: &EnsembleForecast) -> f64 {
        match (market.metric.as_str(), market.direction.as_str()) {
            ("low", "above") => forecast.probability_low_above(market.threshold_f),
            ("low", "below") => forecast.probability_low_below(market.threshold_f),
            ("low", "between") => match (market.range_low, market.range_high) {
                (Some(lo), Some(hi)) => forecast.probability_low_between(lo, hi),
                _ => 0.5,
            },
            (_, "above") => forecast.probability_high_above(market.threshold_f),
            (_, "below") => forecast.probability_high_below(market.threshold_f),
            (_, "between") => match (market.range_low, market.range_high) {
                (Some(lo), Some(hi)) => forecast.probability_high_between(lo, hi),
                _ => 0.5,
            },
            _ => 0.5,
        }
    }

    fn calculate_edge(&self, model_prob: f64, market_price: f64) -> (f64, &'static str) {
        let up_edge = model_prob - market_price;
        let down_edge = (1.0 - model_prob) - (1.0 - market_price);

        if up_edge >= down_edge {
            (up_edge, "up")
        } else {
            (down_edge, "down")
        }
    }

    fn calculate_kelly_size(
        &self,
        probability: f64,
        market_price: f64,
        direction: &str,
        bankroll: f64,
        recent_win_rate: Option<f64>,
    ) -> f64 {
        let (win_prob, price) = match direction {
            "up" => (probability, market_price),
            _ => (1.0 - probability, 1.0 - market_price),
        };

        if price <= 0.0 || price >= 1.0 || bankroll <= 0.0 {
            return 0.0;
        }

        let odds = (1.0 - price) / price;
        let lose_prob = 1.0 - win_prob;
        if odds == 0.0 {
            return 0.0;
        }

        let kelly_fraction = self.adjusted_kelly_fraction(recent_win_rate);
        if kelly_fraction <= 0.0 {
            return 0.0;
        }

        let kelly = ((win_prob * odds - lose_prob) / odds) * kelly_fraction;
        let kelly = kelly.clamp(0.0, self.config.kelly_max_fraction);
        (kelly * bankroll).min(self.config.max_trade_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_market() -> WeatherMarketInfo {
        WeatherMarketInfo {
            market_id: "test-market".into(),
            condition_id: "cond-123".into(),
            question: "Will the high temperature in New York be above 75°F on March 28?".into(),
            slug: "test".into(),
            city_key: "nyc".into(),
            city_name: "New York".into(),
            target_date: "2026-03-28".into(),
            metric: "high".into(),
            direction: "above".into(),
            threshold_f: 75.0,
            range_low: None,
            range_high: None,
            yes_price: 0.40,
            no_price: 0.60,
            token_id_yes: "yes-token".into(),
            token_id_no: "no-token".into(),
            active: true,
        }
    }

    fn make_forecast() -> EnsembleForecast {
        EnsembleForecast {
            city_key: "nyc".into(),
            target_date: "2026-03-28".into(),
            member_highs: vec![78.0, 80.0, 76.0, 79.0, 77.0, 81.0, 75.0, 78.0, 79.0, 80.0],
            member_lows: vec![60.0, 62.0, 58.0, 61.0, 59.0, 63.0, 57.0, 60.0, 61.0, 62.0],
            num_members: 10,
        }
    }

    #[test]
    fn test_weather_signal_generation() {
        let config = WenbotConfig::default();
        let strategy = WenbotStrategy::new(config);
        let market = make_market();
        let forecast = make_forecast();

        let signal = strategy.generate_signal(&market, &forecast, None, 100.0);
        assert!(signal.is_ok());

        let sig = signal.unwrap();
        assert_eq!(sig.direction, SignalDirection::Yes);
        assert!(sig.model_probability > 0.7);
        assert!(sig.edge.abs() >= 0.30);
        assert!(sig.passes_threshold);
    }

    #[test]
    fn test_nws_cross_validation_boost() {
        let config = WenbotConfig::default();
        let strategy = WenbotStrategy::new(config);
        let market = make_market();
        let forecast = make_forecast();

        let sig_agree = strategy.generate_signal(&market, &forecast, Some(80.0), 100.0).unwrap();
        let sig_disagree = strategy.generate_signal(&market, &forecast, Some(70.0), 100.0).unwrap();

        assert_eq!(sig_agree.nws_agrees, Some(true));
        assert_eq!(sig_disagree.nws_agrees, Some(false));
        assert!(sig_agree.suggested_size > sig_disagree.suggested_size);
    }

    #[test]
    fn test_entry_price_filter() {
        let config = WenbotConfig {
            max_entry_price: 0.40,
            ..Default::default()
        };
        let strategy = WenbotStrategy::new(config);
        let mut market = make_market();
        market.yes_price = 0.80;
        market.no_price = 0.20;

        let signal = strategy.generate_signal(&market, &make_forecast(), None, 100.0).unwrap();
        assert!(!signal.passes_threshold);
        assert!(signal.reasoning.contains("FILTERED"));
    }

    #[test]
    fn test_dynamic_kelly_adjustment() {
        let strategy = WenbotStrategy::new(WenbotConfig::default());
        assert_eq!(strategy.adjusted_kelly_fraction(Some(0.30)), 0.0);
        assert!(strategy.adjusted_kelly_fraction(Some(0.40)) < strategy.config().kelly_fraction);
        assert!(strategy.adjusted_kelly_fraction(Some(0.65)) > strategy.config().kelly_fraction);
    }
}

//! BTC microstructure indicators
//!
//! Computes technical indicators from 1-minute candle data.

use serde::{Deserialize, Serialize};

/// BTC microstructure data computed from 1-minute candles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BtcMicrostructure {
    pub price: f64,
    pub rsi: f64,
    pub momentum_1m: f64,
    pub momentum_5m: f64,
    pub momentum_15m: f64,
    pub vwap_deviation: f64,
    pub sma_crossover: f64,
    pub volume_trend: f64,
    pub bollinger_position: f64,
    pub volatility_regime: f64,
    pub change_1h: f64,
    pub change_24h: f64,
}

#[derive(Debug, Clone)]
pub struct Candle {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

pub fn compute_microstructure(candles: &[Candle]) -> Option<BtcMicrostructure> {
    if candles.len() < 2 {
        return None;
    }

    let current = candles.last()?;
    let price = current.close;

    let rsi = compute_rsi(candles, 14);
    let momentum_1m = momentum_at(candles, 1);
    let momentum_5m = momentum_at(candles, 5);
    let momentum_15m = momentum_at(candles, 15);
    let vwap_deviation = compute_vwap_deviation(candles);
    let sma_crossover = compute_sma_crossover(candles, 5, 15);
    let volume_trend = compute_volume_trend(candles);
    let bollinger_position = compute_bollinger_position(candles, 20, 2.0);
    let volatility_regime = compute_volatility_regime(candles, 20, 50);
    let change_1h = momentum_at(candles, 60);
    let change_24h = momentum_at(candles, 1440);

    Some(BtcMicrostructure {
        price,
        rsi,
        momentum_1m,
        momentum_5m,
        momentum_15m,
        vwap_deviation,
        sma_crossover,
        volume_trend,
        bollinger_position,
        volatility_regime,
        change_1h,
        change_24h,
    })
}

fn compute_rsi(candles: &[Candle], period: usize) -> f64 {
    if candles.len() < period + 1 {
        return 50.0;
    }

    let mut avg_gain = 0.0_f64;
    let mut avg_loss = 0.0_f64;

    for i in 1..=period {
        let change = candles[i].close - candles[i - 1].close;
        if change > 0.0 {
            avg_gain += change;
        } else {
            avg_loss += change.abs();
        }
    }
    avg_gain /= period as f64;
    avg_loss /= period as f64;

    let alpha = 1.0 / period as f64;
    for i in (period + 1)..candles.len() {
        let change = candles[i].close - candles[i - 1].close;
        let gain = if change > 0.0 { change } else { 0.0 };
        let loss = if change < 0.0 { change.abs() } else { 0.0 };
        avg_gain = avg_gain * (1.0 - alpha) + gain * alpha;
        avg_loss = avg_loss * (1.0 - alpha) + loss * alpha;
    }

    if avg_loss == 0.0 {
        return 100.0;
    }

    let rs = avg_gain / avg_loss;
    100.0 - (100.0 / (1.0 + rs))
}

fn momentum_at(candles: &[Candle], n: usize) -> f64 {
    if candles.len() < n + 1 {
        return 0.0;
    }

    let recent = candles[candles.len() - 1].close;
    let past = candles[candles.len() - 1 - n].close;
    (recent - past) / past * 100.0
}

fn compute_vwap_deviation(candles: &[Candle]) -> f64 {
    if candles.is_empty() {
        return 0.0;
    }

    let total_pv: f64 = candles.iter().map(|c| (c.high + c.low + c.close) / 3.0 * c.volume).sum();
    let total_v: f64 = candles.iter().map(|c| c.volume).sum();

    if total_v == 0.0 {
        return 0.0;
    }

    let vwap = total_pv / total_v;
    let price = candles.last().map(|c| c.close).unwrap_or(0.0);
    if vwap == 0.0 {
        0.0
    } else {
        (price - vwap) / vwap * 100.0
    }
}

fn compute_sma_crossover(candles: &[Candle], fast: usize, slow: usize) -> f64 {
    if candles.len() < slow {
        return 0.0;
    }

    let price = candles.last().map(|c| c.close).unwrap_or(0.0);
    if price == 0.0 {
        return 0.0;
    }

    let fast_sma: f64 = candles[candles.len() - fast..].iter().map(|c| c.close).sum::<f64>() / fast as f64;
    let slow_sma: f64 = candles[candles.len() - slow..].iter().map(|c| c.close).sum::<f64>() / slow as f64;

    (fast_sma - slow_sma) / price * 100.0
}

fn compute_volume_trend(candles: &[Candle]) -> f64 {
    if candles.len() < 14 {
        return 0.0;
    }

    let recent = &candles[candles.len() - 3..];
    let previous = &candles[candles.len() - 13..candles.len() - 3];
    let recent_avg = recent.iter().map(|c| c.volume).sum::<f64>() / recent.len() as f64;
    let previous_avg = previous.iter().map(|c| c.volume).sum::<f64>() / previous.len() as f64;
    if previous_avg <= 0.0 {
        return 0.0;
    }

    let direction = (candles.last().map(|c| c.close).unwrap_or(0.0) - recent.first().map(|c| c.open).unwrap_or(0.0)).signum();
    (((recent_avg / previous_avg) - 1.0) * direction).clamp(-1.0, 1.0)
}

fn compute_bollinger_position(candles: &[Candle], period: usize, std_mult: f64) -> f64 {
    if candles.len() < period {
        return 0.0;
    }

    let closes: Vec<f64> = candles[candles.len() - period..].iter().map(|c| c.close).collect();
    let mean = closes.iter().sum::<f64>() / closes.len() as f64;
    let variance = closes.iter().map(|c| (c - mean).powi(2)).sum::<f64>() / closes.len() as f64;
    let std_dev = variance.sqrt();
    if std_dev == 0.0 {
        return 0.0;
    }

    let upper = mean + std_mult * std_dev;
    let lower = mean - std_mult * std_dev;
    let width = upper - lower;
    if width == 0.0 {
        return 0.0;
    }

    let price = candles.last().map(|c| c.close).unwrap_or(mean);
    (((price - lower) / width) * 2.0 - 1.0).clamp(-1.0, 1.0)
}

fn compute_atr(candles: &[Candle], period: usize) -> Option<f64> {
    if candles.len() < period + 1 {
        return None;
    }

    let trs: Vec<f64> = candles[candles.len() - period..]
        .iter()
        .enumerate()
        .map(|(idx, candle)| {
            let prev_close = candles[candles.len() - period + idx - 1].close;
            let tr1 = candle.high - candle.low;
            let tr2 = (candle.high - prev_close).abs();
            let tr3 = (candle.low - prev_close).abs();
            tr1.max(tr2).max(tr3)
        })
        .collect();

    Some(trs.iter().sum::<f64>() / trs.len() as f64)
}

fn compute_volatility_regime(candles: &[Candle], current_period: usize, long_period: usize) -> f64 {
    let Some(current_atr) = compute_atr(candles, current_period) else { return 0.0; };
    let Some(long_atr) = compute_atr(candles, long_period) else { return 0.0; };
    if long_atr == 0.0 {
        return 0.0;
    }
    ((current_atr / long_atr) - 1.0).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_candles(prices: &[f64]) -> Vec<Candle> {
        prices
            .iter()
            .enumerate()
            .map(|(i, &p)| Candle {
                timestamp: i as i64,
                open: p,
                high: p * 1.001,
                low: p * 0.999,
                close: p,
                volume: 100.0 + i as f64,
            })
            .collect()
    }

    #[test]
    fn test_rsi_overbought() {
        let prices: Vec<f64> = (0..20).map(|i| 50000.0 + i as f64 * 100.0).collect();
        let candles = make_candles(&prices);
        let ms = compute_microstructure(&candles).unwrap();
        assert!(ms.rsi > 70.0);
    }

    #[test]
    fn test_rsi_oversold() {
        let prices: Vec<f64> = (0..20).map(|i| 60000.0 - i as f64 * 100.0).collect();
        let candles = make_candles(&prices);
        let ms = compute_microstructure(&candles).unwrap();
        assert!(ms.rsi < 30.0);
    }

    #[test]
    fn test_insufficient_candles() {
        let candles = make_candles(&[100.0]);
        assert!(compute_microstructure(&candles).is_none());
    }

    #[test]
    fn test_new_indicators_are_bounded() {
        let prices: Vec<f64> = (0..80).map(|i| 50000.0 + (i as f64 * 25.0)).collect();
        let candles = make_candles(&prices);
        let ms = compute_microstructure(&candles).unwrap();
        assert!((-1.0..=1.0).contains(&ms.volume_trend));
        assert!((-1.0..=1.0).contains(&ms.bollinger_position));
        assert!((-1.0..=1.0).contains(&ms.volatility_regime));
    }
}

use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

use anyhow::{bail, Context, Result};
use chrono::{DateTime, FixedOffset, NaiveDate, TimeZone, Timelike, Utc};
use fivesbot_strategy::{compute_microstructure, Candle, FivesbotConfig, FivesbotStrategy, StrategyError, StrategyProfile};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rand_distr::{Distribution, Normal};

use crate::data::{fetch_aggregated_15m, FetchRange};

const WARMUP_CANDLES: usize = 100;
const BANKROLL: f64 = 1_000.0;
const BJ_OFFSET_SECS: i32 = 8 * 3600;

#[derive(Debug, Clone, Copy)]
pub enum Symbol {
    BtcUsdt,
    EthUsdt,
}

impl Symbol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BtcUsdt => "BTCUSDT",
            Self::EthUsdt => "ETHUSDT",
        }
    }

    pub fn market_name(self) -> &'static str {
        match self {
            Self::BtcUsdt => "btc",
            Self::EthUsdt => "eth",
        }
    }
}

impl Display for Symbol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Symbol {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.to_ascii_uppercase().as_str() {
            "BTCUSDT" => Ok(Self::BtcUsdt),
            "ETHUSDT" => Ok(Self::EthUsdt),
            other => bail!("unsupported symbol {other}; expected BTCUSDT or ETHUSDT"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BacktestConfig {
    pub symbol: Symbol,
    pub profile: String,
    pub max_entry: f64,
    pub min_edge: f64,
    pub min_confidence: f64,
    pub night_only: bool,
    pub seed: u64,
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub show_trades: bool,
}

#[derive(Debug, Clone)]
pub struct TradeRecord {
    pub timestamp_bj: DateTime<FixedOffset>,
    pub direction: String,
    pub entry_price: f64,
    pub model_prob: f64,
    pub market_prob: f64,
    pub edge: f64,
    pub confidence: f64,
    pub size: f64,
    pub won: bool,
    pub pnl: f64,
}

#[derive(Debug, Clone, Default)]
pub struct FilterStats {
    pub generated: usize,
    pub taken: usize,
    pub entry_price_filtered: usize,
    pub edge_filtered: usize,
    pub confidence_filtered: usize,
    pub convergence_filtered: usize,
    pub time_filtered: usize,
}

#[derive(Debug, Clone)]
pub struct BucketStats {
    pub label: String,
    pub trades: usize,
    pub wins: usize,
    pub pnl: f64,
}

#[derive(Debug, Clone)]
pub struct BacktestReport {
    pub symbol: Symbol,
    pub profile: String,
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub filter_stats: FilterStats,
    pub trades: Vec<TradeRecord>,
    pub total_pnl: f64,
    pub avg_pnl: f64,
    pub avg_edge: f64,
    pub avg_confidence: f64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub max_drawdown: f64,
    pub sharpe: f64,
    pub by_hour: Vec<BucketStats>,
    pub by_entry_price: Vec<BucketStats>,
    pub candle_count: usize,
}

pub struct BacktestEngine {
    config: BacktestConfig,
}

impl BacktestEngine {
    pub fn new(config: BacktestConfig) -> Self {
        Self { config }
    }

    pub async fn run(&self) -> Result<BacktestReport> {
        let candles = fetch_aggregated_15m(self.config.symbol.as_str(), &FetchRange::new(self.config.from, self.config.to)).await?;
        if candles.len() < WARMUP_CANDLES + 2 {
            bail!("not enough 15m candles after warmup: {}", candles.len());
        }

        let profile = profile_from_name(&self.config.profile)?;
        let strategy = FivesbotStrategy::with_profile(strategy_config(&self.config), profile.clone());
        let mut rng = StdRng::seed_from_u64(self.config.seed);
        let normal = Normal::new(0.0, 0.09).context("invalid noise config")?;
        let offset = FixedOffset::east_opt(BJ_OFFSET_SECS).unwrap();

        let request_start = self.config.from.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let request_end = self.config.to.and_hms_opt(23, 59, 59).unwrap().and_utc();

        let mut filters = FilterStats::default();
        let mut trades = Vec::new();
        let mut equity_curve = Vec::new();
        let mut cumulative_pnl = 0.0;

        for idx in WARMUP_CANDLES..(candles.len() - 1) {
            let current = &candles[idx];
            let current_time = millis_to_utc(current.timestamp)?;
            if current_time < request_start || current_time > request_end {
                continue;
            }
            if self.config.night_only && !is_night_session(current_time.with_timezone(&offset)) {
                filters.time_filtered += 1;
                continue;
            }

            let window = &candles[..=idx];
            let Some(micro) = compute_microstructure(window) else { continue; };
            filters.generated += 1;

            let up_price = simulate_market_price(window, micro.price, &mut rng, &normal);
            let down_price = 1.0 - up_price;
            let seconds_remaining = 15 * 60;

            match strategy.generate_signal(
                self.config.symbol.market_name(),
                &micro,
                up_price,
                down_price,
                seconds_remaining,
                BANKROLL,
            ) {
                Ok(signal) => {
                    if signal.confidence < self.config.min_confidence {
                        filters.confidence_filtered += 1;
                        continue;
                    }
                    let next = &candles[idx + 1];
                    let won = match signal.effective_direction.as_str() {
                        "up" => next.close > next.open,
                        _ => next.close <= next.open,
                    };
                    let pnl = if won {
                        (1.0 - signal.buy_price) * signal.suggested_size
                    } else {
                        -signal.buy_price * signal.suggested_size
                    };
                    cumulative_pnl += pnl;
                    equity_curve.push(cumulative_pnl);
                    trades.push(TradeRecord {
                        timestamp_bj: current_time.with_timezone(&offset),
                        direction: signal.effective_direction,
                        entry_price: signal.buy_price,
                        model_prob: signal.model_probability,
                        market_prob: signal.market_probability,
                        edge: signal.edge,
                        confidence: signal.confidence,
                        size: signal.suggested_size,
                        won,
                        pnl,
                    });
                    filters.taken += 1;
                }
                Err(StrategyError::EntryPriceTooHigh { .. }) => filters.entry_price_filtered += 1,
                Err(StrategyError::InsufficientEdge { .. }) => {
                    let convergence = count_convergence(window, up_price, &profile);
                    if convergence < profile.min_convergence_votes {
                        filters.convergence_filtered += 1;
                    } else {
                        filters.edge_filtered += 1;
                    }
                }
                Err(StrategyError::TooCloseToExpiry { .. } | StrategyError::TooFresh { .. }) => filters.time_filtered += 1,
                Err(other) => return Err(anyhow::anyhow!(other)).context("strategy evaluation failed"),
            }
        }

        let total_pnl: f64 = trades.iter().map(|t| t.pnl).sum();
        let wins = trades.iter().filter(|t| t.won).count();
        let win_rate = if trades.is_empty() { 0.0 } else { wins as f64 / trades.len() as f64 };
        let avg_pnl = if trades.is_empty() { 0.0 } else { total_pnl / trades.len() as f64 };
        let avg_edge = average(trades.iter().map(|t| t.edge));
        let avg_confidence = average(trades.iter().map(|t| t.confidence));
        let gross_profit: f64 = trades.iter().filter(|t| t.pnl > 0.0).map(|t| t.pnl).sum();
        let gross_loss: f64 = trades.iter().filter(|t| t.pnl < 0.0).map(|t| t.pnl.abs()).sum();
        let profit_factor = if gross_loss == 0.0 { 0.0 } else { gross_profit / gross_loss };
        let max_drawdown = max_drawdown(&equity_curve);
        let sharpe = annualized_sharpe(&trades);

        Ok(BacktestReport {
            symbol: self.config.symbol,
            profile: self.config.profile.clone(),
            from: self.config.from,
            to: self.config.to,
            filter_stats: filters,
            trades: trades.clone(),
            total_pnl,
            avg_pnl,
            avg_edge,
            avg_confidence,
            win_rate,
            profit_factor,
            max_drawdown,
            sharpe,
            by_hour: bucket_by_hour(&trades),
            by_entry_price: bucket_by_entry_price(&trades),
            candle_count: candles.len(),
        })
    }
}

fn strategy_config(config: &BacktestConfig) -> FivesbotConfig {
    FivesbotConfig {
        markets: vec![config.symbol.market_name().to_string()],
        max_entry_price: config.max_entry,
        min_edge_threshold: config.min_edge,
        min_time_remaining: 0,
        max_time_remaining: 3600,
        ..FivesbotConfig::default()
    }
}

fn profile_from_name(name: &str) -> Result<StrategyProfile> {
    match name {
        "classic_four" => Ok(StrategyProfile::classic_four()),
        "btc_eight_indicator" => Ok(StrategyProfile::btc_eight_indicator()),
        other => bail!("unsupported profile {other}"),
    }
}

fn millis_to_utc(ts: i64) -> Result<DateTime<Utc>> {
    Utc.timestamp_millis_opt(ts)
        .single()
        .ok_or_else(|| anyhow::anyhow!("invalid timestamp {ts}"))
}

fn is_night_session(ts: DateTime<FixedOffset>) -> bool {
    let hour = ts.hour();
    hour >= 23 || hour < 9
}

fn simulate_market_price(candles: &[Candle], last_price: f64, rng: &mut StdRng, normal: &Normal<f64>) -> f64 {
    let recent_change = if candles.len() >= 4 {
        (last_price - candles[candles.len() - 4].close) / candles[candles.len() - 4].close
    } else {
        0.0
    };
    let baseline = if candles.len() >= 20 {
        candles[candles.len() - 20..].iter().map(|c| c.close).sum::<f64>() / 20.0
    } else {
        last_price
    };
    let trend_bias = if baseline > 0.0 {
        ((last_price - baseline) / baseline * 20.0).clamp(-0.10, 0.10)
    } else {
        0.0
    };
    let drift = (recent_change * 10.0).clamp(-0.10, 0.10) + trend_bias;
    let noise = normal.sample(rng).clamp(-0.14, 0.14);
    (0.5 + drift + noise + rng.gen_range(-0.015..0.015)).clamp(0.28, 0.72)
}

fn count_convergence(candles: &[Candle], up_price: f64, profile: &StrategyProfile) -> u32 {
    let Some(micro) = compute_microstructure(candles) else { return 0; };
    let rsi_signal = if micro.rsi < 30.0 {
        (0.5 + (30.0 - micro.rsi) / 30.0).clamp(-1.0, 1.0)
    } else if micro.rsi > 70.0 {
        (-0.5 - (micro.rsi - 70.0) / 30.0).clamp(-1.0, 1.0)
    } else if micro.rsi < 45.0 {
        ((45.0 - micro.rsi) / 30.0).clamp(-1.0, 1.0)
    } else if micro.rsi > 55.0 {
        (-(micro.rsi - 55.0) / 30.0).clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let momentum_signal = (micro.momentum_1m * 0.5 + micro.momentum_5m * 0.35 + micro.momentum_15m * 0.15).clamp(-1.0, 1.0);
    let vwap_signal = (micro.vwap_deviation / 0.05).clamp(-1.0, 1.0);
    let sma_signal = (micro.sma_crossover / 0.03).clamp(-1.0, 1.0);
    let market_skew = (up_price - 0.50) * -4.0;
    let volume_trend_signal = micro.volume_trend.clamp(-1.0, 1.0);
    let bollinger_signal = micro.bollinger_position.clamp(-1.0, 1.0);
    let volatility_signal = micro.volatility_regime.clamp(-1.0, 1.0);
    let indicators = [
        (rsi_signal, true),
        (momentum_signal, true),
        (vwap_signal, true),
        (sma_signal, true),
        (market_skew, profile.market_skew_weight > 0.0),
        (volume_trend_signal, profile.volume_trend_weight > 0.0),
        (bollinger_signal, profile.bollinger_weight > 0.0),
        (volatility_signal, profile.volatility_weight > 0.0),
    ];
    let up_votes = indicators.iter().filter(|(value, enabled)| *enabled && *value > 0.05).count() as u32;
    let down_votes = indicators.iter().filter(|(value, enabled)| *enabled && *value < -0.05).count() as u32;
    up_votes.max(down_votes)
}

fn average<I>(iter: I) -> f64
where
    I: Iterator<Item = f64>,
{
    let values: Vec<f64> = iter.collect();
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn max_drawdown(equity_curve: &[f64]) -> f64 {
    let mut peak = 0.0;
    let mut worst = 0.0;
    for value in equity_curve {
        if *value > peak {
            peak = *value;
        }
        let drawdown = *value - peak;
        if drawdown < worst {
            worst = drawdown;
        }
    }
    worst
}

fn annualized_sharpe(trades: &[TradeRecord]) -> f64 {
    if trades.len() < 2 {
        return 0.0;
    }
    let returns: Vec<f64> = trades.iter().map(|t| t.pnl / BANKROLL).collect();
    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (returns.len() as f64 - 1.0);
    let std_dev = variance.sqrt();
    if std_dev == 0.0 {
        return 0.0;
    }
    mean / std_dev * (96.0_f64 * 365.0_f64).sqrt()
}

fn bucket_by_hour(trades: &[TradeRecord]) -> Vec<BucketStats> {
    let labels = ["09-12", "12-15", "15-18", "18-21", "21-23", "23-09"];
    let mut buckets: BTreeMap<&str, Vec<&TradeRecord>> = labels.into_iter().map(|label| (label, Vec::new())).collect();
    for trade in trades {
        let label = match trade.timestamp_bj.hour() {
            9..=11 => "09-12",
            12..=14 => "12-15",
            15..=17 => "15-18",
            18..=20 => "18-21",
            21..=22 => "21-23",
            _ => "23-09",
        };
        buckets.get_mut(label).unwrap().push(trade);
    }
    labels
        .into_iter()
        .map(|label| summarize_bucket(label, buckets.get(label).unwrap()))
        .collect()
}

fn bucket_by_entry_price(trades: &[TradeRecord]) -> Vec<BucketStats> {
    let labels = ["< 0.35", "[0.35,0.40)", "[0.40,0.45)", ">= 0.45"];
    let mut buckets: BTreeMap<&str, Vec<&TradeRecord>> = labels.into_iter().map(|label| (label, Vec::new())).collect();
    for trade in trades {
        let label = if trade.entry_price < 0.35 {
            "< 0.35"
        } else if trade.entry_price < 0.40 {
            "[0.35,0.40)"
        } else if trade.entry_price < 0.45 {
            "[0.40,0.45)"
        } else {
            ">= 0.45"
        };
        buckets.get_mut(label).unwrap().push(trade);
    }
    labels
        .into_iter()
        .map(|label| summarize_bucket(label, buckets.get(label).unwrap()))
        .collect()
}

fn summarize_bucket(label: &str, trades: &[&TradeRecord]) -> BucketStats {
    BucketStats {
        label: label.to_string(),
        trades: trades.len(),
        wins: trades.iter().filter(|t| t.won).count(),
        pnl: trades.iter().map(|t| t.pnl).sum(),
    }
}

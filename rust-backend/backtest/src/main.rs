mod data;
mod engine;
mod report;

use std::str::FromStr;

use anyhow::{bail, Context, Result};
use chrono::{Duration, NaiveDate, Utc};
use clap::{Parser, ValueEnum};
use engine::{BacktestConfig, BacktestEngine, Symbol};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ProfileArg {
    #[value(name = "classic_four", alias = "classic-four")]
    ClassicFour,
    #[value(name = "btc_eight_indicator", alias = "btc-eight-indicator")]
    BtcEightIndicator,
}

impl ProfileArg {
    fn as_str(self) -> &'static str {
        match self {
            Self::ClassicFour => "classic_four",
            Self::BtcEightIndicator => "btc_eight_indicator",
        }
    }
}

#[derive(Debug, Parser)]
#[command(name = "backtest", about = "Backtest the fivesbot strategy on Binance spot candles")]
struct Cli {
    #[arg(long)]
    symbol: String,
    #[arg(long, default_value_t = 7)]
    days: i64,
    #[arg(long, value_enum, default_value_t = ProfileArg::ClassicFour)]
    profile: ProfileArg,
    #[arg(long, default_value_t = 0.45)]
    max_entry: f64,
    #[arg(long, default_value_t = 0.08)]
    min_edge: f64,
    #[arg(long, default_value_t = 0.0)]
    min_confidence: f64,
    #[arg(long, default_value_t = false)]
    night_only: bool,
    #[arg(long)]
    seed: Option<u64>,
    #[arg(long)]
    from: Option<String>,
    #[arg(long)]
    to: Option<String>,
    #[arg(long, default_value_t = false)]
    trades: bool,
}

fn parse_date(value: Option<String>, name: &str) -> Result<Option<NaiveDate>> {
    value
        .map(|raw| NaiveDate::parse_from_str(&raw, "%Y-%m-%d").with_context(|| format!("invalid {name} date: {raw}")))
        .transpose()
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("warn").without_time().init();

    let cli = Cli::parse();
    let symbol = Symbol::from_str(&cli.symbol)?;
    let from = parse_date(cli.from, "from")?;
    let to = parse_date(cli.to, "to")?;

    let (from, to) = match (from, to) {
        (Some(from), Some(to)) => {
            if from > to {
                bail!("--from must be <= --to");
            }
            (from, to)
        }
        (Some(from), None) => (from, from + Duration::days(cli.days.max(1) - 1)),
        (None, Some(to)) => (to - Duration::days(cli.days.max(1) - 1), to),
        (None, None) => {
            let to = Utc::now().date_naive();
            (to - Duration::days(cli.days.max(1) - 1), to)
        }
    };

    let config = BacktestConfig {
        symbol,
        profile: cli.profile.as_str().to_string(),
        max_entry: cli.max_entry,
        min_edge: cli.min_edge,
        min_confidence: cli.min_confidence,
        night_only: cli.night_only,
        seed: cli.seed.unwrap_or(42),
        from,
        to,
        show_trades: cli.trades,
    };

    let engine = BacktestEngine::new(config.clone());
    let report = engine.run().await?;
    println!("{}", report::render_report(&report, config.show_trades));
    Ok(())
}

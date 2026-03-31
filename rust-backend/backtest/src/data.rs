use std::str::FromStr;

use anyhow::{anyhow, bail, Context, Result};
use chrono::{Duration, NaiveDate, Timelike, Utc};
use fivesbot_strategy::Candle;
use reqwest::Client;
use serde_json::Value;

const BINANCE_KLINES_URL: &str = "https://data-api.binance.vision/api/v3/klines";
const BINANCE_LIMIT: usize = 1000;
const AGGREGATION_MINUTES: i64 = 15;
const WARMUP_CANDLES: i64 = 120;

#[derive(Debug, Clone)]
pub struct FetchRange {
    pub requested_to: NaiveDate,
    pub warmup_from: NaiveDate,
}

impl FetchRange {
    pub fn new(requested_from: NaiveDate, requested_to: NaiveDate) -> Self {
        let warmup_minutes = WARMUP_CANDLES * AGGREGATION_MINUTES;
        let warmup_from = requested_from - Duration::minutes(warmup_minutes);
        Self { requested_to, warmup_from }
    }
}

pub async fn fetch_aggregated_15m(symbol: &str, range: &FetchRange) -> Result<Vec<Candle>> {
    let client = Client::builder().no_proxy().build().context("failed to create reqwest client")?;

    let requested_start = range.warmup_from.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp_millis();
    let requested_end = (range.requested_to + Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis();

    let one_minute = fetch_one_minute_klines(&client, symbol, requested_start, requested_end).await?;
    let filtered: Vec<Candle> = one_minute
        .into_iter()
        .filter(|c| c.timestamp >= requested_start && c.timestamp < requested_end)
        .collect();

    aggregate_15m(&filtered)
}

async fn fetch_one_minute_klines(
    client: &Client,
    symbol: &str,
    requested_start: i64,
    requested_end: i64,
) -> Result<Vec<Candle>> {
    let mut all_candles = Vec::new();
    let mut cursor_end = requested_end;

    while cursor_end > requested_start {
        let end_time = cursor_end - 1;
        let response = client
            .get(BINANCE_KLINES_URL)
            .query(&[
                ("symbol", symbol.to_string()),
                ("interval", "1m".to_string()),
                ("limit", BINANCE_LIMIT.to_string()),
                ("endTime", end_time.to_string()),
            ])
            .send()
            .await
            .with_context(|| format!("failed to fetch klines for {symbol} ending at {end_time}"))?;

        if !response.status().is_success() {
            bail!("Binance returned {} for {}", response.status(), BINANCE_KLINES_URL);
        }

        let rows: Vec<Vec<Value>> = response.json().await.context("failed to parse Binance klines response")?;
        if rows.is_empty() {
            break;
        }

        let mut batch = Vec::with_capacity(rows.len());
        for row in rows {
            batch.push(parse_kline_row(&row)?);
        }

        batch.sort_by_key(|c| c.timestamp);
        let first_open_time = batch
            .first()
            .map(|c| c.timestamp)
            .ok_or_else(|| anyhow!("Binance returned an empty kline batch"))?;

        all_candles.extend(batch);

        if first_open_time <= requested_start {
            break;
        }
        cursor_end = first_open_time;
    }

    if all_candles.is_empty() {
        bail!("no candle data returned for {symbol}");
    }

    all_candles.sort_by_key(|c| c.timestamp);
    all_candles.dedup_by_key(|c| c.timestamp);
    Ok(all_candles)
}

fn parse_kline_row(row: &[Value]) -> Result<Candle> {
    if row.len() < 6 {
        bail!("kline row had {} columns, expected at least 6", row.len());
    }

    Ok(Candle {
        timestamp: row[0].as_i64().ok_or_else(|| anyhow!("invalid open_time"))?,
        open: parse_json_number_string(&row[1]).context("invalid open")?,
        high: parse_json_number_string(&row[2]).context("invalid high")?,
        low: parse_json_number_string(&row[3]).context("invalid low")?,
        close: parse_json_number_string(&row[4]).context("invalid close")?,
        volume: parse_json_number_string(&row[5]).context("invalid volume")?,
    })
}

fn parse_json_number_string(value: &Value) -> Result<f64> {
    let text = value.as_str().ok_or_else(|| anyhow!("expected string numeric field"))?;
    f64::from_str(text).with_context(|| format!("failed to parse number: {text}"))
}

fn aggregate_15m(candles: &[Candle]) -> Result<Vec<Candle>> {
    let mut output = Vec::new();
    let mut bucket: Vec<&Candle> = Vec::with_capacity(15);

    for candle in candles {
        let minute = chrono::DateTime::<Utc>::from_timestamp_millis(candle.timestamp)
            .ok_or_else(|| anyhow!("invalid timestamp {}", candle.timestamp))?
            .minute() as i64;
        if minute % AGGREGATION_MINUTES == 0 && !bucket.is_empty() {
            if bucket.len() == 15 {
                output.push(collapse_bucket(&bucket));
            }
            bucket.clear();
        }
        bucket.push(candle);
        if bucket.len() == 15 {
            output.push(collapse_bucket(&bucket));
            bucket.clear();
        }
    }

    if output.is_empty() {
        bail!("no 15m candles produced");
    }
    Ok(output)
}

fn collapse_bucket(bucket: &[&Candle]) -> Candle {
    let first = bucket.first().unwrap();
    let last = bucket.last().unwrap();
    Candle {
        timestamp: first.timestamp,
        open: first.open,
        high: bucket.iter().map(|c| c.high).fold(f64::MIN, f64::max),
        low: bucket.iter().map(|c| c.low).fold(f64::MAX, f64::min),
        close: last.close,
        volume: bucket.iter().map(|c| c.volume).sum(),
    }
}

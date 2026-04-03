use chrono::{Datelike, Duration, Utc};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use tokio::sync::Mutex;
use tracing::{info, warn};

use wenbot_strategy::{EnsembleForecast, WeatherMarketInfo};

/// In-memory cache for ensemble forecasts to avoid hitting Open-Meteo rate limits.
/// Key: "{city_key}|{target_date}", Value: (forecast, inserted_at).
/// TTL: 30 minutes.
struct EnsembleCache {
    entries: HashMap<String, (EnsembleForecast, Instant)>,
    ttl_secs: u64,
    /// If API returned rate-limit error, remember when to retry.
    rate_limited_until: Option<Instant>,
}

impl EnsembleCache {
    fn new(ttl_secs: u64) -> Self {
        Self {
            entries: HashMap::new(),
            ttl_secs,
            rate_limited_until: None,
        }
    }

    fn get(&self, key: &str) -> Option<&EnsembleForecast> {
        let (forecast, inserted) = self.entries.get(key)?;
        if inserted.elapsed().as_secs() < self.ttl_secs {
            Some(forecast)
        } else {
            None
        }
    }

    fn insert(&mut self, key: String, forecast: EnsembleForecast) {
        let before = self.entries.len();
        self.entries.insert(key, (forecast, Instant::now()));
        if self.entries.len() != before {
            info!(
                "ensemble_cache: inserted, total cached entries: {}",
                self.entries.len()
            );
        }
    }

    /// Mark as rate-limited; no API calls will be made until the deadline passes.
    fn set_rate_limited(&mut self, for_secs: u64) {
        self.rate_limited_until = Some(Instant::now() + std::time::Duration::from_secs(for_secs));
        warn!(
            "ensemble_cache: API rate-limited, backing off for {}s",
            for_secs
        );
    }

    /// Returns true if we're currently in a rate-limit cooldown period.
    fn is_rate_limited(&self) -> bool {
        self.rate_limited_until
            .map(|t| Instant::now() < t)
            .unwrap_or(false)
    }

    /// Remove expired entries, return count removed.
    fn evict_expired(&mut self) -> usize {
        let before = self.entries.len();
        self.entries
            .retain(|_, (_, inserted)| inserted.elapsed().as_secs() < self.ttl_secs);
        before - self.entries.len()
    }
}

/// Global ensemble cache: 30-minute TTL.
static ENSEMBLE_CACHE: Mutex<Option<EnsembleCache>> = Mutex::const_new(None);

/// Global market bundle cache: 5-minute TTL to avoid slow API calls on every frontend request.
static BUNDLE_CACHE: Mutex<Option<(Vec<DailyTempMarketBundle>, std::time::Instant)>> = Mutex::const_new(None);

const BUNDLE_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(300); // 5 minutes

async fn get_ensemble_cache() -> tokio::sync::MutexGuard<'static, Option<EnsembleCache>> {
    let mut cache = ENSEMBLE_CACHE.lock().await;
    if cache.is_none() {
        *cache = Some(EnsembleCache::new(30 * 60)); // 30 min TTL
    }
    cache
}

#[derive(Debug, Clone)]
pub struct DailyTempMarketBundle {
    pub market: WeatherMarketInfo,
    pub forecast: EnsembleForecast,
    pub nws_temp_f: Option<f64>,
}

/// Fetch markets with caching (for API routes that need fast responses).
pub async fn fetch_daily_temperature_markets() -> Result<Vec<DailyTempMarketBundle>, Box<dyn std::error::Error + Send + Sync>> {
    // Check cache first
    {
        let cache = BUNDLE_CACHE.lock().await;
        if let Some((ref bundles, fetched_at)) = *cache {
            if fetched_at.elapsed() < BUNDLE_CACHE_TTL {
                info!("bundle_cache: HIT ({} bundles, age {:.0}s)", bundles.len(), fetched_at.elapsed().as_secs());
                return Ok(bundles.clone());
            }
        }
    }

    // Cache miss - fetch fresh data
    let bundles = fetch_daily_temperature_markets_inner().await?;

    // Update cache
    {
        let mut cache = BUNDLE_CACHE.lock().await;
        *cache = Some((bundles.clone(), std::time::Instant::now()));
    }
    info!("bundle_cache: MISS, fetched {} bundles", bundles.len());

    Ok(bundles)
}

/// Internal fetch without caching (called by scheduler which has its own timing).
pub async fn fetch_daily_temperature_markets_inner() -> Result<Vec<DailyTempMarketBundle>, Box<dyn std::error::Error + Send + Sync>> {
    let today = Utc::now().date_naive();
    let tomorrow = today + Duration::days(1);
    let allowed_dates: HashSet<String> = [today, tomorrow]
        .into_iter()
        .map(|d| d.format("%Y-%m-%d").to_string())
        .collect();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("wenbot-rust/0.1.0")
        .no_proxy()
        .build()?;

    let mut bundles = Vec::new();
    let mut seen = HashSet::new();

    for event in fetch_weather_events(&client).await? {
        let markets = event
            .get("markets")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        for market_json in markets {
            let closed = market_json.get("closed").and_then(|v| v.as_bool()).unwrap_or(true);
            if closed {
                continue;
            }
            if let Some(bundle) = parse_polymarket_market(&client, &market_json, &allowed_dates).await {
                let dedupe_key = format!("{}:{}", bundle.market.market_id, bundle.market.direction);
                if seen.insert(dedupe_key) {
                    bundles.push(bundle);
                }
            }
        }
    }

    Ok(bundles)
}

pub async fn fetch_actual_temperature(city_name: &str, target_date: &str, metric: &str) -> Option<f64> {
    let (lat, lon) = city_coords(city_name)?;
    let url = format!(
        "https://archive-api.open-meteo.com/v1/archive?latitude={lat}&longitude={lon}&start_date={date}&end_date={date}&daily=temperature_2m_max,temperature_2m_min&timezone=UTC",
        lat = lat,
        lon = lon,
        date = target_date,
    );
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(15)).build().ok()?;
    let resp: Value = client.get(url).send().await.ok()?.json().await.ok()?;
    let daily = resp.get("daily")?.as_object()?;
    let key = if metric == "low" { "temperature_2m_min" } else { "temperature_2m_max" };
    let temp_c = daily.get(key)?.as_array()?.first()?.as_f64()?;
    Some(c_to_f(temp_c))
}

/// Known weather cities with their Polymarket slug format
const WEATHER_CITIES: &[&str] = &[
    "austin", "beijing", "new-york", "chicago", "miami", "los-angeles",
    "denver", "seattle", "atlanta", "dallas", "houston", "phoenix",
];

/// Known slug prefixes for temperature markets
const TEMP_SLUG_PREFIXES: &[&str] = &[
    "highest-temperature-in",
    "lowest-temperature-in",
];

async fn fetch_weather_events(client: &reqwest::Client) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    let today = Utc::now().date_naive();
    let month_names = ["", "january", "february", "march", "april", "may", "june",
        "july", "august", "september", "october", "november", "december"];

    // Build all slug queries upfront (12 cities x 2 prefixes x 2 days = 48)
    let mut slug_queries: Vec<String> = Vec::with_capacity(WEATHER_CITIES.len() * TEMP_SLUG_PREFIXES.len() * 2);
    for day_offset in 0..=1i64 {
        let date = today + Duration::days(day_offset);
        let month = month_names[date.month() as usize];
        let day = date.day();
        let year = date.year();
        for city in WEATHER_CITIES {
            for prefix in TEMP_SLUG_PREFIXES {
                slug_queries.push(format!("{}-{}-on-{}-{}-{}", prefix, city, month, day, year));
            }
        }
    }

    // Fire all 48 requests concurrently
    let client_ref = client.clone();
    let handles: Vec<_> = slug_queries.into_iter().map(|slug| {
        let c = client_ref.clone();
        tokio::spawn(async move {
            let resp = c
                .get("https://gamma-api.polymarket.com/events")
                .query(&[("slug", slug.as_str())])
                .send()
                .await;
            match resp {
                Ok(r) if r.status().is_success() => r.json::<Vec<Value>>().await.unwrap_or_default(),
                _ => Vec::new(),
            }
        })
    }).collect();

    // Collect results
    let mut events = Vec::new();
    let mut seen_slugs = HashSet::new();
    for handle in handles {
        match handle.await {
            Ok(batch) => {
                for ev in &batch {
                    let ev_slug = ev.get("slug").and_then(|v| v.as_str()).unwrap_or("");
                    if !ev_slug.is_empty() && seen_slugs.insert(ev_slug.to_string()) {
                        events.push(ev.clone());
                    }
                }
            }
            Err(e) => warn!("weather_markets: spawn join error: {}", e),
        }
    }

    warn!("weather_markets: discovered {} events across {} cities x 2 days (parallel fetch)", events.len(), WEATHER_CITIES.len());
    Ok(events)
}

async fn parse_polymarket_market(
    client: &reqwest::Client,
    m: &Value,
    allowed_dates: &HashSet<String>,
) -> Option<DailyTempMarketBundle> {
    let question = m.get("question")?.as_str()?.to_string();
    let slug = m.get("slug").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let q_lower = question.to_lowercase();

    if !q_lower.contains("temperature") {
        return None;
    }

    let outcomes = parse_json_string_or_array_str(m.get("outcomes"));
    let price_vals = parse_json_string_or_array_f64(m.get("outcomePrices"));
    let token_ids = parse_json_string_or_array_str(m.get("clobTokenIds"));
    if outcomes.len() < 2 || price_vals.len() < 2 || token_ids.len() < 2 {
        return None;
    }

    let city_name = extract_city(&question).unwrap_or_else(|| "Unknown".into());
    let city_key = city_name.to_lowercase().replace(' ', "_");
    let metric = if q_lower.contains("low temperature") || q_lower.contains("lowest") { "low" } else { "high" };
    let parsed = match parse_temp_threshold(&question) {
        Some(p) => p,
        None => {
            warn!("weather_markets: skipped (no threshold) question={}", &question[..question.len().min(80)]);
            return None;
        }
    };

    let target_date = match extract_target_date(&question) {
        Some(d) => d,
        None => {
            warn!("weather_markets: skipped (no date) question={}", &question[..question.len().min(80)]);
            return None;
        }
    };
    if !allowed_dates.contains(&target_date) {
        warn!("weather_markets: skipped (date {} not in allowed) question={}", target_date, &question[..question.len().min(80)]);
        return None;
    }

    let (lat, lon) = match city_coords(&city_name) {
        Some(c) => c,
        None => {
            warn!("weather_markets: skipped (no coords for city '{}')", city_name);
            return None;
        }
    };
    let forecast = match fetch_forecast(client, lat, lon, &city_key, &target_date).await {
        Some(f) => f,
        None => {
            warn!("weather_markets: skipped (ensemble fetch failed) city={} date={}", city_name, target_date);
            return None;
        }
    };
    let nws_temp_f = fetch_nws_temperature(client, lat, lon, &target_date, metric).await;

    let condition_id = m.get("conditionId").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let stable_market_id = if !condition_id.is_empty() {
        condition_id.clone()
    } else if !slug.is_empty() {
        slug.clone()
    } else {
        question.clone()
    };

    let market = WeatherMarketInfo {
        market_id: stable_market_id,
        condition_id,
        question: question.clone(),
        slug,
        city_key,
        city_name,
        target_date,
        metric: metric.to_string(),
        direction: parsed.direction,
        threshold_f: parsed.threshold,
        range_low: parsed.range_low,
        range_high: parsed.range_high,
        yes_price: price_vals[0],
        no_price: price_vals[1],
        token_id_yes: token_ids[0].clone(),
        token_id_no: token_ids[1].clone(),
        active: true,
    };

    Some(DailyTempMarketBundle { market, forecast, nws_temp_f })
}

struct ParsedThreshold {
    direction: String,
    threshold: f64,
    range_low: Option<f64>,
    range_high: Option<f64>,
}

fn parse_temp_threshold(question: &str) -> Option<ParsedThreshold> {
    let q = question.to_lowercase().replace('°', "").replace('º', "");
    if let Some(idx) = q.find(" or lower") {
        let thresh = last_number(&q[..idx])?;
        return Some(ParsedThreshold { direction: "below".into(), threshold: thresh, range_low: None, range_high: None });
    }
    if let Some(idx) = q.find(" or below") {
        let thresh = last_number(&q[..idx])?;
        return Some(ParsedThreshold { direction: "below".into(), threshold: thresh, range_low: None, range_high: None });
    }
    if let Some(idx) = q.find(" or higher") {
        let thresh = last_number(&q[..idx])?;
        return Some(ParsedThreshold { direction: "above".into(), threshold: thresh, range_low: None, range_high: None });
    }
    if let Some(idx) = q.find(" or above") {
        let thresh = last_number(&q[..idx])?;
        return Some(ParsedThreshold { direction: "above".into(), threshold: thresh, range_low: None, range_high: None });
    }
    if let Some(idx) = q.find(" between ") {
        let rest = &q[idx + 9..];
        let numbers: Vec<f64> = rest
            .split(|c: char| !(c.is_ascii_digit() || c == '.'))
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse::<f64>().ok())
            .collect();
        if numbers.len() >= 2 {
            return Some(ParsedThreshold {
                direction: "between".into(),
                threshold: (numbers[0] + numbers[1]) / 2.0,
                range_low: Some(numbers[0]),
                range_high: Some(numbers[1]),
            });
        }
    }
    if let Some(idx) = q.find(" be ") {
        let rest = &q[idx + 4..];
        let cleaned = rest.split_whitespace().next()?.replace('c', "").replace('f', "");
        if let Ok(val) = cleaned.parse::<f64>() {
            return Some(ParsedThreshold {
                direction: "between".into(),
                threshold: val,
                range_low: Some(val - 0.5),
                range_high: Some(val + 0.5),
            });
        }
    }
    None
}

fn last_number(s: &str) -> Option<f64> {
    let mut cur = String::new();
    let mut found = None;
    for ch in s.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            cur.push(ch);
        } else if !cur.is_empty() {
            found = cur.parse::<f64>().ok();
            cur.clear();
        }
    }
    if !cur.is_empty() {
        found = cur.parse::<f64>().ok();
    }
    found
}

fn extract_city(question: &str) -> Option<String> {
    let lower = question.to_lowercase();
    let in_pos = lower.find(" in ")?;
    let after_in = &question[in_pos + 4..];
    if let Some(be_pos) = after_in.to_lowercase().find(" be ") {
        return Some(after_in[..be_pos].trim().to_string());
    }
    if let Some(on_pos) = after_in.to_lowercase().find(" on ") {
        return Some(after_in[..on_pos].trim().to_string());
    }
    Some(after_in.trim().to_string())
}

fn extract_target_date(question: &str) -> Option<String> {
    let month_names: HashMap<&str, u32> = HashMap::from([
        ("january", 1), ("february", 2), ("march", 3), ("april", 4), ("may", 5), ("june", 6),
        ("july", 7), ("august", 8), ("september", 9), ("october", 10), ("november", 11), ("december", 12),
    ]);
    let lower = question.to_lowercase();
    let on_pos = lower.find(" on ")?;
    let words: Vec<&str> = lower[on_pos + 4..].split_whitespace().collect();
    if words.len() < 2 {
        return None;
    }
    let month = *month_names.get(words[0])?;
    let day = words[1].trim_matches(|c: char| !c.is_ascii_digit()).parse::<u32>().ok()?;
    let year = words.get(2)
        .and_then(|w| w.trim_matches(|c: char| !c.is_ascii_digit()).parse::<i32>().ok())
        .unwrap_or_else(|| Utc::now().year());
    chrono::NaiveDate::from_ymd_opt(year, month, day).map(|d| d.format("%Y-%m-%d").to_string())
}

fn parse_json_string_or_array_str(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::String(s)) => serde_json::from_str(s).unwrap_or_default(),
        Some(Value::Array(arr)) => arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect(),
        _ => vec![],
    }
}

fn parse_json_string_or_array_f64(v: Option<&Value>) -> Vec<f64> {
    match v {
        Some(Value::String(s)) => serde_json::from_str::<Vec<String>>(s)
            .unwrap_or_default()
            .iter()
            .filter_map(|x| x.parse::<f64>().ok())
            .collect(),
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|x| x.as_str().and_then(|s| s.parse::<f64>().ok()).or_else(|| x.as_f64()))
            .collect(),
        _ => vec![],
    }
}

async fn fetch_forecast(
    client: &reqwest::Client,
    lat: f64,
    lon: f64,
    city_key: &str,
    target_date: &str,
) -> Option<EnsembleForecast> {
    // Check cache first
    let cache_key = format!("{}|{}", city_key, target_date);
    {
        let cache = get_ensemble_cache().await;
        if let Some(cached) = cache.as_ref().and_then(|c| c.get(&cache_key)) {
            info!("ensemble_cache: HIT for {}", cache_key);
            return Some(cached.clone());
        }
        // If rate-limited, skip the ensemble API and go straight to fallback
        if cache.as_ref().map(|c| c.is_rate_limited()).unwrap_or(false) {
            info!("ensemble_cache: rate-limited, using fallback for {}", cache_key);
            return fetch_forecast_fallback(client, lat, lon, city_key, target_date).await;
        }
    }

    let member_ids: String = (1..=30).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    let url = format!(
        "https://ensemble-api.open-meteo.com/v1/ensemble?latitude={lat}&longitude={lon}&hourly=temperature_2m&ensemble_member_ids={member_ids}&forecast_days=3&timezone=auto",
        lat = lat,
        lon = lon,
        member_ids = member_ids,
    );

    let resp_raw = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            warn!("ensemble_cache: request error for {}: {}", cache_key, e);
            return None;
        }
    };
    let resp: Value = match resp_raw.json().await {
        Ok(v) => v,
        Err(e) => {
            warn!("ensemble_cache: parse error for {}: {}", cache_key, e);
            return None;
        }
    };

    // Check for rate-limit error response
    if resp.get("error").and_then(|e| e.as_bool()).unwrap_or(false) {
        let reason = resp.get("reason").and_then(|r| r.as_str()).unwrap_or("unknown");
        warn!("ensemble_cache: API error for {}: {}", cache_key, reason);
        // Set rate-limit backoff for 60 minutes
        {
            let mut cache = get_ensemble_cache().await;
            if let Some(c) = cache.as_mut() {
                c.set_rate_limited(60 * 60);
            }
        }
        // Fallback: try the regular (non-ensemble) forecast API
        info!("ensemble_cache: falling back to regular forecast API for {}", cache_key);
        return fetch_forecast_fallback(client, lat, lon, city_key, target_date).await;
    }
    let hourly = resp.get("hourly")?.as_object()?;
    let times = hourly.get("time")?.as_array()?;

    let mut members: Vec<(&String, &Value)> = hourly
        .iter()
        .filter(|(k, _)| k.starts_with("temperature_2m_member"))
        .collect();
    members.sort_by(|a, b| a.0.cmp(b.0));

    let mut highs = Vec::new();
    let mut lows = Vec::new();

    for (_, series) in members {
        let values = series.as_array()?;
        let mut day_values = Vec::new();
        for (idx, time_val) in times.iter().enumerate() {
            if time_val.as_str()?.starts_with(target_date) {
                if let Some(temp_c) = values.get(idx).and_then(|v| v.as_f64()) {
                    day_values.push(c_to_f(temp_c));
                }
            }
        }
        if !day_values.is_empty() {
            let max = day_values.iter().cloned().fold(f64::MIN, f64::max);
            let min = day_values.iter().cloned().fold(f64::MAX, f64::min);
            highs.push(max);
            lows.push(min);
        }
    }

    if highs.is_empty() || lows.is_empty() {
        warn!("No ensemble members found for {} {}", city_key, target_date);
        return None;
    }

    let num_members = highs.len().min(lows.len());
    highs.truncate(num_members);
    lows.truncate(num_members);

    let forecast = EnsembleForecast {
        city_key: city_key.to_string(),
        target_date: target_date.to_string(),
        member_highs: highs,
        member_lows: lows,
        num_members,
    };

    // Store in cache
    {
        let mut cache = get_ensemble_cache().await;
        if let Some(c) = cache.as_mut() {
            let evicted = c.evict_expired();
            if evicted > 0 {
                info!("ensemble_cache: evicted {} expired entries", evicted);
            }
            c.insert(cache_key, forecast.clone());
        }
    }

    Some(forecast)
}

async fn fetch_nws_temperature(
    client: &reqwest::Client,
    lat: f64,
    lon: f64,
    target_date: &str,
    metric: &str,
) -> Option<f64> {
    let points_url = format!("https://api.weather.gov/points/{lat},{lon}");
    let points: Value = client
        .get(points_url)
        .header("User-Agent", "wenbot-rust/0.1.0")
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    let forecast_url = points.get("properties")?.get("forecast")?.as_str()?;
    let forecast: Value = client
        .get(forecast_url)
        .header("User-Agent", "wenbot-rust/0.1.0")
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    let periods = forecast.get("properties")?.get("periods")?.as_array()?;

    let mut day_temps = Vec::new();
    for period in periods {
        let start = period.get("startTime")?.as_str()?;
        if start.starts_with(target_date) {
            let temp = period.get("temperature")?.as_f64()?;
            day_temps.push(temp);
        }
    }
    if day_temps.is_empty() {
        return None;
    }
    if metric == "low" {
        Some(day_temps.iter().cloned().fold(f64::MAX, f64::min))
    } else {
        Some(day_temps.iter().cloned().fold(f64::MIN, f64::max))
    }
}

fn city_coords(city: &str) -> Option<(f64, f64)> {
    match city.to_lowercase().as_str() {
        "beijing" => Some((39.9042, 116.4074)),
        "new york" => Some((40.7128, -74.0060)),
        "chicago" => Some((41.8781, -87.6298)),
        "miami" => Some((25.7617, -80.1918)),
        "los angeles" => Some((34.0522, -118.2437)),
        "austin" => Some((30.2672, -97.7431)),
        "denver" => Some((39.7392, -104.9903)),
        "seattle" => Some((47.6062, -122.3321)),
        "atlanta" => Some((33.7490, -84.3880)),
        "dallas" => Some((32.7767, -96.7970)),
        "houston" => Some((29.7604, -95.3698)),
        "phoenix" => Some((33.4484, -112.0740)),
        _ => None,
    }
}

fn c_to_f(c: f64) -> f64 {
    c * 9.0 / 5.0 + 32.0
}

/// Fallback: use the regular (non-ensemble) Open-Meteo forecast API when ensemble is rate-limited.
/// This provides a single deterministic forecast line, which we expand into a pseudo-ensemble
/// by adding small noise to simulate uncertainty. The result is cached normally.
async fn fetch_forecast_fallback(
    client: &reqwest::Client,
    lat: f64,
    lon: f64,
    city_key: &str,
    target_date: &str,
) -> Option<EnsembleForecast> {
    let cache_key = format!("{}|{}", city_key, target_date);
    // Check cache again (might have been filled by a concurrent request)
    {
        let cache = get_ensemble_cache().await;
        if let Some(cached) = cache.as_ref().and_then(|c| c.get(&cache_key)) {
            info!("ensemble_cache: HIT (fallback check) for {}", cache_key);
            return Some(cached.clone());
        }
    }

    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}&hourly=temperature_2m&forecast_days=3&timezone=auto",
        lat = lat,
        lon = lon,
    );

    let resp: Value = client.get(&url).send().await.ok()?.json().await.ok()?;
    let hourly = resp.get("hourly")?.as_object()?;
    let times = hourly.get("time")?.as_array()?;
    let temps = hourly.get("temperature_2m")?.as_array()?;

    // Extract day temperatures in Fahrenheit
    let mut day_temps_f = Vec::new();
    for (idx, time_val) in times.iter().enumerate() {
        if time_val.as_str()?.starts_with(target_date) {
            if let Some(temp_c) = temps.get(idx).and_then(|v| v.as_f64()) {
                day_temps_f.push(c_to_f(temp_c));
            }
        }
    }

    if day_temps_f.is_empty() {
        warn!("fallback: no temperature data for {} {}", city_key, target_date);
        return None;
    }

    let det_high = day_temps_f.iter().cloned().fold(f64::MIN, f64::max);
    let det_low = day_temps_f.iter().cloned().fold(f64::MAX, f64::min);

    // Create a pseudo-ensemble: 10 members with ±3°F noise around the deterministic forecast.
    // This gives the strategy enough variance to compute meaningful probabilities.
    let pseudo_members = 10;
    let mut member_highs = Vec::with_capacity(pseudo_members);
    let mut member_lows = Vec::with_capacity(pseudo_members);

    // Use a simple deterministic seed based on city+date for reproducibility
    let seed: u64 = city_key.bytes().chain(target_date.bytes()).fold(0u64, |a, b| a.wrapping_mul(31).wrapping_add(b as u64));

    for i in 0..pseudo_members {
        // Deterministic noise: ±3°F spread, using a simple LCG
        let mut rng = seed.wrapping_add(i as u64 * 2654435761);
        let noise_high = (((rng >> 16) & 0xFFFF) as f64 / 65535.0 - 0.5) * 6.0; // ±3°F
        rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
        let noise_low = (((rng >> 16) & 0xFFFF) as f64 / 65535.0 - 0.5) * 6.0;
        member_highs.push((det_high + noise_high).max(det_low + 1.0)); // ensure high > low
        member_lows.push((det_low + noise_low).min(det_high - 1.0).max(0.0));
    }

    let forecast = EnsembleForecast {
        city_key: city_key.to_string(),
        target_date: target_date.to_string(),
        member_highs,
        member_lows,
        num_members: pseudo_members,
    };

    info!(
        "fallback: created pseudo-ensemble for {} {} (det_high={:.1}°F, det_low={:.1}°F, {} members)",
        city_key, target_date, det_high, det_low, pseudo_members
    );

    // Cache the fallback result too (same TTL)
    {
        let mut cache = get_ensemble_cache().await;
        if let Some(c) = cache.as_mut() {
            c.insert(cache_key, forecast.clone());
        }
    }

    Some(forecast)
}

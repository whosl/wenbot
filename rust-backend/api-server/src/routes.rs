//! HTTP route handlers

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::Instant;

use crate::fivesbot_state::FivesbotPredictorState;
use crate::SharedState;

/// Get the latest price for a token, preferring the WS live feed over DB snapshots.
async fn get_token_price_with_cache(
    state: &SharedState,
    wallet: &virtual_wallet::VirtualWallet,
    market_id: &str,
    token_id: &str,
) -> Option<f64> {
    // 1. Check WS live feed (sub-second fresh)
    {
        let live = state.fivesbot_state.lock().await;
        if let Some(&price) = live.token_live_prices.get(token_id) {
            return Some(price);
        }
    }
    // 2. Fallback to latest DB snapshot (from 60s HTTP sync)
    wallet.get_latest_price(market_id, token_id).await.ok().flatten()
}

/// Response cache for weather API routes (TTL 5 min) to avoid slow Polymarket/Open-Meteo calls per request.
struct ResponseCache {
    signals: Option<(serde_json::Value, Instant)>,
    forecasts: Option<(serde_json::Value, Instant)>,
}

static RESPONSE_CACHE: Mutex<Option<ResponseCache>> = Mutex::new(None);

fn get_response_cache() -> std::sync::MutexGuard<'static, Option<ResponseCache>> {
    RESPONSE_CACHE.lock().unwrap()
}

const CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(300); // 5 minutes

// ─── Health ───

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        backend: "rust".to_string(),
    })
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub backend: String,
}

// ─── Bot Status ───

pub async fn bot_config() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "btc_strategy": {
            "min_edge_threshold": 0.08,
            "max_entry_price": 0.45,
            "min_market_volume": 1000,
            "min_trade_size": 1.0,
            "max_trade_size": 10.0,
            "kelly_max_fraction": 0.05,
            "daily_loss_limit": 1000.0,
            "min_time_remaining": 300,
            "max_time_remaining": 7200,
            "fee_rate": "polymarket_curve",
            "weights": {
                "rsi": 0.25,
                "momentum": 0.25,
                "vwap": 0.20,
                "sma": 0.15,
                "market_skew": 0.15
            }
        },
        "wallet3_btc8_strategy": {
            "min_edge_threshold": 0.06,
            "max_entry_price": 0.55,
            "max_trade_size": 75.0,
            "daily_loss_limit": 1000.0,
            "weights": {
                "rsi": 0.15,
                "momentum": 0.20,
                "vwap": 0.15,
                "sma": 0.10,
                "market_skew": 0.10,
                "volume_trend": 0.10,
                "bollinger": 0.10,
                "volatility": 0.10
            }
        },
        "eth_strategy": {
            "min_edge_threshold": 0.08,
            "max_entry_price": 0.45,
            "min_market_volume": 1000,
            "min_trade_size": 1.0,
            "max_trade_size": 10.0,
            "kelly_max_fraction": 0.05,
            "daily_loss_limit": 1000.0,
            "min_time_remaining": 300,
            "max_time_remaining": 7200,
            "fee_rate": "polymarket_curve",
            "weights": {
                "rsi": 0.25,
                "momentum": 0.25,
                "vwap": 0.20,
                "sma": 0.15,
                "market_skew": 0.15
            }
        },
        "weather": {
            "min_edge_threshold": 0.08,
            "max_entry_price": 0.45,
            "max_trade_size": 5.0,
            "fee_rate": "polymarket_curve",
            "kelly_max_fraction": 0.05,
            "kelly_fraction": 0.25,
            "kelly_lookback_trades": 20,
            "daily_loss_limit": 20.0,
            "total_exposure_cap_ratio": 2.0,
            "low_balance_threshold": 10.0,
            "scan_interval_secs": 300,
            "price_update_interval_secs": 180,
            "settlement_interval_secs": 3600,
            "dynamic_kelly": true,
            "dedup_enabled": true,
            "fee_param_fallback": 0.25,
            "weather_cities": [
                "New York", "Chicago", "Miami", "Los Angeles",
                "Austin", "Denver", "Seattle", "Atlanta",
                "Dallas", "Houston", "Phoenix", "Beijing"
            ]
        }
    }))
}

pub async fn bot_status(State(_state): State<SharedState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "scheduler_running": true,
        "mode": "simulation",
        "recent_events": []
    }))
}

fn predictor_json(p: &FivesbotPredictorState) -> serde_json::Value {
    serde_json::json!({
        "signal": p.signal,
        "confidence": p.confidence,
        "direction": p.direction,
        "trend": p.trend,
        "momentum": p.momentum,
        "volatility": p.volatility,
        "rsi": p.rsi,
    })
}

async fn wallet_balance_json(
    state: &SharedState,
    wallet: &virtual_wallet::VirtualWallet,
) -> Result<serde_json::Value, String> {
    let mut b = wallet.get_balance().await.map_err(|e| e.to_string())?;
    if let Ok(positions) = wallet.list_positions_raw().await {
        let mut total_position_value = 0.0;
        for pos in &positions {
            let latest = get_token_price_with_cache(state, wallet, &pos.market_id, &pos.token_id).await;
            total_position_value += virtual_wallet::mark_to_market(pos, latest);
        }
        b.total_position_value = (total_position_value * 100.0).round() / 100.0;
        b.total_value = ((b.balance + b.total_position_value) * 100.0).round() / 100.0;
    }
    Ok(serde_json::to_value(b).unwrap_or_default())
}

async fn wallet_positions_json(
    state: &SharedState,
    wallet: &virtual_wallet::VirtualWallet,
    include_ledger: bool,
) -> Result<serde_json::Value, String> {
    let positions = wallet.list_positions().await.map_err(|e| e.to_string())?;
    let ledger_entries = if include_ledger { wallet.get_btc_trade_ledger(500).await.unwrap_or_default() } else { Vec::new() };
    let mut enriched = Vec::new();
    for pos in positions {
        let latest = get_token_price_with_cache(state, wallet, &pos.market_id, &pos.token_id).await;
        let current_value = virtual_wallet::mark_to_market_from_summary(&pos, latest);
        let pnl = virtual_wallet::unrealized_pnl_from_summary(&pos, current_value);
        let ledger = ledger_entries.iter().find(|l| l.position_id == pos.id);
        enriched.push(serde_json::json!({
            "id": pos.id,
            "market_id": pos.market_id,
            "market_question": pos.market_question,
            "token_id": pos.token_id,
            "direction": pos.direction,
            "entry_price": pos.entry_price,
            "size": pos.size,
            "quantity": pos.quantity,
            "fee": pos.fee,
            "slippage": pos.slippage,
            "current_price": latest,
            "current_value": (current_value * 100.0).round() / 100.0,
            "unrealized_pnl": (pnl * 100.0).round() / 100.0,
            "edge": ledger.as_ref().map(|l| l.edge),
            "confidence": ledger.as_ref().map(|l| l.confidence),
            "indicator_details": ledger.as_ref().and_then(|l| l.indicator_details.clone()),
            "category": pos.category,
            "status": pos.status,
            "created_at": pos.created_at,
            "city_name": pos.city_name,
            "metric": pos.metric,
            "target_date": pos.target_date,
            "threshold_f": pos.threshold_f,
            "event_slug": pos.event_slug,
            "window_end": pos.window_end,
        }));
    }
    Ok(serde_json::to_value(enriched).unwrap_or_default())
}

pub async fn fivesbot_status(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let live = state.fivesbot_state.lock().await.clone();
    Json(serde_json::json!({
        "wsConnected": live.ws_connected,
        "uptime": live.uptime(),
        "upPrice": live.btc_up_price,
        "downPrice": live.btc_down_price,
        "btcPrice": live.btc_price,
        "currentCycle": live.btc_current_cycle,
        "lastUpdate": live.btc_last_update.map(|v| v.to_rfc3339()),
        "markets": {
            "btc": {
                "predictor": live.btc_predictor.as_ref().map(predictor_json),
                "activeMarkets": live.btc_active_markets,
            }
        }
    }))
}

pub async fn eth_fivesbot_status(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let live = state.fivesbot_state.lock().await.clone();
    Json(serde_json::json!({
        "wsConnected": live.ws_connected,
        "uptime": live.uptime(),
        "upPrice": live.eth_up_price,
        "downPrice": live.eth_down_price,
        "ethPrice": live.eth_price,
        "currentCycle": live.eth_current_cycle,
        "lastUpdate": live.eth_last_update.map(|v| v.to_rfc3339()),
        "markets": {
            "eth": {
                "predictor": live.eth_predictor.as_ref().map(predictor_json),
                "activeMarkets": live.eth_active_markets,
            }
        }
    }))
}


pub async fn wallet3_fivesbot_status(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let live = state.fivesbot_state.lock().await.clone();
    Json(serde_json::json!({
        "wsConnected": live.ws_connected,
        "uptime": live.uptime(),
        "upPrice": live.wallet3_up_price,
        "downPrice": live.wallet3_down_price,
        "btcPrice": live.wallet3_price,
        "currentCycle": live.wallet3_current_cycle,
        "lastUpdate": live.wallet3_last_update.map(|v| v.to_rfc3339()),
        "markets": {
            "btc": {
                "predictor": live.wallet3_predictor.as_ref().map(predictor_json),
                "activeMarkets": live.wallet3_active_markets,
            }
        }
    }))
}

pub async fn wallet3_fivesbot_signals(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    let live = state.fivesbot_state.lock().await.clone();
    match serde_json::to_value(live.wallet3_recent_signals) {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn wallet3_fivesbot_trades(State(state): State<SharedState>, Query(q): Query<HistoryQuery>) -> (StatusCode, Json<serde_json::Value>) {
    let limit = q.limit.unwrap_or(50);
    match state.wallet3.get_btc_trade_ledger(limit).await {
        Ok(trades) => (StatusCode::OK, Json(serde_json::to_value(trades).unwrap_or_default())),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn wallet4_fivesbot_status(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let live = state.fivesbot_state.lock().await.clone();
    Json(serde_json::json!({
        "wsConnected": live.ws_connected,
        "uptime": live.uptime(),
        "upPrice": live.wallet4_up_price,
        "downPrice": live.wallet4_down_price,
        "btcPrice": live.wallet4_price,
        "currentCycle": live.wallet4_current_cycle,
        "lastUpdate": live.wallet4_last_update.map(|v| v.to_rfc3339()),
        "markets": {
            "btc": {
                "predictor": live.wallet4_predictor.as_ref().map(predictor_json),
                "activeMarkets": live.wallet4_active_markets,
            }
        }
    }))
}

pub async fn wallet4_fivesbot_signals(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    let live = state.fivesbot_state.lock().await.clone();
    match serde_json::to_value(live.wallet4_recent_signals) {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn wallet4_fivesbot_trades(State(state): State<SharedState>, Query(q): Query<HistoryQuery>) -> (StatusCode, Json<serde_json::Value>) {
    let limit = q.limit.unwrap_or(50);
    match state.wallet4.get_btc_trade_ledger(limit).await {
        Ok(trades) => (StatusCode::OK, Json(serde_json::to_value(trades).unwrap_or_default())),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn fivesbot_signals(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    let live = state.fivesbot_state.lock().await.clone();
    match serde_json::to_value(live.btc_recent_signals) {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn eth_fivesbot_signals(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    let live = state.fivesbot_state.lock().await.clone();
    match serde_json::to_value(live.eth_recent_signals) {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

// ─── BTC/ETH Trade Ledger ───

pub async fn fivesbot_trades(State(state): State<SharedState>, Query(q): Query<HistoryQuery>) -> (StatusCode, Json<serde_json::Value>) {
    let limit = q.limit.unwrap_or(50);
    match state.wallet1.get_btc_trade_ledger(limit).await {
        Ok(trades) => (StatusCode::OK, Json(serde_json::to_value(trades).unwrap_or_default())),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn eth_fivesbot_trades(State(state): State<SharedState>, Query(q): Query<HistoryQuery>) -> (StatusCode, Json<serde_json::Value>) {
    let limit = q.limit.unwrap_or(50);
    match state.wallet2.get_btc_trade_ledger(limit).await {
        Ok(trades) => (StatusCode::OK, Json(serde_json::to_value(trades).unwrap_or_default())),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

// ─── Wallet 1 (fivesbot BTC) / Wallet 2 (fivesbot ETH) ───

pub async fn wallet_balance(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    match wallet_balance_json(&state, &state.wallet1).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e}))),
    }
}

pub async fn wallet_positions(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    match wallet_positions_json(&state, &state.wallet1, true).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e}))),
    }
}

pub async fn wallet2_balance(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    match wallet_balance_json(&state, &state.wallet2).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e}))),
    }
}

pub async fn wallet2_positions(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    match wallet_positions_json(&state, &state.wallet2, true).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e}))),
    }
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub limit: Option<i64>,
}

pub async fn wallet_history(State(state): State<SharedState>, Query(q): Query<HistoryQuery>) -> (StatusCode, Json<serde_json::Value>) {
    let limit = q.limit.unwrap_or(100);
    match state.wallet1.get_history(limit).await {
        Ok(h) => (StatusCode::OK, Json(serde_json::to_value(h).unwrap_or_default())),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

#[derive(Deserialize)]
pub struct DepositQuery {
    pub amount: f64,
}

pub async fn wallet_deposit(State(state): State<SharedState>, Query(q): Query<DepositQuery>) -> (StatusCode, Json<serde_json::Value>) {
    match state.wallet1.deposit(q.amount).await {
        Ok(b) => (StatusCode::OK, Json(serde_json::to_value(b).unwrap_or_default())),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn wallet_sync_prices(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    match crate::scheduler::sync_wallet1_prices(&state).await {
        Ok(updated) => (StatusCode::OK, Json(serde_json::json!({"updated": updated}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn wallet_settle(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    match crate::scheduler::settle_wallet1_positions(&state).await {
        Ok(settled) => (StatusCode::OK, Json(serde_json::json!({"settled": settled}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn wallet2_history(State(state): State<SharedState>, Query(q): Query<HistoryQuery>) -> (StatusCode, Json<serde_json::Value>) {
    let limit = q.limit.unwrap_or(100);
    match state.wallet2.get_history(limit).await {
        Ok(h) => (StatusCode::OK, Json(serde_json::to_value(h).unwrap_or_default())),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn wallet2_deposit(State(state): State<SharedState>, Query(q): Query<DepositQuery>) -> (StatusCode, Json<serde_json::Value>) {
    match state.wallet2.deposit(q.amount).await {
        Ok(b) => (StatusCode::OK, Json(serde_json::to_value(b).unwrap_or_default())),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn wallet2_sync_prices(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    match crate::scheduler::sync_wallet2_prices(&state).await {
        Ok(updated) => (StatusCode::OK, Json(serde_json::json!({"updated": updated}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn wallet2_settle(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    match crate::scheduler::settle_wallet2_positions(&state).await {
        Ok(settled) => (StatusCode::OK, Json(serde_json::json!({"settled": settled}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

// ─── Wallet 3 (BTC 8-indicator) ───

pub async fn wallet3_balance(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    match wallet_balance_json(&state, &state.wallet3).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e}))),
    }
}

pub async fn wallet3_positions(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    match wallet_positions_json(&state, &state.wallet3, true).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e}))),
    }
}

pub async fn wallet3_history(State(state): State<SharedState>, Query(q): Query<HistoryQuery>) -> (StatusCode, Json<serde_json::Value>) {
    let limit = q.limit.unwrap_or(100);
    match state.wallet3.get_history(limit).await {
        Ok(h) => (StatusCode::OK, Json(serde_json::to_value(h).unwrap_or_default())),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn wallet3_deposit(State(state): State<SharedState>, Query(q): Query<DepositQuery>) -> (StatusCode, Json<serde_json::Value>) {
    match state.wallet3.deposit(q.amount).await {
        Ok(b) => (StatusCode::OK, Json(serde_json::to_value(b).unwrap_or_default())),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn wallet3_sync_prices(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    match crate::scheduler::sync_wallet3_prices(&state).await {
        Ok(updated) => (StatusCode::OK, Json(serde_json::json!({"updated": updated}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn wallet3_settle(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    match crate::scheduler::settle_wallet3_positions(&state).await {
        Ok(settled) => (StatusCode::OK, Json(serde_json::json!({"settled": settled}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

// ─── Wallet 4 (BTC 5min classic_four) ───

pub async fn wallet4_balance(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    match wallet_balance_json(&state, &state.wallet4).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e}))),
    }
}

pub async fn wallet4_positions(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    match wallet_positions_json(&state, &state.wallet4, true).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e}))),
    }
}

pub async fn wallet4_history(State(state): State<SharedState>, Query(q): Query<HistoryQuery>) -> (StatusCode, Json<serde_json::Value>) {
    let limit = q.limit.unwrap_or(100);
    match state.wallet4.get_history(limit).await {
        Ok(h) => (StatusCode::OK, Json(serde_json::to_value(h).unwrap_or_default())),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn wallet4_deposit(State(state): State<SharedState>, Query(q): Query<DepositQuery>) -> (StatusCode, Json<serde_json::Value>) {
    match state.wallet4.deposit(q.amount).await {
        Ok(b) => (StatusCode::OK, Json(serde_json::to_value(b).unwrap_or_default())),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn wallet4_sync_prices(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    match crate::scheduler::sync_wallet4_prices(&state).await {
        Ok(updated) => (StatusCode::OK, Json(serde_json::json!({"updated": updated}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn wallet4_settle(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    match crate::scheduler::settle_wallet4_positions(&state).await {
        Ok(settled) => (StatusCode::OK, Json(serde_json::json!({"settled": settled}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

// ─── Weather (wallet 5) ───

pub async fn weather_wallet_balance(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    match wallet_balance_json(&state, &state.wallet5).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e}))),
    }
}

pub async fn weather_positions(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    match wallet_positions_json(&state, &state.wallet5, false).await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e}))),
    }
}

pub async fn weather_history(State(state): State<SharedState>, Query(q): Query<HistoryQuery>) -> (StatusCode, Json<serde_json::Value>) {
    let limit = q.limit.unwrap_or(100);
    match state.wallet5.get_history(limit).await {
        Ok(h) => (StatusCode::OK, Json(serde_json::to_value(h).unwrap_or_default())),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn weather_deposit(State(state): State<SharedState>, Query(q): Query<DepositQuery>) -> (StatusCode, Json<serde_json::Value>) {
    match state.wallet5.deposit(q.amount).await {
        Ok(b) => (StatusCode::OK, Json(serde_json::to_value(b).unwrap_or_default())),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn weather_signals(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    {
        let cache = get_response_cache();
        if let Some(ref c) = *cache {
            if let Some((ref val, ts)) = c.signals {
                if ts.elapsed() < CACHE_TTL {
                    return (StatusCode::OK, Json(val.clone()));
                }
            }
        }
    }

    let result = match crate::weather_markets::fetch_daily_temperature_markets().await {
        Ok(bundles) => {
            let mut signals = Vec::new();
            for bundle in bundles {
                if let Ok(sig) = state.weather_strategy.generate_signal(&bundle.market, &bundle.forecast, None, 100.0) {
                    signals.push(sig);
                }
            }
            let json_val = serde_json::to_value(&signals).unwrap_or_default();
            (StatusCode::OK, Json(json_val.clone()))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    };

    if let (StatusCode::OK, Json(ref val)) = result {
        let mut cache = get_response_cache();
        if cache.is_none() {
            *cache = Some(ResponseCache {
                signals: None,
                forecasts: None,
            });
        }
        if let Some(ref mut c) = *cache {
            c.signals = Some((val.clone(), Instant::now()));
        }
    }

    result
}

pub async fn weather_forecasts(State(_state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    {
        let cache = get_response_cache();
        if let Some(ref c) = *cache {
            if let Some((ref val, ts)) = c.forecasts {
                if ts.elapsed() < CACHE_TTL {
                    return (StatusCode::OK, Json(val.clone()));
                }
            }
        }
    }

    let result = match crate::weather_markets::fetch_daily_temperature_markets().await {
        Ok(bundles) => {
            let forecasts: Vec<serde_json::Value> = bundles.into_iter().map(|b| {
                serde_json::json!({
                    "city_key": b.market.city_key,
                    "city_name": b.market.city_name,
                    "target_date": b.market.target_date,
                    "market_question": b.market.question,
                    "market_slug": b.market.slug,
                    "range_low": b.market.range_low,
                    "range_high": b.market.range_high,
                    "yes_price": b.market.yes_price,
                    "no_price": b.market.no_price,
                    "mean_high": b.forecast.mean_high(),
                    "std_high": b.forecast.std_high(),
                    "mean_low": b.forecast.mean_low(),
                    "std_low": b.forecast.std_low(),
                    "num_members": b.forecast.num_members,
                    "ensemble_agreement": b.forecast.agreement(),
                })
            }).collect();
            let json_val = serde_json::json!(forecasts);
            (StatusCode::OK, Json(json_val.clone()))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    };

    if let (StatusCode::OK, Json(ref val)) = result {
        let mut cache = get_response_cache();
        if cache.is_none() {
            *cache = Some(ResponseCache {
                signals: None,
                forecasts: None,
            });
        }
        if let Some(ref mut c) = *cache {
            c.forecasts = Some((val.clone(), Instant::now()));
        }
    }

    result
}

#[allow(dead_code)]
fn _parse_timestamp(ts: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(ts).ok().map(|dt| dt.with_timezone(&Utc))
}

//! Polymarket CLOB WebSocket manager for fivesbot real-time price subscriptions.
//!
//! Connects to `wss://ws-subscriptions-clob.polymarket.com/ws/market`, subscribes
//! to the current active BTC up/down 15-minute window's token IDs, and feeds
//! real-time `best_bid` / `best_ask` prices into `FivesbotLiveState`.
//!
//! **Subscription protocol** (confirmed against live CLOB):
//! - Subscribe:   `{"assets_ids": ["token_1", "token_2"]}`
//! - Unsubscribe: `{"assets_ids": []}`  (replacing with new list also works)
//!
//! **Incoming messages**:
//! - Initial snapshot: JSON array of `{ market, asset_id, bids, asks, … }`
//! - `price_changes`: `{ market, price_changes: [{ asset_id, price, best_bid, best_ask, side, … }] }`
//! - `last_trade_price`: `{ asset_id, price, event_type: "last_trade_price", … }`

use std::sync::Arc;

use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use tokio_tungstenite::{
    tungstenite::Message,
    MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, info, warn};

use crate::scheduler::fetch_active_updown_markets;
use crate::AppState;

/// Polymarket CLOB websocket endpoint for market-level orderbook & price subscriptions.
const WS_URL: &str = "wss://ws-subscriptions-clob.polymarket.com/ws/market";
const WS_HOST: &str = "ws-subscriptions-clob.polymarket.com:443";

/// Local Clash proxy for outbound connections.
const PROXY_ADDR: &str = "127.0.0.1:7890";

/// How often the market-window change detector re-evaluates.
const MARKET_CHECK_INTERVAL_SECS: u64 = 10;

/// Reconnect backoff range.
const RECONNECT_BASE_SECS: u64 = 2;
const RECONNECT_MAX_SECS: u64 = 60;

// ─── Wire types ───

#[derive(Debug, Deserialize)]
struct PriceChange {
    asset_id: String,
    #[serde(default)]
    best_bid: Option<String>,
    #[serde(default)]
    best_ask: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PriceChangesMsg {
    price_changes: Vec<PriceChange>,
}

#[derive(Debug, Deserialize)]
struct LastTradeMsg {
    asset_id: String,
    price: String,
    event_type: String,
}

// ─── Public entry point ───

/// Start the fivesbot websocket background task (never returns).
pub async fn start_fivesbot_ws(state: Arc<AppState>) {
    info!("🔌 Fivesbot WebSocket manager starting...");
    let mut reconnect_delay = Duration::from_secs(RECONNECT_BASE_SECS);

    loop {
        match run_ws_session(state.clone()).await {
            Ok(()) => {
                reconnect_delay = Duration::from_secs(RECONNECT_BASE_SECS);
            }
            Err(e) => {
                let mut live = state.fivesbot_state.lock().await;
                live.ws_connected = false;
                drop(live);
                warn!("🔌 WS error: {}, reconnecting in {:?}", e, reconnect_delay);
            }
        }
        tokio::time::sleep(reconnect_delay).await;
        reconnect_delay = (reconnect_delay * 2).min(Duration::from_secs(RECONNECT_MAX_SECS));
    }
}

// ─── Session ───

/// Establish a TCP connection through the local HTTP CONNECT proxy.
async fn proxy_tcp_connect(proxy: &str, target: &str) -> Result<TcpStream, Box<dyn std::error::Error + Send + Sync>> {
    let mut stream = TcpStream::connect(proxy).await?;
    let connect_req = format!(
        "CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n"
    );
    stream.write_all(connect_req.as_bytes()).await?;
    let mut buf = [0u8; 512];
    let n = tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await?;
    let response = String::from_utf8_lossy(&buf[..n]);
    if !response.starts_with("HTTP/1.1 200") && !response.starts_with("HTTP/1.0 200") {
        return Err(format!("Proxy CONNECT failed: {}", response.trim()).into());
    }
    Ok(stream)
}

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// One WS session: connect → resolve tokens → subscribe → read.
/// Returns Ok when the market window changed (need reconnect with new state).
async fn run_ws_session(state: Arc<AppState>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let tcp = proxy_tcp_connect(PROXY_ADDR, WS_HOST).await?;
    let ws_stream = tokio_tungstenite::client_async_tls_with_config(
        WS_URL,
        tcp,
        None,
        None,
    )
    .await?
    .0;
    info!("🔌 Connected to {} (via proxy)", WS_URL);
    let (mut write, mut read) = ws_stream.split();

    // Resolve tokens and subscribe
    let sub = resolve_and_subscribe(&state, &mut write).await?;
    if sub.tokens.is_empty() {
        warn!("🔌 No active market yet, retrying shortly");
        return Ok(());
    }

    {
        let mut live = state.fivesbot_state.lock().await;
        live.ws_connected = true;
    }

    // Channel for the market-change detector to signal us
    let (switch_tx, mut switch_rx) = mpsc::channel::<(String, String)>(1);

    // Spawn market-window change detector
    let check_state = state.clone();
    let prev_up = sub.up_token.clone();
    let prev_down = sub.down_token.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(MARKET_CHECK_INTERVAL_SECS));
        tick.tick().await;
        loop {
            tick.tick().await;
            let current = {
                let live = check_state.fivesbot_state.lock().await;
                let btc = live.btc_active_markets.first().map(|m| (m.up_token_id.clone(), m.down_token_id.clone()));
                let eth = live.eth_active_markets.first().map(|m| (m.up_token_id.clone(), m.down_token_id.clone()));
                Some((btc, eth))
            };
            if let Some((btc, eth)) = current {
                let changed = btc.as_ref().map(|(up, down)| Some(up) != prev_up.as_ref() || Some(down) != prev_down.as_ref()).unwrap_or(false)
                    || eth.as_ref().map(|(up, down)| Some(up) != prev_up.as_ref() || Some(down) != prev_down.as_ref()).unwrap_or(false);
                if changed {
                    let _ = switch_tx.send((String::new(), String::new())).await;
                    return;
                }
            }
        }
    });

    // Read loop — select on WS messages vs market-switch signal
    let tokens = sub.tokens;
    loop {
        tokio::select! {
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        handle_ws_message(&text, &tokens, &state).await;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = write.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Close(_))) => {
                        warn!("🔌 WS closed by server");
                        return Err("WS closed by server".into());
                    }
                    Some(Err(e)) => {
                        return Err(format!("WS read error: {}", e).into());
                    }
                    None => {
                        return Err("WS stream ended".into());
                    }
                    _ => {}
                }
            }
            _ = switch_rx.recv() => {
                // Market window changed — reconnect with fresh state
                info!("🔌 Reconnecting due to market window switch");
                break;
            }
        }
    }

    Ok(())
}

// ─── Subscribe ───

struct SubscriptionInfo {
    up_token: Option<String>,
    down_token: Option<String>,
    tokens: Vec<String>,
}

type WsSink = futures_util::stream::SplitSink<WsStream, Message>;

async fn resolve_and_subscribe(
    state: &AppState,
    write: &mut WsSink,
) -> Result<SubscriptionInfo, Box<dyn std::error::Error + Send + Sync>> {
    let mut tokens = Vec::new();
    {
        let live = state.fivesbot_state.lock().await;
        if let Some(m) = live.btc_active_markets.first() {
            tokens.push(m.up_token_id.clone());
            tokens.push(m.down_token_id.clone());
        }
        if let Some(m) = live.eth_active_markets.first() {
            tokens.push(m.up_token_id.clone());
            tokens.push(m.down_token_id.clone());
        }
    }

    if tokens.is_empty() {
        for asset in ["btc", "eth"] {
            if let Ok(markets) = fetch_active_updown_markets(asset).await {
                if let Some(m) = markets.first() {
                    tokens.push(m.up_token_id.clone());
                    tokens.push(m.down_token_id.clone());
                }
            }
        }
    }

    tokens.sort();
    tokens.dedup();

    if !tokens.is_empty() {
        send_subscribe(write, &tokens.iter().map(String::as_str).collect::<Vec<_>>()).await?;
    }

    Ok(SubscriptionInfo { up_token: tokens.first().cloned(), down_token: tokens.get(1).cloned(), tokens })
}

async fn send_subscribe(
    write: &mut WsSink,
    token_ids: &[&str],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let payload = serde_json::json!({ "assets_ids": token_ids });
    let msg = serde_json::to_string(&payload)?;
    info!(
        "🔌 Subscribing to {} token(s): {}",
        token_ids.len(),
        token_ids.iter().map(|t| &t[..12.min(t.len())]).collect::<Vec<_>>().join(", "),
    );
    write.send(Message::Text(msg.into())).await?;
    Ok(())
}

// ─── Message handling ───

async fn handle_ws_message(text: &str, subscribed: &[String], state: &AppState) {
    // Initial snapshot: JSON array of orderbook objects
    if text.starts_with('[') {
        if let Ok(snapshot) = serde_json::from_str::<Vec<serde_json::Value>>(text) {
            for item in &snapshot {
                let asset_id = match item.get("asset_id").and_then(|v| v.as_str()) {
                    Some(id) if subscribed.iter().any(|t| t == id) => id,
                    _ => continue,
                };
                let bid = extract_best_price(item, "bids");
                let ask = extract_best_price(item, "asks");
                update_price(state, asset_id, bid, ask).await;
            }
        }
        return;
    }

    // price_changes
    if let Ok(msg) = serde_json::from_str::<PriceChangesMsg>(text) {
        for change in &msg.price_changes {
            if subscribed.iter().any(|t| t == &change.asset_id) {
                let bid = change.best_bid.as_ref().and_then(|s| s.parse::<f64>().ok());
                let ask = change.best_ask.as_ref().and_then(|s| s.parse::<f64>().ok());
                update_price(state, &change.asset_id, bid, ask).await;
            }
        }
        return;
    }

    // last_trade_price
    if let Ok(msg) = serde_json::from_str::<LastTradeMsg>(text) {
        if msg.event_type == "last_trade_price"
            && subscribed.iter().any(|t| t == &msg.asset_id)
        {
            if let Ok(price) = msg.price.parse::<f64>() {
                debug!(
                    "🔌 last trade: {}..{} @ {:.4}",
                    &msg.asset_id[..12.min(msg.asset_id.len())],
                    &msg.asset_id[msg.asset_id.len().saturating_sub(4)..],
                    price,
                );
            }
        }
    }
}

fn extract_best_price(item: &serde_json::Value, key: &str) -> Option<f64> {
    item.get(key)?
        .as_array()?
        .first()?
        .get("price")?
        .as_str()?
        .parse::<f64>()
        .ok()
}

async fn update_price(state: &AppState, asset_id: &str, best_bid: Option<f64>, best_ask: Option<f64>) {
    let price = match (best_bid, best_ask) {
        (Some(b), Some(a)) => (b + a) / 2.0,
        (Some(b), None) => b,
        (None, Some(a)) => a,
        (None, None) => return,
    };

    let mut live = state.fivesbot_state.lock().await;

    let is_up = live.btc_active_markets.iter().chain(live.eth_active_markets.iter()).any(|m| m.up_token_id == asset_id);
    let is_down = live.btc_active_markets.iter().chain(live.eth_active_markets.iter()).any(|m| m.down_token_id == asset_id);

    if live.btc_active_markets.iter().any(|m| m.up_token_id == asset_id) && live.btc_up_price != Some(price) {
        debug!("🔌 BTC UP: {:.4} → {:.4}", live.btc_up_price.unwrap_or(0.0), price);
        live.btc_up_price = Some(price);
        live.btc_last_update = Some(Utc::now());
    } else if live.btc_active_markets.iter().any(|m| m.down_token_id == asset_id) && live.btc_down_price != Some(price) {
        debug!("🔌 BTC DOWN: {:.4} → {:.4}", live.btc_down_price.unwrap_or(0.0), price);
        live.btc_down_price = Some(price);
        live.btc_last_update = Some(Utc::now());
    } else if live.eth_active_markets.iter().any(|m| m.up_token_id == asset_id) && live.eth_up_price != Some(price) {
        debug!("🔌 ETH UP: {:.4} → {:.4}", live.eth_up_price.unwrap_or(0.0), price);
        live.eth_up_price = Some(price);
        live.eth_last_update = Some(Utc::now());
    } else if live.eth_active_markets.iter().any(|m| m.down_token_id == asset_id) && live.eth_down_price != Some(price) {
        debug!("🔌 ETH DOWN: {:.4} → {:.4}", live.eth_down_price.unwrap_or(0.0), price);
        live.eth_down_price = Some(price);
        live.eth_last_update = Some(Utc::now());
    }

    // Always feed token_live_prices for wallet1 real-time mark-to-market
    live.token_live_prices.insert(asset_id.to_string(), price);
}

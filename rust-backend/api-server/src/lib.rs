//! Wenbot API Server — axum-based HTTP + WebSocket server

use axum::Router;
use std::{collections::HashMap, sync::Arc};
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
    trace::TraceLayer,
};
use tracing::info;

mod crypto;
mod fivesbot_state;
mod fivesbot_ws;
mod routes;
mod scheduler;
pub mod srp_auth;
mod weather_markets;
mod nws;

pub use fivesbot_state::FivesbotLiveState;

/// Shared application state holding wallet instances
pub struct AppState {
    pub frontend_dir: String,
    pub db_pool: sqlx::SqlitePool,
    /// Virtual wallet 1 (fivesbot BTC strategy)
    pub wallet1: virtual_wallet::VirtualWallet,
    /// Virtual wallet 2 (fivesbot ETH strategy)
    pub wallet2: virtual_wallet::VirtualWallet,
    /// Virtual wallet 3 (BTC 8-indicator strategy)
    pub wallet3: virtual_wallet::VirtualWallet,
    /// Virtual wallet 4 (BTC 5min classic_four strategy)
    pub wallet4: virtual_wallet::VirtualWallet,
    /// Virtual wallet 5 (wenbot weather strategy)
    pub wallet5: virtual_wallet::VirtualWallet,
    /// Wenbot weather strategy
    pub weather_strategy: wenbot_strategy::WenbotStrategy,
    /// Fivesbot BTC/ETH 5-indicator strategy
    pub fivesbot_strategy: fivesbot_strategy::FivesbotStrategy,
    /// Fivesbot BTC 8-indicator strategy (wallet3)
    pub fivesbot_wallet3_strategy: fivesbot_strategy::FivesbotStrategy,
    /// Fivesbot BTC 5min classic_four strategy (wallet4)
    pub fivesbot_wallet4_strategy: fivesbot_strategy::FivesbotStrategy,
    /// Live Fivesbot API state
    pub fivesbot_state: Arc<tokio::sync::Mutex<FivesbotLiveState>>,
    /// Unlocked Polymarket credentials kept in memory only.
    pub polymarket_credentials: Arc<tokio::sync::Mutex<Option<crate::crypto::StoredCredentials>>>,
    /// Polymarket CLOB client for live trading (created on Connect Wallet).
    pub polymarket_client: Arc<tokio::sync::Mutex<Option<polymarket_client::PolymarketClient>>>,
    /// Pending SRP handshakes.
    pub srp_sessions: Arc<tokio::sync::Mutex<HashMap<String, crate::srp_auth::PendingSrpSession>>>,
}

pub type SharedState = Arc<AppState>;

/// Create the axum router with all routes
pub fn create_router(state: SharedState) -> Router {
    let api_routes = Router::new()
        // Health
        .route("/api/health", axum::routing::get(routes::health))
        // Bot status
        .route("/api/bot/config", axum::routing::get(routes::bot_config))
        .route("/api/bot/status", axum::routing::get(routes::bot_status))
        .route("/api/fivesbot/status", axum::routing::get(routes::fivesbot_status))
        .route("/api/fivesbot/signals", axum::routing::get(routes::fivesbot_signals))
        .route("/api/fivesbot/trades", axum::routing::get(routes::fivesbot_trades))
        .route("/api/fivesbot/eth/status", axum::routing::get(routes::eth_fivesbot_status))
        .route("/api/fivesbot/eth/signals", axum::routing::get(routes::eth_fivesbot_signals))
        .route("/api/fivesbot/eth/trades", axum::routing::get(routes::eth_fivesbot_trades))
        .route("/api/fivesbot/wallet3/status", axum::routing::get(routes::wallet3_fivesbot_status))
        .route("/api/fivesbot/wallet3/signals", axum::routing::get(routes::wallet3_fivesbot_signals))
        .route("/api/fivesbot/wallet3/trades", axum::routing::get(routes::wallet3_fivesbot_trades))
        .route("/api/fivesbot/wallet4/status", axum::routing::get(routes::wallet4_fivesbot_status))
        .route("/api/fivesbot/wallet4/signals", axum::routing::get(routes::wallet4_fivesbot_signals))
        .route("/api/fivesbot/wallet4/trades", axum::routing::get(routes::wallet4_fivesbot_trades))
        // Wallet 1 (fivesbot BTC strategy)
        .route("/api/wallet/balance", axum::routing::get(routes::wallet_balance))
        .route("/api/wallet/positions", axum::routing::get(routes::wallet_positions))
        .route("/api/wallet/history", axum::routing::get(routes::wallet_history))
        .route("/api/wallet/deposit", axum::routing::post(routes::wallet_deposit))
        .route("/api/wallet/sync-prices", axum::routing::post(routes::wallet_sync_prices))
        .route("/api/wallet/settle", axum::routing::post(routes::wallet_settle))
        .route("/api/wallet2/balance", axum::routing::get(routes::wallet2_balance))
        .route("/api/wallet2/positions", axum::routing::get(routes::wallet2_positions))
        .route("/api/wallet2/history", axum::routing::get(routes::wallet2_history))
        .route("/api/wallet2/deposit", axum::routing::post(routes::wallet2_deposit))
        .route("/api/wallet2/sync-prices", axum::routing::post(routes::wallet2_sync_prices))
        .route("/api/wallet2/settle", axum::routing::post(routes::wallet2_settle))
        .route("/api/wallet3/balance", axum::routing::get(routes::wallet3_balance))
        .route("/api/wallet3/positions", axum::routing::get(routes::wallet3_positions))
        .route("/api/wallet3/history", axum::routing::get(routes::wallet3_history))
        .route("/api/wallet3/deposit", axum::routing::post(routes::wallet3_deposit))
        .route("/api/wallet3/sync-prices", axum::routing::post(routes::wallet3_sync_prices))
        .route("/api/wallet3/settle", axum::routing::post(routes::wallet3_settle))
        // Wallet 4 (BTC 5min classic_four)
        .route("/api/wallet4/balance", axum::routing::get(routes::wallet4_balance))
        .route("/api/wallet4/positions", axum::routing::get(routes::wallet4_positions))
        .route("/api/wallet4/history", axum::routing::get(routes::wallet4_history))
        .route("/api/wallet4/deposit", axum::routing::post(routes::wallet4_deposit))
        .route("/api/wallet4/sync-prices", axum::routing::post(routes::wallet4_sync_prices))
        .route("/api/wallet4/settle", axum::routing::post(routes::wallet4_settle))
        // Weather (wallet 5)
        .route("/api/weather/signals", axum::routing::get(routes::weather_signals))
        .route("/api/weather/forecasts", axum::routing::get(routes::weather_forecasts))
        .route("/api/weather/balance", axum::routing::get(routes::weather_wallet_balance))
        .route("/api/weather/positions", axum::routing::get(routes::weather_positions))
        .route("/api/weather/history", axum::routing::get(routes::weather_history))
        .route("/api/weather/deposit", axum::routing::post(routes::weather_deposit))
        // Polymarket live wallet connect + read-only data
        .route("/api/wallet/connect/status", axum::routing::get(srp_auth::wallet_status))
        .route("/api/wallet/connect/start", axum::routing::post(srp_auth::connect_start))
        .route("/api/wallet/connect/verify", axum::routing::post(srp_auth::connect_verify))
        .route("/api/wallet/connect/disconnect", axum::routing::post(srp_auth::disconnect))
        .route("/api/wallet/debug-unlock", axum::routing::post(srp_auth::debug_unlock_wallet))
        .route("/api/polymarket/balance", axum::routing::get(srp_auth::polymarket_balance))
        .route("/api/polymarket/positions", axum::routing::get(srp_auth::polymarket_positions))
        .route("/api/polymarket/orders", axum::routing::get(srp_auth::polymarket_orders))
        .route("/api/polymarket/trades", axum::routing::get(srp_auth::polymarket_trades))
        .route("/api/polymarket/test-order", axum::routing::post(srp_auth::test_polymarket_order));

    // Serve frontend static files with SPA fallback to index.html
    let frontend_dir = &state.frontend_dir;
    let index_path = format!("{}/index.html", frontend_dir);

    Router::new()
        .merge(api_routes)
        .fallback_service(
            ServeDir::new(frontend_dir)
                .not_found_service(tower_http::services::ServeFile::new(index_path))
        )
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Start the server
pub async fn run(frontend_dir: String, port: u16, db_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let db_pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(5)
        .connect(db_url)
        .await?;
    srp_auth::ensure_wallet_config_table(&db_pool).await?;

    // Initialize wallet 1 (fivesbot BTC)
    let wallet1 = virtual_wallet::VirtualWallet::new(db_url, "wallet1", 0.25, 100.0).await?;
    info!("✅ Wallet 1 (fivesbot BTC) initialized");

    // Initialize wallet 2 (fivesbot ETH)
    let wallet2 = virtual_wallet::VirtualWallet::new(db_url, "wallet2", 0.25, 100.0).await?;
    info!("✅ Wallet 2 (fivesbot ETH) initialized");

    // Initialize wallet 3 (BTC 8-indicator)
    let wallet3 = virtual_wallet::VirtualWallet::new(db_url, "wallet3", 0.25, 100.0).await?;
    info!("✅ Wallet 3 (BTC 8-indicator) initialized");

    // Initialize wallet 4 (BTC 5min classic_four)
    let wallet4 = virtual_wallet::VirtualWallet::new(db_url, "wallet4", 0.25, 100.0).await?;
    info!("✅ Wallet 4 (BTC 5min classic_four) initialized");

    // Initialize wallet 5 (weather)
    let wallet5 = virtual_wallet::VirtualWallet::new(db_url, "wallet5", 0.25, 100.0).await?;
    info!("✅ Wallet 5 (weather) initialized");

    // Initialize weather strategy
    let weather_config = wenbot_strategy::WenbotConfig::from_env();
    let weather_strategy = wenbot_strategy::WenbotStrategy::new(weather_config);
    info!("✅ Weather strategy initialized");

    // Initialize fivesbot BTC/ETH strategy
    let fivesbot_config = fivesbot_strategy::FivesbotConfig::from_env();
    let fivesbot_strategy = fivesbot_strategy::FivesbotStrategy::new(fivesbot_config.clone());
    let fivesbot_wallet3_strategy = fivesbot_strategy::FivesbotStrategy::new_eight_indicator(fivesbot_config.clone());
    let fivesbot_wallet4_strategy = fivesbot_strategy::FivesbotStrategy::new(fivesbot_config);
    let fivesbot_state = Arc::new(tokio::sync::Mutex::new(FivesbotLiveState::default()));
    info!("✅ Fivesbot BTC/ETH strategy initialized");
    info!("✅ Wallet3 BTC 8-indicator strategy initialized");
    info!("✅ Wallet4 BTC 5min classic_four strategy initialized");

    let state = Arc::new(AppState {
        frontend_dir: frontend_dir.clone(),
        db_pool,
        wallet1,
        wallet2,
        wallet3,
        wallet4,
        wallet5,
        weather_strategy,
        fivesbot_strategy,
        fivesbot_wallet3_strategy,
        fivesbot_wallet4_strategy,
        fivesbot_state,
        polymarket_credentials: Arc::new(tokio::sync::Mutex::new(None)),
        polymarket_client: Arc::new(tokio::sync::Mutex::new(None)),
        srp_sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
    });

    let app = create_router(state.clone());
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    info!("🚀 Wenbot API server starting on http://0.0.0.0:{}", port);
    info!("📂 Frontend served from {}", frontend_dir);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Spawn bot schedulers in background
    let weather_scheduler_state = state.clone();
    tokio::spawn(async move {
        scheduler::start_weather_bot(weather_scheduler_state).await;
    });

    let fivesbot_scheduler_state = state.clone();
    tokio::spawn(async move {
        scheduler::start_fivesbot_bot(fivesbot_scheduler_state).await;
    });

    // Start fivesbot websocket for real-time price subscriptions
    let fivesbot_ws_state = state.clone();
    tokio::spawn(async move {
        fivesbot_ws::start_fivesbot_ws(fivesbot_ws_state).await;
    });

    axum::serve(listener, app).await?;

    Ok(())
}

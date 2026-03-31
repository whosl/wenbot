//! Bot schedulers — weather + fivesbot BTC loops, mark-to-market refresh and settlement

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use chrono::{DateTime, TimeDelta, Utc};
use fivesbot_strategy::{compute_microstructure, BtcMicrostructure, Candle, SignalAction};
use polymarket_client::UpDownMarket;
use serde_json::Value;
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};

use crate::fivesbot_state::{FivesbotPredictorState, UpDownMarketInfo};
use crate::weather_markets::{fetch_actual_temperature, fetch_daily_temperature_markets};
use crate::AppState;

pub async fn start_weather_bot(state: Arc<AppState>) {
    info!("🌤️  Weather bot scheduler starting...");

    let cycle_state = state.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(cycle_state.weather_strategy.config().scan_interval_seconds));
        loop {
            tick.tick().await;
            if let Err(e) = run_weather_cycle(&cycle_state).await {
                error!("Weather cycle error: {}", e);
            }
        }
    });

    let price_state = state.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(180));
        loop {
            tick.tick().await;
            if let Err(e) = refresh_open_position_prices(&price_state).await {
                error!("Price refresh error: {}", e);
            }
        }
    });

    let settlement_state = state.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(3600));
        loop {
            tick.tick().await;
            if let Err(e) = settle_expired_weather_positions(&settlement_state).await {
                error!("Settlement cycle error: {}", e);
            }
        }
    });
}

/// Backfill BTC trade ledger from existing virtual_positions + trade_history.
/// Called once at startup. Skips positions that already have ledger entries.
pub async fn backfill_btc_trade_ledger(state: &AppState) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    use virtual_wallet::BtcTradeLedgerInsert;

    let positions = state.wallet1.get_all_btc_positions().await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    let mut backfilled = 0usize;

    for pos in positions {
        // Skip if already in ledger
        if state.wallet1.has_btc_ledger_entry(pos.id).await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)? {
            continue;
        }

        let asset_price = pos.btc_price.unwrap_or(0.0);

        let entry = BtcTradeLedgerInsert {
            trade_id: format!("btc-{}", pos.id),
            wallet_id: "wallet1".to_string(),
            position_id: pos.id,
            market_slug: pos.market_id.clone(),
            market_question: pos.market_question.clone(),
            token_id: pos.token_id.clone(),
            direction: pos.direction.clone(),
            predicted_direction: String::new(),
            effective_direction: String::new(),
            opened_at: pos.created_at.clone(),
            entry_price: pos.entry_price,
            quantity: pos.quantity,
            size: pos.size,
            edge: 0.0,
            confidence: 0.0,
            model_probability: 0.0,
            market_probability: 0.0,
            suggested_size: 0.0,
            reasoning: "Backfilled from virtual_positions (no signal context available)".to_string(),
            indicator_scores: "{}".to_string(),
            indicator_details: None,
            asset_price,
            fee: pos.fee,
            slippage: pos.slippage,
            is_reconstructed: true,
            reconstruction_source: None,
            match_score: None,
        };

        state.wallet1.insert_btc_trade_ledger(&entry).await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        backfilled += 1;

        // If this position is settled, also update the settlement fields
        if let (Some(sv), Some(ref closed), Some(pnl_val)) = (pos.settlement_value, pos.settled_at, pos.pnl) {
            let result_str = if sv > 0.5 { "win" } else { "loss" };
            state.wallet1.update_btc_trade_ledger_settlement(pos.id, &closed, result_str, pnl_val, sv).await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        }
    }

    if backfilled > 0 {
        info!("₿ Backfilled {} BTC trades into trade ledger", backfilled);
    }
    Ok(backfilled)
}

pub async fn backfill_eth_trade_ledger(state: &AppState) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    use virtual_wallet::BtcTradeLedgerInsert;

    let positions = state.wallet2.get_all_btc_positions().await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    let mut backfilled = 0usize;

    for pos in positions {
        if state.wallet2.has_btc_ledger_entry(pos.id).await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)? {
            continue;
        }

        let asset_price = pos.btc_price.unwrap_or(0.0);
        let entry = BtcTradeLedgerInsert {
            trade_id: format!("eth-{}", pos.id),
            wallet_id: "wallet2".to_string(),
            position_id: pos.id,
            market_slug: pos.market_id.clone(),
            market_question: pos.market_question.clone(),
            token_id: pos.token_id.clone(),
            direction: pos.direction.clone(),
            predicted_direction: String::new(),
            effective_direction: String::new(),
            opened_at: pos.created_at.clone(),
            entry_price: pos.entry_price,
            quantity: pos.quantity,
            size: pos.size,
            edge: 0.0,
            confidence: 0.0,
            model_probability: 0.0,
            market_probability: 0.0,
            suggested_size: 0.0,
            reasoning: "Backfilled from virtual_positions (no signal context available)".to_string(),
            indicator_scores: "{}".to_string(),
            indicator_details: None,
            asset_price,
            fee: pos.fee,
            slippage: pos.slippage,
            is_reconstructed: true,
            reconstruction_source: None,
            match_score: None,
        };

        state.wallet2.insert_btc_trade_ledger(&entry).await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        backfilled += 1;

        if let (Some(sv), Some(ref closed), Some(pnl_val)) = (pos.settlement_value, pos.settled_at, pos.pnl) {
            let result_str = if sv > 0.5 { "win" } else { "loss" };
            state.wallet2.update_btc_trade_ledger_settlement(pos.id, &closed, result_str, pnl_val, sv).await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        }
    }

    if backfilled > 0 {
        info!("Ξ Backfilled {} ETH trades into trade ledger", backfilled);
    }
    Ok(backfilled)
}

pub async fn backfill_wallet4_trade_ledger(state: &AppState) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    use virtual_wallet::BtcTradeLedgerInsert;

    let positions = state.wallet4.get_all_btc_positions().await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    let mut backfilled = 0usize;

    for pos in positions {
        if state.wallet4.has_btc_ledger_entry(pos.id).await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)? {
            continue;
        }

        let asset_price = pos.btc_price.unwrap_or(0.0);
        let entry = BtcTradeLedgerInsert {
            trade_id: format!("btc5m-{}", pos.id),
            wallet_id: "wallet4".to_string(),
            position_id: pos.id,
            market_slug: pos.market_id.clone(),
            market_question: pos.market_question.clone(),
            token_id: pos.token_id.clone(),
            direction: pos.direction.clone(),
            predicted_direction: String::new(),
            effective_direction: String::new(),
            opened_at: pos.created_at.clone(),
            entry_price: pos.entry_price,
            quantity: pos.quantity,
            size: pos.size,
            edge: 0.0,
            confidence: 0.0,
            model_probability: 0.0,
            market_probability: 0.0,
            suggested_size: 0.0,
            reasoning: "Backfilled from virtual_positions (no signal context available)".to_string(),
            indicator_scores: "{}".to_string(),
            indicator_details: None,
            asset_price,
            fee: pos.fee,
            slippage: pos.slippage,
            is_reconstructed: true,
            reconstruction_source: None,
            match_score: None,
        };

        state.wallet4.insert_btc_trade_ledger(&entry).await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        backfilled += 1;

        if let (Some(sv), Some(ref closed), Some(pnl_val)) = (pos.settlement_value, pos.settled_at, pos.pnl) {
            let result_str = if sv > 0.5 { "win" } else { "loss" };
            state.wallet4.update_btc_trade_ledger_settlement(pos.id, &closed, result_str, pnl_val, sv).await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        }
    }

    if backfilled > 0 {
        info!("₿ Backfilled {} W4 BTC5m trades into trade ledger", backfilled);
    }
    Ok(backfilled)
}

pub async fn start_fivesbot_bot(state: Arc<AppState>) {
    info!("₿ Fivesbot scheduler starting...");

    // Backfill BTC trade ledger on startup
    match backfill_btc_trade_ledger(&state).await {
        Ok(n) if n > 0 => info!("₿ Backfilled {} BTC trade ledger entries at startup", n),
        Ok(_) => {}
        Err(e) => error!("₿ Failed to backfill BTC trade ledger: {}", e),
    }
    match backfill_eth_trade_ledger(&state).await {
        Ok(n) if n > 0 => info!("Ξ Backfilled {} ETH trade ledger entries at startup", n),
        Ok(_) => {}
        Err(e) => error!("Ξ Failed to backfill ETH trade ledger: {}", e),
    }

    let scan_state = state.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(30));
        loop {
            tick.tick().await;
            if let Err(e) = run_fivesbot_cycle(&scan_state, "btc", None).await {
                error!("Fivesbot cycle error: {}", e);
            }
        }
    });

    let price_state = state.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(60));
        loop {
            tick.tick().await;
            if let Err(e) = sync_wallet1_prices(&price_state).await {
                error!("Wallet1 price sync error: {}", e);
            }
        }
    });

    let settlement_state = state.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(120));
        loop {
            tick.tick().await;
            if let Err(e) = settle_wallet1_positions(&settlement_state).await {
                error!("Wallet1 settlement error: {}", e);
            }
        }
    });

    let eth_scan_state = state.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(30));
        loop {
            tick.tick().await;
            if let Err(e) = run_fivesbot_cycle(&eth_scan_state, "eth", None).await {
                error!("ETH fivesbot cycle error: {}", e);
            }
        }
    });

    let eth_price_state = state.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(60));
        loop {
            tick.tick().await;
            if let Err(e) = sync_wallet2_prices(&eth_price_state).await {
                error!("Wallet2 price sync error: {}", e);
            }
        }
    });

    let eth_settlement_state = state.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(120));
        loop {
            tick.tick().await;
            if let Err(e) = settle_wallet2_positions(&eth_settlement_state).await {
                error!("Wallet2 settlement error: {}", e);
            }
        }
    });

    let wallet3_scan_state = state.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(30));
        loop {
            tick.tick().await;
            if let Err(e) = run_fivesbot_cycle(&wallet3_scan_state, "btc", Some("wallet3")).await {
                error!("Wallet3 BTC8 cycle error: {}", e);
            }
        }
    });

    let wallet3_price_state = state.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(60));
        loop {
            tick.tick().await;
            if let Err(e) = sync_wallet3_prices(&wallet3_price_state).await {
                error!("Wallet3 price sync error: {}", e);
            }
        }
    });

    let wallet3_settlement_state = state.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(120));
        loop {
            tick.tick().await;
            if let Err(e) = settle_wallet3_positions(&wallet3_settlement_state).await {
                error!("Wallet3 settlement error: {}", e);
            }
        }
    });

    // Wallet 4: BTC 5min classic_four
    match backfill_wallet4_trade_ledger(&state).await {
        Ok(n) if n > 0 => info!("₿ Backfilled {} W4 BTC 5min trade ledger entries at startup", n),
        Ok(_) => {}
        Err(e) => error!("₿ Failed to backfill W4 trade ledger: {}", e),
    }

    let wallet4_scan_state = state.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(30));
        loop {
            tick.tick().await;
            if let Err(e) = run_fivesbot_5m_cycle(&wallet4_scan_state).await {
                error!("Wallet4 BTC5m cycle error: {}", e);
            }
        }
    });

    let wallet4_price_state = state.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(60));
        loop {
            tick.tick().await;
            if let Err(e) = sync_wallet4_prices(&wallet4_price_state).await {
                error!("Wallet4 price sync error: {}", e);
            }
        }
    });

    let wallet4_settlement_state = state.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(120));
        loop {
            tick.tick().await;
            if let Err(e) = settle_wallet4_positions(&wallet4_settlement_state).await {
                error!("Wallet4 settlement error: {}", e);
            }
        }
    });
}

async fn run_weather_cycle(state: &AppState) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("🌤️  Weather cycle starting...");

    let bundles = fetch_daily_temperature_markets().await?;
    let history = state
        .wallet5
        .get_recent_history(state.weather_strategy.config().kelly_lookback_trades as i64)
        .await
        .unwrap_or_default();
    let recent_win_rate = if history.is_empty() {
        None
    } else {
        Some(history.iter().filter(|t| t.pnl > 0.0).count() as f64 / history.len() as f64)
    };

    let api_failures = Arc::new(AtomicUsize::new(0));
    let wallet_state = state.wallet5.wallet_state().await?;
    let today_realized_pnl = state.wallet5.get_today_realized_pnl().await.unwrap_or(0.0);
    let open_exposure = state.wallet5.get_open_exposure().await.unwrap_or(0.0);
    let cfg = state.weather_strategy.config();

    if wallet_state.balance < cfg.low_balance_threshold {
        error!("Weather wallet balance is low: ${:.2}", wallet_state.balance);
    }
    if today_realized_pnl <= -cfg.daily_loss_limit {
        warn!("Daily loss limit breached: realized pnl ${:.2} <= -${:.2}", today_realized_pnl, cfg.daily_loss_limit);
    }

    let mut signals_generated = 0;
    let mut trades_opened = 0;

    for bundle in bundles {
        let market = bundle.market;
        let forecast = bundle.forecast;
        let nws_temp = bundle.nws_temp_f;

        let signal = match state.weather_strategy.generate_signal_with_win_rate(&market, &forecast, nws_temp, wallet_state.balance, recent_win_rate) {
            Ok(s) => s,
            Err(_) => continue,
        };
        signals_generated += 1;

        if !signal.passes_threshold || signal.suggested_size <= 0.5 {
            continue;
        }

        let direction = match signal.direction {
            wenbot_strategy::SignalDirection::Yes => "YES",
            wenbot_strategy::SignalDirection::No => "NO",
        };

        if state.wallet5.has_open_position(&signal.market_id, direction).await.unwrap_or(false) {
            info!("Skipping duplicate weather position {} {}", signal.market_id, direction);
            continue;
        }

        if today_realized_pnl <= -cfg.daily_loss_limit {
            warn!("Skipping trade because daily loss limit is active");
            continue;
        }

        let token_id = if direction == "YES" { market.token_id_yes.clone() } else { market.token_id_no.clone() };
        let fee_param = match virtual_wallet::FeeService::fetch_fee_param(&token_id).await {
            Some(fp) => fp,
            None => {
                api_failures.fetch_add(1, Ordering::SeqCst);
                0.25
            }
        };

        let mut size = signal.suggested_size.min(cfg.max_trade_size);
        let exposure_cap = wallet_state.balance * cfg.total_exposure_cap_ratio;
        if open_exposure >= exposure_cap {
            warn!("Skipping trade because exposure cap is already exhausted");
            continue;
        }
        size = size.min((exposure_cap - open_exposure).max(0.0));
        if size <= 0.5 {
            continue;
        }

        let trade = virtual_wallet::TradeInput {
            market_id: signal.market_id.clone(),
            market_question: signal.market_question.clone(),
            token_id: token_id.clone(),
            direction: direction.to_string(),
            entry_price: signal.entry_price,
            size,
            slippage: 0.01,
            category: "weather".to_string(),
            target_date: Some(market.target_date.clone()),
            threshold_f: Some(market.threshold_f),
            city_name: Some(market.city_name.clone()),
            metric: Some(market.metric.clone()),
            event_slug: Some(market.slug.clone()),
            window_end: None,
            btc_price: None,
            fee_param: Some(fee_param),
        };

        match state.wallet5.open_position(&trade).await {
            Ok(_) => {
                info!(
                    "🌤️  OPENED POLY WEATHER: {} {} {} @ {:.2} (edge={:+.1}%)",
                    market.city_name, market.metric, market.direction, signal.entry_price, signal.edge * 100.0
                );
                trades_opened += 1;
            }
            Err(e) => warn!("Failed to open position for {}: {}", market.question, e),
        }
    }

    if api_failures.load(Ordering::SeqCst) >= cfg.api_failure_alert_threshold {
        error!("External API failures reached {} during weather cycle", api_failures.load(Ordering::SeqCst));
    }

    info!("🌤️  Weather cycle complete: {} signals, {} trades opened", signals_generated, trades_opened);
    Ok(())
}

async fn run_fivesbot_cycle(state: &AppState, asset: &str, wallet_override: Option<&str>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let markets = fetch_active_updown_markets(asset).await?;
    let micro = fetch_microstructure(asset).await?;
    let (wallet, wallet_id, strategy, asset_key, category) = match wallet_override.unwrap_or("") {
        "wallet3" => (&state.wallet3, "wallet3", &state.fivesbot_wallet3_strategy, "wallet3", "btc8".to_string()),
        _ if asset.eq_ignore_ascii_case("eth") => (&state.wallet2, "wallet2", &state.fivesbot_strategy, "eth", asset.to_lowercase()),
        _ => (&state.wallet1, "wallet1", &state.fivesbot_strategy, "btc", asset.to_lowercase()),
    };
    let wallet_state = wallet.wallet_state().await?;
    let daily_pnl = wallet.get_today_realized_pnl().await.unwrap_or(0.0);
    let cfg = strategy.config();

    {
        let mut live = state.fivesbot_state.lock().await;
        if asset_key.eq_ignore_ascii_case("eth") {
            live.eth_last_update = Some(Utc::now());
            live.eth_price = Some(micro.price);
            live.eth_active_markets = markets.iter().cloned().map(UpDownMarketInfo::from).collect();
            if let Some(first) = markets.first() {
                live.eth_current_cycle = format_cycle_label(first);
                live.eth_up_price = Some(first.up_price);
                live.eth_down_price = Some(first.down_price);
            }
        } else if asset_key.eq_ignore_ascii_case("wallet3") {
            live.wallet3_last_update = Some(Utc::now());
            live.wallet3_price = Some(micro.price);
            live.wallet3_active_markets = markets.iter().cloned().map(UpDownMarketInfo::from).collect();
            if let Some(first) = markets.first() {
                live.wallet3_current_cycle = format_cycle_label(first);
                live.wallet3_up_price = Some(first.up_price);
                live.wallet3_down_price = Some(first.down_price);
            }
        } else {
            live.btc_last_update = Some(Utc::now());
            live.btc_price = Some(micro.price);
            live.btc_active_markets = markets.iter().cloned().map(UpDownMarketInfo::from).collect();
            if let Some(first) = markets.first() {
                live.btc_current_cycle = format_cycle_label(first);
                live.btc_up_price = Some(first.up_price);
                live.btc_down_price = Some(first.down_price);
            }
        }
    }

    let daily_loss_limit_hit = daily_pnl <= -1000.0;
    if daily_loss_limit_hit {
        warn!("Skipping {} fivesbot trades: daily loss limit hit ({:.2})", asset.to_uppercase(), daily_pnl);
    }

    for market in markets {
        let seconds_remaining = seconds_remaining(&market.end_date).unwrap_or_default();
        let mut signal = match strategy.generate_signal(
            &market.slug,
            &micro,
            market.up_price,
            market.down_price,
            seconds_remaining,
            wallet_state.balance,
        ) {
            Ok(signal) => signal,
            Err(e) => {
                warn!("{} generate_signal failed for {}: {}", asset, market.slug, e);
                continue;
            }
        };

        signal.buy_token_id = if matches!(signal.action, SignalAction::BuyUp) {
            market.up_token_id.clone()
        } else {
            market.down_token_id.clone()
        };

        {
            let mut live = state.fivesbot_state.lock().await;
            let predictor = Some(build_predictor_state(&signal, &micro));
            if asset_key.eq_ignore_ascii_case("eth") {
                live.eth_predictor = predictor;
                live.eth_current_cycle = format_cycle_label(&market);
                live.eth_up_price = Some(market.up_price);
                live.eth_down_price = Some(market.down_price);
            } else if asset_key.eq_ignore_ascii_case("wallet3") {
                live.wallet3_predictor = predictor;
                live.wallet3_current_cycle = format_cycle_label(&market);
                live.wallet3_up_price = Some(market.up_price);
                live.wallet3_down_price = Some(market.down_price);
            } else {
                live.btc_predictor = predictor;
                live.btc_current_cycle = format_cycle_label(&market);
                live.btc_up_price = Some(market.up_price);
                live.btc_down_price = Some(market.down_price);
            }
            live.push_signal(asset_key, signal.clone());
        }

        if signal.suggested_size <= 0.0 || matches!(signal.action, SignalAction::Hold) {
            continue;
        }

        if daily_loss_limit_hit {
            warn!("Skipping trade because daily loss limit is active");
            continue;
        }

        let direction = if matches!(signal.action, SignalAction::BuyUp) { "YES" } else { "NO" };
        if wallet.has_open_position(&market.slug, direction).await.unwrap_or(false) {
            continue;
        }

        let fee_param = virtual_wallet::FeeService::fetch_fee_param(&signal.buy_token_id).await.unwrap_or(0.25);
        let trade = virtual_wallet::TradeInput {
            market_id: market.slug.clone(),
            market_question: market.question.clone(),
            token_id: signal.buy_token_id.clone(),
            direction: direction.to_string(),
            entry_price: signal.buy_price,
            size: signal.suggested_size.min(cfg.max_trade_size),
            category: category.clone(),
            slippage: 0.01,
            target_date: None,
            threshold_f: None,
            city_name: None,
            metric: None,
            event_slug: Some(market.event_slug.clone()),
            window_end: market.end_date.clone(),
            btc_price: Some(micro.price),
            fee_param: Some(fee_param),
        };

        match wallet.open_position(&trade).await {
            Ok(pos_id) => {
                let icon = if asset.eq_ignore_ascii_case("eth") { "Ξ" } else { "₿" };
                info!(
                    "{} OPENED FIVESBOT: {} {} @ {:.3} size ${:.2} edge {:+.1}%",
                    icon,
                    market.slug,
                    direction,
                    signal.buy_price,
                    trade.size,
                    signal.edge * 100.0,
                );

                let indicator_json = serde_json::to_string(&signal.indicator_scores).unwrap_or_else(|_| "{}".to_string());
                let indicator_details_json = serde_json::to_string(&signal.indicator_details).ok();
                if let Some(details) = &indicator_details_json {
                    info!("{} indicator_details {} {}", icon, market.slug, details);
                }
                let ledger_entry = virtual_wallet::BtcTradeLedgerInsert {
                    trade_id: format!("{}-{}", asset, pos_id.id),
                    wallet_id: wallet_id.to_string(),
                    position_id: pos_id.id,
                    market_slug: market.slug.clone(),
                    market_question: market.question.clone(),
                    token_id: signal.buy_token_id.clone(),
                    direction: direction.to_string(),
                    predicted_direction: signal.predicted_direction.clone(),
                    effective_direction: signal.effective_direction.clone(),
                    opened_at: Utc::now().to_rfc3339(),
                    entry_price: signal.buy_price,
                    quantity: pos_id.quantity,
                    size: trade.size,
                    edge: signal.edge,
                    confidence: signal.confidence,
                    model_probability: signal.model_probability,
                    market_probability: signal.market_probability,
                    suggested_size: signal.suggested_size,
                    reasoning: signal.reasoning.clone(),
                    indicator_scores: indicator_json,
                    indicator_details: indicator_details_json,
                    asset_price: micro.price,
                    fee: pos_id.fee,
                    slippage: pos_id.slippage,
                    is_reconstructed: false,
                    reconstruction_source: None,
                    match_score: None,
                };
                if let Err(e) = wallet.insert_btc_trade_ledger(&ledger_entry).await {
                    error!("Failed to write {} trade ledger for position {}: {}", asset.to_uppercase(), pos_id.id, e);
                }
            }
            Err(e) => warn!("Failed to open {} fivesbot position on {}: {}", asset.to_uppercase(), market.slug, e),
        }
    }

    Ok(())
}

async fn sync_wallet_prices(wallet: &virtual_wallet::VirtualWallet) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    let positions = wallet.list_positions().await?;
    let mut updated = 0usize;
    for pos in positions {
        if let Some(price) = fetch_token_price(&pos.token_id).await? {
            wallet.record_price(&pos.market_id, &pos.token_id, price).await?;
            updated += 1;
        }
    }
    Ok(updated)
}

pub async fn sync_wallet1_prices(state: &AppState) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    sync_wallet_prices(&state.wallet1).await
}

pub async fn sync_wallet2_prices(state: &AppState) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    sync_wallet_prices(&state.wallet2).await
}

pub async fn sync_wallet3_prices(state: &AppState) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    sync_wallet_prices(&state.wallet3).await
}

pub async fn sync_wallet4_prices(state: &AppState) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    sync_wallet_prices(&state.wallet4).await
}

async fn settle_wallet_positions(wallet: &virtual_wallet::VirtualWallet) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    let positions = wallet.list_positions().await?;
    let mut settled = 0usize;
    let now = Utc::now();

    for pos in positions {
        let Some(window_end) = pos_window_end(&pos.market_id, &pos.created_at, pos.token_id.as_str(), wallet).await? else {
            continue;
        };
        if window_end > now {
            continue;
        }

        let market_slug = pos.market_id.clone();
        let market = fetch_market_by_slug(&market_slug).await?;
        let Some(token_won) = resolved_token_won(&market, &pos.token_id) else { continue; };
        let settlement_value = if token_won { 1.0 } else { 0.0 };
        wallet.settle_and_credit(pos.id, settlement_value, None).await?;
        let credit = settlement_value * pos.quantity;
        let settled_pnl = credit - pos.size - pos.fee;
        let closed_at = Utc::now().to_rfc3339();
        let result_str = if token_won { "win" } else { "loss" };
        if let Err(e) = wallet.update_btc_trade_ledger_settlement(pos.id, &closed_at, result_str, settled_pnl, settlement_value).await {
            tracing::warn!("Failed to update trade ledger settlement for position {}: {}", pos.id, e);
        }
        settled += 1;
    }
    Ok(settled)
}

pub async fn settle_wallet1_positions(state: &AppState) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    settle_wallet_positions(&state.wallet1).await
}

pub async fn settle_wallet2_positions(state: &AppState) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    settle_wallet_positions(&state.wallet2).await
}

pub async fn settle_wallet3_positions(state: &AppState) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    settle_wallet_positions(&state.wallet3).await
}

pub async fn settle_wallet4_positions(state: &AppState) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    settle_wallet_positions_5m(&state.wallet4).await
}

async fn refresh_open_position_prices(state: &AppState) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let positions = state.wallet5.list_positions().await?;
    let mut failures = 0usize;

    for pos in positions {
        match fetch_token_price(&pos.token_id).await {
            Ok(Some(price)) => {
                state.wallet5.record_price(&pos.market_id, &pos.token_id, price).await?;
            }
            Ok(None) => {}
            Err(e) => {
                failures += 1;
                warn!("Price refresh failed for token {}: {}", pos.token_id, e);
            }
        }
    }

    if failures >= state.weather_strategy.config().api_failure_alert_threshold {
        error!("Price refresh API failures reached {}", failures);
    }
    Ok(())
}

async fn settle_expired_weather_positions(state: &AppState) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let today = Utc::now().date_naive().format("%Y-%m-%d").to_string();
    let positions = state.wallet5.get_positions_ready_for_settlement("weather", &today).await?;

    for pos in positions {
        let city = match &pos.city_name { Some(v) => v.clone(), None => continue };
        let target_date = match &pos.target_date { Some(v) => v.clone(), None => continue };
        let metric = pos.metric.clone().unwrap_or_else(|| "high".to_string());
        let actual_temp = fetch_actual_temperature(&city, &target_date, &metric).await;
        let actual_temp = match actual_temp {
            Some(v) => v,
            None => {
                warn!("No settlement temperature for {} {}", city, target_date);
                continue;
            }
        };

        let settlement_value = evaluate_settlement(&pos.market_question, &pos.direction, pos.threshold_f, actual_temp);
        if settlement_value != 0.0 && settlement_value != 1.0 {
            error!("Invalid settlement value {} for position {}", settlement_value, pos.id);
            continue;
        }

        let credit = state.wallet5.settle_and_credit(pos.id, settlement_value, Some(actual_temp)).await?;
        info!(
            "Settled weather position {} for {} on {} at {:.1}°F => {} (credit {:.4})",
            pos.id,
            city,
            target_date,
            actual_temp,
            settlement_value,
            credit,
        );
    }

    Ok(())
}

fn evaluate_settlement(question: &str, trade_direction: &str, threshold: Option<f64>, actual_temp: f64) -> f64 {
    let threshold = threshold.unwrap_or(actual_temp);
    let q = question.to_lowercase();
    let market_yes = if q.contains(" or above") || q.contains("above ") {
        actual_temp >= threshold
    } else if q.contains(" or below") || q.contains("below ") {
        actual_temp <= threshold
    } else if q.contains(" between ") {
        let nums: Vec<f64> = q
            .split(|c: char| !(c.is_ascii_digit() || c == '.'))
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse::<f64>().ok())
            .collect();
        if nums.len() >= 2 {
            actual_temp >= nums[0] && actual_temp <= nums[1]
        } else {
            false
        }
    } else {
        warn!("Unknown weather market question for settlement: {}", question);
        false
    };

    let trade_wins = match trade_direction.to_uppercase().as_str() {
        "YES" => market_yes,
        "NO" => !market_yes,
        other => {
            warn!("Unknown trade direction for settlement: {}", other);
            false
        }
    };
    if trade_wins { 1.0 } else { 0.0 }
}

/// Create a reqwest client that routes through the local Clash proxy.
/// data-api.binance.vision used to work without proxy but is now blocked.
fn proxied_client(timeout_secs: u64) -> Result<reqwest::Client, Box<dyn std::error::Error + Send + Sync>> {
    let proxy = reqwest::Proxy::all("http://127.0.0.1:7890")?;
    reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .proxy(proxy)
        .build()
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
}

pub async fn fetch_active_updown_markets(asset: &str) -> Result<Vec<UpDownMarket>, Box<dyn std::error::Error + Send + Sync>> {
    let client = proxied_client(15)?;

    // BTC updown markets use slug pattern: btc-updown-15m-{epoch}
    // epoch is the window start time in UTC seconds (despite being labeled "ET" in the question)
    // Each window is 15 minutes (900 seconds).
    let now = Utc::now();
    let now_epoch = now.timestamp();
    // Round down to nearest 15-min boundary
    let window_epoch = (now_epoch / 900) * 900;

    // Query slugs concurrently
    let mut handles: Vec<_> = Vec::new();
    for offset in -1..=3i32 {
        let epoch = window_epoch + (offset as i64) * 900;
        let slug = format!("{}-updown-15m-{}", asset.to_lowercase(), epoch);
        let c = client.clone();
        handles.push(tokio::spawn(async move {
            let resp = c
                .get("https://gamma-api.polymarket.com/events")
                .query(&[("slug", slug.as_str())])
                .send()
                .await;
            match resp {
                Ok(r) if r.status().is_success() => r.json::<Value>().await.ok(),
                _ => None,
            }
        }));
    }

    let mut out = Vec::new();
    for handle in handles {
        let Some(Some(maybe_events)) = handle.await.ok() else { continue };
        let events = maybe_events.get("data")
            .and_then(|v| v.as_array())
            .or_else(|| maybe_events.as_array())
            .cloned()
            .unwrap_or_default();

        for event in events {
            for market in event.get("markets").and_then(|v| v.as_array()).into_iter().flatten() {
                let slug = market.get("slug").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                let question = market.get("question").and_then(|v| v.as_str()).unwrap_or_default().to_string();

                // Only BTC updown markets
                let slug_prefix = format!("{}-updown-15m-", asset.to_lowercase());
                if !slug.starts_with(&slug_prefix) {
                    continue;
                }

                let outcomes = parse_string_array(market.get("outcomes"));
                if outcomes.len() != 2 {
                    continue;
                }
                let up_idx = if outcomes[0].eq_ignore_ascii_case("up") { 0 } else { 1 };
                let down_idx = 1 - up_idx;

                let token_ids = parse_string_array(market.get("clobTokenIds").or_else(|| market.get("clob_token_ids")));
                let prices = parse_f64_array(market.get("outcomePrices").or_else(|| market.get("outcome_prices")));
                if token_ids.len() < 2 || prices.len() < 2 {
                    continue;
                }

                let end_date = market
                    .get("endDate")
                    .or_else(|| market.get("end_date"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                // Skip already-closed markets (resolved to 1.0 / 0.0)
                let up_price = prices[up_idx];
                let down_price = prices[down_idx];
                if up_price >= 0.99 || down_price >= 0.99 {
                    continue;
                }

                // Check seconds remaining - only trade windows with > 1 min left
                if let Some(secs) = seconds_remaining(&end_date) {
                    if secs < 60 || secs > 15 * 60 {
                        continue;
                    }
                }

                let event_slug = event.get("slug").and_then(|v| v.as_str()).unwrap_or(asset).to_string();

                out.push(UpDownMarket {
                    market_id: market.get("id").or_else(|| market.get("market_id")).and_then(|v| v.as_str()).unwrap_or(&slug).to_string(),
                    condition_id: market.get("conditionId").or_else(|| market.get("condition_id")).and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    slug,
                    question,
                    up_token_id: token_ids[up_idx].clone(),
                    down_token_id: token_ids[down_idx].clone(),
                    up_price,
                    down_price,
                    end_date,
                    event_slug,
                });
            }
        }
    }

    out.sort_by_key(|m| {
        m.end_date
            .as_ref()
            .and_then(|s| parse_dt(s))
            .map(|dt| (dt - now).num_seconds())
            .unwrap_or(i64::MAX)
    });
    Ok(out)
}

pub async fn fetch_active_updown_5m_markets(asset: &str) -> Result<Vec<UpDownMarket>, Box<dyn std::error::Error + Send + Sync>> {
    let client = proxied_client(15)?;

    let now = Utc::now();
    let now_epoch = now.timestamp();
    // 5-minute windows
    let window_epoch = (now_epoch / 300) * 300;

    let mut handles: Vec<_> = Vec::new();
    for offset in -1..=3i32 {
        let epoch = window_epoch + (offset as i64) * 300;
        let slug = format!("{}-updown-5m-{}", asset.to_lowercase(), epoch);
        let c = client.clone();
        handles.push(tokio::spawn(async move {
            let resp = c
                .get("https://gamma-api.polymarket.com/events")
                .query(&[("slug", slug.as_str())])
                .send()
                .await;
            match resp {
                Ok(r) if r.status().is_success() => r.json::<Value>().await.ok(),
                _ => None,
            }
        }));
    }

    let mut out = Vec::new();
    for handle in handles {
        let Some(Some(maybe_events)) = handle.await.ok() else { continue };
        let events = maybe_events.get("data")
            .and_then(|v| v.as_array())
            .or_else(|| maybe_events.as_array())
            .cloned()
            .unwrap_or_default();

        for event in events {
            for market in event.get("markets").and_then(|v| v.as_array()).into_iter().flatten() {
                let slug = market.get("slug").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                let question = market.get("question").and_then(|v| v.as_str()).unwrap_or_default().to_string();

                let slug_prefix = format!("{}-updown-5m-", asset.to_lowercase());
                if !slug.starts_with(&slug_prefix) {
                    continue;
                }

                let outcomes = parse_string_array(market.get("outcomes"));
                if outcomes.len() != 2 {
                    continue;
                }
                let up_idx = if outcomes[0].eq_ignore_ascii_case("up") { 0 } else { 1 };
                let down_idx = 1 - up_idx;

                let token_ids = parse_string_array(market.get("clobTokenIds").or_else(|| market.get("clob_token_ids")));
                let prices = parse_f64_array(market.get("outcomePrices").or_else(|| market.get("outcome_prices")));
                if token_ids.len() < 2 || prices.len() < 2 {
                    continue;
                }

                let end_date = market
                    .get("endDate")
                    .or_else(|| market.get("end_date"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let up_price = prices[up_idx];
                let down_price = prices[down_idx];
                if up_price >= 0.99 || down_price >= 0.99 {
                    continue;
                }

                // 5-minute windows: trade windows with > 30s left and < 5min
                if let Some(secs) = seconds_remaining(&end_date) {
                    if secs < 30 || secs > 5 * 60 {
                        continue;
                    }
                }

                let event_slug = event.get("slug").and_then(|v| v.as_str()).unwrap_or(asset).to_string();

                out.push(UpDownMarket {
                    market_id: market.get("id").or_else(|| market.get("market_id")).and_then(|v| v.as_str()).unwrap_or(&slug).to_string(),
                    condition_id: market.get("conditionId").or_else(|| market.get("condition_id")).and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    slug,
                    question,
                    up_token_id: token_ids[up_idx].clone(),
                    down_token_id: token_ids[down_idx].clone(),
                    up_price,
                    down_price,
                    end_date,
                    event_slug,
                });
            }
        }
    }

    out.sort_by_key(|m| {
        m.end_date
            .as_ref()
            .and_then(|s| parse_dt(s))
            .map(|dt| (dt - now).num_seconds())
            .unwrap_or(i64::MAX)
    });
    Ok(out)
}

async fn run_fivesbot_5m_cycle(state: &AppState) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let markets = fetch_active_updown_5m_markets("btc").await?;
    let micro = fetch_microstructure_5m("btc").await?;
    let wallet = &state.wallet4;
    let wallet_id = "wallet4";
    let strategy = &state.fivesbot_wallet4_strategy;
    let wallet_state = wallet.wallet_state().await?;
    let daily_pnl = wallet.get_today_realized_pnl().await.unwrap_or(0.0);
    let cfg = strategy.config();

    {
        let mut live = state.fivesbot_state.lock().await;
        live.wallet4_last_update = Some(Utc::now());
        live.wallet4_price = Some(micro.price);
        live.wallet4_active_markets = markets.iter().cloned().map(UpDownMarketInfo::from).collect();
        if let Some(first) = markets.first() {
            live.wallet4_current_cycle = format_cycle_label(first);
            live.wallet4_up_price = Some(first.up_price);
            live.wallet4_down_price = Some(first.down_price);
        }
    }

    let daily_loss_limit_hit = daily_pnl <= -1000.0;
    if daily_loss_limit_hit {
        warn!("Skipping W4 BTC5m trades: daily loss limit hit ({:.2})", daily_pnl);
    }

    for market in markets {
        let seconds_remaining = seconds_remaining(&market.end_date).unwrap_or_default();
        let mut signal = match strategy.generate_signal(
            &market.slug,
            &micro,
            market.up_price,
            market.down_price,
            seconds_remaining,
            wallet_state.balance,
        ) {
            Ok(signal) => signal,
            Err(e) => {
                warn!("W4 BTC5m generate_signal failed for {}: {}", market.slug, e);
                continue;
            }
        };

        signal.buy_token_id = if matches!(signal.action, SignalAction::BuyUp) {
            market.up_token_id.clone()
        } else {
            market.down_token_id.clone()
        };

        {
            let mut live = state.fivesbot_state.lock().await;
            live.wallet4_predictor = Some(build_predictor_state(&signal, &micro));
            live.wallet4_current_cycle = format_cycle_label(&market);
            live.wallet4_up_price = Some(market.up_price);
            live.wallet4_down_price = Some(market.down_price);
            live.push_signal("wallet4", signal.clone());
        }

        if signal.suggested_size <= 0.0 || matches!(signal.action, SignalAction::Hold) {
            continue;
        }

        if daily_loss_limit_hit {
            continue;
        }

        let direction = if matches!(signal.action, SignalAction::BuyUp) { "YES" } else { "NO" };
        if wallet.has_open_position(&market.slug, direction).await.unwrap_or(false) {
            continue;
        }

        let fee_param = virtual_wallet::FeeService::fetch_fee_param(&signal.buy_token_id).await.unwrap_or(0.25);
        let trade = virtual_wallet::TradeInput {
            market_id: market.slug.clone(),
            market_question: market.question.clone(),
            token_id: signal.buy_token_id.clone(),
            direction: direction.to_string(),
            entry_price: signal.buy_price,
            size: signal.suggested_size.min(cfg.max_trade_size),
            category: "btc5m".to_string(),
            slippage: 0.01,
            target_date: None,
            threshold_f: None,
            city_name: None,
            metric: None,
            event_slug: Some(market.event_slug.clone()),
            window_end: market.end_date.clone(),
            btc_price: Some(micro.price),
            fee_param: Some(fee_param),
        };

        match wallet.open_position(&trade).await {
            Ok(pos_id) => {
                info!(
                    "₿5m OPENED FIVESBOT: {} {} @ {:.3} size ${:.2} edge {:+.1}%",
                    market.slug,
                    direction,
                    signal.buy_price,
                    trade.size,
                    signal.edge * 100.0,
                );

                let indicator_json = serde_json::to_string(&signal.indicator_scores).unwrap_or_else(|_| "{}".to_string());
                let indicator_details_json = serde_json::to_string(&signal.indicator_details).ok();
                if let Some(details) = &indicator_details_json {
                    info!("₿5m indicator_details {} {}", market.slug, details);
                }
                let ledger_entry = virtual_wallet::BtcTradeLedgerInsert {
                    trade_id: format!("btc5m-{}", pos_id.id),
                    wallet_id: wallet_id.to_string(),
                    position_id: pos_id.id,
                    market_slug: market.slug.clone(),
                    market_question: market.question.clone(),
                    token_id: signal.buy_token_id.clone(),
                    direction: direction.to_string(),
                    predicted_direction: signal.predicted_direction.clone(),
                    effective_direction: signal.effective_direction.clone(),
                    opened_at: Utc::now().to_rfc3339(),
                    entry_price: signal.buy_price,
                    quantity: pos_id.quantity,
                    size: trade.size,
                    edge: signal.edge,
                    confidence: signal.confidence,
                    model_probability: signal.model_probability,
                    market_probability: signal.market_probability,
                    suggested_size: signal.suggested_size,
                    reasoning: signal.reasoning.clone(),
                    indicator_scores: indicator_json,
                    indicator_details: indicator_details_json,
                    asset_price: micro.price,
                    fee: pos_id.fee,
                    slippage: pos_id.slippage,
                    is_reconstructed: false,
                    reconstruction_source: None,
                    match_score: None,
                };
                if let Err(e) = wallet.insert_btc_trade_ledger(&ledger_entry).await {
                    error!("Failed to write W4 BTC5m trade ledger for position {}: {}", pos_id.id, e);
                }
            }
            Err(e) => warn!("Failed to open W4 BTC5m fivesbot position on {}: {}", market.slug, e),
        }
    }

    Ok(())
}

async fn fetch_microstructure_5m(asset: &str) -> Result<BtcMicrostructure, Box<dyn std::error::Error + Send + Sync>> {
    let client = proxied_client(15)?;

    // Use 5m candles for the 5-minute strategy
    let candles = match fetch_candles_binance_5m(&client, asset).await {
        Ok(c) => {
            info!("Fetched {} BTC 5m candles from Binance", c.len());
            c
        }
        Err(e) => {
            warn!("Binance 5m API failed ({}), falling back to 1m aggregated", e);
            fetch_candles_binance(&client, asset).await?
        }
    };

    let mut micro = compute_microstructure(&candles).ok_or("insufficient BTC 5m candles")?;

    if micro.change_24h == 0.0 {
        let day_data: Value = client
            .get("https://min-api.cryptocompare.com/data/v2/histohour")
            .query(&[("fsym", if asset.eq_ignore_ascii_case("eth") { "ETH" } else { "BTC" }), ("tsym", "USD"), ("limit", "24")])
            .send()
            .await?
            .json()
            .await?;
        let hour_data = day_data.get("Data")
            .and_then(|v| v.get("Data"))
            .and_then(|v| v.as_array());
        if let Some(hours) = hour_data {
            if let (Some(first), Some(last)) = (hours.first(), hours.last()) {
                let first_close = first.get("close").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let last_close = last.get("close").and_then(|v| v.as_f64()).unwrap_or(0.0);
                if first_close > 0.0 {
                    micro.change_24h = (last_close - first_close) / first_close * 100.0;
                }
            }
        }
    }

    Ok(micro)
}

/// Fetch BTC 5m candles from Binance
async fn fetch_candles_binance_5m(client: &reqwest::Client, asset: &str) -> Result<Vec<Candle>, Box<dyn std::error::Error + Send + Sync>> {
    let resp = client
        .get("https://data-api.binance.vision/api/v3/klines")
        .query(&[("symbol", if asset.eq_ignore_ascii_case("eth") { "ETHUSDT" } else { "BTCUSDT" }), ("interval", "5m"), ("limit", "100")])
        .send()
        .await?
        .error_for_status()?;

    let klines: Value = resp.json().await?;
    let candles: Vec<Candle> = klines
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| {
            let arr = row.as_array()?;
            Some(Candle {
                timestamp: arr.first()?.as_i64()?,
                open: arr.get(1)?.as_str()?.parse().ok()?,
                high: arr.get(2)?.as_str()?.parse().ok()?,
                low: arr.get(3)?.as_str()?.parse().ok()?,
                close: arr.get(4)?.as_str()?.parse().ok()?,
                volume: arr.get(5)?.as_str()?.parse().ok()?,
            })
        })
        .collect();

    if candles.is_empty() {
        return Err("Binance returned 0 5m candles".into());
    }
    Ok(candles)
}

async fn settle_wallet_positions_5m(wallet: &virtual_wallet::VirtualWallet) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    let positions = wallet.list_positions().await?;
    let mut settled = 0usize;
    let now = Utc::now();

    for pos in positions {
        let Some(window_end) = pos_window_end_5m(&pos.market_id, &pos.created_at) else {
            continue;
        };
        if window_end > now {
            continue;
        }

        let market_slug = pos.market_id.clone();
        let market = fetch_market_by_slug(&market_slug).await?;
        let Some(token_won) = resolved_token_won(&market, &pos.token_id) else { continue; };
        let settlement_value = if token_won { 1.0 } else { 0.0 };
        wallet.settle_and_credit(pos.id, settlement_value, None).await?;
        let credit = settlement_value * pos.quantity;
        let settled_pnl = credit - pos.size - pos.fee;
        let closed_at = Utc::now().to_rfc3339();
        let result_str = if token_won { "win" } else { "loss" };
        if let Err(e) = wallet.update_btc_trade_ledger_settlement(pos.id, &closed_at, result_str, settled_pnl, settlement_value).await {
            tracing::warn!("Failed to update trade ledger settlement for W4 position {}: {}", pos.id, e);
        }
        settled += 1;
    }
    Ok(settled)
}

fn pos_window_end_5m(market_id: &str, created_at: &str) -> Option<DateTime<Utc>> {
    if let Some(dt) = parse_slug_window_end_5m(market_id) {
        return Some(dt);
    }
    parse_dt(created_at).map(|dt| dt + TimeDelta::minutes(5))
}

fn parse_slug_window_end_5m(slug: &str) -> Option<DateTime<Utc>> {
    let ts = slug.rsplit('-').next()?.parse::<i64>().ok()?;
    DateTime::from_timestamp(ts + 5 * 60, 0)
}

async fn fetch_microstructure(asset: &str) -> Result<BtcMicrostructure, Box<dyn std::error::Error + Send + Sync>> {
    let client = proxied_client(15)?;

    // Try Binance first (best data quality), fallback to CryptoCompare
    let candles = match fetch_candles_binance(&client, asset).await {
        Ok(c) => {
            info!("Fetched {} BTC candles from Binance", c.len());
            c
        }
        Err(e) => {
            warn!("Binance API failed ({}), falling back to CryptoCompare", e);
            fetch_candles_cryptocompare(&client, asset).await?
        }
    };

    let mut micro = compute_microstructure(&candles).ok_or("insufficient BTC candles")?;

    // Fetch 24h change
    if micro.change_24h == 0.0 {
        let day_data: Value = client
            .get("https://min-api.cryptocompare.com/data/v2/histohour")
            .query(&[("fsym", if asset.eq_ignore_ascii_case("eth") { "ETH" } else { "BTC" }), ("tsym", "USD"), ("limit", "24")])
            .send()
            .await?
            .json()
            .await?;
        let hour_data = day_data.get("Data")
            .and_then(|v| v.get("Data"))
            .and_then(|v| v.as_array());
        if let Some(hours) = hour_data {
            if let (Some(first), Some(last)) = (hours.first(), hours.last()) {
                let first_close = first.get("close").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let last_close = last.get("close").and_then(|v| v.as_f64()).unwrap_or(0.0);
                if first_close > 0.0 {
                    micro.change_24h = (last_close - first_close) / first_close * 100.0;
                }
            }
        }
    }

    Ok(micro)
}

/// Fetch BTC 1m candles from Binance (data-api.binance.vision bypasses geo-restriction)
async fn fetch_candles_binance(client: &reqwest::Client, asset: &str) -> Result<Vec<Candle>, Box<dyn std::error::Error + Send + Sync>> {
    let resp = client
        .get("https://data-api.binance.vision/api/v3/klines")
        .query(&[("symbol", if asset.eq_ignore_ascii_case("eth") { "ETHUSDT" } else { "BTCUSDT" }), ("interval", "1m"), ("limit", "100")])
        .send()
        .await?
        .error_for_status()?;

    let klines: Value = resp.json().await?;
    let candles: Vec<Candle> = klines
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| {
            let arr = row.as_array()?;
            Some(Candle {
                timestamp: arr.first()?.as_i64()?,
                open: arr.get(1)?.as_str()?.parse().ok()?,
                high: arr.get(2)?.as_str()?.parse().ok()?,
                low: arr.get(3)?.as_str()?.parse().ok()?,
                close: arr.get(4)?.as_str()?.parse().ok()?,
                volume: arr.get(5)?.as_str()?.parse().ok()?,
            })
        })
        .collect();

    if candles.is_empty() {
        return Err("Binance returned 0 candles".into());
    }
    Ok(candles)
}

/// Fallback: fetch BTC 1m candles from CryptoCompare
async fn fetch_candles_cryptocompare(client: &reqwest::Client, asset: &str) -> Result<Vec<Candle>, Box<dyn std::error::Error + Send + Sync>> {
    let klines: Value = client
        .get("https://min-api.cryptocompare.com/data/v2/histominute")
        .query(&[("fsym", if asset.eq_ignore_ascii_case("eth") { "ETH" } else { "BTC" }), ("tsym", "USD"), ("limit", "100")])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let kline_data = klines.get("Data")
        .and_then(|v| v.get("Data"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let candles: Vec<Candle> = kline_data
        .into_iter()
        .filter_map(|row| {
            Some(Candle {
                timestamp: row.get("time")?.as_i64()? * 1000,
                open: row.get("open")?.as_f64()?,
                high: row.get("high")?.as_f64()?,
                low: row.get("low")?.as_f64()?,
                close: row.get("close")?.as_f64()?,
                volume: row.get("volumeto").and_then(|v| v.as_f64()).unwrap_or(0.0),
            })
        })
        .collect();

    if candles.is_empty() {
        return Err("CryptoCompare returned 0 candles".into());
    }
    Ok(candles)
}

async fn fetch_token_price(token_id: &str) -> Result<Option<f64>, Box<dyn std::error::Error + Send + Sync>> {
    let client = proxied_client(10)?;
    let url = format!("https://clob.polymarket.com/price?token_id={}&side=buy", token_id);
    let resp = client.get(&url).send().await?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    let json: Value = resp.json().await?;
    Ok(json.get("price").and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))))
}

async fn fetch_market_by_slug(slug: &str) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let client = proxied_client(10)?;
    Ok(client
        .get(format!("https://gamma-api.polymarket.com/markets/slug/{slug}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

fn resolved_token_won(market: &Value, token_id: &str) -> Option<bool> {
    let token_ids = parse_string_array(market.get("clobTokenIds").or_else(|| market.get("clob_token_ids")));
    let prices = parse_f64_array(market.get("outcomePrices").or_else(|| market.get("outcome_prices")));
    let idx = token_ids.iter().position(|t| t == token_id)?;
    let price = *prices.get(idx)?;
    if price >= 0.99 {
        Some(true)
    } else if price <= 0.01 {
        Some(false)
    } else {
        None
    }
}

async fn pos_window_end(
    market_id: &str,
    created_at: &str,
    _token_id: &str,
    _wallet: &virtual_wallet::VirtualWallet,
) -> Result<Option<DateTime<Utc>>, Box<dyn std::error::Error + Send + Sync>> {
    if let Some(dt) = parse_slug_window_end(market_id) {
        return Ok(Some(dt));
    }
    Ok(parse_dt(created_at).map(|dt| dt + TimeDelta::minutes(15)))
}

fn build_predictor_state(
    signal: &fivesbot_strategy::TradingSignal,
    micro: &BtcMicrostructure,
) -> FivesbotPredictorState {
    let trend = signal.indicator_scores.composite;
    let momentum = signal.indicator_scores.momentum_signal;
    let volatility = micro.volatility_regime.abs().max(micro.momentum_15m.abs() / 100.0);
    let signal_name = match signal.action {
        SignalAction::BuyUp => "UP",
        SignalAction::BuyDown => "DOWN",
        SignalAction::Hold => "HOLD",
    };

    FivesbotPredictorState {
        signal: signal_name.to_string(),
        confidence: signal.confidence,
        direction: signal.effective_direction.clone(),
        trend,
        momentum,
        volatility,
        rsi: micro.rsi,
    }
}

fn format_cycle_label(market: &UpDownMarket) -> String {
    let suffix = market
        .end_date
        .as_ref()
        .and_then(|v| parse_dt(v))
        .map(|dt| dt.format("%H:%M").to_string())
        .unwrap_or_else(|| "--:--".to_string());
    format!("{} ({})", market.slug, suffix)
}

fn seconds_remaining(end_date: &Option<String>) -> Option<u32> {
    let dt = parse_dt(end_date.as_ref()?)?;
    let secs = (dt - Utc::now()).num_seconds();
    if secs < 0 { None } else { Some(secs as u32) }
}

fn parse_dt(input: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(input).ok().map(|dt| dt.with_timezone(&Utc))
}

fn parse_slug_window_end(slug: &str) -> Option<DateTime<Utc>> {
    let ts = slug.rsplit('-').next()?.parse::<i64>().ok()?;
    DateTime::from_timestamp(ts + 15 * 60, 0)
}

fn parse_string_array(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(arr)) => arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect(),
        Some(Value::String(s)) => serde_json::from_str::<Vec<String>>(s).unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn parse_f64_array(value: Option<&Value>) -> Vec<f64> {
    match value {
        Some(Value::Array(arr)) => arr.iter().filter_map(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))).collect(),
        Some(Value::String(s)) => serde_json::from_str::<Vec<String>>(s)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| v.parse().ok())
            .collect(),
        _ => Vec::new(),
    }
}

fn contains_up_down(outcomes: &[String]) -> bool {
    outcomes.iter().any(|o| o.eq_ignore_ascii_case("up"))
        && outcomes.iter().any(|o| o.eq_ignore_ascii_case("down"))
}

//! Virtual Wallet Manager — high-level wallet operations

use crate::db::{OpenPositionParams, WalletDatabase};
use crate::error::{Result, WalletError};
use crate::fee::FeeService;
use crate::models::*;
use crate::valuation::{mark_to_market, unrealized_pnl};
use tracing::info;

/// The main virtual wallet interface
pub struct VirtualWallet {
    db: WalletDatabase,
    default_fee_param: f64,
}

impl VirtualWallet {
    pub async fn new(
        database_url: &str,
        wallet_id: &str,
        default_fee_param: f64,
        initial_balance: f64,
    ) -> Result<Self> {
        let db = WalletDatabase::new(database_url, wallet_id, initial_balance).await?;
        Ok(Self { db, default_fee_param })
    }

    pub async fn new_default(
        database_url: &str,
        default_fee_param: f64,
        initial_balance: f64,
    ) -> Result<Self> {
        Self::new(database_url, "wallet1", default_fee_param, initial_balance).await
    }

    pub async fn from_env() -> Result<Self> {
        let db_url = std::env::var("VIRTUAL_WALLET_DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:virtual_wallet.db".into());
        let wallet_id = std::env::var("VIRTUAL_WALLET_ID")
            .unwrap_or_else(|_| "wallet1".into());
        let fee_param = std::env::var("DEFAULT_FEE_PARAM")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.25);
        let initial_balance = std::env::var("VIRTUAL_WALLET_INITIAL_BALANCE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(100.0);
        Self::new(&db_url, &wallet_id, fee_param, initial_balance).await
    }

    pub async fn get_balance(&self) -> Result<BalanceSummary> {
        let wallet = self.db.get_wallet_state().await?;
        let positions = self.db.get_open_positions().await?;

        let mut total_position_value = 0.0;
        for pos in &positions {
            let latest = self.db.get_latest_price(&pos.market_id, &pos.token_id).await?;
            total_position_value += mark_to_market(pos, latest);
        }

        let total_value = wallet.balance + total_position_value;
        let win_rate = if wallet.total_trades > 0 {
            (wallet.winning_trades as f64 / wallet.total_trades as f64) * 100.0
        } else {
            0.0
        };

        Ok(BalanceSummary {
            balance: (wallet.balance * 100.0).round() / 100.0,
            total_position_value: (total_position_value * 100.0).round() / 100.0,
            total_value: (total_value * 100.0).round() / 100.0,
            total_trades: wallet.total_trades,
            winning_trades: wallet.winning_trades,
            win_rate: (win_rate * 10.0).round() / 10.0,
            total_pnl: (wallet.total_pnl * 100.0).round() / 100.0,
        })
    }

    pub async fn deposit(&self, amount: f64) -> Result<BalanceSummary> {
        if amount <= 0.0 {
            return Err(WalletError::InvalidInput(format!(
                "Invalid deposit amount: {}",
                amount
            )));
        }

        self.db.update_balance(amount).await?;
        info!("Wallet '{}' credited: +${:.2}", self.db.wallet_id(), amount);
        self.get_balance().await
    }

    pub async fn reset(&self, initial_balance: f64) -> Result<()> {
        self.db.reset_wallet(initial_balance).await?;
        info!("Wallet '{}' reset to ${:.2}", self.db.wallet_id(), initial_balance);
        Ok(())
    }

    pub async fn open_position(&self, trade: &TradeInput) -> Result<PositionSummary> {
        self.validate_trade(&trade.direction, trade.entry_price, trade.size)?;

        let wallet = self.db.get_wallet_state().await?;
        if trade.size > wallet.balance {
            return Err(WalletError::InsufficientBalance {
                need: trade.size,
                have: wallet.balance,
            });
        }

        let fee_param = trade.fee_param.unwrap_or(self.default_fee_param);
        let fee = FeeService::calculate_fee(trade.entry_price, fee_param);
        let total_cost = trade.size + fee;

        if total_cost > wallet.balance {
            return Err(WalletError::InsufficientBalance {
                need: total_cost,
                have: wallet.balance,
            });
        }

        let slippage = if trade.slippage > 0.0 { trade.slippage } else { 0.01 };

        let pos_params = OpenPositionParams {
            market_id: trade.market_id.clone(),
            market_question: trade.market_question.clone(),
            token_id: trade.token_id.clone(),
            direction: trade.direction.clone(),
            entry_price: trade.entry_price,
            size: trade.size,
            fee,
            slippage,
            category: trade.category.clone(),
            target_date: trade.target_date.clone(),
            threshold_f: trade.threshold_f,
            city_name: trade.city_name.clone(),
            metric: trade.metric.clone(),
            event_slug: trade.event_slug.clone(),
            window_end: trade.window_end.clone(),
            btc_price: trade.btc_price,
        };

        let pos_id = self.db.open_position(&pos_params).await?;
        self.db.update_balance(-total_cost).await?;

        info!(
            "Wallet '{}' position opened: {} {} @ ${:.4}, size=${:.2}, fee=${:.4}",
            self.db.wallet_id(), trade.direction, trade.market_question, trade.entry_price, trade.size, fee
        );

        self.get_position_summary(pos_id).await
    }

    pub async fn settle_position(
        &self,
        position_id: i64,
        settlement_value: f64,
        actual_temp: Option<f64>,
    ) -> Result<()> {
        self.db.settle_position(position_id, settlement_value, actual_temp).await?;
        let result = if settlement_value > 0.5 { "WIN" } else { "LOSS" };
        info!(
            "Wallet '{}' position {} settled: {} (value={:.2})",
            self.db.wallet_id(), position_id, result, settlement_value
        );
        Ok(())
    }

    pub async fn settle_and_credit(
        &self,
        position_id: i64,
        settlement_value: f64,
        actual_temp: Option<f64>,
    ) -> Result<f64> {
        let credit = self.db.settle_and_credit(position_id, settlement_value, actual_temp).await?;
        let result = if settlement_value > 0.5 { "WIN" } else { "LOSS" };
        info!(
            "Wallet '{}' position {} settled_and_credited: {} (value={:.2}, credit={:.4})",
            self.db.wallet_id(), position_id, result, settlement_value, credit
        );
        Ok(credit)
    }

    pub async fn record_price(&self, market_id: &str, token_id: &str, price: f64) -> Result<()> {
        self.db.record_price(market_id, token_id, price).await
    }

    /// Get the latest price snapshot for a token from the database.
    pub async fn get_latest_price(&self, market_id: &str, token_id: &str) -> Result<Option<f64>> {
        self.db.get_latest_price(market_id, token_id).await
    }

    pub async fn list_positions(&self) -> Result<Vec<PositionSummary>> {
        let positions = self.db.get_open_positions().await?;
        let mut summaries = Vec::new();

        for pos in positions {
            let latest = self.db.get_latest_price(&pos.market_id, &pos.token_id).await?;
            let current_value = mark_to_market(&pos, latest);
            let pnl = unrealized_pnl(&pos, current_value);

            summaries.push(PositionSummary {
                id: pos.id,
                market_id: pos.market_id,
                market_question: pos.market_question,
                token_id: pos.token_id,
                direction: pos.direction,
                entry_price: pos.entry_price,
                size: pos.size,
                quantity: pos.quantity,
                fee: pos.fee,
                slippage: pos.slippage,
                current_price: latest,
                current_value: (current_value * 100.0).round() / 100.0,
                unrealized_pnl: (pnl * 100.0).round() / 100.0,
                category: pos.category,
                status: pos.status,
                created_at: pos.created_at,
                city_name: pos.city_name,
                metric: pos.metric,
                target_date: pos.target_date,
                threshold_f: pos.threshold_f,
                event_slug: pos.event_slug,
                window_end: pos.window_end,
            });
        }

        Ok(summaries)
    }

    /// List raw open positions (without MTM calc) for external price enrichment.
    pub async fn list_positions_raw(&self) -> Result<Vec<VirtualPosition>> {
        self.db.get_open_positions().await
    }

    pub async fn get_history(&self, limit: i64) -> Result<Vec<TradeHistory>> {
        self.db.get_trade_history(limit).await
    }

    pub async fn get_recent_history(&self, limit: i64) -> Result<Vec<TradeHistory>> {
        self.db.get_recent_trade_history(limit).await
    }

    pub async fn has_open_position(&self, market_id: &str, direction: &str) -> Result<bool> {
        self.db.has_open_position(market_id, direction).await
    }

    pub async fn get_today_realized_pnl(&self) -> Result<f64> {
        self.db.get_today_realized_pnl().await
    }

    pub async fn get_open_exposure(&self) -> Result<f64> {
        self.db.get_open_exposure().await
    }

    pub async fn get_positions_ready_for_settlement(&self, category: &str, today: &str) -> Result<Vec<VirtualPosition>> {
        self.db.get_positions_ready_for_settlement(category, today).await
    }

    pub async fn wallet_state(&self) -> Result<WalletState> {
        self.db.get_wallet_state().await
    }

    /// Get access to the underlying database pool (for advanced queries).
    pub fn db(&self) -> &sqlx::SqlitePool {
        self.db.pool()
    }

    // ─── BTC Trade Ledger ───

    /// Insert a BTC trade ledger entry with full signal context
    pub async fn insert_btc_trade_ledger(&self, entry: &BtcTradeLedgerInsert) -> Result<i64> {
        self.db.insert_btc_trade_ledger(entry).await
    }

    /// Check if a ledger entry exists for a position
    pub async fn has_btc_ledger_entry(&self, position_id: i64) -> Result<bool> {
        self.db.has_btc_ledger_entry(position_id).await
    }

    /// Update settlement fields on an existing ledger entry
    pub async fn update_btc_trade_ledger_settlement(
        &self,
        position_id: i64,
        closed_at: &str,
        result: &str,
        pnl: f64,
        settlement_value: f64,
    ) -> Result<()> {
        self.db.update_btc_trade_ledger_settlement(position_id, closed_at, result, pnl, settlement_value).await
    }

    /// Query BTC trade ledger entries
    pub async fn get_btc_trade_ledger(&self, limit: i64) -> Result<Vec<BtcTradeLedger>> {
        self.db.get_btc_trade_ledger(limit).await
    }

    /// Get all BTC positions (open and settled) for backfill purposes.
    pub async fn get_all_btc_positions(&self) -> Result<Vec<VirtualPosition>> {
        sqlx::query_as::<_, VirtualPosition>(
            "SELECT * FROM virtual_positions WHERE wallet_id = ?1 AND category = 'btc' ORDER BY created_at ASC"
        )
        .bind(self.db.wallet_id())
        .fetch_all(self.db.pool())
        .await
        .map_err(WalletError::DatabaseError)
    }

    fn validate_trade(&self, direction: &str, entry_price: f64, size: f64) -> Result<()> {
        match direction.to_uppercase().as_str() {
            "YES" | "NO" => {}
            other => return Err(WalletError::InvalidInput(format!("Invalid direction: {}", other))),
        }
        if !(0.0..1.0).contains(&entry_price) {
            return Err(WalletError::InvalidInput(format!("Invalid entry_price: {}", entry_price)));
        }
        if size <= 0.0 {
            return Err(WalletError::InvalidInput(format!("Invalid size: {}", size)));
        }
        Ok(())
    }

    async fn get_position_summary(&self, id: i64) -> Result<PositionSummary> {
        let pos = self.db.get_position(id).await?;
        let latest = self.db.get_latest_price(&pos.market_id, &pos.token_id).await?;
        let current_value = mark_to_market(&pos, latest);
        let pnl = unrealized_pnl(&pos, current_value);

        Ok(PositionSummary {
            id: pos.id,
            market_id: pos.market_id,
            market_question: pos.market_question,
            token_id: pos.token_id,
            direction: pos.direction,
            entry_price: pos.entry_price,
            size: pos.size,
            quantity: pos.quantity,
            fee: pos.fee,
            slippage: pos.slippage,
            current_price: latest,
            current_value: (current_value * 100.0).round() / 100.0,
            unrealized_pnl: (pnl * 100.0).round() / 100.0,
            category: pos.category,
            status: pos.status,
            created_at: pos.created_at,
            city_name: pos.city_name,
            metric: pos.metric,
            target_date: pos.target_date,
            threshold_f: pos.threshold_f,
            event_slug: pos.event_slug,
            window_end: pos.window_end,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PositionSummary {
    pub id: i64,
    pub market_id: String,
    pub market_question: String,
    pub token_id: String,
    pub direction: String,
    pub entry_price: f64,
    pub size: f64,
    pub quantity: f64,
    pub fee: f64,
    pub slippage: f64,
    pub current_price: Option<f64>,
    pub current_value: f64,
    pub unrealized_pnl: f64,
    pub category: String,
    pub status: String,
    pub created_at: String,
    pub city_name: Option<String>,
    pub metric: Option<String>,
    pub target_date: Option<String>,
    pub threshold_f: Option<f64>,
    pub event_slug: Option<String>,
    pub window_end: Option<String>,
}

#[cfg(test)]
mod tests {}

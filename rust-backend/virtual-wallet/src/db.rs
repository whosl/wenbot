//! SQLite database layer for the virtual wallet

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use tracing::info;

use crate::error::{Result, WalletError};
use crate::models::*;

/// Database wrapper providing typed access to the wallet SQLite DB
pub struct WalletDatabase {
    pool: SqlitePool,
    wallet_id: String,
}

impl WalletDatabase {
    /// Connect and initialize the database schema
    pub async fn new(database_url: &str, wallet_id: &str, initial_balance: f64) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        let db = Self {
            pool,
            wallet_id: wallet_id.to_string(),
        };
        db.init_schema().await?;
        db.ensure_wallet_state(initial_balance).await?;

        Ok(db)
    }

    /// Run schema migrations
    async fn init_schema(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS wallet_state (
                wallet_id TEXT PRIMARY KEY,
                balance REAL NOT NULL DEFAULT 100.0,
                total_trades INTEGER NOT NULL DEFAULT 0,
                winning_trades INTEGER NOT NULL DEFAULT 0,
                total_pnl REAL NOT NULL DEFAULT 0.0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS virtual_positions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                wallet_id TEXT NOT NULL DEFAULT 'wallet1',
                market_id TEXT NOT NULL,
                market_question TEXT,
                token_id TEXT,
                direction TEXT NOT NULL,
                entry_price REAL NOT NULL,
                effective_entry REAL,
                size REAL NOT NULL,
                quantity REAL NOT NULL,
                fee REAL DEFAULT 0.0,
                slippage REAL DEFAULT 0.0,
                category TEXT DEFAULT 'weather',
                target_date TEXT,
                threshold_f REAL,
                city_name TEXT,
                metric TEXT DEFAULT 'high',
                event_slug TEXT,
                window_end TEXT,
                btc_price REAL,
                status TEXT DEFAULT 'open',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                settled_at TEXT,
                settlement_value REAL,
                actual_temperature REAL,
                pnl REAL,
                rsi_version TEXT DEFAULT 'v2'
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS trade_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                wallet_id TEXT NOT NULL DEFAULT 'wallet1',
                position_id INTEGER,
                market_id TEXT,
                market_question TEXT,
                direction TEXT,
                entry_price REAL,
                size REAL,
                quantity REAL,
                settlement_value REAL,
                actual_temperature REAL,
                pnl REAL,
                result TEXT,
                opened_at TEXT,
                closed_at TEXT,
                indicator_details TEXT
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS price_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                wallet_id TEXT NOT NULL DEFAULT 'wallet1',
                market_id TEXT NOT NULL,
                token_id TEXT,
                price REAL NOT NULL,
                timestamp TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .execute(&self.pool)
        .await?;

        self.add_wallet_id_column_if_missing("wallet_state", "wallet_id TEXT NOT NULL DEFAULT 'wallet1'")
            .await?;
        self.add_wallet_id_column_if_missing("virtual_positions", "wallet_id TEXT NOT NULL DEFAULT 'wallet1'")
            .await?;
        self.add_wallet_id_column_if_missing("trade_history", "wallet_id TEXT NOT NULL DEFAULT 'wallet1'")
            .await?;
        self.add_column_if_missing("trade_history", "indicator_details", "TEXT DEFAULT NULL")
            .await?;
        self.add_wallet_id_column_if_missing("price_snapshots", "wallet_id TEXT NOT NULL DEFAULT 'wallet1'")
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_positions_wallet_status ON virtual_positions(wallet_id, status)")
            .execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_positions_wallet_market ON virtual_positions(wallet_id, market_id)")
            .execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_positions_wallet_market_direction_status ON virtual_positions(wallet_id, market_id, direction, status)")
            .execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_snapshots_wallet_market ON price_snapshots(wallet_id, market_id, token_id)")
            .execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_snapshots_time ON price_snapshots(timestamp)")
            .execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_trade_history_wallet_closed_at ON trade_history(wallet_id, closed_at)")
            .execute(&self.pool).await?;

        sqlx::query("UPDATE virtual_positions SET status = 'open' WHERE status IS NULL OR status = ''")
            .execute(&self.pool).await?;

        // ─── BTC Trade Ledger table ───
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS btc_trade_ledger (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                trade_id TEXT NOT NULL UNIQUE,
                wallet_id TEXT NOT NULL DEFAULT 'wallet1',
                position_id INTEGER NOT NULL,
                market_slug TEXT NOT NULL,
                market_question TEXT,
                token_id TEXT,
                direction TEXT NOT NULL,
                predicted_direction TEXT DEFAULT '',
                effective_direction TEXT DEFAULT '',
                opened_at TEXT NOT NULL,
                closed_at TEXT,
                entry_price REAL NOT NULL,
                quantity REAL NOT NULL,
                size REAL NOT NULL,
                edge REAL DEFAULT 0.0,
                confidence REAL DEFAULT 0.0,
                model_probability REAL DEFAULT 0.0,
                market_probability REAL DEFAULT 0.0,
                suggested_size REAL DEFAULT 0.0,
                reasoning TEXT DEFAULT '',
                indicator_scores TEXT DEFAULT '{}',
                indicator_details TEXT,
                asset_price REAL DEFAULT 0.0,
                result TEXT,
                pnl REAL,
                settlement_value REAL,
                fee REAL DEFAULT 0.0,
                slippage REAL DEFAULT 0.0,
                is_reconstructed INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_btc_ledger_position ON btc_trade_ledger(position_id)")
            .execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_btc_ledger_wallet ON btc_trade_ledger(wallet_id)")
            .execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_btc_ledger_opened ON btc_trade_ledger(wallet_id, opened_at)")
            .execute(&self.pool).await?;

        self.add_column_if_missing("btc_trade_ledger", "indicator_details", "TEXT DEFAULT NULL").await?;

        // Add reconstruction columns if missing
        self.add_column_if_missing("btc_trade_ledger", "reconstruction_source", "TEXT DEFAULT NULL").await?;
        self.add_column_if_missing("btc_trade_ledger", "match_score", "REAL DEFAULT NULL").await?;

        // Add rsi_version column if missing (added for multi-RSI strategy)
        self.add_column_if_missing("virtual_positions", "rsi_version", "TEXT DEFAULT 'v2'").await?;

        Ok(())
    }

    async fn add_wallet_id_column_if_missing(&self, table_name: &str, column_def: &str) -> Result<()> {
        let pragma = format!("PRAGMA table_info({})", table_name);
        let columns = sqlx::query_as::<_, TableInfoRow>(&pragma)
            .fetch_all(&self.pool)
            .await?;

        if !columns.iter().any(|column| column.name == "wallet_id") {
            let alter = format!("ALTER TABLE {} ADD COLUMN {}", table_name, column_def);
            sqlx::query(&alter).execute(&self.pool).await?;
        }

        Ok(())
    }

    async fn add_column_if_missing(&self, table_name: &str, column_name: &str, column_def: &str) -> Result<()> {
        let pragma = format!("PRAGMA table_info({})", table_name);
        let columns = sqlx::query_as::<_, TableInfoRow>(&pragma)
            .fetch_all(&self.pool)
            .await?;

        if !columns.iter().any(|column| column.name == column_name) {
            let alter = format!("ALTER TABLE {} ADD COLUMN {} {}", table_name, column_name, column_def);
            sqlx::query(&alter).execute(&self.pool).await?;
        }

        Ok(())
    }

    /// Ensure the wallet state singleton exists
    async fn ensure_wallet_state(&self, initial_balance: f64) -> Result<()> {
        let existing = sqlx::query_as::<_, WalletState>(
            "SELECT * FROM wallet_state WHERE wallet_id = ?1"
        )
        .bind(&self.wallet_id)
        .fetch_optional(&self.pool)
        .await?;

        if existing.is_none() {
            sqlx::query(
                "INSERT INTO wallet_state (wallet_id, balance, total_trades, winning_trades, total_pnl)
                 VALUES (?1, ?2, 0, 0, 0.0)",
            )
            .bind(&self.wallet_id)
            .bind(initial_balance)
            .execute(&self.pool)
            .await?;

            info!("Virtual wallet '{}' initialized with ${:.2}", self.wallet_id, initial_balance);
        }

        Ok(())
    }

    pub async fn get_wallet_state(&self) -> Result<WalletState> {
        sqlx::query_as::<_, WalletState>("SELECT * FROM wallet_state WHERE wallet_id = ?1")
            .bind(&self.wallet_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| WalletError::DatabaseError(e))
    }

    pub async fn update_balance(&self, delta: f64) -> Result<()> {
        sqlx::query(
            "UPDATE wallet_state SET balance = balance + ?1, updated_at = datetime('now') WHERE wallet_id = ?2",
        )
        .bind(delta)
        .bind(&self.wallet_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn increment_trade_stats(&self, won: bool, pnl: f64) -> Result<()> {
        sqlx::query(
            "UPDATE wallet_state SET
                total_trades = total_trades + 1,
                winning_trades = winning_trades + CASE WHEN ?1 THEN 1 ELSE 0 END,
                total_pnl = total_pnl + ?2,
                updated_at = datetime('now')
             WHERE wallet_id = ?3",
        )
        .bind(won)
        .bind(pnl)
        .bind(&self.wallet_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn open_position(&self, pos: &OpenPositionParams) -> Result<i64> {
        let effective_entry = Some(pos.entry_price * (1.0 + pos.slippage));
        let fee = pos.fee;
        let quantity = if effective_entry.unwrap_or(pos.entry_price) > 0.0 {
            pos.size / effective_entry.unwrap_or(pos.entry_price)
        } else {
            0.0
        };

        let result = sqlx::query(
            "INSERT INTO virtual_positions (
                wallet_id, market_id, market_question, token_id, direction, entry_price,
                effective_entry, size, quantity, fee, slippage, category,
                target_date, threshold_f, city_name, metric, event_slug,
                window_end, btc_price, status, created_at, rsi_version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, 'open', datetime('now'), 'v2')"
        )
        .bind(&self.wallet_id)
        .bind(&pos.market_id)
        .bind(&pos.market_question)
        .bind(&pos.token_id)
        .bind(&pos.direction)
        .bind(pos.entry_price)
        .bind(effective_entry)
        .bind(pos.size)
        .bind(quantity)
        .bind(fee)
        .bind(pos.slippage)
        .bind(&pos.category)
        .bind(&pos.target_date)
        .bind(pos.threshold_f)
        .bind(&pos.city_name)
        .bind(&pos.metric)
        .bind(&pos.event_slug)
        .bind(&pos.window_end)
        .bind(pos.btc_price)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    pub async fn get_open_positions(&self) -> Result<Vec<VirtualPosition>> {
        sqlx::query_as::<_, VirtualPosition>(
            "SELECT * FROM virtual_positions WHERE wallet_id = ?1 AND status = 'open' ORDER BY created_at DESC"
        )
        .bind(&self.wallet_id)
        .fetch_all(&self.pool)
        .await
        .map_err(WalletError::DatabaseError)
    }

    pub async fn has_open_position(&self, market_id: &str, direction: &str) -> Result<bool> {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT 1 FROM virtual_positions WHERE wallet_id = ?1 AND market_id = ?2 AND upper(direction) = upper(?3) AND status = 'open' LIMIT 1"
        )
        .bind(&self.wallet_id)
        .bind(market_id)
        .bind(direction)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.is_some())
    }

    pub async fn get_position(&self, id: i64) -> Result<VirtualPosition> {
        sqlx::query_as::<_, VirtualPosition>(
            "SELECT * FROM virtual_positions WHERE wallet_id = ?1 AND id = ?2"
        )
        .bind(&self.wallet_id)
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| WalletError::PositionNotFound(id))
    }

    pub async fn settle_position(&self, id: i64, settlement_value: f64, actual_temp: Option<f64>) -> Result<()> {
        let pos = self.get_position(id).await?;
        let credit = settlement_value * pos.quantity;
        let pnl = credit - pos.size - pos.fee;
        let won = settlement_value > 0.5;

        sqlx::query(
            "UPDATE virtual_positions SET
                status = 'settled',
                settlement_value = ?1,
                actual_temperature = ?2,
                pnl = ?3,
                settled_at = datetime('now')
             WHERE wallet_id = ?4 AND id = ?5"
        )
        .bind(settlement_value)
        .bind(actual_temp)
        .bind(pnl)
        .bind(&self.wallet_id)
        .bind(id)
        .execute(&self.pool)
        .await?;

        let indicator_details: Option<String> = sqlx::query_scalar(
            "SELECT indicator_details FROM btc_trade_ledger
             WHERE wallet_id = ?1 AND position_id = ?2
             ORDER BY id DESC LIMIT 1"
        )
        .bind(&self.wallet_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .flatten();

        sqlx::query(
            "INSERT INTO trade_history (
                wallet_id, position_id, market_id, market_question, direction, entry_price,
                size, quantity, settlement_value, actual_temperature, pnl, result,
                opened_at, closed_at, indicator_details
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, datetime('now'), ?14)"
        )
        .bind(&self.wallet_id)
        .bind(id)
        .bind(&pos.market_id)
        .bind(&pos.market_question)
        .bind(&pos.direction)
        .bind(pos.entry_price)
        .bind(pos.size)
        .bind(pos.quantity)
        .bind(settlement_value)
        .bind(actual_temp)
        .bind(pnl)
        .bind(if won { "win" } else { "loss" })
        .bind(&pos.created_at)
        .bind(indicator_details)
        .execute(&self.pool)
        .await?;

        self.increment_trade_stats(won, pnl).await?;
        Ok(())
    }

    pub async fn settle_and_credit(&self, id: i64, settlement_value: f64, actual_temp: Option<f64>) -> Result<f64> {
        self.settle_position(id, settlement_value, actual_temp).await?;
        let pos = self.get_position(id).await?;
        let credit = settlement_value * pos.quantity;
        if credit > 0.0 {
            self.update_balance(credit).await?;
        }
        Ok(credit)
    }

    pub async fn record_price(&self, market_id: &str, token_id: &str, price: f64) -> Result<()> {
        sqlx::query(
            "INSERT INTO price_snapshots (wallet_id, market_id, token_id, price)
             VALUES (?1, ?2, ?3, ?4)"
        )
        .bind(&self.wallet_id)
        .bind(market_id)
        .bind(token_id)
        .bind(price)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_latest_price(&self, market_id: &str, token_id: &str) -> Result<Option<f64>> {
        let row: Option<(f64,)> = sqlx::query_as(
            "SELECT price FROM price_snapshots
             WHERE wallet_id = ?1 AND market_id = ?2 AND token_id = ?3
             ORDER BY timestamp DESC LIMIT 1"
        )
        .bind(&self.wallet_id)
        .bind(market_id)
        .bind(token_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.0))
    }

    pub async fn get_trade_history(&self, limit: i64) -> Result<Vec<TradeHistory>> {
        sqlx::query_as::<_, TradeHistory>(
            "SELECT 
                th.id,
                th.wallet_id,
                th.position_id,
                th.market_id,
                th.market_question,
                th.direction,
                th.entry_price,
                th.size,
                th.quantity,
                th.settlement_value,
                th.actual_temperature,
                th.pnl,
                th.result,
                COALESCE(NULLIF(th.opened_at, ''), btl.opened_at, vp.created_at, '') AS opened_at,
                th.closed_at,
                th.indicator_details
             FROM trade_history th
             LEFT JOIN btc_trade_ledger btl
               ON btl.wallet_id = th.wallet_id AND btl.position_id = th.position_id
             LEFT JOIN virtual_positions vp
               ON vp.wallet_id = th.wallet_id AND vp.id = th.position_id
             WHERE th.wallet_id = ?1
             ORDER BY th.closed_at DESC
             LIMIT ?2"
        )
        .bind(&self.wallet_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(WalletError::DatabaseError)
    }

    pub async fn get_recent_trade_history(&self, limit: i64) -> Result<Vec<TradeHistory>> {
        self.get_trade_history(limit).await
    }

    pub async fn get_today_realized_pnl(&self) -> Result<f64> {
        let row: Option<(f64,)> = sqlx::query_as(
            "SELECT COALESCE(SUM(pnl), 0.0) FROM trade_history
             WHERE wallet_id = ?1 AND date(closed_at) = date('now', 'localtime')"
        )
        .bind(&self.wallet_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.0).unwrap_or(0.0))
    }

    pub async fn get_open_exposure(&self) -> Result<f64> {
        let row: Option<(f64,)> = sqlx::query_as(
            "SELECT COALESCE(SUM(size + COALESCE(fee, 0.0)), 0.0) FROM virtual_positions
             WHERE wallet_id = ?1 AND status = 'open'"
        )
        .bind(&self.wallet_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.0).unwrap_or(0.0))
    }

    pub async fn get_positions_ready_for_settlement(&self, category: &str, target_date: &str) -> Result<Vec<VirtualPosition>> {
        sqlx::query_as::<_, VirtualPosition>(
            "SELECT * FROM virtual_positions
             WHERE wallet_id = ?1 AND status = 'open' AND category = ?2 AND target_date IS NOT NULL AND date(target_date) < date(?3)"
        )
        .bind(&self.wallet_id)
        .bind(category)
        .bind(target_date)
        .fetch_all(&self.pool)
        .await
        .map_err(WalletError::DatabaseError)
    }

    pub async fn reset_wallet(&self, initial_balance: f64) -> Result<()> {
        sqlx::query("DELETE FROM virtual_positions WHERE wallet_id = ?1").bind(&self.wallet_id).execute(&self.pool).await?;
        sqlx::query("DELETE FROM trade_history WHERE wallet_id = ?1").bind(&self.wallet_id).execute(&self.pool).await?;
        sqlx::query("DELETE FROM price_snapshots WHERE wallet_id = ?1").bind(&self.wallet_id).execute(&self.pool).await?;
        sqlx::query("DELETE FROM btc_trade_ledger WHERE wallet_id = ?1").bind(&self.wallet_id).execute(&self.pool).await?;

        sqlx::query(
            "UPDATE wallet_state SET
                balance = ?1, total_trades = 0, winning_trades = 0, total_pnl = 0.0,
                updated_at = datetime('now')
             WHERE wallet_id = ?2"
        )
        .bind(initial_balance)
        .bind(&self.wallet_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // ─── BTC Trade Ledger methods ───

    /// Insert a new BTC trade ledger entry. Returns the ledger row id.
    pub async fn insert_btc_trade_ledger(&self, entry: &BtcTradeLedgerInsert) -> Result<i64> {
        let result = sqlx::query(
            "INSERT INTO btc_trade_ledger (
                trade_id, wallet_id, position_id, market_slug, market_question, token_id,
                direction, predicted_direction, effective_direction, opened_at, entry_price,
                quantity, size, edge, confidence, model_probability, market_probability,
                suggested_size, reasoning, indicator_scores, indicator_details, asset_price, fee, slippage, is_reconstructed,
                reconstruction_source, match_score
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27)"
        )
        .bind(&entry.trade_id)
        .bind(&entry.wallet_id)
        .bind(entry.position_id)
        .bind(&entry.market_slug)
        .bind(&entry.market_question)
        .bind(&entry.token_id)
        .bind(&entry.direction)
        .bind(&entry.predicted_direction)
        .bind(&entry.effective_direction)
        .bind(&entry.opened_at)
        .bind(entry.entry_price)
        .bind(entry.quantity)
        .bind(entry.size)
        .bind(entry.edge)
        .bind(entry.confidence)
        .bind(entry.model_probability)
        .bind(entry.market_probability)
        .bind(entry.suggested_size)
        .bind(&entry.reasoning)
        .bind(&entry.indicator_scores)
        .bind(&entry.indicator_details)
        .bind(entry.asset_price)
        .bind(entry.fee)
        .bind(entry.slippage)
        .bind(entry.is_reconstructed)
        .bind(&entry.reconstruction_source)
        .bind(entry.match_score)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Check if a ledger entry exists for the given position_id
    pub async fn has_btc_ledger_entry(&self, position_id: i64) -> Result<bool> {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT 1 FROM btc_trade_ledger WHERE position_id = ?1 LIMIT 1"
        )
        .bind(position_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.is_some())
    }

    /// Update settlement fields on an existing ledger entry (identified by position_id).
    pub async fn update_btc_trade_ledger_settlement(
        &self,
        position_id: i64,
        closed_at: &str,
        result: &str,
        pnl: f64,
        settlement_value: f64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE btc_trade_ledger SET closed_at = ?1, result = ?2, pnl = ?3, settlement_value = ?4
             WHERE position_id = ?5"
        )
        .bind(closed_at)
        .bind(result)
        .bind(pnl)
        .bind(settlement_value)
        .bind(position_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Query BTC trade ledger entries, ordered by opened_at DESC.
    pub async fn get_btc_trade_ledger(&self, limit: i64) -> Result<Vec<BtcTradeLedger>> {
        sqlx::query_as::<_, BtcTradeLedger>(
            "SELECT * FROM btc_trade_ledger WHERE wallet_id = ?1 ORDER BY opened_at DESC LIMIT ?2"
        )
        .bind(&self.wallet_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(WalletError::DatabaseError)
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn wallet_id(&self) -> &str {
        &self.wallet_id
    }
}

#[derive(Debug, sqlx::FromRow)]
struct TableInfoRow {
    name: String,
}

/// Parameters for opening a new position
pub struct OpenPositionParams {
    pub market_id: String,
    pub market_question: String,
    pub token_id: String,
    pub direction: String,
    pub entry_price: f64,
    pub size: f64,
    pub fee: f64,
    pub slippage: f64,
    pub category: String,
    pub target_date: Option<String>,
    pub threshold_f: Option<f64>,
    pub city_name: Option<String>,
    pub metric: Option<String>,
    pub event_slug: Option<String>,
    pub window_end: Option<String>,
    pub btc_price: Option<f64>,
}

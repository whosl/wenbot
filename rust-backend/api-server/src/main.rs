#!/usr/bin/env cargo
//! Wenbot API Server binary

use api_server::run;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "api_server=debug,tower_http=debug".into())
        )
        .init();

    let frontend_dir = std::env::var("FRONTEND_DIR")
        .unwrap_or_else(|_| "../frontend/dist".to_string());
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8000".to_string())
        .parse()
        .unwrap_or(8000);
    let db_url = std::env::var("VIRTUAL_WALLET_DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:virtual_wallet.db".to_string());

    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("setup-polymarket") {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await?;
        api_server::srp_auth::setup_polymarket_interactive(&pool).await?;
        return Ok(());
    }

    run(frontend_dir, port, &db_url).await
}

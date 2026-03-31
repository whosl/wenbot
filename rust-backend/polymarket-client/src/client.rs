//! Main Polymarket CLOB client

use crate::auth::L1Signature;
use crate::error::{ClientError, Result};
use crate::types::*;

/// The main Polymarket CLOB client
pub struct PolymarketClient {
    http: reqwest::Client,
    host: String,
    chain_id: u64,
    signer: L1Signature,
}

impl PolymarketClient {
    /// Create a new client from configuration
    pub fn new(config: crate::ClientConfig) -> Result<Self> {
        let mut builder = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("wenbot-rust/0.1.0");

        if let Some(proxy) = &config.proxy {
            let proxy_config = reqwest::Proxy::all(proxy)
                .map_err(|e| ClientError::Configuration(format!("Invalid proxy URL '{}': {}", proxy, e)))?;
            builder = builder.proxy(proxy_config);
        }

        let http = builder
            .build()
            .map_err(|e| ClientError::Configuration(format!("Failed to build HTTP client: {}", e)))?;

        Ok(Self {
            http,
            host: config.host.trim_end_matches('/').to_string(),
            chain_id: config.chain_id,
            signer: L1Signature::new(&config.credentials),
        })
    }

    /// Create client from environment variables
    pub fn from_env() -> anyhow::Result<Self> {
        let config = crate::ClientConfig::from_env()?;
        Self::new(config).map_err(Into::into)
    }

    // ─── Market Info ───

    /// Fetch markets from Gamma API (polymarket.com's public market data)
    pub async fn fetch_markets(
        &self,
        limit: Option<u32>,
        offset: Option<u32>,
        active: Option<bool>,
    ) -> Result<Vec<Market>> {
        let mut params = vec![];
        if let Some(l) = limit { params.push(("limit", l.to_string())); }
        if let Some(o) = offset { params.push(("offset", o.to_string())); }
        if let Some(a) = active { params.push(("active", a.to_string())); }

        let url = "https://gamma-api.polymarket.com/markets";
        let resp = self
            .http
            .get(url)
            .query(&params)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(ClientError::ApiError {
                status: resp.status().as_u16(),
                body: resp.text().await.unwrap_or_default(),
            });
        }

        resp.json().await.map_err(|e| ClientError::ParseError(e.to_string()))
    }

    /// Fetch a specific market by slug
    pub async fn fetch_market_by_slug(&self, slug: &str) -> Result<Market> {
        let url = format!("https://gamma-api.polymarket.com/markets/slug/{slug}");
        let resp = self.http.get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(ClientError::ApiError {
                status: resp.status().as_u16(),
                body: resp.text().await.unwrap_or_default(),
            });
        }

        resp.json().await.map_err(|e| ClientError::ParseError(e.to_string()))
    }

    /// Fetch price for a specific token
    pub async fn fetch_token_price(&self, token_id: &str) -> Result<f64> {
        let url = format!("{}/price", self.host);
        let params = [("token_id", token_id), ("side", "buy")];

        let resp = self.http.get(&url).query(&params).send().await?;

        if !resp.status().is_success() {
            return Err(ClientError::ApiError {
                status: resp.status().as_u16(),
                body: resp.text().await.unwrap_or_default(),
            });
        }

        let text = resp.text().await?;
        text.parse::<f64>()
            .map_err(|_| ClientError::ParseError(format!("Invalid price: {}", text)))
    }

    /// Fetch fee rate for a token
    pub async fn fetch_fee_rate(&self, token_id: &str) -> Result<FeeRateResponse> {
        let url = format!("{}/fee-rate", self.host);
        let resp = self
            .http
            .get(&url)
            .query(&[("token_id", token_id)])
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(ClientError::ApiError {
                status: resp.status().as_u16(),
                body: resp.text().await.unwrap_or_default(),
            });
        }

        resp.json().await.map_err(|e| ClientError::ParseError(e.to_string()))
    }

    // ─── Order Management ───

    /// Place an order (L1 signed)
    pub async fn place_order(&self, order: &Order) -> Result<OrderResponse> {
        let path = "/order";
        let body = serde_json::to_string(order)?;
        let headers = self.signer.sign_request("POST", path, &body);

        let url = format!("{}{}", self.host, path);
        let mut req = self.http.post(&url);
        for (k, v) in &headers.headers {
            req = req.header(k.as_str(), v.as_str());
        }

        let resp = req.body(body).send().await?;

        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = resp
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(1000);
            return Err(ClientError::RateLimited { retry_after_ms: retry_after });
        }

        if !resp.status().is_success() {
            return Err(ClientError::ApiError {
                status: resp.status().as_u16(),
                body: resp.text().await.unwrap_or_default(),
            });
        }

        resp.json().await.map_err(|e| ClientError::ParseError(e.to_string()))
    }

    /// Cancel an order by ID
    pub async fn cancel_order(&self, order_id: &str) -> Result<()> {
        let path = format!("/order/{}", order_id);
        let headers = self.signer.sign_request("DELETE", &path, "");

        let url = format!("{}{}", self.host, path);
        let mut req = self.http.delete(&url);
        for (k, v) in &headers.headers {
            req = req.header(k.as_str(), v.as_str());
        }

        let resp = req.send().await?;

        if !resp.status().is_success() {
            return Err(ClientError::ApiError {
                status: resp.status().as_u16(),
                body: resp.text().await.unwrap_or_default(),
            });
        }

        Ok(())
    }

    /// Cancel all open orders
    pub async fn cancel_all_orders(&self) -> Result<()> {
        let path = "/orders";
        let headers = self.signer.sign_request("DELETE", path, "");

        let url = format!("{}{}", self.host, path);
        let mut req = self.http.delete(&url);
        for (k, v) in &headers.headers {
            req = req.header(k.as_str(), v.as_str());
        }

        let resp = req.send().await?;

        if !resp.status().is_success() {
            return Err(ClientError::ApiError {
                status: resp.status().as_u16(),
                body: resp.text().await.unwrap_or_default(),
            });
        }

        Ok(())
    }

    // ─── Balance & Positions ───

    /// Get USDC balance and allowance
    pub async fn get_balance(&self) -> Result<BalanceInfo> {
        let path = "/balance-allowance";
        let params = [("asset_type", "COLLATERAL")];
        let headers = self.signer.sign_request("GET", path, "");

        let url = format!("{}{}", self.host, path);
        let mut req = self.http.get(&url).query(&params);
        for (k, v) in &headers.headers {
            req = req.header(k.as_str(), v.as_str());
        }

        let resp = req.send().await?;

        if !resp.status().is_success() {
            return Err(ClientError::ApiError {
                status: resp.status().as_u16(),
                body: resp.text().await.unwrap_or_default(),
            });
        }

        let data: serde_json::Value = resp.json().await?;
        Ok(BalanceInfo {
            usdc_balance: data["balance"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0)
                / 1_000_000.0,
            usdc_allowance: data["allowance"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0)
                / 1_000_000.0,
        })
    }

    /// Get open orders
    pub async fn get_open_orders(&self) -> Result<Vec<serde_json::Value>> {
        let path = "/orders";
        let headers = self.signer.sign_request("GET", path, "");

        let url = format!("{}{}", self.host, path);
        let mut req = self.http.get(&url);
        for (k, v) in &headers.headers {
            req = req.header(k.as_str(), v.as_str());
        }

        let resp = req.send().await?;

        if !resp.status().is_success() {
            return Err(ClientError::ApiError {
                status: resp.status().as_u16(),
                body: resp.text().await.unwrap_or_default(),
            });
        }

        resp.json().await.map_err(|e| ClientError::ParseError(e.to_string()))
    }

    // ─── Orderbook ───

    /// Fetch orderbook for a token
    pub async fn fetch_orderbook(&self, token_id: &str) -> Result<Orderbook> {
        let path = "/book";
        let url = format!("{}{}", self.host, path);

        let resp = self
            .http
            .get(&url)
            .query(&[("token_id", token_id)])
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(ClientError::ApiError {
                status: resp.status().as_u16(),
                body: resp.text().await.unwrap_or_default(),
            });
        }

        let data: serde_json::Value = resp.json().await?;

        let bids: Vec<OrderbookSide> = data["bids"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|b| Some(OrderbookSide {
                        price: b["price"].as_str()?.parse().ok()?,
                        size: b["size"].as_str()?.parse().ok()?,
                    }))
                    .collect()
            })
            .unwrap_or_default();

        let asks: Vec<OrderbookSide> = data["asks"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|a| Some(OrderbookSide {
                        price: a["price"].as_str()?.parse().ok()?,
                        size: a["size"].as_str()?.parse().ok()?,
                    }))
                    .collect()
            })
            .unwrap_or_default();

        let best_bid = bids.first().map(|b| b.price);
        let best_ask = asks.first().map(|a| a.price);
        let spread = match (best_bid, best_ask) {
            (Some(b), Some(a)) => Some(a - b),
            _ => None,
        };

        Ok(Orderbook {
            market: data["market"].as_str().unwrap_or("").to_string(),
            asset_id: token_id.to_string(),
            bids,
            asks,
            best_bid,
            best_ask,
            spread,
        })
    }

    /// Get the chain ID
    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }
}

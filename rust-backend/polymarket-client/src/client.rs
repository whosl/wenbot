//! Main Polymarket CLOB client

use crate::auth::L1Signature;
use crate::error::{ClientError, Result};
use crate::types::*;

#[derive(Clone)]
pub struct PolymarketClient {
    http: reqwest::Client,
    host: String,
    chain_id: u64,
    signer: L1Signature,
    private_key: Option<String>,
    /// Proxy wallet (funder) address for POLY_PROXY signatures
    funder_address: Option<String>,
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
            signer: L1Signature::new(&config.credentials, &config.address),
            private_key: config.private_key,
            funder_address: config.funder_address,
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

    /// Place a signed order (EIP-712) on the CLOB
    pub async fn place_signed_order(&self, order: &crate::types::SignedOrder) -> Result<OrderResponse> {
        let path = "/order";
        let api_key = self.signer.api_key();
        let wrapper = serde_json::json!({
            "order": order,
            "orderType": "GTC",
            "owner": api_key,
            "postOnly": false,
        });
        let body = serde_json::to_string(&wrapper)?;
        // Use compact JSON (no whitespace) to match Python SDK's separators=(",", ":")
        let body = serde_json::to_string(&wrapper)?;
        tracing::info!("📤 POLY request body (first 500): {}", &body[..body.len().min(500)]);
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

    /// Build and sign an EIP-712 order for a BUY
    ///
    /// `maker_address` should be the proxy wallet address (funder) for POLY_PROXY signatures.
    /// Falls back to EOA if None (but for MetaMask+proxy, pass the proxy address).
    /// `neg_risk` should be true for neg-risk markets.
    pub fn build_buy_order(
        &self,
        token_id: &str,
        price: f64,
        size: f64,
        maker_address: Option<&str>,
        neg_risk: bool,
    ) -> Option<crate::types::SignedOrder> {
        let pk = match self.private_key.as_ref() {
            Some(k) => k,
            None => {
                tracing::warn!("build_buy_order: no private key configured");
                return None;
            }
        };

        // Parse token_id (can be up to 77 decimal digits for Polymarket)
        let token_id_big: num_bigint::BigUint = match token_id.parse() {
            Ok(v) => v,
            Err(_) => {
                tracing::warn!("build_buy_order: failed to parse token_id '{}' (len={})", token_id, token_id.len());
                return None;
            }
        };

        // BUY order amounts (matching Python SDK `get_order_amounts` for BUY):
        //   size = number of shares, price = cost per share in USD
        //   raw_taker_amt = round_down(size, 2)   (shares, with 2 decimal precision)
        //   raw_maker_amt = raw_taker_amt * price   (dollars to spend)
        // Then convert to token decimals (* 10^6)
        //
        // Python: to_token_decimals(x) = int(round_normal(x * 10^6, 0))
        let raw_taker_amt = round_down_2(size);         // shares
        let raw_maker_amt = raw_taker_amt * price;      // dollars
        let maker_amount = (raw_maker_amt * 1_000_000.0).round() as u64;
        let taker_amount = (raw_taker_amt * 1_000_000.0).round() as u64;

        let salt_val = crate::eip712::generate_salt();

        use num_bigint::BigUint;

        // Determine maker address and signature type based on whether funder is set
        // - If funder (proxy wallet) is set: maker = funder, signatureType = 1 (POLY_PROXY)
        // - If no funder (pure EOA): maker = signer (EOA address), signatureType = 0 (EOA)
        let has_funder = maker_address.is_some() || self.funder_address.is_some();
        let maker_addr = if has_funder {
            maker_address
                .map(|s| s.to_string())
                .or_else(|| self.funder_address.clone())
                .unwrap()
        } else {
            self.signer.address()
        };
        let signature_type = if has_funder {
            crate::eip712::SIG_POLY_PROXY  // 1
        } else {
            crate::eip712::SIG_EOA  // 0
        };

        let fields = crate::eip712::OrderFields {
            salt: salt_val,
            maker: maker_addr,
            signer: self.signer.address(),
            taker: "0x0000000000000000000000000000000000000000".to_string(),
            token_id: token_id_big,
            maker_amount: BigUint::from(maker_amount),
            taker_amount: BigUint::from(taker_amount),
            expiration: BigUint::from(0u64),
            nonce: BigUint::from(0u64),
            fee_rate_bps: BigUint::from(0u64),
            side: 0, // BUY
            signature_type,
        };

        Some(crate::eip712::build_signed_order(&fields, pk, neg_risk))
    }

    /// Place an order (legacy, simple JSON - may not work with Polymarket)
    pub async fn place_order(&self, order: &crate::types::Order) -> Result<OrderResponse> {
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
        // Polymarket requires signature_type query parameter for EOA wallets
        let sig_type = if self.funder_address.is_some() { "1" } else { "0" };
        let params = [("asset_type", "COLLATERAL"), ("signature_type", sig_type)];
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

    /// Check if private key is configured for EIP-712 signing
    pub fn has_private_key(&self) -> bool {
        self.private_key.is_some()
    }

    /// Get the funder (proxy wallet) address
    pub fn funder_address(&self) -> Option<&str> {
        self.funder_address.as_deref()
    }

    /// Get the signer address
    pub fn address(&self) -> String {
        self.signer.address()
    }
}

/// Round down to 2 significant decimal places (matching Python SDK round_down(x, 2))
fn round_down_2(x: f64) -> f64 {
    (x * 100.0).floor() / 100.0
}

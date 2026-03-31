//! Fee calculation — Polymarket official fee curve
//!
//! Fee curve: `fee_param * price^2 * (1 - price)^2`
//! Where fee_param is fetched from the Polymarket `/fee-rate` endpoint.
//! Default fee_param = 0.25 for crypto markets.
//! If base_fee=0, the market is fee-free.

use tracing::{debug, info};

/// Fee service for calculating Polymarket trading fees
pub struct FeeService;

impl FeeService {
    /// Calculate the Polymarket taker fee for a given entry price.
    ///
    /// # Arguments
    /// * `entry_price` - Token price at entry (0-1)
    /// * `fee_param` - Fee parameter (typically 0.25 for crypto)
    ///
    /// # Returns
    /// Fee in dollars (per dollar traded)
    pub fn calculate_fee(entry_price: f64, fee_param: f64) -> f64 {
        fee_param * entry_price.powi(2) * (1.0 - entry_price).powi(2)
    }

    /// Fetch the fee parameter from Polymarket's fee-rate API.
    ///
    /// Returns the fee_param to use in calculate_fee().
    /// If the API returns base_fee=0, returns 0.0 (fee-free market).
    pub async fn fetch_fee_param(token_id: &str) -> Option<f64> {
        let url = format!(
            "https://clob.polymarket.com/fee-rate?token_id={}",
            token_id
        );

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .ok()?;

        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(data) => {
                        let base_fee: f64 = data.get("base_fee").and_then(|v| v.as_f64())?;

                        if base_fee == 0.0 {
                            info!("Polymarket fee-rate: base_fee=0 (fee-free market)");
                            return Some(0.0);
                        }

                        let fee_param = base_fee / 4000.0;
                        info!(
                            "Polymarket fee-rate: base_fee={} bps → fee_param={:.4}",
                            base_fee, fee_param
                        );
                        Some(fee_param)
                    }
                    Err(e) => {
                        debug!("Failed to parse fee-rate response: {}", e);
                        None
                    }
                }
            }
            Ok(resp) => {
                debug!("Polymarket fee-rate API returned {}", resp.status());
                None
            }
            Err(e) => {
                debug!("Polymarket fee-rate API unavailable: {}", e);
                None
            }
        }
    }

    /// Calculate fee with automatic API lookup.
    ///
    /// Tries to fetch fee_param from API. Falls back to default if unavailable.
    pub async fn calculate_fee_with_api(
        token_id: &str,
        entry_price: f64,
        default_fee_param: f64,
    ) -> f64 {
        let fee_param = Self::fetch_fee_param(token_id)
            .await
            .unwrap_or(default_fee_param);
        Self::calculate_fee(entry_price, fee_param)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fee_at_50_cents() {
        // At 0.50: 0.25 * 0.25 * 0.25 = 0.015625
        let fee = FeeService::calculate_fee(0.50, 0.25);
        assert!((fee - 0.015625).abs() < 1e-10);
    }

    #[test]
    fn test_fee_at_extremes() {
        // Near 0 or 1, fee approaches 0
        assert!(FeeService::calculate_fee(0.01, 0.25) < 0.0001);
        assert!(FeeService::calculate_fee(0.99, 0.25) < 0.0001);
    }

    #[test]
    fn test_fee_free_market() {
        // fee_param=0 means no fee
        let fee = FeeService::calculate_fee(0.50, 0.0);
        assert!(fee.abs() < 1e-10);
    }

    #[test]
    fn test_fee_max_at_50_cents() {
        // Fee curve peaks at p=0.5
        let fee_50 = FeeService::calculate_fee(0.50, 0.25);
        let fee_30 = FeeService::calculate_fee(0.30, 0.25);
        let fee_70 = FeeService::calculate_fee(0.70, 0.25);
        assert!(fee_50 > fee_30);
        assert!(fee_50 > fee_70);
    }
}

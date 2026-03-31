//! Valuation helpers — marked-to-market pricing and PnL

use crate::manager::PositionSummary;
use crate::models::VirtualPosition;

/// Calculate the marked-to-market value of a position
pub fn mark_to_market(position: &VirtualPosition, latest_price: Option<f64>) -> f64 {
    match latest_price {
        Some(price) => position.quantity * price,
        None => position.size,
    }
}

/// Calculate unrealized PnL for an open position
pub fn unrealized_pnl(position: &VirtualPosition, current_value: f64) -> f64 {
    if position.status != "open" {
        return position.pnl.unwrap_or(0.0);
    }
    current_value - position.size - position.fee
}

/// MTM from a PositionSummary (used by routes that enrich with external prices).
pub fn mark_to_market_from_summary(pos: &PositionSummary, latest_price: Option<f64>) -> f64 {
    match latest_price {
        Some(price) => pos.quantity * price,
        None => pos.current_value,
    }
}

/// Unrealized PnL from a PositionSummary.
pub fn unrealized_pnl_from_summary(pos: &PositionSummary, current_value: f64) -> f64 {
    if pos.status != "open" {
        return 0.0;
    }
    current_value - pos.size - pos.fee
}

/// Get position direction from string
pub fn effective_price_for_direction(position: &VirtualPosition) -> f64 {
    // effective_entry takes priority (accounts for slippage)
    position.effective_entry.unwrap_or(position.entry_price)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_position() -> VirtualPosition {
        VirtualPosition {
            id: 1,
            wallet_id: "wallet1".into(),
            market_id: "test".into(),
            market_question: "Test".into(),
            token_id: "token123".into(),
            direction: "YES".into(),
            entry_price: 0.50,
            effective_entry: Some(0.51),
            size: 10.0,
            quantity: 19.61,
            fee: 0.15,
            slippage: 0.01,
            category: "btc".into(),
            target_date: None,
            threshold_f: None,
            city_name: None,
            metric: None,
            event_slug: None,
            window_end: None,
            btc_price: None,
            status: "open".into(),
            created_at: "2026-01-01T00:00:00".into(),
            settled_at: None,
            settlement_value: None,
            actual_temperature: None,
            pnl: None,
        }
    }

    #[test]
    fn test_mark_to_market_with_price() {
        let pos = mock_position();
        let value = mark_to_market(&pos, Some(0.60));
        assert!((value - 19.61 * 0.60).abs() < 1e-10);
    }

    #[test]
    fn test_mark_to_market_without_price() {
        let pos = mock_position();
        let value = mark_to_market(&pos, None);
        assert!((value - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_unrealized_pnl() {
        let pos = mock_position();
        let value = 19.61 * 0.60;
        let pnl = unrealized_pnl(&pos, value);
        // pnl = current_value - size - fee = 11.766 - 10.0 - 0.15 = 1.616
        assert!((pnl - 1.616).abs() < 0.01);
    }
}

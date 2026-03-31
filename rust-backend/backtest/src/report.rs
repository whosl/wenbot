use crate::engine::{BacktestReport, BucketStats};

pub fn render_report(report: &BacktestReport, show_trades: bool) -> String {
    let wins = report.trades.iter().filter(|t| t.won).count();
    let losses = report.trades.len().saturating_sub(wins);
    let mut out = String::new();
    out.push_str(&format!(
        "=== Backtest: {} 15m, {}, {} ~ {} ===\n\n",
        report.symbol, report.profile, report.from, report.to
    ));
    out.push_str(&format!("Signals generated: {}\n", report.filter_stats.generated));
    out.push_str(&format!(
        "Trades taken:      {} ({:.1}% of signals)\n",
        report.filter_stats.taken,
        pct(report.filter_stats.taken, report.filter_stats.generated)
    ));
    out.push_str(&format!("  - Entry price filtered: {:>4}\n", report.filter_stats.entry_price_filtered));
    out.push_str(&format!("  - Edge filtered:        {:>4}\n", report.filter_stats.edge_filtered));
    out.push_str(&format!("  - Confidence filtered:  {:>4}\n", report.filter_stats.confidence_filtered));
    out.push_str(&format!("  - Convergence filtered: {:>4}\n", report.filter_stats.convergence_filtered));
    if report.filter_stats.time_filtered > 0 {
        out.push_str(&format!("  - Time filtered:        {:>4}\n", report.filter_stats.time_filtered));
    }

    out.push_str("\nResults:\n");
    out.push_str(&format!("  Wins:     {} ({:.1}%)\n", wins, report.win_rate * 100.0));
    out.push_str(&format!("  Losses:   {} ({:.1}%)\n", losses, 100.0 - report.win_rate * 100.0));
    out.push_str(&format!("  Total PnL: {:+.2}\n", report.total_pnl));
    out.push_str(&format!("  Avg PnL/trade: {:+.2}\n", report.avg_pnl));
    out.push_str(&format!("  Avg Edge: {:+.3}\n", report.avg_edge));
    out.push_str(&format!("  Avg Confidence: {:.1}%\n", report.avg_confidence * 100.0));
    out.push_str(&format!("  Profit Factor: {:.2}\n", report.profit_factor));
    out.push_str(&format!("  Max Drawdown: {:+.2}\n", report.max_drawdown));
    out.push_str(&format!("  Sharpe (annualized): {:.2}\n", report.sharpe));
    out.push_str(&format!("  15m candles loaded: {}\n", report.candle_count));

    out.push_str("\nBy Hour (Beijing):\n");
    for bucket in &report.by_hour {
        out.push_str(&format_bucket(bucket));
    }

    out.push_str("\nBy Entry Price:\n");
    for bucket in &report.by_entry_price {
        out.push_str(&format_bucket(bucket));
    }

    if show_trades {
        out.push_str("\nTrades:\n");
        for trade in &report.trades {
            out.push_str(&format!(
                "  {} | {:>4} | entry {:.3} | model {:.3} | market {:.3} | edge {:+.3} | conf {:.1}% | size {:.2} | pnl {:+.2}\n",
                trade.timestamp_bj.format("%Y-%m-%d %H:%M"),
                trade.direction,
                trade.entry_price,
                trade.model_prob,
                trade.market_prob,
                trade.edge,
                trade.confidence * 100.0,
                trade.size,
                trade.pnl,
            ));
        }
    }

    out
}

fn format_bucket(bucket: &BucketStats) -> String {
    let win_rate = if bucket.trades == 0 { 0.0 } else { bucket.wins as f64 / bucket.trades as f64 * 100.0 };
    let avg = if bucket.trades == 0 { 0.0 } else { bucket.pnl / bucket.trades as f64 };
    format!(
        "  {}: {} trades, {:.1}% win, {:+.2}, avg {:+.2}\n",
        bucket.label, bucket.trades, win_rate, bucket.pnl, avg
    )
}

fn pct(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 / total as f64 * 100.0
    }
}

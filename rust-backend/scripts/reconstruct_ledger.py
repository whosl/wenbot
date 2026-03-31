#!/usr/bin/env python3
"""
BTC Trade Ledger Reconstruction Script

Reconstructs missing signal fields for historical BTC trades by:
1. Parsing window epoch from market_slug to infer opened_at
2. Fetching historical market data from Polymarket API to get market_probability
3. Computing effective_direction from direction and entry_price
4. Writing reconstructed fields back to the ledger

Matching strategy:
- market_slug: parse epoch timestamp → derive opened_at (window start)
- direction YES → effective_direction depends on which token was bought
  - YES direction, entry_price <= 0.5 → effective_direction = "up" (bought UP token cheap)
  - NO direction, entry_price <= 0.5 → effective_direction = "down" (bought DOWN token cheap)
  - YES direction, entry_price > 0.5 → contrarian bet (bought UP token expensive)  
  - NO direction, entry_price > 0.5 → contrarian bet (bought DOWN token expensive)
- market_probability: inferred from entry_price
  - If direction=YES, market_probability = up_price (entry_price ≈ up_price)
  - If direction=NO, market_probability = 1 - entry_price (entry_price ≈ down_price)
"""

import sqlite3
import json
import sys
import os
import requests
from datetime import datetime, timezone, timedelta

DB_PATH = os.path.expanduser("~/wenbot/rust-backend/virtual_wallet.db")
REPORT_PATH = os.path.expanduser("~/wenbot/rust-backend/reports/btc-trade-ledger.md")

# PT timezone offset for market question display
ET_OFFSET = timedelta(hours=-4)  # EDT in March
UTC_OFFSET = timedelta(hours=0)


def parse_window_epoch(market_slug: str) -> int | None:
    """Extract window start epoch from market_slug like 'btc-updown-15m-1774682100'"""
    parts = market_slug.rsplit("-", 1)
    if len(parts) == 2:
        try:
            return int(parts[1])
        except ValueError:
            return None
    return None


def epoch_to_rfc3339(epoch: int) -> str:
    """Convert UTC epoch to RFC3339 string"""
    dt = datetime.fromtimestamp(epoch, tz=timezone.utc)
    return dt.strftime("%Y-%m-%dT%H:%M:%S.000000+00:00")


def epoch_to_human(epoch: int) -> str:
    """Convert UTC epoch to human readable string (Asia/Shanghai)"""
    dt = datetime.fromtimestamp(epoch, tz=timezone(timedelta(hours=8)))
    return dt.strftime("%Y-%m-%d %H:%M:%S")


def fetch_market_data_from_polymarket(slug: str) -> dict | None:
    """Fetch market data from Polymarket API to get historical prices"""
    try:
        client = requests.Session()
        client.trust_env = False  # bypass proxy env vars since Clash TUN handles routing
        
        resp = client.get(
            "https://gamma-api.polymarket.com/events",
            params={"slug": slug},
            timeout=15
        )
        if resp.status_code != 200:
            print(f"  ⚠ Polymarket API returned {resp.status_code} for {slug}")
            return None
        
        data = resp.json()
        events = data.get("data", data) if isinstance(data, dict) else data
        if not isinstance(events, list):
            events = [events]
        
        for event in events:
            markets = event.get("markets", [])
            for market in markets:
                market_slug = market.get("slug", "")
                if market_slug != slug:
                    continue
                
                # Parse outcome prices
                outcome_str = market.get("outcomePrices", "[]")
                if isinstance(outcome_str, str):
                    prices = json.loads(outcome_str)
                else:
                    prices = outcome_str
                
                if len(prices) >= 2:
                    up_price = float(prices[0]) if not isinstance(prices[0], (int, float)) else prices[0]
                    down_price = float(prices[1]) if not isinstance(prices[1], (int, float)) else prices[1]
                else:
                    continue
                
                # Parse outcomes to determine which is UP
                outcomes_str = market.get("outcomes", "[]")
                if isinstance(outcomes_str, str):
                    outcomes = json.loads(outcomes_str)
                else:
                    outcomes = outcomes_str
                
                up_idx = 0
                if len(outcomes) >= 2:
                    if outcomes[0].lower() == "down":
                        up_idx = 1
                
                return {
                    "up_price": float(prices[up_idx]),
                    "down_price": float(prices[1 - up_idx]),
                    "question": market.get("question", ""),
                }
        
        print(f"  ⚠ No market found in Polymarket for {slug}")
        return None
    except Exception as e:
        print(f"  ✗ Error fetching {slug}: {e}")
        return None


def infer_effective_direction(direction: str, entry_price: float) -> str:
    """
    Infer the effective_direction (up/down) from the trade direction and entry_price.
    
    If you bought YES (UP token), effective_direction is "up"
    If you bought NO (DOWN token), effective_direction is "down"
    
    The entry_price tells you how expensive the token was at the time.
    """
    if direction.upper() == "YES":
        return "up"
    else:
        return "down"


def compute_market_probability(direction: str, entry_price: float, poly_data: dict | None) -> float:
    """
    Infer market_probability (UP token implied probability) from trade data.
    
    We do NOT use poly_data for market_probability because Polymarket returns
    resolved prices (0.0 or 1.0) for expired markets, not the prices at trade time.
    
    - If direction=YES, we bought the UP token at entry_price, so up_price ≈ entry_price
    - If direction=NO, we bought the DOWN token at entry_price, so down_price ≈ entry_price
      and up_price ≈ 1 - entry_price
    """
    if direction.upper() == "YES":
        return entry_price
    else:
        return 1.0 - entry_price


def compute_match_score(has_opened_at: bool, has_market_data: bool, entry_price_reasonable: bool) -> float:
    """Score how confident we are in the reconstruction (0.0 - 1.0)"""
    score = 0.0
    if has_opened_at:
        score += 0.4
    if has_market_data:
        score += 0.4
    if entry_price_reasonable:
        score += 0.2
    return round(score, 2)


def main():
    os.makedirs(os.path.dirname(REPORT_PATH), exist_ok=True)
    
    print("=" * 60)
    print("BTC Trade Ledger Reconstruction")
    print("=" * 60)
    
    conn = sqlite3.connect(DB_PATH)
    conn.row_factory = sqlite3.Row
    cursor = conn.cursor()
    
    # First, add new columns if they don't exist
    new_columns = [
        ("reconstruction_source", "TEXT DEFAULT NULL"),
        ("match_score", "REAL DEFAULT NULL"),
    ]
    
    for col_name, col_def in new_columns:
        try:
            cursor.execute(f"ALTER TABLE btc_trade_ledger ADD COLUMN {col_name} {col_def}")
            print(f"✓ Added column: {col_name}")
        except sqlite3.OperationalError as e:
            if "duplicate column name" in str(e):
                pass  # Column already exists
            else:
                raise
    
    conn.commit()
    
    # Fetch all reconstructed trades
    cursor.execute("""
        SELECT id, trade_id, position_id, market_slug, direction, opened_at, closed_at,
               entry_price, size, edge, confidence, model_probability, market_probability,
               reasoning, indicator_scores, asset_price, result, pnl, is_reconstructed
        FROM btc_trade_ledger
        WHERE is_reconstructed = 1
        ORDER BY id ASC
    """)
    
    rows = cursor.fetchall()
    print(f"\nFound {len(rows)} reconstructed trades to enhance\n")
    
    matched = 0
    unmatched = 0
    results = []
    
    for row in rows:
        trade_id = row["trade_id"]
        market_slug = row["market_slug"]
        direction = row["direction"]
        entry_price = row["entry_price"]
        current_opened_at = row["opened_at"]
        current_edge = row["edge"]
        current_confidence = row["confidence"]
        
        print(f"Processing {trade_id} | {market_slug} | {direction} | entry={entry_price:.3f}")
        
        # Step 1: Parse window epoch from market_slug
        window_epoch = parse_window_epoch(market_slug)
        if window_epoch is None:
            print(f"  ✗ Cannot parse epoch from {market_slug}")
            unmatched += 1
            results.append((row, None, "epoch_parse_fail", 0.0))
            continue
        
        inferred_opened_at = epoch_to_rfc3339(window_epoch)
        has_opened_at = bool(current_opened_at and current_opened_at.strip())
        
        # Step 2: Fetch Polymarket data for this market
        poly_data = fetch_market_data_from_polymarket(market_slug)
        
        # Step 3: Compute reconstructed fields
        effective_direction = infer_effective_direction(direction, entry_price)
        market_prob = compute_market_probability(direction, entry_price, poly_data)
        market_prob = float(market_prob) if market_prob is not None else 0.0
        
        # Entry price reasonable check (should be between 0.05 and 0.95)
        entry_reasonable = 0.05 < entry_price < 0.95
        
        match_score = compute_match_score(has_opened_at or bool(window_epoch), 
                                          poly_data is not None, 
                                          entry_reasonable)
        
        # Step 4: Build update
        updates = {}
        source_tags = []
        
        if not has_opened_at and window_epoch:
            updates["opened_at"] = inferred_opened_at
            source_tags.append("epoch_inferred")
            print(f"  ✓ Set opened_at: {epoch_to_human(window_epoch)}")
        
        if effective_direction:
            updates["effective_direction"] = effective_direction
            print(f"  ✓ effective_direction: {effective_direction}")
        
        if market_prob > 0:
            updates["market_probability"] = market_prob
            source_tags.append("poly_market_data" if poly_data else "price_inferred")
            print(f"  ✓ market_probability: {market_prob:.4f}")
        
        if poly_data:
            updates["reasoning"] = f"Reconstructed | {market_slug} | entry_price={entry_price:.3f} → market_prob={market_prob:.3f} | market verified on Polymarket (resolved: up={poly_data['up_price']:.0f})"
            source_tags.append("polymarket_fetch")
        else:
            updates["reasoning"] = f"Reconstructed | {market_slug} | entry_price={entry_price:.3f} → market_prob={market_prob:.3f} (from entry_price, no Polymarket verification)"
        
        if source_tags:
            updates["reconstruction_source"] = ",".join(source_tags)
        
        updates["match_score"] = match_score
        
        # Update the DB
        if updates:
            set_clauses = []
            values = []
            for k, v in updates.items():
                set_clauses.append(f"{k} = ?")
                values.append(v)
            
            values.append(row["id"])
            cursor.execute(
                f"UPDATE btc_trade_ledger SET {', '.join(set_clauses)} WHERE id = ?",
                values
            )
        
        matched += 1
        results.append((row, poly_data, updates.get("reconstruction_source", ""), match_score))
        print(f"  ✓ match_score: {match_score}")
        print()
    
    conn.commit()
    
    # Also check non-reconstructed trades for missing opened_at
    cursor.execute("""
        SELECT id, trade_id, market_slug, opened_at
        FROM btc_trade_ledger
        WHERE (is_reconstructed = 0 OR is_reconstructed IS NULL)
          AND (opened_at IS NULL OR opened_at = '')
        ORDER BY id ASC
    """)
    non_recon_missing = cursor.fetchall()
    for row in non_recon_missing:
        window_epoch = parse_window_epoch(row["market_slug"])
        if window_epoch:
            inferred = epoch_to_rfc3339(window_epoch)
            cursor.execute("UPDATE btc_trade_ledger SET opened_at = ? WHERE id = ?", (inferred, row["id"]))
            print(f"  ✓ Fixed opened_at for non-reconstructed {row['trade_id']}: {epoch_to_human(window_epoch)}")
    
    conn.commit()
    
    # ─── Part C: Generate report ───
    print("\n" + "=" * 60)
    print("Generating report...")
    print("=" * 60)
    
    cursor.execute("""
        SELECT id, trade_id, position_id, market_slug, direction, predicted_direction,
               effective_direction, opened_at, closed_at, entry_price, quantity, size,
               edge, confidence, model_probability, market_probability, suggested_size,
               reasoning, indicator_scores, asset_price, result, pnl, settlement_value,
               fee, slippage, is_reconstructed, reconstruction_source, match_score
        FROM btc_trade_ledger
        ORDER BY id ASC
    """)
    
    all_trades = cursor.fetchall()
    
    report_lines = []
    report_lines.append("# BTC Trade Ledger Report")
    report_lines.append(f"")
    report_lines.append(f"**Generated:** {datetime.now().strftime('%Y-%m-%d %H:%M:%S')} (Asia/Shanghai)")
    report_lines.append(f"**Total trades:** {len(all_trades)}")
    report_lines.append(f"**Reconstructed:** {sum(1 for t in all_trades if t['is_reconstructed'])}")
    report_lines.append(f"**Signal-captured:** {sum(1 for t in all_trades if not t['is_reconstructed'])}")
    report_lines.append(f"")
    
    # Summary stats
    settled = [t for t in all_trades if t['result']]
    wins = [t for t in settled if t['result'] == 'win']
    losses = [t for t in settled if t['result'] == 'loss']
    total_pnl = sum(t['pnl'] or 0 for t in settled)
    
    report_lines.append("## Summary")
    report_lines.append(f"")
    report_lines.append(f"| Metric | Value |")
    report_lines.append(f"|--------|-------|")
    report_lines.append(f"| Total Trades | {len(all_trades)} |")
    report_lines.append(f"| Settled | {len(settled)} |")
    report_lines.append(f"| Open | {len(all_trades) - len(settled)} |")
    report_lines.append(f"| Wins | {len(wins)} |")
    report_lines.append(f"| Losses | {len(losses)} |")
    report_lines.append(f"| Win Rate | {len(wins)/len(settled)*100:.1f}% |" if settled else "| Win Rate | N/A |")
    report_lines.append(f"| Total PnL | ${total_pnl:.2f} |")
    report_lines.append(f"| Avg PnL | ${total_pnl/len(settled):.2f} |" if settled else "| Avg PnL | N/A |")
    report_lines.append(f"")
    
    # Per-trade detail
    report_lines.append("## Trade Details")
    report_lines.append(f"")
    report_lines.append(f"| # | ID | Slug | Dir | Eff Dir | Opened | Closed | Entry | Size | PnL | Result | Edge | Conf | MP | Recon | Score |")
    report_lines.append(f"|---|-----|------|-----|---------|--------|--------|-------|------|-----|--------|------|------|----|-------|-------|")
    
    for t in all_trades:
        # Format opened_at
        opened = t['opened_at'] or "?"
        if opened != "?" and "T" in opened:
            try:
                dt = datetime.fromisoformat(opened.replace("+00:00", "+08:00").replace("+00:00", ""))
                # Handle various formats
                if opened.endswith("+00:00"):
                    dt = datetime.fromisoformat(opened[:-6]).replace(tzinfo=timezone.utc)
                    dt = dt.astimezone(timezone(timedelta(hours=8)))
                opened = dt.strftime("%m-%d %H:%M")
            except:
                opened = opened[:16]
        else:
            opened = opened[:16] if len(opened) > 16 else opened
        
        # Format closed_at
        closed = t['closed_at'] or ""
        if closed and "T" in closed:
            try:
                if closed.endswith("+00:00"):
                    dt = datetime.fromisoformat(closed[:-6]).replace(tzinfo=timezone.utc)
                    dt = dt.astimezone(timezone(timedelta(hours=8)))
                closed = dt.strftime("%m-%d %H:%M")
            except:
                closed = closed[:16]
        elif closed:
            closed = closed[:16]
        
        # Short slug
        slug = t['market_slug'].replace("btc-updown-15m-", "")
        
        # PnL
        pnl_str = f"${t['pnl']:.2f}" if t['pnl'] is not None else "—"
        
        # Result
        result = t['result'] or "—"
        
        # Edge
        edge = f"{t['edge']*100:.1f}%" if t['edge'] else "—"
        
        # Confidence
        conf = f"{t['confidence']:.0%}" if t['confidence'] else "—"
        
        # Market probability
        mp = f"{t['market_probability']:.3f}" if t['market_probability'] else "—"
        
        # Reconstruction
        recon = "Yes" if t['is_reconstructed'] else "No"
        score = f"{t['match_score']:.1f}" if t['match_score'] is not None else "—"
        
        report_lines.append(
            f"| {t['id']} | {t['trade_id']} | {slug} | {t['direction']} | {t['effective_direction'] or '—'} | "
            f"{opened} | {closed} | {t['entry_price']:.3f} | ${t['size']:.2f} | {pnl_str} | "
            f"{result} | {edge} | {conf} | {mp} | {recon} | {score} |"
        )
    
    report_lines.append("")
    
    # Reasoning details
    report_lines.append("## Signal Context (Reasoning)")
    report_lines.append("")
    
    for t in all_trades:
        if t['reasoning']:
            reasoning = t['reasoning'][:200]
            report_lines.append(f"### {t['trade_id']} ({t['market_slug']})")
            report_lines.append(f"")
            report_lines.append(f"- **Direction:** {t['direction']} → {t['effective_direction'] or '?'}")
            report_lines.append(f"- **Edge:** {t['edge']}")
            report_lines.append(f"- **Confidence:** {t['confidence']}")
            report_lines.append(f"- **Model Prob:** {t['model_probability']}")
            report_lines.append(f"- **Market Prob:** {t['market_probability']}")
            report_lines.append(f"- **Reconstruction:** {'Yes' if t['is_reconstructed'] else 'No'} ({t['reconstruction_source'] or 'signal_capture'})")
            report_lines.append(f"- **Match Score:** {t['match_score'] or 'N/A'}")
            report_lines.append(f"- **Reasoning:** {reasoning}{'...' if len(t['reasoning']) > 200 else ''}")
            
            # Indicator scores
            if t['indicator_scores'] and t['indicator_scores'] != '{}':
                try:
                    scores = json.loads(t['indicator_scores']) if isinstance(t['indicator_scores'], str) else t['indicator_scores']
                    if scores:
                        report_lines.append(f"- **Indicators:** RSI={scores.get('rsi_signal', '?'):.2f} Mom={scores.get('momentum_signal', '?'):.2f} VWAP={scores.get('vwap_signal', '?'):.2f} SMA={scores.get('sma_signal', '?'):.2f} Skew={scores.get('market_skew', '?'):.2f}")
                except:
                    pass
            
            report_lines.append("")
    
    # Write report
    with open(REPORT_PATH, "w") as f:
        f.write("\n".join(report_lines))
    
    print(f"\n✓ Report written to: {REPORT_PATH}")
    print(f"\n{'=' * 60}")
    print(f"RESULTS:")
    print(f"{'=' * 60}")
    print(f"  Matched/enhanced:  {matched}")
    print(f"  Unmatched:         {unmatched}")
    print(f"  Report:            {REPORT_PATH}")
    print(f"  New columns:       reconstruction_source, match_score")
    print(f"{'=' * 60}")
    
    conn.close()


if __name__ == "__main__":
    main()

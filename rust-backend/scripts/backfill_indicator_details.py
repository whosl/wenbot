#!/usr/bin/env python3
import json
import re
import sqlite3
import subprocess
from collections import defaultdict
from pathlib import Path

DB_PATH = Path('/home/wenzhuolin/wenbot/rust-backend/virtual_wallet.db')
LOG_LINES = 5000
NEUTRAL_EPSILON = 0.05

PROFILE_WEIGHTS = {
    'classic_four': {
        'rsi': 0.25,
        'momentum': 0.25,
        'vwap': 0.20,
        'sma': 0.15,
        'market_skew': 0.0,
        'volume_trend': 0.0,
        'bollinger': 0.0,
        'volatility': 0.0,
    },
    'btc_eight_indicator': {
        'rsi': 0.15,
        'momentum': 0.20,
        'vwap': 0.15,
        'sma': 0.10,
        'market_skew': 0.10,
        'volume_trend': 0.10,
        'bollinger': 0.10,
        'volatility': 0.10,
    },
}

INDICATOR_KEYS = [
    ('rsi', 'rsi_signal'),
    ('momentum', 'momentum_signal'),
    ('vwap', 'vwap_signal'),
    ('sma', 'sma_signal'),
    ('market_skew', 'market_skew'),
    ('volume_trend', 'volume_trend_signal'),
    ('bollinger', 'bollinger_signal'),
    ('volatility', 'volatility_signal'),
]

LOG_PATTERN = re.compile(
    r'(?P<ts>\d{4}-\d{2}-\d{2}T[^ ]+) .*?profile=(?P<profile>[a-z_]+) market=(?P<market>[^ ]+) '\
    r'\| RSI=(?P<rsi>[-+]?\d+(?:\.\d+)?) Mom=(?P<momentum>[-+]?\d+(?:\.\d+)?) '\
    r'VWAP=(?P<vwap>[-+]?\d+(?:\.\d+)?) SMA=(?P<sma>[-+]?\d+(?:\.\d+)?) '\
    r'Skew=(?P<market_skew>[-+]?\d+(?:\.\d+)?) VolTr=(?P<volume_trend>[-+]?\d+(?:\.\d+)?) '\
    r'Boll=(?P<bollinger>[-+]?\d+(?:\.\d+)?) VolReg=(?P<volatility>[-+]?\d+(?:\.\d+)?) '\
)


def ensure_column(cur, table, column, decl):
    cur.execute(f'PRAGMA table_info({table})')
    columns = {row[1] for row in cur.fetchall()}
    if column not in columns:
        cur.execute(f'ALTER TABLE {table} ADD COLUMN {column} {decl}')


def normalized_score(signal: float) -> float:
    return max(0.0, min(1.0, (max(-1.0, min(1.0, signal)) + 1.0) / 2.0))


def vote(signal: float) -> str:
    if signal > NEUTRAL_EPSILON:
        return 'up'
    if signal < -NEUTRAL_EPSILON:
        return 'down'
    return 'neutral'


def build_details(profile: str, scores: dict) -> str | None:
    weights = PROFILE_WEIGHTS.get(profile)
    if not weights:
        return None
    details = {}
    for public_key, score_key in INDICATOR_KEYS:
        raw = scores.get(score_key, 0.0)
        raw = float(raw)
        details[public_key] = {
            'vote': vote(raw),
            'score': normalized_score(raw),
            'weight': weights[public_key],
        }
    return json.dumps(details, separators=(',', ':'))


def profile_from_reasoning(reasoning: str, wallet_id: str | None = None) -> str | None:
    m = re.search(r'profile=([a-z_]+)', reasoning or '')
    if m:
        return m.group(1)
    if wallet_id == 'wallet3':
        return 'btc_eight_indicator'
    return 'classic_four'


def load_log_candidates() -> dict[str, list[str]]:
    cmd = f"pm2 logs wenbot-rust-api --lines {LOG_LINES} --nostream"
    proc = subprocess.run(cmd, shell=True, capture_output=True, text=True)
    output = (proc.stdout or '') + '\n' + (proc.stderr or '')
    candidates: dict[str, list[str]] = defaultdict(list)
    for line in output.splitlines():
        m = LOG_PATTERN.search(line)
        if not m:
            continue
        profile = m.group('profile')
        score_map = {
            'rsi_signal': float(m.group('rsi')),
            'momentum_signal': float(m.group('momentum')),
            'vwap_signal': float(m.group('vwap')),
            'sma_signal': float(m.group('sma')),
            'market_skew': float(m.group('market_skew')),
            'volume_trend_signal': float(m.group('volume_trend')),
            'bollinger_signal': float(m.group('bollinger')),
            'volatility_signal': float(m.group('volatility')),
        }
        details = build_details(profile, score_map)
        if details:
            candidates[m.group('market')].append(details)
    return candidates


def main():
    conn = sqlite3.connect(DB_PATH)
    cur = conn.cursor()

    ensure_column(cur, 'trade_history', 'indicator_details', 'TEXT')
    ensure_column(cur, 'btc_trade_ledger', 'indicator_details', 'TEXT')
    conn.commit()

    log_candidates = load_log_candidates()

    updated_ledger = 0
    updated_from_logs = 0

    cur.execute(
        """
        SELECT id, wallet_id, market_slug, reasoning, indicator_scores
        FROM btc_trade_ledger
        WHERE indicator_details IS NULL OR TRIM(indicator_details) = ''
        ORDER BY id ASC
        """
    )
    for row_id, wallet_id, market_slug, reasoning, indicator_scores in cur.fetchall():
        details = None
        profile = profile_from_reasoning(reasoning, wallet_id)
        if indicator_scores and indicator_scores != '{}':
            try:
                details = build_details(profile, json.loads(indicator_scores)) if profile else None
            except Exception:
                details = None
        if details is None and log_candidates.get(market_slug):
            details = log_candidates[market_slug][-1]
            updated_from_logs += 1
        if details:
            cur.execute('UPDATE btc_trade_ledger SET indicator_details = ? WHERE id = ?', (details, row_id))
            updated_ledger += 1

    cur.execute(
        """
        UPDATE trade_history
        SET indicator_details = (
            SELECT b.indicator_details
            FROM btc_trade_ledger b
            WHERE b.wallet_id = trade_history.wallet_id
              AND b.position_id = trade_history.position_id
              AND b.indicator_details IS NOT NULL
              AND TRIM(b.indicator_details) <> ''
            ORDER BY b.id DESC
            LIMIT 1
        )
        WHERE indicator_details IS NULL OR TRIM(indicator_details) = ''
        """
    )
    updated_history = cur.rowcount if cur.rowcount != -1 else 0

    conn.commit()

    cur.execute("SELECT COUNT(*) FROM btc_trade_ledger WHERE indicator_details IS NOT NULL AND TRIM(indicator_details) <> ''")
    ledger_filled = cur.fetchone()[0]
    cur.execute("SELECT COUNT(*) FROM trade_history WHERE indicator_details IS NOT NULL AND TRIM(indicator_details) <> ''")
    history_filled = cur.fetchone()[0]

    print(json.dumps({
        'updated_ledger': updated_ledger,
        'updated_from_logs': updated_from_logs,
        'updated_trade_history': updated_history,
        'ledger_with_indicator_details': ledger_filled,
        'trade_history_with_indicator_details': history_filled,
        'log_markets_seen': len(log_candidates),
    }, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()

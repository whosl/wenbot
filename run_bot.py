#!/usr/bin/env python3
"""Weather bot CLI.

示例:
- python run_bot.py --scan       # 扫描市场 + 生成信号（dry-run）
- python run_bot.py --positions  # 查看余额 / 持仓 / 挂单
- python run_bot.py --live       # 实盘扫描 + 下单
"""
from __future__ import annotations

import argparse
import asyncio
import logging
from typing import Iterable

from backend.core.weather_signals import scan_for_weather_signals
from backend.data.clob_client import get_balance, get_open_orders, get_positions, place_order
from backend.models.database import init_db

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s | %(levelname)s | %(name)s | %(message)s",
)
logger = logging.getLogger("weather_cli")


def _print_signal(signal, index: int) -> None:
    entry_price = signal.market.yes_price if signal.direction == "yes" else signal.market.no_price
    print(
        f"[{index}] {signal.market.city_name:<14} | {signal.market.metric:>4} {signal.market.direction:<5} {signal.market.threshold_f:>5.1f}F"
        f" | model={signal.model_probability:>6.1%} market={signal.market_probability:>6.1%} edge={signal.edge:>+6.1%}"
        f" | action={signal.direction.upper():<3} @ {entry_price:.2f} size=${signal.suggested_size:.2f}"
    )


def _print_jsonish_list(title: str, rows: Iterable) -> None:
    rows = list(rows)
    print(f"\n== {title} ({len(rows)}) ==")
    for row in rows[:20]:
        print(row)
    if len(rows) > 20:
        print(f"... and {len(rows) - 20} more")


async def run_scan() -> int:
    """扫描天气市场并输出信号，不下单。"""
    signals = await scan_for_weather_signals()
    actionable = [signal for signal in signals if signal.passes_threshold]

    print(f"\n扫描完成：共 {len(signals)} 个信号，可交易 {len(actionable)} 个\n")

    for idx, signal in enumerate(signals[:20], start=1):
        _print_signal(signal, idx)

    if actionable:
        print("\nTop actionable:")
        for idx, signal in enumerate(actionable[:10], start=1):
            _print_signal(signal, idx)

    return 0


async def run_positions() -> int:
    """查看余额、持仓和挂单。"""
    balance = get_balance()
    positions = get_positions()
    open_orders = get_open_orders()

    print("== Balance ==")
    print(balance)
    _print_jsonish_list("Positions", positions)
    _print_jsonish_list("Open Orders", open_orders)
    return 0


async def run_live() -> int:
    """扫描后按 actionable signals 直接下单。"""
    signals = await scan_for_weather_signals()
    actionable = [signal for signal in signals if signal.passes_threshold]

    if not actionable:
        print("没有可交易信号，未下单。")
        return 0

    placed = 0
    for signal in actionable:
        token_id = signal.market.yes_token_id if signal.direction == "yes" else signal.market.no_token_id
        entry_price = signal.market.yes_price if signal.direction == "yes" else signal.market.no_price

        if not token_id:
            logger.warning("跳过 %s：缺少 token_id", signal.market.title)
            continue
        if entry_price <= 0:
            logger.warning("跳过 %s：价格无效 %s", signal.market.title, entry_price)
            continue

        # suggested_size 是美元风险敞口；CLOB 下单 size 需要 shares
        shares = round(max(signal.suggested_size / entry_price, 1.0), 4)

        try:
            resp = place_order(
                token_id=token_id,
                side="BUY",
                price=entry_price,
                size=shares,
            )
            placed += 1
            print(
                f"已下单: {signal.market.city_name} {signal.direction.upper()} token={token_id} "
                f"price={entry_price:.2f} shares={shares} resp={resp}"
            )
        except Exception as e:
            logger.error("下单失败 %s: %s", signal.market.title, e)

    print(f"\n实盘结束：成功提交 {placed} 笔订单。")
    return 0 if placed else 1


async def amain() -> int:
    parser = argparse.ArgumentParser(description="Polymarket wenbot CLI")
    parser.add_argument("--scan", action="store_true", help="扫描天气市场并生成 dry-run 信号")
    parser.add_argument("--positions", action="store_true", help="查看余额、持仓和挂单")
    parser.add_argument("--live", action="store_true", help="扫描并实盘下单")
    args = parser.parse_args()

    init_db()

    selected = sum([args.scan, args.positions, args.live])
    if selected != 1:
        parser.error("必须且只能选择一个参数：--scan / --positions / --live")

    if args.scan:
        return await run_scan()
    if args.positions:
        return await run_positions()
    return await run_live()


if __name__ == "__main__":
    raise SystemExit(asyncio.run(amain()))

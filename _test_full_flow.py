"""全流程测试：扫描信号 → 虚拟钱包执行 → 查看持仓和历史"""
import asyncio
import logging
import time

logging.basicConfig(level=logging.WARNING, format="%(asctime)s %(levelname)s %(message)s")


async def main():
    start = time.time()

    # 1. 扫描市场 + 生成信号
    print("=" * 60)
    print("STEP 1: 扫描市场 + 生成信号")
    print("=" * 60)

    from backend.core.weather_signals import scan_for_weather_signals

    t0 = time.time()
    signals = await scan_for_weather_signals()
    t1 = time.time()

    actionable = [s for s in signals if s.passes_threshold]
    print(f"耗时: {t1-t0:.1f}s")
    print(f"总信号: {len(signals)}, 可交易: {len(actionable)}")

    if not actionable:
        print("没有可交易信号。")
        print(f"总耗时: {time.time()-start:.1f}s")
        return

    actionable.sort(key=lambda s: abs(s.edge), reverse=True)
    for s in actionable:
        m = s.market
        nws = ""
        if s.nws_forecast is not None:
            tag = "✓" if s.nws_agrees else "✗"
            nws = f" | NWS {s.nws_forecast:.0f}F {tag}"
        print(f"  {m.city_name} {m.metric} {m.threshold_f}F {m.direction} | {m.target_date} | edge={s.edge:+.1%} | conf={s.confidence:.0%} | dir={s.direction} | size=${s.suggested_size:.2f}{nws}")

    # 2. 执行交易（虚拟钱包）
    print()
    print("=" * 60)
    print("STEP 2: 执行交易（虚拟钱包）")
    print("=" * 60)

    from virtual_wallet import get_interface
    wallet = get_interface()

    balance_before = wallet.get_balance()
    print(f"执行前余额: ${balance_before.get('balance', '?')} | 总权益: ${balance_before.get('total_value', '?')}")

    executed = 0
    for s in actionable[:5]:  # 只执行 top 5
        m = s.market
        t2 = time.time()
        try:
            # 选择 token_id 和 price
            if s.direction == "yes":
                token_id = m.yes_token_id or ""
                entry_price = m.yes_price
            else:
                token_id = m.no_token_id or ""
                entry_price = m.no_price

            result = wallet.trade(
                market_id=m.market_id,
                market_question=m.title,
                token_id=token_id,
                direction=s.direction.upper(),
                entry_price=entry_price,
                size=s.suggested_size,
                target_date=m.target_date,
                threshold_f=m.threshold_f,
                city_name=m.city_name,
                metric=m.metric,
            )
            t3 = time.time()
            status = result.get("status", "unknown")
            if status in ("opened", "success", "executed"):
                executed += 1
                print(f"  ✅ {m.city_name} {m.metric} {m.threshold_f}F {s.direction} | ${s.suggested_size:.2f} @ {entry_price:.2f} | {t3-t2:.2f}s")
            else:
                msg = result.get("error", result.get("message", str(result)))
                print(f"  ❌ {m.city_name} {m.metric} {m.threshold_f}F {s.direction} | {msg[:80]} | {t3-t2:.2f}s")
        except Exception as e:
            print(f"  ❌ {m.city_name} {m.metric} {m.threshold_f}F {s.direction} | {e}")

    balance_after = wallet.get_balance()
    print(f"执行后余额: ${balance_after.get('balance', '?')} | 总权益: ${balance_after.get('total_value', '?')}")
    print(f"成功执行: {executed}/{min(5, len(actionable))}")

    # 3. 查看持仓
    print()
    print("=" * 60)
    print("STEP 3: 当前持仓")
    print("=" * 60)

    positions = wallet.get_positions(status="open")
    print(f"持仓数: {len(positions)}")
    for p in positions[:10]:
        pnl = p.get("unrealized_pnl")
        pnl_str = f"${pnl:.2f}" if pnl is not None else "N/A"
        price = p.get("current_price")
        print(f"  {p.get('market_id', '?')[:30]} | dir={p.get('direction')} | qty={p.get('quantity', '?')} | price={price} | pnl={pnl_str}")

    # 4. 查看历史
    print()
    print("=" * 60)
    print("STEP 4: 交易历史")
    print("=" * 60)

    history = wallet.get_history(limit=10)
    print(f"历史记录: {len(history)}")
    for h in history[:10]:
        pnl = h.get("pnl", 0) or 0
        print(f"  {h.get('type')} | {str(h.get('market_id', '?'))[:30]} | dir={h.get('direction')} | amount=${h.get('amount', 0):.2f} | pnl=${pnl:.2f}")

    print()
    print(f"总耗时: {time.time()-start:.1f}s")
    print("DONE")


asyncio.run(main())

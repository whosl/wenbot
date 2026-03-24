"""Polymarket CLOB API client helpers."""
from __future__ import annotations

import logging
import os
from typing import Any, Dict, List, Optional

import httpx
from py_clob_client.client import ClobClient
from py_clob_client.clob_types import (
    ApiCreds,
    AssetType,
    BalanceAllowanceParams,
    OpenOrderParams,
    OrderArgs,
    OrderType,
)

from backend.config import settings

logger = logging.getLogger("trading_bot")


# Monkey-patch py_clob_client's httpx client to use system proxy (Clash TUN).
# The library uses `httpx.Client(http2=True)` without proxy/trust_env,
# which breaks when going through a proxy.
def _patch_clob_http_client():
    """Replace py_clob_client's default httpx.Client with one that respects proxy env vars."""
    try:
        import py_clob_client.http_helpers.helpers as helpers
        proxy = os.environ.get("https_proxy") or os.environ.get("HTTPS_PROXY") or os.environ.get("http_proxy") or os.environ.get("HTTP_PROXY")
        if proxy:
            helpers._http_client = httpx.Client(http2=True, proxy=proxy, verify=False)
            logger.info("Patched py_clob_client to use proxy: %s", proxy)
    except Exception as e:
        logger.warning("Failed to patch py_clob_client proxy: %s", e)


_patch_clob_http_client()

# 默认凭证。优先读取 config/env；为空时回退到这里，保证向后兼容。
DEFAULT_PRIVATE_KEY = "0x11dadaad5127c4e266342605c2c865ea475999fdd921df61b2fdc3a0d1c5beb6"
DEFAULT_API_KEY = "fbba7637-af92-257b-1826-a3d211cfba5e"
DEFAULT_API_SECRET = "yCS4WIvzjmsKdPcD0kzdEBhiXSYSloUQi-RvYwjZDFk="
DEFAULT_API_PASSPHRASE = "f02f26decc2cb5cbd065656ad62e570d80e05e779464ab019674c449b587f655"
DEFAULT_CHAIN_ID = 137
DEFAULT_HOST = "https://clob.polymarket.com"


def _get_clob_settings() -> dict[str, Any]:
    return {
        "private_key": settings.POLYMARKET_PRIVATE_KEY or DEFAULT_PRIVATE_KEY,
        "api_key": settings.POLYMARKET_API_KEY or DEFAULT_API_KEY,
        "api_secret": getattr(settings, "POLYMARKET_API_SECRET", None) or DEFAULT_API_SECRET,
        "api_passphrase": getattr(settings, "POLYMARKET_API_PASSPHRASE", None) or DEFAULT_API_PASSPHRASE,
        "chain_id": settings.POLYMARKET_CLOB_CHAIN_ID or DEFAULT_CHAIN_ID,
        "host": settings.POLYMARKET_CLOB_HOST or DEFAULT_HOST,
    }

_clob_client: Optional[ClobClient] = None


def get_clob_client() -> ClobClient:
    """返回已认证的 ClobClient 单例。"""
    global _clob_client
    if _clob_client is None:
        cfg = _get_clob_settings()
        client = ClobClient(
            host=cfg["host"],
            chain_id=cfg["chain_id"],
            key=cfg["private_key"],
        )
        client.set_api_creds(
            ApiCreds(
                cfg["api_key"],
                cfg["api_secret"],
                cfg["api_passphrase"],
            )
        )
        _clob_client = client
    return _clob_client


def get_balance() -> Dict[str, Any]:
    """查询 USDC 余额/allowance 信息。"""
    client = get_clob_client()
    try:
        params = BalanceAllowanceParams(asset_type=AssetType.COLLATERAL)
        return client.get_balance_allowance(params) or {}
    except Exception as e:
        logger.warning(f"Failed to fetch CLOB balance: {e}")
        return {"error": str(e)}


def _fetch_positions_via_http(address: str) -> Optional[List[Dict[str, Any]]]:
    """尝试从 Polymarket 数据接口查询持仓。

    不同部署环境下可用路径可能不同，这里按顺序尝试常见 endpoint。
    """
    candidate_urls = [
        "https://data-api.polymarket.com/positions",
        "https://data-api.polymarket.com/positions-value",
        "https://data-api.polymarket.com/trades",
    ]

    for url in candidate_urls:
        try:
            with httpx.Client(timeout=15.0) as http_client:
                response = http_client.get(url, params={"user": address})
                if response.status_code >= 400:
                    continue
                data = response.json()
                if isinstance(data, list):
                    return data
                if isinstance(data, dict):
                    for key in ("data", "positions", "items"):
                        value = data.get(key)
                        if isinstance(value, list):
                            return value
        except Exception:
            continue

    return None


def get_positions() -> List[Dict[str, Any]]:
    """查询当前钱包持仓。

    优先尝试 Polymarket 数据接口；若失败，则退回到用户成交记录，
    至少能让 CLI 看见近期持仓相关数据。
    """
    client = get_clob_client()

    try:
        address = client.get_address()
    except Exception:
        address = None

    if address:
        positions = _fetch_positions_via_http(address)
        if positions is not None:
            return positions

    try:
        trades = client.get_trades()
        if isinstance(trades, dict):
            return trades.get("data", trades.get("trades", [])) or []
        return trades or []
    except Exception as e:
        logger.warning(f"Failed to fetch CLOB positions: {e}")
        return []


def get_open_orders() -> List[Dict[str, Any]]:
    """查询当前挂单。"""
    client = get_clob_client()
    try:
        orders = client.get_orders(OpenOrderParams())
        if isinstance(orders, dict):
            return orders.get("data", orders.get("orders", [])) or []
        return orders or []
    except Exception as e:
        logger.warning(f"Failed to fetch CLOB open orders: {e}")
        return []


def place_order(token_id: str, side: str, price: float, size: float) -> Dict[str, Any]:
    """下限价 GTC 单。

    side 支持 BUY / SELL 或 yes / no 风格的上层映射结果。
    """
    client = get_clob_client()
    normalized_side = side.upper()
    if normalized_side not in {"BUY", "SELL"}:
        raise ValueError(f"Unsupported order side: {side}")

    order_args = OrderArgs(
        token_id=str(token_id),
        price=float(price),
        size=float(size),
        side=normalized_side,
    )

    try:
        return client.create_and_post_order(order_args, OrderType.GTC)
    except TypeError:
        # 某些 py-clob-client 版本要求显式 create_order + post_order
        signed_order = client.create_order(order_args)
        return client.post_order(signed_order, OrderType.GTC)


def cancel_order(order_id: str) -> Dict[str, Any]:
    """撤销单个订单。"""
    client = get_clob_client()
    try:
        return client.cancel(order_id)
    except Exception as e:
        logger.warning(f"Failed to cancel CLOB order {order_id}: {e}")
        return {"error": str(e), "order_id": order_id}

"""Observed weather settlement helpers for Wunderground / Open-Meteo."""
from __future__ import annotations

import json
import logging
import re
from datetime import date, datetime
from typing import Dict, Optional
from urllib.parse import quote

import httpx

from backend.data.weather import CITY_CONFIG

logger = logging.getLogger("trading_bot")


def _coerce_date(target_date: date | str | datetime) -> date:
    if isinstance(target_date, datetime):
        return target_date.date()
    if isinstance(target_date, str):
        return date.fromisoformat(target_date)
    return target_date


def _extract_temperature_pair_from_html(html: str) -> Optional[Dict[str, float]]:
    """从 Wunderground HTML 中尽量解析 high/low。

    页面结构常变，所以这里尽量做宽松匹配；如果失效就走 Open-Meteo fallback。
    """
    patterns = [
        r'"temperatureMax"\s*:\s*([\-0-9.]+).*?"temperatureMin"\s*:\s*([\-0-9.]+)',
        r'"maxTemperature"\s*:\s*([\-0-9.]+).*?"minTemperature"\s*:\s*([\-0-9.]+)',
        r'High\s*</span>\s*<span[^>]*>\s*([\-0-9.]+).*?Low\s*</span>\s*<span[^>]*>\s*([\-0-9.]+)',
    ]

    for pattern in patterns:
        match = re.search(pattern, html, flags=re.IGNORECASE | re.DOTALL)
        if match:
            try:
                return {"high": float(match.group(1)), "low": float(match.group(2))}
            except ValueError:
                continue

    # 有些页面会把 JSON-LD 塞进 script 标签
    for script_match in re.finditer(r'<script[^>]*>(.*?)</script>', html, flags=re.IGNORECASE | re.DOTALL):
        script_content = script_match.group(1)
        if "temperatureMax" not in script_content and "temperatureMin" not in script_content:
            continue
        try:
            for obj_match in re.finditer(r'\{.*?\}', script_content, flags=re.DOTALL):
                block = obj_match.group(0)
                if "temperatureMax" in block and "temperatureMin" in block:
                    normalized = block.replace("\n", " ")
                    match = re.search(r'"temperatureMax"\s*:\s*([\-0-9.]+).*?"temperatureMin"\s*:\s*([\-0-9.]+)', normalized)
                    if match:
                        return {"high": float(match.group(1)), "low": float(match.group(2))}
        except Exception:
            continue

    return None


async def _fetch_wunderground_page(city_name: str, target_day: date) -> Optional[str]:
    """抓取 Wunderground 历史页 HTML。"""
    city_slug = quote(city_name.lower().replace("_", "-").replace(" ", "-"))
    url_candidates = [
        f"https://www.wunderground.com/history/daily/{city_slug}/date/{target_day.isoformat()}",
        f"https://www.wunderground.com/history/daily/date/{target_day.isoformat()}/{city_slug}",
    ]

    headers = {
        "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123 Safari/537.36",
        "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        "Accept-Language": "en-US,en;q=0.9",
    }

    async with httpx.AsyncClient(timeout=20.0, follow_redirects=True) as client:
        for url in url_candidates:
            try:
                response = await client.get(url, headers=headers)
                if response.status_code == 200 and response.text:
                    return response.text
            except Exception as e:
                logger.debug(f"Wunderground fetch failed for {url}: {e}")

    return None


async def _fetch_open_meteo_archive(city_name: str, target_day: date) -> Optional[Dict[str, float]]:
    """用 Open-Meteo archive 兜底实测高低温。"""
    city_key = None
    for key, config in CITY_CONFIG.items():
        if config["name"].lower() == city_name.lower() or key == city_name:
            city_key = key
            break

    if not city_key:
        return None

    city = CITY_CONFIG[city_key]
    params = {
        "latitude": city["lat"],
        "longitude": city["lon"],
        "daily": "temperature_2m_max,temperature_2m_min",
        "temperature_unit": "fahrenheit",
        "start_date": target_day.isoformat(),
        "end_date": target_day.isoformat(),
        "timezone": "GMT",
    }

    try:
        async with httpx.AsyncClient(timeout=20.0) as client:
            response = await client.get("https://archive-api.open-meteo.com/v1/archive", params=params)
            response.raise_for_status()
            data = response.json()
            daily = data.get("daily", {})
            highs = daily.get("temperature_2m_max", []) or []
            lows = daily.get("temperature_2m_min", []) or []
            if highs and lows and highs[0] is not None and lows[0] is not None:
                return {"high": float(highs[0]), "low": float(lows[0])}
    except Exception as e:
        logger.warning(f"Open-Meteo archive fallback failed for {city_name}: {e}")

    return None


async def fetch_wunderground_observed(city_name: str, target_date: date | str | datetime) -> Optional[Dict[str, float]]:
    """获取城市指定日期的实测高低温（华氏度）。

    返回格式: {"high": float, "low": float}
    优先尝试 Wunderground，失败时退回 Open-Meteo archive。
    """
    target_day = _coerce_date(target_date)

    try:
        html = await _fetch_wunderground_page(city_name, target_day)
        if html:
            parsed = _extract_temperature_pair_from_html(html)
            if parsed:
                return parsed
    except Exception as e:
        logger.warning(f"Failed to fetch Wunderground observed temperature for {city_name}: {e}")

    return await _fetch_open_meteo_archive(city_name, target_day)

# 前端重写任务

## 目标
完全重写 wenbot 前端（React + TypeScript + Tailwind），确保**手机端适配**良好，功能与当前页面一致。

## 技术栈
- React 18 + TypeScript
- Tailwind CSS (已配置)
- Vite (已配置)
- @tanstack/react-query (已安装)
- framer-motion (已安装)
- recharts (已安装)

## 关键要求
1. **手机优先**：所有页面必须在 375px 宽度下正常显示，表格可横向滚动
2. **暗色主题**：黑色背景，类似 Bloomberg terminal 美学
3. **单页应用**：URL query 参数路由 (`?view=wallet`)
4. **API base**：`window.location.origin + '/api'`

## 后端 API 端点完整列表

### Health & Config
- `GET /api/health` → `{ status: string, version: string, backend: string }`
- `GET /api/bot/config` → 见下方 JSON
- `GET /api/bot/status` → `{ scheduler_running: boolean, mode: string, recent_events: any[] }`

### Fivesbot (BTC 15min)
- `GET /api/fivesbot/status` → 见下方 JSON
- `GET /api/fivesbot/signals` → `TradingSignal[]` 见下方

### Wallet 1 (BTC 策略)
- `GET /api/wallet/balance` → `{ balance, total_pnl, total_position_value, total_trades, total_value, win_rate, winning_trades }`
- `GET /api/wallet/positions` → `VirtualPosition[]`
- `GET /api/wallet/history?limit=N` → `TradeHistory[]`
- `POST /api/wallet/deposit?amount=N` → BalanceSummary
- `POST /api/wallet/sync-prices` → `{ updated: number }`
- `POST /api/wallet/settle` → `{ settled: number }`

### Weather (Wallet 3 天气策略)
- `GET /api/weather/signals` → `WeatherSignal[]`
- `GET /api/weather/forecasts` → 见下方 JSON
- `GET /api/weather/balance` → BalanceSummary (同 wallet balance 格式)
- `GET /api/weather/positions` → `VirtualPosition[]`
- `GET /api/weather/history?limit=N` → `TradeHistory[]`
- `POST /api/weather/deposit?amount=N` → BalanceSummary

## 数据结构

### /api/bot/config 响应
```json
{
  "btc": {
    "min_edge_threshold": 0.08,
    "max_entry_price": 0.45,
    "min_market_volume": 1000,
    "min_trade_size": 1.0,
    "max_trade_size": 10.0,
    "kelly_max_fraction": 0.05,
    "daily_loss_limit": 20.0,
    "min_time_remaining": 300,
    "max_time_remaining": 7200,
    "fee_rate": "polymarket_curve",
    "weights": { "rsi": 0.25, "momentum": 0.25, "vwap": 0.20, "sma": 0.15, "market_skew": 0.15 }
  },
  "weather": {
    "min_edge_threshold": 0.08,
    "max_entry_price": 0.45,
    "max_trade_size": 5.0,
    "fee_rate": "polymarket_curve",
    "kelly_max_fraction": 0.05,
    "kelly_fraction": 0.25,
    "kelly_lookback_trades": 20,
    "daily_loss_limit": 20.0,
    "total_exposure_cap_ratio": 2.0,
    "low_balance_threshold": 10.0,
    "scan_interval_secs": 300,
    "price_update_interval_secs": 180,
    "settlement_interval_secs": 3600,
    "dynamic_kelly": true,
    "dedup_enabled": true,
    "fee_param_fallback": 0.25,
    "weather_cities": ["New York", "Chicago", "Miami", "Los Angeles", "Austin", "Denver", "Seattle", "Atlanta", "Dallas", "Houston", "Phoenix", "Beijing"]
  }
}
```

### /api/fivesbot/status 响应
```json
{
  "wsConnected": false,
  "uptime": "2h 30m",
  "upPrice": 0.695,
  "downPrice": 0.305,
  "btcPrice": 66422.99,
  "currentCycle": "btc-updown-15m-1774682100 (07:30)",
  "lastUpdate": "2026-03-28T07:24:57Z",
  "markets": {
    "btc": {
      "predictor": {
        "signal": "DOWN",
        "confidence": 0.5,
        "direction": "down",
        "trend": 0.216,
        "momentum": 0.0096,
        "volatility": 0.001,
        "rsi": 63.6
      },
      "activeMarkets": [{
        "market_id": "1740525",
        "slug": "btc-updown-15m-1774682100",
        "question": "Bitcoin Up or Down - March 28, 3:15AM-3:30AM ET",
        "up_token_id": "...",
        "down_token_id": "...",
        "up_price": 0.695,
        "down_price": 0.305,
        "end_date": "2026-03-28T07:30:00Z",
        "event_slug": "btc-updown-15m-1774682100"
      }]
    }
  }
}
```

### VirtualPosition
```typescript
interface VirtualPosition {
  id: number;
  wallet_id: string;
  market_id: string;
  market_question: string;
  token_id: string;
  direction: "YES" | "NO";
  entry_price: number;
  effective_entry?: number;
  size: number;
  quantity: number;
  fee: number;
  slippage: number;
  category: "weather" | "btc";
  target_date?: string;
  threshold_f?: number;
  city_name?: string;
  metric?: "high" | "low";
  event_slug?: string;
  window_end?: string;
  btc_price?: number;
  status: "open" | "settled" | "canceled";
  created_at: string;
  settled_at?: string;
  settlement_value?: number;
  actual_temperature?: number;
  pnl?: number;
}
```

### TradeHistory
```typescript
interface TradeHistory {
  id: number;
  wallet_id: string;
  position_id: number;
  market_id: string;
  market_question: string;
  direction: "YES" | "NO";
  entry_price: number;
  size: number;
  quantity: number;
  settlement_value: number;
  actual_temperature?: number;
  pnl: number;
  result: "win" | "loss";
  opened_at: string;
  closed_at: string;
}
```

### WeatherSignal
```typescript
interface WeatherSignal {
  market_id: string;
  market_question: string;
  direction: "YES" | "NO";
  model_probability: number;
  market_probability: number;
  edge: number;
  confidence: number;
  suggested_size: number;
  buy_token_id: string;
  entry_price: number;
  passes_threshold: boolean;
  reasoning: string;
  ensemble_mean: number;
  ensemble_std: number;
  ensemble_members: number;
  nws_forecast?: number;
  nws_agrees?: boolean;
  city_key: string;
  city_name: string;
  target_date: string;
  metric: "high" | "low";
  threshold_f: number;
  timestamp: string;
}
```

### WeatherForecast
```typescript
interface WeatherForecast {
  city_key: string;
  city_name: string;
  target_date: string;
  market_question: string;
  market_slug: string;
  range_low: number;
  range_high: number;
  yes_price: number;
  no_price: number;
  mean_high: number;
  std_high: number;
  mean_low: number;
  std_low: number;
  num_members: number;
  ensemble_agreement: number;
}
```

### BalanceSummary
```typescript
interface BalanceSummary {
  balance: number;
  total_pnl: number;
  total_position_value: number;
  total_trades: number;
  total_value: number;
  win_rate: number;
  winning_trades: number;
}
```

### TradingSignal (fivesbot)
```typescript
interface TradingSignal {
  market_slug: string;
  market_question: string;
  direction: string;
  confidence: number;
  edge: number;
  entry_price: number;
  suggested_size: number;
  rsi: number;
  momentum: number;
  trend: number;
  volatility: number;
  timestamp: string;
}
```

## 页面结构

### 默认首页 (无 query 参数)
这是旧的 Trading Terminal 页面，简化为：
- Header：标题 "WENBOT"、BTC 价格、状态指示
- **Fivesbot Monitor**：BTC 实时状态（价格、信号、活跃市场窗口）
- **三个虚拟钱包概览卡片**：wallet1(BTC策略)、wallet2(预留)、wallet3(天气策略)
- **Bot Runtime 状态**：运行时间、模式

### ?view=wallet 页面
这是主要管理页面，包含 tabs：
- **Overview**：钱包概览（余额/PnL/trades/win rate 统计面板）、BTC Feed（信号/趋势/波动率）、虚拟钱包列表
- **Virtual Wallet**：选中的钱包详情（余额、持仓表、历史表、存款按钮）
- **Polymarket**：占位（暂时无数据）
- **Strategy**：完整策略配置（BTC + Weather 参数展示）
- **History**：交易历史表

### 操作按钮
- Sync Prices (POST /api/wallet/sync-prices)
- Settle Wallet (POST /api/wallet/settle)
- Deposit（POST /api/wallet/deposit?amount=N，弹出输入框）
- 刷新按钮（重新 fetch 数据）

## 注意事项
- 后端 serve 前端 dist：`~/wenbot/frontend/dist/`
- 构建命令：`cd ~/wenbot/frontend && npx vite build`
- 后端会自动 serve `index.html` 和 `dist/` 下的静态文件
- 所有 API 请求用 axios，baseURL 为 `window.location.origin + '/api'`
- `@tanstack/react-query` 管理数据获取和缓存
- refetchInterval 参考：balance 10s, positions 10s, history 15s, signals 30s, forecasts 30s, fivesbot-status 5s, bot-status 5s
- 数字用 tabular-nums 字体
- 颜色规范：正数绿色、负数红色、warning 黄色、BTC 琥珀色、天气 青色

import axios from 'axios'

const API_BASE = `${window.location.origin}/api`

const api = axios.create({
  baseURL: API_BASE,
  timeout: 15000,
})

// ===== Fivesbot (BTC 15min) =====

export interface FivesbotMarket {
  market_id: string
  slug: string
  question: string
  up_token_id: string
  down_token_id: string
  up_price: number
  down_price: number
  end_date: string
  event_slug: string
}

export interface FivesbotPredictor {
  signal: string
  confidence: number
  direction: string
  trend: number
  momentum: number
  volatility: number
  rsi: number
}

export interface FivesbotAssetMarkets {
  predictor: FivesbotPredictor
  activeMarkets: FivesbotMarket[]
}

export interface FivesbotStatus {
  wsConnected: boolean
  uptime: string
  upPrice: number
  downPrice: number
  btcPrice: number
  currentCycle: string
  lastUpdate: string
  markets: { btc: FivesbotAssetMarkets }
}

export interface FivesbotSignal {
  signal: string
  confidence: number
  direction: string
  edge: number
  rsi: number
  momentum: number
  vwap: number
  sma: number
  market_skew: number
  composite: number
  market_slug: string
  up_price: number
  down_price: number
  actionable: boolean
  conviction: number
  conviction_reason: string
  timestamp: string
}

export async function fetchFivesbotStatus(): Promise<FivesbotStatus> {
  const { data } = await api.get('/fivesbot/status')
  return data
}

export async function fetchFivesbotSignals(): Promise<FivesbotSignal[]> {
  const { data } = await api.get('/fivesbot/signals')
  return data
}

export interface EthFivesbotStatus {
  wsConnected: boolean
  uptime: string
  upPrice: number
  downPrice: number
  ethPrice: number
  currentCycle: string
  lastUpdate: string
  markets: { eth: FivesbotAssetMarkets }
}

export async function fetchEthFivesbotStatus(): Promise<EthFivesbotStatus> {
  const { data } = await api.get('/fivesbot/eth/status')
  return data
}

export async function fetchEthFivesbotSignals(): Promise<FivesbotSignal[]> {
  const { data } = await api.get('/fivesbot/eth/signals')
  return data
}

export async function fetchWallet3FivesbotStatus(): Promise<FivesbotStatus> {
  const { data } = await api.get('/fivesbot/wallet3/status')
  return data
}

export async function fetchWallet3FivesbotSignals(): Promise<FivesbotSignal[]> {
  const { data } = await api.get('/fivesbot/wallet3/signals')
  return data
}

export async function fetchWallet4FivesbotStatus(): Promise<FivesbotStatus> {
  const { data } = await api.get('/fivesbot/wallet4/status')
  return data
}

export async function fetchWallet4FivesbotSignals(): Promise<FivesbotSignal[]> {
  const { data } = await api.get('/fivesbot/wallet4/signals')
  return data
}

// ===== Fivesbot Trades (BTC Trade Ledger) =====

export interface FivesbotTrade {
  id: number
  trade_id: string
  wallet_id: string
  position_id: number
  market_slug: string
  market_question: string
  token_id: string
  direction: string
  predicted_direction: string
  effective_direction: string
  opened_at: string
  closed_at: string | null
  entry_price: number
  quantity: number
  size: number
  edge: number
  confidence: number
  model_probability: number
  market_probability: number
  suggested_size: number
  reasoning: string
  indicator_scores: string
  asset_price: number
  result: string | null
  pnl: number | null
  settlement_value: number | null
  fee: number
  slippage: number
  is_reconstructed: boolean
  reconstruction_source: string | null
  match_score: number | null
  created_at: string
}

export async function fetchFivesbotTrades(limit = 50): Promise<FivesbotTrade[]> {
  const { data } = await api.get('/fivesbot/trades', { params: { limit } })
  return data
}

export async function fetchEthFivesbotTrades(limit = 50): Promise<FivesbotTrade[]> {
  const { data } = await api.get('/fivesbot/eth/trades', { params: { limit } })
  return data
}

export async function fetchWallet3FivesbotTrades(limit = 50): Promise<FivesbotTrade[]> {
  const { data } = await api.get('/fivesbot/wallet3/trades', { params: { limit } })
  return data
}

export async function fetchWallet4FivesbotTrades(limit = 50): Promise<FivesbotTrade[]> {
  const { data } = await api.get('/fivesbot/wallet4/trades', { params: { limit } })
  return data
}

// ===== Bot Config & Status =====

export interface BotConfig {
  // Will be populated from actual response
  [key: string]: unknown
}

export interface BotStatus {
  [key: string]: unknown
}

export async function fetchBotConfig(): Promise<BotConfig> {
  const { data } = await api.get('/bot/config')
  return data
}

export async function fetchBotStatus(): Promise<BotStatus> {
  const { data } = await api.get('/bot/status')
  return data
}

// ===== Wallet (BTC, Wallet 1) =====

export interface WalletBalance {
  balance: number
  total_pnl: number
  total_position_value: number
  total_trades: number
  total_value: number
  win_rate: number
  winning_trades: number
}

export interface WalletPosition {
  id: number
  wallet_id?: string
  market_id: string
  market_question: string
  token_id?: string
  direction: string
  entry_price: number
  effective_entry?: number
  size: number
  quantity: number
  fee?: number
  slippage?: number
  current_price?: number | null
  current_value?: number | null
  unrealized_pnl?: number | null
  edge?: number | null
  confidence?: number | null
  category?: string
  target_date?: string | null
  threshold_f?: number | null
  city_name?: string | null
  metric?: string | null
  event_slug?: string
  window_end?: string
  btc_price?: number
  status?: string
  created_at?: string
  settled_at?: string | null
  settlement_value?: number | null
  actual_temperature?: number | null
  pnl?: number | null
}

export interface WalletHistory {
  id: number
  wallet_id: string
  position_id: number
  market_id: string
  market_question: string
  direction: string
  entry_price: number
  size: number
  quantity: number
  settlement_value: number
  actual_temperature: number | null
  pnl: number
  result: string
  opened_at: string
  closed_at: string
}

export async function fetchWalletBalance(): Promise<WalletBalance> {
  const { data } = await api.get('/wallet/balance')
  return data
}

export async function fetchWalletPositions(): Promise<WalletPosition[]> {
  const { data } = await api.get('/wallet/positions')
  return data
}

export async function fetchWalletHistory(limit = 100): Promise<WalletHistory[]> {
  const { data } = await api.get('/wallet/history', { params: { limit } })
  return data
}

export async function depositWallet(amount: number): Promise<unknown> {
  const { data } = await api.post('/wallet/deposit', null, { params: { amount } })
  return data
}

export async function syncPrices(): Promise<unknown> {
  const { data } = await api.post('/wallet/sync-prices')
  return data
}

export async function settleWallet(): Promise<unknown> {
  const { data } = await api.post('/wallet/settle')
  return data
}


export async function fetchWallet2Balance(): Promise<WalletBalance> {
  const { data } = await api.get('/wallet2/balance')
  return data
}

export async function fetchWallet2Positions(): Promise<WalletPosition[]> {
  const { data } = await api.get('/wallet2/positions')
  return data
}

export async function fetchWallet2History(limit = 100): Promise<WalletHistory[]> {
  const { data } = await api.get('/wallet2/history', { params: { limit } })
  return data
}

export async function depositWallet2(amount: number): Promise<unknown> {
  const { data } = await api.post('/wallet2/deposit', null, { params: { amount } })
  return data
}

export async function syncWallet2Prices(): Promise<unknown> {
  const { data } = await api.post('/wallet2/sync-prices')
  return data
}

export async function settleWallet2(): Promise<unknown> {
  const { data } = await api.post('/wallet2/settle')
  return data
}

// ===== Wallet 3 (BTC 8-indicator) =====

export async function fetchWallet3Balance(): Promise<WalletBalance> {
  const { data } = await api.get('/wallet3/balance')
  return data
}

export async function fetchWallet3Positions(): Promise<WalletPosition[]> {
  const { data } = await api.get('/wallet3/positions')
  return data
}

export async function fetchWallet3History(limit = 100): Promise<WalletHistory[]> {
  const { data } = await api.get('/wallet3/history', { params: { limit } })
  return data
}

export async function depositWallet3(amount: number): Promise<unknown> {
  const { data } = await api.post('/wallet3/deposit', null, { params: { amount } })
  return data
}

export async function syncWallet3Prices(): Promise<unknown> {
  const { data } = await api.post('/wallet3/sync-prices')
  return data
}

export async function settleWallet3(): Promise<unknown> {
  const { data } = await api.post('/wallet3/settle')
  return data
}

// ===== Wallet 4 (BTC 5min classic_four) =====

export async function fetchWallet4Balance(): Promise<WalletBalance> {
  const { data } = await api.get('/wallet4/balance')
  return data
}

export async function fetchWallet4Positions(): Promise<WalletPosition[]> {
  const { data } = await api.get('/wallet4/positions')
  return data
}

export async function fetchWallet4History(limit = 100): Promise<WalletHistory[]> {
  const { data } = await api.get('/wallet4/history', { params: { limit } })
  return data
}

export async function depositWallet4(amount: number): Promise<unknown> {
  const { data } = await api.post('/wallet4/deposit', null, { params: { amount } })
  return data
}

export async function syncWallet4Prices(): Promise<unknown> {
  const { data } = await api.post('/wallet4/sync-prices')
  return data
}

export async function settleWallet4(): Promise<unknown> {
  const { data } = await api.post('/wallet4/settle')
  return data
}

// ===== Weather (Wallet 5) =====

export interface WeatherBalance {
  balance: number
  total_pnl: number
  total_position_value: number
  total_trades: number
  total_value: number
  win_rate: number
  winning_trades: number
}

export interface WeatherPosition {
  id: number
  wallet_id: string
  market_id: string
  market_question: string
  token_id: string
  direction: string
  entry_price: number
  size: number
  quantity: number
  fee: number
  slippage: number
  category: string
  target_date: string | null
  threshold_f: number | null
  city_name: string | null
  metric: string | null
  event_slug: string
  window_end: string | null
  btc_price: number | null
  status: string
  created_at: string
  settled_at: string | null
  settlement_value: number | null
  actual_temperature: number | null
  pnl: number | null
  current_price?: number | null
  current_value?: number | null
  unrealized_pnl?: number | null
}

export interface WeatherHistory {
  id: number
  wallet_id: string
  position_id: number
  market_id: string
  market_question: string
  direction: string
  entry_price: number
  size: number
  quantity: number
  settlement_value: number
  actual_temperature: number | null
  pnl: number
  result: string
  opened_at: string
  closed_at: string
}

export interface WeatherSignalData {
  market_id: string
  market_question: string
  direction: string
  model_probability: number
  market_probability: number
  edge: number
  confidence: number
  suggested_size: number
  buy_token_id: string
  entry_price: number
  passes_threshold: boolean
  reasoning: string
  ensemble_mean: number
  ensemble_std: number
  ensemble_members: number
  nws_forecast: number
  nws_agrees: boolean
  city_key: string
  city_name: string
  target_date: string
  metric: string
  threshold_f: number
  timestamp: string
}

export interface WeatherForecastData {
  city_key: string
  city_name: string
  target_date: string
  market_question: string
  market_slug: string
  range_low: number
  range_high: number
  yes_price: number
  no_price: number
  mean_high: number
  std_high: number
  mean_low: number
  std_low: number
  num_members: number
  ensemble_agreement: number
}

export async function fetchWeatherBalance(): Promise<WeatherBalance> {
  const { data } = await api.get('/weather/balance')
  return data
}

export async function fetchWeatherPositions(): Promise<WeatherPosition[]> {
  const { data } = await api.get('/weather/positions')
  return data
}

export async function fetchWeatherHistory(limit = 100): Promise<WeatherHistory[]> {
  const { data } = await api.get('/weather/history', { params: { limit } })
  return data
}

export async function fetchWeatherSignals(): Promise<WeatherSignalData[]> {
  const { data } = await api.get('/weather/signals')
  return data
}

export async function fetchWeatherForecasts(): Promise<WeatherForecastData[]> {
  const { data } = await api.get('/weather/forecasts')
  return data
}

export async function depositWeather(amount: number): Promise<unknown> {
  const { data } = await api.post('/weather/deposit', null, { params: { amount } })
  return data
}

// ===== Wallet Connect (Polymarket SRP) =====

export interface WalletConnectionStatus {
  configured: boolean
  connected: boolean
}

export interface WalletConnectStartResponse {
  session_id: string
  salt: string
  B: string
  n_hex: string
  g_hex: string
}

export interface WalletConnectVerifyRequest {
  session_id: string
  A: string
  M1: string
  wrapped_file_key: string
  wrapped_file_key_iv: string
}

export interface WalletConnectVerifyResponse {
  M2: string
  connected: boolean
}

export async function getConnectionStatus(): Promise<WalletConnectionStatus> {
  const { data } = await api.get('/wallet/connect/status')
  return data
}

export async function connectStart(username = 'polymarket'): Promise<WalletConnectStartResponse> {
  const { data } = await api.post('/wallet/connect/start', { username })
  return data
}

export async function connectVerify(payload: WalletConnectVerifyRequest): Promise<WalletConnectVerifyResponse> {
  const { data } = await api.post('/wallet/connect/verify', payload)
  return data
}

export async function disconnect(): Promise<{ connected: boolean }> {
  const { data } = await api.post('/wallet/connect/disconnect')
  return data
}

// ===== Polymarket (Live, read-only) =====

export interface PolyBalance {
  [key: string]: unknown
}

export interface PolyPosition {
  [key: string]: unknown
}

export interface PolyOrder {
  [key: string]: unknown
}

export interface PolyTrade {
  [key: string]: unknown
}

export async function fetchPolyBalance(): Promise<PolyBalance> {
  const { data } = await api.get('/polymarket/balance')
  return data
}

export async function fetchPolyPositions(): Promise<PolyPosition[]> {
  const { data } = await api.get('/polymarket/positions')
  return data
}

export async function fetchPolyOrders(): Promise<PolyOrder[]> {
  const { data } = await api.get('/polymarket/orders')
  return data
}

export async function fetchPolyTrades(limit = 50): Promise<PolyTrade[]> {
  const { data } = await api.get('/polymarket/trades', { params: { limit } })
  return data
}

// ===== Balance Snapshots =====

export interface BalanceSnapshot {
  [key: string]: unknown
}

export async function fetchBalanceSnapshots(days = 7): Promise<BalanceSnapshot[]> {
  const { data } = await api.get('/balance-snapshots', { params: { days } })
  return data
}

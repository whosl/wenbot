import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  fetchFivesbotStatus,
  fetchFivesbotSignals,
  fetchFivesbotTrades,
  fetchEthFivesbotStatus,
  fetchEthFivesbotSignals,
  fetchEthFivesbotTrades,
  fetchWallet3FivesbotStatus,
  fetchWallet3FivesbotSignals,
  fetchWallet3FivesbotTrades,
  fetchWallet4FivesbotStatus,
  fetchWallet4FivesbotSignals,
  fetchWallet4FivesbotTrades,
  fetchBotConfig,
  fetchBotStatus,
  fetchWalletBalance,
  fetchWalletPositions,
  fetchWalletHistory,
  fetchWallet2Balance,
  fetchWallet2Positions,
  fetchWallet2History,
  fetchWallet3Balance,
  fetchWallet3Positions,
  fetchWallet3History,
  fetchWallet4Balance,
  fetchWallet4Positions,
  fetchWallet4History,
  fetchWeatherBalance,
  fetchWeatherPositions,
  fetchWeatherHistory,
  fetchWeatherSignals,
  fetchWeatherForecasts,
  fetchPolyBalance,
  fetchPolyPositions,
  fetchPolyOrders,
  fetchPolyTrades,
  fetchBalanceSnapshots,
  getConnectionStatus,
  disconnect,
  depositWallet,
  depositWallet2,
  depositWallet3,
  depositWallet4,
  syncPrices,
  syncWallet2Prices,
  syncWallet3Prices,
  syncWallet4Prices,
  settleWallet,
  settleWallet2,
  settleWallet3,
  settleWallet4,
} from './api'

// ===== Fivesbot =====
export function useFivesbotStatus() {
  return useQuery({
    queryKey: ['fivesbot-status'],
    queryFn: fetchFivesbotStatus,
    refetchInterval: 5000,
    retry: 2,
  })
}

export function useFivesbotSignals() {
  return useQuery({
    queryKey: ['fivesbot-signals'],
    queryFn: fetchFivesbotSignals,
    refetchInterval: 30000,
    retry: 2,
  })
}

export function useFivesbotTrades(limit = 50) {
  return useQuery({
    queryKey: ['fivesbot-trades', limit],
    queryFn: () => fetchFivesbotTrades(limit),
    refetchInterval: 30000,
    retry: 2,
  })
}

export function useEthFivesbotStatus() {
  return useQuery({
    queryKey: ['eth-fivesbot-status'],
    queryFn: fetchEthFivesbotStatus,
    refetchInterval: 5000,
    retry: 2,
  })
}

export function useEthFivesbotSignals() {
  return useQuery({
    queryKey: ['eth-fivesbot-signals'],
    queryFn: fetchEthFivesbotSignals,
    refetchInterval: 30000,
    retry: 2,
  })
}

export function useEthFivesbotTrades(limit = 50) {
  return useQuery({
    queryKey: ['eth-fivesbot-trades', limit],
    queryFn: () => fetchEthFivesbotTrades(limit),
    refetchInterval: 30000,
    retry: 2,
  })
}


export function useWallet3FivesbotStatus() {
  return useQuery({ queryKey: ['wallet3-fivesbot-status'], queryFn: fetchWallet3FivesbotStatus, refetchInterval: 5000, retry: 2 })
}

export function useWallet3FivesbotSignals() {
  return useQuery({ queryKey: ['wallet3-fivesbot-signals'], queryFn: fetchWallet3FivesbotSignals, refetchInterval: 30000, retry: 2 })
}

export function useWallet3FivesbotTrades(limit = 50) {
  return useQuery({ queryKey: ['wallet3-fivesbot-trades', limit], queryFn: () => fetchWallet3FivesbotTrades(limit), refetchInterval: 30000, retry: 2 })
}

export function useWallet4FivesbotStatus() {
  return useQuery({ queryKey: ['wallet4-fivesbot-status'], queryFn: fetchWallet4FivesbotStatus, refetchInterval: 5000, retry: 2 })
}

export function useWallet4FivesbotSignals() {
  return useQuery({ queryKey: ['wallet4-fivesbot-signals'], queryFn: fetchWallet4FivesbotSignals, refetchInterval: 30000, retry: 2 })
}

export function useWallet4FivesbotTrades(limit = 50) {
  return useQuery({ queryKey: ['wallet4-fivesbot-trades', limit], queryFn: () => fetchWallet4FivesbotTrades(limit), refetchInterval: 30000, retry: 2 })
}

// ===== Bot =====
export function useBotConfig() {
  return useQuery({
    queryKey: ['bot-config'],
    queryFn: fetchBotConfig,
    refetchInterval: 30000,
    retry: 2,
  })
}

export function useBotStatus() {
  return useQuery({
    queryKey: ['bot-status'],
    queryFn: fetchBotStatus,
    refetchInterval: 5000,
    retry: 2,
  })
}

// ===== Wallet Connect =====
export function useWalletConnectionStatus() {
  return useQuery({
    queryKey: ['wallet-connect-status'],
    queryFn: getConnectionStatus,
    refetchInterval: 5000,
    retry: 1,
  })
}

export function useDisconnectWallet() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: disconnect,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['wallet-connect-status'] })
      qc.invalidateQueries({ queryKey: ['poly-balance'] })
      qc.invalidateQueries({ queryKey: ['poly-orders'] })
      qc.invalidateQueries({ queryKey: ['poly-positions'] })
      qc.invalidateQueries({ queryKey: ['poly-trades'] })
    },
  })
}

// ===== Wallet 1 (BTC) =====
export function useWalletBalance() {
  return useQuery({
    queryKey: ['wallet-balance'],
    queryFn: fetchWalletBalance,
    refetchInterval: 10000,
    retry: 2,
  })
}

export function useWalletPositions() {
  return useQuery({
    queryKey: ['wallet-positions'],
    queryFn: fetchWalletPositions,
    refetchInterval: 10000,
    retry: 2,
  })
}

export function useWalletHistory() {
  return useQuery({
    queryKey: ['wallet-history'],
    queryFn: () => fetchWalletHistory(9999),
    staleTime: 60_000,
    retry: 2,
  })
}

export function useDepositWallet() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: depositWallet,
    onSuccess: () => qc.invalidateQueries({ queryKey: ['wallet-balance'] }),
  })
}

export function useSyncPrices() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: syncPrices,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['wallet-balance'] })
      qc.invalidateQueries({ queryKey: ['wallet-positions'] })
    },
  })
}

export function useSettleWallet() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: settleWallet,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['wallet-balance'] })
      qc.invalidateQueries({ queryKey: ['wallet-positions'] })
      qc.invalidateQueries({ queryKey: ['wallet-history'] })
    },
  })
}

export function useWallet2Balance() {
  return useQuery({ queryKey: ['wallet2-balance'], queryFn: fetchWallet2Balance, refetchInterval: 10000, retry: 2 })
}

export function useWallet2Positions() {
  return useQuery({ queryKey: ['wallet2-positions'], queryFn: fetchWallet2Positions, refetchInterval: 10000, retry: 2 })
}

export function useWallet2History() {
  return useQuery({ queryKey: ['wallet2-history'], queryFn: () => fetchWallet2History(9999), staleTime: 60_000, retry: 2 })
}

export function useDepositWallet2() {
  const qc = useQueryClient()
  return useMutation({ mutationFn: depositWallet2, onSuccess: () => qc.invalidateQueries({ queryKey: ['wallet2-balance'] }) })
}

export function useSyncWallet2Prices() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: syncWallet2Prices,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['wallet2-balance'] })
      qc.invalidateQueries({ queryKey: ['wallet2-positions'] })
    },
  })
}

export function useSettleWallet2() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: settleWallet2,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['wallet2-balance'] })
      qc.invalidateQueries({ queryKey: ['wallet2-positions'] })
      qc.invalidateQueries({ queryKey: ['wallet2-history'] })
    },
  })
}

// ===== Wallet 3 (BTC 8-indicator) =====
export function useWallet3Balance() {
  return useQuery({ queryKey: ['wallet3-balance'], queryFn: fetchWallet3Balance, refetchInterval: 10000, retry: 2 })
}

export function useWallet3Positions() {
  return useQuery({ queryKey: ['wallet3-positions'], queryFn: fetchWallet3Positions, refetchInterval: 10000, retry: 2 })
}

export function useWallet3History() {
  return useQuery({ queryKey: ['wallet3-history'], queryFn: () => fetchWallet3History(9999), staleTime: 60_000, retry: 2 })
}

export function useDepositWallet3() {
  const qc = useQueryClient()
  return useMutation({ mutationFn: depositWallet3, onSuccess: () => qc.invalidateQueries({ queryKey: ['wallet3-balance'] }) })
}

export function useSyncWallet3Prices() {
  const qc = useQueryClient()
  return useMutation({ mutationFn: syncWallet3Prices, onSuccess: () => { qc.invalidateQueries({ queryKey: ['wallet3-balance'] }); qc.invalidateQueries({ queryKey: ['wallet3-positions'] }) } })
}

export function useSettleWallet3() {
  const qc = useQueryClient()
  return useMutation({ mutationFn: settleWallet3, onSuccess: () => { qc.invalidateQueries({ queryKey: ['wallet3-balance'] }); qc.invalidateQueries({ queryKey: ['wallet3-positions'] }); qc.invalidateQueries({ queryKey: ['wallet3-history'] }) } })
}

// ===== Wallet 4 (BTC 5min classic_four) =====
export function useWallet4Balance() {
  return useQuery({ queryKey: ['wallet4-balance'], queryFn: fetchWallet4Balance, refetchInterval: 10000, retry: 2 })
}

export function useWallet4Positions() {
  return useQuery({ queryKey: ['wallet4-positions'], queryFn: fetchWallet4Positions, refetchInterval: 10000, retry: 2 })
}

export function useWallet4History() {
  return useQuery({ queryKey: ['wallet4-history'], queryFn: () => fetchWallet4History(9999), staleTime: 60_000, retry: 2 })
}

export function useDepositWallet4() {
  const qc = useQueryClient()
  return useMutation({ mutationFn: depositWallet4, onSuccess: () => qc.invalidateQueries({ queryKey: ['wallet4-balance'] }) })
}

export function useSyncWallet4Prices() {
  const qc = useQueryClient()
  return useMutation({ mutationFn: syncWallet4Prices, onSuccess: () => { qc.invalidateQueries({ queryKey: ['wallet4-balance'] }); qc.invalidateQueries({ queryKey: ['wallet4-positions'] }) } })
}

export function useSettleWallet4() {
  const qc = useQueryClient()
  return useMutation({ mutationFn: settleWallet4, onSuccess: () => { qc.invalidateQueries({ queryKey: ['wallet4-balance'] }); qc.invalidateQueries({ queryKey: ['wallet4-positions'] }); qc.invalidateQueries({ queryKey: ['wallet4-history'] }) } })
}

// ===== Weather (Wallet 5) =====
export function useWeatherBalance() {
  return useQuery({
    queryKey: ['weather-balance'],
    queryFn: fetchWeatherBalance,
    refetchInterval: 10000,
    retry: 2,
  })
}

export function useWeatherPositions() {
  return useQuery({
    queryKey: ['weather-positions'],
    queryFn: fetchWeatherPositions,
    refetchInterval: 10000,
    retry: 2,
  })
}

export function useWeatherHistory(limit = 100) {
  return useQuery({
    queryKey: ['weather-history', limit],
    queryFn: () => fetchWeatherHistory(limit),
    refetchInterval: 15000,
    retry: 2,
  })
}

export function useWeatherSignals() {
  return useQuery({
    queryKey: ['weather-signals'],
    queryFn: fetchWeatherSignals,
    refetchInterval: 30000,
    retry: 2,
  })
}

export function useWeatherForecasts() {
  return useQuery({
    queryKey: ['weather-forecasts'],
    queryFn: fetchWeatherForecasts,
    refetchInterval: 30000,
    retry: 2,
  })
}

export function useDepositWeather() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: depositWeather,
    onSuccess: () => qc.invalidateQueries({ queryKey: ['weather-balance'] }),
  })
}

// ===== Polymarket =====
export function usePolyBalance() {
  return useQuery({
    queryKey: ['poly-balance'],
    queryFn: fetchPolyBalance,
    refetchInterval: 60000,
    retry: 1,
  })
}

export function usePolyPositions() {
  return useQuery({
    queryKey: ['poly-positions'],
    queryFn: fetchPolyPositions,
    refetchInterval: 60000,
    retry: 1,
  })
}

export function usePolyOrders() {
  return useQuery({
    queryKey: ['poly-orders'],
    queryFn: fetchPolyOrders,
    refetchInterval: 60000,
    retry: 1,
  })
}

export function usePolyTrades(limit = 50) {
  return useQuery({
    queryKey: ['poly-trades', limit],
    queryFn: () => fetchPolyTrades(limit),
    refetchInterval: 120000,
    retry: 1,
  })
}

// ===== Balance Snapshots =====
export function useBalanceSnapshots(days = 7) {
  return useQuery({
    queryKey: ['balance-snapshots', days],
    queryFn: () => fetchBalanceSnapshots(days),
    refetchInterval: 60000,
    retry: 1,
  })
}

import { useMemo, useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { SectionHeader, MiniTable, QueryState, DepositButton, MobileCard, TabBar, formatPrice, truncate, DirectionBadge, PnLValue, UnrealizedPnlBlock, IndicatorPopover } from './ui'
import {
  useWalletBalance, useWalletPositions, useWalletHistory, useFivesbotTrades, useDepositWallet, useSyncPrices, useSettleWallet,
  useWallet2Balance, useWallet2Positions, useWallet2History, useEthFivesbotTrades, useDepositWallet2, useSyncWallet2Prices, useSettleWallet2,
  useWallet3Balance, useWallet3Positions, useWallet3History, useWallet3FivesbotTrades, useDepositWallet3, useSyncWallet3Prices, useSettleWallet3,
  useWallet4Balance, useWallet4Positions, useWallet4History, useWallet4FivesbotTrades, useDepositWallet4, useSyncWallet4Prices, useSettleWallet4,
  useWeatherBalance, useWeatherPositions, useWeatherHistory, useDepositWeather, useWalletConnectionStatus, useDisconnectWallet,
  usePolyBalance, usePolyOrders, usePolyPositions,
} from './hooks'
import { RefreshCw, CircleDollarSign, TrendingUp, ArrowUpDown, Clock, ChevronLeft, ChevronRight, Plug, Power } from 'lucide-react'
import { connectWalletWithPassword } from './srp'

type WalletView = 'poly' | 'btc' | 'eth' | 'btc8' | 'btc5' | 'weather'

function formatUtcToUtc8Time(ts?: string | null): string {
  if (!ts) return '—'
  const normalized = /z$|[+-]\d{2}:?\d{2}$/i.test(ts) ? ts : ts.replace(' ', 'T') + 'Z'
  const date = new Date(normalized)
  if (Number.isNaN(date.getTime())) return '—'
  return new Date(date.getTime() + 8 * 60 * 60 * 1000).toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12: false, timeZone: 'UTC' })
}

function PaginatedHistory({ history, trades }: {
  history: { data?: any[]; isLoading: boolean; error: Error | null }
  trades: { data?: any[] }
}) {
  const [page, setPage] = useState(1)
  const [pageSize, setPageSize] = useState(20)
  const totalCount = history.data?.length ?? 0
  const totalPages = Math.max(1, Math.ceil(totalCount / pageSize))

  // Reset to page 1 if current page exceeds total
  useMemo(() => { if (page > totalPages) setPage(1) }, [totalPages])

  const pageData = useMemo(() => {
    const all = history.data ?? []
    return all.slice((page - 1) * pageSize, page * pageSize)
  }, [history.data, page, pageSize])

  const tradesByPosition = useMemo(() => {
    const m = new Map<number, any>()
    for (const t of trades.data ?? []) m.set(t.position_id, t)
    return m
  }, [trades.data])

  if (!history.data?.length) {
    return <div className="py-8 text-center text-[10px] text-neutral-700 font-mono">No trade history</div>
  }

  return (
    <div className="space-y-2">
      {/* Desktop table */}
      <div className="hidden sm:block bg-[#0a0a0a] border border-[#1a1a1a] rounded-lg overflow-hidden">
        <div className="overflow-x-auto">
          <MiniTable
            columns={[
              { key: 'direction', label: 'Dir', width: '40px' },
              { key: 'market_question', label: 'Market' },
              { key: 'result', label: 'Result', width: '50px' },
              { key: 'entry_display', label: 'Entry', width: '55px', align: 'right' },
              { key: 'pnl', label: 'PnL', width: '70px', align: 'right' },
              { key: 'edge_display', label: 'Edge', width: '55px', align: 'right' },
              { key: 'conf_display', label: 'Conf', width: '45px', align: 'right' },
              { key: 'opened_at', label: 'Opened', width: '70px' },
              { key: 'closed_at', label: 'Closed', width: '70px' },
            ]}
            data={pageData.map((h: any) => {
              const trade = tradesByPosition.get(h.position_id)
              return {
                ...h,
                edge_display: trade ? `${trade.edge >= 0 ? '+' : ''}${(trade.edge * 100).toFixed(1)}%` : '—',
                conf_display: <IndicatorPopover details={h.indicator_details} confidence={trade?.confidence} />,
                entry_display: formatPrice(h.entry_price),
                opened_at: formatUtcToUtc8Time(h.opened_at),
                closed_at: formatUtcToUtc8Time(h.closed_at),
              }
            })}
          />
        </div>
      </div>
      {/* Mobile cards */}
      <div className="sm:hidden space-y-2">
        {pageData.map((h: any) => {
            const trade = tradesByPosition.get(h.position_id)
            return (
          <MobileCard key={h.id}>
            <div className="flex items-center justify-between mb-1.5">
              <div className="flex items-center gap-1.5">
                <DirectionBadge direction={h.direction} />
                {trade && <span className="text-[9px] text-neutral-500 font-mono">{trade.edge >= 0 ? '+' : ''}{(trade.edge * 100).toFixed(1)}%</span>}
                <IndicatorPopover details={h.indicator_details} confidence={trade?.confidence} />
              </div>
              <PnLValue value={h.pnl} />
            </div>
            <div className="text-[11px] text-neutral-300 font-mono mb-1 break-all">{truncate(h.market_question, 60)}</div>
            <div className="flex items-center justify-between text-[9px] text-neutral-600 font-mono">
              <span>Entry {formatPrice(h.entry_price)}</span>
              <span>{formatUtcToUtc8Time(h.opened_at)} → {formatUtcToUtc8Time(h.closed_at)}</span>
            </div>
          </MobileCard>
            )
          })}
      </div>
      {/* Pagination controls */}
      {totalPages > 1 && (
        <div className="flex items-center justify-between pt-2">
          <div className="flex items-center gap-1">
            <span className="text-[9px] text-neutral-600 font-mono mr-1">Per page:</span>
            {[10, 20, 50].map(s => (
              <button key={s} onClick={() => { setPageSize(s); setPage(1) }} className={`px-2 py-1 text-[9px] font-mono rounded border transition-colors ${pageSize === s ? 'border-amber-500/40 text-amber-400 bg-amber-500/5' : 'border-[#1a1a1a] text-neutral-600 hover:text-neutral-400 hover:border-neutral-600'}`}>{s}</button>
            ))}
          </div>
          <div className="flex items-center gap-2">
            <span className="text-[9px] text-neutral-600 font-mono">{totalCount} trades</span>
            <button onClick={() => setPage(p => Math.max(1, p - 1))} disabled={page <= 1} className="p-1 border border-[#1a1a1a] rounded disabled:opacity-30 hover:border-neutral-600 transition-colors"><ChevronLeft size={12} className="text-neutral-400" /></button>
            <span className="text-[10px] text-neutral-400 font-mono tabular-nums">{page}/{totalPages}</span>
            <button onClick={() => setPage(p => Math.min(totalPages, p + 1))} disabled={page >= totalPages} className="p-1 border border-[#1a1a1a] rounded disabled:opacity-30 hover:border-neutral-600 transition-colors"><ChevronRight size={12} className="text-neutral-400" /></button>
          </div>
        </div>
      )}
    </div>
  )
}

function FivesWallet({ title, balance, positions, history, trades, deposit, sync, settle, accent }: any) {
  return (
    <div className="p-3 space-y-3 pb-24">
      <QueryState isLoading={balance.isLoading} error={balance.error}>
        <div className="bg-[#0a0a0a] border border-[#1a1a1a] rounded-xl p-5">
          <div className="flex items-center justify-between mb-2">
            <span className="text-[9px] text-neutral-600 uppercase tracking-[0.12em] font-mono">{title}</span>
            <span className={`text-sm font-semibold font-mono tabular-nums ${balance.data && balance.data.total_pnl >= 0 ? 'text-green-400' : 'text-red-400'}`}>{balance.data ? `${balance.data.total_pnl >= 0 ? '+' : ''}$${balance.data.total_pnl.toFixed(2)}` : ''} PnL</span>
          </div>
          <div className="text-3xl font-bold font-mono tabular-nums text-neutral-100 tracking-tight">${balance.data?.balance.toFixed(2) ?? '0.00'}</div>
          <div className="grid grid-cols-3 gap-3 mt-3 pt-3 border-t border-[#1a1a1a]">
            <div><div className="text-[9px] text-neutral-600 uppercase tracking-wider font-mono mb-0.5">Equity</div><div className="text-sm font-mono tabular-nums text-neutral-300">${balance.data?.total_value.toFixed(2) ?? '0.00'}</div></div>
            <div><div className="text-[9px] text-neutral-600 uppercase tracking-wider font-mono mb-0.5">Trades</div><div className="text-sm font-mono tabular-nums text-neutral-300">{balance.data?.total_trades ?? 0}</div></div>
            <div><div className="text-[9px] text-neutral-600 uppercase tracking-wider font-mono mb-0.5">Win Rate</div><div className="text-sm font-mono tabular-nums text-neutral-300">{balance.data ? `${balance.data.win_rate.toFixed(1)}%` : '0%'}</div></div>
          </div>
        </div>
      </QueryState>

      <div className="flex items-center gap-2">
        <button onClick={() => sync.mutate()} disabled={sync.isPending} className="flex-1 flex items-center justify-center gap-1.5 py-2.5 text-[10px] uppercase tracking-wider font-mono text-neutral-400 border border-[#262626] rounded-lg bg-[#0a0a0a] disabled:opacity-40"><RefreshCw size={11} className={sync.isPending ? 'animate-spin' : ''} />{sync.isPending ? 'Syncing' : 'Sync Prices'}</button>
        <button onClick={() => settle.mutate()} disabled={settle.isPending} className="flex-1 flex items-center justify-center gap-1.5 py-2.5 text-[10px] uppercase tracking-wider font-mono text-neutral-400 border border-[#262626] rounded-lg bg-[#0a0a0a] disabled:opacity-40"><CircleDollarSign size={11} />{settle.isPending ? 'Settling' : 'Settle'}</button>
        <DepositButton onDeposit={(amt) => deposit.mutate(amt)} isPending={deposit.isPending} label="Deposit" />
      </div>

      <SectionHeader title="Open Positions" badge={positions.data?.length} badgeColor={accent} icon={<ArrowUpDown size={12} className="text-neutral-600" />} />
      <QueryState isLoading={positions.isLoading} error={positions.error}>
        <div className="space-y-2">
          {positions.data?.length ? (<><div className="hidden sm:block bg-[#0a0a0a] border border-[#1a1a1a] rounded-lg overflow-hidden"><div className="overflow-x-auto"><MiniTable columns={[{ key: 'direction', label: 'Dir', width: '40px' }, { key: 'market_question', label: 'Market' }, { key: 'entry_price', label: 'Entry', width: '55px', align: 'right' }, { key: 'current_price_display', label: 'Px', width: '55px', align: 'right' }, { key: 'current_value_display', label: 'Val', width: '60px', align: 'right' }, { key: 'unrealized_pnl_display', label: 'uPnL', width: '70px', align: 'right' }, { key: 'edge_display', label: 'Edge', width: '55px', align: 'right' }, { key: 'conf_display', label: 'Conf', width: '55px', align: 'right' }, { key: 'opened_at_display', label: 'Opened', width: '70px' }]} data={positions.data.map((p: any) => ({ ...p, entry_price: formatPrice(p.entry_price), current_price_display: p.current_price != null ? formatPrice(p.current_price) : '—', current_value_display: p.current_value != null ? `$${p.current_value.toFixed(2)}` : '—', unrealized_pnl_display: p.unrealized_pnl == null ? '—' : <span className={p.unrealized_pnl >= 0 ? 'text-green-400' : 'text-red-400'}>{`${p.unrealized_pnl >= 0 ? '+' : ''}$${p.unrealized_pnl.toFixed(2)}`}</span>, edge_display: p.edge != null ? <span className={p.edge >= 0 ? 'text-green-400' : 'text-red-400'}>{`${p.edge >= 0 ? '+' : ''}${(p.edge * 100).toFixed(1)}%`}</span> : '—', conf_display: <IndicatorPopover details={p.indicator_details} confidence={p.confidence} />, opened_at_display: p.created_at ? formatUtcToUtc8Time(p.created_at) : '—' }))} /></div></div><div className="sm:hidden space-y-2">{positions.data.map((p: any) => <MobileCard key={p.id}><div className="flex items-center justify-between mb-1.5"><DirectionBadge direction={p.direction} /><span className="text-[9px] text-neutral-700 font-mono">{p.window_end ? formatUtcToUtc8Time(p.window_end) : p.created_at ? formatUtcToUtc8Time(p.created_at) : ''}</span></div><div className="text-[11px] text-neutral-300 font-mono mb-2 break-all">{truncate(p.market_question, 60)}</div><div className="flex items-center justify-between mb-1"><span className="text-[9px] text-neutral-500 font-mono">Entry {formatPrice(p.entry_price)}</span><IndicatorPopover details={p.indicator_details} confidence={p.confidence} /></div><UnrealizedPnlBlock entryPrice={p.entry_price} currentPrice={p.current_price} unrealizedPnl={p.unrealized_pnl} currentValue={p.current_value} size={p.size} quantity={p.quantity} /></MobileCard>)}</div></>) : <div className="py-8 text-center text-[10px] text-neutral-700 font-mono">No open positions</div>}
        </div>
      </QueryState>

      <SectionHeader title="Trade History" badge={history.data?.length} badgeColor="neutral" icon={<Clock size={12} className="text-neutral-600" />} />
      <QueryState isLoading={history.isLoading} error={history.error}>
        <PaginatedHistory history={history} trades={trades} />
      </QueryState>
    </div>
  )
}

function WeatherWallet() {
  const balance = useWeatherBalance()
  const positions = useWeatherPositions()
  const history = useWeatherHistory(9999)
  const deposit = useDepositWeather()
  const qc = useQueryClient()
  const [page, setPage] = useState(1)
  const [pageSize, setPageSize] = useState(20)
  const [refreshing, setRefreshing] = useState(false)
  const totalCount = history.data?.length ?? 0
  const totalPages = Math.max(1, Math.ceil(totalCount / pageSize))
  const pageData = useMemo(() => (history.data ?? []).slice((page - 1) * pageSize, page * pageSize), [history.data, page, pageSize])
  useMemo(() => { if (page > totalPages) setPage(1) }, [totalPages])

  const handleRefresh = async () => {
    setRefreshing(true)
    await Promise.all([qc.invalidateQueries({ queryKey: ['weather-balance'] }), qc.invalidateQueries({ queryKey: ['weather-positions'] }), qc.invalidateQueries({ queryKey: ['weather-history'] })])
    setRefreshing(false)
  }
  return (
    <div className="p-3 space-y-3 pb-24">
      <QueryState isLoading={balance.isLoading} error={balance.error}><div className="bg-[#0a0a0a] border border-[#1a1a1a] rounded-xl p-5"><div className="flex items-center justify-between mb-2"><span className="text-[9px] text-neutral-600 uppercase tracking-[0.12em] font-mono">Wallet 5 · Weather</span><span className={`text-sm font-semibold font-mono tabular-nums ${balance.data && balance.data.total_pnl >= 0 ? 'text-green-400' : 'text-red-400'}`}>{balance.data ? `${balance.data.total_pnl >= 0 ? '+' : ''}$${balance.data.total_pnl.toFixed(2)}` : ''} PnL</span></div><div className="text-3xl font-bold font-mono tabular-nums text-neutral-100 tracking-tight">${balance.data?.balance.toFixed(2) ?? '0.00'}</div></div></QueryState>
      <div className="flex items-center gap-2"><button onClick={handleRefresh} disabled={refreshing} className="flex-1 flex items-center justify-center gap-1.5 py-2.5 text-[10px] uppercase tracking-wider font-mono text-neutral-400 border border-[#262626] rounded-lg bg-[#0a0a0a] disabled:opacity-40"><RefreshCw size={11} className={refreshing ? 'animate-spin' : ''} />{refreshing ? 'Refreshing' : 'Refresh'}</button><DepositButton onDeposit={(amt) => deposit.mutate(amt)} isPending={deposit.isPending} label="Deposit" /></div>
      <SectionHeader title="Open Positions" badge={positions.data?.length} badgeColor="cyan" icon={<ArrowUpDown size={12} className="text-neutral-600" />} />
      <QueryState isLoading={positions.isLoading} error={positions.error}>
        <div className="space-y-2">
          {positions.data?.length ? (<><div className="hidden sm:block bg-[#0a0a0a] border border-[#1a1a1a] rounded-lg overflow-hidden"><div className="overflow-x-auto"><MiniTable columns={[{ key: 'direction', label: 'Dir', width: '40px' }, { key: 'market_question', label: 'Market' }, { key: 'city_name_display', label: 'City', width: '70px' }, { key: 'entry_price', label: 'Entry', width: '55px', align: 'right' }, { key: 'current_price_display', label: 'Px', width: '55px', align: 'right' }, { key: 'current_value_display', label: 'Val', width: '60px', align: 'right' }, { key: 'unrealized_pnl_display', label: 'uPnL', width: '70px', align: 'right' }, { key: 'edge_display', label: 'Edge', width: '55px', align: 'right' }, { key: 'conf_display', label: 'Conf', width: '55px', align: 'right' }, { key: 'opened_at_display', label: 'Opened', width: '70px' }]} data={positions.data.map((p: any) => ({ ...p, city_name_display: p.city_name || '—', entry_price: formatPrice(p.entry_price), current_price_display: p.current_price != null ? formatPrice(p.current_price) : '—', current_value_display: p.current_value != null ? `$${p.current_value.toFixed(2)}` : '—', unrealized_pnl_display: p.unrealized_pnl == null ? '—' : <span className={p.unrealized_pnl >= 0 ? 'text-green-400' : 'text-red-400'}>{`${p.unrealized_pnl >= 0 ? '+' : ''}$${p.unrealized_pnl.toFixed(2)}`}</span>, edge_display: p.edge != null ? <span className={p.edge >= 0 ? 'text-green-400' : 'text-red-400'}>{`${p.edge >= 0 ? '+' : ''}${(p.edge * 100).toFixed(1)}%`}</span> : '—', conf_display: <IndicatorPopover details={p.indicator_details} confidence={p.confidence} />, opened_at_display: p.created_at ? formatUtcToUtc8Time(p.created_at) : '—' }))} /></div></div><div className="sm:hidden space-y-2">{positions.data.map((p: any) => <MobileCard key={p.id}><div className="flex items-center justify-between mb-1.5"><DirectionBadge direction={p.direction} />{p.city_name && <span className="text-[9px] text-cyan-400/60 font-mono">{p.city_name}</span>}<span className="text-[9px] text-neutral-700 font-mono">{p.window_end ? formatUtcToUtc8Time(p.window_end) : p.created_at ? formatUtcToUtc8Time(p.created_at) : ''}</span></div><div className="text-[11px] text-neutral-300 font-mono mb-2 break-all">{truncate(p.market_question, 60)}</div><div className="flex items-center justify-between mb-1"><span className="text-[9px] text-neutral-500 font-mono">Entry {formatPrice(p.entry_price)}</span><IndicatorPopover details={p.indicator_details} confidence={p.confidence} /></div><UnrealizedPnlBlock entryPrice={p.entry_price} currentPrice={p.current_price} unrealizedPnl={p.unrealized_pnl} currentValue={p.current_value} size={p.size} quantity={p.quantity} /></MobileCard>)}</div></>) : <div className="py-8 text-center text-[10px] text-neutral-700 font-mono">No open positions</div>}
        </div>
      </QueryState>
      <SectionHeader title="Trade History" badge={history.data?.length} badgeColor="neutral" icon={<Clock size={12} className="text-neutral-600" />} />
      <QueryState isLoading={history.isLoading} error={history.error}>
        <div className="space-y-2">
          {pageData.length ? (
            <>
              {/* Desktop table */}
              <div className="hidden sm:block bg-[#0a0a0a] border border-[#1a1a1a] rounded-lg overflow-hidden">
                <div className="overflow-x-auto">
                  <MiniTable
                    columns={[
                      { key: 'direction', label: 'Dir', width: '40px' },
                      { key: 'market_question', label: 'Market' },
                      { key: 'city_name_display', label: 'City', width: '70px' },
                      { key: 'result', label: 'Result', width: '50px' },
                      { key: 'entry_price_display', label: 'Entry', width: '55px', align: 'right' },
                      { key: 'pnl', label: 'PnL', width: '70px', align: 'right' },
                      { key: 'opened_at', label: 'Opened', width: '70px' },
                      { key: 'closed_at', label: 'Closed', width: '70px' },
                    ]}
                    data={pageData.map((h: any) => ({
                      ...h,
                      city_name_display: h.city_name || '—',
                      entry_price_display: formatPrice(h.entry_price),
                      opened_at: formatUtcToUtc8Time(h.opened_at),
                      closed_at: formatUtcToUtc8Time(h.closed_at),
                    }))}
                  />
                </div>
              </div>
              {/* Mobile cards */}
              <div className="sm:hidden space-y-2">
                {pageData.map((h: any) => (
                  <MobileCard key={h.id}>
                    <div className="flex items-center justify-between mb-1.5">
                      <div className="flex items-center gap-1.5">
                        <DirectionBadge direction={h.direction} />
                        {h.city_name && <span className="text-[9px] text-cyan-400/60 font-mono">{h.city_name}</span>}
                      </div>
                      <PnLValue value={h.pnl} />
                    </div>
                    <div className="text-[11px] text-neutral-300 font-mono mb-1 break-all">{truncate(h.market_question, 60)}</div>
                    <div className="flex items-center justify-between text-[9px] text-neutral-600 font-mono">
                      <span>Entry {formatPrice(h.entry_price)}</span>
                      <span>{formatUtcToUtc8Time(h.opened_at)} → {formatUtcToUtc8Time(h.closed_at)}</span>
                    </div>
                  </MobileCard>
                ))}
              </div>
            </>
          ) : <div className="py-8 text-center text-[10px] text-neutral-700 font-mono">No trade history</div>}
          {totalPages > 1 && (
            <div className="flex items-center justify-between pt-2">
              <div className="flex items-center gap-1">
                <span className="text-[9px] text-neutral-600 font-mono mr-1">Per page:</span>
                {[10, 20, 50].map(s => (
                  <button key={s} onClick={() => { setPageSize(s); setPage(1) }} className={`px-2 py-1 text-[9px] font-mono rounded border transition-colors ${pageSize === s ? 'border-cyan-500/40 text-cyan-400 bg-cyan-500/5' : 'border-[#1a1a1a] text-neutral-600 hover:text-neutral-400 hover:border-neutral-600'}`}>{s}</button>
                ))}
              </div>
              <div className="flex items-center gap-2">
                <span className="text-[9px] text-neutral-600 font-mono">{totalCount} trades</span>
                <button onClick={() => setPage(p => Math.max(1, p - 1))} disabled={page <= 1} className="p-1 border border-[#1a1a1a] rounded disabled:opacity-30 hover:border-neutral-600 transition-colors"><ChevronLeft size={12} className="text-neutral-400" /></button>
                <span className="text-[10px] text-neutral-400 font-mono tabular-nums">{page}/{totalPages}</span>
                <button onClick={() => setPage(p => Math.min(totalPages, p + 1))} disabled={page >= totalPages} className="p-1 border border-[#1a1a1a] rounded disabled:opacity-30 hover:border-neutral-600 transition-colors"><ChevronRight size={12} className="text-neutral-400" /></button>
              </div>
            </div>
          )}
        </div>
      </QueryState>
    </div>
  )
}

function BtcWallet() {
  return <FivesWallet title="Wallet 1 · BTC" balance={useWalletBalance()} positions={useWalletPositions()} history={useWalletHistory()} trades={useFivesbotTrades(200)} deposit={useDepositWallet()} sync={useSyncPrices()} settle={useSettleWallet()} accent="amber" />
}

function EthWallet() {
  return <FivesWallet title="Wallet 2 · ETH" balance={useWallet2Balance()} positions={useWallet2Positions()} history={useWallet2History()} trades={useEthFivesbotTrades(200)} deposit={useDepositWallet2()} sync={useSyncWallet2Prices()} settle={useSettleWallet2()} accent="violet" />
}

function Btc8Wallet() {
  return <FivesWallet title="Wallet 3 · BTC 8" balance={useWallet3Balance()} positions={useWallet3Positions()} history={useWallet3History()} trades={useWallet3FivesbotTrades(200)} deposit={useDepositWallet3()} sync={useSyncWallet3Prices()} settle={useSettleWallet3()} accent="emerald" />
}

function Btc5Wallet() {
  return <FivesWallet title="Wallet 4 · BTC 5m" balance={useWallet4Balance()} positions={useWallet4Positions()} history={useWallet4History()} trades={useWallet4FivesbotTrades(200)} deposit={useDepositWallet4()} sync={useSyncWallet4Prices()} settle={useSettleWallet4()} accent="blue" />
}

function PolyWallet() {
  const qc = useQueryClient()
  const { data: balance, isLoading: balLoading, error: balError, refetch: refetchBal } = usePolyBalance()
  const { data: orders, isLoading: ordLoading } = usePolyOrders()
  const { data: positions, isLoading: posLoading } = usePolyPositions()
  const status = useWalletConnectionStatus()
  const disconnect = useDisconnectWallet()
  const [open, setOpen] = useState(false)
  const [password, setPassword] = useState('')
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const usdc = balance?.usdc_balance ?? 0
  const allowance = balance?.usdc_allowance ?? 0
  const configured = status.data?.configured
  const connected = status.data?.connected

  const handleConnect = async () => {
    setSubmitting(true)
    setError(null)
    try {
      await connectWalletWithPassword(password)
      setPassword('')
      setOpen(false)
      await Promise.all([
        qc.invalidateQueries({ queryKey: ['wallet-connect-status'] }),
        qc.invalidateQueries({ queryKey: ['poly-balance'] }),
        qc.invalidateQueries({ queryKey: ['poly-orders'] }),
      ])
    } catch (e: any) {
      setError(e?.response?.data?.error || e?.message || 'Connect failed')
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="p-3 space-y-3">
      {/* Balance card with connect/disconnect */}
      <div className="bg-black/40 border border-[#262626] rounded-xl p-4">
        <div className="flex items-center justify-between mb-3">
          <div>
            <div className="text-[10px] text-neutral-500 uppercase tracking-[0.15em] font-mono">Polymarket Wallet</div>
            <div className="text-xs text-neutral-400 font-mono mt-0.5">
              {connected ? <span className="text-emerald-400">● 已连接</span> : configured ? <span className="text-neutral-400">○ 未连接</span> : <span className="text-amber-400">○ 未配置</span>}
            </div>
          </div>
          <div className="flex items-center gap-2">
            <button onClick={() => { refetchBal() }} className="p-1.5 rounded-lg border border-[#262626] hover:border-orange-500/30 text-neutral-500 hover:text-orange-400 transition-colors">
              <RefreshCw size={12} />
            </button>
            {connected ? (
              <button onClick={() => disconnect.mutate()} disabled={disconnect.isPending} className="inline-flex items-center gap-1 px-2.5 py-1.5 text-[10px] uppercase tracking-wider font-mono text-red-300 border border-red-500/20 rounded-lg bg-red-500/5 disabled:opacity-50">
                <Power size={10} />{disconnect.isPending ? '...' : 'Disconnect'}
              </button>
            ) : (
              <button onClick={() => setOpen(true)} disabled={!configured || status.isLoading} className="inline-flex items-center gap-1 px-2.5 py-1.5 text-[10px] uppercase tracking-wider font-mono text-neutral-300 border border-[#262626] rounded-lg bg-[#050505] disabled:opacity-50">
                <Plug size={10} />Connect
              </button>
            )}
          </div>
        </div>
        <div className="flex items-baseline gap-1">
          <span className="text-2xl font-semibold text-orange-400 font-mono">${usdc.toFixed(2)}</span>
          <span className="text-[10px] text-neutral-500 font-mono">USDC</span>
        </div>
        {allowance > 0 && (
          <div className="mt-1 text-[10px] text-neutral-500 font-mono">Allowance: ${allowance.toFixed(2)}</div>
        )}
      </div>

      {/* Open Orders */}
      <SectionHeader title="Open Orders" icon={<Clock size={12} />} />
      <QueryState isLoading={ordLoading} error={undefined}>
        {Array.isArray(orders) && orders.length > 0 ? (
          <MiniTable
            columns={[
              { key: 'market', label: 'Market', render: (v: any) => truncate(String(v.question || v.market || ''), 40) },
              { key: 'side', label: 'Side', render: (v: any) => <DirectionBadge direction={v.side === 'BUY' ? 'YES' : 'NO'} /> },
              { key: 'price', label: 'Price', render: (v: any) => formatPrice(v) },
              { key: 'size', label: 'Size', render: (v: any) => String(v) },
            ]}
            data={orders}
          />
        ) : (
          <div className="text-[11px] text-neutral-600 font-mono text-center py-4">No open orders</div>
        )}
      </QueryState>

      {/* Positions */}
      <SectionHeader title="Positions" icon={<TrendingUp size={12} />} />
      <QueryState isLoading={posLoading} error={undefined}>
        {Array.isArray(positions) && positions.length > 0 ? (
          <MiniTable
            columns={[
              { key: 'market', label: 'Market', render: (v: any) => truncate(String(v.question || v.market || v.asset || ''), 40) },
              { key: 'side', label: 'Side', render: (v: any) => <DirectionBadge direction={v.side || 'YES'} /> },
              { key: 'size', label: 'Size', render: (v: any) => formatPrice(v.size ?? v.initialValue ?? v) },
              { key: 'pnl', label: 'PnL', render: (v: any) => <PnLValue value={v.pnl ?? 0} /> },
            ]}
            data={positions}
          />
        ) : (
          <div className="text-[11px] text-neutral-600 font-mono text-center py-4">No positions</div>
        )}
      </QueryState>

      {/* Connect modal */}
      {open && (
        <div className="fixed inset-0 z-50 bg-black/70 backdrop-blur-sm flex items-center justify-center p-4">
          <div className="w-full max-w-md bg-[#0a0a0a] border border-[#1a1a1a] rounded-2xl p-5 shadow-2xl">
            <div className="text-sm font-semibold text-neutral-100 font-mono">连接 Polymarket 实盘钱包</div>
            <div className="text-[11px] text-neutral-500 font-mono mt-2 leading-relaxed">密码不会明文发送。前端会先完成 SRP 握手，再用会话密钥包裹解锁密钥。</div>
            <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoFocus className="mt-4 w-full bg-black border border-[#262626] rounded-lg px-3 py-2.5 text-sm text-neutral-100 font-mono outline-none focus:border-amber-500/40" placeholder="输入解锁密码" />
            {error && <div className="mt-3 text-[11px] text-red-400 font-mono">{error}</div>}
            <div className="mt-4 flex items-center justify-end gap-2">
              <button onClick={() => { setOpen(false); setPassword(''); setError(null) }} className="px-3 py-2 text-[10px] uppercase tracking-wider font-mono text-neutral-400 border border-[#262626] rounded-lg">Cancel</button>
              <button onClick={handleConnect} disabled={!password || submitting} className="px-3 py-2 text-[10px] uppercase tracking-wider font-mono text-amber-300 border border-amber-500/20 rounded-lg bg-amber-500/5 disabled:opacity-50">{submitting ? 'Connecting' : 'Connect'}</button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

function WalletConnectBar() {
  const qc = useQueryClient()
  const status = useWalletConnectionStatus()
  const disconnect = useDisconnectWallet()
  const [open, setOpen] = useState(false)
  const [password, setPassword] = useState('')
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const handleConnect = async () => {
    setSubmitting(true)
    setError(null)
    try {
      await connectWalletWithPassword(password)
      setPassword('')
      setOpen(false)
      await Promise.all([
        qc.invalidateQueries({ queryKey: ['wallet-connect-status'] }),
        qc.invalidateQueries({ queryKey: ['poly-balance'] }),
        qc.invalidateQueries({ queryKey: ['poly-orders'] }),
      ])
    } catch (e: any) {
      setError(e?.response?.data?.error || e?.message || 'Connect failed')
    } finally {
      setSubmitting(false)
    }
  }

  const configured = status.data?.configured
  const connected = status.data?.connected

  return (
    <>
      <div className="px-3 pt-3 pb-2">
        <div className="bg-[#0a0a0a] border border-[#1a1a1a] rounded-xl p-3 flex items-center justify-between gap-3">
          <div>
            <div className="text-[9px] text-neutral-600 uppercase tracking-[0.12em] font-mono">Polymarket Wallet</div>
            <div className={`text-xs font-mono mt-1 ${connected ? 'text-green-400' : configured ? 'text-neutral-300' : 'text-amber-400'}`}>
              {connected ? '已连接' : configured ? '未连接' : '未配置，请先运行 setup-polymarket'}
            </div>
          </div>
          {connected ? (
            <button onClick={() => disconnect.mutate()} disabled={disconnect.isPending} className="inline-flex items-center gap-1.5 px-3 py-2 text-[10px] uppercase tracking-wider font-mono text-red-300 border border-red-500/20 rounded-lg bg-red-500/5 disabled:opacity-50">
              <Power size={12} />{disconnect.isPending ? 'Disconnecting' : 'Disconnect'}
            </button>
          ) : (
            <button onClick={() => setOpen(true)} disabled={!configured || status.isLoading} className="inline-flex items-center gap-1.5 px-3 py-2 text-[10px] uppercase tracking-wider font-mono text-neutral-300 border border-[#262626] rounded-lg bg-[#050505] disabled:opacity-50">
              <Plug size={12} />Connect Wallet
            </button>
          )}
        </div>
      </div>

      {open && (
        <div className="fixed inset-0 z-50 bg-black/70 backdrop-blur-sm flex items-center justify-center p-4">
          <div className="w-full max-w-md bg-[#0a0a0a] border border-[#1a1a1a] rounded-2xl p-5 shadow-2xl">
            <div className="text-sm font-semibold text-neutral-100 font-mono">连接 Polymarket 实盘钱包</div>
            <div className="text-[11px] text-neutral-500 font-mono mt-2 leading-relaxed">密码不会明文发送。前端会先完成 SRP 握手，再用会话密钥包裹解锁用密钥。</div>
            <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoFocus className="mt-4 w-full bg-black border border-[#262626] rounded-lg px-3 py-2.5 text-sm text-neutral-100 font-mono outline-none focus:border-amber-500/40" placeholder="输入解锁密码" />
            {error && <div className="mt-3 text-[11px] text-red-400 font-mono">{error}</div>}
            <div className="mt-4 flex items-center justify-end gap-2">
              <button onClick={() => { setOpen(false); setPassword(''); setError(null) }} className="px-3 py-2 text-[10px] uppercase tracking-wider font-mono text-neutral-400 border border-[#262626] rounded-lg">Cancel</button>
              <button onClick={handleConnect} disabled={!password || submitting} className="px-3 py-2 text-[10px] uppercase tracking-wider font-mono text-amber-300 border border-amber-500/20 rounded-lg bg-amber-500/5 disabled:opacity-50">{submitting ? 'Connecting' : 'Connect'}</button>
            </div>
          </div>
        </div>
      )}
    </>
  )
}

export function WalletTab() {
  const [view, setView] = useState<WalletView>('poly')
  const walletTabs = [
    { id: 'poly', label: 'POLY', color: 'orange', icon: <span className="text-xs">🎯</span> },
    { id: 'btc', label: 'BTC (W1)', color: 'amber', icon: <span className="text-xs">₿</span> },
    { id: 'eth', label: 'ETH (W2)', color: 'violet', icon: <span className="text-xs">Ξ</span> },
    { id: 'btc8', label: 'BTC 8 (W3)', color: 'emerald', icon: <span className="text-xs">₿</span> },
    { id: 'btc5', label: 'BTC 5m (W4)', color: 'blue', icon: <span className="text-xs">₿</span> },
    { id: 'weather', label: 'Weather (W5)', color: 'cyan', icon: <span className="text-xs">☁</span> },
  ]

  return <div className="flex flex-col h-full"><TabBar tabs={walletTabs} active={view} onChange={(id) => setView(id as WalletView)} /><div className="flex-1 overflow-y-auto">{view === 'poly' ? <PolyWallet /> : view === 'btc' ? <BtcWallet /> : view === 'eth' ? <EthWallet /> : view === 'btc8' ? <Btc8Wallet /> : view === 'btc5' ? <Btc5Wallet /> : <WeatherWallet />}</div></div>
}

import { useState, useEffect, useCallback } from 'react'
import {
  StatCard,
  TabBar,
  MiniTable,
  LiveClock,
  QueryState,
  ConfidenceBar,
  Collapsible,
  formatPnl,
  formatPercent,
  formatPrice,
  truncate,
} from './wallet/ui'
import {
  useFivesbotStatus,
  useFivesbotSignals,
  useEthFivesbotStatus,
  useEthFivesbotSignals,
  useWalletBalance,
  useWallet2Balance,
  useWeatherBalance,
  useWeatherSignals,
  useSyncPrices,
  useSettleWallet,
} from './wallet/hooks'
import { RefreshCw, CircleDollarSign, Activity, BarChart3, Flame, TrendingUp, Zap, Globe, Wallet, Settings } from 'lucide-react'

// ===== Time-ago helper =====
function timeAgo(isoString: string | null | undefined): string {
  if (!isoString) return '—'
  const diff = Math.floor((Date.now() - new Date(isoString).getTime()) / 1000)
  if (diff < 5) return 'just now'
  if (diff < 60) return `${diff}s ago`
  const mins = Math.floor(diff / 60)
  if (mins < 60) return `${mins}m ago`
  return `${Math.floor(mins / 60)}h ago`
}

// ===== Color helpers =====
function pnlColor(v: number): string {
  return v >= 0 ? 'text-green-400' : 'text-red-400'
}

function directionColor(dir: string): string {
  const d = dir.toLowerCase()
  if (d === 'up' || d === 'yes' || d === 'long') return 'text-green-400'
  if (d === 'down' || d === 'no' || d === 'short') return 'text-red-400'
  return 'text-neutral-400'
}

// ===== Fivesbot Dashboard =====
function Dashboard() {
  const status = useFivesbotStatus()
  const signals = useFivesbotSignals()
  const weatherSignals = useWeatherSignals()
  const ethStatus = useEthFivesbotStatus()
  const ethSignals = useEthFivesbotSignals()
  const btcWallet = useWalletBalance()
  const ethWallet = useWallet2Balance()
  const wxWallet = useWeatherBalance()
  const syncMut = useSyncPrices()
  const settleMut = useSettleWallet()

  // Tick every second for live "time ago" display
  const [, setTick] = useState(0)
  useEffect(() => {
    const id = setInterval(() => setTick(t => t + 1), 1000)
    return () => clearInterval(id)
  }, [])

  const s = status.data
  const sigs = signals.data ?? []
  const wxSigs = weatherSignals.data ?? []
  const ethSigs = ethSignals.data ?? []

  return (
    <div className="flex flex-col gap-3 p-3 md:p-4 max-w-6xl mx-auto">
      {/* ===== BTC Price Hero ===== */}
      <QueryState isLoading={status.isLoading} error={status.error}>
        {s && (
          <div className="bg-[#0a0a0a] border border-[#1a1a1a] rounded-xl p-4 md:p-5">
            <div className="flex flex-col sm:flex-row sm:items-end justify-between gap-3">
              <div>
                <div className="flex items-center gap-2 mb-2">
                  {s.wsConnected ? (
                    <span className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-green-500/10 border border-green-500/30 live-badge-breathe">
                      <span className="w-2 h-2 rounded-full bg-green-500 live-pulse" />
                      <span className="text-[9px] text-green-400 uppercase tracking-[0.12em] font-mono font-bold">Live</span>
                    </span>
                  ) : (
                    <span className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-neutral-800/50 border border-neutral-700/50">
                      <span className="w-2 h-2 rounded-full bg-neutral-600" />
                      <span className="text-[9px] text-neutral-500 uppercase tracking-[0.12em] font-mono">Idle</span>
                    </span>
                  )}
                  <span className="text-[9px] text-neutral-600 uppercase tracking-[0.12em] font-mono">
                    BTC / Fivesbot
                  </span>
                  {s.lastUpdate && (
                    <span className="text-[9px] text-neutral-700 font-mono">
                      Last tick {timeAgo(s.lastUpdate)}
                    </span>
                  )}
                </div>
                <div className="text-3xl md:text-4xl font-bold font-mono tabular-nums text-amber-400 tracking-tight">
                  ${s.btcPrice.toLocaleString(undefined, { maximumFractionDigits: 0 })}
                </div>
                <div className="flex items-center gap-4 mt-2 text-xs font-mono tabular-nums">
                  <span className={`flex items-center gap-1.5 px-2 py-1 rounded-md ${
                    s.wsConnected ? 'bg-green-500/5 border border-green-500/10' : ''
                  }`}>
                    <span className={`w-1.5 h-1.5 rounded-full bg-green-500 ${s.wsConnected ? 'live-pulse' : ''}`} />
                    <span className="text-green-400 font-medium">UP</span>
                    <span className="text-green-400/70 text-sm">{formatPrice(s.upPrice)}</span>
                  </span>
                  <span className={`flex items-center gap-1.5 px-2 py-1 rounded-md ${
                    s.wsConnected ? 'bg-red-500/5 border border-red-500/10' : ''
                  }`}>
                    <span className={`w-1.5 h-1.5 rounded-full bg-red-500 ${s.wsConnected ? 'live-pulse' : ''}`} />
                    <span className="text-red-400 font-medium">DN</span>
                    <span className="text-red-400/70 text-sm">{formatPrice(s.downPrice)}</span>
                  </span>
                  {s.currentCycle && (
                    <span className="text-neutral-600">
                      Cycle <span className="text-neutral-400">{s.currentCycle}</span>
                    </span>
                  )}
                </div>
              </div>

              {/* Action Buttons - desktop */}
              <div className="flex items-center gap-2 sm:shrink-0">
                <button
                  onClick={() => syncMut.mutate()}
                  disabled={syncMut.isPending}
                  className="flex items-center gap-1.5 px-3 py-2 bg-neutral-900 border border-[#262626] hover:border-neutral-500 text-neutral-400 hover:text-neutral-200 text-[10px] uppercase tracking-wider font-mono transition-all rounded-lg disabled:opacity-40"
                >
                  <RefreshCw size={11} className={syncMut.isPending ? 'animate-spin' : ''} />
                  {syncMut.isPending ? 'Syncing' : 'Sync'}
                </button>
                <button
                  onClick={() => settleMut.mutate()}
                  disabled={settleMut.isPending}
                  className="flex items-center gap-1.5 px-3 py-2 bg-neutral-900 border border-[#262626] hover:border-neutral-500 text-neutral-400 hover:text-neutral-200 text-[10px] uppercase tracking-wider font-mono transition-all rounded-lg disabled:opacity-40"
                >
                  <CircleDollarSign size={11} />
                  {settleMut.isPending ? 'Settling' : 'Settle'}
                </button>
              </div>
            </div>

            {/* Predictor Panel - Grid */}
            <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-2 mt-4 pt-4 border-t border-[#1a1a1a]">
              <StatCard
                label="Signal"
                value={s.markets?.btc?.predictor?.signal ?? '—'}
                color={directionColor(s.markets?.btc?.predictor?.signal ?? '')}
                icon={<Activity size={12} />}
              />
              <div className="bg-[#0a0a0a] border border-[#1a1a1a] rounded-lg p-3">
                <div className="flex items-center gap-1.5 mb-1.5">
                  <Zap size={12} className="text-neutral-600" />
                  <span className="text-[9px] text-neutral-600 uppercase tracking-[0.12em] font-mono">Conf</span>
                </div>
                <div className="text-lg font-semibold font-mono tabular-nums text-neutral-100 leading-tight">
                  {formatPercent(s.markets?.btc?.predictor?.confidence ?? 0)}
                </div>
                <div className="mt-2">
                  <ConfidenceBar value={s.markets?.btc?.predictor?.confidence ?? 0} />
                </div>
              </div>
              <StatCard
                label="RSI"
                value={(s.markets?.btc?.predictor?.rsi ?? 0).toFixed(1)}
                icon={<BarChart3 size={12} className="text-neutral-600" />}
                color={(s.markets?.btc?.predictor?.rsi ?? 50) > 70 ? 'text-red-400' : (s.markets?.btc?.predictor?.rsi ?? 50) < 30 ? 'text-green-400' : 'text-neutral-100'}
              />
              <StatCard
                label="Momentum"
                value={(s.markets?.btc?.predictor?.momentum ?? 0).toFixed(2)}
                icon={<Flame size={12} className="text-neutral-600" />}
                color={(s.markets?.btc?.predictor?.momentum ?? 0) >= 0 ? 'text-green-400' : 'text-red-400'}
              />
              <StatCard
                label="Trend"
                value={(s.markets?.btc?.predictor?.trend ?? 0).toFixed(2)}
                icon={<TrendingUp size={12} className="text-neutral-600" />}
                color={(s.markets?.btc?.predictor?.trend ?? 0) >= 0 ? 'text-green-400' : 'text-red-400'}
              />
              <StatCard
                label="Volatility"
                value={(s.markets?.btc?.predictor?.volatility ?? 0).toFixed(2)}
                icon={<Activity size={12} className="text-neutral-600" />}
              />
            </div>
          </div>
        )}
      </QueryState>

      {/* ===== Active Markets ===== */}
      <QueryState isLoading={status.isLoading} error={status.error}>
        {s && s.markets?.btc?.activeMarkets && s.markets.btc.activeMarkets.length > 0 && (
          <Collapsible
            title="Active Markets"
            icon={<BarChart3 size={12} className="text-neutral-600" />}
            badge={s.markets.btc.activeMarkets.length}
            badgeColor="amber"
            defaultOpen={true}
          >
            <div className="overflow-x-auto">
              <MiniTable
                columns={[
                  { key: 'slug', label: 'Market' },
                  { key: 'up_price', label: 'UP', align: 'right' },
                  { key: 'down_price', label: 'DN', align: 'right' },
                  { key: 'end_date', label: 'End' },
                ]}
                data={s.markets.btc.activeMarkets.map(m => ({
                  slug: truncate(m.slug, 30),
                  up_price: formatPrice(m.up_price),
                  down_price: formatPrice(m.down_price),
                  end_date: new Date(m.end_date).toLocaleTimeString('en-US', { hour12: false, hour: '2-digit', minute: '2-digit' }),
                }))}
              />
            </div>
          </Collapsible>
        )}
      </QueryState>

      {/* ===== Wallet Overview Cards ===== */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
        <QueryState isLoading={btcWallet.isLoading} error={btcWallet.error}>
          {btcWallet.data && (
            <WalletOverviewCard
              title="BTC Strategy"
              icon="₿"
              color="amber"
              balance={btcWallet.data}
            />
          )}
        </QueryState>

        <QueryState isLoading={ethWallet.isLoading} error={ethWallet.error}>
          {ethWallet.data && (
            <WalletOverviewCard
              title="ETH Strategy"
              icon="Ξ"
              color="violet"
              balance={ethWallet.data}
            />
          )}
        </QueryState>

        <QueryState isLoading={wxWallet.isLoading} error={wxWallet.error}>
          {wxWallet.data && (
            <WalletOverviewCard
              title="Weather Strategy"
              icon="☁"
              color="cyan"
              balance={wxWallet.data}
            />
          )}
        </QueryState>
      </div>

      {/* ===== Readable Summary Row ===== */}
      <div className="grid grid-cols-1 lg:grid-cols-[1.3fr_0.7fr] gap-3">
        <div className="bg-[#0a0a0a] border border-[#1a1a1a] rounded-xl p-4">
          <div className="flex items-center gap-2 mb-3">
            <BarChart3 size={13} className="text-neutral-600" />
            <span className="text-[9px] text-neutral-600 uppercase tracking-[0.14em] font-mono">Snapshot</span>
          </div>
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            <SummaryItem
              label="BTC"
              value={s ? `$${s.btcPrice.toLocaleString(undefined, { maximumFractionDigits: 0 })}` : '—'}
              color="text-amber-400"
            />
            <SummaryItem
              label="Signal"
              value={s?.markets?.btc?.predictor?.signal ?? '—'}
              color={directionColor(s?.markets?.btc?.predictor?.signal ?? '')}
            />
            <SummaryItem
              label="BTC PnL"
              value={btcWallet.data ? formatPnl(btcWallet.data.total_pnl).text : '—'}
              color={btcWallet.data ? pnlColor(btcWallet.data.total_pnl) : 'text-neutral-400'}
            />
            <SummaryItem
              label="ETH PnL"
              value={ethWallet.data ? formatPnl(ethWallet.data.total_pnl).text : '—'}
              color={ethWallet.data ? pnlColor(ethWallet.data.total_pnl) : 'text-neutral-400'}
            />
            <SummaryItem
              label="WX Equity"
              value={wxWallet.data ? `$${wxWallet.data.total_value.toFixed(2)}` : '—'}
              color="text-cyan-400"
            />
          </div>
        </div>

        <div className="bg-[#0a0a0a] border border-[#1a1a1a] rounded-xl p-4">
          <div className="flex items-center gap-2 mb-3">
            <Activity size={13} className="text-neutral-600" />
            <span className="text-[9px] text-neutral-600 uppercase tracking-[0.14em] font-mono">Quick Read</span>
          </div>
          <div className="space-y-2 text-[11px] font-mono">
            <InfoLine label="Runtime" value={s?.lastUpdate ? 'Fresh' : 'Waiting'} />
            <InfoLine label="Markets" value={String(s?.markets?.btc?.activeMarkets?.length ?? 0)} />
            <InfoLine label="BTC Sigs" value={String(sigs.length)} />
            <InfoLine label="ETH Sigs" value={String(ethSigs.length)} />
            <InfoLine label="WX Sigs" value={String(wxSigs.length)} />
          </div>
        </div>
      </div>

      {/* ===== Recent Signals ===== */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-3">
        <QueryState isLoading={signals.isLoading} error={signals.error}>
          <div className="bg-[#0a0a0a] border border-[#1a1a1a] rounded-xl overflow-hidden">
            <div className="flex items-center gap-2 px-4 py-3 border-b border-[#1a1a1a]">
              <Zap size={13} className="text-amber-500" />
              <span className="text-[10px] text-neutral-400 uppercase tracking-[0.12em] font-mono font-semibold">BTC Signals</span>
              <span className="px-1.5 py-0.5 text-[8px] font-bold uppercase border font-mono bg-amber-500/10 text-amber-400 border-amber-500/20">BTC</span>
              <span className="flex-1" />
              <span className="text-[9px] text-neutral-700 font-mono">{sigs.length}</span>
            </div>
            <div className="overflow-x-auto">
              <table className="w-full text-[11px] font-mono">
                <thead>
                  <tr className="text-neutral-600 border-b border-[#1a1a1a]">
                    <th className="py-2.5 px-3 text-[8px] uppercase tracking-[0.14em] font-semibold text-left">Time</th>
                    <th className="py-2.5 px-3 text-[8px] uppercase tracking-[0.14em] font-semibold text-left">Signal</th>
                    <th className="py-2.5 px-3 text-[8px] uppercase tracking-[0.14em] font-semibold text-right">Edge</th>
                    <th className="py-2.5 px-3 text-[8px] uppercase tracking-[0.14em] font-semibold text-left">Conf</th>
                  </tr>
                </thead>
                <tbody>
                  {sigs.length === 0 ? (
                    <tr><td colSpan={4} className="py-8 text-center text-neutral-700 text-[10px]">No BTC signals</td></tr>
                  ) : sigs.slice(0, 5).map((sig, i) => (
                    <tr key={i} className="border-b border-[#0f0f0f] hover:bg-[#0d0d0d] transition-colors">
                      <td className="py-2.5 px-3 text-neutral-500 whitespace-nowrap">{new Date(sig.timestamp).toLocaleTimeString('en-US', { hour12: false, hour: '2-digit', minute: '2-digit' })}</td>
                      <td className="py-2.5 px-3 whitespace-nowrap">
                        <span className={`font-semibold ${sig.signal === 'UP' ? 'text-green-400' : sig.signal === 'DOWN' ? 'text-red-400' : 'text-neutral-400'}`}>
                          {sig.signal}
                        </span>
                      </td>
                      <td className="py-2.5 px-3 whitespace-nowrap text-right">
                        <span className={sig.edge >= 5 ? 'text-green-400 font-semibold' : sig.edge >= 0 ? 'text-amber-400' : 'text-red-400'}>
                          {sig.edge >= 0 ? '+' : ''}{sig.edge.toFixed(1)}%
                        </span>
                      </td>
                      <td className="py-2.5 px-3 whitespace-nowrap">
                        <span className={sig.confidence >= 0.6 ? 'text-green-400' : sig.confidence >= 0.4 ? 'text-amber-400' : 'text-red-400'}>
                          {formatPercent(sig.confidence)}
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </QueryState>

        <QueryState isLoading={ethSignals.isLoading || ethStatus.isLoading} error={ethSignals.error || ethStatus.error}>
          <div className="bg-[#0a0a0a] border border-[#1a1a1a] rounded-xl overflow-hidden">
            <div className="flex items-center gap-2 px-4 py-3 border-b border-[#1a1a1a]">
              <Zap size={13} className="text-violet-500" />
              <span className="text-[10px] text-neutral-400 uppercase tracking-[0.12em] font-mono font-semibold">ETH Signals</span>
              <span className="px-1.5 py-0.5 text-[8px] font-bold uppercase border font-mono bg-violet-500/10 text-violet-400 border-violet-500/20">ETH</span>
              <span className="flex-1" />
              <span className="text-[9px] text-neutral-700 font-mono">{ethSigs.length}</span>
            </div>
            <div className="overflow-x-auto">
              <table className="w-full text-[11px] font-mono">
                <thead>
                  <tr className="text-neutral-600 border-b border-[#1a1a1a]">
                    <th className="py-2.5 px-3 text-[8px] uppercase tracking-[0.14em] font-semibold text-left">Time</th>
                    <th className="py-2.5 px-3 text-[8px] uppercase tracking-[0.14em] font-semibold text-left">Signal</th>
                    <th className="py-2.5 px-3 text-[8px] uppercase tracking-[0.14em] font-semibold text-right">Edge</th>
                    <th className="py-2.5 px-3 text-[8px] uppercase tracking-[0.14em] font-semibold text-left">Conf</th>
                  </tr>
                </thead>
                <tbody>
                  {ethSigs.length === 0 ? (
                    <tr><td colSpan={4} className="py-8 text-center text-neutral-700 text-[10px]">No ETH signals</td></tr>
                  ) : ethSigs.slice(0, 5).map((sig, i) => (
                    <tr key={i} className="border-b border-[#0f0f0f] hover:bg-[#0d0d0d] transition-colors">
                      <td className="py-2.5 px-3 text-neutral-500 whitespace-nowrap">{new Date(sig.timestamp).toLocaleTimeString('en-US', { hour12: false, hour: '2-digit', minute: '2-digit' })}</td>
                      <td className="py-2.5 px-3 whitespace-nowrap"><span className={`font-semibold ${sig.signal === 'UP' ? 'text-green-400' : sig.signal === 'DOWN' ? 'text-red-400' : 'text-neutral-400'}`}>{sig.signal}</span></td>
                      <td className="py-2.5 px-3 whitespace-nowrap text-right"><span className={sig.edge >= 5 ? 'text-green-400 font-semibold' : sig.edge >= 0 ? 'text-amber-400' : 'text-red-400'}>{sig.edge >= 0 ? '+' : ''}{sig.edge.toFixed(1)}%</span></td>
                      <td className="py-2.5 px-3 whitespace-nowrap"><span className={sig.confidence >= 0.6 ? 'text-green-400' : sig.confidence >= 0.4 ? 'text-amber-400' : 'text-red-400'}>{formatPercent(sig.confidence)}</span></td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </QueryState>

        <QueryState isLoading={weatherSignals.isLoading} error={weatherSignals.error}>
          <div className="bg-[#0a0a0a] border border-[#1a1a1a] rounded-xl overflow-hidden">
            <div className="flex items-center gap-2 px-4 py-3 border-b border-[#1a1a1a]">
              <Globe size={13} className="text-cyan-500" />
              <span className="text-[10px] text-neutral-400 uppercase tracking-[0.12em] font-mono font-semibold">WX Signals</span>
              <span className="px-1.5 py-0.5 text-[8px] font-bold uppercase border font-mono bg-cyan-500/10 text-cyan-400 border-cyan-500/20">WX</span>
              <span className="flex-1" />
              <span className="text-[9px] text-neutral-700 font-mono">{wxSigs.length}</span>
            </div>
            <div className="overflow-x-auto">
              <table className="w-full text-[11px] font-mono">
                <thead>
                  <tr className="text-neutral-600 border-b border-[#1a1a1a]">
                    <th className="py-2.5 px-3 text-[8px] uppercase tracking-[0.14em] font-semibold text-left">City</th>
                    <th className="py-2.5 px-3 text-[8px] uppercase tracking-[0.14em] font-semibold text-left">Dir</th>
                    <th className="py-2.5 px-3 text-[8px] uppercase tracking-[0.14em] font-semibold text-right">Edge</th>
                    <th className="py-2.5 px-3 text-[8px] uppercase tracking-[0.14em] font-semibold text-left">Conf</th>
                  </tr>
                </thead>
                <tbody>
                  {[...wxSigs]
                    .sort((a, b) => b.edge - a.edge)
                    .slice(0, 5)
                    .length === 0 ? (
                    <tr><td colSpan={4} className="py-8 text-center text-neutral-700 text-[10px]">No WX signals</td></tr>
                  ) : [...wxSigs]
                    .sort((a, b) => b.edge - a.edge)
                    .slice(0, 5)
                    .map((sig, i) => (
                    <tr key={i} className="border-b border-[#0f0f0f] hover:bg-[#0d0d0d] transition-colors">
                      <td className="py-2.5 px-3 text-neutral-300 whitespace-nowrap">{truncate(sig.city_name, 14)}</td>
                      <td className="py-2.5 px-3 whitespace-nowrap">
                        <span className={`font-semibold ${sig.direction === 'YES' ? 'text-green-400' : 'text-red-400'}`}>
                          {sig.direction}
                        </span>
                      </td>
                      <td className="py-2.5 px-3 whitespace-nowrap text-right">
                        <span className={sig.edge >= 5 ? 'text-green-400 font-semibold' : sig.edge >= 0 ? 'text-amber-400' : 'text-red-400'}>
                          {sig.edge >= 0 ? '+' : ''}{sig.edge.toFixed(1)}%
                        </span>
                      </td>
                      <td className="py-2.5 px-3 whitespace-nowrap">
                        <span className={sig.confidence >= 0.6 ? 'text-green-400' : sig.confidence >= 0.4 ? 'text-amber-400' : 'text-red-400'}>
                          {formatPercent(sig.confidence)}
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </QueryState>
      </div>
    </div>
  )
}

// ===== Wallet overview card =====
function WalletOverviewCard({
  title,
  icon,
  color,
  balance,
}: {
  title: string
  icon: string
  color: 'amber' | 'cyan' | 'violet'
  balance: { balance: number; total_pnl: number; total_value: number; total_trades: number; win_rate: number }
}) {
  const accent = color === 'amber' ? 'text-amber-400' : color === 'violet' ? 'text-violet-400' : 'text-cyan-400'
  const borderAccent = color === 'amber' ? 'border-amber-500/10 hover:border-amber-500/20' : color === 'violet' ? 'border-violet-500/10 hover:border-violet-500/20' : 'border-cyan-500/10 hover:border-cyan-500/20'
  const glowClass = color === 'amber' ? 'card-glow-amber' : color === 'violet' ? '' : 'card-glow-cyan'

  return (
    <div className={`bg-[#0a0a0a] border rounded-xl p-4 transition-all ${borderAccent} ${glowClass}`}>
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <span className={`w-2 h-2 rounded-full ${color === 'amber' ? 'bg-amber-500' : color === 'violet' ? 'bg-violet-500' : 'bg-cyan-500'}`} />
          <span className="text-[9px] text-neutral-600 uppercase tracking-[0.12em] font-mono">
            {icon} {title}
          </span>
        </div>
        <span className={`px-1.5 py-0.5 text-[8px] font-bold uppercase font-mono ${
          color === 'amber'
            ? 'bg-amber-500/10 text-amber-400 border border-amber-500/20'
            : color === 'violet'
            ? 'bg-violet-500/10 text-violet-400 border border-violet-500/20'
            : 'bg-cyan-500/10 text-cyan-400 border border-cyan-500/20'
        }`}>
          W{title === 'BTC Strategy' ? '1' : title === 'ETH Strategy' ? '2' : '3'}
        </span>
      </div>
      <div className="flex items-end justify-between gap-3 mb-2">
        <div>
          <div className={`text-2xl font-bold font-mono tabular-nums ${accent} tracking-tight`}>
            ${balance.balance.toFixed(2)}
          </div>
          <div className="text-[9px] text-neutral-600 font-mono mt-1 uppercase tracking-[0.1em]">
            Cash Balance
          </div>
        </div>
        <div className="text-right">
          <div className="text-[10px] text-neutral-600 font-mono uppercase tracking-[0.1em]">Equity</div>
          <div className="text-sm font-mono tabular-nums text-neutral-300">${balance.total_value.toFixed(2)}</div>
        </div>
      </div>
      <div className="grid grid-cols-3 gap-2 mt-4 pt-4 border-t border-[#1a1a1a]">
        <MetricBlock label="PnL" value={formatPnl(balance.total_pnl).text} color={pnlColor(balance.total_pnl)} />
        <MetricBlock label="Trades" value={String(balance.total_trades)} />
        <MetricBlock label="Win Rate" value={`${balance.win_rate.toFixed(1)}%`} color={balance.win_rate >= 50 ? 'text-green-400' : 'text-red-400'} />
      </div>
    </div>
  )
}

function MetricBlock({ label, value, color = 'text-neutral-100' }: { label: string; value: string; color?: string }) {
  return (
    <div className="rounded-lg bg-black/30 border border-[#161616] px-3 py-2">
      <div className="text-[8px] text-neutral-600 uppercase tracking-[0.12em] font-mono mb-1">{label}</div>
      <div className={`text-sm font-mono tabular-nums ${color}`}>{value}</div>
    </div>
  )
}

function SummaryItem({ label, value, color = 'text-neutral-100' }: { label: string; value: string; color?: string }) {
  return (
    <div className="rounded-lg bg-black/30 border border-[#161616] px-3 py-2">
      <div className="text-[8px] text-neutral-600 uppercase tracking-[0.12em] font-mono mb-1">{label}</div>
      <div className={`text-sm md:text-base font-mono tabular-nums ${color}`}>{value}</div>
    </div>
  )
}

function InfoLine({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-3 py-1.5 border-b border-[#141414] last:border-b-0">
      <span className="text-[9px] text-neutral-600 uppercase tracking-[0.12em]">{label}</span>
      <span className="text-neutral-300 tabular-nums">{value}</span>
    </div>
  )
}

// ===== Wallet Page =====
import { OverviewTab } from './wallet/OverviewTab'
import { WalletTab } from './wallet/WalletTab'
import { StrategyTab } from './wallet/StrategyTab'
import { PolyTab as PolymarketTab } from './wallet/PolyTab'

const WALLET_TABS = [
  { id: 'overview', label: 'Overview', icon: <BarChart3 size={14} /> },
  { id: 'virtual', label: 'Wallet', icon: <Wallet size={14} /> },
  { id: 'strategy', label: 'Strategy', icon: <Settings size={14} /> },
  { id: 'polymarket', label: 'Polymarket', icon: <Globe size={14} /> },
]

function WalletPage() {
  const [tab, setTab] = useState('overview')

  return (
    <div className="flex flex-col min-h-0 flex-1">
      <TabBar tabs={WALLET_TABS} active={tab} onChange={setTab} />
      <div className="flex-1 min-h-0 overflow-y-auto p-3 md:p-4">
        <div className="max-w-6xl mx-auto">
          {tab === 'overview' && <OverviewTab />}
          {tab === 'virtual' && <WalletTab />}
          {tab === 'strategy' && <StrategyTab />}
          {tab === 'polymarket' && <PolymarketTab />}
        </div>
      </div>
    </div>
  )
}

// ===== Main App =====
function App() {
  const [view, setView] = useState(() => {
    const params = new URLSearchParams(window.location.search)
    return params.get('view') || ''
  })

  useEffect(() => {
    const handler = () => {
      const params = new URLSearchParams(window.location.search)
      setView(params.get('view') || '')
    }
    window.addEventListener('popstate', handler)
    return () => window.removeEventListener('popstate', handler)
  }, [])

  const navigate = useCallback((newView: string) => {
    const url = newView ? `${window.location.pathname}?view=${newView}` : window.location.pathname
    window.history.pushState({}, '', url)
    setView(newView)
  }, [])

  return (
    <div className="h-screen bg-black text-neutral-200 flex flex-col overflow-hidden">
      {/* Header */}
      <header className="shrink-0 border-b border-[#1a1a1a] px-4 py-2 flex items-center gap-2 relative">
        {/* Nav tabs */}
        <button
          onClick={() => navigate('')}
          className={`px-3 py-1.5 text-[10px] font-mono uppercase tracking-[0.08em] transition-all rounded ${
            view === ''
              ? 'text-amber-400 bg-amber-500/5'
              : 'text-neutral-600 hover:text-neutral-400'
          }`}
        >
          Dashboard
        </button>
        <button
          onClick={() => navigate('wallet')}
          className={`px-3 py-1.5 text-[10px] font-mono uppercase tracking-[0.08em] transition-all rounded ${
            view === 'wallet'
              ? 'text-amber-400 bg-amber-500/5'
              : 'text-neutral-600 hover:text-neutral-400'
          }`}
        >
          Wallet
        </button>

        <div className="w-px h-4 bg-[#1a1a1a] mx-1" />

        {/* Title */}
        <h1 className="text-[10px] font-bold text-neutral-300 uppercase tracking-[0.2em] font-mono hidden sm:block">
          WENBOT
        </h1>

        <div className="flex-1" />

        {/* Clock */}
        <LiveClock />
      </header>

      {/* Content */}
      <main className="flex-1 min-h-0 overflow-y-auto">
        {view === 'wallet' ? <WalletPage /> : <Dashboard />}
      </main>

      {/* Footer */}
      <footer className="shrink-0 border-t border-[#111] px-4 py-1 flex items-center justify-between">
        <span className="text-[8px] text-neutral-800 font-mono hidden sm:block">
          Binance · Polymarket · Open-Meteo
        </span>
        <span className="text-[8px] text-neutral-800 font-mono sm:hidden">WENBOT</span>
        <div className="flex items-center gap-1.5">
          <div className="w-1 h-1 rounded-full bg-green-500" />
          <span className="text-[8px] text-neutral-700 font-mono">Connected</span>
        </div>
      </footer>
    </div>
  )
}

export default App

import { useMemo, useState } from 'react'
import { ConfidenceBar, formatPercent, QueryState, LiveClock } from './ui'
import {
  useFivesbotStatus,
  useEthFivesbotStatus,
  useWalletBalance,
  useWallet2Balance,
  useWallet3Balance,
  useWallet4Balance,
  useWeatherBalance,
  usePolyBalance,
  useWalletHistory,
  useWallet2History,
  useWallet3History,
  useWallet4History,
  useWeatherHistory,
} from './hooks'
import { TrendingUp, TrendingDown } from 'lucide-react'
import { ResponsiveContainer, LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip } from 'recharts'

type CurvePoint = {
  label: string
  w1: number
  w2: number
  w3: number
  w4: number
  w5: number
  total: number
  pnl_total: number
}

const INITIALS = { w1: 100, w2: 100, w3: 100, w4: 100, w5: 100 }

function WalletCard({ title, tag, color, data }: { title: string; tag: string; color: string; data?: any }) {
  return (
    <div className="bg-[#0a0a0a] border border-[#1a1a1a] rounded-lg p-4">
      <div className="flex items-center justify-between mb-3">
        <span className="text-[9px] text-neutral-600 uppercase tracking-[0.12em] font-mono">{title}</span>
        <span className={`px-1.5 py-0.5 text-[8px] font-bold uppercase font-mono border rounded ${color}`}>{tag}</span>
      </div>
      <div className="text-xl font-bold font-mono tabular-nums text-neutral-100 tracking-tight">${data?.balance?.toFixed?.(2) ?? '—'}</div>
      <div className="text-[9px] text-neutral-600 font-mono mt-1">Equity ${data?.total_value?.toFixed?.(2) ?? '0.00'}</div>
      <div className="flex gap-3 mt-3 pt-3 border-t border-[#1a1a1a] text-[10px] font-mono">
        <span className={data && data.total_pnl >= 0 ? 'text-green-400' : 'text-red-400'}>
          PnL {data ? `${data.total_pnl >= 0 ? '+' : ''}$${data.total_pnl.toFixed(2)}` : '—'}
        </span>
        <span className="text-neutral-500">{data?.total_trades ?? 0} trades</span>
        <span className="text-neutral-500">{data ? `${data.win_rate.toFixed(1)}%` : '—'} WR</span>
      </div>
    </div>
  )
}

export function OverviewTab() {
  const fivesbot = useFivesbotStatus()
  const ethFivesbot = useEthFivesbotStatus()
  const wallet = useWalletBalance()
  const wallet2 = useWallet2Balance()
  const wallet3 = useWallet3Balance()
  const wallet4 = useWallet4Balance()
  const weather = useWeatherBalance()
  const poly = usePolyBalance()
  const walletHistory = useWalletHistory(200)
  const wallet2History = useWallet2History(200)
  const wallet3History = useWallet3History(200)
  const wallet4History = useWallet4History(200)
  const weatherHistory = useWeatherHistory(200)
  const [curveMode, setCurveMode] = useState<'equity' | 'pnl'>('equity')

  const btcSignal = fivesbot.data?.markets?.btc?.predictor
  const ethSignal = ethFivesbot.data?.markets?.eth?.predictor

  const equityCurve = useMemo<CurvePoint[]>(() => {
    const events = [
      ...(walletHistory.data ?? []).map(h => ({ t: h.closed_at, k: 'w1' as const, pnl: h.pnl })),
      ...(wallet2History.data ?? []).map(h => ({ t: h.closed_at, k: 'w2' as const, pnl: h.pnl })),
      ...(wallet3History.data ?? []).map(h => ({ t: h.closed_at, k: 'w3' as const, pnl: h.pnl })),
      ...(wallet4History.data ?? []).map(h => ({ t: h.closed_at, k: 'w4' as const, pnl: h.pnl })),
      ...(weatherHistory.data ?? []).map(h => ({ t: h.closed_at, k: 'w5' as const, pnl: h.pnl })),
    ].filter(x => x.t).sort((a, b) => new Date(a.t).getTime() - new Date(b.t).getTime())

    let eq = { ...INITIALS }
    const points: CurvePoint[] = [{ label: 'Start', w1: eq.w1, w2: eq.w2, w3: eq.w3, w4: eq.w4, w5: eq.w5, total: 500, pnl_total: 0 }]
    for (const e of events) {
      eq[e.k] += e.pnl
      points.push({
        label: new Date(e.t).toLocaleString('en-US', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', hour12: false }),
        w1: Number(eq.w1.toFixed(2)),
        w2: Number(eq.w2.toFixed(2)),
        w3: Number(eq.w3.toFixed(2)),
        w4: Number(eq.w4.toFixed(2)),
        w5: Number(eq.w5.toFixed(2)),
        total: Number((eq.w1 + eq.w2 + eq.w3 + eq.w4 + eq.w5).toFixed(2)),
        pnl_total: Number((eq.w1 + eq.w2 + eq.w3 + eq.w4 + eq.w5 - 500).toFixed(2)),
      })
    }
    points.push({
      label: 'Now',
      w1: wallet.data?.total_value ?? eq.w1,
      w2: wallet2.data?.total_value ?? eq.w2,
      w3: wallet3.data?.total_value ?? eq.w3,
      w4: wallet4.data?.total_value ?? eq.w4,
      w5: weather.data?.total_value ?? eq.w5,
      total: (wallet.data?.total_value ?? eq.w1) + (wallet2.data?.total_value ?? eq.w2) + (wallet3.data?.total_value ?? eq.w3) + (wallet4.data?.total_value ?? eq.w4) + (weather.data?.total_value ?? eq.w5),
      pnl_total: (wallet.data?.total_pnl ?? (eq.w1 - 100)) + (wallet2.data?.total_pnl ?? (eq.w2 - 100)) + (wallet3.data?.total_pnl ?? (eq.w3 - 100)) + (wallet4.data?.total_pnl ?? (eq.w4 - 100)) + (weather.data?.total_pnl ?? (eq.w5 - 100)),
    })
    return points
  }, [walletHistory.data, wallet2History.data, wallet3History.data, wallet4History.data, weatherHistory.data, wallet.data, wallet2.data, wallet3.data, wallet4.data, weather.data])

  return (
    <div className="p-3 space-y-3 pb-24">
      <div className="bg-[#0a0a0a] border border-[#1a1a1a] rounded-xl p-4">
        <div className="flex items-center justify-between mb-3">
          <span className="text-[9px] text-neutral-600 uppercase tracking-[0.12em] font-mono">Bot Status · BTC / ETH</span>
          <div className="flex items-center gap-3">
            {fivesbot.data?.uptime && <span className="text-[9px] text-neutral-700 font-mono">{fivesbot.data.uptime}</span>}
            <LiveClock />
          </div>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          <div className="bg-[#060606] border border-[#151515] rounded-lg p-3">
            <div className="flex items-center justify-between mb-2">
              <span className="text-[9px] text-neutral-600 uppercase tracking-[0.12em] font-mono">BTC 15m</span>
              <span className="text-[9px] text-neutral-700 font-mono">{fivesbot.data?.currentCycle ?? '—'}</span>
            </div>
            <div className="text-2xl font-bold tabular-nums text-amber-400 font-mono tracking-tight mb-2">${fivesbot.data?.btcPrice?.toLocaleString(undefined, { maximumFractionDigits: 0 }) ?? '—'}</div>
            {btcSignal && <div className="bg-black/30 border border-[#111] rounded-lg p-2.5"><div className="flex items-center gap-2 flex-wrap mb-2"><span className={`inline-flex items-center gap-1 px-2 py-1 text-[10px] font-bold uppercase border rounded-md font-mono ${btcSignal.signal === 'UP' ? 'bg-green-500/10 text-green-400 border-green-500/20' : 'bg-red-500/10 text-red-400 border-red-500/20'}`}>{btcSignal.signal === 'UP' ? <TrendingUp size={10} /> : <TrendingDown size={10} />}{btcSignal.signal}</span><span className="text-[10px] text-neutral-500 font-mono">Conf {formatPercent(btcSignal.confidence)}</span><span className="text-[10px] text-neutral-500 font-mono">RSI {btcSignal.rsi.toFixed(0)}</span></div><ConfidenceBar value={btcSignal.confidence} /></div>}
          </div>
          <div className="bg-[#060606] border border-[#151515] rounded-lg p-3">
            <div className="flex items-center justify-between mb-2">
              <span className="text-[9px] text-neutral-600 uppercase tracking-[0.12em] font-mono">ETH 15m</span>
              <span className="text-[9px] text-neutral-700 font-mono">{ethFivesbot.data?.currentCycle ?? '—'}</span>
            </div>
            <div className="text-2xl font-bold tabular-nums text-cyan-400 font-mono tracking-tight mb-2">${ethFivesbot.data?.ethPrice?.toLocaleString(undefined, { maximumFractionDigits: 0 }) ?? '—'}</div>
            {ethSignal && <div className="bg-black/30 border border-[#111] rounded-lg p-2.5"><div className="flex items-center gap-2 flex-wrap mb-2"><span className={`inline-flex items-center gap-1 px-2 py-1 text-[10px] font-bold uppercase border rounded-md font-mono ${ethSignal.signal === 'UP' ? 'bg-green-500/10 text-green-400 border-green-500/20' : 'bg-red-500/10 text-red-400 border-red-500/20'}`}>{ethSignal.signal === 'UP' ? <TrendingUp size={10} /> : <TrendingDown size={10} />}{ethSignal.signal}</span><span className="text-[10px] text-neutral-500 font-mono">Conf {formatPercent(ethSignal.confidence)}</span><span className="text-[10px] text-neutral-500 font-mono">RSI {ethSignal.rsi.toFixed(0)}</span></div><ConfidenceBar value={ethSignal.confidence} /></div>}
          </div>
        </div>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-6 gap-3">
        <QueryState isLoading={wallet.isLoading} error={wallet.error}><WalletCard title="Wallet 1 · BTC" tag="W1" color="bg-amber-500/10 text-amber-400 border-amber-500/20" data={wallet.data} /></QueryState>
        <QueryState isLoading={wallet2.isLoading} error={wallet2.error}><WalletCard title="Wallet 2 · ETH" tag="W2" color="bg-violet-500/10 text-violet-400 border-violet-500/20" data={wallet2.data} /></QueryState>
        <QueryState isLoading={wallet3.isLoading} error={wallet3.error}><WalletCard title="Wallet 3 · BTC 8" tag="W3" color="bg-emerald-500/10 text-emerald-400 border-emerald-500/20" data={wallet3.data} /></QueryState>
        <QueryState isLoading={wallet4.isLoading} error={wallet4.error}><WalletCard title="Wallet 4 · BTC 5m" tag="W4" color="bg-blue-500/10 text-blue-400 border-blue-500/20" data={wallet4.data} /></QueryState>
        <QueryState isLoading={weather.isLoading} error={weather.error}><WalletCard title="Wallet 5 · Weather" tag="W5" color="bg-cyan-500/10 text-cyan-400 border-cyan-500/20" data={weather.data} /></QueryState>
        <QueryState isLoading={poly.isLoading} error={poly.error}><div className="bg-[#0a0a0a] border border-[#1a1a1a] rounded-lg p-4"><div className="text-[9px] text-neutral-600 uppercase tracking-[0.12em] font-mono mb-3">Polymarket · Live</div><div className="text-xl font-bold font-mono tabular-nums text-neutral-500 tracking-tight">{poly.data ? `$${(poly.data as Record<string, unknown>).balance ?? '—'}` : '—'}</div></div></QueryState>
      </div>

      <QueryState isLoading={walletHistory.isLoading || wallet2History.isLoading || wallet3History.isLoading || wallet4History.isLoading || weatherHistory.isLoading} error={walletHistory.error || wallet2History.error || wallet3History.error || wallet4History.error || weatherHistory.error}>
        <div className="bg-[#0a0a0a] border border-[#1a1a1a] rounded-xl p-4">
          <div className="flex items-center justify-between mb-3">
            <div className="text-[9px] text-neutral-600 uppercase tracking-[0.12em] font-mono">All Wallets {curveMode === 'equity' ? 'Equity' : 'PnL'} Curve</div>
            <button onClick={() => setCurveMode(curveMode === 'equity' ? 'pnl' : 'equity')} className="px-2 py-1 text-[10px] font-mono border border-[#262626] rounded text-neutral-400">{curveMode === 'equity' ? 'Show PnL' : 'Show Equity'}</button>
          </div>
          <div className="h-72">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={equityCurve}>
                <CartesianGrid stroke="#1a1a1a" strokeDasharray="3 3" />
                <XAxis dataKey="label" stroke="#666" tick={{ fontSize: 10 }} />
                <YAxis stroke="#666" tick={{ fontSize: 10 }} />
                <Tooltip />
                <Line type="monotone" dataKey={curveMode === 'equity' ? 'w1' : undefined} stroke="#f59e0b" dot={false} />
                <Line type="monotone" dataKey={curveMode === 'equity' ? 'w2' : undefined} stroke="#a855f7" dot={false} />
                <Line type="monotone" dataKey={curveMode === 'equity' ? 'w3' : undefined} stroke="#10b981" dot={false} />
                <Line type="monotone" dataKey={curveMode === 'equity' ? 'w4' : undefined} stroke="#3b82f6" dot={false} />
                <Line type="monotone" dataKey={curveMode === 'equity' ? 'w5' : undefined} stroke="#06b6d4" dot={false} />
                <Line type="monotone" dataKey={curveMode === 'equity' ? 'total' : 'pnl_total'} stroke="#fafafa" strokeWidth={2} dot={false} />
              </LineChart>
            </ResponsiveContainer>
          </div>
        </div>
      </QueryState>
    </div>
  )
}

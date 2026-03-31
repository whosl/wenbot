import { useState } from 'react'
import { SectionHeader, MiniTable, QueryState, TabBar } from './ui'
import {
  usePolyBalance,
  usePolyPositions,
  usePolyOrders,
  usePolyTrades,
} from './hooks'
import { Globe, DollarSign, TrendingUp, CreditCard, BarChart3, ArrowUpDown, Clock } from 'lucide-react'

type PolyView = 'balance' | 'positions' | 'orders' | 'trades'

export function PolyTab() {
  const [view, setView] = useState<PolyView>('balance')

  const tabs = [
    { id: 'balance', label: 'Balance', icon: <DollarSign size={12} /> },
    { id: 'positions', label: 'Positions', icon: <TrendingUp size={12} /> },
    { id: 'orders', label: 'Orders', icon: <CreditCard size={12} /> },
    { id: 'trades', label: 'Trades', icon: <BarChart3 size={12} /> },
  ]

  return (
    <div className="flex flex-col h-full">
      <TabBar tabs={tabs} active={view} onChange={(id) => setView(id as PolyView)} />
      <div className="flex-1 overflow-y-auto">
        {view === 'balance' && <PolyBalance />}
        {view === 'positions' && <PolyPositions />}
        {view === 'orders' && <PolyOrders />}
        {view === 'trades' && <PolyTrades />}
      </div>
    </div>
  )
}

function PolyBalance() {
  const { data, isLoading, error } = usePolyBalance()

  return (
    <div className="p-3 space-y-3 pb-24">
      <QueryState isLoading={isLoading} error={error}>
        <div className="bg-[#0a0a0a] border border-blue-500/10 rounded-xl p-5">
          <div className="flex items-center gap-2 mb-4">
            <Globe size={14} className="text-blue-400/60" />
            <span className="text-[9px] text-blue-400/70 uppercase tracking-[0.12em] font-mono">Polymarket · Live (Read-Only)</span>
          </div>
          {data ? (
            <div className="space-y-0 font-mono text-sm">
              {Object.entries(data as Record<string, unknown>).map(([key, value], i) => (
                <div key={key} className={`flex justify-between items-center py-2 ${i > 0 ? 'border-t border-[#1a1a1a]' : ''}`}>
                  <span className="text-[9px] text-neutral-600 uppercase tracking-wider">{key.replace(/_/g, ' ')}</span>
                  <span className="tabular-nums text-neutral-200 text-xs">
                    {typeof value === 'number' ? (value >= 100 ? `$${value.toLocaleString(undefined, { maximumFractionDigits: 2 })}` : value.toFixed(4)) : String(value ?? '—')}
                  </span>
                </div>
              ))}
            </div>
          ) : (
            <div className="text-neutral-700 text-[11px] font-mono py-8 text-center">
              No balance data available
            </div>
          )}
        </div>
      </QueryState>
    </div>
  )
}

function PolyPositions() {
  const { data, isLoading, error } = usePolyPositions()

  return (
    <div className="p-3 pb-24">
      <SectionHeader title="Positions" badge={Array.isArray(data) ? data.length : 0} badgeColor="neutral" icon={<ArrowUpDown size={12} className="text-neutral-600" />} />
      <QueryState isLoading={isLoading} error={error}>
        {data && Array.isArray(data) && data.length > 0 ? (
          <div className="overflow-x-auto bg-[#0a0a0a] border border-[#1a1a1a] rounded-lg overflow-hidden">
            <MiniTable
              columns={[
                { key: 'market', label: 'Market' },
                { key: 'side', label: 'Side' },
                { key: 'size', label: 'Size' },
                { key: 'avg_price', label: 'Avg Price' },
                { key: 'pnl', label: 'PnL' },
              ]}
              data={data as Record<string, unknown>[]}
            />
          </div>
        ) : (
          <div className="py-10 text-center text-[10px] text-neutral-700 font-mono">No positions</div>
        )}
      </QueryState>
    </div>
  )
}

function PolyOrders() {
  const { data, isLoading, error } = usePolyOrders()

  return (
    <div className="p-3 pb-24">
      <SectionHeader title="Open Orders" badge={Array.isArray(data) ? data.length : 0} badgeColor="neutral" icon={<CreditCard size={12} className="text-neutral-600" />} />
      <QueryState isLoading={isLoading} error={error}>
        {data && Array.isArray(data) && data.length > 0 ? (
          <div className="overflow-x-auto bg-[#0a0a0a] border border-[#1a1a1a] rounded-lg overflow-hidden">
            <MiniTable
              columns={[
                { key: 'market', label: 'Market' },
                { key: 'side', label: 'Side' },
                { key: 'price', label: 'Price' },
                { key: 'size', label: 'Size' },
                { key: 'status', label: 'Status' },
              ]}
              data={data as Record<string, unknown>[]}
            />
          </div>
        ) : (
          <div className="py-10 text-center text-[10px] text-neutral-700 font-mono">No open orders</div>
        )}
      </QueryState>
    </div>
  )
}

function PolyTrades() {
  const { data, isLoading, error } = usePolyTrades(50)

  return (
    <div className="p-3 pb-24">
      <SectionHeader title="Trade History" badge={Array.isArray(data) ? data.length : 0} badgeColor="neutral" icon={<Clock size={12} className="text-neutral-600" />} />
      <QueryState isLoading={isLoading} error={error}>
        {data && Array.isArray(data) && data.length > 0 ? (
          <div className="overflow-x-auto bg-[#0a0a0a] border border-[#1a1a1a] rounded-lg overflow-hidden">
            <MiniTable
              columns={[
                { key: 'market', label: 'Market' },
                { key: 'side', label: 'Side' },
                { key: 'price', label: 'Price' },
                { key: 'size', label: 'Size' },
                { key: 'pnl', label: 'PnL' },
                { key: 'timestamp', label: 'Time' },
              ]}
              data={data as Record<string, unknown>[]}
            />
          </div>
        ) : (
          <div className="py-10 text-center text-[10px] text-neutral-700 font-mono">No trades</div>
        )}
      </QueryState>
    </div>
  )
}

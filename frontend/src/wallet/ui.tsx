import { useState, useEffect, type ReactNode } from 'react'
import { RefreshCw, TrendingUp, TrendingDown, Wallet, Activity, BarChart3, Settings, Globe, Clock, ChevronDown, ChevronRight, Plus, CircleDollarSign, ArrowUpDown, Trophy, Percent, Flame, Zap } from 'lucide-react'

// ===== Reusable Icons =====
export const Icons = {
  refresh: <RefreshCw size={12} />,
  up: <TrendingUp size={12} />,
  down: <TrendingDown size={12} />,
  wallet: <Wallet size={12} />,
  activity: <Activity size={12} />,
  chart: <BarChart3 size={12} />,
  settings: <Settings size={12} />,
  globe: <Globe size={12} />,
  clock: <Clock size={12} />,
  deposit: <Plus size={12} />,
  dollar: <CircleDollarSign size={12} />,
  trades: <ArrowUpDown size={12} />,
  trophy: <Trophy size={12} />,
  percent: <Percent size={12} />,
  flame: <Flame size={12} />,
  zap: <Zap size={12} />,
}

// ===== Stat Card =====
interface StatCardProps {
  label: string
  value: string | number
  sub?: string
  color?: string
  icon?: ReactNode
  accentColor?: string
}

export function StatCard({ label, value, sub, color, icon }: StatCardProps) {
  return (
    <div className="bg-[#0a0a0a] border border-[#1a1a1a] rounded-lg p-3 hover:border-[#262626] transition-colors">
      <div className="flex items-center gap-1.5 mb-1.5">
        {icon && <span className="text-neutral-600">{icon}</span>}
        <span className="text-[9px] text-neutral-600 uppercase tracking-[0.12em] font-mono">{label}</span>
      </div>
      <div className={`text-lg font-semibold font-mono tabular-nums leading-tight ${color || 'text-neutral-100'}`}>
        {value}
      </div>
      {sub && (
        <div className="text-[9px] text-neutral-600 font-mono mt-1">{sub}</div>
      )}
    </div>
  )
}

// ===== Section Header =====
interface SectionHeaderProps {
  title: string
  badge?: string | number
  badgeColor?: string
  right?: ReactNode
  icon?: ReactNode
}

export function SectionHeader({ title, badge, badgeColor = 'amber', right, icon }: SectionHeaderProps) {
  const badgeColors: Record<string, string> = {
    amber: 'bg-amber-500/10 text-amber-400 border-amber-500/20',
    cyan: 'bg-cyan-500/10 text-cyan-400 border-cyan-500/20',
    green: 'bg-green-500/10 text-green-400 border-green-500/20',
    red: 'bg-red-500/10 text-red-400 border-red-500/20',
    neutral: 'bg-neutral-500/10 text-neutral-400 border-neutral-500/20',
    blue: 'bg-blue-500/10 text-blue-400 border-blue-500/20',
  }

  return (
    <div className="flex items-center justify-between px-3 py-2.5 border-b border-[#1a1a1a]">
      <div className="flex items-center gap-2">
        {icon && <span className="text-neutral-600">{icon}</span>}
        <span className="text-[9px] text-neutral-500 uppercase tracking-[0.12em] font-mono">{title}</span>
        {badge !== undefined && (
          <span className={`px-1.5 py-0.5 text-[8px] font-bold uppercase border font-mono ${badgeColors[badgeColor] || badgeColors.neutral}`}>
            {badge}
          </span>
        )}
      </div>
      {right}
    </div>
  )
}

// ===== Status Dot =====
interface StatusDotProps {
  online: boolean
  label?: string
}

export function StatusDot({ online, label }: StatusDotProps) {
  return (
    <div className="flex items-center gap-1.5">
      <div className={`w-1.5 h-1.5 rounded-full ${online ? 'bg-green-500 animate-pulse' : 'bg-neutral-700'}`} />
      {label && <span className="text-[9px] text-neutral-600 font-mono">{label}</span>}
    </div>
  )
}

// ===== Mini Table (scrollable) =====
interface MiniTableProps {
  columns: { key: string; label: string; width?: string; align?: string }[]
  data: Record<string, unknown>[]
  emptyText?: string
}

export function MiniTable({ columns, data, emptyText = 'No data' }: MiniTableProps) {
  return (
    <div className="overflow-x-auto -mx-3 px-3">
      <table className="w-full text-[11px] font-mono min-w-0">
        <thead>
          <tr className="text-neutral-600 border-b border-[#1a1a1a]">
            {columns.map(col => (
              <th
                key={col.key}
                className={`py-1.5 px-2 text-[8px] uppercase tracking-[0.12em] whitespace-nowrap text-left font-medium ${col.align === 'right' ? 'text-right' : ''}`}
                style={col.width ? { minWidth: col.width } : undefined}
              >
                {col.label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {data.length === 0 ? (
            <tr>
              <td colSpan={columns.length} className="py-6 text-center text-neutral-700 text-[10px]">
                {emptyText}
              </td>
            </tr>
          ) : (
            data.map((row, i) => (
              <tr key={i} className="border-b border-[#0f0f0f] hover:bg-[#0d0d0d] transition-colors">
                {columns.map(col => (
                  <td
                    key={col.key}
                    className={`py-1.5 px-2 whitespace-nowrap tabular-nums ${col.align === 'right' ? 'text-right' : ''}`}
                  >
                    {renderCell(row[col.key])}
                  </td>
                ))}
              </tr>
            ))
          )}
        </tbody>
      </table>
    </div>
  )
}

function renderCell(value: unknown): ReactNode {
  if (value === null || value === undefined) return <span className="text-neutral-800">—</span>
  // Pass through ReactNode directly (e.g. JSX from cell renderers)
  if (typeof value === 'object') return value as ReactNode
  if (typeof value === 'number') {
    return formatNumber(value)
  }
  if (typeof value === 'boolean') {
    return value ? <span className="text-green-400">✓</span> : <span className="text-red-400">✗</span>
  }
  if (typeof value === 'string') {
    const lower = value.toLowerCase()
    if (lower === 'up' || lower === 'yes' || lower === 'long') return <span className="text-green-400">{value}</span>
    if (lower === 'down' || lower === 'no' || lower === 'short') return <span className="text-red-400">{value}</span>
    if (lower === 'win') return <span className="text-green-400">{value}</span>
    if (lower === 'loss') return <span className="text-red-400">{value}</span>
    if (lower === 'pending') return <span className="text-amber-400">{value}</span>
    if (lower === 'open' || lower === 'active') return <span className="text-cyan-400">{value}</span>
    return <span className="text-neutral-300">{value}</span>
  }
  return String(value)
}

export function formatNumber(n: number): ReactNode {
  if (Math.abs(n) >= 1000) {
    return <span>{n.toLocaleString(undefined, { maximumFractionDigits: 0 })}</span>
  }
  if (Math.abs(n) >= 1) {
    return <span>{n.toFixed(2)}</span>
  }
  return <span>{n.toFixed(4)}</span>
}

export function formatPnl(pnl: number | null | undefined): { text: string; color: string } {
  if (pnl === null || pnl === undefined) return { text: '—', color: 'text-neutral-700' }
  const sign = pnl >= 0 ? '+' : ''
  return {
    text: `${sign}$${pnl.toFixed(2)}`,
    color: pnl >= 0 ? 'text-green-400' : 'text-red-400',
  }
}

export function formatPercent(n: number): string {
  return `${(n * 100).toFixed(1)}%`
}

export function formatPrice(p: number): string {
  return `${(p * 100).toFixed(1)}¢`
}

// ===== Confidence Bar =====
interface ConfidenceBarProps {
  value: number // 0-1
  color?: string
}

export function ConfidenceBar({ value, color }: ConfidenceBarProps) {
  const pct = Math.min(100, Math.max(0, value * 100))
  const barColor = color || (value >= 0.6 ? 'bg-green-500' : value >= 0.4 ? 'bg-amber-500' : 'bg-red-500')
  return (
    <div className="w-full">
      <div className="h-1 bg-[#1a1a1a] rounded-full overflow-hidden">
        <div
          className={`h-full rounded-full transition-all duration-500 ${barColor}`}
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  )
}

// ===== Collapsible Section =====
interface CollapsibleProps {
  title: string
  icon?: ReactNode
  badge?: string | number
  badgeColor?: string
  defaultOpen?: boolean
  children: ReactNode
}

export function Collapsible({ title, icon, badge, badgeColor = 'neutral', defaultOpen = false, children }: CollapsibleProps) {
  const [open, setOpen] = useState(defaultOpen)
  const badgeColors: Record<string, string> = {
    amber: 'bg-amber-500/10 text-amber-400 border-amber-500/20',
    cyan: 'bg-cyan-500/10 text-cyan-400 border-cyan-500/20',
    green: 'bg-green-500/10 text-green-400 border-green-500/20',
    red: 'bg-red-500/10 text-red-400 border-red-500/20',
    neutral: 'bg-neutral-500/10 text-neutral-400 border-neutral-500/20',
  }

  return (
    <div className="bg-[#0a0a0a] border border-[#1a1a1a] rounded-lg overflow-hidden">
      <button
        onClick={() => setOpen(!open)}
        className="w-full flex items-center justify-between px-3 py-2.5 hover:bg-[#0d0d0d] transition-colors"
      >
        <div className="flex items-center gap-2">
          {icon && <span className="text-neutral-600">{icon}</span>}
          <span className="text-[9px] text-neutral-500 uppercase tracking-[0.12em] font-mono">{title}</span>
          {badge !== undefined && (
            <span className={`px-1.5 py-0.5 text-[8px] font-bold uppercase border font-mono ${badgeColors[badgeColor] || badgeColors.neutral}`}>
              {badge}
            </span>
          )}
        </div>
        {open ? <ChevronDown size={12} className="text-neutral-600" /> : <ChevronRight size={12} className="text-neutral-600" />}
      </button>
      {open && <div className="border-t border-[#1a1a1a]">{children}</div>}
    </div>
  )
}

// ===== Deposit Modal =====
interface DepositButtonProps {
  onDeposit: (amount: number) => void
  isPending: boolean
  label?: string
}

export function DepositButton({ onDeposit, isPending, label = 'Deposit' }: DepositButtonProps) {
  const [show, setShow] = useState(false)
  const [amount, setAmount] = useState('50')

  const presets = [25, 50, 100, 200]

  const handleDeposit = () => {
    const num = parseFloat(amount)
    if (num > 0) {
      onDeposit(num)
      setShow(false)
    }
  }

  if (!show) {
    return (
      <button
        onClick={() => setShow(true)}
        disabled={isPending}
        className="flex items-center gap-1.5 px-3 py-2 bg-amber-500/5 border border-amber-500/20 hover:border-amber-500/40 hover:bg-amber-500/10 text-amber-400 text-[10px] uppercase tracking-wider font-mono transition-all rounded disabled:opacity-50"
      >
        <Plus size={11} />
        {isPending ? '...' : label}
      </button>
    )
  }

  return (
    <div className="fixed inset-0 z-50 flex items-end sm:items-center justify-center bg-black/80 backdrop-blur-sm" onClick={() => setShow(false)}>
      <div className="bg-[#0a0a0a] border border-[#262626] rounded-t-xl sm:rounded-xl w-full sm:w-80 p-5 animate-slide-up" onClick={e => e.stopPropagation()}>
        <div className="flex items-center gap-2 mb-4">
          <CircleDollarSign size={14} className="text-amber-400" />
          <span className="text-[10px] text-neutral-400 uppercase tracking-[0.12em] font-mono">Deposit Amount</span>
        </div>
        <div className="grid grid-cols-4 gap-2 mb-4">
          {presets.map(p => (
            <button
              key={p}
              onClick={() => setAmount(String(p))}
              className={`py-2.5 text-xs font-mono border rounded-lg transition-all ${
                amount === String(p)
                  ? 'border-amber-500/50 text-amber-400 bg-amber-500/10 shadow-[0_0_8px_rgba(245,158,11,0.1)]'
                  : 'border-[#262626] text-neutral-500 hover:border-neutral-600 hover:text-neutral-300'
              }`}
            >
              ${p}
            </button>
          ))}
        </div>
        <input
          type="number"
          value={amount}
          onChange={e => setAmount(e.target.value)}
          className="w-full bg-[#111] border border-[#262626] rounded-lg px-4 py-2.5 text-base font-mono text-neutral-100 mb-4 focus:border-amber-500/30 transition-colors"
          placeholder="Custom amount"
          autoFocus
        />
        <div className="flex gap-2">
          <button
            onClick={() => setShow(false)}
            className="flex-1 py-2.5 text-[10px] uppercase tracking-wider font-mono text-neutral-500 border border-[#262626] rounded-lg hover:bg-[#111] transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={handleDeposit}
            className="flex-1 py-2.5 text-[10px] uppercase tracking-wider font-mono text-black bg-amber-400 rounded-lg font-semibold hover:bg-amber-300 transition-colors"
          >
            Confirm
          </button>
        </div>
      </div>
    </div>
  )
}

// ===== Tab Bar =====
interface TabBarProps {
  tabs: { id: string; label: string; badge?: number | string; color?: string; icon?: ReactNode }[]
  active: string
  onChange: (id: string) => void
}

export function TabBar({ tabs, active, onChange }: TabBarProps) {
  return (
    <div className="flex border-b border-[#1a1a1a] overflow-x-auto shrink-0 scrollbar-none">
      {tabs.map(tab => {
        const isActive = tab.id === active
        const activeColor = tab.color === 'cyan' ? 'text-cyan-400 border-cyan-400' :
          tab.color === 'blue' ? 'text-blue-400 border-blue-400' :
          'text-amber-400 border-amber-400'
        return (
          <button
            key={tab.id}
            onClick={() => onChange(tab.id)}
            className={`px-3 py-2.5 text-[10px] font-mono uppercase tracking-[0.08em] whitespace-nowrap border-b-2 transition-all shrink-0 flex items-center gap-1.5 ${
              isActive
                ? `${activeColor} bg-[#0a0a0a]/50`
                : 'border-transparent text-neutral-600 hover:text-neutral-400 hover:bg-[#0a0a0a]/30'
            }`}
          >
            {tab.icon && <span className={isActive ? 'opacity-100' : 'opacity-50'}>{tab.icon}</span>}
            {tab.label}
            {tab.badge !== undefined && (
              <span className={`ml-1 px-1.5 py-0.5 text-[8px] rounded font-mono ${
                isActive ? 'bg-amber-400/10 text-amber-400' : 'bg-neutral-800/50 text-neutral-600'
              }`}>
                {tab.badge}
              </span>
            )}
          </button>
        )
      })}
    </div>
  )
}

// ===== Query Error/Loading =====
interface QueryStateProps {
  isLoading: boolean
  error: unknown
  children: ReactNode
}

export function QueryState({ isLoading, error, children }: QueryStateProps) {
  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-8">
        <div className="flex items-center gap-2">
          <div className="w-3 h-3 border border-neutral-700 border-t-amber-500 rounded-full animate-spin" />
          <span className="text-[10px] text-neutral-600 uppercase tracking-wider font-mono">Loading</span>
        </div>
      </div>
    )
  }
  if (error) {
    return (
      <div className="flex items-center justify-center py-8">
        <div className="text-[10px] text-red-500/60 font-mono">Failed to load</div>
      </div>
    )
  }
  return <>{children}</>
}

// ===== Mobile Card =====
interface MobileCardProps {
  children: ReactNode
}

export function MobileCard({ children }: MobileCardProps) {
  return (
    <div className="bg-[#0a0a0a] border border-[#1a1a1a] rounded-lg p-3 mb-2 hover:border-[#262626] transition-colors">
      {children}
    </div>
  )
}

export function CardRow({ label, value, color }: { label: string; value: ReactNode; color?: string }) {
  return (
    <div className="flex justify-between items-center py-0.5">
      <span className="text-[9px] text-neutral-600 font-mono uppercase tracking-wider">{label}</span>
      <span className={`text-[11px] font-mono tabular-nums ${color || 'text-neutral-300'}`}>{value}</span>
    </div>
  )
}

// ===== Live Clock =====
export function LiveClock() {
  const [time, setTime] = useState(() => new Date())

  useEffect(() => {
    const interval = setInterval(() => setTime(new Date()), 1000)
    return () => clearInterval(interval)
  }, [])

  return (
    <div className="flex items-center gap-1.5">
      <Clock size={10} className="text-neutral-600" />
      <span className="text-[10px] tabular-nums text-neutral-500 font-mono">
        {time.toLocaleTimeString('en-US', { hour12: false })}
      </span>
    </div>
  )
}

// ===== Truncate text =====
export function truncate(str: string, max: number): string {
  if (str.length <= max) return str
  return str.slice(0, max - 1) + '…'
}

// ===== Direction Badge =====
export function DirectionBadge({ direction }: { direction: string }) {
  const d = direction.toLowerCase()
  const isUp = d === 'up' || d === 'yes' || d === 'long'
  const isDown = d === 'down' || d === 'no' || d === 'short'
  return (
    <span className={`inline-flex items-center gap-1 px-1.5 py-0.5 text-[9px] font-bold uppercase border rounded font-mono ${
      isUp ? 'bg-green-500/10 text-green-400 border-green-500/20' :
      isDown ? 'bg-red-500/10 text-red-400 border-red-500/20' :
      'bg-neutral-500/10 text-neutral-400 border-neutral-500/20'
    }`}>
      {isUp && <TrendingUp size={8} />}
      {isDown && <TrendingDown size={8} />}
      {direction}
    </span>
  )
}

// ===== PnL Display =====
export function IndicatorPopover({ details, confidence }: { details: string | null | undefined; confidence: number | null | undefined }) {
  const [open, setOpen] = useState(false)

  if (!details) {
    return <span className="text-neutral-800">—</span>
  }

  let parsed: Record<string, { vote: string; score: number; weight: number }> | null = null
  try { parsed = JSON.parse(details) } catch { return <span>{confidence !== null ? `${(confidence * 100).toFixed(0)}%` : '—'}</span> }

  if (!parsed) return <span>{confidence !== null ? `${(confidence * 100).toFixed(0)}%` : '—'}</span>

  const entries = Object.entries(parsed).filter(([_, v]) => v.weight > 0)
  const confText = confidence !== null ? `${(confidence * 100).toFixed(0)}%` : '—'

  return (
    <div className="relative inline-block">
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center gap-0.5 text-amber-400/80 hover:text-amber-300 transition-colors cursor-pointer"
        title="Click to view indicator details"
      >
        <span>{confText}</span>
        <span className="text-[8px] opacity-60">?</span>
      </button>
      {open && (
        <>
          <div className="fixed inset-0 z-30" onClick={() => setOpen(false)} />
          <div className="absolute right-0 top-full mt-1 z-40 bg-[#111] border border-[#2a2a2a] rounded-lg shadow-xl p-2.5 min-w-[200px]">
            <div className="text-[8px] uppercase tracking-widest text-neutral-600 mb-2">Indicators</div>
            <div className="space-y-1">
              {entries.map(([name, v]) => (
                <div key={name} className="flex items-center justify-between text-[10px] font-mono">
                  <span className="text-neutral-400 capitalize min-w-[80px]">{name.replace(/_/g, ' ')}</span>
                  <div className="flex items-center gap-2">
                    <span className={`w-3.5 text-center text-[8px] font-bold ${v.vote === 'up' ? 'text-green-400' : v.vote === 'down' ? 'text-red-400' : 'text-neutral-600'}`}>
                      {v.vote === 'up' ? '↑' : v.vote === 'down' ? '↓' : '—'}
                    </span>
                    <span className="text-neutral-300 w-10 text-right">{(v.score * 100).toFixed(0)}%</span>
                    <span className="text-neutral-600 w-8 text-right">{(v.weight * 100).toFixed(0)}%</span>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </>
      )}
    </div>
  )
}

export function PnLValue({ value, size = 'text-sm' }: { value: number | null | undefined; size?: string }) {
  if (value === null || value === undefined) return <span className="text-neutral-700">—</span>
  const info = formatPnl(value)
  return <span className={`font-mono tabular-nums font-semibold ${info.color} ${size}`}>{info.text}</span>
}

// ===== Unrealized PnL Block (for mobile position cards) =====
export function UnrealizedPnlBlock({
  entryPrice,
  currentPrice,
  unrealizedPnl,
  currentValue,
  size,
  quantity,
  btcPrice,
}: {
  entryPrice: number
  currentPrice?: number | null
  unrealizedPnl?: number | null
  currentValue?: number | null
  size: number
  quantity: number
  btcPrice?: number
}) {
  const hasLive = currentPrice != null
  const pnl = unrealizedPnl

  return (
    <div className="space-y-1.5">
      {/* PnL highlight */}
      {pnl != null ? (
        <div className={`flex items-center justify-between px-2.5 py-2 rounded-lg border ${
          pnl >= 0
            ? 'bg-green-500/5 border-green-500/10'
            : 'bg-red-500/5 border-red-500/10'
        }`}>
          <span className="text-[8px] text-neutral-600 uppercase tracking-[0.12em] font-mono">Unrealized</span>
          <span className={`text-base font-bold font-mono tabular-nums ${pnl >= 0 ? 'text-green-400' : 'text-red-400'}`}>
            {pnl >= 0 ? '+' : ''}{pnl.toFixed(2)}
          </span>
        </div>
      ) : null}
      {/* Detail grid */}
      <div className="grid grid-cols-2 gap-1">
        <CardRow label="Entry" value={formatPrice(entryPrice)} />
        {hasLive && (
          <CardRow label="Px" value={formatPrice(currentPrice!)} color="text-amber-400" />
        )}
        <CardRow label="Size" value={`$${size.toFixed(2)}`} />
        {currentValue != null && (
          <CardRow label="Val" value={`$${currentValue.toFixed(2)}`} color="text-neutral-200" />
        )}
        <CardRow label="Qty" value={quantity.toFixed(1)} />
        {btcPrice ? (
          <CardRow label="BTC" value={`$${btcPrice.toLocaleString(undefined, { maximumFractionDigits: 0 })}`} />
        ) : null}
      </div>
    </div>
  )
}

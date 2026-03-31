import { SectionHeader, QueryState } from './ui'
import { useBotConfig } from './hooks'
import { Settings, ChevronDown, ChevronRight, Zap, Wind, Info } from 'lucide-react'
import { useState } from 'react'

export function StrategyTab() {
  const { data, isLoading, error } = useBotConfig()

  return (
    <div className="p-3 space-y-3 pb-24">
      <SectionHeader title="Strategy Configuration" badge="Live" badgeColor="green" icon={<Settings size={12} className="text-neutral-600" />} />

      <QueryState isLoading={isLoading} error={error}>
        {data ? (
          <div className="space-y-3">
            {/* BTC Strategy Config */}
            {data.btc_strategy != null && (
              <div className="bg-[#0a0a0a] border border-amber-500/10 rounded-xl overflow-hidden">
                <div className="flex items-center gap-2 px-4 py-3 border-b border-[#1a1a1a]">
                  <Zap size={12} className="text-amber-500/60" />
                  <span className="text-[9px] text-amber-400/70 uppercase tracking-[0.12em] font-mono">BTC 15-Min Strategy</span>
                </div>
                <div className="p-4">
                  <ConfigTree data={data.btc_strategy as Record<string, unknown>} prefix="btc_strategy" depth={0} />
                </div>
              </div>
            )}

            {/* ETH Strategy Config */}
            {data.eth_strategy != null && (
              <div className="bg-[#0a0a0a] border border-violet-500/10 rounded-xl overflow-hidden">
                <div className="flex items-center gap-2 px-4 py-3 border-b border-[#1a1a1a]">
                  <Zap size={12} className="text-violet-500/60" />
                  <span className="text-[9px] text-violet-400/70 uppercase tracking-[0.12em] font-mono">ETH 15-Min Strategy</span>
                </div>
                <div className="p-4">
                  <ConfigTree data={data.eth_strategy as Record<string, unknown>} prefix="eth_strategy" depth={0} />
                </div>
              </div>
            )}

            {/* Weather Strategy Config */}
            {data.weather_strategy != null && (
              <div className="bg-[#0a0a0a] border border-cyan-500/10 rounded-xl overflow-hidden">
                <div className="flex items-center gap-2 px-4 py-3 border-b border-[#1a1a1a]">
                  <Wind size={12} className="text-cyan-500/60" />
                  <span className="text-[9px] text-cyan-400/70 uppercase tracking-[0.12em] font-mono">Weather Temperature Strategy</span>
                </div>
                <div className="p-4">
                  <ConfigTree data={data.weather_strategy as Record<string, unknown>} prefix="weather_strategy" depth={0} />
                </div>
              </div>
            )}

            {/* General Config */}
            {data.general != null && (
              <div className="bg-[#0a0a0a] border border-[#1a1a1a] rounded-xl overflow-hidden">
                <div className="flex items-center gap-2 px-4 py-3 border-b border-[#1a1a1a]">
                  <Info size={12} className="text-neutral-600" />
                  <span className="text-[9px] text-neutral-500 uppercase tracking-[0.12em] font-mono">General</span>
                </div>
                <div className="p-4">
                  <ConfigTree data={data.general as Record<string, unknown>} prefix="general" depth={0} />
                </div>
              </div>
            )}

            {/* Fallback */}
            {(!data.btc_strategy && !data.eth_strategy && !data.weather_strategy && !data.general) && (
              <div className="bg-[#0a0a0a] border border-[#1a1a1a] rounded-xl p-4">
                <div className="flex items-center gap-2 mb-3">
                  <Settings size={12} className="text-neutral-600" />
                  <span className="text-[9px] text-neutral-500 uppercase tracking-[0.12em] font-mono">Bot Configuration</span>
                </div>
                <ConfigTree data={data as Record<string, unknown>} prefix="" depth={0} />
              </div>
            )}
          </div>
        ) : (
          <div className="py-12 text-center text-[10px] text-neutral-700 font-mono">No config available</div>
        )}
      </QueryState>
    </div>
  )
}

// ===== Config tree with collapsible sections =====
function ConfigTree({ data, prefix, depth = 0 }: { data: Record<string, unknown>; prefix?: string; depth?: number }) {
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({})

  const toggle = (key: string) => {
    setCollapsed(prev => ({ ...prev, [key]: !prev[key] }))
  }

  const entries = Object.entries(data)
  const isTopLevel = depth === 0

  return (
    <div className={depth > 0 ? 'ml-3 pl-3 border-l border-[#1a1a1a]' : ''}>
      {entries.map(([key, value]) => {
        const fullKey = prefix ? `${prefix}.${key}` : key

        if (value === null || value === undefined) return null

        if (typeof value === 'object' && !Array.isArray(value)) {
          const isCollapsed = collapsed[fullKey] ?? (isTopLevel ? false : true)
          return (
            <div key={fullKey} className="my-2">
              <button
                onClick={() => toggle(fullKey)}
                className="flex items-center gap-1.5 py-1 text-[10px] text-neutral-400 uppercase tracking-wider font-mono hover:text-neutral-300 transition-colors"
              >
                {isCollapsed ? <ChevronRight size={10} /> : <ChevronDown size={10} />}
                <span>{key.replace(/_/g, ' ')}</span>
                <span className="text-neutral-700 text-[8px]">({Object.keys(value as object).length})</span>
              </button>
              {!isCollapsed && (
                <ConfigTree data={value as Record<string, unknown>} prefix={fullKey} depth={depth + 1} />
              )}
            </div>
          )
        }

        if (Array.isArray(value)) {
          return (
            <div key={fullKey} className="flex justify-between items-center py-0.5 my-0.5">
              <span className="text-[9px] text-neutral-600 font-mono uppercase tracking-wider">{key.replace(/_/g, ' ')}</span>
              <span className="text-[11px] font-mono tabular-nums text-neutral-400">
                [{value.map(v => typeof v === 'string' ? `"${v}"` : String(v)).join(', ')}]
              </span>
            </div>
          )
        }

        const displayValue = typeof value === 'boolean'
          ? (value ? '✓ enabled' : '✗ disabled')
          : String(value)

        const color = typeof value === 'boolean'
          ? (value ? 'text-green-400' : 'text-red-400')
          : 'text-neutral-300'

        return (
          <div key={fullKey} className="flex justify-between items-center py-0.5 my-0.5">
            <span className="text-[9px] text-neutral-600 font-mono uppercase tracking-wider">{key.replace(/_/g, ' ')}</span>
            <span className={`text-[11px] font-mono tabular-nums ${color}`}>{displayValue}</span>
          </div>
        )
      })}
    </div>
  )
}

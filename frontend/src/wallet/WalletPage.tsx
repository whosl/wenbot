import { useState } from 'react'
import { TabBar, LiveClock, StatusDot } from './ui'
import { useFivesbotStatus } from './hooks'
import { OverviewTab } from './OverviewTab'
import { WalletTab } from './WalletTab'
import { PolyTab } from './PolyTab'
import { StrategyTab } from './StrategyTab'

type MainTab = 'overview' | 'wallet' | 'poly' | 'strategy'

export default function WalletPage() {
  const [activeTab, setActiveTab] = useState<MainTab>('overview')
  const fivesbot = useFivesbotStatus()

  const tabs = [
    { id: 'overview', label: 'Overview', icon: <span className="text-xs">📊</span> },
    { id: 'wallet', label: 'Wallet', icon: <span className="text-xs">💰</span> },
    { id: 'poly', label: 'Polymarket', icon: <span className="text-xs">🌐</span> },
    { id: 'strategy', label: 'Strategy', icon: <span className="text-xs">⚙</span> },
  ]

  return (
    <div className="min-h-screen bg-black text-neutral-200 flex flex-col">
      {/* Header */}
      <header className="shrink-0 border-b border-[#1a1a1a] px-4 py-2 flex items-center gap-3">
        <h1 className="text-[10px] font-bold text-neutral-300 uppercase tracking-[0.2em] font-mono whitespace-nowrap">
          WENBOT
        </h1>
        <div className="w-px h-4 bg-[#1a1a1a]" />
        <span className={`px-1.5 py-0.5 text-[8px] font-bold uppercase font-mono ${
          fivesbot.data?.wsConnected
            ? 'bg-green-500/10 text-green-500 border border-green-500/20'
            : 'bg-neutral-800 text-neutral-500 border border-neutral-700'
        }`}>
          {fivesbot.data?.wsConnected ? 'Live' : 'Offline'}
        </span>
        <span className="px-1.5 py-0.5 text-[8px] font-bold uppercase font-mono bg-amber-500/10 text-amber-400 border border-amber-500/20">
          Sim
        </span>

        {/* BTC price in header */}
        {fivesbot.data?.btcPrice && (
          <div className="flex items-center gap-1.5 ml-auto">
            <span className="text-[9px] text-neutral-600 font-mono">BTC</span>
            <span className="text-xs font-bold tabular-nums text-neutral-200 font-mono">
              ${fivesbot.data.btcPrice.toLocaleString(undefined, { maximumFractionDigits: 0 })}
            </span>
          </div>
        )}

        <LiveClock />
      </header>

      {/* Main Tab Bar */}
      <div className="shrink-0 border-b border-[#1a1a1a]">
        <div className="max-w-4xl mx-auto">
          <TabBar
            tabs={tabs}
            active={activeTab}
            onChange={(id) => setActiveTab(id as MainTab)}
          />
        </div>
      </div>

      {/* Content */}
      <main className="flex-1 overflow-y-auto">
        <div className="max-w-4xl mx-auto">
          {activeTab === 'overview' && <OverviewTab />}
          {activeTab === 'wallet' && <WalletTab />}
          {activeTab === 'poly' && <PolyTab />}
          {activeTab === 'strategy' && <StrategyTab />}
        </div>
      </main>

      {/* Footer */}
      <footer className="shrink-0 border-t border-[#111] px-4 py-1.5 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <StatusDot online={fivesbot.data?.wsConnected ?? false} label="WS" />
          <span className="text-[9px] text-neutral-700 font-mono">BTC 5-min + Weather Temp</span>
        </div>
        <span className="text-[9px] text-neutral-700 font-mono">
          {fivesbot.data?.uptime ?? '—'}
        </span>
      </footer>
    </div>
  )
}

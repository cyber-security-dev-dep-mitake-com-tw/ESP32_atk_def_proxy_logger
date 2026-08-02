import { useCallback, useEffect, useState } from 'react'
import { getNodes, getSafety, sendCommand } from './api'
import { useEvents } from './hooks/useEvents'
import { NodeGrid } from './components/NodeGrid'
import { PacketStream } from './components/PacketStream'
import { AlertFeed } from './components/AlertFeed'
import { AttackConsole } from './components/AttackConsole'
import type { NodeStatus, SafetyState } from './types'

export default function App() {
  const [nodes, setNodes] = useState<NodeStatus[]>([])
  const [safety, setSafety] = useState<SafetyState>({ lab_mode: false, allowlist: [] })
  const records = useEvents()

  const refresh = useCallback(async () => {
    setNodes(await getNodes().catch(() => []))
    setSafety(await getSafety().catch(() => ({ lab_mode: false, allowlist: [] })))
  }, [])

  useEffect(() => {
    refresh()
    const t = setInterval(refresh, 1500)
    return () => clearInterval(t)
  }, [refresh])

  const startMonitor = () => sendCommand('node1', { cmd: 'start_monitor' })
  const startDetect = () => sendCommand('node2', { cmd: 'start_deauth_detect', threshold: 5, window_ms: 1000 })

  return (
    <div className="app">
      <header>
        <h1>ESP32 Attack / Defense / Proxy Logger</h1>
        <div className="actions">
          <button onClick={startMonitor}>Start Node1 monitor</button>
          <button onClick={startDetect}>Start Node2 detector</button>
        </div>
      </header>
      <NodeGrid nodes={nodes} />
      <PacketStream records={records} />
      <AlertFeed records={records} />
      <AttackConsole safety={safety} onRefresh={refresh} />
    </div>
  )
}

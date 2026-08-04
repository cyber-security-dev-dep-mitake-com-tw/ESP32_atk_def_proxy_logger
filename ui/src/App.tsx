import { useCallback, useEffect, useState } from 'react'
import { getNodes, getSafety, sendCommand } from './api'
import { useEvents } from './hooks/useEvents'
import { NodeGrid } from './components/NodeGrid'
import { PacketStream } from './components/PacketStream'
import { AlertFeed } from './components/AlertFeed'
import { AttackConsole } from './components/AttackConsole'
import { CaptureReport } from './components/CaptureReport'
import type { NodeStatus, SafetyState } from './types'

export default function App() {
  const [nodes, setNodes] = useState<NodeStatus[]>([])
  const [safety, setSafety] = useState<SafetyState>({ lab_mode: false, allowlist: [] })
  const [cmdErr, setCmdErr] = useState('')
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

  const startMonitor = async () => {
    const err = await sendCommand('node1', { cmd: 'start_monitor' })
    setCmdErr(err ? `Node1: ${err}` : '')
  }
  const startDetect = async () => {
    const err = await sendCommand('node2', {
      cmd: 'start_deauth_detect',
      threshold: 5,
      window_ms: 1000,
    })
    setCmdErr(err ? `Node2: ${err}` : '')
  }

  const node1Connected = nodes.some((n) => n.id === 'node1' && n.connected)

  return (
    <div className="app">
      <header>
        <h1>ESP32 Attack / Defense / Proxy Logger</h1>
        <div className="actions">
          <button onClick={startMonitor}>Start Node1 monitor</button>
          <button onClick={startDetect}>Start Node2 detector</button>
        </div>
        {cmdErr && <p className="warn">{cmdErr}</p>}
      </header>
      <NodeGrid nodes={nodes} />
      <PacketStream records={records} node1Connected={node1Connected} />
      <AlertFeed records={records} />
      <CaptureReport records={records} />
      <AttackConsole safety={safety} onRefresh={refresh} />
    </div>
  )
}

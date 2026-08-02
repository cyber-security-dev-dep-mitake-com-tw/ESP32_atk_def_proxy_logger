// Thin client for the Go agent REST API.
import type { Command, NodeStatus, SafetyState } from './types'

export async function getNodes(): Promise<NodeStatus[]> {
  const r = await fetch('/api/nodes')
  return r.json()
}

export async function getSafety(): Promise<SafetyState> {
  const r = await fetch('/api/safety')
  return r.json()
}

export async function setLabMode(on: boolean): Promise<void> {
  await fetch('/api/safety/labmode', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ on }),
  })
}

// sendCommand returns an error string when the backend refuses (e.g. safety gate),
// or null on success.
export async function sendCommand(nodeId: string, cmd: Command): Promise<string | null> {
  const r = await fetch(`/api/nodes/${nodeId}/command`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(cmd),
  })
  if (r.ok) return null
  const body = await r.json().catch(() => ({ error: `HTTP ${r.status}` }))
  return body.error ?? `HTTP ${r.status}`
}

// Types mirroring the Go backend's proto + api packages.

export interface NodeStatus {
  id: string
  kind: string
  connected: boolean
  last_seen: number
  pps: number
  packets: number
}

export interface Event {
  ev: string
  ts?: number
  ch?: number
  rssi?: number
  len?: number
  raw?: string
  pps?: number
  dropped?: number
  heap?: number
  bssid?: string
  src?: string
  count?: number
  level?: string
  msg?: string
}

export interface Record {
  node_id: string
  event: Event
}

export interface SafetyState {
  lab_mode: boolean
  allowlist: string[]
}

export interface Command {
  cmd: string
  ch?: number
  dwell_ms?: number
  channels?: number[]
  threshold?: number
  window_ms?: number
  type?: string
  bssid?: string
  client?: string
  confirm_own_net?: boolean
}

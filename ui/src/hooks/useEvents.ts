import { useEffect, useRef, useState } from 'react'
import type { Record } from '../types'

// useEvents subscribes to the backend event WebSocket and keeps the most recent
// `max` records in state. Reconnects automatically on drop.
export function useEvents(max = 500): Record[] {
  const [records, setRecords] = useState<Record[]>([])
  const ref = useRef<Record[]>([])

  useEffect(() => {
    let ws: WebSocket | null = null
    let stopped = false
    let retry: ReturnType<typeof setTimeout>

    const connect = () => {
      const proto = location.protocol === 'https:' ? 'wss' : 'ws'
      ws = new WebSocket(`${proto}://${location.host}/api/events`)
      ws.onmessage = (e) => {
        try {
          const rec: Record = JSON.parse(e.data)
          ref.current = [...ref.current, rec].slice(-max)
          setRecords(ref.current)
        } catch {
          /* ignore malformed frames */
        }
      }
      ws.onclose = () => {
        if (!stopped) retry = setTimeout(connect, 1000)
      }
    }
    connect()

    return () => {
      stopped = true
      clearTimeout(retry)
      ws?.close()
    }
  }, [max])

  return records
}

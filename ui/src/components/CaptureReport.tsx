import type { Record } from '../types'

// CaptureReport shows the same rollups as the Julia analysis watcher so the UI
// reflects /data/events.jsonl without waiting for the analysis container logs.
export function CaptureReport({ records }: { records: Record[] }) {
  const packets = records.filter((r) => r.event.ev === 'packet')
  const alerts = records.filter((r) => r.event.ev === 'deauth_alert')

  const channels = new Map<number, number>()
  const rssiVals: number[] = []
  for (const r of packets) {
    const ch = r.event.ch ?? 0
    channels.set(ch, (channels.get(ch) || 0) + 1)
    if (typeof r.event.rssi === 'number') rssiVals.push(r.event.rssi)
  }

  const deauth = new Map<string, number>()
  for (const r of alerts) {
    const b = r.event.bssid || '?'
    deauth.set(b, (deauth.get(b) || 0) + (r.event.count || 1))
  }

  const rssiN = rssiVals.length
  const rssiMean = rssiN ? rssiVals.reduce((a, b) => a + b, 0) / rssiN : 0
  const rssiMin = rssiN ? Math.min(...rssiVals) : 0
  const rssiMax = rssiN ? Math.max(...rssiVals) : 0

  return (
    <section className="capture-report">
      <h2>Capture report</h2>
      <p className="muted">
        Live rollup of the event stream (same metrics as Julia analysis).
      </p>
      <p>
        records: <strong>{records.length}</strong>
      </p>
      <h3>Channel utilization (packets/channel)</h3>
      {channels.size === 0 ? (
        <p className="muted">No Node1 packet events yet (needs USB host agent).</p>
      ) : (
        <ul>
          {[...channels.entries()]
            .sort((a, b) => a[0] - b[0])
            .map(([ch, n]) => (
              <li key={ch}>
                ch{ch}: {n}
              </li>
            ))}
        </ul>
      )}
      <h3>RSSI</h3>
      <p>
        n={rssiN} mean={rssiMean.toFixed(1)} min={rssiMin} max={rssiMax}
      </p>
      <h3>Deauth alerts by BSSID</h3>
      {deauth.size === 0 ? (
        <p className="muted">(none)</p>
      ) : (
        <ul>
          {[...deauth.entries()]
            .sort((a, b) => b[1] - a[1])
            .map(([bssid, n]) => (
              <li key={bssid} className="alert-row">
                {bssid}: {n}
              </li>
            ))}
        </ul>
      )}
    </section>
  )
}

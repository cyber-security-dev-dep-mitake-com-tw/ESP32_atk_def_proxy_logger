import type { Record } from '../types'

// AlertFeed mirrors the Julia analysis `deauth_summary`: total count per BSSID,
// plus the most recent raw alert rows from the live event stream.
export function AlertFeed({ records }: { records: Record[] }) {
  const alerts = records.filter((r) => r.event.ev === 'deauth_alert')
  const recent = alerts.slice(-50).reverse()

  // Same aggregation as analysis/src/ESP32Analysis.jl deauth_summary.
  const byBssid = new Map<string, number>()
  for (const r of alerts) {
    const b = r.event.bssid || '?'
    byBssid.set(b, (byBssid.get(b) || 0) + (r.event.count || 1))
  }
  const summary = [...byBssid.entries()].sort((a, b) => b[1] - a[1])

  return (
    <section>
      <h2>Deauth Alerts (Node2)</h2>
      {alerts.length === 0 && <p className="muted">No alerts.</p>}
      {summary.length > 0 && (
        <div className="alert-summary">
          <h3>By BSSID (session totals)</h3>
          <table>
            <thead>
              <tr>
                <th>BSSID</th>
                <th>Total count</th>
              </tr>
            </thead>
            <tbody>
              {summary.map(([bssid, total]) => (
                <tr key={bssid} className="alert-row">
                  <td>{bssid}</td>
                  <td>{total}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {recent.length > 0 && (
        <>
          <h3>Recent alerts</h3>
          <table>
            <thead>
              <tr>
                <th>BSSID</th>
                <th>Source</th>
                <th>Count</th>
                <th>RSSI</th>
              </tr>
            </thead>
            <tbody>
              {recent.map((r, i) => (
                <tr key={i} className="alert-row">
                  <td>{r.event.bssid}</td>
                  <td>{r.event.src || '—'}</td>
                  <td>{r.event.count}</td>
                  <td>{r.event.rssi ?? '—'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </section>
  )
}

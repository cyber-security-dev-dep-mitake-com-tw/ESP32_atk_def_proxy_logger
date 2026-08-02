import type { Record } from '../types'

// AlertFeed lists deauth alerts from Node2, newest first.
export function AlertFeed({ records }: { records: Record[] }) {
  const alerts = records.filter((r) => r.event.ev === 'deauth_alert').slice(-50).reverse()
  return (
    <section>
      <h2>Deauth Alerts (Node2)</h2>
      {alerts.length === 0 && <p className="muted">No alerts.</p>}
      <table>
        <thead>
          <tr><th>BSSID</th><th>Source</th><th>Count</th><th>RSSI</th></tr>
        </thead>
        <tbody>
          {alerts.map((r, i) => (
            <tr key={i} className="alert-row">
              <td>{r.event.bssid}</td>
              <td>{r.event.src}</td>
              <td>{r.event.count}</td>
              <td>{r.event.rssi}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  )
}

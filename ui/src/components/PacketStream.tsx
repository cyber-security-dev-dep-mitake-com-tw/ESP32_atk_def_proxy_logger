import type { Record } from '../types'

// PacketStream shows recent Node1 packet + stats events and a running rate.
export function PacketStream({ records }: { records: Record[] }) {
  const pkts = records.filter((r) => r.event.ev === 'packet')
  const lastStats = [...records].reverse().find((r) => r.event.ev === 'stats')
  return (
    <section>
      <h2>Packet Monitor (Node1)</h2>
      <p>
        Captured this session: <strong>{pkts.length}</strong>
        {lastStats && <> &middot; live pps: <strong>{lastStats.event.pps}</strong></>}
      </p>
      <div className="stream">
        {pkts.slice(-12).reverse().map((r, i) => (
          <code key={i}>
            ch{r.event.ch} rssi{r.event.rssi} len{r.event.len}
          </code>
        ))}
      </div>
    </section>
  )
}

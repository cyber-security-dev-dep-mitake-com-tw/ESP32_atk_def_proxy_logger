import type { Record } from '../types'

// PacketStream shows Node1 packet + stats only. Other nodes' stats (e.g. Node2
// detector counters) must not appear as "live pps" here.
export function PacketStream({
  records,
  node1Connected,
}: {
  records: Record[]
  node1Connected: boolean
}) {
  const node1 = records.filter((r) => r.node_id === 'node1')
  const pkts = node1.filter((r) => r.event.ev === 'packet')
  const lastStats = [...node1].reverse().find((r) => r.event.ev === 'stats')

  return (
    <section>
      <h2>Packet Monitor (Node1)</h2>
      {!node1Connected && (
        <p className="warn">
          Node1 not connected. On macOS, Docker cannot see USB serial — run the host
          agent: <code>./scripts/run-host-agent.sh</code>
        </p>
      )}
      <p>
        Captured this session: <strong>{pkts.length}</strong>
        {lastStats && (
          <>
            {' '}
            &middot; live pps: <strong>{lastStats.event.pps}</strong>
          </>
        )}
      </p>
      <div className="stream">
        {pkts
          .slice(-12)
          .reverse()
          .map((r, i) => (
            <code key={i}>
              ch{r.event.ch} rssi{r.event.rssi} len{r.event.len}
            </code>
          ))}
      </div>
    </section>
  )
}

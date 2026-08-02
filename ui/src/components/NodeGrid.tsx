import type { NodeStatus } from '../types'

// NodeGrid shows the live status of each connected probe.
export function NodeGrid({ nodes }: { nodes: NodeStatus[] }) {
  return (
    <section>
      <h2>Nodes</h2>
      <div className="grid">
        {nodes.length === 0 && <p className="muted">No nodes connected.</p>}
        {nodes.map((n) => (
          <div key={n.id} className={`card ${n.connected ? 'up' : 'down'}`}>
            <div className="card-title">
              <span className={`dot ${n.connected ? 'green' : 'red'}`} />
              {n.id}
            </div>
            <dl>
              <dt>transport</dt><dd>{n.kind}</dd>
              <dt>pps</dt><dd>{n.pps}</dd>
              <dt>packets</dt><dd>{n.packets}</dd>
            </dl>
          </div>
        ))}
      </div>
    </section>
  )
}

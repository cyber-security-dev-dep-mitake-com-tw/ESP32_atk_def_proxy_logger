import { useState } from 'react'
import { sendCommand, setLabMode } from '../api'
import type { SafetyState } from '../types'

// AttackConsole is the gated Node3 control. Attacks are disabled unless lab mode
// is on and the target BSSID is on the backend allowlist; the backend enforces
// this regardless of the UI, this is just the operator-facing guard rail.
export function AttackConsole({ safety, onRefresh }: { safety: SafetyState; onRefresh: () => void }) {
  const [bssid, setBssid] = useState('')
  const [confirm, setConfirm] = useState(false)
  const [result, setResult] = useState<string>('')

  const toggleLab = async () => {
    await setLabMode(!safety.lab_mode)
    onRefresh()
  }

  const attack = async () => {
    const err = await sendCommand('node3', {
      cmd: 'attack',
      type: 'deauth',
      bssid,
      confirm_own_net: confirm,
    })
    setResult(err ? `Refused: ${err}` : 'Attack command sent.')
    onRefresh()
  }

  return (
    <section className="danger-zone">
      <h2>Lab Attack Console (Node3)</h2>
      <p className="warn">
        Own-network testing only. Deauthing networks you do not own is illegal.
      </p>
      <label className="lab-toggle">
        <input type="checkbox" checked={safety.lab_mode} onChange={toggleLab} />
        Lab mode {safety.lab_mode ? 'ENABLED' : 'disabled'}
      </label>
      <div className="allowlist">
        Allowed BSSIDs: {safety.allowlist.length ? safety.allowlist.join(', ') : '(none configured)'}
      </div>
      <fieldset disabled={!safety.lab_mode}>
        <input
          placeholder="target BSSID (must be on allowlist)"
          value={bssid}
          onChange={(e) => setBssid(e.target.value)}
        />
        <label>
          <input type="checkbox" checked={confirm} onChange={(e) => setConfirm(e.target.checked)} />
          I confirm this is my own network
        </label>
        <button onClick={attack} disabled={!confirm || !bssid}>
          Send deauth test
        </button>
      </fieldset>
      {result && <p className="result">{result}</p>}
    </section>
  )
}

# Safety & Legal

This project includes an **active attack** capability (Node3) intended solely for
testing networks **you own and control** in a lab environment.

## Rules

1. **Only target your own network.** Deauthentication, disassociation, and beacon
   spam against networks you do not own is illegal in most jurisdictions (in the
   US, e.g., 47 U.S.C. § 333; similar laws elsewhere).
2. **Node1 (monitor) and Node2 (detector) are passive** — they only receive. Running
   them is generally lawful, but local regulations on interception may still apply.
3. **Node3 is disabled by default** and gated at three independent layers:
   - **Firmware** (`firmware/node3-attacker`): a compiled-in `OWN_NETWORKS` allowlist;
     TX to any other BSSID is refused in code.
   - **Backend** (`backend/internal/api/safety.go`): the `SafetyGate` refuses every
     `attack` command unless (a) lab mode is enabled, (b) `confirm_own_net` is true,
     and (c) the target BSSID is on the operator-configured `--own-bssids` allowlist.
   - **UI** (`AttackConsole`): controls are hidden until lab mode is on and require an
     explicit "this is my own network" confirmation.

## Configuring the allowlist

- Backend: `agent --own-bssids "AA:BB:CC:DD:EE:FF,11:22:33:44:55:66"`
- Firmware: edit `OWN_NETWORKS` in `firmware/node3-attacker/src/main.rs` before flashing.

Both must include a BSSID before Node3 will transmit to it. There is no override.

## Responsible use

Use this only for authorized security testing, education, and defense. If you are
unsure whether a test is legal, **do not run it.**

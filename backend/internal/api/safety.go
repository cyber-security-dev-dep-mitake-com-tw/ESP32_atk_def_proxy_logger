package api

import (
	"fmt"
	"strings"
	"sync"

	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/proto"
)

// SafetyGate enforces that Node3 attack commands only ever target BSSIDs on an
// operator-configured allowlist of networks they own, and only when lab mode is
// explicitly enabled and the command carries confirm_own_net. This is the last
// line of defense before an attack command is forwarded to hardware.
type SafetyGate struct {
	mu        sync.RWMutex
	labMode   bool
	allowlist map[string]bool // normalized BSSID -> true
}

// NewSafetyGate builds a gate seeded with the given own-network BSSIDs. Lab mode
// starts disabled, so attacks are refused until explicitly enabled.
func NewSafetyGate(ownBSSIDs []string) *SafetyGate {
	g := &SafetyGate{allowlist: map[string]bool{}}
	for _, b := range ownBSSIDs {
		g.allowlist[normalizeBSSID(b)] = true
	}
	return g
}

// SetLabMode toggles whether attack commands are permitted at all.
func (g *SafetyGate) SetLabMode(on bool) {
	g.mu.Lock()
	defer g.mu.Unlock()
	g.labMode = on
}

// LabMode reports whether lab mode is currently enabled.
func (g *SafetyGate) LabMode() bool {
	g.mu.RLock()
	defer g.mu.RUnlock()
	return g.labMode
}

// Allowed returns the normalized allowlist entries.
func (g *SafetyGate) Allowed() []string {
	g.mu.RLock()
	defer g.mu.RUnlock()
	out := make([]string, 0, len(g.allowlist))
	for b := range g.allowlist {
		out = append(out, b)
	}
	return out
}

// Check validates an attack command. It returns a non-nil error describing why the
// command is refused; a nil error means the command may proceed.
func (g *SafetyGate) Check(c proto.Command) error {
	if c.Cmd != proto.CmdAttack {
		return nil // non-attack commands are unrestricted
	}
	g.mu.RLock()
	defer g.mu.RUnlock()

	if !g.labMode {
		return fmt.Errorf("attack refused: lab mode is disabled")
	}
	if !c.ConfirmOwnNet {
		return fmt.Errorf("attack refused: confirm_own_net must be true")
	}
	if c.BSSID == "" {
		return fmt.Errorf("attack refused: target bssid required")
	}
	if !g.allowlist[normalizeBSSID(c.BSSID)] {
		return fmt.Errorf("attack refused: bssid %q not in own-network allowlist", c.BSSID)
	}
	return nil
}

// normalizeBSSID lowercases and strips separators so "AA:BB:CC:DD:EE:FF",
// "aa-bb-cc-dd-ee-ff" and "aabbccddeeff" compare equal.
func normalizeBSSID(b string) string {
	b = strings.ToLower(strings.TrimSpace(b))
	b = strings.NewReplacer(":", "", "-", "", ".", "").Replace(b)
	return b
}

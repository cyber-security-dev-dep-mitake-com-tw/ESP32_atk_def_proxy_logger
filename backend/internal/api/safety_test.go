package api

import (
	"testing"

	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/proto"
)

func attackCmd(bssid string, confirm bool) proto.Command {
	return proto.Command{Cmd: proto.CmdAttack, Type: "deauth", BSSID: bssid, ConfirmOwnNet: confirm}
}

func TestSafetyGate_NonAttackAlwaysAllowed(t *testing.T) {
	g := NewSafetyGate(nil)
	if err := g.Check(proto.Command{Cmd: proto.CmdStartMonitor}); err != nil {
		t.Fatalf("non-attack command should be allowed: %v", err)
	}
}

func TestSafetyGate_RefusesWhenLabModeOff(t *testing.T) {
	g := NewSafetyGate([]string{"AA:BB:CC:DD:EE:FF"})
	if err := g.Check(attackCmd("AA:BB:CC:DD:EE:FF", true)); err == nil {
		t.Fatal("expected refusal when lab mode off")
	}
}

func TestSafetyGate_RefusesWithoutConfirm(t *testing.T) {
	g := NewSafetyGate([]string{"AA:BB:CC:DD:EE:FF"})
	g.SetLabMode(true)
	if err := g.Check(attackCmd("AA:BB:CC:DD:EE:FF", false)); err == nil {
		t.Fatal("expected refusal without confirm_own_net")
	}
}

func TestSafetyGate_RefusesUnknownBSSID(t *testing.T) {
	g := NewSafetyGate([]string{"AA:BB:CC:DD:EE:FF"})
	g.SetLabMode(true)
	if err := g.Check(attackCmd("11:22:33:44:55:66", true)); err == nil {
		t.Fatal("expected refusal for bssid not on allowlist")
	}
}

func TestSafetyGate_AllowsOwnNetwork(t *testing.T) {
	g := NewSafetyGate([]string{"AA:BB:CC:DD:EE:FF"})
	g.SetLabMode(true)
	// Different separator/case must still match via normalization.
	if err := g.Check(attackCmd("aa-bb-cc-dd-ee-ff", true)); err != nil {
		t.Fatalf("own-network attack should be allowed: %v", err)
	}
}

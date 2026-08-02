package api

import (
	"encoding/base64"
	"path/filepath"
	"testing"
	"time"

	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/pcap"
	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/proto"
	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/store"
	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/transport"
)

func newTestHub(t *testing.T) (*Hub, *pcap.Writer) {
	t.Helper()
	dir := t.TempDir()
	st, err := store.New(filepath.Join(dir, "events.jsonl"), 100)
	if err != nil {
		t.Fatalf("store: %v", err)
	}
	t.Cleanup(func() { st.Close() })
	pw, err := pcap.NewWriter(filepath.Join(dir, "cap.pcap"))
	if err != nil {
		t.Fatalf("pcap: %v", err)
	}
	t.Cleanup(func() { pw.Close() })
	return NewHub(st, pw, NewSafetyGate(nil)), pw
}

func waitFor(t *testing.T, cond func() bool) {
	t.Helper()
	deadline := time.Now().Add(2 * time.Second)
	for time.Now().Before(deadline) {
		if cond() {
			return
		}
		time.Sleep(5 * time.Millisecond)
	}
	t.Fatal("condition not met before timeout")
}

func TestHub_PacketEventWritesPCAPAndStatus(t *testing.T) {
	hub, pw := newTestHub(t)
	node := transport.NewFakeNode("node1", transport.KindSerial)
	hub.AddNode(node)

	radiotap := []byte{0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0xc0, 0x00}
	node.Inject(proto.Event{
		Ev:  proto.EvPacket,
		Ch:  6,
		Raw: base64.StdEncoding.EncodeToString(radiotap),
	})

	waitFor(t, func() bool { return pw.Count() == 1 })

	var st NodeStatus
	for _, s := range hub.Statuses() {
		if s.ID == "node1" {
			st = s
		}
	}
	if st.Packets != 1 {
		t.Fatalf("status.Packets = %d, want 1", st.Packets)
	}
}

func TestHub_SubscribeReceivesEvents(t *testing.T) {
	hub, _ := newTestHub(t)
	node := transport.NewFakeNode("node2", transport.KindWebSocket)
	hub.AddNode(node)

	ch, unsub := hub.Subscribe()
	defer unsub()

	node.Inject(proto.Event{Ev: proto.EvDeauthAlert, BSSID: "aa:bb", Count: 9})

	select {
	case rec := <-ch:
		if rec.NodeID != "node2" || rec.Event.Count != 9 {
			t.Fatalf("unexpected record: %+v", rec)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("no event received")
	}
}

func TestHub_SendCommandRoutesToNode(t *testing.T) {
	hub, _ := newTestHub(t)
	node := transport.NewFakeNode("node1", transport.KindSerial)
	hub.AddNode(node)

	if err := hub.SendCommand("node1", proto.Command{Cmd: proto.CmdSetChannel, Ch: 11}); err != nil {
		t.Fatalf("send: %v", err)
	}
	waitFor(t, func() bool { return len(node.Sent()) == 1 })
	if node.Sent()[0].Ch != 11 {
		t.Fatalf("wrong command forwarded: %+v", node.Sent()[0])
	}
}

func TestHub_SendCommandUnknownNode(t *testing.T) {
	hub, _ := newTestHub(t)
	if err := hub.SendCommand("ghost", proto.Command{Cmd: proto.CmdGetStats}); err == nil {
		t.Fatal("expected error for unknown node")
	}
}

func TestHub_AttackBlockedBySafetyGate(t *testing.T) {
	dir := t.TempDir()
	st, _ := store.New(filepath.Join(dir, "e.jsonl"), 10)
	defer st.Close()
	hub := NewHub(st, nil, NewSafetyGate([]string{"aa:bb:cc:dd:ee:ff"}))
	node := transport.NewFakeNode("node3", transport.KindWebSocket)
	hub.AddNode(node)

	// Lab mode off => blocked, nothing forwarded.
	err := hub.SendCommand("node3", proto.Command{Cmd: proto.CmdAttack, BSSID: "aa:bb:cc:dd:ee:ff", ConfirmOwnNet: true})
	if err == nil {
		t.Fatal("expected attack to be blocked with lab mode off")
	}
	if len(node.Sent()) != 0 {
		t.Fatal("blocked attack must not reach the node")
	}
}

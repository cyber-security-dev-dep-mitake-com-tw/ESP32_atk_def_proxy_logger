package api

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"testing"

	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/proto"
	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/store"
	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/transport"
)

func newTestServer(t *testing.T, gate *SafetyGate) (*httptest.Server, *Hub) {
	t.Helper()
	st, err := store.New(filepath.Join(t.TempDir(), "e.jsonl"), 10)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { st.Close() })
	hub := NewHub(st, nil, gate)
	srv := httptest.NewServer(NewServer(hub).Handler())
	t.Cleanup(srv.Close)
	return srv, hub
}

func TestHealthEndpoint(t *testing.T) {
	srv, _ := newTestServer(t, NewSafetyGate(nil))
	resp, err := http.Get(srv.URL + "/api/health")
	if err != nil {
		t.Fatal(err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("status = %d", resp.StatusCode)
	}
}

func TestCommandEndpoint(t *testing.T) {
	srv, hub := newTestServer(t, NewSafetyGate(nil))
	node := transport.NewFakeNode("node1", transport.KindSerial)
	hub.AddNode(node)

	body, _ := json.Marshal(proto.Command{Cmd: proto.CmdSetChannel, Ch: 6})
	resp, err := http.Post(srv.URL+"/api/nodes/node1/command", "application/json", bytes.NewReader(body))
	if err != nil {
		t.Fatal(err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("status = %d", resp.StatusCode)
	}
	waitFor(t, func() bool { return len(node.Sent()) == 1 })
}

func TestCommandEndpoint_AttackForbidden(t *testing.T) {
	srv, hub := newTestServer(t, NewSafetyGate([]string{"aa:bb:cc:dd:ee:ff"}))
	node := transport.NewFakeNode("node3", transport.KindWebSocket)
	hub.AddNode(node)

	body, _ := json.Marshal(proto.Command{Cmd: proto.CmdAttack, BSSID: "aa:bb:cc:dd:ee:ff", ConfirmOwnNet: true})
	resp, err := http.Post(srv.URL+"/api/nodes/node3/command", "application/json", bytes.NewReader(body))
	if err != nil {
		t.Fatal(err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusForbidden {
		t.Fatalf("expected 403 with lab mode off, got %d", resp.StatusCode)
	}
}

func TestLabModeToggleThenAttackAllowed(t *testing.T) {
	srv, hub := newTestServer(t, NewSafetyGate([]string{"aa:bb:cc:dd:ee:ff"}))
	node := transport.NewFakeNode("node3", transport.KindWebSocket)
	hub.AddNode(node)

	// Enable lab mode.
	lm, _ := json.Marshal(map[string]bool{"on": true})
	r1, err := http.Post(srv.URL+"/api/safety/labmode", "application/json", bytes.NewReader(lm))
	if err != nil {
		t.Fatal(err)
	}
	r1.Body.Close()

	body, _ := json.Marshal(proto.Command{Cmd: proto.CmdAttack, BSSID: "AA:BB:CC:DD:EE:FF", ConfirmOwnNet: true})
	resp, err := http.Post(srv.URL+"/api/nodes/node3/command", "application/json", bytes.NewReader(body))
	if err != nil {
		t.Fatal(err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("expected attack allowed after lab mode on, got %d", resp.StatusCode)
	}
	waitFor(t, func() bool { return len(node.Sent()) == 1 })
}

func TestNodesEndpoint(t *testing.T) {
	srv, hub := newTestServer(t, NewSafetyGate(nil))
	hub.AddNode(transport.NewFakeNode("node1", transport.KindSerial))

	resp, err := http.Get(srv.URL + "/api/nodes")
	if err != nil {
		t.Fatal(err)
	}
	defer resp.Body.Close()
	var got []NodeStatus
	if err := json.NewDecoder(resp.Body).Decode(&got); err != nil {
		t.Fatal(err)
	}
	if len(got) != 1 || got[0].ID != "node1" {
		t.Fatalf("unexpected nodes: %+v", got)
	}
}

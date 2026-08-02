package api

import (
	"sync"
	"time"

	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/pcap"
	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/proto"
	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/store"
	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/transport"
)

// NodeStatus is the UI-facing snapshot of a node.
type NodeStatus struct {
	ID        string  `json:"id"`
	Kind      string  `json:"kind"`
	Connected bool    `json:"connected"`
	LastSeen  float64 `json:"last_seen"`
	PPS       int     `json:"pps"`
	Packets   int     `json:"packets"`
}

// Hub fans node events out to subscribers, records Node1 packets to PCAP, and
// routes commands from the UI to the right node (through the safety gate).
type Hub struct {
	mu     sync.RWMutex
	nodes  map[string]transport.Node
	status map[string]*NodeStatus

	store *store.Store
	pcap  *pcap.Writer
	gate  *SafetyGate

	subs   map[chan store.Record]struct{}
	subsMu sync.Mutex

	now func() time.Time
}

// NewHub builds a hub. pcapW may be nil (packets are then not recorded).
func NewHub(st *store.Store, pcapW *pcap.Writer, gate *SafetyGate) *Hub {
	return &Hub{
		nodes:  map[string]transport.Node{},
		status: map[string]*NodeStatus{},
		store:  st,
		pcap:   pcapW,
		gate:   gate,
		subs:   map[chan store.Record]struct{}{},
		now:    time.Now,
	}
}

// AddNode registers a node and starts consuming its event stream. The returned
// channel is closed when the node disconnects, so an HTTP handler that owns the
// connection can block on it without stealing events from the consume loop.
func (h *Hub) AddNode(n transport.Node) <-chan struct{} {
	h.mu.Lock()
	h.nodes[n.ID()] = n
	h.status[n.ID()] = &NodeStatus{ID: n.ID(), Kind: string(n.Kind()), Connected: true}
	h.mu.Unlock()
	done := make(chan struct{})
	go h.consume(n, done)
	return done
}

func (h *Hub) consume(n transport.Node, done chan struct{}) {
	defer close(done)
	for ev := range n.Events() {
		h.handleEvent(n.ID(), ev)
	}
	// Channel closed => node disconnected.
	h.mu.Lock()
	if s, ok := h.status[n.ID()]; ok {
		s.Connected = false
	}
	h.mu.Unlock()
}

func (h *Hub) handleEvent(nodeID string, ev proto.Event) {
	nowTs := float64(h.now().UnixNano()) / 1e9

	h.mu.Lock()
	if s, ok := h.status[nodeID]; ok {
		s.LastSeen = nowTs
		switch ev.Ev {
		case proto.EvStats:
			s.PPS = ev.PPS
		case proto.EvPacket:
			s.Packets++
		}
	}
	h.mu.Unlock()

	// Record Node1 packet frames to PCAP.
	if ev.Ev == proto.EvPacket && h.pcap != nil {
		if frame, err := ev.DecodeRaw(); err == nil {
			ts := h.now()
			if ev.Ts > 0 {
				ts = time.Unix(0, int64(ev.Ts*1e9))
			}
			_ = h.pcap.WritePacket(frame, ts)
		}
	}

	rec := store.Record{NodeID: nodeID, Event: ev}
	if h.store != nil {
		_ = h.store.Append(rec)
	}
	h.publish(rec)
}

// SendCommand routes a command to a node, enforcing the safety gate first.
func (h *Hub) SendCommand(nodeID string, c proto.Command) error {
	if err := h.gate.Check(c); err != nil {
		return err
	}
	h.mu.RLock()
	n, ok := h.nodes[nodeID]
	h.mu.RUnlock()
	if !ok {
		return errNodeNotFound(nodeID)
	}
	return n.Send(c)
}

// Statuses returns a snapshot of all node statuses.
func (h *Hub) Statuses() []NodeStatus {
	h.mu.RLock()
	defer h.mu.RUnlock()
	out := make([]NodeStatus, 0, len(h.status))
	for _, s := range h.status {
		out = append(out, *s)
	}
	return out
}

// Subscribe returns a channel of records plus an unsubscribe func.
func (h *Hub) Subscribe() (<-chan store.Record, func()) {
	ch := make(chan store.Record, 256)
	h.subsMu.Lock()
	h.subs[ch] = struct{}{}
	h.subsMu.Unlock()
	return ch, func() {
		h.subsMu.Lock()
		if _, ok := h.subs[ch]; ok {
			delete(h.subs, ch)
			close(ch)
		}
		h.subsMu.Unlock()
	}
}

func (h *Hub) publish(r store.Record) {
	h.subsMu.Lock()
	defer h.subsMu.Unlock()
	for ch := range h.subs {
		select {
		case ch <- r:
		default: // drop for slow subscribers rather than block the event loop
		}
	}
}

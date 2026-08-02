package transport

import (
	"sync"

	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/proto"
)

// FakeNode is an in-memory Node for tests and for the --demo mode of the agent.
// Commands are recorded; events can be injected via Inject.
type FakeNode struct {
	id     string
	kind   Kind
	events chan proto.Event
	mu     sync.Mutex
	sent   []proto.Command
	closed bool
}

// NewFakeNode builds a fake node with the given id and transport kind.
func NewFakeNode(id string, kind Kind) *FakeNode {
	return &FakeNode{id: id, kind: kind, events: make(chan proto.Event, 256)}
}

func (n *FakeNode) ID() string { return n.id }

func (n *FakeNode) Kind() Kind { return n.kind }

func (n *FakeNode) Send(c proto.Command) error {
	n.mu.Lock()
	defer n.mu.Unlock()
	n.sent = append(n.sent, c)
	return nil
}

// Sent returns a copy of every command received.
func (n *FakeNode) Sent() []proto.Command {
	n.mu.Lock()
	defer n.mu.Unlock()
	out := make([]proto.Command, len(n.sent))
	copy(out, n.sent)
	return out
}

// Inject pushes an event as if it came from the node.
func (n *FakeNode) Inject(e proto.Event) {
	n.mu.Lock()
	defer n.mu.Unlock()
	if !n.closed {
		n.events <- e
	}
}

func (n *FakeNode) Events() <-chan proto.Event { return n.events }

func (n *FakeNode) Close() error {
	n.mu.Lock()
	defer n.mu.Unlock()
	if !n.closed {
		n.closed = true
		close(n.events)
	}
	return nil
}

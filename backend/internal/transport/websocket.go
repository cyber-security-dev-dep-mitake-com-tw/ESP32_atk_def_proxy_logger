package transport

import (
	"sync"

	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/proto"
	"github.com/gorilla/websocket"
)

// WSNode speaks NDJSON over a WebSocket. The ESP32 (Node2/Node3) dials into the
// backend, which upgrades the request and constructs this node.
type WSNode struct {
	id     string
	conn   *websocket.Conn
	events chan proto.Event
	mu     sync.Mutex
	once   sync.Once
}

// NewWSNode wraps an upgraded WebSocket connection and starts reading events.
func NewWSNode(id string, conn *websocket.Conn) *WSNode {
	n := &WSNode{
		id:     id,
		conn:   conn,
		events: make(chan proto.Event, 256),
	}
	go n.readLoop()
	return n
}

func (n *WSNode) readLoop() {
	defer close(n.events)
	for {
		_, msg, err := n.conn.ReadMessage()
		if err != nil {
			return
		}
		ev, perr := proto.ParseEvent(msg)
		if perr != nil {
			continue
		}
		n.events <- ev
	}
}

func (n *WSNode) ID() string { return n.id }

func (n *WSNode) Kind() Kind { return KindWebSocket }

func (n *WSNode) Send(c proto.Command) error {
	b, err := proto.MarshalCommand(c)
	if err != nil {
		return err
	}
	n.mu.Lock()
	defer n.mu.Unlock()
	return n.conn.WriteMessage(websocket.TextMessage, b)
}

func (n *WSNode) Events() <-chan proto.Event { return n.events }

func (n *WSNode) Close() error {
	var err error
	n.once.Do(func() { err = n.conn.Close() })
	return err
}

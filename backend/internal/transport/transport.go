// Package transport abstracts the two ways a node connects to the backend: a
// UART/USB serial link (Node1) or an inbound WebSocket (Node2/Node3). Both are
// modeled as a Node that accepts Commands and streams Events.
package transport

import (
	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/proto"
)

// Kind identifies the physical transport of a node.
type Kind string

const (
	KindSerial    Kind = "serial"
	KindWebSocket Kind = "websocket"
)

// Node is a single connected ESP32 probe.
type Node interface {
	// ID is a stable identifier, e.g. "node1".
	ID() string
	// Kind reports the transport type.
	Kind() Kind
	// Send delivers a command to the node.
	Send(proto.Command) error
	// Events returns a channel of events streamed from the node. Closed on disconnect.
	Events() <-chan proto.Event
	// Close terminates the connection.
	Close() error
}

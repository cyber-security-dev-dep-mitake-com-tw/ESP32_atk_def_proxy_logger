package transport

import (
	"bufio"
	"io"
	"sync"

	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/proto"
	"go.bug.st/serial"
)

// SerialNode speaks NDJSON over a UART/USB serial port (used by Node1).
type SerialNode struct {
	id     string
	port   io.ReadWriteCloser
	events chan proto.Event
	mu     sync.Mutex
	once   sync.Once
}

// OpenSerial opens the named serial device at baud and starts reading events.
func OpenSerial(id, name string, baud int) (*SerialNode, error) {
	port, err := serial.Open(name, &serial.Mode{BaudRate: baud})
	if err != nil {
		return nil, err
	}
	return NewSerialNode(id, port), nil
}

// NewSerialNode wraps an already-open read/write/closer (handy for tests via pipes).
func NewSerialNode(id string, rwc io.ReadWriteCloser) *SerialNode {
	n := &SerialNode{
		id:     id,
		port:   rwc,
		events: make(chan proto.Event, 256),
	}
	go n.readLoop()
	return n
}

func (n *SerialNode) readLoop() {
	defer close(n.events)
	sc := bufio.NewScanner(n.port)
	sc.Buffer(make([]byte, 0, 64*1024), 1024*1024)
	for sc.Scan() {
		line := sc.Bytes()
		if len(line) == 0 {
			continue
		}
		ev, err := proto.ParseEvent(line)
		if err != nil {
			continue // skip malformed lines rather than tear down the link
		}
		n.events <- ev
	}
}

func (n *SerialNode) ID() string { return n.id }

func (n *SerialNode) Kind() Kind { return KindSerial }

func (n *SerialNode) Send(c proto.Command) error {
	b, err := proto.MarshalCommand(c)
	if err != nil {
		return err
	}
	n.mu.Lock()
	defer n.mu.Unlock()
	_, err = n.port.Write(b)
	return err
}

func (n *SerialNode) Events() <-chan proto.Event { return n.events }

func (n *SerialNode) Close() error {
	var err error
	n.once.Do(func() { err = n.port.Close() })
	return err
}

package api

import (
	"context"
	"encoding/base64"
	"time"

	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/proto"
	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/transport"
)

// StartDemo registers three fake nodes and drives them with synthetic traffic so
// the full stack (backend, UI, Robot tests) can run without ESP32 hardware. It
// returns when ctx is cancelled.
func StartDemo(ctx context.Context, hub *Hub) {
	n1 := transport.NewFakeNode("node1", transport.KindSerial)
	n2 := transport.NewFakeNode("node2", transport.KindWebSocket)
	n3 := transport.NewFakeNode("node3", transport.KindWebSocket)
	hub.AddNode(n1)
	hub.AddNode(n2)
	hub.AddNode(n3)

	// A minimal but valid radiotap + 802.11 beacon-ish frame.
	frame := []byte{0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00}
	raw := base64.StdEncoding.EncodeToString(frame)

	go func() {
		ticker := time.NewTicker(200 * time.Millisecond)
		defer ticker.Stop()
		i := 0
		for {
			select {
			case <-ctx.Done():
				n1.Close()
				n2.Close()
				n3.Close()
				return
			case <-ticker.C:
				i++
				n1.Inject(proto.Event{Ev: proto.EvPacket, Ch: 6, RSSI: -42, Len: 10, Raw: raw})
				if i%5 == 0 {
					n1.Inject(proto.Event{Ev: proto.EvStats, PPS: 25, Heap: 180000})
				}
				if i%10 == 0 {
					n2.Inject(proto.Event{Ev: proto.EvDeauthAlert, BSSID: "aa:bb:cc:dd:ee:ff",
						Src: "11:22:33:44:55:66", Count: 12, RSSI: -30})
				}
			}
		}
	}()
}

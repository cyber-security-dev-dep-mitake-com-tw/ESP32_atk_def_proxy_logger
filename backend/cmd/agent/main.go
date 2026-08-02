// Command agent is the PC-side control backend for the ESP32 attack/defense/proxy
// logger. It connects to the three nodes (Node1 over serial, Node2/Node3 over
// inbound WebSocket), records Node1 traffic to PCAP, persists events to JSONL, and
// serves a REST + WebSocket API for the React UI.
package main

import (
	"context"
	"flag"
	"log"
	"net/http"
	"os"
	"os/signal"
	"path/filepath"
	"strings"
	"syscall"
	"time"

	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/api"
	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/pcap"
	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/store"
	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/transport"
)

// version is stamped at release time via -ldflags "-X main.version=...".
var version = "dev"

func main() {
	var (
		addr       = flag.String("addr", ":8080", "HTTP listen address")
		dataDir    = flag.String("data", "./data", "directory for pcap + event logs")
		serialPort = flag.String("node1-serial", "", "serial device for Node1 (e.g. /dev/tty.usbserial-0001); empty to skip")
		baud       = flag.Int("baud", 921600, "Node1 serial baud rate")
		ownBSSIDs  = flag.String("own-bssids", "", "comma-separated allowlist of your own BSSIDs for Node3 attacks")
		demo       = flag.Bool("demo", false, "run with synthetic nodes (no hardware)")
	)
	showVersion := flag.Bool("version", false, "print version and exit")
	flag.Parse()

	if *showVersion {
		log.Printf("esp32 control agent %s", version)
		return
	}

	if err := os.MkdirAll(*dataDir, 0o755); err != nil {
		log.Fatalf("data dir: %v", err)
	}

	pcapPath := filepath.Join(*dataDir, "node1-"+time.Now().Format("20060102-150405")+".pcap")
	pw, err := pcap.NewWriter(pcapPath)
	if err != nil {
		log.Fatalf("pcap writer: %v", err)
	}
	defer pw.Close()

	st, err := store.New(filepath.Join(*dataDir, "events.jsonl"), 5000)
	if err != nil {
		log.Fatalf("store: %v", err)
	}
	defer st.Close()

	gate := api.NewSafetyGate(splitCSV(*ownBSSIDs))
	hub := api.NewHub(st, pw, gate)

	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
	defer stop()

	if *demo {
		log.Println("running in DEMO mode with synthetic nodes")
		api.StartDemo(ctx, hub)
	} else if *serialPort != "" {
		node, err := transport.OpenSerial("node1", *serialPort, *baud)
		if err != nil {
			log.Fatalf("open node1 serial: %v", err)
		}
		hub.AddNode(node)
		log.Printf("Node1 connected on %s @ %d baud", *serialPort, *baud)
	}

	srv := &http.Server{Addr: *addr, Handler: api.NewServer(hub).Handler()}
	go func() {
		log.Printf("control API listening on %s (pcap=%s)", *addr, pcapPath)
		if err := srv.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			log.Fatalf("http: %v", err)
		}
	}()

	<-ctx.Done()
	log.Println("shutting down")
	shutCtx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	defer cancel()
	_ = srv.Shutdown(shutCtx)
}

func splitCSV(s string) []string {
	if strings.TrimSpace(s) == "" {
		return nil
	}
	parts := strings.Split(s, ",")
	out := make([]string, 0, len(parts))
	for _, p := range parts {
		if p = strings.TrimSpace(p); p != "" {
			out = append(out, p)
		}
	}
	return out
}

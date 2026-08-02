package api

import (
	"encoding/json"
	"net/http"

	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/proto"
	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/transport"
	"github.com/gorilla/websocket"
)

// Server exposes the hub over HTTP: a REST control API for the UI and two
// WebSocket endpoints — one for the browser to stream events, one for ESP32
// nodes (Node2/Node3) to dial in.
type Server struct {
	hub      *Hub
	upgrader websocket.Upgrader
}

// NewServer builds an HTTP server around a hub.
func NewServer(hub *Hub) *Server {
	return &Server{
		hub: hub,
		upgrader: websocket.Upgrader{
			// Local tool: allow any origin so the Vite dev server can connect.
			CheckOrigin: func(*http.Request) bool { return true },
		},
	}
}

// Handler returns the fully wired HTTP mux.
func (s *Server) Handler() http.Handler {
	mux := http.NewServeMux()
	mux.HandleFunc("GET /api/health", s.handleHealth)
	mux.HandleFunc("GET /api/nodes", s.handleNodes)
	mux.HandleFunc("POST /api/nodes/{id}/command", s.handleCommand)
	mux.HandleFunc("GET /api/safety", s.handleGetSafety)
	mux.HandleFunc("POST /api/safety/labmode", s.handleSetLabMode)
	mux.HandleFunc("GET /api/events", s.handleEventsWS) // browser subscribes
	mux.HandleFunc("GET /ws/node/{id}", s.handleNodeWS) // ESP32 dials in
	return mux
}

func writeJSON(w http.ResponseWriter, code int, v any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(code)
	_ = json.NewEncoder(w).Encode(v)
}

func (s *Server) handleHealth(w http.ResponseWriter, _ *http.Request) {
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

func (s *Server) handleNodes(w http.ResponseWriter, _ *http.Request) {
	writeJSON(w, http.StatusOK, s.hub.Statuses())
}

func (s *Server) handleCommand(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	var c proto.Command
	if err := json.NewDecoder(r.Body).Decode(&c); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid json: " + err.Error()})
		return
	}
	if err := s.hub.SendCommand(id, c); err != nil {
		// A safety refusal is a client error, not a server fault.
		writeJSON(w, http.StatusForbidden, map[string]string{"error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]string{"status": "sent"})
}

func (s *Server) handleGetSafety(w http.ResponseWriter, _ *http.Request) {
	writeJSON(w, http.StatusOK, map[string]any{
		"lab_mode":  s.hub.gate.LabMode(),
		"allowlist": s.hub.gate.Allowed(),
	})
}

func (s *Server) handleSetLabMode(w http.ResponseWriter, r *http.Request) {
	var body struct {
		On bool `json:"on"`
	}
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid json"})
		return
	}
	s.hub.gate.SetLabMode(body.On)
	writeJSON(w, http.StatusOK, map[string]bool{"lab_mode": body.On})
}

// handleEventsWS streams live records to a browser client and backfills recent history.
func (s *Server) handleEventsWS(w http.ResponseWriter, r *http.Request) {
	conn, err := s.upgrader.Upgrade(w, r, nil)
	if err != nil {
		return
	}
	defer conn.Close()

	ch, unsub := s.hub.Subscribe()
	defer unsub()

	// Backfill retained events so a freshly-opened UI is not blank.
	if s.hub.store != nil {
		for _, rec := range s.hub.store.Recent() {
			if conn.WriteJSON(rec) != nil {
				return
			}
		}
	}
	for rec := range ch {
		if conn.WriteJSON(rec) != nil {
			return
		}
	}
}

// handleNodeWS upgrades an inbound ESP32 connection into a WSNode and registers it.
func (s *Server) handleNodeWS(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	conn, err := s.upgrader.Upgrade(w, r, nil)
	if err != nil {
		return
	}
	node := transport.NewWSNode(id, conn)
	done := s.hub.AddNode(node)
	// Block until the node disconnects so the handler (and connection) stays alive.
	<-done
}

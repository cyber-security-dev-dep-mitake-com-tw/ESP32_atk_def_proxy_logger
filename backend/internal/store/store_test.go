package store

import (
	"path/filepath"
	"testing"

	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/proto"
)

func TestAppendAndRing(t *testing.T) {
	p := filepath.Join(t.TempDir(), "events.jsonl")
	s, err := New(p, 2)
	if err != nil {
		t.Fatalf("new: %v", err)
	}
	defer s.Close()

	for i := 0; i < 3; i++ {
		if err := s.Append(Record{NodeID: "node2", Event: proto.Event{Ev: proto.EvLog, Count: i}}); err != nil {
			t.Fatalf("append: %v", err)
		}
	}
	r := s.Recent()
	if len(r) != 2 {
		t.Fatalf("ring len = %d, want 2 (bounded)", len(r))
	}
	// Oldest retained should be the second insert (count 1), newest count 2.
	if r[0].Event.Count != 1 || r[1].Event.Count != 2 {
		t.Fatalf("ring contents unexpected: %+v", r)
	}
}

// Package store persists node events to an append-only JSONL file and keeps an
// in-memory ring of the most recent events for the UI to backfill on connect.
package store

import (
	"bufio"
	"encoding/json"
	"os"
	"sync"

	"github.com/dennislee/esp32-atk-def-proxy-logger/backend/internal/proto"
)

// Record is one stored event annotated with the node it came from.
type Record struct {
	NodeID string      `json:"node_id"`
	Event  proto.Event `json:"event"`
}

// Store appends records to disk and retains the last ringSize in memory.
type Store struct {
	mu       sync.Mutex
	f        *os.File
	w        *bufio.Writer
	ring     []Record
	ringSize int
}

// New opens (creating/appending) a JSONL file and retains ringSize recent records.
func New(path string, ringSize int) (*Store, error) {
	f, err := os.OpenFile(path, os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o644)
	if err != nil {
		return nil, err
	}
	return &Store{
		f:        f,
		w:        bufio.NewWriter(f),
		ring:     make([]Record, 0, ringSize),
		ringSize: ringSize,
	}, nil
}

// Append writes a record to disk and the in-memory ring.
func (s *Store) Append(r Record) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	b, err := json.Marshal(r)
	if err != nil {
		return err
	}
	if _, err := s.w.Write(append(b, '\n')); err != nil {
		return err
	}
	if err := s.w.Flush(); err != nil {
		return err
	}
	if len(s.ring) >= s.ringSize {
		s.ring = s.ring[1:]
	}
	s.ring = append(s.ring, r)
	return nil
}

// Recent returns a copy of the retained records, oldest first.
func (s *Store) Recent() []Record {
	s.mu.Lock()
	defer s.mu.Unlock()
	out := make([]Record, len(s.ring))
	copy(out, s.ring)
	return out
}

// Close flushes and closes the file.
func (s *Store) Close() error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if err := s.w.Flush(); err != nil {
		return err
	}
	return s.f.Close()
}

package pcap

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/google/gopacket/pcapgo"
)

func TestWriteAndReadBack(t *testing.T) {
	dir := t.TempDir()
	p := filepath.Join(dir, "cap.pcap")

	w, err := NewWriter(p)
	if err != nil {
		t.Fatalf("new writer: %v", err)
	}
	// Minimal radiotap header (8 bytes: version, pad, len=8, present=0) + dummy 802.11.
	frame := []byte{0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0xc0, 0x00}
	if err := w.WritePacket(frame, time.Unix(100, 0)); err != nil {
		t.Fatalf("write: %v", err)
	}
	if err := w.WritePacket(frame, time.Time{}); err != nil {
		t.Fatalf("write zero-ts: %v", err)
	}
	if w.Count() != 2 {
		t.Fatalf("count = %d, want 2", w.Count())
	}
	if err := w.Close(); err != nil {
		t.Fatalf("close: %v", err)
	}

	f, err := os.Open(p)
	if err != nil {
		t.Fatalf("open: %v", err)
	}
	defer f.Close()
	r, err := pcapgo.NewReader(f)
	if err != nil {
		t.Fatalf("reader: %v", err)
	}
	if r.LinkType() != LinkTypeRadiotap {
		t.Fatalf("linktype = %v, want %v", r.LinkType(), LinkTypeRadiotap)
	}
	n := 0
	for {
		_, _, err := r.ReadPacketData()
		if err != nil {
			break
		}
		n++
	}
	if n != 2 {
		t.Fatalf("read back %d packets, want 2", n)
	}
}

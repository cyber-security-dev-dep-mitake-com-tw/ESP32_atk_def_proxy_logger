// Package pcap wraps raw 802.11 frames emitted by Node1 into standard PCAP files
// that open directly in Wireshark. Frames arrive already prefixed with a radiotap
// header synthesized on the ESP32, so the link type is LINKTYPE_IEEE802_11_RADIOTAP.
package pcap

import (
	"os"
	"sync"
	"time"

	"github.com/google/gopacket/layers"
	"github.com/google/gopacket/pcapgo"
)

// LinkTypeRadiotap is the DLT for 802.11 plus radiotap headers (LINKTYPE_IEEE802_11_RADIOTAP = 127).
const LinkTypeRadiotap = layers.LinkTypeIEEE80211Radio

// Writer is a concurrency-safe PCAP file writer.
type Writer struct {
	mu    sync.Mutex
	f     *os.File
	w     *pcapgo.Writer
	path  string
	count int
}

// NewWriter creates (or truncates) a PCAP file at path and writes its global header.
func NewWriter(path string) (*Writer, error) {
	f, err := os.Create(path)
	if err != nil {
		return nil, err
	}
	w := pcapgo.NewWriter(f)
	// snaplen 65535 is plenty for 802.11 management/data frames.
	if err := w.WriteFileHeader(65535, LinkTypeRadiotap); err != nil {
		f.Close()
		return nil, err
	}
	return &Writer{f: f, w: w, path: path}, nil
}

// WritePacket appends one frame captured at ts. If ts is the zero time, now is used.
func (w *Writer) WritePacket(frame []byte, ts time.Time) error {
	w.mu.Lock()
	defer w.mu.Unlock()
	if ts.IsZero() {
		ts = time.Now()
	}
	err := w.w.WritePacket(newCI(ts, len(frame)), frame)
	if err == nil {
		w.count++
	}
	return err
}

// Count returns the number of packets written so far.
func (w *Writer) Count() int {
	w.mu.Lock()
	defer w.mu.Unlock()
	return w.count
}

// Path returns the file path being written.
func (w *Writer) Path() string { return w.path }

// Close flushes and closes the underlying file.
func (w *Writer) Close() error {
	w.mu.Lock()
	defer w.mu.Unlock()
	return w.f.Close()
}

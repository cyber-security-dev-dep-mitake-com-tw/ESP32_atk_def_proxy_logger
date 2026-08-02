package pcap

import (
	"time"

	"github.com/google/gopacket"
)

// newCI builds the capture info gopacket needs for each record.
func newCI(ts time.Time, n int) gopacket.CaptureInfo {
	return gopacket.CaptureInfo{
		Timestamp:     ts,
		CaptureLength: n,
		Length:        n,
	}
}

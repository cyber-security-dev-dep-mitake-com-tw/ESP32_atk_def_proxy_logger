// Package proto defines the unified NDJSON command/event protocol spoken by every
// ESP32 node, regardless of whether the underlying transport is UART or WebSocket.
package proto

import (
	"encoding/base64"
	"encoding/json"
	"fmt"
)

// Command is a message sent from the PC to a node. Only the fields relevant to the
// given Cmd are populated; the rest are omitted on the wire.
type Command struct {
	Cmd string `json:"cmd"`

	// set_channel
	Ch int `json:"ch,omitempty"`

	// start_hop
	DwellMs  int   `json:"dwell_ms,omitempty"`
	Channels []int `json:"channels,omitempty"`

	// start_deauth_detect
	Threshold int `json:"threshold,omitempty"`
	WindowMs  int `json:"window_ms,omitempty"`

	// attack (Node3)
	Type          string `json:"type,omitempty"`
	BSSID         string `json:"bssid,omitempty"`
	Client        string `json:"client,omitempty"`
	ConfirmOwnNet bool   `json:"confirm_own_net,omitempty"`
}

// Known command verbs.
const (
	CmdSetChannel        = "set_channel"
	CmdStartMonitor      = "start_monitor"
	CmdStopMonitor       = "stop_monitor"
	CmdStartHop          = "start_hop"
	CmdStopHop           = "stop_hop"
	CmdGetStats          = "get_stats"
	CmdStartDeauthDetect = "start_deauth_detect"
	CmdStopDeauthDetect  = "stop_deauth_detect"
	CmdAttack            = "attack"
)

// Event is a message emitted by a node toward the PC.
type Event struct {
	Ev string  `json:"ev"`
	Ts float64 `json:"ts,omitempty"`

	// packet
	Ch   int    `json:"ch,omitempty"`
	RSSI int    `json:"rssi,omitempty"`
	Len  int    `json:"len,omitempty"`
	Raw  string `json:"raw,omitempty"` // base64 radiotap + 802.11

	// stats
	PPS     int `json:"pps,omitempty"`
	Dropped int `json:"dropped,omitempty"`
	Heap    int `json:"heap,omitempty"`

	// deauth_alert
	BSSID string `json:"bssid,omitempty"`
	Src   string `json:"src,omitempty"`
	Count int    `json:"count,omitempty"`

	// log
	Level string `json:"level,omitempty"`
	Msg   string `json:"msg,omitempty"`
}

// Known event verbs.
const (
	EvPacket      = "packet"
	EvStats       = "stats"
	EvDeauthAlert = "deauth_alert"
	EvLog         = "log"
)

// DecodeRaw returns the decoded binary frame carried in a packet event.
func (e Event) DecodeRaw() ([]byte, error) {
	if e.Raw == "" {
		return nil, fmt.Errorf("proto: event has no raw payload")
	}
	return base64.StdEncoding.DecodeString(e.Raw)
}

// MarshalCommand serializes a command to a single NDJSON line (newline included).
func MarshalCommand(c Command) ([]byte, error) {
	b, err := json.Marshal(c)
	if err != nil {
		return nil, err
	}
	return append(b, '\n'), nil
}

// ParseEvent decodes a single NDJSON line into an Event.
func ParseEvent(line []byte) (Event, error) {
	var e Event
	if err := json.Unmarshal(line, &e); err != nil {
		return Event{}, fmt.Errorf("proto: parse event: %w", err)
	}
	if e.Ev == "" {
		return Event{}, fmt.Errorf("proto: missing ev field")
	}
	return e, nil
}

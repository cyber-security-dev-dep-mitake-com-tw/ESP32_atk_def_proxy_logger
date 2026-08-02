package proto

import "testing"

func TestMarshalCommand(t *testing.T) {
	b, err := MarshalCommand(Command{Cmd: CmdSetChannel, Ch: 6})
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	if b[len(b)-1] != '\n' {
		t.Fatalf("expected trailing newline, got %q", b)
	}
	want := `{"cmd":"set_channel","ch":6}` + "\n"
	if string(b) != want {
		t.Fatalf("got %q want %q", b, want)
	}
}

func TestParseEvent(t *testing.T) {
	e, err := ParseEvent([]byte(`{"ev":"deauth_alert","bssid":"aa:bb","count":12}`))
	if err != nil {
		t.Fatalf("parse: %v", err)
	}
	if e.Ev != EvDeauthAlert || e.BSSID != "aa:bb" || e.Count != 12 {
		t.Fatalf("unexpected event: %+v", e)
	}
}

func TestParseEventMissingEv(t *testing.T) {
	if _, err := ParseEvent([]byte(`{"count":1}`)); err == nil {
		t.Fatal("expected error for missing ev")
	}
}

func TestDecodeRaw(t *testing.T) {
	// "hi" base64 == "aGk="
	e := Event{Ev: EvPacket, Raw: "aGk="}
	b, err := e.DecodeRaw()
	if err != nil {
		t.Fatalf("decode: %v", err)
	}
	if string(b) != "hi" {
		t.Fatalf("got %q", b)
	}
}

package conformance_test

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"runtime"
	"testing"

	"github.com/babelforce/rtvbp/sdk/go"
	v1 "github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
	"github.com/babelforce/rtvbp/sdk/go/envelope/v1classic"
)

type vectorCase struct {
	Name  string `json:"name"`
	JSON  string `json:"json"`
	Error string `json:"error"`
}

type payloadSide struct {
	Schema  string       `json:"schema"`
	Valid   []vectorCase `json:"valid"`
	Invalid []vectorCase `json:"invalid"`
}

type payloadVector struct {
	Method   string      `json:"method"`
	Request  payloadSide `json:"request"`
	Response payloadSide `json:"response"`
}

func TestPayloadVectors(t *testing.T) {
	paths, err := filepath.Glob(vectorPath(t, "payloads", "*.json"))
	if err != nil || len(paths) == 0 {
		t.Fatalf("payload vectors: paths=%v error=%v", paths, err)
	}
	for _, path := range paths {
		var vector payloadVector
		readVector(t, path, &vector)
		for _, side := range []struct {
			name  string
			cases payloadSide
		}{{"request", vector.Request}, {"response", vector.Response}} {
			t.Run(vector.Method+"/"+side.name, func(t *testing.T) {
				factory := payloadFactory(t, vector.Method, side.name)
				for _, sample := range side.cases.Valid {
					t.Run("valid/"+sample.Name, func(t *testing.T) {
						value := factory()
						if err := json.Unmarshal([]byte(sample.JSON), value); err != nil {
							t.Fatalf("decode: %v", err)
						}
						validatePayload(t, value, false)
						encoded, err := json.Marshal(value)
						if err != nil {
							t.Fatalf("encode: %v", err)
						}
						if string(encoded) != sample.JSON {
							t.Fatalf("encoded bytes\n got: %s\nwant: %s", encoded, sample.JSON)
						}
					})
				}
				for _, sample := range side.cases.Invalid {
					t.Run("invalid/"+sample.Name, func(t *testing.T) {
						value := factory()
						err := json.Unmarshal([]byte(sample.JSON), value)
						if sample.Error == "decode" {
							if err == nil {
								t.Fatal("decode succeeded, want error")
							}
							return
						}
						if err != nil {
							t.Fatalf("decode: %v", err)
						}
						validatePayload(t, value, true)
					})
				}
			})
		}
	}
}

func validatePayload(t *testing.T, value any, wantError bool) {
	t.Helper()
	validation, ok := value.(rtvbp.Validation)
	if !ok {
		if wantError {
			t.Fatalf("%T has no generated validation", value)
		}
		return
	}
	err := validation.Validate()
	if wantError && err == nil {
		t.Fatal("Validate() succeeded, want error")
	}
	if !wantError && err != nil {
		t.Fatalf("Validate(): %v", err)
	}
}

func payloadFactory(t *testing.T, method, side string) func() any {
	t.Helper()
	var request, response any
	switch method {
	case v1.MethodApplicationMove:
		request, response = v1.ApplicationMoveRequest{}, v1.ApplicationMoveResponse{}
	case v1.MethodAudioBufferClear:
		request, response = v1.AudioBufferClearRequest{}, v1.AudioBufferClearResponse{}
	case v1.MethodCallHangup:
		request, response = v1.CallHangupRequest{}, v1.EmptyResponse{}
	case v1.MethodPing:
		request, response = v1.PingRequest{}, v1.PingResponse{}
	case v1.MethodRecordingStart:
		request, response = v1.RecordingStartRequest{}, v1.RecordingStartResponse{}
	case v1.MethodRecordingStop:
		request, response = v1.RecordingStopRequest{}, v1.EmptyResponse{}
	case v1.MethodSessionGet:
		request, response = v1.SessionGetRequest{}, v1.SessionGetResponse{}
	case v1.MethodSessionInitialize:
		request, response = v1.SessionInitializeRequest{}, v1.SessionInitializeResponse{}
	case v1.MethodSessionSet:
		request, response = v1.SessionSetRequest{}, v1.EmptyResponse{}
	case v1.MethodSessionTerminate:
		request, response = v1.SessionTerminateRequest{}, v1.EmptyResponse{}
	default:
		t.Fatalf("unknown vector method %q", method)
	}
	value := request
	if side == "response" {
		value = response
	}
	typeOf := reflect.TypeOf(value)
	return func() any { return reflect.New(typeOf).Interface() }
}

type envelopeVector struct {
	Envelope string             `json:"envelope"`
	Encode   []envelopeCase     `json:"encode"`
	Decode   []envelopeCase     `json:"decode"`
	Invalid  []invalidFrameCase `json:"invalid"`
}

type envelopeCase struct {
	Name  string    `json:"name"`
	Frame frameSpec `json:"frame"`
	Bytes string    `json:"bytes"`
}

type invalidFrameCase struct {
	Name  string `json:"name"`
	Bytes string `json:"bytes"`
}

type frameSpec struct {
	Kind     string          `json:"kind"`
	ID       string          `json:"id"`
	Method   string          `json:"method"`
	Params   json.RawMessage `json:"params"`
	Response string          `json:"response"`
	Result   json.RawMessage `json:"result"`
	Error    *wireErrorSpec  `json:"error"`
	Event    string          `json:"event"`
	Data     json.RawMessage `json:"data"`
}

type wireErrorSpec struct {
	Code    int             `json:"code"`
	Message string          `json:"message"`
	Data    json.RawMessage `json:"data"`
}

func TestClassicEnvelopeVectors(t *testing.T) {
	var vector envelopeVector
	readVector(t, vectorPath(t, "envelope", "classic.v1", "frames.json"), &vector)
	codec := v1classic.Envelope{}
	if codec.Name() != vector.Envelope {
		t.Fatalf("envelope name = %q, want %q", codec.Name(), vector.Envelope)
	}
	for _, sample := range vector.Encode {
		t.Run("encode/"+sample.Name, func(t *testing.T) {
			encoded, err := codec.Encode(sample.Frame.controlFrame(t))
			if err != nil {
				t.Fatalf("Encode(): %v", err)
			}
			if string(encoded) != sample.Bytes {
				t.Fatalf("encoded bytes\n got: %s\nwant: %s", encoded, sample.Bytes)
			}
		})
	}
	for _, sample := range vector.Decode {
		t.Run("decode/"+sample.Name, func(t *testing.T) {
			decoded, err := codec.Decode([]byte(sample.Bytes))
			if err != nil {
				t.Fatalf("Decode(): %v", err)
			}
			assertFrame(t, decoded, sample.Frame.controlFrame(t))
		})
	}
	for _, sample := range vector.Invalid {
		t.Run("invalid/"+sample.Name, func(t *testing.T) {
			if _, err := codec.Decode([]byte(sample.Bytes)); err == nil {
				t.Fatal("Decode() succeeded, want error")
			}
		})
	}
}

func (spec frameSpec) controlFrame(t *testing.T) rtvbp.ControlFrame {
	t.Helper()
	frame := rtvbp.ControlFrame{ID: spec.ID}
	switch spec.Kind {
	case "request":
		frame.Kind, frame.Method, frame.Payload = rtvbp.KindRequest, spec.Method, compactJSON(t, spec.Params)
	case "response":
		frame.Kind, frame.CorrelID, frame.Payload = rtvbp.KindResponse, spec.Response, compactJSON(t, spec.Result)
		if spec.Error != nil {
			frame.Err = &rtvbp.WireError{Code: spec.Error.Code, Message: spec.Error.Message, Data: compactJSON(t, spec.Error.Data)}
		}
	case "event":
		frame.Kind, frame.Method, frame.Payload = rtvbp.KindEvent, spec.Event, compactJSON(t, spec.Data)
	default:
		t.Fatalf("unknown frame kind %q", spec.Kind)
	}
	return frame
}

func compactJSON(t *testing.T, value json.RawMessage) json.RawMessage {
	t.Helper()
	if len(value) == 0 {
		return nil
	}
	var compacted bytes.Buffer
	if err := json.Compact(&compacted, value); err != nil {
		t.Fatalf("compact JSON %q: %v", value, err)
	}
	return compacted.Bytes()
}

func assertFrame(t *testing.T, got, want rtvbp.ControlFrame) {
	t.Helper()
	if got.Kind != want.Kind || got.ID != want.ID || got.CorrelID != want.CorrelID || got.Method != want.Method {
		t.Fatalf("frame identity = %#v, want %#v", got, want)
	}
	assertJSON(t, got.Payload, want.Payload)
	if (got.Err == nil) != (want.Err == nil) {
		t.Fatalf("frame error = %#v, want %#v", got.Err, want.Err)
	}
	if got.Err != nil {
		if got.Err.Code != want.Err.Code || got.Err.Message != want.Err.Message {
			t.Fatalf("frame error = %#v, want %#v", got.Err, want.Err)
		}
		assertJSON(t, got.Err.Data, want.Err.Data)
	}
}

func assertJSON(t *testing.T, got, want json.RawMessage) {
	t.Helper()
	if len(got) == 0 || len(want) == 0 {
		if len(got) != len(want) {
			t.Fatalf("JSON presence = %q, want %q", got, want)
		}
		return
	}
	var gotValue, wantValue any
	if err := json.Unmarshal(got, &gotValue); err != nil {
		t.Fatalf("decode got JSON %q: %v", got, err)
	}
	if err := json.Unmarshal(want, &wantValue); err != nil {
		t.Fatalf("decode wanted JSON %q: %v", want, err)
	}
	if !reflect.DeepEqual(gotValue, wantValue) {
		t.Fatalf("JSON = %s, want %s", got, want)
	}
}

func vectorPath(t *testing.T, parts ...string) string {
	t.Helper()
	_, source, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("locate conformance harness")
	}
	all := []string{filepath.Dir(source), "..", "..", "..", "conformance", "babelforce.v1"}
	return filepath.Join(append(all, parts...)...)
}

func readVector(t *testing.T, path string, target any) {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read %s: %v", path, err)
	}
	if err := json.Unmarshal(data, target); err != nil {
		t.Fatalf("decode %s: %v", path, err)
	}
}

func describeFrame(frame rtvbp.ControlFrame) string {
	return fmt.Sprintf("kind=%d id=%q response=%q method=%q payload=%s error=%#v", frame.Kind, frame.ID, frame.CorrelID, frame.Method, frame.Payload, frame.Err)
}

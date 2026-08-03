// Command capture-rtvbp-go-v0.40.0 records the deployed protocol's JSON bytes and the exact
// encoding/json spellings at the boundary of the Rust compatibility envelope.
//
// This command is intentionally disposable. It depends on the old rtvbp-go
// module and is not part of any repository-wide build.
package main

import (
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"runtime"

	"github.com/babelforce/rtvbp-go/proto"
	"github.com/babelforce/rtvbp-go/proto/protov1"
)

type fixture struct {
	path  string
	value any
}

type float64Boundary struct {
	Name    string  `json:"name"`
	Bits    string  `json:"bits"`
	JSON    *string `json:"json,omitempty"`
	Rejects bool    `json:"rejects,omitempty"`
}

type namedFloat64 struct {
	name  string
	value float64
}

func main() {
	out := flag.String("out", defaultOutputRoot(), "golden fixture output directory")
	floatBoundariesOut := flag.String(
		"float-boundaries-out",
		defaultFloatBoundaryOutput(),
		"Go float64 boundary authority output path",
	)
	flag.Parse()

	if err := capture(*out); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	if err := captureFloat64Boundaries(*floatBoundariesOut); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func capture(root string) error {
	for _, item := range fixtures() {
		data, err := json.Marshal(item.value)
		if err != nil {
			return fmt.Errorf("marshal %s: %w", item.path, err)
		}

		path := filepath.Join(root, filepath.FromSlash(item.path))
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			return fmt.Errorf("create fixture directory for %s: %w", item.path, err)
		}
		if err := os.WriteFile(path, data, 0o644); err != nil {
			return fmt.Errorf("write %s: %w", item.path, err)
		}
	}
	return nil
}

func captureFloat64Boundaries(path string) error {
	captured := make([]float64Boundary, 0, len(float64BoundaryInputs()))
	for _, input := range float64BoundaryInputs() {
		item := float64Boundary{
			Name: input.name,
			Bits: fmt.Sprintf("%016x", math.Float64bits(input.value)),
		}
		data, err := json.Marshal(input.value)
		if err != nil {
			item.Rejects = true
		} else {
			encoded := string(data)
			item.JSON = &encoded
		}
		captured = append(captured, item)
	}

	data, err := json.Marshal(captured)
	if err != nil {
		return fmt.Errorf("marshal float64 boundary authority: %w", err)
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return fmt.Errorf("create float64 boundary authority directory: %w", err)
	}
	if err := os.WriteFile(path, data, 0o644); err != nil {
		return fmt.Errorf("write float64 boundary authority: %w", err)
	}
	return nil
}

func float64BoundaryInputs() []namedFloat64 {
	return []namedFloat64{
		{name: "positive-zero", value: 0},
		{name: "negative-zero", value: math.Copysign(0, -1)},
		{name: "one-e-minus-seven", value: 1e-7},
		{name: "one-e-minus-six", value: 1e-6},
		{name: "below-one-e-minus-five", value: math.Nextafter(1e-5, 0)},
		{name: "one-e-minus-five", value: 1e-5},
		{name: "canonical-fraction", value: 106.66666666666667},
		{name: "two-to-53", value: math.Exp2(53)},
		{name: "above-two-to-53", value: math.Nextafter(math.Exp2(53), math.Inf(1))},
		{name: "below-two-to-63", value: math.Nextafter(math.Exp2(63), 0)},
		{name: "two-to-63", value: math.Exp2(63)},
		{name: "one-e19", value: 1e19},
		{name: "below-one-e21", value: math.Nextafter(1e21, 0)},
		{name: "one-e21", value: 1e21},
		{name: "not-a-number", value: math.Float64frombits(0x7ff8000000000001)},
		{name: "positive-infinity", value: math.Inf(1)},
	}
}

func fixtures() []fixture {
	codec := protov1.AudioCodec{
		ID:         "L16/8000/1",
		Name:       "L16",
		SampleRate: 8000,
		BitDepth:   16,
		Channels:   1,
	}
	dtmf := &protov1.DTMFEvent{
		Seq:        7,
		PressedAt:  1_700_000_000_000,
		ReleasedAt: 1_700_000_000_120,
		Digit:      "5",
	}

	requestFrame := proto.NewRequest("session.get", nil)
	requestFrame.ID = "request-1"
	requestWithParamsFrame := proto.NewRequest("session.terminate", &protov1.SessionTerminateRequest{Reason: "completed"})
	requestWithParamsFrame.ID = "request-terminate-1"

	okFrame := requestFrame.Ok(&protov1.EmptyResponse{})
	okNoResultFrame := requestFrame.Ok(nil)
	var nilResult *protov1.EmptyResponse
	okNullResultFrame := requestFrame.Ok(nilResult)
	errorFrame := requestFrame.NotOk(&proto.ResponseError{
		Code:    proto.ErrStatusBadRequest,
		Message: "invalid request",
		Data: map[string]any{
			"field":     "reason",
			"retryable": false,
		},
	})
	unknownErrorFrame := requestFrame.NotOk(proto.NewError(proto.ErrUnknown, errors.New("unknown failure")))
	internalErrorFrame := requestFrame.NotOk(proto.ToResponseError(errors.New("internal failure")))
	notImplementedFrame := requestWithParamsFrame.NotOk(proto.NotImplemented("session.terminate is not supported. please use application.move or call.hangup instead"))
	eventFrame := proto.NewEvent("dtmf", dtmf)
	eventFrame.ID = "event-1"
	audioInfoNonzero := mustJSON[protov1.AudioInfoEvent](`{"read":{"bytes":1280,"bytes_per_second":12800,"bytes_total":6400},"write":{"bytes":32,"bytes_per_second":106.66666666666667,"bytes_total":96}}`)

	return []fixture{
		{path: "payloads/application.move.request.json", value: &protov1.ApplicationMoveRequest{Reason: "handoff", ApplicationID: "app-2"}},
		{path: "payloads/application.move.response.json", value: &protov1.ApplicationMoveResponse{NextApplicationID: "app-2"}},
		{path: "payloads/audio.buffer.clear.request.json", value: &protov1.AudioBufferClearRequest{}},
		{path: "payloads/audio.buffer.clear.response.json", value: &protov1.AudioBufferClearResponse{Len: 320}},
		{path: "payloads/call.hangup.request.json", value: &protov1.CallHangupRequest{Reason: "caller"}},
		{path: "payloads/call.hangup.response.json", value: &protov1.EmptyResponse{}},
		{path: "payloads/ping.request.json", value: &protov1.PingRequest{T0: 1_700_000_000_000, RTT: 42, Data: map[string]any{"probe": "canonical"}}},
		{path: "payloads/ping.response.json", value: &protov1.PingResponse{T0: 1_700_000_000_000, T1: 1_700_000_000_010, T2: 1_700_000_000_012, OWD: 5, Data: map[string]any{"probe": "canonical"}}},
		{path: "payloads/recording.start.request.json", value: &protov1.RecordingStartRequest{Tags: []string{"support", "canonical"}}},
		{path: "payloads/recording.start.response.json", value: &protov1.RecordingStartResponse{ID: "recording-1"}},
		{path: "payloads/recording.stop.request.json", value: &protov1.RecordingStopRequest{ID: "recording-1"}},
		{path: "payloads/recording.stop.response.json", value: &protov1.EmptyResponse{}},
		{path: "payloads/session.get.request.json", value: &protov1.SessionGetRequest{Keys: []string{"customer", "attempt"}}},
		{path: "payloads/session.get.response.json", value: map[string]any{"customer": "Ada", "attempt": 2}},
		{path: "payloads/session.initialize.request.json", value: &protov1.SessionInitializeRequest{
			AppInfo:             protov1.AppInfo{ID: "app-1"},
			CallInfo:            protov1.CallInfo{ID: "call-1", SessionID: "session-1", From: "+12025550100", To: "+12025550101"},
			AudioCodecOfferings: []protov1.AudioCodec{codec},
			Metadata:            nil,
		}},
		{path: "payloads/session.initialize.response.json", value: &protov1.SessionInitializeResponse{AudioCodec: nil}},
		{path: "payloads/session.set.request.json", value: &protov1.SessionSetRequest{Data: map[string]any{"attempt": 2, "customer": "Ada"}}},
		{path: "payloads/session.set.response.json", value: &protov1.EmptyResponse{}},
		{path: "payloads/session.terminate.request.json", value: &protov1.SessionTerminateRequest{Reason: "completed"}},
		{path: "payloads/session.terminate.response.json", value: &protov1.EmptyResponse{}},

		{path: "events/audio.info.json", value: &protov1.AudioInfoEvent{}},
		{path: "events/audio.speech.started.json", value: &protov1.AudioSpeechStartedEvent{Origin: "sender"}},
		{path: "events/call.hangup.json", value: &protov1.CallHangupEvent{Reason: "caller"}},
		{path: "events/dtmf.json", value: dtmf},
		{path: "events/session.updated.json", value: &protov1.SessionUpdatedEvent{AudioCodec: &codec}},
		{path: "variants/payloads/application.move.request-empty.json", value: &protov1.ApplicationMoveRequest{}},
		{path: "variants/payloads/application.move.response-no-next.json", value: &protov1.ApplicationMoveResponse{}},
		{path: "variants/payloads/ping.request-no-optionals.json", value: &protov1.PingRequest{T0: 1_700_000_000_000}},
		{path: "variants/payloads/ping.response-no-data.json", value: &protov1.PingResponse{T0: 1_700_000_000_000, T1: 1_700_000_000_010, T2: 1_700_000_000_012, OWD: 5}},
		{path: "variants/payloads/recording.start.request-no-tags.json", value: &protov1.RecordingStartRequest{}},
		{path: "variants/events/audio.info-nonzero.json", value: audioInfoNonzero},
		{path: "variants/events/call.hangup-no-reason.json", value: &protov1.CallHangupEvent{}},

		{path: "envelope/classic.v1/request.json", value: requestFrame},
		{path: "envelope/classic.v1/request-with-params.json", value: requestWithParamsFrame},
		{path: "envelope/classic.v1/response-ok.json", value: okFrame},
		{path: "envelope/classic.v1/response-ok-no-result.json", value: okNoResultFrame},
		{path: "envelope/classic.v1/response-ok-null-result.json", value: okNullResultFrame},
		{path: "envelope/classic.v1/response-error.json", value: errorFrame},
		{path: "envelope/classic.v1/response-error-unknown.json", value: unknownErrorFrame},
		{path: "envelope/classic.v1/response-error-internal.json", value: internalErrorFrame},
		{path: "envelope/classic.v1/response-error-not-implemented.json", value: notImplementedFrame},
		{path: "envelope/classic.v1/event.json", value: eventFrame},
	}
}

func mustJSON[T any](raw string) *T {
	var value T
	if err := json.Unmarshal([]byte(raw), &value); err != nil {
		panic(err)
	}
	return &value
}

func defaultOutputRoot() string {
	_, source, _, ok := runtime.Caller(0)
	if !ok {
		panic("cannot locate capture source")
	}
	return filepath.Clean(filepath.Join(filepath.Dir(source), "..", "..", "babelforce.v1", "golden"))
}

func defaultFloatBoundaryOutput() string {
	return filepath.Join(filepath.Dir(defaultOutputRoot()), "authority", "go-float64-boundaries.json")
}

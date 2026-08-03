// Command capture-rtvbp-go-v0.37.2 records the common v0.37.2 protocol JSON bytes.
//
// This command is intentionally disposable. It depends on the old rtvbp-go
// module and exists only to compare its common wire surface with the frozen
// v0.40.0 authority.
package main

import (
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"os"
	"path/filepath"

	"github.com/babelforce/rtvbp-go/proto"
	"github.com/babelforce/rtvbp-go/proto/protov1"
)

type fixture struct {
	path  string
	value any
}

func main() {
	out := flag.String("out", "", "output directory (required)")
	flag.Parse()
	if *out == "" {
		flag.Usage()
		os.Exit(2)
	}

	if err := capture(*out); err != nil {
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
	requestWithParamsFrame := proto.NewRequest(
		"session.terminate",
		&protov1.SessionTerminateRequest{Reason: "completed"},
	)
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
	unknownErrorFrame := requestFrame.NotOk(
		proto.NewError(proto.ErrUnknown, errors.New("unknown failure")),
	)
	internalErrorFrame := requestFrame.NotOk(
		proto.ToResponseError(errors.New("internal failure")),
	)
	notImplementedFrame := requestWithParamsFrame.NotOk(proto.NotImplemented(
		"session.terminate is not supported. please use application.move or call.hangup instead",
	))
	eventFrame := proto.NewEvent("dtmf", dtmf)
	eventFrame.ID = "event-1"

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

		{path: "events/audio.speech.started.json", value: &protov1.AudioSpeechStartedEvent{Origin: "sender"}},
		{path: "events/call.hangup.json", value: &protov1.CallHangupEvent{Reason: "caller"}},
		{path: "events/dtmf.json", value: dtmf},
		{path: "events/session.updated.json", value: &protov1.SessionUpdatedEvent{AudioCodec: &codec}},

		{path: "variants/payloads/application.move.request-empty.json", value: &protov1.ApplicationMoveRequest{}},
		{path: "variants/payloads/application.move.response-no-next.json", value: &protov1.ApplicationMoveResponse{}},
		{path: "variants/payloads/ping.request-no-optionals.json", value: &protov1.PingRequest{T0: 1_700_000_000_000}},
		{path: "variants/payloads/ping.response-no-data.json", value: &protov1.PingResponse{T0: 1_700_000_000_000, T1: 1_700_000_000_010, T2: 1_700_000_000_012, OWD: 5}},
		{path: "variants/payloads/recording.start.request-no-tags.json", value: &protov1.RecordingStartRequest{}},
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

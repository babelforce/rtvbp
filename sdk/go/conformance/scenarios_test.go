package conformance_test

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"path/filepath"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	v1 "github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
	"github.com/babelforce/rtvbp/sdk/go/envelope/v1classic"
	"github.com/babelforce/rtvbp/sdk/go/transport/memory"
)

type scenario struct {
	Name  string            `json:"name"`
	Roles map[string]string `json:"roles"`
	Cases []scenarioCase    `json:"cases"`
}

type scenarioCase struct {
	Name  string         `json:"name"`
	Steps []scenarioStep `json:"steps"`
}

type scenarioStep struct {
	Kind     string          `json:"kind"`
	From     string          `json:"from"`
	ID       string          `json:"id"`
	Method   string          `json:"method"`
	Params   json.RawMessage `json:"params"`
	Response string          `json:"response"`
	Result   json.RawMessage `json:"result"`
	Error    *wireErrorSpec  `json:"error"`
	Event    string          `json:"event"`
	Data     json.RawMessage `json:"data"`
}

type requestResult struct {
	response rtvbp.Response
	err      error
}

func TestGeneratedScenariosForBothRoles(t *testing.T) {
	paths, err := filepath.Glob(vectorPath(t, "scenarios", "*.json"))
	if err != nil || len(paths) != 4 {
		t.Fatalf("scenario vectors: paths=%v error=%v", paths, err)
	}
	for _, path := range paths {
		var scenario scenario
		readVector(t, path, &scenario)
		for roleName, role := range scenario.Roles {
			if role != "application" && role != "voice" {
				t.Fatalf("scenario %s role %s = %q", scenario.Name, roleName, role)
			}
			for _, testCase := range scenario.Cases {
				t.Run(scenario.Name+"/"+testCase.Name+"/local-"+role, func(t *testing.T) {
					runScenarioCase(t, testCase, roleName, role)
				})
			}
		}
	}
}

func runScenarioCase(t *testing.T, testCase scenarioCase, localName, localRole string) {
	t.Helper()
	local, peer := memory.NewPair()
	handler := &scenarioHandler{
		responses: scenarioResponses(testCase),
		events:    make(chan string, len(testCase.Steps)),
	}
	registrations := make([]any, 0)
	switch localRole {
	case "application":
		registrations = append(registrations, v1.ApplicationHandlers(handler)...)
		registrations = append(registrations, v1.ApplicationEventHandlers(applicationScenarioEvents{handler})...)
	case "voice":
		registrations = append(registrations, v1.VoiceHandlers(handler)...)
		registrations = append(registrations, v1.VoiceEventHandlers(handler)...)
	default:
		t.Fatalf("unknown local role %q", localRole)
	}
	var nextID atomic.Int64
	session := rtvbp.NewSession(
		v1classic.Envelope{},
		rtvbp.WithTransport(local),
		rtvbp.WithHandler(rtvbp.NewHandler(rtvbp.HandlerConfig{}, registrations...)),
		rtvbp.WithIDGenerator(func() string { return fmt.Sprintf("sdk-%d", nextID.Add(1)) }),
		rtvbp.WithRequestTimeout(2*time.Second),
		rtvbp.WithCloseTimeout(2*time.Second),
	)
	done := session.Run(context.Background())
	waitForState(t, session, rtvbp.SessionStateActive)
	t.Cleanup(func() {
		ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
		defer cancel()
		if err := session.Close(ctx); err != nil {
			t.Errorf("close session: %v", err)
		}
		select {
		case err := <-done:
			if err != nil {
				t.Errorf("run session: %v", err)
			}
		case <-ctx.Done():
			t.Error("session did not finish")
		}
	})

	bindings := make(map[string]string)
	pending := make(map[string]<-chan requestResult)
	for _, step := range testCase.Steps {
		localOrigin := step.From == localName
		switch step.Kind {
		case "request":
			if localOrigin {
				request := decodeNamedRequest(t, step.Method, step.Params)
				result := make(chan requestResult, 1)
				requestCtx := testContext(t)
				go func() {
					response, err := session.Request(requestCtx, request)
					result <- requestResult{response: response, err: err}
				}()
				frame := receiveFrame(t, peer)
				if frame.Kind != rtvbp.KindRequest || frame.Method != step.Method {
					t.Fatalf("originated request %s, want method %q", describeFrame(frame), step.Method)
				}
				assertJSON(t, frame.Payload, step.Params)
				bindOriginatedID(t, bindings, step.ID, frame.ID)
				pending[step.ID] = result
			} else {
				id := bindPeerID(t, bindings, step.ID)
				sendFrame(t, peer, rtvbp.ControlFrame{Kind: rtvbp.KindRequest, ID: id, Method: step.Method, Payload: step.Params})
			}
		case "response":
			id := boundID(t, bindings, step.Response)
			if localOrigin {
				frame := receiveFrame(t, peer)
				if frame.Kind != rtvbp.KindResponse || frame.CorrelID != id {
					t.Fatalf("originated response %s, want response %q", describeFrame(frame), id)
				}
				assertScenarioResponse(t, frame.Payload, frame.Err, step)
			} else {
				frame := rtvbp.ControlFrame{Kind: rtvbp.KindResponse, CorrelID: id, Payload: step.Result}
				if step.Error != nil {
					frame.Err = step.Error.controlError()
				}
				sendFrame(t, peer, frame)
				result, ok := pending[step.Response]
				if !ok {
					t.Fatalf("response %q has no local pending request", step.Response)
				}
				outcome := awaitRequest(t, result)
				assertScenarioResponse(t, outcome.response.Payload, outcome.response.Err, step)
				if step.Error == nil && outcome.err != nil {
					t.Fatalf("request error = %v", outcome.err)
				}
				if step.Error != nil {
					var remote *rtvbp.RemoteError
					if !errors.As(outcome.err, &remote) {
						t.Fatalf("request error = %#v, want RemoteError", outcome.err)
					}
				}
				delete(pending, step.Response)
			}
		case "event":
			if localOrigin {
				event := decodeNamedEvent(t, step.Event, step.Data)
				if err := session.EventDispatch(testContext(t), event); err != nil {
					t.Fatalf("dispatch %s: %v", step.Event, err)
				}
				frame := receiveFrame(t, peer)
				if frame.Kind != rtvbp.KindEvent || frame.Method != step.Event {
					t.Fatalf("originated event %s, want event %q", describeFrame(frame), step.Event)
				}
				assertJSON(t, frame.Payload, step.Data)
				bindOriginatedID(t, bindings, step.ID, frame.ID)
			} else {
				id := bindPeerID(t, bindings, step.ID)
				sendFrame(t, peer, rtvbp.ControlFrame{Kind: rtvbp.KindEvent, ID: id, Method: step.Event, Payload: step.Data})
				select {
				case got := <-handler.events:
					if got != step.Event {
						t.Fatalf("handled event = %q, want %q", got, step.Event)
					}
				case <-time.After(3 * time.Second):
					t.Fatalf("handler did not receive %q", step.Event)
				}
			}
		default:
			t.Fatalf("unknown scenario step kind %q", step.Kind)
		}
	}
	if len(pending) != 0 {
		t.Fatalf("unresolved local requests: %v", pending)
	}
}

func scenarioResponses(testCase scenarioCase) map[string]json.RawMessage {
	methods := make(map[string]string)
	responses := make(map[string]json.RawMessage)
	for _, step := range testCase.Steps {
		if step.Kind == "request" {
			methods[step.ID] = step.Method
		}
		if step.Kind == "response" && step.Error == nil {
			responses[methods[step.Response]] = step.Result
		}
	}
	return responses
}

type scenarioHandler struct {
	responses map[string]json.RawMessage
	events    chan string
}

func (handler *scenarioHandler) response(method string, target any) error {
	payload, ok := handler.responses[method]
	if !ok {
		return fmt.Errorf("scenario has no response for %s", method)
	}
	return json.Unmarshal(payload, target)
}

func (handler *scenarioHandler) event(name string) error {
	handler.events <- name
	return nil
}

func (handler *scenarioHandler) ApplicationMove(context.Context, rtvbp.SHC, *v1.ApplicationMoveRequest) (*v1.ApplicationMoveResponse, error) {
	result := new(v1.ApplicationMoveResponse)
	return result, handler.response(v1.MethodApplicationMove, result)
}

func (handler *scenarioHandler) AudioBufferClear(context.Context, rtvbp.SHC, *v1.AudioBufferClearRequest) (*v1.AudioBufferClearResponse, error) {
	result := new(v1.AudioBufferClearResponse)
	return result, handler.response(v1.MethodAudioBufferClear, result)
}

func (handler *scenarioHandler) CallHangup(context.Context, rtvbp.SHC, *v1.CallHangupRequest) (*v1.EmptyResponse, error) {
	result := new(v1.EmptyResponse)
	return result, handler.response(v1.MethodCallHangup, result)
}

func (handler *scenarioHandler) Ping(context.Context, rtvbp.SHC, *v1.PingRequest) (*v1.PingResponse, error) {
	result := new(v1.PingResponse)
	return result, handler.response(v1.MethodPing, result)
}

func (handler *scenarioHandler) RecordingStart(context.Context, rtvbp.SHC, *v1.RecordingStartRequest) (*v1.RecordingStartResponse, error) {
	result := new(v1.RecordingStartResponse)
	return result, handler.response(v1.MethodRecordingStart, result)
}

func (handler *scenarioHandler) RecordingStop(context.Context, rtvbp.SHC, *v1.RecordingStopRequest) (*v1.EmptyResponse, error) {
	result := new(v1.EmptyResponse)
	return result, handler.response(v1.MethodRecordingStop, result)
}

func (handler *scenarioHandler) SessionGet(context.Context, rtvbp.SHC, *v1.SessionGetRequest) (*v1.SessionGetResponse, error) {
	result := new(v1.SessionGetResponse)
	return result, handler.response(v1.MethodSessionGet, result)
}

func (handler *scenarioHandler) SessionInitialize(context.Context, rtvbp.SHC, *v1.SessionInitializeRequest) (*v1.SessionInitializeResponse, error) {
	result := new(v1.SessionInitializeResponse)
	return result, handler.response(v1.MethodSessionInitialize, result)
}

func (handler *scenarioHandler) SessionSet(context.Context, rtvbp.SHC, *v1.SessionSetRequest) (*v1.EmptyResponse, error) {
	result := new(v1.EmptyResponse)
	return result, handler.response(v1.MethodSessionSet, result)
}

func (handler *scenarioHandler) SessionTerminate(context.Context, rtvbp.SHC, *v1.SessionTerminateRequest) (*v1.EmptyResponse, error) {
	result := new(v1.EmptyResponse)
	return result, handler.response(v1.MethodSessionTerminate, result)
}

func (handler *scenarioHandler) AgentToolCall(context.Context, rtvbp.SHC, *v1.AgentToolCallEvent) error {
	return handler.event(v1.EventAgentToolCall)
}

func (handler *scenarioHandler) AudioInfo(context.Context, rtvbp.SHC, *v1.AudioInfoEvent) error {
	return handler.event(v1.EventAudioInfo)
}

func (handler *scenarioHandler) AudioSpeechStarted(context.Context, rtvbp.SHC, *v1.AudioSpeechStartedEvent) error {
	return handler.event(v1.EventAudioSpeechStarted)
}

func (handler *scenarioHandler) Dtmf(context.Context, rtvbp.SHC, *v1.DtmfEvent) error {
	return handler.event(v1.EventDtmf)
}

func (handler *scenarioHandler) InputTranscript(context.Context, rtvbp.SHC, *v1.InputTranscriptEvent) error {
	return handler.event(v1.EventInputTranscript)
}

func (handler *scenarioHandler) OutputTranscriptDelta(context.Context, rtvbp.SHC, *v1.OutputTranscriptDeltaEvent) error {
	return handler.event(v1.EventOutputTranscriptDelta)
}

func (handler *scenarioHandler) OutputTranscriptDone(context.Context, rtvbp.SHC, *v1.OutputTranscriptDoneEvent) error {
	return handler.event(v1.EventOutputTranscriptDone)
}

func (handler *scenarioHandler) SessionUpdated(context.Context, rtvbp.SHC, *v1.SessionUpdatedEvent) error {
	return handler.event(v1.EventSessionUpdated)
}

type applicationScenarioEvents struct{ handler *scenarioHandler }

func (events applicationScenarioEvents) AudioInfo(context.Context, rtvbp.SHC, *v1.AudioInfoEvent) error {
	return events.handler.event(v1.EventAudioInfo)
}

func (events applicationScenarioEvents) CallHangup(context.Context, rtvbp.SHC, *v1.CallHangupEvent) error {
	return events.handler.event(v1.EventCallHangup)
}

func (events applicationScenarioEvents) Dtmf(context.Context, rtvbp.SHC, *v1.DtmfEvent) error {
	return events.handler.event(v1.EventDtmf)
}

func (events applicationScenarioEvents) SessionUpdated(context.Context, rtvbp.SHC, *v1.SessionUpdatedEvent) error {
	return events.handler.event(v1.EventSessionUpdated)
}

func decodeNamedRequest(t *testing.T, method string, payload json.RawMessage) rtvbp.NamedRequest {
	t.Helper()
	var request rtvbp.NamedRequest
	switch method {
	case v1.MethodApplicationMove:
		request = new(v1.ApplicationMoveRequest)
	case v1.MethodAudioBufferClear:
		request = new(v1.AudioBufferClearRequest)
	case v1.MethodCallHangup:
		request = new(v1.CallHangupRequest)
	case v1.MethodPing:
		request = new(v1.PingRequest)
	case v1.MethodRecordingStart:
		request = new(v1.RecordingStartRequest)
	case v1.MethodRecordingStop:
		request = new(v1.RecordingStopRequest)
	case v1.MethodSessionGet:
		request = new(v1.SessionGetRequest)
	case v1.MethodSessionInitialize:
		request = new(v1.SessionInitializeRequest)
	case v1.MethodSessionSet:
		request = new(v1.SessionSetRequest)
	case v1.MethodSessionTerminate:
		request = new(v1.SessionTerminateRequest)
	default:
		t.Fatalf("unknown scenario method %q", method)
	}
	if err := json.Unmarshal(payload, request); err != nil {
		t.Fatalf("decode %s request: %v", method, err)
	}
	return request
}

func decodeNamedEvent(t *testing.T, name string, payload json.RawMessage) rtvbp.NamedEvent {
	t.Helper()
	var event rtvbp.NamedEvent
	switch name {
	case v1.EventAgentToolCall:
		event = new(v1.AgentToolCallEvent)
	case v1.EventAudioInfo:
		event = new(v1.AudioInfoEvent)
	case v1.EventAudioSpeechStarted:
		event = new(v1.AudioSpeechStartedEvent)
	case v1.EventCallHangup:
		event = new(v1.CallHangupEvent)
	case v1.EventDtmf:
		event = new(v1.DtmfEvent)
	case v1.EventInputTranscript:
		event = new(v1.InputTranscriptEvent)
	case v1.EventOutputTranscriptDelta:
		event = new(v1.OutputTranscriptDeltaEvent)
	case v1.EventOutputTranscriptDone:
		event = new(v1.OutputTranscriptDoneEvent)
	case v1.EventSessionUpdated:
		event = new(v1.SessionUpdatedEvent)
	default:
		t.Fatalf("unknown scenario event %q", name)
	}
	if err := json.Unmarshal(payload, event); err != nil {
		t.Fatalf("decode %s event: %v", name, err)
	}
	return event
}

func (spec *wireErrorSpec) controlError() *rtvbp.WireError {
	if spec == nil {
		return nil
	}
	return &rtvbp.WireError{Code: spec.Code, Message: spec.Message, Data: spec.Data}
}

func assertScenarioResponse(t *testing.T, payload json.RawMessage, wireError *rtvbp.WireError, step scenarioStep) {
	t.Helper()
	if step.Error == nil {
		if wireError != nil {
			t.Fatalf("response error = %#v, want result %s", wireError, step.Result)
		}
		assertJSON(t, payload, step.Result)
		return
	}
	if wireError == nil {
		t.Fatalf("response result = %s, want error %#v", payload, step.Error)
	}
	want := step.Error.controlError()
	if wireError.Code != want.Code || wireError.Message != want.Message {
		t.Fatalf("response error = %#v, want %#v", wireError, want)
	}
	assertJSON(t, wireError.Data, want.Data)
}

func bindOriginatedID(t *testing.T, bindings map[string]string, name, value string) {
	t.Helper()
	if !strings.HasPrefix(name, "$") || value == "" {
		t.Fatalf("invalid originated id binding %q=%q", name, value)
	}
	if _, exists := bindings[name]; exists {
		t.Fatalf("duplicate id binding %q", name)
	}
	bindings[name] = value
}

func bindPeerID(t *testing.T, bindings map[string]string, name string) string {
	t.Helper()
	if value, ok := bindings[name]; ok {
		return value
	}
	if !strings.HasPrefix(name, "$") {
		t.Fatalf("invalid peer id binding %q", name)
	}
	value := "peer-" + strings.TrimPrefix(name, "$")
	bindings[name] = value
	return value
}

func boundID(t *testing.T, bindings map[string]string, name string) string {
	t.Helper()
	value, ok := bindings[name]
	if !ok {
		t.Fatalf("unbound id %q", name)
	}
	return value
}

func sendFrame(t *testing.T, peer rtvbp.Transport, frame rtvbp.ControlFrame) {
	t.Helper()
	data, err := (v1classic.Envelope{}).Encode(frame)
	if err != nil {
		t.Fatalf("encode scripted frame: %v", err)
	}
	if err := peer.Control().Send(testContext(t), data); err != nil {
		t.Fatalf("send scripted frame: %v", err)
	}
}

func receiveFrame(t *testing.T, peer rtvbp.Transport) rtvbp.ControlFrame {
	t.Helper()
	received, err := peer.Control().Recv(testContext(t))
	if err != nil {
		t.Fatalf("receive SDK frame: %v", err)
	}
	frame, err := (v1classic.Envelope{}).Decode(received.Data)
	if err != nil {
		t.Fatalf("decode SDK frame %s: %v", received.Data, err)
	}
	return frame
}

func awaitRequest(t *testing.T, result <-chan requestResult) requestResult {
	t.Helper()
	select {
	case outcome := <-result:
		return outcome
	case <-time.After(3 * time.Second):
		t.Fatal("local request did not finish")
		return requestResult{}
	}
}

func waitForState(t *testing.T, session *rtvbp.Session, want rtvbp.SessionState) {
	t.Helper()
	deadline := time.Now().Add(3 * time.Second)
	for time.Now().Before(deadline) {
		if session.State() == want {
			return
		}
		time.Sleep(time.Millisecond)
	}
	t.Fatalf("session state = %s, want %s", session.State(), want)
}

func testContext(t *testing.T) context.Context {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	t.Cleanup(cancel)
	return ctx
}

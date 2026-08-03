package babelforcev1

import (
	"context"
	"errors"
	"fmt"
	"maps"
	"sync"
	"testing"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
	"github.com/babelforce/rtvbp/sdk/go/envelope/v1classic"
	"github.com/babelforce/rtvbp/sdk/go/transport/memory"
)

type fakeTelephony struct {
	mu            sync.Mutex
	variables     map[string]any
	moved         *babelforcev1.ApplicationMoveRequest
	hungUp        bool
	dtmfHandler   TelephonyDtmfHandler
	hangupHandler TelephonyHangupHandler
}

func newFakeTelephony() *fakeTelephony {
	return &fakeTelephony{variables: make(map[string]any)}
}

func (telephony *fakeTelephony) Move(
	_ context.Context,
	request *babelforcev1.ApplicationMoveRequest,
) (*babelforcev1.ApplicationMoveResponse, error) {
	telephony.mu.Lock()
	defer telephony.mu.Unlock()
	telephony.moved = request
	next := request.ApplicationID
	if next == "" {
		next = "<id_of_next_node_if_any>"
	}
	return &babelforcev1.ApplicationMoveResponse{NextApplicationID: next}, nil
}

func (telephony *fakeTelephony) Hangup(
	_ context.Context,
	_ *babelforcev1.CallHangupRequest,
) error {
	telephony.mu.Lock()
	defer telephony.mu.Unlock()
	telephony.hungUp = true
	return nil
}

func (telephony *fakeTelephony) SessionVariablesSet(
	_ context.Context,
	request *babelforcev1.SessionSetRequest,
) error {
	telephony.mu.Lock()
	defer telephony.mu.Unlock()
	maps.Copy(telephony.variables, request.Data)
	return nil
}

func (telephony *fakeTelephony) SessionVariablesGet(
	_ context.Context,
	request *babelforcev1.SessionGetRequest,
) (map[string]any, error) {
	telephony.mu.Lock()
	defer telephony.mu.Unlock()
	values := make(map[string]any)
	if len(request.Keys) == 0 {
		maps.Copy(values, telephony.variables)
		return values, nil
	}
	for _, key := range request.Keys {
		if value, ok := telephony.variables[key]; ok {
			values[key] = value
		}
	}
	return values, nil
}

func (*fakeTelephony) RecordingStart(
	context.Context,
	*babelforcev1.RecordingStartRequest,
) (*babelforcev1.RecordingStartResponse, error) {
	return &babelforcev1.RecordingStartResponse{ID: "recording-1"}, nil
}

func (*fakeTelephony) RecordingStop(_ context.Context, recordingID string) error {
	if recordingID == "" {
		return fmt.Errorf("recording ID is required")
	}
	return nil
}

func (telephony *fakeTelephony) OnDTMF(handler TelephonyDtmfHandler) error {
	telephony.mu.Lock()
	defer telephony.mu.Unlock()
	if telephony.dtmfHandler != nil {
		return fmt.Errorf("DTMF handler already set")
	}
	telephony.dtmfHandler = handler
	return nil
}

func (telephony *fakeTelephony) OnHangup(handler TelephonyHangupHandler) error {
	telephony.mu.Lock()
	defer telephony.mu.Unlock()
	if telephony.hangupHandler != nil {
		return fmt.Errorf("hangup handler already set")
	}
	telephony.hangupHandler = handler
	return nil
}

func (telephony *fakeTelephony) emitDTMF(event *babelforcev1.DtmfEvent) {
	telephony.mu.Lock()
	handler := telephony.dtmfHandler
	telephony.mu.Unlock()
	handler(event)
}

type applicationHandler struct {
	updated chan *babelforcev1.SessionUpdatedEvent
	events  chan rtvbp.NamedEvent
}

func newApplicationHandler() *applicationHandler {
	return &applicationHandler{
		updated: make(chan *babelforcev1.SessionUpdatedEvent, 1),
		events:  make(chan rtvbp.NamedEvent, 16),
	}
}

func (*applicationHandler) Ping(
	ctx context.Context,
	_ rtvbp.SHC,
	request *babelforcev1.PingRequest,
) (*babelforcev1.PingResponse, error) {
	inbound, ok := rtvbp.InboundRequest(ctx)
	if !ok {
		return nil, fmt.Errorf("missing inbound request")
	}
	now := time.Now().UnixMilli()
	return &babelforcev1.PingResponse{
		T0: request.T0, T1: inbound.ReceivedAt.UnixMilli(), T2: now, OWD: now - request.T0, Data: request.Data,
	}, nil
}

func (*applicationHandler) SessionInitialize(
	ctx context.Context,
	shc rtvbp.SHC,
	request *babelforcev1.SessionInitializeRequest,
) (*babelforcev1.SessionInitializeResponse, error) {
	format, err := MediaFormat(&request.AudioCodecOfferings[0], DefaultPTime)
	if err != nil {
		return nil, err
	}
	if err := shc.OpenAudio(ctx, format); err != nil {
		return nil, err
	}
	return &babelforcev1.SessionInitializeResponse{AudioCodec: &request.AudioCodecOfferings[0]}, nil
}

func (*applicationHandler) SessionTerminate(
	context.Context,
	rtvbp.SHC,
	*babelforcev1.SessionTerminateRequest,
) (*babelforcev1.EmptyResponse, error) {
	return &babelforcev1.EmptyResponse{}, nil
}

func (handler *applicationHandler) AudioInfo(
	_ context.Context,
	_ rtvbp.SHC,
	event *babelforcev1.AudioInfoEvent,
) error {
	handler.events <- event
	return nil
}

func (handler *applicationHandler) CallHangup(
	_ context.Context,
	_ rtvbp.SHC,
	event *babelforcev1.CallHangupEvent,
) error {
	handler.events <- event
	return nil
}

func (handler *applicationHandler) Dtmf(
	_ context.Context,
	_ rtvbp.SHC,
	event *babelforcev1.DtmfEvent,
) error {
	handler.events <- event
	return nil
}

func (handler *applicationHandler) SessionUpdated(
	_ context.Context,
	_ rtvbp.SHC,
	event *babelforcev1.SessionUpdatedEvent,
) error {
	handler.updated <- event
	return nil
}

type bridgeSessions struct {
	application     *rtvbp.Session
	voice           *rtvbp.Session
	applicationDone <-chan error
	voiceDone       <-chan error
	applicationSide *applicationHandler
	voiceSide       *VoiceHandler
	telephony       *fakeTelephony
}

func startBridgeSessions(t *testing.T) bridgeSessions {
	t.Helper()
	left, right := memory.NewPair(memory.WithMedia())
	applicationSide := newApplicationHandler()
	registrations := babelforcev1.ApplicationHandlers(applicationSide)
	registrations = append(registrations, babelforcev1.ApplicationEventHandlers(applicationSide)...)
	applicationSession := newTestSession(
		left,
		rtvbp.NewHandler(rtvbp.HandlerConfig{}, registrations...),
	)

	telephony := newFakeTelephony()
	voiceSide := NewVoiceHandler(
		telephony,
		HandlerConfig{
			Call:        babelforcev1.CallInfo{ID: "call-1", SessionID: "session-1", From: "1000", To: "1001"},
			Application: babelforcev1.AppInfo{ID: "app-1"},
			Metadata:    map[string]any{"test": true},
			AudioFormat: DefaultMediaFormat(),
		},
		nil,
	)
	voiceSession := newTestSession(right, voiceSide)

	applicationDone := applicationSession.Run(t.Context())
	waitForState(t, applicationSession, rtvbp.SessionStateActive)
	voiceDone := voiceSession.Run(t.Context())
	select {
	case <-applicationSide.updated:
	case err := <-voiceDone:
		t.Fatalf("voice session ended during initialization: %v", err)
	case <-t.Context().Done():
		t.Fatal(t.Context().Err())
	}
	waitForState(t, voiceSession, rtvbp.SessionStateActive)
	return bridgeSessions{
		application:     applicationSession,
		voice:           voiceSession,
		applicationDone: applicationDone,
		voiceDone:       voiceDone,
		applicationSide: applicationSide,
		voiceSide:       voiceSide,
		telephony:       telephony,
	}
}

func newTestSession(transport rtvbp.Transport, handler rtvbp.SessionHandler) *rtvbp.Session {
	return rtvbp.NewSession(
		v1classic.Envelope{},
		rtvbp.WithTransport(transport),
		rtvbp.WithHandler(handler),
		rtvbp.WithRequestTimeout(2*time.Second),
		rtvbp.WithCloseTimeout(2*time.Second),
	)
}

func waitForState(t *testing.T, session *rtvbp.Session, want rtvbp.SessionState) {
	t.Helper()
	deadline := time.Now().Add(2 * time.Second)
	for time.Now().Before(deadline) {
		if session.State() == want {
			return
		}
		time.Sleep(time.Millisecond)
	}
	t.Fatalf("session state = %s, want %s", session.State(), want)
}

func awaitSession(t *testing.T, done <-chan error) {
	t.Helper()
	select {
	case err := <-done:
		if err != nil {
			t.Fatalf("session ended with %v", err)
		}
	case <-t.Context().Done():
		t.Fatal(t.Context().Err())
	}
}

func closeBridgeSessions(t *testing.T, sessions bridgeSessions) {
	t.Helper()
	if sessions.voice.State() == rtvbp.SessionStateActive {
		if err := sessions.voiceSide.Terminate("end_of_test"); err != nil &&
			!errors.Is(err, rtvbp.ErrSessionClosed) {
			t.Fatalf("terminate: %v", err)
		}
	}
	awaitSession(t, sessions.voiceDone)
	awaitSession(t, sessions.applicationDone)
}

func TestVoiceBridgeGeneratedNonTerminalDispatch(t *testing.T) {
	sessions := startBridgeSessions(t)
	peer := babelforcev1.NewVoicePeer(sessions.application)

	if _, err := peer.Ping(t.Context(), NewPingRequest()); err != nil {
		t.Fatalf("ping: %v", err)
	}
	if _, err := sessions.application.Request(
		t.Context(),
		&babelforcev1.SessionTerminateRequest{Reason: "reverse"},
	); err == nil || err.Error() != "501: session.terminate is not supported. please use application.move or call.hangup instead" {
		t.Fatalf("reverse terminate error = %v", err)
	}
	if _, err := peer.AudioBufferClear(t.Context(), &babelforcev1.AudioBufferClearRequest{}); err != nil {
		t.Fatalf("audio clear: %v", err)
	}
	if _, err := peer.SessionSet(
		t.Context(),
		&babelforcev1.SessionSetRequest{Data: map[string]any{"foo": "bar", "count": 23}},
	); err != nil {
		t.Fatalf("session set: %v", err)
	}
	values, err := peer.SessionGet(
		t.Context(),
		&babelforcev1.SessionGetRequest{Keys: []string{"foo", "count", "missing"}},
	)
	if err != nil {
		t.Fatalf("session get: %v", err)
	}
	if (*values)["foo"] != "bar" || (*values)["count"] != float64(23) {
		t.Fatalf("session values = %#v", *values)
	}
	recording, err := peer.RecordingStart(
		t.Context(),
		&babelforcev1.RecordingStartRequest{Tags: []string{"test"}},
	)
	if err != nil {
		t.Fatalf("recording start: %v", err)
	}
	if _, err := peer.RecordingStop(
		t.Context(),
		&babelforcev1.RecordingStopRequest{ID: recording.ID},
	); err != nil {
		t.Fatalf("recording stop: %v", err)
	}

	now := time.Now().UnixMilli()
	sessions.telephony.emitDTMF(&babelforcev1.DtmfEvent{
		PressedAt: now, ReleasedAt: now + 100, Digit: "5",
	})
	select {
	case event := <-sessions.applicationSide.events:
		dtmf, ok := event.(*babelforcev1.DtmfEvent)
		if !ok || dtmf.Seq != 0 || dtmf.Digit != "5" {
			t.Fatalf("DTMF event = %#v", event)
		}
	case <-t.Context().Done():
		t.Fatal(t.Context().Err())
	}
	closeBridgeSessions(t, sessions)
}

func TestGeneratedPayloadValidation(t *testing.T) {
	tests := []struct {
		name  string
		value rtvbp.Validation
	}{
		{name: "hangup reason", value: &babelforcev1.CallHangupRequest{}},
		{name: "terminate reason", value: &babelforcev1.SessionTerminateRequest{}},
		{name: "recording ID", value: &babelforcev1.RecordingStopRequest{}},
		{name: "ping timestamp", value: &babelforcev1.PingRequest{}},
		{name: "DTMF digit", value: &babelforcev1.DtmfEvent{}},
		{
			name: "DTMF timestamp order",
			value: &babelforcev1.DtmfEvent{
				PressedAt: 2, ReleasedAt: 1, Digit: "5",
			},
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if err := test.value.Validate(); err == nil {
				t.Fatal("Validate() succeeded for invalid payload")
			}
		})
	}

	if err := (&babelforcev1.DtmfEvent{
		PressedAt: 1, ReleasedAt: 2, Digit: "5",
	}).Validate(); err != nil {
		t.Fatalf("valid DTMF event: %v", err)
	}
}

func TestGeneratedPeerRejectsInvalidRequestBeforeDispatch(t *testing.T) {
	sessions := startBridgeSessions(t)
	defer closeBridgeSessions(t, sessions)

	_, err := babelforcev1.NewVoicePeer(sessions.application).CallHangup(
		t.Context(),
		&babelforcev1.CallHangupRequest{},
	)
	if !errors.Is(err, rtvbp.ErrRequestValidationFailed) {
		t.Fatalf("CallHangup() error = %v, want request validation failure", err)
	}
	sessions.telephony.mu.Lock()
	hungUp := sessions.telephony.hungUp
	sessions.telephony.mu.Unlock()
	if hungUp {
		t.Fatal("invalid request reached the telephony adapter")
	}
	if got := sessions.voice.State(); got != rtvbp.SessionStateActive {
		t.Fatalf("voice session state = %s, want active", got)
	}
}

func TestVoiceBridgeTerminalOperationsRespondThenClose(t *testing.T) {
	tests := []struct {
		name   string
		invoke func(context.Context, *babelforcev1.VoicePeer) error
		check  func(*testing.T, *fakeTelephony)
	}{
		{
			name: "application move",
			invoke: func(ctx context.Context, peer *babelforcev1.VoicePeer) error {
				response, err := peer.ApplicationMove(ctx, &babelforcev1.ApplicationMoveRequest{
					Reason: "handoff", ApplicationID: "app-2",
				})
				if err == nil && response.NextApplicationID != "app-2" {
					return fmt.Errorf("next application = %q", response.NextApplicationID)
				}
				return err
			},
			check: func(t *testing.T, telephony *fakeTelephony) {
				t.Helper()
				if telephony.moved == nil || telephony.moved.ApplicationID != "app-2" {
					t.Fatalf("move = %#v", telephony.moved)
				}
			},
		},
		{
			name: "call hangup",
			invoke: func(ctx context.Context, peer *babelforcev1.VoicePeer) error {
				_, err := peer.CallHangup(ctx, &babelforcev1.CallHangupRequest{Reason: "caller"})
				return err
			},
			check: func(t *testing.T, telephony *fakeTelephony) {
				t.Helper()
				if !telephony.hungUp {
					t.Fatal("telephony was not hung up")
				}
			},
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			sessions := startBridgeSessions(t)
			if err := test.invoke(t.Context(), babelforcev1.NewVoicePeer(sessions.application)); err != nil {
				t.Fatalf("terminal request: %v", err)
			}
			test.check(t, sessions.telephony)
			awaitSession(t, sessions.voiceDone)
			awaitSession(t, sessions.applicationDone)
		})
	}
}

func TestGeneratedVoiceHandlerOwnsPingTiming(t *testing.T) {
	handler := &VoiceHandler{}
	registrations := babelforcev1.VoiceHandlers(handler)
	var ping rtvbp.RequestHandler
	for _, candidate := range registrations {
		requestHandler := candidate.(rtvbp.RequestHandler)
		if requestHandler.MethodName() == "ping" {
			ping = requestHandler
		}
	}
	if ping == nil {
		t.Fatal("generated voice registrations omitted ping")
	}
	receivedAt := time.Now()
	shc := rtvbp.NewTestingSHC()
	request := NewPingRequest()
	payload := fmt.Appendf(nil, "{\"t0\":%d}", request.T0)
	if err := ping.Handle(
		t.Context(),
		shc,
		rtvbp.Request{Method: "ping", Payload: payload, ReceivedAt: receivedAt},
	); err != nil {
		t.Fatal(err)
	}
}

var _ TelephonyAdapter = (*fakeTelephony)(nil)
var _ babelforcev1.ApplicationHandler = (*applicationHandler)(nil)
var _ babelforcev1.ApplicationEventHandler = (*applicationHandler)(nil)

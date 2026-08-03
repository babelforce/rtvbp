package interop_test

import (
	"context"
	"fmt"
	"io"
	"testing"
	"time"

	old "github.com/babelforce/rtvbp-go"
	oldv1 "github.com/babelforce/rtvbp-go/proto/protov1"
	oldws "github.com/babelforce/rtvbp-go/transport/ws"
	new "github.com/babelforce/rtvbp/sdk/go"
	newbridge "github.com/babelforce/rtvbp/sdk/go/bridge/babelforcev1"
	newv1 "github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
	"github.com/babelforce/rtvbp/sdk/go/envelope/v1classic"
	newws "github.com/babelforce/rtvbp/sdk/go/transport/ws"
	"go.uber.org/goleak"
)

const legacyPingInterval = 20 * time.Millisecond

var (
	mediaFormat = new.MediaFormat{Encoding: "L16", SampleRate: 8000, BitDepth: 16, Channels: 1, PTime: 20 * time.Millisecond}
	audioProbe  = make([]byte, 320)
)

func init() {
	for index := range audioProbe {
		audioProbe[index] = byte(index)
	}
}

func TestPublishedV037VoiceAgainstNewApplication(t *testing.T) {
	defer goleak.VerifyNone(t, goleak.IgnoreCurrent())
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	application := newApplicationHandler()
	registrations := newv1.ApplicationHandlers(application)
	registrations = append(registrations, newv1.ApplicationEventHandlers(application)...)
	server := newws.NewServer(newws.ServerConfig{Addr: "127.0.0.1:0"}, new.NewHandler(new.HandlerConfig{}, registrations...))
	if err := server.Listen(); err != nil {
		t.Fatalf("listen new server: %v", err)
	}
	defer func() { _ = server.Shutdown(context.Background()) }()

	telephony := &oldTelephony{ready: make(chan struct{})}
	voice := oldv1.NewClientHandler(telephony, &oldv1.ClientHandlerConfig{
		Call: oldv1.CallInfo{ID: "call", SessionID: "session", From: "100", To: "200"},
		App:  oldv1.AppInfo{ID: "app"}, Metadata: map[string]any{"interop": true},
		PingInterval: legacyPingInterval, SampleRate: 8000,
	}, func(_ context.Context, handler old.SHC) error {
		return exchangeAudio(handler.AudioStream())
	})
	session := old.NewSession(oldws.Client(oldws.ClientConfig{
		Dial: oldws.DialConfig{URL: server.URL()}, PingInterval: legacyPingInterval, SampleRate: 8000,
	}), old.WithHandler(voice))
	done := session.Run(ctx)

	waitSignal(t, application.initialized, "new application initialization")
	waitSignal(t, application.audio, "bidirectional audio")
	waitSignal(t, application.updated, "session.updated")
	waitSignal(t, telephony.ready, "old DTMF registration")
	telephony.sendDTMF("5")
	waitSignal(t, application.dtmf, "dtmf")
	waitSignal(t, application.ping, "legacy application ping after idle interval")
	if err := voice.Terminate("interop complete"); err != nil {
		t.Fatalf("old voice terminate: %v", err)
	}
	waitSignal(t, application.terminated, "session.terminate")
	waitDone(t, done)
}

func TestNewVoiceAgainstPublishedV037Application(t *testing.T) {
	defer goleak.VerifyNone(t, goleak.IgnoreCurrent())
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	application := newOldApplicationHandler()
	server := oldws.NewServer(oldws.ServerConfig{Addr: "127.0.0.1:0"}, application.handler)
	if err := server.Listen(); err != nil {
		t.Fatalf("listen old server: %v", err)
	}
	defer func() { _ = server.Shutdown(context.Background()) }()

	telephony := &newTelephony{ready: make(chan struct{})}
	voice := newbridge.NewVoiceHandler(telephony, newbridge.HandlerConfig{
		Call:        newv1.CallInfo{ID: "call", SessionID: "session", From: "100", To: "200"},
		Application: newv1.AppInfo{ID: "app"}, Metadata: map[string]any{"interop": true}, AudioFormat: mediaFormat,
	}, func(_ context.Context, handler new.SHC) error {
		return exchangeAudio(handler.AudioStream())
	})
	session := new.NewSession(
		v1classic.Envelope{},
		newws.Client(newws.ClientConfig{
			Dial: newws.DialConfig{URL: server.URL()}, AudioFormat: mediaFormat,
			Subprotocols: []string{}, // v0.37 offers no Sec-WebSocket-Protocol header.
		}),
		new.WithHandler(voice),
	)
	done := session.Run(ctx)

	waitSignal(t, application.initialized, "old application initialization")
	waitSignal(t, application.audio, "bidirectional audio")
	waitSignal(t, application.updated, "session.updated")
	waitSignal(t, telephony.ready, "new DTMF registration")
	telephony.sendDTMF("7")
	waitSignal(t, application.dtmf, "dtmf")
	if err := voice.Terminate("interop complete"); err != nil {
		t.Fatalf("new voice terminate: %v", err)
	}
	waitSignal(t, application.terminated, "session.terminate")
	waitDone(t, done)
}

type newApplication struct {
	initialized chan struct{}
	updated     chan struct{}
	dtmf        chan struct{}
	terminated  chan struct{}
	audio       chan struct{}
	ping        chan struct{}
}

func newApplicationHandler() *newApplication {
	return &newApplication{
		initialized: make(chan struct{}, 1), updated: make(chan struct{}, 1),
		dtmf: make(chan struct{}, 1), terminated: make(chan struct{}, 1),
		audio: make(chan struct{}, 1), ping: make(chan struct{}, 1),
	}
}

func (handler *newApplication) Ping(ctx context.Context, _ new.SHC, request *newv1.PingRequest) (*newv1.PingResponse, error) {
	inbound, ok := new.InboundRequest(ctx)
	if !ok {
		return nil, fmt.Errorf("missing inbound request")
	}
	now := time.Now().UnixMilli()
	signal(handler.ping)
	return &newv1.PingResponse{T0: request.T0, T1: inbound.ReceivedAt.UnixMilli(), T2: now, OWD: now - request.T0, Data: request.Data}, nil
}

func (handler *newApplication) SessionInitialize(ctx context.Context, shc new.SHC, request *newv1.SessionInitializeRequest) (*newv1.SessionInitializeResponse, error) {
	if len(request.AudioCodecOfferings) == 0 {
		return nil, fmt.Errorf("missing codec offering")
	}
	selected := request.AudioCodecOfferings[0]
	format, err := newbridge.MediaFormat(&selected, newbridge.DefaultPTime)
	if err != nil {
		return nil, err
	}
	if err := shc.OpenAudio(ctx, format); err != nil {
		return nil, err
	}
	go echoAudio(shc.AudioStream(), handler.audio)
	signal(handler.initialized)
	return &newv1.SessionInitializeResponse{AudioCodec: &selected}, nil
}

func (handler *newApplication) SessionTerminate(context.Context, new.SHC, *newv1.SessionTerminateRequest) (*newv1.EmptyResponse, error) {
	signal(handler.terminated)
	return &newv1.EmptyResponse{}, nil
}

func (*newApplication) AudioInfo(context.Context, new.SHC, *newv1.AudioInfoEvent) error   { return nil }
func (*newApplication) CallHangup(context.Context, new.SHC, *newv1.CallHangupEvent) error { return nil }
func (handler *newApplication) Dtmf(context.Context, new.SHC, *newv1.DtmfEvent) error {
	signal(handler.dtmf)
	return nil
}
func (handler *newApplication) SessionUpdated(context.Context, new.SHC, *newv1.SessionUpdatedEvent) error {
	signal(handler.updated)
	return nil
}

type oldApplication struct {
	handler     old.SessionHandler
	initialized chan struct{}
	updated     chan struct{}
	dtmf        chan struct{}
	terminated  chan struct{}
	audio       chan struct{}
}

func newOldApplicationHandler() *oldApplication {
	application := &oldApplication{
		initialized: make(chan struct{}, 1), updated: make(chan struct{}, 1),
		dtmf: make(chan struct{}, 1), terminated: make(chan struct{}, 1), audio: make(chan struct{}, 1),
	}
	application.handler = old.NewHandler(old.HandlerConfig{},
		oldv1.NewPingHandler(),
		old.HandleRequest(func(_ context.Context, shc old.SHC, request *oldv1.SessionInitializeRequest) (*oldv1.SessionInitializeResponse, error) {
			if len(request.AudioCodecOfferings) == 0 {
				return nil, fmt.Errorf("missing codec offering")
			}
			selected := request.AudioCodecOfferings[0]
			go echoAudio(shc.AudioStream(), application.audio)
			signal(application.initialized)
			return &oldv1.SessionInitializeResponse{AudioCodec: &selected}, nil
		}),
		old.HandleRequest(func(_ context.Context, shc old.SHC, _ *oldv1.SessionTerminateRequest) (*oldv1.EmptyResponse, error) {
			signal(application.terminated)
			go func() {
				time.Sleep(10 * time.Millisecond)
				_ = shc.Close(context.Background(), nil)
			}()
			return &oldv1.EmptyResponse{}, nil
		}),
		old.HandleEvent(func(_ context.Context, _ old.SHC, _ *oldv1.SessionUpdatedEvent) error {
			signal(application.updated)
			return nil
		}),
		old.HandleEvent(func(_ context.Context, _ old.SHC, _ *oldv1.DTMFEvent) error {
			signal(application.dtmf)
			return nil
		}),
	)
	return application
}

func echoAudio(stream io.ReadWriter, complete chan struct{}) {
	buffer := make([]byte, len(audioProbe))
	if _, err := io.ReadFull(stream, buffer); err != nil {
		return
	}
	if _, err := stream.Write(buffer); err != nil {
		return
	}
	if string(buffer) == string(audioProbe) {
		signal(complete)
	}
}

func exchangeAudio(stream io.ReadWriter) error {
	if _, err := stream.Write(audioProbe); err != nil {
		return err
	}
	buffer := make([]byte, len(audioProbe))
	if _, err := io.ReadFull(stream, buffer); err != nil {
		return err
	}
	if string(buffer) != string(audioProbe) {
		return fmt.Errorf("audio probe changed")
	}
	return nil
}

func waitSignal(t *testing.T, ch <-chan struct{}, name string) {
	t.Helper()
	select {
	case <-ch:
	case <-time.After(3 * time.Second):
		t.Fatalf("timed out waiting for %s", name)
	}
}

func waitDone(t *testing.T, done <-chan error) {
	t.Helper()
	select {
	case err := <-done:
		if err != nil {
			t.Fatalf("session run: %v", err)
		}
	case <-time.After(3 * time.Second):
		t.Fatal("session did not finish")
	}
}

func signal(ch chan struct{}) {
	select {
	case ch <- struct{}{}:
	default:
	}
}

type oldTelephony struct {
	dtmf  oldv1.TelephonyDtmfHandler
	ready chan struct{}
}

func (*oldTelephony) Move(context.Context, *oldv1.ApplicationMoveRequest) (*oldv1.ApplicationMoveResponse, error) {
	return &oldv1.ApplicationMoveResponse{}, nil
}
func (*oldTelephony) Hangup(context.Context, *oldv1.CallHangupRequest) error              { return nil }
func (*oldTelephony) SessionVariablesSet(context.Context, *oldv1.SessionSetRequest) error { return nil }
func (*oldTelephony) SessionVariablesGet(context.Context, *oldv1.SessionGetRequest) (map[string]any, error) {
	return map[string]any{}, nil
}
func (*oldTelephony) RecordingStart(context.Context, *oldv1.RecordingStartRequest) (*oldv1.RecordingStartResponse, error) {
	return &oldv1.RecordingStartResponse{ID: "recording"}, nil
}
func (*oldTelephony) RecordingStop(context.Context, string) error { return nil }
func (telephony *oldTelephony) OnDTMF(handler oldv1.TelephonyDtmfHandler) error {
	telephony.dtmf = handler
	close(telephony.ready)
	return nil
}
func (*oldTelephony) OnHangup(oldv1.TelephonyHangupHandler) error { return nil }
func (telephony *oldTelephony) sendDTMF(digit string) {
	now := time.Now().UnixMilli()
	telephony.dtmf(&oldv1.DTMFEvent{Digit: digit, PressedAt: now, ReleasedAt: now + 1})
}

type newTelephony struct {
	dtmf  newbridge.TelephonyDtmfHandler
	ready chan struct{}
}

func (*newTelephony) Move(context.Context, *newv1.ApplicationMoveRequest) (*newv1.ApplicationMoveResponse, error) {
	return &newv1.ApplicationMoveResponse{}, nil
}
func (*newTelephony) Hangup(context.Context, *newv1.CallHangupRequest) error              { return nil }
func (*newTelephony) SessionVariablesSet(context.Context, *newv1.SessionSetRequest) error { return nil }
func (*newTelephony) SessionVariablesGet(context.Context, *newv1.SessionGetRequest) (map[string]any, error) {
	return map[string]any{}, nil
}
func (*newTelephony) RecordingStart(context.Context, *newv1.RecordingStartRequest) (*newv1.RecordingStartResponse, error) {
	return &newv1.RecordingStartResponse{ID: "recording"}, nil
}
func (*newTelephony) RecordingStop(context.Context, string) error { return nil }
func (telephony *newTelephony) OnDTMF(handler newbridge.TelephonyDtmfHandler) error {
	telephony.dtmf = handler
	close(telephony.ready)
	return nil
}
func (*newTelephony) OnHangup(newbridge.TelephonyHangupHandler) error { return nil }
func (telephony *newTelephony) sendDTMF(digit string) {
	now := time.Now().UnixMilli()
	telephony.dtmf(&newv1.DtmfEvent{Digit: digit, PressedAt: now, ReleasedAt: now + 1})
}

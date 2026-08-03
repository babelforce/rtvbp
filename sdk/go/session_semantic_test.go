package rtvbp_test

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/babelforce/rtvbp/sdk/go/envelope/v1classic"
	"github.com/babelforce/rtvbp/sdk/go/transport/memory"
	"go.uber.org/goleak"
)

type sequenceEvent struct {
	Seq int `json:"seq"`
}

func (*sequenceEvent) EventName() string { return "dtmf" }

type testRequest struct {
	Method string `json:"-"`
}

func (r *testRequest) MethodName() string { return r.Method }

type emptyResponse struct{}

func TestSessionDispatchesEventsSeriallyInReceiveOrder(t *testing.T) {
	left, peer := memory.NewPair()
	var active atomic.Int32
	var maximum atomic.Int32
	observed := make(chan int, 64)
	handler := rtvbp.NewHandler(rtvbp.HandlerConfig{}, rtvbp.HandleEvent(func(_ context.Context, _ rtvbp.SHC, event *sequenceEvent) error {
		now := active.Add(1)
		for old := maximum.Load(); now > old && !maximum.CompareAndSwap(old, now); old = maximum.Load() {
		}
		defer active.Add(-1)
		observed <- event.Seq
		return nil
	}))
	session, done := runSession(t, left, handler)

	for index := 0; index < cap(observed); index++ {
		sendFrame(t, peer, rtvbp.ControlFrame{
			Kind:    rtvbp.KindEvent,
			ID:      fmt.Sprintf("event-%d", index),
			Method:  "dtmf",
			Payload: json.RawMessage(fmt.Sprintf(`{"seq":%d}`, index)),
		})
	}
	for want := 0; want < cap(observed); want++ {
		if got := receive(t, observed); got != want {
			t.Fatalf("event order = %d at index %d", got, want)
		}
	}
	if got := maximum.Load(); got != 1 {
		t.Fatalf("maximum concurrent handlers = %d, want 1", got)
	}
	closeSession(t, session, done)
}

func TestResponseFastPathBypassesBlockedDispatcher(t *testing.T) {
	left, peer := memory.NewPair()
	entered := make(chan struct{})
	release := make(chan struct{})
	handler := rtvbp.NewHandler(rtvbp.HandlerConfig{}, rtvbp.HandleEvent(func(_ context.Context, _ rtvbp.SHC, _ *sequenceEvent) error {
		select {
		case <-entered:
		default:
			close(entered)
		}
		<-release
		return nil
	}))
	session, done := runSession(t, left, handler)
	sendFrame(t, peer, rtvbp.ControlFrame{Kind: rtvbp.KindEvent, ID: "blocked", Method: "dtmf", Payload: json.RawMessage(`{"seq":0}`)})
	receiveSignal(t, entered)

	result := make(chan error, 1)
	go func() {
		_, err := session.Request(context.Background(), &testRequest{Method: "fast.path"})
		result <- err
	}()
	request := recvFrame(t, peer)
	for index := 1; index < 32; index++ {
		sendFrame(t, peer, rtvbp.ControlFrame{Kind: rtvbp.KindEvent, ID: fmt.Sprintf("queued-%d", index), Method: "dtmf", Payload: json.RawMessage(fmt.Sprintf(`{"seq":%d}`, index))})
	}
	sendFrame(t, peer, rtvbp.ControlFrame{Kind: rtvbp.KindResponse, CorrelID: request.ID, Payload: json.RawMessage(`{}`)})
	if err := receive(t, result); err != nil {
		t.Fatalf("Request() error = %v", err)
	}
	close(release)
	closeSession(t, session, done)
}

func TestNestedRequestFromRequestHandlerDoesNotDeadlock(t *testing.T) {
	left, right := memory.NewPair()
	leftHandler := rtvbp.NewHandler(rtvbp.HandlerConfig{}, rtvbp.HandleRequest(func(ctx context.Context, handler rtvbp.SHC, _ *outerRequest) (*emptyResponse, error) {
		_, err := handler.Request(ctx, &innerRequest{})
		return &emptyResponse{}, err
	}))
	rightHandler := rtvbp.NewHandler(rtvbp.HandlerConfig{}, rtvbp.HandleRequest(func(context.Context, rtvbp.SHC, *innerRequest) (*emptyResponse, error) {
		return &emptyResponse{}, nil
	}))
	leftSession, leftDone := runSession(t, left, leftHandler)
	rightSession, rightDone := runSession(t, right, rightHandler)

	if _, err := rightSession.Request(context.Background(), &outerRequest{}); err != nil {
		t.Fatalf("nested request failed: %v", err)
	}
	closeSession(t, leftSession, leftDone)
	awaitDone(t, rightDone)
	if rightSession.State() != rtvbp.SessionStateClosed {
		t.Fatalf("peer state = %s, want closed", rightSession.State())
	}
}

func TestOnBeginCanIssueRequest(t *testing.T) {
	defer goleak.VerifyNone(t)

	left, right := memory.NewPair()
	rightSession, rightDone := runSession(t, right, rtvbp.NewHandler(rtvbp.HandlerConfig{}, rtvbp.HandleRequest(func(context.Context, rtvbp.SHC, *innerRequest) (*emptyResponse, error) {
		return &emptyResponse{}, nil
	})))
	onBegin := make(chan error, 1)
	leftHandler := rtvbp.NewHandler(rtvbp.HandlerConfig{OnBegin: func(ctx context.Context, handler rtvbp.SHC) error {
		_, err := handler.Request(ctx, &innerRequest{})
		onBegin <- err
		return err
	}})
	leftSession, leftDone := runSession(t, left, leftHandler)
	if err := receive(t, onBegin); err != nil {
		t.Fatalf("OnBegin Request() error = %v", err)
	}
	closeSession(t, leftSession, leftDone)
	if err := awaitDone(t, rightDone); err != nil {
		t.Fatalf("peer Run() error = %v", err)
	}
	if rightSession.State() != rtvbp.SessionStateClosed {
		t.Fatalf("peer state = %s, want closed", rightSession.State())
	}
}

type outerRequest struct{}

func (*outerRequest) MethodName() string { return "outer" }

type innerRequest struct{}

func (*innerRequest) MethodName() string { return "inner" }

func TestCloseResolvesEveryPendingRequest(t *testing.T) {
	left, peer := memory.NewPair()
	session, done := runSession(t, left, rtvbp.NewHandler(rtvbp.HandlerConfig{}))
	const count = 8
	results := make(chan error, count)
	for index := 0; index < count; index++ {
		go func(index int) {
			_, err := session.Request(context.Background(), &testRequest{Method: fmt.Sprintf("pending.%d", index)})
			results <- err
		}(index)
	}
	for index := 0; index < count; index++ {
		_ = recvFrame(t, peer)
	}
	if err := session.Close(testContext(t)); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
	for index := 0; index < count; index++ {
		if err := receive(t, results); !errors.Is(err, rtvbp.ErrSessionClosed) {
			t.Fatalf("pending Request() error = %v, want ErrSessionClosed", err)
		}
	}
	awaitDone(t, done)
}

func TestRespondThenCloseFlushesResponseBeforeEOF(t *testing.T) {
	left, peer := memory.NewPair()
	handler := rtvbp.NewHandler(rtvbp.HandlerConfig{}, rtvbp.HandleTerminalRequest(func(context.Context, rtvbp.SHC, *terminalRequest) (*emptyResponse, error) {
		return &emptyResponse{}, nil
	}))
	session, done := runSession(t, left, handler)
	sendFrame(t, peer, rtvbp.ControlFrame{Kind: rtvbp.KindRequest, ID: "terminal-1", Method: "terminal", Payload: json.RawMessage(`{}`)})
	response := recvFrame(t, peer)
	if response.Kind != rtvbp.KindResponse || response.CorrelID != "terminal-1" {
		t.Fatalf("terminal response = %#v", response)
	}
	if _, err := peer.Control().Recv(testContext(t)); !errors.Is(err, io.EOF) {
		t.Fatalf("Recv after response = %v, want EOF", err)
	}
	awaitDone(t, done)
	if session.State() != rtvbp.SessionStateClosed {
		t.Fatalf("state = %s, want closed", session.State())
	}
}

type terminalRequest struct{}

func (*terminalRequest) MethodName() string { return "terminal" }

func TestDeferredResponseReleasesDispatcherAndIsOneShot(t *testing.T) {
	left, peer := memory.NewPair()
	handle := make(chan rtvbp.DeferredResponse, 1)
	eventSeen := make(chan struct{})
	handler := rtvbp.NewHandler(
		rtvbp.HandlerConfig{},
		rtvbp.HandleRequest(func(_ context.Context, handler rtvbp.SHC, _ *deferredRequest) (*emptyResponse, error) {
			deferred, err := handler.DeferResponse()
			if err == nil {
				handle <- deferred
			}
			return nil, err
		}),
		rtvbp.HandleEvent(func(context.Context, rtvbp.SHC, *sequenceEvent) error {
			close(eventSeen)
			return nil
		}),
	)
	session, done := runSession(t, left, handler)
	sendFrame(t, peer, rtvbp.ControlFrame{Kind: rtvbp.KindRequest, ID: "deferred-1", Method: "deferred", Payload: json.RawMessage(`{}`)})
	deferred := receive(t, handle)
	sendFrame(t, peer, rtvbp.ControlFrame{Kind: rtvbp.KindEvent, ID: "event-after", Method: "dtmf", Payload: json.RawMessage(`{"seq":1}`)})
	receiveSignal(t, eventSeen)
	if err := deferred.Respond(context.Background(), rtvbp.Response{Payload: json.RawMessage(`{}`)}); err != nil {
		t.Fatalf("deferred Respond() error = %v", err)
	}
	if response := recvFrame(t, peer); response.CorrelID != "deferred-1" {
		t.Fatalf("deferred response = %#v", response)
	}
	if err := deferred.Respond(context.Background(), rtvbp.Response{}); !errors.Is(err, rtvbp.ErrResponseAlreadySent) {
		t.Fatalf("second Respond() error = %v, want ErrResponseAlreadySent", err)
	}
	closeSession(t, session, done)
}

func TestDeferredResponseAfterSessionCloseReturnsSessionClosed(t *testing.T) {
	defer goleak.VerifyNone(t)

	left, peer := memory.NewPair()
	handle := make(chan rtvbp.DeferredResponse, 1)
	handler := rtvbp.NewHandler(rtvbp.HandlerConfig{}, rtvbp.HandleRequest(func(_ context.Context, handler rtvbp.SHC, _ *deferredRequest) (*emptyResponse, error) {
		deferred, err := handler.DeferResponse()
		if err == nil {
			handle <- deferred
		}
		return nil, err
	}))
	session, done := runSession(t, left, handler)
	sendFrame(t, peer, rtvbp.ControlFrame{Kind: rtvbp.KindRequest, ID: "deferred-close", Method: "deferred", Payload: json.RawMessage(`{}`)})
	deferred := receive(t, handle)
	closeSession(t, session, done)
	if err := deferred.Respond(context.Background(), rtvbp.Response{}); !errors.Is(err, rtvbp.ErrSessionClosed) {
		t.Fatalf("Respond() after close error = %v, want ErrSessionClosed", err)
	}
}

type deferredRequest struct{}

func (*deferredRequest) MethodName() string { return "deferred" }

func TestKeepaliveFailureFailsSessionAndPendingRequest(t *testing.T) {
	left, peer := memory.NewPair()
	failure := make(chan struct{})
	transport := &failingKeepaliveTransport{Transport: left, fail: failure}
	session := rtvbp.NewSession(
		v1classic.Envelope{},
		rtvbp.WithTransport(transport),
		rtvbp.WithHandler(rtvbp.NewHandler(rtvbp.HandlerConfig{})),
		rtvbp.WithKeepalivePolicy(rtvbp.KeepalivePolicy{Interval: time.Second, Timeout: time.Second, MaxMisses: 1}),
	)
	done := session.Run(context.Background())
	waitState(t, session, rtvbp.SessionStateActive)
	requestDone := make(chan error, 1)
	go func() {
		_, err := session.Request(context.Background(), &testRequest{Method: "pending"})
		requestDone <- err
	}()
	_ = recvFrame(t, peer)
	close(failure)
	if err := receive(t, requestDone); !errors.Is(err, rtvbp.ErrKeepaliveTimeout) {
		t.Fatalf("Request() error = %v, want ErrKeepaliveTimeout", err)
	}
	if err := awaitDone(t, done); !errors.Is(err, rtvbp.ErrKeepaliveTimeout) {
		t.Fatalf("Run() error = %v, want ErrKeepaliveTimeout", err)
	}
	if session.State() != rtvbp.SessionStateFailed {
		t.Fatalf("state = %s, want failed", session.State())
	}
}

type failingKeepaliveTransport struct {
	rtvbp.Transport
	fail <-chan struct{}
}

func (t *failingKeepaliveTransport) MonitorKeepalive(ctx context.Context, _ rtvbp.KeepalivePolicy) error {
	select {
	case <-ctx.Done():
		return ctx.Err()
	case <-t.fail:
		return rtvbp.ErrKeepaliveTimeout
	}
}

func TestLifecycleConnectingActiveClosingClosed(t *testing.T) {
	left, _ := memory.NewPair()
	releaseFactory := make(chan struct{})
	onBeginState := make(chan rtvbp.SessionState, 1)
	session := rtvbp.NewSession(
		v1classic.Envelope{},
		rtvbp.WithTransportFactory(func(context.Context, rtvbp.Envelope) (rtvbp.Transport, error) {
			<-releaseFactory
			return left, nil
		}),
		rtvbp.WithHandler(rtvbp.NewHandler(rtvbp.HandlerConfig{OnBegin: func(_ context.Context, handler rtvbp.SHC) error {
			onBeginState <- handler.State()
			return nil
		}})),
	)
	done := session.Run(context.Background())
	if session.State() != rtvbp.SessionStateConnecting {
		t.Fatalf("state = %s, want connecting", session.State())
	}
	close(releaseFactory)
	if state := receive(t, onBeginState); state != rtvbp.SessionStateConnecting {
		t.Fatalf("OnBegin state = %s, want connecting", state)
	}
	waitState(t, session, rtvbp.SessionStateActive)
	closeSession(t, session, done)
}

func TestHandlerCloseFromOnBeginIsNonBlockingAndGraceful(t *testing.T) {
	defer goleak.VerifyNone(t)

	left, _ := memory.NewPair()
	closeReturned := make(chan error, 1)
	session := rtvbp.NewSession(
		v1classic.Envelope{},
		rtvbp.WithTransport(left),
		rtvbp.WithCloseTimeout(200*time.Millisecond),
		rtvbp.WithHandler(rtvbp.NewHandler(rtvbp.HandlerConfig{OnBegin: func(ctx context.Context, handler rtvbp.SHC) error {
			closeReturned <- handler.Close(ctx)
			return nil
		}})),
	)
	done := session.Run(context.Background())
	if err := receive(t, closeReturned); err != nil {
		t.Fatalf("SHC.Close() error = %v", err)
	}
	if err := awaitDone(t, done); err != nil {
		t.Fatalf("Run() error = %v", err)
	}
	if session.State() != rtvbp.SessionStateClosed {
		t.Fatalf("state = %s, want closed", session.State())
	}
}

func TestCloseDiscardsQueuedDispatchBeforeBlockingTransportClose(t *testing.T) {
	defer goleak.VerifyNone(t)

	left, peer := memory.NewPair()
	closeStarted := make(chan struct{})
	releaseTransport := make(chan struct{})
	transport := &blockingCloseTransport{Transport: left, started: closeStarted, release: releaseTransport}
	firstEntered := make(chan struct{})
	releaseFirst := make(chan struct{})
	firstReturned := make(chan struct{})
	extraDispatched := make(chan struct{}, 1)
	handler := rtvbp.NewHandler(rtvbp.HandlerConfig{}, rtvbp.HandleEvent(func(_ context.Context, _ rtvbp.SHC, event *sequenceEvent) error {
		if event.Seq == 0 {
			close(firstEntered)
			<-releaseFirst
			close(firstReturned)
		} else {
			select {
			case extraDispatched <- struct{}{}:
			default:
			}
		}
		return nil
	}))
	session, done := runSession(t, transport, handler)
	sendFrame(t, peer, rtvbp.ControlFrame{Kind: rtvbp.KindEvent, ID: "first", Method: "dtmf", Payload: json.RawMessage(`{"seq":0}`)})
	receiveSignal(t, firstEntered)
	for index := 1; index <= 32; index++ {
		sendFrame(t, peer, rtvbp.ControlFrame{Kind: rtvbp.KindEvent, ID: fmt.Sprintf("queued-%d", index), Method: "dtmf", Payload: json.RawMessage(fmt.Sprintf(`{"seq":%d}`, index))})
	}
	closeResult := make(chan error, 1)
	go func() { closeResult <- session.Close(testContext(t)) }()
	receiveSignal(t, closeStarted)
	close(releaseFirst)
	receiveSignal(t, firstReturned)
	select {
	case <-extraDispatched:
		t.Fatal("queued event dispatched after shutdown began")
	case <-time.After(50 * time.Millisecond):
	}
	close(releaseTransport)
	if err := receive(t, closeResult); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
	if err := awaitDone(t, done); err != nil {
		t.Fatalf("Run() error = %v", err)
	}
	if session.State() != rtvbp.SessionStateClosed {
		t.Fatalf("state = %s, want closed", session.State())
	}
}

func TestCloseJoinsTransportAndCloseHandlerFailures(t *testing.T) {
	defer goleak.VerifyNone(t)

	transportFailure := errors.New("transport close failed")
	handlerFailure := errors.New("close handler failed")
	left, _ := memory.NewPair()
	transport := &failingCloseTransport{Transport: left, failure: transportFailure}
	session, done := runSession(t, transport, rtvbp.NewHandler(rtvbp.HandlerConfig{}))
	session.OnClose(func(context.Context) error { return handlerFailure })
	if err := session.Close(testContext(t)); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
	err := awaitDone(t, done)
	for _, want := range []error{transportFailure, handlerFailure} {
		if !errors.Is(err, want) {
			t.Fatalf("Run() error = %v, want joined %v", err, want)
		}
	}
	if session.State() != rtvbp.SessionStateFailed {
		t.Fatalf("state = %s, want failed", session.State())
	}
}

func TestCloseCancelsTransportFactoryWhileConnecting(t *testing.T) {
	defer goleak.VerifyNone(t)

	factoryCanceled := make(chan struct{})
	session := rtvbp.NewSession(
		v1classic.Envelope{},
		rtvbp.WithTransportFactory(func(ctx context.Context, _ rtvbp.Envelope) (rtvbp.Transport, error) {
			<-ctx.Done()
			close(factoryCanceled)
			return nil, ctx.Err()
		}),
		rtvbp.WithHandler(rtvbp.NewHandler(rtvbp.HandlerConfig{})),
	)
	done := session.Run(context.Background())
	if session.State() != rtvbp.SessionStateConnecting {
		t.Fatalf("state = %s, want connecting", session.State())
	}
	if err := session.Close(testContext(t)); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
	receiveSignal(t, factoryCanceled)
	if err := awaitDone(t, done); err != nil {
		t.Fatalf("Run() error = %v", err)
	}
	if session.State() != rtvbp.SessionStateClosed {
		t.Fatalf("state = %s, want closed", session.State())
	}
}

func TestCanceledTransportFactoryClosesLateTransportBeforeSessionCompletes(t *testing.T) {
	defer goleak.VerifyNone(t)

	left, _ := memory.NewPair()
	closed := make(chan struct{})
	transport := &trackingCloseTransport{Transport: left, closed: closed}
	session := rtvbp.NewSession(
		v1classic.Envelope{},
		rtvbp.WithCloseTimeout(time.Second),
		rtvbp.WithTransportFactory(func(ctx context.Context, _ rtvbp.Envelope) (rtvbp.Transport, error) {
			<-ctx.Done()
			return transport, ctx.Err()
		}),
		rtvbp.WithHandler(rtvbp.NewHandler(rtvbp.HandlerConfig{})),
	)
	done := session.Run(context.Background())
	if err := session.Close(testContext(t)); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
	receiveSignal(t, closed)
	if err := awaitDone(t, done); err != nil {
		t.Fatalf("Run() error = %v", err)
	}
}

func TestTransportFactoryErrorClosesPartialTransport(t *testing.T) {
	defer goleak.VerifyNone(t)

	want := errors.New("dial failed")
	left, _ := memory.NewPair()
	closed := make(chan struct{})
	transport := &trackingCloseTransport{Transport: left, closed: closed}
	session := rtvbp.NewSession(
		v1classic.Envelope{},
		rtvbp.WithTransportFactory(func(context.Context, rtvbp.Envelope) (rtvbp.Transport, error) {
			return transport, want
		}),
		rtvbp.WithHandler(rtvbp.NewHandler(rtvbp.HandlerConfig{})),
	)
	done := session.Run(context.Background())
	if err := awaitDone(t, done); !errors.Is(err, want) {
		t.Fatalf("Run() error = %v, want %v", err, want)
	}
	receiveSignal(t, closed)
	if session.State() != rtvbp.SessionStateFailed {
		t.Fatalf("state = %s, want failed", session.State())
	}
}

func TestLiveControlReaderCancellationFailsSession(t *testing.T) {
	defer goleak.VerifyNone(t)

	left, _ := memory.NewPair()
	transport := &controlOverrideTransport{Transport: left, control: canceledControlChannel{}}
	session := rtvbp.NewSession(
		v1classic.Envelope{},
		rtvbp.WithTransport(transport),
		rtvbp.WithHandler(rtvbp.NewHandler(rtvbp.HandlerConfig{})),
	)
	done := session.Run(context.Background())
	if err := awaitDone(t, done); !errors.Is(err, context.Canceled) {
		t.Fatalf("Run() error = %v, want context.Canceled", err)
	}
	if session.State() != rtvbp.SessionStateFailed {
		t.Fatalf("state = %s, want failed", session.State())
	}
}

func TestLiveKeepaliveMonitorNilReturnFailsSession(t *testing.T) {
	defer goleak.VerifyNone(t)

	left, _ := memory.NewPair()
	transport := stoppedKeepaliveTransport{Transport: left}
	session := rtvbp.NewSession(
		v1classic.Envelope{},
		rtvbp.WithTransport(transport),
		rtvbp.WithKeepalivePolicy(rtvbp.KeepalivePolicy{Interval: time.Second, Timeout: time.Second, MaxMisses: 1}),
		rtvbp.WithHandler(rtvbp.NewHandler(rtvbp.HandlerConfig{})),
	)
	done := session.Run(context.Background())
	if err := awaitDone(t, done); err == nil {
		t.Fatal("Run() error = nil, want keepalive monitor failure")
	}
	if session.State() != rtvbp.SessionStateFailed {
		t.Fatalf("state = %s, want failed", session.State())
	}
}

type blockingCloseTransport struct {
	rtvbp.Transport
	started chan struct{}
	release <-chan struct{}
	once    sync.Once
}

func (t *blockingCloseTransport) Close(ctx context.Context) error {
	t.once.Do(func() { close(t.started) })
	select {
	case <-ctx.Done():
		return ctx.Err()
	case <-t.release:
		return t.Transport.Close(ctx)
	}
}

type trackingCloseTransport struct {
	rtvbp.Transport
	closed chan struct{}
	once   sync.Once
}

type failingCloseTransport struct {
	rtvbp.Transport
	failure error
}

func (t *failingCloseTransport) Close(ctx context.Context) error {
	return errors.Join(t.Transport.Close(ctx), t.failure)
}

func (t *trackingCloseTransport) Close(ctx context.Context) error {
	t.once.Do(func() { close(t.closed) })
	return t.Transport.Close(ctx)
}

type controlOverrideTransport struct {
	rtvbp.Transport
	control rtvbp.ControlChannel
}

func (t *controlOverrideTransport) Control() rtvbp.ControlChannel { return t.control }

type canceledControlChannel struct{}

func (canceledControlChannel) Send(context.Context, []byte) error { return nil }
func (canceledControlChannel) Recv(context.Context) (rtvbp.Received, error) {
	return rtvbp.Received{}, context.Canceled
}

type stoppedKeepaliveTransport struct{ rtvbp.Transport }

func (stoppedKeepaliveTransport) MonitorKeepalive(context.Context, rtvbp.KeepalivePolicy) error {
	return nil
}

func runSession(t *testing.T, transport rtvbp.Transport, handler rtvbp.SessionHandler) (*rtvbp.Session, <-chan error) {
	t.Helper()
	var next atomic.Int64
	session := rtvbp.NewSession(
		v1classic.Envelope{},
		rtvbp.WithTransport(transport),
		rtvbp.WithHandler(handler),
		rtvbp.WithIDGenerator(func() string { return fmt.Sprintf("request-%d", next.Add(1)) }),
		rtvbp.WithRequestTimeout(2*time.Second),
		rtvbp.WithCloseTimeout(2*time.Second),
	)
	done := session.Run(context.Background())
	waitState(t, session, rtvbp.SessionStateActive)
	return session, done
}

func sendFrame(t *testing.T, peer rtvbp.Transport, frame rtvbp.ControlFrame) {
	t.Helper()
	data, err := (v1classic.Envelope{}).Encode(frame)
	if err != nil {
		t.Fatalf("Encode() error = %v", err)
	}
	if err := peer.Control().Send(testContext(t), data); err != nil {
		t.Fatalf("Send() error = %v", err)
	}
}

func recvFrame(t *testing.T, peer rtvbp.Transport) rtvbp.ControlFrame {
	t.Helper()
	received, err := peer.Control().Recv(testContext(t))
	if err != nil {
		t.Fatalf("Recv() error = %v", err)
	}
	frame, err := (v1classic.Envelope{}).Decode(received.Data)
	if err != nil {
		t.Fatalf("Decode() error = %v", err)
	}
	return frame
}

func closeSession(t *testing.T, session *rtvbp.Session, done <-chan error) {
	t.Helper()
	if err := session.Close(testContext(t)); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
	if err := awaitDone(t, done); err != nil {
		t.Fatalf("Run() error = %v", err)
	}
}

func awaitDone(t *testing.T, done <-chan error) error {
	t.Helper()
	select {
	case err := <-done:
		return err
	case <-time.After(3 * time.Second):
		t.Fatal("session did not finish")
		return nil
	}
}

func waitState(t *testing.T, session *rtvbp.Session, want rtvbp.SessionState) {
	t.Helper()
	deadline := time.Now().Add(3 * time.Second)
	for time.Now().Before(deadline) {
		if session.State() == want {
			return
		}
		time.Sleep(time.Millisecond)
	}
	t.Fatalf("state = %s, want %s", session.State(), want)
}

func testContext(t *testing.T) context.Context {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	t.Cleanup(cancel)
	return ctx
}

func receive[T any](t *testing.T, channel <-chan T) T {
	t.Helper()
	select {
	case value := <-channel:
		return value
	case <-time.After(3 * time.Second):
		t.Fatal("timed out waiting for value")
		var zero T
		return zero
	}
}

func receiveSignal(t *testing.T, channel <-chan struct{}) {
	t.Helper()
	receive(t, channel)
}

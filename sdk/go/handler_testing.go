package rtvbp

import (
	"context"
	"log/slog"
	"sync"
)

type TestingSHC struct {
	Response Response
	state    SessionState
	deferred bool
	sent     bool
	mu       sync.Mutex
	logger   *slog.Logger
}

func (t *TestingSHC) SessionID() string { return "test" }
func (t *TestingSHC) Log() *slog.Logger { return t.logger }

func (t *TestingSHC) Request(context.Context, NamedRequest) (Response, error) {
	return Response{}, ErrSessionClosed
}

func (t *TestingSHC) Respond(_ context.Context, response Response) error {
	t.mu.Lock()
	defer t.mu.Unlock()
	if t.sent {
		return ErrResponseAlreadySent
	}
	t.Response = response
	t.sent = true
	return nil
}

func (t *TestingSHC) RespondThenClose(ctx context.Context, response Response) error {
	if err := t.Respond(ctx, response); err != nil {
		return err
	}
	t.mu.Lock()
	t.state = SessionStateClosed
	t.mu.Unlock()
	return nil
}

func (t *TestingSHC) DeferResponse() (DeferredResponse, error) {
	t.mu.Lock()
	defer t.mu.Unlock()
	if t.sent || t.deferred {
		return nil, ErrResponseAlreadySent
	}
	t.deferred = true
	return &testingDeferred{handler: t}, nil
}

func (t *TestingSHC) Notify(context.Context, NamedEvent) error { return nil }
func (t *TestingSHC) AudioStream() HandlerAudio                { return nil }

func (t *TestingSHC) Close(context.Context) error {
	t.mu.Lock()
	t.state = SessionStateClosed
	t.mu.Unlock()
	return nil
}

func (t *TestingSHC) State() SessionState {
	t.mu.Lock()
	defer t.mu.Unlock()
	return t.state
}

type testingDeferred struct{ handler *TestingSHC }

func (d *testingDeferred) Respond(ctx context.Context, response Response) error {
	return d.handler.Respond(ctx, response)
}
func (d *testingDeferred) RespondThenClose(ctx context.Context, response Response) error {
	return d.handler.RespondThenClose(ctx, response)
}

func NewTestingSHC() *TestingSHC {
	return &TestingSHC{state: SessionStateActive, logger: slog.Default()}
}

var _ SHC = (*TestingSHC)(nil)

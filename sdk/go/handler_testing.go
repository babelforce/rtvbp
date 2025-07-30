package rtvbp

import (
	"context"
	"log/slog"
	"sync"

	"github.com/babelforce/rtvbp-go/proto"
)

type TestingSHC struct {
	Response *proto.Response
	state    SessionState
	mu       sync.Mutex
}

func (t *TestingSHC) SessionID() string {
	//TODO implement me
	panic("implement me")
}

func (t *TestingSHC) Log() *slog.Logger {
	//TODO implement me
	panic("implement me")
}

func (t *TestingSHC) Request(ctx context.Context, req NamedRequest) (*proto.Response, error) {
	//TODO implement me
	panic("implement me")
}

func (t *TestingSHC) Respond(ctx context.Context, res *proto.Response) error {
	t.Response = res
	return nil
}

func (t *TestingSHC) Notify(ctx context.Context, evt NamedEvent) error {
	//TODO implement me
	panic("implement me")
}

func (t *TestingSHC) AudioStream() HandlerAudio {
	//TODO implement me
	panic("implement me")
}

func (t *TestingSHC) Close(ctx context.Context, cb func(context.Context, SHC) error) error {
	defer func() {
		t.mu.Lock()
		defer t.mu.Unlock()
		t.state = SessionStateClosed
	}()
	return cb(ctx, t)
}

func (t *TestingSHC) State() SessionState {
	return t.state
}

var _ SHC = &TestingSHC{}

func NewTestingSHC() *TestingSHC {
	return &TestingSHC{
		state: SessionStateActive,
	}
}

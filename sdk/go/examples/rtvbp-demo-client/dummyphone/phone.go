package dummyphone

import (
	"context"
	"fmt"
	"github.com/babelforce/rtvbp-go/proto/protov1"
	"log/slog"
	"sync"
	"time"
)

type PhoneSystem struct {
	log          *slog.Logger
	mu           sync.Mutex
	closed       bool
	cancel       context.CancelFunc
	onHangupFunc func()
}

func (d *PhoneSystem) OnHangup(f func()) {
	d.onHangupFunc = f
}

func (d *PhoneSystem) SimulateUserHangup() error {
	d.mu.Lock()
	defer d.mu.Unlock()

	if d.closed {
		return fmt.Errorf("telephony: already shutdown")
	}

	// simulate some delay
	<-time.After(100 * time.Millisecond)

	if d.onHangupFunc != nil {
		d.onHangupFunc()
	}
	<-time.After(1 * time.Second)
	d.cancel()

	return nil
}

func endWith[R any](d *PhoneSystem, r *R) (*R, error) {
	if err := d.SimulateUserHangup(); err != nil {
		return nil, err
	}
	return r, nil
}

func (d *PhoneSystem) Hangup(_ context.Context, req *protov1.CallHangupRequest) (*protov1.CallHangupResponse, error) {
	d.log.Info("hangup", slog.Any("req", req))
	return endWith(d, &protov1.CallHangupResponse{})
}

func (d *PhoneSystem) Move(_ context.Context, req *protov1.ApplicationMoveRequest) (*protov1.ApplicationMoveResponse, error) {
	d.log.Info("move", slog.Any("req", req))
	return endWith(d, &protov1.ApplicationMoveResponse{})
}

func New(log *slog.Logger) (*PhoneSystem, context.Context) {
	ctx, cancelCtxFunc := context.WithCancel(context.Background())

	return &PhoneSystem{
		log:    log,
		cancel: cancelCtxFunc,
	}, ctx
}

var _ protov1.TelephonyAdapter = &PhoneSystem{}

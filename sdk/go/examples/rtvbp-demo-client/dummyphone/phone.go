package dummyphone

import (
	"context"
	"fmt"
	"log/slog"
	"sync"
	"time"

	"github.com/babelforce/rtvbp-go/proto/protov1"
)

type PhoneSystem struct {
	log      *slog.Logger
	mu       sync.Mutex
	closed   bool
	cancel   context.CancelFunc
	onHangup protov1.TelephonyHangupHandler
	onDtmf   protov1.TelephonyDtmfHandler
}

func (d *PhoneSystem) OnDTMF(onDtmf protov1.TelephonyDtmfHandler) error {
	d.mu.Lock()
	defer d.mu.Unlock()
	if d.onDtmf != nil {
		return fmt.Errorf("dtmf event handler already set")
	}
	d.onDtmf = onDtmf
	return nil
}

func (d *PhoneSystem) OnHangup(onHangup protov1.TelephonyHangupHandler) error {
	d.mu.Lock()
	defer d.mu.Unlock()
	if d.onHangup != nil {
		return fmt.Errorf("hangup event handler already set")
	}
	d.onHangup = onHangup
	return nil
}

func (d *PhoneSystem) SessionVariablesSet(ctx context.Context, req *protov1.SessionSetRequest) error {
	//TODO implement me
	panic("implement me")
}

func (d *PhoneSystem) SessionVariablesGet(ctx context.Context, req *protov1.SessionGetRequest) (map[string]any, error) {
	//TODO implement me
	panic("implement me")
}

func (d *PhoneSystem) RecordingStart(ctx context.Context, req *protov1.RecordingStartRequest) (*protov1.RecordingStartResponse, error) {
	//TODO implement me
	panic("implement me")
}

func (d *PhoneSystem) RecordingStop(ctx context.Context, recordingID string) error {
	//TODO implement me
	panic("implement me")
}

func (d *PhoneSystem) EmulateDTMF(digit string) {
	for _, c := range digit {
		evt := &protov1.DTMFEvent{
			Digit:     string(c),
			PressedAt: time.Now().UnixMilli(),
		}
		<-time.After(750 * time.Millisecond)
		evt.ReleasedAt = time.Now().UnixMilli()
		println("emulate DTMF>:", evt.Digit, ":", evt.String())
		d.onDtmf(evt)
	}
}

func (d *PhoneSystem) EmulateHangup() error {
	d.mu.Lock()
	defer d.mu.Unlock()

	if d.closed {
		return fmt.Errorf("telephony: already shutdown")
	}

	// simulate some delay
	<-time.After(100 * time.Millisecond)

	if d.onHangup != nil {
		d.onHangup(&protov1.CallHangupEvent{})
	}
	<-time.After(1 * time.Second)
	d.cancel()

	return nil
}

func (d *PhoneSystem) Hangup(_ context.Context, req *protov1.CallHangupRequest) error {
	d.log.Info("hangup", slog.Any("req", req))
	err := d.EmulateHangup()
	if err != nil {
		return err
	}
	return nil
}

func (d *PhoneSystem) Move(_ context.Context, req *protov1.ApplicationMoveRequest) (*protov1.ApplicationMoveResponse, error) {
	d.log.Info("move", slog.Any("req", req))
	err := d.EmulateHangup()
	if err != nil {
		return nil, err
	}
	return &protov1.ApplicationMoveResponse{}, nil
}

func New(log *slog.Logger) (*PhoneSystem, context.Context) {
	ctx, cancelCtxFunc := context.WithCancel(context.Background())

	return &PhoneSystem{
		log:    log,
		cancel: cancelCtxFunc,
	}, ctx
}

var _ protov1.TelephonyAdapter = &PhoneSystem{}

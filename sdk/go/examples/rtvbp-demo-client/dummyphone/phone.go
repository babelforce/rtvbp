package dummyphone

import (
	"context"
	"fmt"
	"log/slog"
	"sync"
	"time"

	v1bridge "github.com/babelforce/rtvbp/sdk/go/bridge/babelforcev1"
	v1 "github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
)

type PhoneSystem struct {
	log              *slog.Logger
	mu               sync.Mutex
	dtmfMu           sync.Mutex
	closed           bool
	cancel           context.CancelFunc
	onHangup         v1bridge.TelephonyHangupHandler
	onDtmf           v1bridge.TelephonyDtmfHandler
	nextDtmfSeq      int
	nextRecordingID  uint64
	sessionVariables map[string]any
	activeRecordings map[string][]string
}

func (d *PhoneSystem) OnDTMF(onDtmf v1bridge.TelephonyDtmfHandler) error {
	if onDtmf == nil {
		return fmt.Errorf("dtmf event handler is nil")
	}
	d.mu.Lock()
	defer d.mu.Unlock()
	if d.closed {
		return fmt.Errorf("telephony: already shutdown")
	}
	if d.onDtmf != nil {
		return fmt.Errorf("dtmf event handler already set")
	}
	d.onDtmf = onDtmf
	return nil
}

func (d *PhoneSystem) OnHangup(onHangup v1bridge.TelephonyHangupHandler) error {
	if onHangup == nil {
		return fmt.Errorf("hangup event handler is nil")
	}
	d.mu.Lock()
	defer d.mu.Unlock()
	if d.closed {
		return fmt.Errorf("telephony: already shutdown")
	}
	if d.onHangup != nil {
		return fmt.Errorf("hangup event handler already set")
	}
	d.onHangup = onHangup
	return nil
}

func (d *PhoneSystem) SessionVariablesSet(ctx context.Context, req *v1.SessionSetRequest) error {
	if err := contextError(ctx); err != nil {
		return err
	}
	if req == nil {
		return fmt.Errorf("session set request is nil")
	}

	d.mu.Lock()
	defer d.mu.Unlock()
	for key, value := range req.Data {
		d.sessionVariables[key] = value
	}
	return nil
}

func (d *PhoneSystem) SessionVariablesGet(ctx context.Context, req *v1.SessionGetRequest) (map[string]any, error) {
	if err := contextError(ctx); err != nil {
		return nil, err
	}
	if req == nil {
		return nil, fmt.Errorf("session get request is nil")
	}

	d.mu.Lock()
	defer d.mu.Unlock()
	values := make(map[string]any)
	if len(req.Keys) == 0 {
		for key, value := range d.sessionVariables {
			values[key] = value
		}
		return values, nil
	}
	for _, key := range req.Keys {
		if value, ok := d.sessionVariables[key]; ok {
			values[key] = value
		}
	}
	return values, nil
}

func (d *PhoneSystem) RecordingStart(ctx context.Context, req *v1.RecordingStartRequest) (*v1.RecordingStartResponse, error) {
	if err := contextError(ctx); err != nil {
		return nil, err
	}
	if req == nil {
		return nil, fmt.Errorf("recording start request is nil")
	}

	d.mu.Lock()
	d.nextRecordingID++
	recordingID := fmt.Sprintf("recording-%d", d.nextRecordingID)
	d.activeRecordings[recordingID] = append([]string(nil), req.Tags...)
	d.mu.Unlock()
	d.log.Info("start recording", slog.String("recording_id", recordingID), slog.Any("tags", req.Tags))
	return &v1.RecordingStartResponse{ID: recordingID}, nil
}

func (d *PhoneSystem) RecordingStop(ctx context.Context, recordingID string) error {
	if err := contextError(ctx); err != nil {
		return err
	}
	if recordingID == "" {
		return fmt.Errorf("recording ID is empty")
	}

	d.mu.Lock()
	if _, ok := d.activeRecordings[recordingID]; !ok {
		d.mu.Unlock()
		return fmt.Errorf("recording %q is not active", recordingID)
	}
	delete(d.activeRecordings, recordingID)
	d.mu.Unlock()
	d.log.Info("stop recording", slog.String("recording_id", recordingID))
	return nil
}

func (d *PhoneSystem) EmulateDTMF(digits string) {
	d.dtmfMu.Lock()
	defer d.dtmfMu.Unlock()

	d.mu.Lock()
	if d.closed {
		d.mu.Unlock()
		d.log.Warn("skip DTMF after shutdown")
		return
	}
	if d.onDtmf == nil {
		d.mu.Unlock()
		d.log.Warn("skip DTMF before handler registration")
		return
	}
	d.mu.Unlock()

	for _, digit := range digits {
		d.mu.Lock()
		if d.closed {
			d.mu.Unlock()
			d.log.Warn("stop DTMF sequence after shutdown")
			return
		}
		onDtmf := d.onDtmf
		sequence := d.nextDtmfSeq
		d.nextDtmfSeq++
		d.mu.Unlock()

		evt := &v1.DtmfEvent{
			Seq:       sequence,
			Digit:     string(digit),
			PressedAt: time.Now().UnixMilli(),
		}
		time.Sleep(750 * time.Millisecond)
		evt.ReleasedAt = time.Now().UnixMilli()
		d.log.Info("emulate DTMF", slog.String("digit", evt.Digit))
		onDtmf(evt)
	}
}

func (d *PhoneSystem) EmulateHangup(reason string) error {
	if reason == "" {
		return fmt.Errorf("hangup reason is empty")
	}
	d.mu.Lock()
	if d.closed {
		d.mu.Unlock()
		return fmt.Errorf("telephony: already shutdown")
	}
	d.closed = true
	onHangup := d.onHangup
	cancel := d.cancel
	d.mu.Unlock()
	defer cancel()

	// simulate some delay
	time.Sleep(100 * time.Millisecond)

	if onHangup != nil {
		onHangup(&v1.CallHangupEvent{Reason: reason})
	}
	time.Sleep(1 * time.Second)

	return nil
}

func (d *PhoneSystem) Hangup(ctx context.Context, req *v1.CallHangupRequest) error {
	if err := contextError(ctx); err != nil {
		return err
	}
	if req == nil {
		return fmt.Errorf("call hangup request is nil")
	}
	d.log.Info("hangup", slog.Any("req", req))
	return d.EmulateHangup(req.Reason)
}

func (d *PhoneSystem) Move(ctx context.Context, req *v1.ApplicationMoveRequest) (*v1.ApplicationMoveResponse, error) {
	if err := contextError(ctx); err != nil {
		return nil, err
	}
	if req == nil {
		return nil, fmt.Errorf("application move request is nil")
	}
	d.log.Info("move", slog.Any("req", req))
	err := d.EmulateHangup(req.MethodName())
	if err != nil {
		return nil, err
	}
	return &v1.ApplicationMoveResponse{}, nil
}

func New(log *slog.Logger) (*PhoneSystem, context.Context) {
	if log == nil {
		log = slog.Default()
	}
	ctx, cancelCtxFunc := context.WithCancel(context.Background())

	return &PhoneSystem{
		log:              log,
		cancel:           cancelCtxFunc,
		sessionVariables: make(map[string]any),
		activeRecordings: make(map[string][]string),
	}, ctx
}

func contextError(ctx context.Context) error {
	if ctx == nil {
		return fmt.Errorf("context is nil")
	}
	return ctx.Err()
}

var _ v1bridge.TelephonyAdapter = &PhoneSystem{}

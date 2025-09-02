package protov1

import (
	"context"
	"fmt"
	"sync"
	"time"

	"github.com/babelforce/rtvbp-go"
	"github.com/babelforce/rtvbp-go/internal/idgen"
)

type TelephonyDtmfHandler func(dtmf *DTMFEvent)
type TelephonyHangupHandler func(hangup *CallHangupEvent)

type TelephonyAdapter interface {
	// Move moves the call to another application
	Move(ctx context.Context, req *ApplicationMoveRequest) (*ApplicationMoveResponse, error)

	// Hangup hangs up the call
	Hangup(ctx context.Context, req *CallHangupRequest) error

	// SessionVariablesSet sets session variables
	SessionVariablesSet(ctx context.Context, req *SessionSetRequest) error

	// SessionVariablesGet gets session variables
	SessionVariablesGet(ctx context.Context, req *SessionGetRequest) (map[string]any, error)

	// RecordingStart starts recording
	RecordingStart(ctx context.Context, req *RecordingStartRequest) (*RecordingStartResponse, error)

	// RecordingStop stops recording
	RecordingStop(ctx context.Context, recordingID string) error

	// OnDTMF sets the callback for DTMF events
	OnDTMF(onDtmf TelephonyDtmfHandler) error

	// OnHangup sets the callback for hangup events
	OnHangup(onHangup TelephonyHangupHandler) error
}

type FakeTelephonyAdapter struct {
	mu     sync.Mutex
	vars   map[string]any
	moved  *ApplicationMoveRequest
	hangup bool
	events chan rtvbp.NamedEvent
	// event handlers
	dtmfHandler   TelephonyDtmfHandler
	hangupHandler TelephonyHangupHandler
}

// Run starts the fake telephony adapter
func (f *FakeTelephonyAdapter) Run(ctx context.Context) {
	// start goroutine to process faked events
	go func() {
		for {
			select {
			case <-ctx.Done():
				return
			case d := <-f.events:
				switch e := d.(type) {
				case *DTMFEvent:
					f.dtmfHandler(e)
				case *CallHangupEvent:
					f.hangupHandler(e)
				}
			}
		}
	}()
}

// fakeSendDTMF simulates a DTMF digit being pressed
// Note: calling this function equals to releasing the DTMF button
func (f *FakeTelephonyAdapter) fakeSendDTMF(digit rune, duration time.Duration) {
	println("fakeSendDTMF>", string(digit), duration.String())
	now := time.Now()
	f.events <- &DTMFEvent{
		Digit:      string(digit),
		PressedAt:  now.Add(-duration).UnixMilli(),
		ReleasedAt: now.UnixMilli(),
	}
}

func (f *FakeTelephonyAdapter) fakeHangup() {
	f.events <- &CallHangupEvent{}
}

func (f *FakeTelephonyAdapter) OnDTMF(cb TelephonyDtmfHandler) error {
	f.mu.Lock()
	defer f.mu.Unlock()
	if f.dtmfHandler != nil {
		return fmt.Errorf("dtmf event handler already set")
	}
	f.dtmfHandler = cb
	return nil
}

func (f *FakeTelephonyAdapter) OnHangup(cb TelephonyHangupHandler) error {
	f.mu.Lock()
	defer f.mu.Unlock()
	if f.hangupHandler != nil {
		return fmt.Errorf("hangup event handler already set")
	}
	f.hangupHandler = cb
	return nil
}

func (f *FakeTelephonyAdapter) RecordingStart(ctx context.Context, req *RecordingStartRequest) (*RecordingStartResponse, error) {
	return &RecordingStartResponse{
		ID: idgen.ID(),
	}, nil
}

func (f *FakeTelephonyAdapter) RecordingStop(ctx context.Context, recordingID string) error {
	if recordingID == "" {
		return fmt.Errorf("recordingID is required")
	}
	return nil
}

func (f *FakeTelephonyAdapter) SessionVariablesGet(ctx context.Context, req *SessionGetRequest) (map[string]any, error) {
	f.mu.Lock()
	defer f.mu.Unlock()

	var out = make(map[string]any)
	for _, k := range req.Keys {
		v, ok := f.vars[k]
		if ok {
			out[k] = v
		}
	}
	return out, nil
}

func (f *FakeTelephonyAdapter) SessionVariablesSet(ctx context.Context, req *SessionSetRequest) error {
	f.mu.Lock()
	defer f.mu.Unlock()

	for k, v := range req.Data {
		f.vars[k] = v
	}
	return nil
}

func (f *FakeTelephonyAdapter) Move(ctx context.Context, req *ApplicationMoveRequest) (*ApplicationMoveResponse, error) {
	f.moved = req
	next := req.ApplicationID
	if next == "" {
		next = "<id_of_next_node_if_any>"
	}
	return &ApplicationMoveResponse{
		NextApplicationID: next,
	}, nil
}

func (f *FakeTelephonyAdapter) Hangup(ctx context.Context, req *CallHangupRequest) error {
	f.hangup = true
	return nil
}

var _ TelephonyAdapter = &FakeTelephonyAdapter{}

func newFakeTelephonyAdapter() *FakeTelephonyAdapter {
	return &FakeTelephonyAdapter{
		vars:   make(map[string]any),
		events: make(chan rtvbp.NamedEvent, 1),
	}
}

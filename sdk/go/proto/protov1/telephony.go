package protov1

import (
	"context"
	"fmt"
	"sync"

	"github.com/babelforce/rtvbp-go/proto"
)

type TelephonyAdapter interface {
	Move(ctx context.Context, req *ApplicationMoveRequest) (*ApplicationMoveResponse, error)
	Hangup(ctx context.Context, req *CallHangupRequest) (*CallHangupResponse, error)
	SessionVariablesSet(ctx context.Context, req *SessionSetRequest) error
	SessionVariablesGet(ctx context.Context, req *SessionGetRequest) (map[string]any, error)
	RecordingStart(ctx context.Context, req *RecordingStartRequest) (*RecordingStartResponse, error)
	RecordingStop(ctx context.Context, recordingID string) error
	// Play(prompt, etc)
}

type FakeTelephonyAdapter struct {
	mu     sync.Mutex
	vars   map[string]any
	moved  *ApplicationMoveRequest
	hangup bool
}

func (f *FakeTelephonyAdapter) RecordingStart(ctx context.Context, req *RecordingStartRequest) (*RecordingStartResponse, error) {
	return &RecordingStartResponse{
		ID: proto.ID(),
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

func (f *FakeTelephonyAdapter) Hangup(ctx context.Context, req *CallHangupRequest) (*CallHangupResponse, error) {
	f.hangup = true
	return &CallHangupResponse{}, nil
}

var _ TelephonyAdapter = &FakeTelephonyAdapter{}

func newFakeTelephonyAdapter() *FakeTelephonyAdapter {
	return &FakeTelephonyAdapter{
		vars: make(map[string]any),
	}
}

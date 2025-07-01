package protov1

import "context"

type TelephonyAdapter interface {
	Move(ctx context.Context, req *ApplicationMoveRequest) (*ApplicationMoveResponse, error)
	Hangup(ctx context.Context) error
	// Play(prompt, etc)
}

type FakeTelephonyAdapter struct {
	moved  *ApplicationMoveRequest
	hangup bool
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

func (f *FakeTelephonyAdapter) Hangup(ctx context.Context) error {
	f.hangup = true
	return nil
}

var _ TelephonyAdapter = &FakeTelephonyAdapter{}

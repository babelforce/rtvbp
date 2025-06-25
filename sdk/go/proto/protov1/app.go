package protov1

import (
	"babelforce.go/ivr/rtvbp/rtvbp-go"
	"context"
)

type ApplicationMoveRequest struct {
	ApplicationID string `json:"application_id,omitempty"`
	Continue      bool   `json:"continue,omitempty"`
}

func (m *ApplicationMoveRequest) MethodName() string {
	return "application.move"
}

func (m *ApplicationMoveRequest) PostResponseHook(ctx context.Context, hc rtvbp.SHC) error {
	return terminateAndClose("application.move")(ctx, hc)
}

type ApplicationMoveResponse struct {
}

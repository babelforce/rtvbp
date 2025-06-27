package protov1

import (
	"context"
	"github.com/babelforce/rtvbp-go"
)

// ApplicationMoveRequest moves to another location in the IVR graph
// If ApplicationID is not specified the application will be moved to the
// next IVR graph node (if any)
type ApplicationMoveRequest struct {
	// Reason is an optional reason to specify
	Reason string `json:"reason,omitempty"`

	// ApplicationID is the ID of the IVR graph node to continue the flow in
	ApplicationID string `json:"application_id,omitempty"`
}

func (m *ApplicationMoveRequest) MethodName() string {
	return "application.move"
}

func (m *ApplicationMoveRequest) PostResponseHook(ctx context.Context, hc rtvbp.SHC) error {
	return terminateAndClose("application.move")(ctx, hc)
}

type ApplicationMoveResponse struct {
}

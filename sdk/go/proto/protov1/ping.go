package protov1

import (
	"context"
	"fmt"
	"time"

	"github.com/babelforce/rtvbp-go"
	"github.com/babelforce/rtvbp-go/proto"
)

type PingRequest struct {
	T0   int64 `json:"t0"`             // T0 is the time the sender of the PingRequest provided.
	Data any   `json:"data,omitempty"` // Data is arbitrary data that can be sent along with the PingRequest.
}

func (r *PingRequest) Validate() error {
	if r.T0 == 0 {
		return fmt.Errorf("ping request T0 is required")
	}
	return nil
}

func (r *PingRequest) MethodName() string {
	return "ping"
}

func NewPingRequest(data any) *PingRequest {
	return &PingRequest{
		T0:   time.Now().UnixMilli(),
		Data: data,
	}
}

type PingResponse struct {
	T0   int64 `json:"t0"`             // T0 is the time provided in PingRequest.T0
	T1   int64 `json:"t1"`             // T1 is the time the peer received the PingRequest from the transport layer
	T2   int64 `json:"t2"`             // T2 is the time the PingRequest is being received and handled by the application layer
	OWD  int64 `json:"owd"`            // OWD is the one-way delay of message traversal between requester to the responder
	Data any   `json:"data,omitempty"` // Data is the data provided in PingRequest.Data.
}

func (r *PingResponse) Validate() error {
	if r.T0 == 0 {
		return fmt.Errorf("ping response T0 is required")
	}

	if r.T1 == 0 {
		return fmt.Errorf("ping response T1 is required")
	}

	if r.T2 == 0 {
		return fmt.Errorf("ping response T2 is required")
	}

	return nil
}

func NewPingHandler() rtvbp.RequestHandler {
	return rtvbp.HandleRequest(func(ctx context.Context, hc rtvbp.SHC, req *PingRequest) (*PingResponse, error) {

		r, ok := ctx.Value("request").(*proto.Request)
		if !ok {
			return nil, fmt.Errorf("failed to extract original request from context")
		}

		t2 := time.Now().UnixMilli()
		return &PingResponse{
			T0:   req.T0,
			T1:   r.GetReceivedAt(),
			T2:   t2,
			OWD:  t2 - req.T0,
			Data: req.Data,
		}, nil
	})
}

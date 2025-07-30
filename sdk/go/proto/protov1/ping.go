package protov1

import (
	"context"
	"fmt"
	"log/slog"
	"time"

	"github.com/babelforce/rtvbp-go"
	"github.com/babelforce/rtvbp-go/proto"
)

// PingRequest ping request
type PingRequest struct {
	T0   int64 `json:"t0"`             // T0 is the time the sender of the PingRequest provided.
	RTT  int64 `json:"rtt,omitempty"`  // RTT is the round trip time measured by a previous PingRequest
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

func NewPingRequest() *PingRequest {
	return &PingRequest{
		T0: time.Now().UnixMilli(),
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

func ping(ctx context.Context, h rtvbp.SHC, lastRTT int64) (int, error) {

	ctx, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()

	pingReq := NewPingRequest()
	pingReq.RTT = lastRTT

	res, err := h.Request(ctx, pingReq)
	if err != nil {
		return 0, err
	}

	pingRes, err := proto.As[PingResponse](res.Result)
	if err != nil {
		return 0, err
	}

	receivedAt := time.Now().UnixMilli()
	rtt := time.Duration(receivedAt-pingReq.T0) * time.Millisecond
	owd := time.Duration(pingRes.OWD) * time.Millisecond
	h.Log().Debug(
		"ping response",
		slog.Duration("owd", owd),
		slog.Duration("rtt", rtt),
	)

	return int(rtt.Milliseconds()), nil
}

func StartPinger(ctx context.Context, pingInterval time.Duration, h rtvbp.SHC) {

	if pingInterval == 0 {
		pingInterval = 10 * time.Second
	}
	h.Log().Debug("starting pinger", slog.Any("interval", pingInterval))

	pingTicker := time.NewTicker(pingInterval)
	defer pingTicker.Stop()

	lastRTT := int64(0)

	for {
		select {
		case <-pingTicker.C:
			rtt, err := ping(ctx, h, lastRTT)
			if err != nil {
				h.Log().Error("ping failed", slog.Any("err", err))
			}
			lastRTT = int64(rtt)
		case <-ctx.Done():
			h.Log().Info("pinger stopped")
			return
		}
	}
}

// NewPingHandler creates a request handler for PingRequest
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

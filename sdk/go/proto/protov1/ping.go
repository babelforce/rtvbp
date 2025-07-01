package protov1

import (
	"context"
	"github.com/babelforce/rtvbp-go"
	"log/slog"
	"time"
)

type PingRequest struct {
	Sequence  int   `json:"seq"`
	Timestamp int64 `json:"ts"`
	Data      any   `json:"data,omitempty"`
}

func (r *PingRequest) MethodName() string {
	return "ping"
}

type PingResponse struct {
	Sequence  int   `json:"seq"`
	Timestamp int64 `json:"ts"`
	Data      any   `json:"data,omitempty"`
}

func NewPingHandler() rtvbp.RequestHandler {
	return rtvbp.HandleRequest(func(ctx context.Context, hc rtvbp.SHC, req *PingRequest) (*PingResponse, error) {
		hc.Log().Info("ping request", slog.Any("request", req))
		return &PingResponse{
			Timestamp: time.Now().UnixMilli(),
			Sequence:  req.Sequence,
			Data:      req.Data,
		}, nil
	})
}

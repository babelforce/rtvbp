package babelforcev1

import (
	"context"
	"fmt"
	"log/slog"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
)

// NewPingHandler returns the babelforce.v1 measurement-ping responder. Catalog
// ping is separate from transport keepalive; applications register this helper
// when they do not otherwise need a full generated role implementation.
func NewPingHandler() rtvbp.RequestHandler {
	return rtvbp.HandleRequest(func(
		ctx context.Context,
		_ rtvbp.SHC,
		request *babelforcev1.PingRequest,
	) (*babelforcev1.PingResponse, error) {
		return pingResponse(ctx, request)
	})
}

func pingResponse(
	ctx context.Context,
	request *babelforcev1.PingRequest,
) (*babelforcev1.PingResponse, error) {
	inbound, ok := rtvbp.InboundRequest(ctx)
	if !ok {
		return nil, fmt.Errorf("failed to extract original request from context")
	}
	t2 := time.Now().UnixMilli()
	return &babelforcev1.PingResponse{
		T0:   request.T0,
		T1:   inbound.ReceivedAt.UnixMilli(),
		T2:   t2,
		OWD:  t2 - request.T0,
		Data: request.Data,
	}, nil
}

func NewPingRequest() *babelforcev1.PingRequest {
	return &babelforcev1.PingRequest{T0: time.Now().UnixMilli()}
}

func Ping(ctx context.Context, shc rtvbp.SHC, lastRTT int64) (int, error) {
	ctx, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()

	request := NewPingRequest()
	request.RTT = lastRTT
	response, err := babelforcev1.NewVoicePeer(shc).Ping(ctx, request)
	if err != nil {
		return 0, err
	}
	receivedAt := time.Now().UnixMilli()
	rtt := time.Duration(receivedAt-request.T0) * time.Millisecond
	shc.Log().Debug(
		"ping response",
		slog.Duration("owd", time.Duration(response.OWD)*time.Millisecond),
		slog.Duration("rtt", rtt),
	)
	return int(rtt.Milliseconds()), nil
}

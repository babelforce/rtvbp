package babelforcev1

import (
	"context"
	"log/slog"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
)

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

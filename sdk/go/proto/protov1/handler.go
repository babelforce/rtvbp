package protov1

import (
	"context"
	"github.com/babelforce/rtvbp-go"
	"github.com/babelforce/rtvbp-go/proto"
	"log/slog"
	"time"
)

type HandlerConfig struct {
	Metadata     map[string]any
	Audio        *AudioConfig
	PingInterval time.Duration
}

// NewClientHandler creates the handler which runs as client
func NewClientHandler(
	tel TelephonyAdapter,
	config *HandlerConfig,
	onAudio func(ctx context.Context, h rtvbp.SHC) error,
) rtvbp.SessionHandler {

	return rtvbp.NewHandler(
		rtvbp.HandlerConfig{

			OnBegin: func(ctx context.Context, h rtvbp.SHC) error {
				if err := onAudio(ctx, h); err != nil {
					return err
				}

				// notify session.update
				_ = h.Notify(ctx, &SessionUpdatedEvent{
					Audio:    config.Audio,
					Metadata: config.Metadata,
				})

				// periodic application level ping
				go ping(ctx, config.PingInterval, h)

				return nil
			},
		},
		// REQ: ping
		NewPingHandler(),
		// REQ: call.hangup
		rtvbp.HandleRequest(func(ctx context.Context, hc rtvbp.SHC, req *CallHangupRequest) (*CallHangupResponse, error) {
			err := tel.Hangup(ctx)
			if err != nil {
				return nil, err
			}
			return &CallHangupResponse{}, nil
		}),
		// REQ: application.move
		rtvbp.HandleRequest(
			func(ctx context.Context, hc rtvbp.SHC, req *ApplicationMoveRequest) (*ApplicationMoveResponse, error) {
				return tel.Move(ctx, req)
			},
		),
		// REQ: session.terminate
		rtvbp.HandleRequest(
			func(ctx context.Context, hc rtvbp.SHC, req *SessionTerminateRequest) (*SessionTerminateResponse, error) {
				hc.Log().Info("session terminate request", slog.Any("request", req))

				// Attempt to move the call
				_, err := tel.Move(ctx, &ApplicationMoveRequest{})
				if err != nil {
					return nil, err
				}

				//
				return &SessionTerminateResponse{}, nil
			},
		),
	)
}

func ping(ctx context.Context, pingInterval time.Duration, h rtvbp.SHC) {

	if pingInterval == 0 {
		pingInterval = 10 * time.Second
	}
	h.Log().Info("starting ping", slog.Any("interval", pingInterval))

	var seq = 1

	pingTicker := time.NewTicker(pingInterval)
	for {
		select {
		case <-pingTicker.C:
			ping := &PingRequest{
				Sequence:  seq,
				Timestamp: time.Now().UnixMilli(),
			}
			pong, err := h.Request(ctx, ping)
			seq = seq + 1

			if err != nil {
				h.Log().Error("failed to send ping", slog.Any("err", err))
			} else {
				pr, err := proto.As[PingResponse](pong.Result)
				if err != nil {
					h.Log().Error("failed to parse ping response", slog.Any("err", err))
					return
				}
				rtt := ping.Timestamp - pr.Timestamp

				h.Log().Info("ping response", slog.Any("response", pong), slog.Any("rtt", rtt))

			}
		case <-ctx.Done():
			return
		}
	}
}

func terminateAndClose(reason string) func(context.Context, rtvbp.SHC) error {
	return func(ctx context.Context, hc rtvbp.SHC) error {
		terminateCtx, cancel := context.WithTimeout(ctx, 1*time.Second)
		defer cancel()

		// request to terminate the session
		_, err := hc.Request(terminateCtx, &SessionTerminateRequest{
			Reason: reason,
		})
		if err != nil {
			hc.Log().Error("failed to request terminate session", slog.Any("err", err))
		}

		// close
		closeCtx, cancel := context.WithTimeout(ctx, 5*time.Second)
		defer cancel()
		return hc.Close(closeCtx)
	}
}

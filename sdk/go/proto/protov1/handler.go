package protov1

import (
	"context"
	"github.com/babelforce/rtvbp-go"
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
		rtvbp.HandleRequest(func(ctx context.Context, hc rtvbp.SHC, req *PingRequest) (*PingResponse, error) {
			err := tel.Hangup(ctx)
			if err != nil {
				return nil, err
			}
			return &PingResponse{
				Data: req.Data,
			}, nil
		}),
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

func ping(ctx context.Context, pingInterval time.Duration, h rtvbp.SHC) func() {
	if pingInterval == 0 {
		pingInterval = 10 * time.Second
	}
	return func() {
		pingTicker := time.NewTicker(pingInterval)
		select {
		case <-pingTicker.C:
			pong, err := h.Request(ctx, &PingRequest{Data: map[string]any{
				"time": time.Now().Unix(),
			}})
			if err != nil {
				h.Log().Error("failed to send ping", slog.Any("err", err))
			} else {
				h.Log().Debug("ping response", slog.Any("response", pong))
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

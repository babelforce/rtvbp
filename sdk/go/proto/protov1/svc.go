package protov1

import (
	"context"
	"github.com/babelforce/rtvbp-go"
	"io"
	"log/slog"
	"time"
)

type TelephonyAdapter interface {
	Move(ctx context.Context, req *ApplicationMoveRequest) (*ApplicationMoveResponse, error)
	Hangup(ctx context.Context) error
	// Play(prompt, etc)
}

type Config struct {
	Metadata     map[string]any
	Audio        *AudioConfig
	PingInterval time.Duration
}

func Handler(
	tel TelephonyAdapter,
	config *Config,
	onStreamProcess func(ctx context.Context, s io.ReadWriter) error,
) rtvbp.SessionHandler {

	return rtvbp.NewHandler(
		rtvbp.HandlerConfig{

			OnBegin: func(ctx context.Context, h rtvbp.SHC) error {
				// TODO: later do request/response based handshake here
				if err := onStreamProcess(ctx, h.AudioStream()); err != nil {
					h.Log().Error("failed to process audio stream", slog.Any("err", err))
					return err
				}

				// send session.update
				_ = h.Notify(ctx, &SessionUpdatedEvent{
					Audio:    config.Audio,
					Metadata: config.Metadata,
				})

				// periodic application level ping
				go ping(ctx, config.PingInterval, h)

				return nil
			},
		},
		rtvbp.HandleRequest(func(ctx context.Context, hc rtvbp.SHC, req *CallHangupRequest) (*CallHangupResponse, error) {
			err := tel.Hangup(ctx)
			if err != nil {
				return nil, err
			}
			return &CallHangupResponse{}, nil
		}),
		rtvbp.HandleRequest(
			func(ctx context.Context, hc rtvbp.SHC, req *ApplicationMoveRequest) (*ApplicationMoveResponse, error) {
				return tel.Move(ctx, req)
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
		println("TERMINATING", reason, "SESSION", hc.SessionID())
		// request to terminate the session
		_, err := hc.Request(ctx, &SessionTerminateRequest{
			// TODO: TerminationReason
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

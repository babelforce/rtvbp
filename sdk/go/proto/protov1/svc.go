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

func Handler(
	tel TelephonyAdapter,
	audioStream io.ReadWriter,
	audioConfig *AudioConfig,
) rtvbp.SessionHandler {
	return rtvbp.NewHandler(
		rtvbp.HandlerConfig{
			Audio: func() (io.ReadWriter, error) {
				return audioStream, nil
			},
			BeginHandler: func(ctx context.Context, h rtvbp.SHC) error {
				_ = h.Notify(ctx, &SessionUpdatedEvent{
					Audio: audioConfig,
				})

				// emit some event
				/*_ = h.Notify(ctx, &protov1.DummyEvent{
					Text: "hello from client",
				})*/

				// example request
				/*moved, err := h.Request(ctx, &protov1.ApplicationMoveRequest{})
				if err != nil {
					return err
				}
				h.Log().Info("application move response", "response", moved)
				*/

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
				// 1. telephony: application move
				moveResponse, err := tel.Move(ctx, req)
				if err != nil {
					return nil, err
				}
				hc.Log().Debug("application moved", slog.Any("move", moveResponse))

				// TODO: must find solution: this should be queued somehow
				go func() {
					<-time.After(1 * time.Second)
					err := terminateAndClose("application.move")(ctx, hc)
					if err != nil {
						hc.Log().Error("failed to terminate session", slog.Any("err", err))
					}
				}()

				return moveResponse, nil
			},
		),
	)
}

func terminateAndClose(reason string) func(context.Context, rtvbp.SHC) error {
	return func(ctx context.Context, hc rtvbp.SHC) error {
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

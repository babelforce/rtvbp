package protov1

import (
	"context"
	"fmt"
	"log/slog"
	"sync"
	"time"

	"github.com/babelforce/rtvbp-go"
	"github.com/babelforce/rtvbp-go/proto"
)

type EmptyResponse struct {
}

type ClientHandlerConfig struct {
	Call         CallInfo
	App          AppInfo
	Metadata     map[string]any
	PingInterval time.Duration
	SampleRate   int
}

type ClientHandler struct {
	rtvbp.SessionHandler
	mu          sync.Mutex
	initialized bool
}

func (ch *ClientHandler) sessionInitialize(ctx context.Context, h rtvbp.SHC, req *SessionInitializeRequest) (*SessionInitializeResponse, error) {
	ch.mu.Lock()
	defer ch.mu.Unlock()

	if ch.initialized {
		return nil, fmt.Errorf("session already initialized")
	}

	r1, err := h.Request(ctx, req)
	if err != nil {
		return nil, err
	}
	r2, err := proto.As[SessionInitializeResponse](r1.Result)
	if err != nil {
		return nil, err
	}

	if r2 == nil {
		return nil, fmt.Errorf("invalid response")
	}

	// TODO: verify that returned audio codec is supported by the client (is in the list of audio codecs offered)

	ch.initialized = true

	// notify session.update
	_ = h.Notify(ctx, &SessionUpdatedEvent{
		AudioCodec: r2.AudioCodec,
	})

	return r2, nil
}

// OnHangup is called when the call is hung up by the remote party
// Triggers the following sequence
// - EVT call.hangup
// - REQ session.terminate(reason=hangup)
func (ch *ClientHandler) OnHangup(ctx context.Context, s *rtvbp.Session) error {
	_ = s.Notify(ctx, &CallHangupEvent{})
	if _, err := s.Request(ctx, &SessionTerminateRequest{Reason: "hangup"}); err != nil {
		return err
	}
	return nil
}

// NewClientHandler creates the handler which runs as client
func NewClientHandler(
	tel TelephonyAdapter,
	config *ClientHandlerConfig,
	onAudio func(ctx context.Context, h rtvbp.SHC) error,
) *ClientHandler {
	hdl := &ClientHandler{}

	var check rtvbp.RequestMiddlewareFunc = func(ctx context.Context, h rtvbp.SHC, req *proto.Request) error {
		hdl.mu.Lock()
		defer hdl.mu.Unlock()
		if !hdl.initialized {
			return fmt.Errorf("session not initialized")
		}
		return nil
	}

	hdl.SessionHandler = rtvbp.NewHandler(
		rtvbp.HandlerConfig{

			OnBegin: func(ctx context.Context, h rtvbp.SHC) error {
				//
				r, err := hdl.sessionInitialize(ctx, h, &SessionInitializeRequest{
					Metadata:            config.Metadata,
					AudioCodecOfferings: []AudioCodec{newL16Codec(config.SampleRate)},
					CallInfo:            config.Call,
					AppInfo:             config.App,
				})
				if err != nil {
					return err
				}
				h.Log().Info("session initialized", slog.Any("response", r))

				// start audio streaming
				if err := onAudio(ctx, h); err != nil {
					return err
				}

				// periodic application level pinger
				go startPinger(ctx, config.PingInterval, h)

				return nil
			},
		},
		// REQ: ping
		NewPingHandler(),
		// REQ: call.hangup
		rtvbp.Middleware(check, rtvbp.HandleRequest(func(ctx context.Context, hc rtvbp.SHC, req *CallHangupRequest) (*CallHangupResponse, error) {
			return tel.Hangup(ctx, req)
		})),
		// REQ: application.move
		rtvbp.Middleware(check, rtvbp.HandleRequest(
			func(ctx context.Context, hc rtvbp.SHC, req *ApplicationMoveRequest) (*ApplicationMoveResponse, error) {
				return tel.Move(ctx, req)
			},
		)),
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
		// REQ: audio.buffer.clear
		rtvbp.Middleware(check, rtvbp.HandleRequest(
			func(ctx context.Context, hc rtvbp.SHC, req *AudioBufferClearRequest) (*AudioBufferClearResponse, error) {
				n, err := hc.AudioStream().ClearBuffer()
				if err != nil {
					return nil, err
				}
				return &AudioBufferClearResponse{
					Len: n,
				}, nil
			},
		)),
		rtvbp.Middleware(check, rtvbp.HandleRequest(
			func(ctx context.Context, shc rtvbp.SHC, req *SessionSetRequest) (*EmptyResponse, error) {
				err := tel.SessionVariablesSet(ctx, req)
				if err != nil {
					return nil, err
				}

				return &EmptyResponse{}, nil
			},
		)),
		rtvbp.Middleware(check, rtvbp.HandleRequest(
			func(ctx context.Context, shc rtvbp.SHC, req *SessionGetRequest) (map[string]any, error) {
				return tel.SessionVariablesGet(ctx, req)
			},
		)),
		rtvbp.Middleware(check, rtvbp.HandleRequest(
			func(ctx context.Context, shc rtvbp.SHC, req *RecordingStartRequest) (*RecordingStartResponse, error) {
				return tel.RecordingStart(ctx, req)
			},
		)),
		rtvbp.Middleware(check, rtvbp.HandleRequest(
			func(ctx context.Context, shc rtvbp.SHC, req *RecordingStopRequest) (*EmptyResponse, error) {
				err := tel.RecordingStop(ctx, req.ID)
				if err != nil {
					return nil, err
				}
				return &EmptyResponse{}, nil
			},
		)),
	)

	return hdl
}

func terminateAndClose(ctx context.Context, hc rtvbp.SHC, reason string) error {

	terminateCtx, cancel := context.WithTimeout(ctx, 5*time.Second)
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

package protov1

import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"sync"
	"sync/atomic"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
)

type EmptyResponse struct {
}

type ClientHandlerConfig struct {
	Call        CallInfo
	App         AppInfo
	Metadata    map[string]any
	AudioFormat rtvbp.MediaFormat
}

type ClientHandler struct {
	rtvbp.SessionHandler
	mu          sync.Mutex
	initialized bool
	shc         rtvbp.SHC
	dtmfSeq     atomic.Int64
	audioFormat rtvbp.MediaFormat
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
	var r2 *SessionInitializeResponse
	if len(r1.Payload) != 0 {
		err = json.Unmarshal(r1.Payload, &r2)
	}
	if err != nil {
		return nil, err
	}

	if r2 == nil {
		return nil, fmt.Errorf("invalid response")
	}

	selected, err := MediaFormat(r2.AudioCodec, ch.audioFormat.PTime)
	if err != nil {
		return nil, err
	}
	if selected != ch.audioFormat {
		return nil, fmt.Errorf("peer selected unsupported audio format %#v", selected)
	}
	if err := h.AcceptAudio(ctx); err != nil {
		return nil, fmt.Errorf("accept negotiated audio: %w", err)
	}

	ch.initialized = true
	ch.shc = h

	// notify session.update
	_ = h.Notify(ctx, &SessionUpdatedEvent{
		AudioCodec: r2.AudioCodec,
	})

	return r2, nil
}

func (ch *ClientHandler) Observe(ctx context.Context, interval time.Duration) rtvbp.AudioStreamObserver {
	at := &audioInfoTracker{ch: ch}
	at.Start(ctx, interval)
	return at.asObserver()
}

func (ch *ClientHandler) Terminate(reason string) error {
	ch.mu.Lock()
	defer ch.mu.Unlock()

	if !ch.initialized {
		return fmt.Errorf("termination failed: session not initialized")
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	return sessionTerminateAndClose(ctx, ch.shc, reason)
}

// NewClientHandler creates the handler which runs as client
func NewClientHandler(
	tel TelephonyAdapter,
	config *ClientHandlerConfig,
	onAudio func(ctx context.Context, h rtvbp.SHC) error,
) *ClientHandler {
	audioFormat := config.AudioFormat
	if audioFormat == (rtvbp.MediaFormat{}) {
		audioFormat = DefaultMediaFormat()
	}
	hdl := &ClientHandler{audioFormat: audioFormat}

	var check rtvbp.RequestMiddlewareFunc = func(ctx context.Context, h rtvbp.SHC, req rtvbp.Request) error {
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
				if _, err := audioFormat.FrameBytes(); err != nil {
					return fmt.Errorf("invalid configured audio format: %w", err)
				}
				r, err := hdl.sessionInitialize(ctx, h, &SessionInitializeRequest{
					Metadata:            config.Metadata,
					AudioCodecOfferings: []AudioCodec{audioCodec(audioFormat)},
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

				// wire DTMF events
				if err := tel.OnDTMF(func(e *DTMFEvent) {
					e.Seq = int(hdl.dtmfSeq.Add(1) - 1)
					if err := h.Notify(ctx, e); err != nil {
						h.Log().Error("failed to notify DTMF", slog.Any("err", err))
					}
				}); err != nil {
					return fmt.Errorf("failed to setup DTMF: %w", err)
				}

				// wire call.hangup event
				if err := tel.OnHangup(func(e *CallHangupEvent) {
					_ = h.Notify(ctx, e)
					_, err := h.Request(ctx, &SessionTerminateRequest{Reason: "hangup"})
					if err != nil {
						h.Log().Error("failed to terminate session on call.hangup event", slog.Any("err", err))
					}
				}); err != nil {
					return fmt.Errorf("failed to setup hangup event: %w", err)
				}

				return nil
			},
		},
		// REQ: ping
		NewPingHandler(),
		// REQ: call.hangup
		rtvbp.Middleware(check, rtvbp.HandleRequest(func(ctx context.Context, hc rtvbp.SHC, req *CallHangupRequest) (*EmptyResponse, error) {
			err := tel.Hangup(ctx, req)
			if err != nil {
				return nil, err
			}
			return &EmptyResponse{}, nil
		})),
		// REQ: application.move
		rtvbp.Middleware(check, rtvbp.HandleRequest(
			func(ctx context.Context, hc rtvbp.SHC, req *ApplicationMoveRequest) (*ApplicationMoveResponse, error) {
				return tel.Move(ctx, req)
			},
		)),
		// REQ: session.terminate
		rtvbp.HandleWithError[*SessionTerminateRequest](rtvbp.WireError{
			Code:    501,
			Message: "session.terminate is not supported. please use application.move or call.hangup instead",
		}),
		// REQ: audio.buffer.clear
		rtvbp.Middleware(check, rtvbp.HandleRequest(
			func(ctx context.Context, hc rtvbp.SHC, req *AudioBufferClearRequest) (*AudioBufferClearResponse, error) {
				n, err := hc.AudioStream().ClearReadBuffer()
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

func sessionTerminateAndClose(ctx context.Context, hc rtvbp.SHC, reason string) error {
	terminateCtx, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()
	_, err := hc.Request(terminateCtx, &SessionTerminateRequest{Reason: reason})
	if err != nil {
		return err
	}
	return hc.Close(ctx)
}

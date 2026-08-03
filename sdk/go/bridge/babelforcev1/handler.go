package babelforcev1

import (
	"context"
	"fmt"
	"log/slog"
	"sync"
	"sync/atomic"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
)

type HandlerConfig struct {
	Call        babelforcev1.CallInfo
	Application babelforcev1.AppInfo
	Metadata    map[string]any
	AudioFormat rtvbp.MediaFormat
}

type VoiceHandler struct {
	rtvbp.SessionHandler

	telephony   TelephonyAdapter
	config      HandlerConfig
	onAudio     func(context.Context, rtvbp.SHC) error
	audioFormat rtvbp.MediaFormat

	mu          sync.Mutex
	initialized bool
	shc         rtvbp.SHC
	dtmfSeq     atomic.Int64
}

func NewVoiceHandler(
	telephony TelephonyAdapter,
	config HandlerConfig,
	onAudio func(context.Context, rtvbp.SHC) error,
) *VoiceHandler {
	audioFormat := config.AudioFormat
	if audioFormat == (rtvbp.MediaFormat{}) {
		audioFormat = DefaultMediaFormat()
	}
	if onAudio == nil {
		onAudio = func(context.Context, rtvbp.SHC) error { return nil }
	}
	handler := &VoiceHandler{
		telephony:   telephony,
		config:      config,
		onAudio:     onAudio,
		audioFormat: audioFormat,
	}

	checkInitialized := rtvbp.RequestMiddlewareFunc(func(
		_ context.Context,
		_ rtvbp.SHC,
		_ rtvbp.Request,
	) error {
		handler.mu.Lock()
		defer handler.mu.Unlock()
		if !handler.initialized {
			return fmt.Errorf("session not initialized")
		}
		return nil
	})

	registrations := babelforcev1.VoiceHandlers(handler)
	for index, registration := range registrations {
		if requestHandler, ok := registration.(rtvbp.RequestHandler); ok {
			registrations[index] = rtvbp.Middleware(checkInitialized, requestHandler)
		}
	}
	handler.SessionHandler = rtvbp.NewHandler(
		rtvbp.HandlerConfig{OnBegin: handler.begin},
		registrations...,
	)
	return handler
}

func (handler *VoiceHandler) begin(ctx context.Context, shc rtvbp.SHC) error {
	if handler.telephony == nil {
		return fmt.Errorf("telephony adapter is required")
	}
	if _, err := handler.audioFormat.FrameBytes(); err != nil {
		return fmt.Errorf("invalid configured audio format: %w", err)
	}
	response, err := handler.initialize(ctx, shc)
	if err != nil {
		return err
	}
	shc.Log().Info("session initialized", slog.Any("response", response))

	if err := handler.onAudio(ctx, shc); err != nil {
		return err
	}

	events := babelforcev1.NewVoiceEvents(shc)
	peer := babelforcev1.NewApplicationPeer(shc)
	if err := handler.telephony.OnDTMF(func(event *babelforcev1.DtmfEvent) {
		event.Seq = int(handler.dtmfSeq.Add(1) - 1)
		if err := events.Dtmf(ctx, event); err != nil {
			shc.Log().Error("failed to notify DTMF", slog.Any("err", err))
		}
	}); err != nil {
		return fmt.Errorf("failed to setup DTMF: %w", err)
	}
	if err := handler.telephony.OnHangup(func(event *babelforcev1.CallHangupEvent) {
		if err := events.CallHangup(ctx, event); err != nil {
			shc.Log().Error("failed to notify call hangup", slog.Any("err", err))
		}
		if _, err := peer.SessionTerminate(ctx, &babelforcev1.SessionTerminateRequest{Reason: "hangup"}); err != nil {
			shc.Log().Error("failed to terminate session on call hangup", slog.Any("err", err))
		}
	}); err != nil {
		return fmt.Errorf("failed to setup hangup event: %w", err)
	}
	return nil
}

func (handler *VoiceHandler) initialize(
	ctx context.Context,
	shc rtvbp.SHC,
) (*babelforcev1.SessionInitializeResponse, error) {
	handler.mu.Lock()
	defer handler.mu.Unlock()
	if handler.initialized {
		return nil, fmt.Errorf("session already initialized")
	}

	metadata := handler.config.Metadata
	response, err := babelforcev1.NewApplicationPeer(shc).SessionInitialize(
		ctx,
		&babelforcev1.SessionInitializeRequest{
			Application:         handler.config.Application,
			Call:                handler.config.Call,
			AudioCodecOfferings: []babelforcev1.AudioCodec{audioCodec(handler.audioFormat)},
			Metadata:            &metadata,
		},
	)
	if err != nil {
		return nil, err
	}
	selected, err := MediaFormat(response.AudioCodec, handler.audioFormat.PTime)
	if err != nil {
		return nil, err
	}
	if selected != handler.audioFormat {
		return nil, fmt.Errorf("peer selected unsupported audio format %#v", selected)
	}
	if err := shc.AcceptAudio(ctx); err != nil {
		return nil, fmt.Errorf("accept negotiated audio: %w", err)
	}

	handler.initialized = true
	handler.shc = shc
	if err := babelforcev1.NewVoiceEvents(shc).SessionUpdated(
		ctx,
		&babelforcev1.SessionUpdatedEvent{AudioCodec: response.AudioCodec},
	); err != nil {
		return nil, fmt.Errorf("notify session updated: %w", err)
	}
	return response, nil
}

func (handler *VoiceHandler) Observe(ctx context.Context, interval time.Duration) rtvbp.AudioStreamObserver {
	tracker := &audioInfoTracker{handler: handler}
	tracker.start(ctx, interval)
	return tracker.observer()
}

func (handler *VoiceHandler) Terminate(reason string) error {
	handler.mu.Lock()
	shc := handler.shc
	initialized := handler.initialized
	handler.mu.Unlock()
	if !initialized {
		return fmt.Errorf("termination failed: session not initialized")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	_, err := babelforcev1.NewApplicationPeer(shc).SessionTerminate(
		ctx,
		&babelforcev1.SessionTerminateRequest{Reason: reason},
	)
	return err
}

func (*VoiceHandler) Ping(
	ctx context.Context,
	_ rtvbp.SHC,
	request *babelforcev1.PingRequest,
) (*babelforcev1.PingResponse, error) {
	return pingResponse(ctx, request)
}

func (handler *VoiceHandler) ApplicationMove(
	ctx context.Context,
	_ rtvbp.SHC,
	request *babelforcev1.ApplicationMoveRequest,
) (*babelforcev1.ApplicationMoveResponse, error) {
	return handler.telephony.Move(ctx, request)
}

func (handler *VoiceHandler) AudioBufferClear(
	_ context.Context,
	shc rtvbp.SHC,
	_ *babelforcev1.AudioBufferClearRequest,
) (*babelforcev1.AudioBufferClearResponse, error) {
	count, err := shc.AudioStream().ClearReadBuffer()
	if err != nil {
		return nil, err
	}
	return &babelforcev1.AudioBufferClearResponse{Len: count}, nil
}

func (handler *VoiceHandler) CallHangup(
	ctx context.Context,
	_ rtvbp.SHC,
	request *babelforcev1.CallHangupRequest,
) (*babelforcev1.EmptyResponse, error) {
	if err := handler.telephony.Hangup(ctx, request); err != nil {
		return nil, err
	}
	return &babelforcev1.EmptyResponse{}, nil
}

func (handler *VoiceHandler) RecordingStart(
	ctx context.Context,
	_ rtvbp.SHC,
	request *babelforcev1.RecordingStartRequest,
) (*babelforcev1.RecordingStartResponse, error) {
	return handler.telephony.RecordingStart(ctx, request)
}

func (handler *VoiceHandler) RecordingStop(
	ctx context.Context,
	_ rtvbp.SHC,
	request *babelforcev1.RecordingStopRequest,
) (*babelforcev1.EmptyResponse, error) {
	if err := handler.telephony.RecordingStop(ctx, request.ID); err != nil {
		return nil, err
	}
	return &babelforcev1.EmptyResponse{}, nil
}

func (handler *VoiceHandler) SessionGet(
	ctx context.Context,
	_ rtvbp.SHC,
	request *babelforcev1.SessionGetRequest,
) (*babelforcev1.SessionGetResponse, error) {
	values, err := handler.telephony.SessionVariablesGet(ctx, request)
	if err != nil {
		return nil, err
	}
	response := babelforcev1.SessionGetResponse(values)
	return &response, nil
}

func (handler *VoiceHandler) SessionSet(
	ctx context.Context,
	_ rtvbp.SHC,
	request *babelforcev1.SessionSetRequest,
) (*babelforcev1.EmptyResponse, error) {
	if err := handler.telephony.SessionVariablesSet(ctx, request); err != nil {
		return nil, err
	}
	return &babelforcev1.EmptyResponse{}, nil
}

var _ babelforcev1.VoiceHandler = (*VoiceHandler)(nil)

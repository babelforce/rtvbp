package babelforcev1

import (
	"context"
	"log/slog"
	"sync/atomic"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
)

type audioInfoTracker struct {
	handler *VoiceHandler
	w       atomic.Int64
	wt      atomic.Int64
	r       atomic.Int64
	rt      atomic.Int64
}

func (tracker *audioInfoTracker) dispatch(ctx context.Context, interval time.Duration) {
	written := tracker.w.Swap(0)
	tracker.wt.Add(written)
	read := tracker.r.Swap(0)
	tracker.rt.Add(read)

	tracker.handler.mu.Lock()
	shc := tracker.handler.shc
	tracker.handler.mu.Unlock()
	if shc == nil {
		return
	}
	event := &babelforcev1.AudioInfoEvent{
		Read: babelforcev1.AudioInfoItem{
			Bytes:          read,
			BytesPerSecond: float64(read) / interval.Seconds(),
			BytesTotal:     tracker.rt.Load(),
		},
		Write: babelforcev1.AudioInfoItem{
			Bytes:          written,
			BytesPerSecond: float64(written) / interval.Seconds(),
			BytesTotal:     tracker.wt.Load(),
		},
	}
	shc.Log().Info("audio info", slog.Any("event", event))
	if written == 0 {
		shc.Log().Warn("no audio data written")
	}
	if read == 0 {
		shc.Log().Warn("no audio data read")
	}
	if err := babelforcev1.NewVoiceEvents(shc).AudioInfo(ctx, event); err != nil {
		shc.Log().Error("failed to notify audio info", slog.Any("err", err))
	}
}

func (tracker *audioInfoTracker) start(ctx context.Context, interval time.Duration) {
	go func() {
		timer := time.NewTicker(interval)
		defer timer.Stop()
		for {
			select {
			case <-ctx.Done():
				return
			case <-timer.C:
				tracker.dispatch(ctx, interval)
			}
		}
	}()
}

func (tracker *audioInfoTracker) observer() rtvbp.AudioStreamObserver {
	return rtvbp.AudioStreamObserver{
		OnRead:  func(count int) { tracker.r.Add(int64(count)) },
		OnWrite: func(count int) { tracker.w.Add(int64(count)) },
	}
}

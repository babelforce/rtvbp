package protov1

import (
	"context"
	"log/slog"
	"sync/atomic"
	"time"

	"github.com/babelforce/rtvbp-go"
)

type audioInfoItem struct {
	Bytes          int64   `json:"bytes"`            // Bytes is the number of bytes read or written within the last interval
	BytesPerSecond float64 `json:"bytes_per_second"` // BytesPerSecond is the number of bytes read or written per second within the last interval
	BytesTotal     int64   `json:"bytes_total"`      // BytesTotal is the total number of bytes read or written since the session started
}

type AudioInfoEvent struct {
	Read  audioInfoItem `json:"read"`  // Read describes how much audio data has been read from the telephony stream
	Write audioInfoItem `json:"write"` // Write describes how much audio data has been written to the telephony stream
}

func (e *AudioInfoEvent) EventName() string {
	return "audio.info"
}

func (e *AudioInfoEvent) String() string {
	return "AudioInfoEvent"
}

type audioInfoTracker struct {
	ch *ClientHandler
	w  atomic.Int64
	wt atomic.Int64
	r  atomic.Int64
	rt atomic.Int64
}

func (t *audioInfoTracker) dispatch(ctx context.Context, interval time.Duration) {
	w := t.w.Swap(0)
	t.wt.Add(w)
	wt := t.wt.Load()
	r := t.r.Swap(0)
	t.rt.Add(r)
	rt := t.rt.Load()

	wr := float64(w) / interval.Seconds()
	rr := float64(r) / interval.Seconds()

	//
	t.ch.mu.Lock()
	defer t.ch.mu.Unlock()
	if t.ch.shc == nil {
		return
	}
	evt := &AudioInfoEvent{
		Read:  audioInfoItem{Bytes: r, BytesPerSecond: rr, BytesTotal: rt},
		Write: audioInfoItem{Bytes: w, BytesPerSecond: wr, BytesTotal: wt},
	}
	t.ch.shc.Log().Info(
		"audio info",
		slog.Any("event", evt),
	)
	if w == 0 {
		t.ch.shc.Log().Warn("no audio data written")
	}
	if r == 0 {
		t.ch.shc.Log().Warn("no audio data read")
	}
	_ = t.ch.shc.Notify(ctx, evt)
}

func (t *audioInfoTracker) Start(ctx context.Context, interval time.Duration) {
	go func() {
		tmr := time.NewTicker(interval)
		defer tmr.Stop()
		for {
			select {
			case <-ctx.Done():
				return
			case <-tmr.C:
				t.dispatch(ctx, interval)
			}
		}
	}()

}

func (t *audioInfoTracker) asObserver() rtvbp.AudioStreamObserver {
	return rtvbp.AudioStreamObserver{
		OnRead: func(n int) {
			t.r.Add(int64(n))
		},
		OnWrite: func(n int) {
			t.w.Add(int64(n))
		},
	}
}

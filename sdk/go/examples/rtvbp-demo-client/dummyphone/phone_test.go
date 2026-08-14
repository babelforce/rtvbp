package dummyphone

import (
	"context"
	"errors"
	"io"
	"log/slog"
	"reflect"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	v1 "github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
)

func testPhone() (*PhoneSystem, context.Context) {
	return New(slog.New(slog.NewTextHandler(io.Discard, nil)))
}

func TestEmulateDTMFBeforeHandlerRegistrationIsSafe(t *testing.T) {
	phone, _ := testPhone()
	phone.EmulateDTMF("5")

	phone, _ = New(nil)
	phone.EmulateDTMF("5")
}

func TestDTMFRegistrationAndSequence(t *testing.T) {
	phone, _ := testPhone()
	if err := phone.OnDTMF(nil); err == nil {
		t.Fatal("accepted a nil DTMF handler")
	}

	var events []*v1.DtmfEvent
	if err := phone.OnDTMF(func(event *v1.DtmfEvent) {
		events = append(events, event)
	}); err != nil {
		t.Fatalf("register DTMF handler: %v", err)
	}
	if err := phone.OnDTMF(func(*v1.DtmfEvent) {}); err == nil {
		t.Fatal("accepted a duplicate DTMF handler")
	}

	phone.EmulateDTMF("5#")
	if len(events) != 2 {
		t.Fatalf("got %d DTMF events, want 2", len(events))
	}
	for index, event := range events {
		if event.Digit != []string{"5", "#"}[index] {
			t.Errorf("event %d digit = %q", index, event.Digit)
		}
		if event.Seq != index {
			t.Errorf("event %d sequence = %d", index, event.Seq)
		}
		if err := event.Validate(); err != nil {
			t.Errorf("event %d is invalid: %v", index, err)
		}
	}
}

func TestSessionVariablesAreStatefulAndCopied(t *testing.T) {
	phone, _ := testPhone()
	ctx := context.Background()
	request := &v1.SessionSetRequest{Data: map[string]any{"answer": 42, "state": "ready"}}
	if err := phone.SessionVariablesSet(ctx, request); err != nil {
		t.Fatalf("set variables: %v", err)
	}
	request.Data["answer"] = 0

	all, err := phone.SessionVariablesGet(ctx, &v1.SessionGetRequest{})
	if err != nil {
		t.Fatalf("get all variables: %v", err)
	}
	if want := map[string]any{"answer": 42, "state": "ready"}; !reflect.DeepEqual(all, want) {
		t.Fatalf("all variables = %#v, want %#v", all, want)
	}
	all["answer"] = -1

	selected, err := phone.SessionVariablesGet(ctx, &v1.SessionGetRequest{Keys: []string{"answer", "missing"}})
	if err != nil {
		t.Fatalf("get selected variables: %v", err)
	}
	if want := map[string]any{"answer": 42}; !reflect.DeepEqual(selected, want) {
		t.Fatalf("selected variables = %#v, want %#v", selected, want)
	}
}

func TestSessionVariablesRejectInvalidAndCanceledCalls(t *testing.T) {
	phone, _ := testPhone()
	if err := phone.SessionVariablesSet(context.Background(), nil); err == nil {
		t.Fatal("set accepted a nil request")
	}
	if _, err := phone.SessionVariablesGet(context.Background(), nil); err == nil {
		t.Fatal("get accepted a nil request")
	}

	canceled, cancel := context.WithCancel(context.Background())
	cancel()
	if err := phone.SessionVariablesSet(canceled, &v1.SessionSetRequest{}); !errors.Is(err, context.Canceled) {
		t.Fatalf("set canceled error = %v", err)
	}
	if _, err := phone.SessionVariablesGet(canceled, &v1.SessionGetRequest{}); !errors.Is(err, context.Canceled) {
		t.Fatalf("get canceled error = %v", err)
	}
}

func TestRecordingLifecycle(t *testing.T) {
	phone, _ := testPhone()
	ctx := context.Background()
	first, err := phone.RecordingStart(ctx, &v1.RecordingStartRequest{Tags: []string{"demo"}})
	if err != nil {
		t.Fatalf("start first recording: %v", err)
	}
	second, err := phone.RecordingStart(ctx, &v1.RecordingStartRequest{})
	if err != nil {
		t.Fatalf("start second recording: %v", err)
	}
	if first.ID == "" || second.ID == "" || first.ID == second.ID {
		t.Fatalf("recording IDs are not unique: %q, %q", first.ID, second.ID)
	}
	if err := phone.RecordingStop(ctx, first.ID); err != nil {
		t.Fatalf("stop first recording: %v", err)
	}
	if err := phone.RecordingStop(ctx, first.ID); err == nil {
		t.Fatal("stopped the same recording twice")
	}
	if err := phone.RecordingStop(ctx, "missing"); err == nil {
		t.Fatal("stopped an unknown recording")
	}
}

func TestRecordingRejectsInvalidAndCanceledCalls(t *testing.T) {
	phone, _ := testPhone()
	if _, err := phone.RecordingStart(context.Background(), nil); err == nil {
		t.Fatal("start accepted a nil request")
	}
	if err := phone.RecordingStop(context.Background(), ""); err == nil {
		t.Fatal("stop accepted an empty ID")
	}

	canceled, cancel := context.WithCancel(context.Background())
	cancel()
	if _, err := phone.RecordingStart(canceled, &v1.RecordingStartRequest{}); !errors.Is(err, context.Canceled) {
		t.Fatalf("start canceled error = %v", err)
	}
	if err := phone.RecordingStop(canceled, "recording-1"); !errors.Is(err, context.Canceled) {
		t.Fatalf("stop canceled error = %v", err)
	}
}

func TestHangupIsExactlyOnceAndCancelsThePhone(t *testing.T) {
	phone, phoneContext := testPhone()
	if err := phone.OnHangup(nil); err == nil {
		t.Fatal("accepted a nil hangup handler")
	}

	var calls atomic.Int32
	if err := phone.OnHangup(func(event *v1.CallHangupEvent) {
		if event.Reason != "test" {
			t.Errorf("hangup reason = %q", event.Reason)
		}
		calls.Add(1)
	}); err != nil {
		t.Fatalf("register hangup handler: %v", err)
	}
	if err := phone.OnHangup(func(*v1.CallHangupEvent) {}); err == nil {
		t.Fatal("accepted a duplicate hangup handler")
	}

	if err := phone.EmulateHangup("test"); err != nil {
		t.Fatalf("first hangup: %v", err)
	}
	if err := phone.EmulateHangup("again"); err == nil {
		t.Fatal("accepted a second hangup")
	}
	if got := calls.Load(); got != 1 {
		t.Fatalf("hangup callback count = %d", got)
	}
	select {
	case <-phoneContext.Done():
	case <-time.After(time.Second):
		t.Fatal("phone context was not canceled")
	}
}

func TestHangupCallbackRunsOutsideStateLock(t *testing.T) {
	phone, _ := testPhone()
	callbackDone := make(chan struct{})
	if err := phone.OnHangup(func(*v1.CallHangupEvent) {
		if _, err := phone.SessionVariablesGet(context.Background(), &v1.SessionGetRequest{}); err != nil {
			t.Errorf("read state from hangup callback: %v", err)
		}
		close(callbackDone)
	}); err != nil {
		t.Fatalf("register hangup handler: %v", err)
	}

	done := make(chan error, 1)
	go func() { done <- phone.EmulateHangup("test") }()
	select {
	case err := <-done:
		if err != nil {
			t.Fatalf("hangup: %v", err)
		}
	case <-time.After(3 * time.Second):
		t.Fatal("hangup callback deadlocked on phone state")
	}
	select {
	case <-callbackDone:
	default:
		t.Fatal("hangup callback did not complete")
	}
}

func TestTelephonyRequestsRejectNilInsteadOfPanicking(t *testing.T) {
	phone, _ := testPhone()
	if err := phone.Hangup(context.Background(), nil); err == nil {
		t.Fatal("hangup accepted a nil request")
	}
	if response, err := phone.Move(context.Background(), nil); err == nil || response != nil {
		t.Fatalf("move result = %#v, %v", response, err)
	}
}

func TestConcurrentStateAccess(t *testing.T) {
	phone, _ := testPhone()
	ctx := context.Background()
	var workers sync.WaitGroup
	for index := range 20 {
		workers.Add(1)
		go func() {
			defer workers.Done()
			_ = phone.SessionVariablesSet(ctx, &v1.SessionSetRequest{Data: map[string]any{"value": index}})
			_, _ = phone.SessionVariablesGet(ctx, &v1.SessionGetRequest{})
			recording, err := phone.RecordingStart(ctx, &v1.RecordingStartRequest{})
			if err == nil {
				_ = phone.RecordingStop(ctx, recording.ID)
			}
		}()
	}
	workers.Wait()
}

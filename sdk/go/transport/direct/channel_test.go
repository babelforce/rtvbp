package direct

import (
	"context"
	"fmt"
	"log/slog"
	"sync"
	"testing"
	"time"

	"github.com/babelforce/rtvbp-go"
	"github.com/babelforce/rtvbp-go/proto/protov1"
	"github.com/stretchr/testify/require"
)

type DummyEvent struct {
	Text string `json:"text,omitempty"`
}

func (e *DummyEvent) EventName() string {
	return "dummy"
}

func (e *DummyEvent) String() string {
	return "DummyEvent"
}

var _ rtvbp.NamedEvent = &DummyEvent{}

func TestDirectTransport(t *testing.T) {

	var (
		t1, t2 = New()
	)

	_ = t1.Write([]byte("hello"))
	_ = t2.Write([]byte("world"))

	require.Equal(t, "hello", string((<-t2.ReadChan()).Data))
	require.Equal(t, "world", string((<-t1.ReadChan()).Data))
}

func TestSessionWithDirectTransport(t *testing.T) {
	slog.SetLogLoggerLevel(slog.LevelDebug)
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	var wg sync.WaitGroup
	wg.Add(2)

	h := rtvbp.NewHandler(
		rtvbp.HandlerConfig{
			OnBegin: func(ctx context.Context, h rtvbp.SHC) error {
				_ = h.Notify(ctx, &DummyEvent{Text: fmt.Sprintf("hello from session: %s", h.SessionID())})
				_, _ = h.Request(ctx, &protov1.ApplicationMoveRequest{})
				return nil
			},
		},
		rtvbp.HandleEvent(func(ctx context.Context, hc rtvbp.SHC, evt *DummyEvent) error {
			wg.Done()
			return nil
		}),
		rtvbp.HandleRequest(func(ctx context.Context, hc rtvbp.SHC, req *protov1.ApplicationMoveRequest) (*protov1.ApplicationMoveResponse, error) {
			return &protov1.ApplicationMoveResponse{}, nil
		}),
	)

	s1, s2 := NewTestSessions(h, h)

	go s1.Run(ctx)
	go s2.Run(ctx)

	wg.Wait()
}

package direct

import (
	"context"
	"fmt"
	"github.com/babelforce/rtvbp-go"
	"github.com/babelforce/rtvbp-go/proto/protov1"
	"github.com/stretchr/testify/require"
	"log/slog"
	"sync"
	"testing"
	"time"
)

func TestDirectTransport(t *testing.T) {

	var (
		t1, t2 = newTransports()
		c1     = t1.Control()
		c2     = t2.Control()
	)

	c1.WriteChan() <- []byte("hello")
	c2.WriteChan() <- []byte("world")

	require.Equal(t, "hello", string(<-c2.ReadChan()))
	require.Equal(t, "world", string(<-c1.ReadChan()))
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
				_ = h.Notify(ctx, &protov1.DummyEvent{Text: fmt.Sprintf("hello from session: %s", h.SessionID())})
				_, _ = h.Request(ctx, &protov1.ApplicationMoveRequest{})
				return nil
			},
		},
		rtvbp.HandleEvent(func(ctx context.Context, hc rtvbp.SHC, evt *protov1.DummyEvent) error {
			wg.Done()
			return nil
		}),
		rtvbp.HandleRequest(func(ctx context.Context, hc rtvbp.SHC, req *protov1.ApplicationMoveRequest) (*protov1.ApplicationMoveResponse, error) {
			return &protov1.ApplicationMoveResponse{}, nil
		}),
	)

	var f = func(t rtvbp.Transport) func(context.Context) (rtvbp.Transport, error) {
		return func(ctx context.Context) (rtvbp.Transport, error) {
			return t, nil
		}
	}

	var (
		t1, t2 = newTransports()
		s1     = rtvbp.NewSession(f(t1), h)
		s2     = rtvbp.NewSession(f(t2), h)
	)

	go s1.Run(ctx)
	go s2.Run(ctx)

	wg.Wait()

}

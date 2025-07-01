package protov1

import (
	"context"
	"github.com/babelforce/rtvbp-go"
	"github.com/babelforce/rtvbp-go/proto"
	"github.com/babelforce/rtvbp-go/transport/ws"
	"github.com/stretchr/testify/require"
	"log/slog"
	"testing"
	"time"
)

func createTestClientHandler(tel TelephonyAdapter) rtvbp.SessionHandler {

	return NewClientHandler(
		tel,
		&HandlerConfig{},
		func(ctx context.Context, h rtvbp.SHC) error {
			return nil
		},
	)
}

type serverHandler struct {
	rtvbp.SessionHandler
}

func createTestServerHandler(
	t *testing.T,
	tel *FakeTelephonyAdapter,
	scenario func(t *testing.T, ctx context.Context, h rtvbp.SHC, tel *FakeTelephonyAdapter),
) (*serverHandler, chan struct{}) {
	done := make(chan struct{}, 1)
	updatedCh := make(chan *SessionUpdatedEvent, 1)
	return &serverHandler{
		SessionHandler: rtvbp.NewHandler(
			rtvbp.HandlerConfig{
				OnBegin: func(ctx context.Context, h rtvbp.SHC) error {
					go func() {
						// wait until session updated event is received
						<-updatedCh

						// run the scenario
						scenario(t, ctx, h, tel)

						// mark scenario as done
						done <- struct{}{}
					}()
					return nil
				},
			},
			rtvbp.HandleRequest(func(ctx context.Context, hc rtvbp.SHC, req *SessionTerminateRequest) (*SessionTerminateResponse, error) {
				return &SessionTerminateResponse{}, nil
			}),
			rtvbp.HandleEvent(func(ctx context.Context, hc rtvbp.SHC, evt *SessionUpdatedEvent) error {
				hc.Log().Info("session updated", slog.Any("event", evt))
				updatedCh <- evt
				return nil
			}),
			NewPingHandler(),
		),
	}, done
}

func testScenario(t *testing.T, scenario func(t *testing.T, ctx context.Context, h rtvbp.SHC, tel *FakeTelephonyAdapter)) {
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	tel := &FakeTelephonyAdapter{}

	// server
	srvHdl, done := createTestServerHandler(t, tel, scenario)
	srv := ws.NewServer(ws.ServerConfig{
		Addr: "127.0.0.1:0",
	}, srvHdl)
	defer func() {
		require.NoError(t, srv.Shutdown(ctx))
	}()
	require.NoError(t, srv.Listen())

	// client
	clientHdl := createTestClientHandler(tel)
	require.NotNil(t, clientHdl)
	require.NotNil(t, tel)
	client := srv.NewClientSession(clientHdl)
	select {
	case <-ctx.Done():
		t.Fatal("timeout", ctx.Err())
	case err := <-client.Run(ctx):
		require.NoError(t, err)
	}

	// wait for scenario to finish
	select {
	case <-ctx.Done():
		t.Fatal("timeout", ctx.Err())
	case <-done:
	}

}

type tc struct {
	name string
	fn   func(t *testing.T, ctx context.Context, h rtvbp.SHC, tel *FakeTelephonyAdapter)
}

func TestHandler(outerT *testing.T) {
	slog.SetLogLoggerLevel(slog.LevelDebug)

	var testCases = []tc{
		{
			name: "ping",
			fn: func(t *testing.T, ctx context.Context, h rtvbp.SHC, tel *FakeTelephonyAdapter) {
				res, err := h.Request(ctx, &PingRequest{Data: "hello"})
				require.NoError(t, err)
				require.NotNil(t, res)
				//require.Equal(t, map[string]interface{}{"data": "hello"}, res.Result)
				_, _ = h.Request(ctx, &SessionTerminateRequest{})
			},
		},
		{
			name: "terminate session",
			fn: func(t *testing.T, ctx context.Context, h rtvbp.SHC, tel *FakeTelephonyAdapter) {
				res, err := h.Request(ctx, &SessionTerminateRequest{})
				require.NoError(t, err)
				require.NotNil(t, res)
			},
		},

		{
			name: "move to app by id",
			fn: func(t *testing.T, ctx context.Context, h rtvbp.SHC, tel *FakeTelephonyAdapter) {
				res, err := h.Request(ctx, &ApplicationMoveRequest{
					Reason:        "something",
					ApplicationID: "1234",
				})
				require.NoError(t, err)
				require.NotNil(t, res)

				res2, err := proto.As[ApplicationMoveResponse](res.Result)
				require.NoError(t, err)
				require.Equal(t, "1234", res2.NextApplicationID)

				// tel state
				require.NotNil(t, tel.moved)
				require.Equal(t, "1234", tel.moved.ApplicationID)
				require.Equal(t, "something", tel.moved.Reason)
			},
		},

		{
			name: "move to next app",
			fn: func(t *testing.T, ctx context.Context, h rtvbp.SHC, tel *FakeTelephonyAdapter) {
				res, err := h.Request(ctx, &ApplicationMoveRequest{
					Reason: "something",
				})
				require.NoError(t, err)
				require.NotNil(t, res)

				res2, err := proto.As[ApplicationMoveResponse](res.Result)
				require.NoError(t, err)
				require.Equal(t, "<id_of_next_node_if_any>", res2.NextApplicationID)

				// tel state
				require.NotNil(t, tel.moved)
				require.Equal(t, "", tel.moved.ApplicationID)
				require.Equal(t, "something", tel.moved.Reason)
			},
		},
		{
			name: "hangup call",
			fn: func(t *testing.T, ctx context.Context, h rtvbp.SHC, tel *FakeTelephonyAdapter) {
				res, err := h.Request(ctx, &CallHangupRequest{})
				require.NoError(t, err)
				require.NotNil(t, res)

				require.True(t, tel.hangup)
			},
		},
	}

	for _, tc := range testCases {
		outerT.Run(tc.name, func(t *testing.T) {
			testScenario(t, tc.fn)
		})
	}
}

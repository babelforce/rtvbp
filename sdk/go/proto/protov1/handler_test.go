package protov1

import (
	"context"
	"github.com/babelforce/rtvbp-go"
	"github.com/babelforce/rtvbp-go/proto"
	"github.com/babelforce/rtvbp-go/transport/ws"
	"github.com/google/uuid"
	"github.com/stretchr/testify/require"
	"log/slog"
	"testing"
	"time"
)

func createTestClientHandler(tel TelephonyAdapter) rtvbp.SessionHandler {
	return NewClientHandler(
		tel,
		&ClientHandlerConfig{
			SampleRate: 8_000,
			Metadata: map[string]any{
				"foobar": 23,
			},
			Call: CallInfo{
				ID:        uuid.NewString(),
				SessionID: uuid.NewString(),
				From:      "493010001000",
				To:        "493010001001",
			},
			App: AppInfo{
				ID: uuid.NewString(),
			},
		},
		func(ctx context.Context, h rtvbp.SHC) error {
			return nil
		},
	)
}

func createTestServerHandler(
	t *testing.T,
	tel *FakeTelephonyAdapter,
	scenario func(t *testing.T, ctx context.Context, h rtvbp.SHC, tel *FakeTelephonyAdapter),
) (rtvbp.SessionHandler, chan struct{}) {
	done := make(chan struct{}, 1)
	updatedCh := make(chan struct{}, 1)
	return rtvbp.NewHandler(
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
		rtvbp.HandleRequest(func(ctx context.Context, hc rtvbp.SHC, req *SessionInitializeRequest) (*SessionInitializeResponse, error) {
			defer func() {
				updatedCh <- struct{}{}
			}()
			return &SessionInitializeResponse{
				AudioCodec: &req.AudioCodecOfferings[0],
			}, nil
		}),
		rtvbp.HandleRequest(func(ctx context.Context, hc rtvbp.SHC, req *SessionTerminateRequest) (*SessionTerminateResponse, error) {
			return &SessionTerminateResponse{}, nil
		}),
		NewPingHandler(),
	), done
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
	t.Cleanup(func() {
		require.NoError(t, srv.Shutdown(context.TODO()))
	})
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
	// name is the name of the test case
	name string

	// fn describes the scenario on the server side (integrator)
	fn func(t *testing.T, ctx context.Context, h rtvbp.SHC, tel *FakeTelephonyAdapter)
}

func TestHandlerUseCasesHappyPath(outerT *testing.T) {
	slog.SetLogLoggerLevel(slog.LevelDebug)

	var testCases = []tc{
		{
			name: "ping",
			fn: func(t *testing.T, ctx context.Context, h rtvbp.SHC, tel *FakeTelephonyAdapter) {
				res, err := h.Request(ctx, &PingRequest{Data: "hello"})
				require.NoError(t, err)
				require.NotNil(t, res)
				_, _ = h.Request(ctx, &SessionTerminateRequest{})
			},
		},
		{
			name: "session.terminate",
			fn: func(t *testing.T, ctx context.Context, h rtvbp.SHC, tel *FakeTelephonyAdapter) {
				res, err := h.Request(ctx, &SessionTerminateRequest{})
				require.NoError(t, err)
				require.NotNil(t, res)
			},
		},
		{
			name: "application.move (by id)",
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
			name: "application.move (next)",
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
			name: "call.hangup",
			fn: func(t *testing.T, ctx context.Context, h rtvbp.SHC, tel *FakeTelephonyAdapter) {
				res, err := h.Request(ctx, &CallHangupRequest{})
				require.NoError(t, err)
				require.NotNil(t, res)
				require.True(t, tel.hangup)
			},
		},
		{
			name: "audio.buffer.clear",
			fn: func(t *testing.T, ctx context.Context, h rtvbp.SHC, tel *FakeTelephonyAdapter) {
				res, err := h.Request(ctx, &AudioBufferClearRequest{})
				require.NoError(t, err)
				require.NotNil(t, res)

				_, _ = h.Request(ctx, &SessionTerminateRequest{})
			},
		},
	}

	for _, tc := range testCases {
		outerT.Run(tc.name, func(t *testing.T) {
			testScenario(t, tc.fn)
		})
	}
}

// TODO: what if no response is retrieved for session.initialize -> MUST fail and terminate session
// TODO: what if requests are send before session.initialize request is handled -> requests must fail

package main

import (
	"context"
	"fmt"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/babelforce/rtvbp/sdk/go/envelope/v1classic"
	"github.com/babelforce/rtvbp/sdk/go/transport/ws"
	"go.uber.org/goleak"
)

type T struct {
}

func (t *T) Error(i ...any) {
	fmt.Printf("ERROR: %v\n", i...)
}

var _ goleak.TestingT = &T{}

func runTest(ctx context.Context, cc ws.ClientConfig) error {
	conn, err := ws.Dial(ctx, cc)
	if err != nil {
		return err
	}

	sess := rtvbp.NewSession(
		v1classic.Envelope{},
		rtvbp.WithTransport(conn),
		rtvbp.WithHandler(rtvbp.NewHandler(
			rtvbp.HandlerConfig{
				OnBegin: func(ctx context.Context, h rtvbp.SHC) error {
					return nil
				},
			},
		)),
	)

	done := sess.Run(ctx)

	go func() {
		<-time.After(5 * time.Second)
		_ = sess.Close(context.Background())
	}()

	<-done

	return nil

}

func main() {
	defer goleak.VerifyNone(&T{})

	srv := ws.NewServer(
		ws.ServerConfig{
			Addr:  "127.0.0.1:0",
			Debug: false,
		},
		rtvbp.NewHandler(
			rtvbp.HandlerConfig{
				OnBegin: func(ctx context.Context, h rtvbp.SHC) error {
					go func() {
						reader := h.AudioStream()
						buf := make([]byte, 1024)
						for {
							n, err := reader.Read(buf)
							if err != nil {
								return
							}

							data := buf[:n]
							_, err = reader.Write(data)
							if err != nil {
								return
							}
						}
					}()
					return nil
				},
			},
		),
	)

	if err := srv.Listen(); err != nil {
		panic(err)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()

	err := runTest(ctx, srv.GetClientConfig())
	if err != nil {
		panic(err)
	}

	_ = srv.Shutdown(context.Background())
}

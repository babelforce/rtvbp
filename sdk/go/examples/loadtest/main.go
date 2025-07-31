package main

import (
	"context"
	"fmt"
	"io"
	"sync"
	"time"

	"github.com/babelforce/rtvbp-go"
	"github.com/babelforce/rtvbp-go/transport/ws"
	"go.uber.org/goleak"
)

type T struct {
}

func (t *T) Error(i ...any) {
	fmt.Printf("ERROR: %v\n", i...)
}

var _ goleak.TestingT = &T{}

type dummyAudio struct {
	n      int
	closed bool
	mu     sync.Mutex
}

func (d *dummyAudio) Close() error {
	d.mu.Lock()
	defer d.mu.Unlock()
	d.closed = true
	return nil
}

func (d *dummyAudio) Read(p []byte) (n int, err error) {
	d.mu.Lock()
	defer d.mu.Unlock()
	if d.closed {
		return 0, io.EOF
	}

	<-time.After(1 * time.Millisecond)
	//println("read...")
	n = len(p)
	for i := 0; i < n; i++ {
		p[i] = 0x00
	}
	d.n += n
	return n, nil
}

func (d *dummyAudio) Write(p []byte) (n int, err error) {
	d.mu.Lock()
	defer d.mu.Unlock()
	if d.closed {
		return 0, io.ErrClosedPipe
	}
	return len(p), nil
}

var _ io.ReadWriteCloser = &dummyAudio{}

func runTest(ctx context.Context, cc ws.ClientConfig) error {
	bb := &dummyAudio{}
	defer bb.Close()

	conn, err := ws.Connect(ctx, cc, bb)
	if err != nil {
		return err
	}

	sess := rtvbp.NewSession(rtvbp.WithTransport(conn), rtvbp.WithHandler(rtvbp.NewHandler(
		rtvbp.HandlerConfig{
			OnBegin: func(ctx context.Context, h rtvbp.SHC) error {
				return nil
			},
		},
	)))

	done := sess.Run(ctx)

	go func() {
		<-time.After(5 * time.Second)
		_ = sess.Close(context.Background(), nil)
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

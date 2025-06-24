package audio

import (
	"errors"
	"github.com/smallnest/ringbuffer"
	"io"
	"log/slog"
)

type nonBlockingRW struct {
	aio  AudioIO
	read *ringbuffer.RingBuffer
}

func (n2 *nonBlockingRW) Read(p []byte) (n int, err error) {
	n, err = n2.read.Read(p)
	if err != nil {
		if errors.Is(err, ringbuffer.ErrIsEmpty) {
			return 0, nil
		}
		return 0, err
	}

	println("read=", len(p))

	return n, nil
}

func (n2 *nonBlockingRW) Write(p []byte) (n int, err error) {
	println("write=", len(p))
	n2.aio.WriteChan() <- p
	println("wrote", len(p))
	return len(p), nil
}

func NewNonBlockingRW(aio AudioIO) io.ReadWriter {
	var (
		bufSize = 1024 * 8
		x       = &nonBlockingRW{
			aio:  aio,
			read: ringbuffer.New(bufSize).SetBlocking(true),
		}
	)

	go func() {
		for {
			select {
			case data := <-aio.ReadChan():
				_, err := x.read.Write(data)
				if err != nil {
					slog.Error(
						"failed to write to ringbuffer",
						slog.Any("err", err),
					)
					return
				}
			}
		}
	}()

	return x
}

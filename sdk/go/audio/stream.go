package audio

import (
	"io"
	"log/slog"
)

func mediaCopy(a io.Reader, b io.Writer) error {
	buf := make([]byte, 1024)
	for {
		n, err := a.Read(buf)
		if err != nil {
			if err == io.EOF {
				return nil
			}
			// TODO: handle reset
			return err
		}
		_, err = b.Write(buf[:n])
		if err != nil {
			return err
		}
	}
}

func DuplexCopy(a io.ReadWriter, b io.ReadWriter) {
	go func() {
		err := mediaCopy(a, b)
		if err != nil {
			slog.Error("failed to copy audio", slog.Any("err", err))
		}
	}()
	go func() {
		err := mediaCopy(b, a)
		if err != nil {
			slog.Error("failed to copy audio", slog.Any("err", err))
		}
	}()
}

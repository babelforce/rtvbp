package audio

import (
	"io"
	"log/slog"
	"time"
)

func Copy(a io.Reader, b io.Writer, size int) error {
	return CopyWithBuf(a, b, make([]byte, size))
}

func CopyWithBuf(a io.Reader, b io.Writer, buf []byte) error {
	for {
		n, err := a.Read(buf)
		if err != nil {
			if err == io.EOF {
				return nil
			}

			if err.Error() == "reset called" {
				<-time.After(10 * time.Millisecond)
				continue
			}
			return err
		}
		_, err = b.Write(buf[:n])
		if err != nil {
			return err
		}
	}
}

func DuplexCopy(a io.ReadWriter, na int, b io.ReadWriter, nb int) {
	go func() {
		err := Copy(a, b, na)
		if err != nil {
			slog.Error("failed to copy audio", slog.Any("err", err))
		}
	}()
	go func() {
		err := Copy(b, a, nb)
		if err != nil {
			slog.Error("failed to copy audio", slog.Any("err", err))
		}
	}()
}

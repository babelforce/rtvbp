package rtvbp

import (
	"github.com/stretchr/testify/require"
	"go.uber.org/goleak"
	"io"
	"testing"
	"time"
)

func TestAudio(t *testing.T) {
	defer goleak.VerifyNone(t)

	a, b := NewAudioChannel(1024)

	n, err := a.Write([]byte("hello"))
	require.NoError(t, err)

	buf := make([]byte, 1024)
	n, err = b.Read(buf)
	require.NoError(t, err)
	require.Equal(t, "hello", string(buf[:n]))

	require.NoError(t, a.Close())

	n, err = b.Read(buf)
	require.Equal(t, io.EOF, err)
	require.Equal(t, 0, n)
}

func TestBlocking(t *testing.T) {
	defer goleak.VerifyNone(t)

	a, b := NewAudioChannel(1024)
	go func() {
		<-time.After(50 * time.Millisecond)
		require.NoError(t, a.Close())
	}()

	buf := make([]byte, 1024)
	_, err := b.Read(buf)
	require.Equal(t, io.EOF, err)
}

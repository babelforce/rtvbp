package rtvbp

import (
	"github.com/babelforce/rtvbp-go/audio"
	"github.com/stretchr/testify/require"
	"testing"
)

func TestAudio_pipe(t *testing.T) {
	p := audio.NewDuplex()
	p.Inner().WriteChan() <- []byte("test")
	data := <-p.Outer().ReadChan()
	require.Equal(t, "test", string(data))

	p.Close()
	p.Outer().WriteChan() <- []byte("test")
}

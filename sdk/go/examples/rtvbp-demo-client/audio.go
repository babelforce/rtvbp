package main

import (
	"context"
	"encoding/binary"
	"fmt"
	"github.com/gordonklaus/portaudio"
	"io"
	"log/slog"
)

const (
	framesPerBuffer = 160
)

// TODO: allow to resample - so we can always configure as 48khz but then expose something else

func pipeLocalAudio(
	ctx context.Context,
	reader io.Reader,
	writer io.Writer,
	sampleRate float64,
) error {

	if err := portaudio.Initialize(); err != nil {
		return err
	}
	defer portaudio.Terminate()

	inputDevice, err := portaudio.DefaultInputDevice()
	if err != nil {
		return err
	}
	slog.Info("input device", "name", inputDevice.Name)

	outputDevice, err := portaudio.DefaultOutputDevice()
	if err != nil {
		return err
	}
	slog.Info("output device", "name", outputDevice.Name)

	params := portaudio.StreamParameters{
		Input: portaudio.StreamDeviceParameters{
			Device:   inputDevice,
			Channels: 1,
			Latency:  inputDevice.DefaultLowInputLatency,
		},
		Output: portaudio.StreamDeviceParameters{
			Device:   outputDevice,
			Channels: 1,
			Latency:  outputDevice.DefaultLowOutputLatency,
		},
		SampleRate:      sampleRate,
		FramesPerBuffer: framesPerBuffer,
	}

	stop := make(chan error, 1)

	callback := func(in, out []int16) {
		// Write input to the stream
		if len(in) > 0 {
			if err := binary.Write(writer, binary.LittleEndian, in); err != nil {
				stop <- fmt.Errorf("audio write error: %w", err)
				return
			}
		}

		// Read output from the stream, but in a non-blocking way
		byteBuf := make([]byte, len(out)*2)
		n, err := reader.Read(byteBuf)
		if err != nil {
			slog.Error("audio read error", slog.Any("err", err))
			stop <- fmt.Errorf("audio read error: %w", err)
			return
		}

		// Convert the bytes read into int16 values
		count := n / 2
		for i := 0; i < count && i < len(out); i++ {
			out[i] = int16(byteBuf[i*2]) | int16(byteBuf[i*2+1])<<8
		}

		// Fill the rest with silence
		for i := count; i < len(out); i++ {
			out[i] = 0
		}
	}

	stream, err := portaudio.OpenStream(params, callback)
	if err != nil {
		return err
	}
	defer stream.Close()
	if err := stream.Start(); err != nil {
		return err
	}
	defer stream.Stop()

	slog.Info("full-duplex audio stream started")

	select {
	case <-ctx.Done():
		return nil
	case err := <-stop:
		return err
	}
}

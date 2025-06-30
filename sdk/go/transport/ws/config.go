package ws

import (
	"log/slog"
)

type TransportConfig struct {
	AudioBufferSize int
	ChunkSize       int
	Logger          *slog.Logger
}

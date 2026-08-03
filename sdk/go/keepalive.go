package rtvbp

import (
	"context"
	"errors"
	"fmt"
	"time"
)

var ErrKeepaliveTimeout = errors.New("rtvbp: keepalive timeout")

type KeepalivePolicy struct {
	Interval  time.Duration
	Timeout   time.Duration
	MaxMisses int
}

func (p KeepalivePolicy) Enabled() bool {
	return p != (KeepalivePolicy{})
}

func (p KeepalivePolicy) Validate() error {
	if !p.Enabled() {
		return nil
	}
	if p.Interval <= 0 {
		return fmt.Errorf("keepalive interval must be positive")
	}
	if p.Timeout <= 0 {
		return fmt.Errorf("keepalive timeout must be positive")
	}
	if p.MaxMisses <= 0 {
		return fmt.Errorf("keepalive max misses must be positive")
	}
	return nil
}

// KeepaliveTransport optionally supplies a transport-native health monitor.
// MonitorKeepalive blocks until the context ends or monitoring fails.
type KeepaliveTransport interface {
	MonitorKeepalive(ctx context.Context, policy KeepalivePolicy) error
}

package webrtcws

import (
	"context"
	"errors"
	"fmt"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/babelforce/rtvbp/sdk/go/transport/ws"
	"github.com/pion/webrtc/v4"
)

// ClientConfig configures a WebRTC-audio RTVBP client. WebSocket continues to carry control and
// perform the HTTP upgrade, authentication, and profile negotiation.
type ClientConfig struct {
	WebSocket      ws.ClientConfig
	PeerConnection webrtc.Configuration
}

// Client configures a session transport factory for the WebRTC-audio binding.
func Client(config ClientConfig) rtvbp.Option {
	return rtvbp.WithTransportFactory(func(ctx context.Context, envelope rtvbp.Envelope) (rtvbp.Transport, error) {
		resolved := config.WebSocket
		if resolved.Subprotocols == nil {
			resolved.Subprotocols = []string{Subprotocol}
		}
		resolved.Defaults()
		base, err := ws.DialDetached(ctx, resolved)
		if err != nil {
			return nil, err
		}
		if base.WireSubprotocol() != Subprotocol {
			_ = base.Close(context.Background())
			return nil, fmt.Errorf("webrtcws: server selected %q, want %q", base.WireSubprotocol(), Subprotocol)
		}
		transport, err := newTransport(base, Config{
			PeerConnection: config.PeerConnection,
			AudioFormat:    resolved.AudioFormat,
		})
		if err != nil {
			_ = base.Close(context.Background())
			return nil, err
		}
		if err := negotiateOffer(ctx, transport.Control(), envelope, transport.peer); err != nil {
			_ = transport.Close(context.Background())
			return nil, err
		}
		return transport, nil
	})
}

// AddToServer enables the WebRTC-audio binding alongside a ServerConfig's existing plain
// WebSocket-audio binding. Clients choose between rtvbp.webrtc.v1 and rtvbp.v1 at upgrade time.
// Headerless legacy clients continue to select plain WebSocket audio.
func AddToServer(base ws.ServerConfig, config Config) ws.ServerConfig {
	base.Subprotocols = addSubprotocol(base.Subprotocols, Subprotocol)
	previous := base.AcceptedTransport
	base.AcceptedTransport = func(ctx context.Context, envelope rtvbp.Envelope, websocketTransport *ws.Transport) (rtvbp.Transport, error) {
		if websocketTransport.WireSubprotocol() != Subprotocol {
			if previous != nil {
				return previous(ctx, envelope, websocketTransport)
			}
			return websocketTransport, nil
		}
		if previous != nil {
			return nil, errors.New("webrtcws: cannot compose another accepted transport decorator with the WebRTC profile")
		}
		transport, err := newTransport(websocketTransport, config)
		if err != nil {
			return nil, err
		}
		if err := negotiateAnswer(ctx, transport.Control(), envelope, transport.peer); err != nil {
			_ = transport.Close(context.Background())
			return nil, err
		}
		return transport, nil
	}
	return base
}

func addSubprotocol(protocols []string, protocol string) []string {
	if protocols == nil {
		return []string{ws.DefaultSubprotocol, protocol}
	}
	for _, candidate := range protocols {
		if candidate == protocol {
			return append([]string(nil), protocols...)
		}
	}
	result := make([]string, 0, len(protocols)+1)
	result = append(result, protocols...)
	result = append(result, protocol)
	return result
}

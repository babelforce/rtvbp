package ws

import (
	"errors"
	"fmt"
	"net/http"

	"github.com/gorilla/websocket"
)

// DefaultSubprotocol is the deployed WebSocket/envelope/catalog profile.
const DefaultSubprotocol = "rtvbp.v1"

var errUnsupportedSubprotocol = errors.New("websocket: unsupported RTVBP subprotocol")

func defaultSubprotocols(protocols []string) []string {
	if protocols == nil {
		return []string{DefaultSubprotocol}
	}
	cloned := make([]string, len(protocols))
	copy(cloned, protocols)
	return cloned
}

func effectiveSubprotocol(selected string) string {
	if selected == "" {
		return DefaultSubprotocol
	}
	return selected
}

func upgradeWebSocket(writer http.ResponseWriter, request *http.Request, supported []string) (*websocket.Conn, error) {
	supported = defaultSubprotocols(supported)
	offered := websocket.Subprotocols(request)
	if len(offered) != 0 && !hasSubprotocolMatch(offered, supported) {
		http.Error(writer, "Unsupported WebSocket subprotocol", http.StatusBadRequest)
		return nil, fmt.Errorf("%w: offered %v, supported %v", errUnsupportedSubprotocol, offered, supported)
	}

	upgrader := websocket.Upgrader{Subprotocols: supported}
	return upgrader.Upgrade(writer, request, nil)
}

func hasSubprotocolMatch(offered, supported []string) bool {
	for _, supportedProtocol := range supported {
		for _, offeredProtocol := range offered {
			if supportedProtocol == offeredProtocol {
				return true
			}
		}
	}
	return false
}

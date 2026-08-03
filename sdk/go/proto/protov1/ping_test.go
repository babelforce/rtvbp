package protov1

import (
	"context"
	"encoding/json"
	"testing"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	"github.com/stretchr/testify/require"
)

func TestPingHandler(t *testing.T) {
	shc := rtvbp.NewTestingSHC()
	h := NewPingHandler()

	params, err := json.Marshal(NewPingRequest())
	require.NoError(t, err)
	receivedAt := time.Now()
	req := rtvbp.Request{ID: "ping-1", Method: "ping", Payload: params, ReceivedAt: receivedAt}

	err = h.Handle(context.Background(), shc, req)
	require.NoError(t, err)
	var response PingResponse
	require.NoError(t, json.Unmarshal(shc.Response.Payload, &response))
	require.Equal(t, receivedAt.UnixMilli(), response.T1)
}

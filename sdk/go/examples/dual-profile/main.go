package main

import (
	"context"
	"fmt"
	"log"
	"net/http"
	"time"

	"github.com/babelforce/rtvbp/sdk/go"
	babelforcev1 "github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
	"github.com/babelforce/rtvbp/sdk/go/catalog/demov1"
	"github.com/babelforce/rtvbp/sdk/go/envelope/v1classic"
	"github.com/babelforce/rtvbp/sdk/go/profile"
	"github.com/babelforce/rtvbp/sdk/go/transport/ws"
	"github.com/gorilla/websocket"
)

const demoProfile = profile.ProfileRtvbpDemoV1

func main() {
	profiles := profileHandlers()
	log.Printf("serving %v on ws://127.0.0.1:8080/ws", profileNames(profiles))
	log.Fatal(http.ListenAndServe("127.0.0.1:8080", http.HandlerFunc(serveProfiles(profiles))))
}

func profileHandlers() map[string]rtvbp.SessionHandler {
	demo := &demoApplication{}
	legacy := &babelforceApplication{}
	return map[string]rtvbp.SessionHandler{
		ws.DefaultSubprotocol: rtvbp.NewHandler(
			rtvbp.HandlerConfig{},
			babelforcev1.ApplicationHandlers(legacy)...,
		),
		demoProfile: rtvbp.NewHandler(
			rtvbp.HandlerConfig{},
			demov1.ApplicationHandlers(demo)...,
		),
	}
}

func serveProfiles(profiles map[string]rtvbp.SessionHandler) func(http.ResponseWriter, *http.Request) {
	return func(writer http.ResponseWriter, request *http.Request) {
		supported := profileNames(profiles)
		offered := websocket.Subprotocols(request)
		if len(offered) != 0 && !profileMatch(offered, supported) {
			http.Error(writer, "Unsupported RTVBP profile", http.StatusBadRequest)
			return
		}
		upgrader := websocket.Upgrader{}
		if len(offered) != 0 {
			upgrader.Subprotocols = supported
		}
		conn, err := upgrader.Upgrade(writer, request, nil)
		if err != nil {
			return
		}
		profile := conn.Subprotocol()
		if profile == "" {
			profile = ws.DefaultSubprotocol
		}
		handler := profiles[profile]
		transport, err := ws.NewTransport(context.Background(), conn, nil)
		if err != nil {
			_ = conn.Close()
			return
		}
		session := rtvbp.NewSession(
			v1classic.Envelope{},
			rtvbp.WithTransport(transport),
			rtvbp.WithHandler(handler),
		)
		<-session.Run(context.Background())
	}
}

func profileNames(profiles map[string]rtvbp.SessionHandler) []string {
	ordered := []string{ws.DefaultSubprotocol, demoProfile}
	names := make([]string, 0, len(profiles))
	for _, profile := range ordered {
		if _, ok := profiles[profile]; ok {
			names = append(names, profile)
		}
	}
	return names
}

func profileMatch(offered, supported []string) bool {
	for _, candidate := range offered {
		for _, profile := range supported {
			if candidate == profile {
				return true
			}
		}
	}
	return false
}

type demoApplication struct{}

func (*demoApplication) DemoEcho(ctx context.Context, shc rtvbp.SHC, request *demov1.DemoEchoRequest) (*demov1.DemoEchoResponse, error) {
	if err := demov1.NewApplicationEvents(shc).DemoObserved(ctx, &demov1.DemoObservedEvent{Message: request.Message}); err != nil {
		return nil, err
	}
	return &demov1.DemoEchoResponse{Message: request.Message}, nil
}

type babelforceApplication struct{}

func (*babelforceApplication) Ping(ctx context.Context, _ rtvbp.SHC, request *babelforcev1.PingRequest) (*babelforcev1.PingResponse, error) {
	inbound, ok := rtvbp.InboundRequest(ctx)
	if !ok {
		return nil, fmt.Errorf("missing inbound request")
	}
	now := time.Now().UnixMilli()
	return &babelforcev1.PingResponse{
		T0: request.T0, T1: inbound.ReceivedAt.UnixMilli(), T2: now,
		OWD: now - request.T0, Data: request.Data,
	}, nil
}

func (*babelforceApplication) SessionInitialize(context.Context, rtvbp.SHC, *babelforcev1.SessionInitializeRequest) (*babelforcev1.SessionInitializeResponse, error) {
	return &babelforcev1.SessionInitializeResponse{}, nil
}

func (*babelforceApplication) SessionTerminate(context.Context, rtvbp.SHC, *babelforcev1.SessionTerminateRequest) (*babelforcev1.EmptyResponse, error) {
	return &babelforcev1.EmptyResponse{}, nil
}

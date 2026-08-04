module rtvbp_demo_client

go 1.24.4

require (
	github.com/babelforce/rtvbp/sdk/go v0.0.0
	github.com/codewandler/audio-go v1.0.1
	github.com/golang-jwt/jwt/v5 v5.3.0
	github.com/google/uuid v1.6.0
	github.com/gordonklaus/portaudio v0.0.0-20250206071425-98a94950218b
	github.com/matoous/go-nanoid/v2 v2.1.0
	github.com/pion/webrtc/v4 v4.2.13
	go.uber.org/goleak v1.3.0
)

require (
	github.com/gorilla/websocket v1.5.3 // indirect
	github.com/pion/datachannel v1.6.0 // indirect
	github.com/pion/dtls/v3 v3.1.2 // indirect
	github.com/pion/ice/v4 v4.2.5 // indirect
	github.com/pion/interceptor v0.1.45 // indirect
	github.com/pion/logging v0.2.4 // indirect
	github.com/pion/mdns/v2 v2.1.0 // indirect
	github.com/pion/randutil v0.1.0 // indirect
	github.com/pion/rtcp v1.2.16 // indirect
	github.com/pion/rtp v1.10.2 // indirect
	github.com/pion/sctp v1.10.0 // indirect
	github.com/pion/sdp/v3 v3.0.18 // indirect
	github.com/pion/srtp/v3 v3.0.10 // indirect
	github.com/pion/stun/v3 v3.1.2 // indirect
	github.com/pion/transport/v4 v4.0.1 // indirect
	github.com/pion/turn/v5 v5.0.4 // indirect
	github.com/smallnest/ringbuffer v0.0.0-20250317021400-0da97b586904 // indirect
	github.com/wlynxg/anet v0.0.5 // indirect
	golang.org/x/crypto v0.48.0 // indirect
	golang.org/x/net v0.50.0 // indirect
	golang.org/x/sys v0.41.0 // indirect
	golang.org/x/time v0.14.0 // indirect
)

replace github.com/babelforce/rtvbp/sdk/go v0.0.0 => ../..

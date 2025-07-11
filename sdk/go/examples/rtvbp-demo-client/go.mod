module rtvbp_demo_client

go 1.24.3

require (
	github.com/babelforce/rtvbp-go v0.0.0
	github.com/codewandler/audio-go v1.0.1
	github.com/golang-jwt/jwt/v5 v5.2.2
	github.com/google/uuid v1.6.0
	github.com/gordonklaus/portaudio v0.0.0-20250206071425-98a94950218b
	github.com/matoous/go-nanoid/v2 v2.1.0
)

require (
	github.com/gorilla/websocket v1.5.3 // indirect
	github.com/smallnest/ringbuffer v0.0.0-20250317021400-0da97b586904 // indirect
)

replace github.com/babelforce/rtvbp-go v0.0.0 => ../../../rtvbp-go

module github.com/babelforce/rtvbp/conformance/interop/rtvbp-go-v0.37.2

go 1.24.4

require (
	github.com/babelforce/rtvbp-go v0.37.2
	github.com/babelforce/rtvbp/sdk/go v0.0.0
	go.uber.org/goleak v1.3.0
)

require (
	github.com/gorilla/websocket v1.5.3 // indirect
	github.com/matoous/go-nanoid/v2 v2.1.0 // indirect
	github.com/smallnest/ringbuffer v0.0.0-20250317021400-0da97b586904 // indirect
)

replace github.com/babelforce/rtvbp/sdk/go v0.0.0 => ../../../sdk/go

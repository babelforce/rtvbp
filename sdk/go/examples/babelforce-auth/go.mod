module github.com/babelforce/rtvbp/sdk/go/examples/babelforce-auth

go 1.24.4

require (
	github.com/babelforce/rtvbp/sdk/go v0.0.0
	github.com/golang-jwt/jwt/v5 v5.3.0
)

require (
	github.com/gorilla/websocket v1.5.3 // indirect
	github.com/matoous/go-nanoid/v2 v2.1.0 // indirect
	github.com/smallnest/ringbuffer v0.0.0-20250317021400-0da97b586904 // indirect
)

replace github.com/babelforce/rtvbp/sdk/go v0.0.0 => ../..

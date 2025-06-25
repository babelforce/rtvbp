module rtvbp_demo_client

go 1.24.3

require (
	babelforce.go/ivr/rtvbp/rtvbp-go v0.0.0
	github.com/gordonklaus/portaudio v0.0.0-20250206071425-98a94950218b
	github.com/smallnest/ringbuffer v0.0.0-20250317021400-0da97b586904
)

require (
	github.com/inconshreveable/mousetrap v1.1.0 // indirect
	github.com/spf13/cobra v1.9.1 // indirect
	github.com/spf13/pflag v1.0.6 // indirect
)

//replace babelforce.go/ivr/rtvbp/rtvbp-go v0.0.0 => ../../../rtvbp-go

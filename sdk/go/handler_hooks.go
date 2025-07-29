package rtvbp

import "context"

type (
	MessageOnBeforeSend interface {
		OnBeforeSend()
	}

	OnAfterReplyHook interface {
		OnAfterReply(ctx context.Context, hc SHC) error // OnAfterReply is called when a handler has replied successfully to a request
	}
)

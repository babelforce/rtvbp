package babelforcev1

import (
	"context"

	"github.com/babelforce/rtvbp/sdk/go/catalog/babelforcev1"
)

type TelephonyDtmfHandler func(*babelforcev1.DtmfEvent)
type TelephonyHangupHandler func(*babelforcev1.CallHangupEvent)

type TelephonyAdapter interface {
	Move(context.Context, *babelforcev1.ApplicationMoveRequest) (*babelforcev1.ApplicationMoveResponse, error)
	Hangup(context.Context, *babelforcev1.CallHangupRequest) error
	SessionVariablesSet(context.Context, *babelforcev1.SessionSetRequest) error
	SessionVariablesGet(context.Context, *babelforcev1.SessionGetRequest) (map[string]any, error)
	RecordingStart(context.Context, *babelforcev1.RecordingStartRequest) (*babelforcev1.RecordingStartResponse, error)
	RecordingStop(context.Context, string) error
	OnDTMF(TelephonyDtmfHandler) error
	OnHangup(TelephonyHangupHandler) error
}

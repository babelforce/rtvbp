package main

import (
	"context"
	"github.com/babelforce/rtvbp-go/proto/protov1"
	"log/slog"
)

type dummyPhoneSystem struct {
	done chan struct{}
	log  *slog.Logger
}

func (d *dummyPhoneSystem) Hangup(_ context.Context, req *protov1.CallHangupRequest) (*protov1.CallHangupResponse, error) {
	defer close(d.done)
	d.log.Info("hangup", slog.Any("req", req))
	return &protov1.CallHangupResponse{}, nil
}

func (d *dummyPhoneSystem) Move(_ context.Context, req *protov1.ApplicationMoveRequest) (*protov1.ApplicationMoveResponse, error) {
	defer close(d.done)
	d.log.Info("move", slog.Any("req", req))
	return &protov1.ApplicationMoveResponse{}, nil
}

var _ protov1.TelephonyAdapter = &dummyPhoneSystem{}

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

func (d *dummyPhoneSystem) Hangup(_ context.Context) error {
	d.log.Info("hangup")
	return nil
}

func (d *dummyPhoneSystem) Move(_ context.Context, req *protov1.ApplicationMoveRequest) (*protov1.ApplicationMoveResponse, error) {
	defer close(d.done)
	d.log.Info("move", slog.Any("req", req))
	return &protov1.ApplicationMoveResponse{}, nil
}

var _ protov1.TelephonyAdapter = &dummyPhoneSystem{}

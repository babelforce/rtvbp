package protov1

import (
	"fmt"
)

type RecordingStartRequest struct {
	Tags []string `json:"tags,omitempty"`
}

func (r *RecordingStartRequest) MethodName() string {
	return "recording.start"
}

type RecordingStartResponse struct {
	ID string `json:"id"`
}

func (r *RecordingStartResponse) Validate() error {
	if r.ID == "" {
		return fmt.Errorf("recording.start response ID is required")
	}
	return nil
}

type RecordingStopRequest struct {
	ID string `json:"id"`
}

func (r *RecordingStopRequest) Validate() error {
	if r.ID == "" {
		return fmt.Errorf("recording.stop request ID is required")
	}
	return nil
}

func (r *RecordingStopRequest) MethodName() string {
	return "recording.stop"
}

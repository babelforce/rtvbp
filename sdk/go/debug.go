package rtvbp

import (
	"fmt"
	"strings"
)

func debugFrame(sessionID string, frame ControlFrame, direction string) {
	var output strings.Builder
	fmt.Fprintf(&output, "FRAME(%s|%s)", sessionID, direction)
	if direction == "in" {
		output.WriteString(" <-- ")
	} else {
		output.WriteString(" --> ")
	}
	fmt.Fprintf(&output, "kind=%d id=%q correl=%q method=%q payload=%s", frame.Kind, frame.ID, frame.CorrelID, frame.Method, frame.Payload)
	if frame.Err != nil {
		fmt.Fprintf(&output, " error=%d:%s", frame.Err.Code, frame.Err.Message)
	}
	fmt.Println(output.String())
}

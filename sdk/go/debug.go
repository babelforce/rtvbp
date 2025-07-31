package rtvbp

import (
	"encoding/json"
	"fmt"
	"strings"

	"github.com/babelforce/rtvbp-go/proto"
)

func debugMessage(sessionID string, m proto.Message, direction string) {

	buf := strings.Builder{}
	buf.WriteString(fmt.Sprintf("MSG(%s|%s)", sessionID, direction))
	if direction == "in" {
		buf.WriteString(" <-- ")
	} else {
		buf.WriteString(" --> ")
	}

	data, err := json.MarshalIndent(m, "", "  ")
	if err != nil {
		buf.WriteString(fmt.Sprintf("failed to marshal message: %+v %s", m, err))
		buf.WriteString("\n")
		fmt.Println(buf.String())
		return
	}

	buf.WriteString("\n")
	buf.WriteString(string(data))
	buf.WriteString("\n")
	fmt.Println(buf.String())

}

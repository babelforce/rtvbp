package proto

import (
	"encoding/json"
	"fmt"
	"github.com/stretchr/testify/require"
	"testing"
)

func TestError(t *testing.T) {
	// create a cause
	cause := NewBadRequestError(fmt.Errorf("x is required"))
	require.Error(t, cause)

	// convert to response error
	re := ToResponseError(cause)
	require.Error(t, re, "must be an error")
	require.Equal(t, re, cause, "must be the same cause")
	require.Equal(t, ErrStatusBadRequest, re.Code, "code is correct")

	res := NewRequest("1", "foo", "banana").NotOk(re)
	require.Equal(t, ErrStatusBadRequest, res.Error.Code, "code is correct")
	require.Equal(t, "x is required", res.Error.Message, "message is correct")
	require.Nil(t, res.Result, "no result")
	_, err := json.Marshal(&res)
	require.NoError(t, err, "can json encode")
}

package proto

import (
	"encoding/json"
	"fmt"
	"testing"

	"github.com/stretchr/testify/assert"

	"github.com/stretchr/testify/require"
)

func TestParse(t *testing.T) {
	t.Run("parse empty", func(t *testing.T) {
		var err error
		_, err = ParseValidMessage([]byte(""))
		assert.Error(t, err)

		_, err = ParseValidMessage([]byte("{}"))
		assert.Error(t, err)

		_, err = ParseValidMessage([]byte("[]"))
		assert.Error(t, err)
	})
}

func TestEvent(t *testing.T) {
	t.Run("event", func(t *testing.T) {
		e := NewEvent("test", nil)
		require.NotEmpty(t, e.ID)
		require.NoError(t, e.Validate())
	})

	t.Run("incorrect events", func(t *testing.T) {
		assert.Error(t, NewEvent("", "").Validate(), "empty event is invalid")
		assert.Error(t, newEventWithVersionAndID("x", "123", "banana", nil).Validate(), "invalid version")
	})

	t.Run("correct", func(t *testing.T) {
		assert.NoError(t, NewEvent("banana", nil).Validate())
	})
}

func TestResponse(t *testing.T) {

	t.Run("from request", func(t *testing.T) {
		req := NewRequest("foo", "bar")

		t.Run("ok", func(t *testing.T) {
			res1 := req.Ok("baz")
			require.Equal(t, res1.Response, req.ID)
			require.Equal(t, "baz", res1.Result)
			require.Nil(t, res1.Error)
			data, err := json.Marshal(&res1)
			require.NoError(t, err)
			require.NotNil(t, data)
		})

		t.Run("not ok", func(t *testing.T) {
			res2 := req.NotOk(BadRequest(fmt.Errorf("x is required")))
			require.Equal(t, res2.Response, req.ID)
			require.Equal(t, "x is required", res2.Error.Message)
		})

	})

}

func TestError(t *testing.T) {
	// create a cause
	cause := BadRequest(fmt.Errorf("x is required"))
	require.Error(t, cause)

	// convert to response error
	re := ToResponseError(cause)
	require.Error(t, re, "must be an error")
	require.Equal(t, re, cause, "must be the same cause")
	require.Equal(t, ErrStatusBadRequest, re.Code, "code is correct")

	res := newRequestWithIdAndVersion("1", "1", "foo", "banana").NotOk(re)
	require.Equal(t, ErrStatusBadRequest, res.Error.Code, "code is correct")
	require.Equal(t, "x is required", res.Error.Message, "message is correct")
	require.Nil(t, res.Result, "no result")
	_, err := json.Marshal(&res)
	require.NoError(t, err, "can json encode")
}

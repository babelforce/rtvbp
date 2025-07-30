---
sidebar_position: 3
---

# Client

---

## Requests


### session.initialize

Before any audio data is being sent out or processed the `session.initialize`
request must be handled. The session owning peer offers multiple audio codec
and the handler must respond with a chosen codec.

**Request**

```json
{
  "version": "1",
  "id": "pD37CkLRXs6iMnBMBzwh2",
  "method": "session.initialize",
  "params": {
    "application": {
      "id": "90e4301109094031b61e354553c09efa"
    },
    "audio_codec_offerings": [
      {
        "id": "L16/8000/1",
        "name": "L16",
        "sample_rate": 8000,
        "bit_depth": 16,
        "channels": 1
      }
    ],
    "call": {
      "id": "1b4e147aa667472bacc613f97379d0f4",
      "session_id": "4ee4ae74f35b4cff81262c0a2bd05492",
      "from": "491704184334",
      "to": "493091734928"
    },
    "metadata": {
      "recording_consent": "yeah"
    }
  }
}
```

**Response**

When requesting `session.initialize` from your peer we expect
that you chose one of the offered audio codec.

```json
{
  "response": "pD37CkLRXs6iMnBMBzwh2",
  "result": {
    "audio_codec": {
      "id": "L16/8000/1",
      "name": "L16",
      "sample_rate": 8000,
      "bit_depth": 16,
      "channels": 1
    }
  },
  "version": "1"
}
```

After this response is received by the session initializer audio processing
begins and the [session.updated](#sessionupdated) event is being dispatched



### session.terminate

## Events

### session.updated

```json
{
  "data": {
    "audio_codec": {
      "bit_depth": 16,
      "channels": 1,
      "id": "L16/8000/1",
      "name": "L16",
      "sample_rate": 8000
    },
    "metadata": {
      "application": {
        "id": "1234"
      },
      "call": {
        "from": "+4910002000",
        "id": "1234",
        "to": "+4910002000"
      },
      "recording_consent": true
    }
  },
  "event": "session.updated",
  "id": "atjPj9BSH4xtLkPcXqa2z",
  "version": "1"
}
```

### session.terminated

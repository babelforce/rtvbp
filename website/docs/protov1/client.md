---
sidebar_position: 3
---

# Session Owner

---

## Requests

When implementing a session handler you *MUST* implement request handlers for at least
[session.initialize](#sessioninitialize) and [session.terminate](#sessionterminate) requests. 

### session.initialize

Before any audio data is exchanged, the `session.initialize` request must be handled.  
The **Client** (babelforce Cloud) offers one or more audio codecs, and the **Server** (your application / session handler) must respond with the chosen codec.

#### Request Fields

| Field | Type | Required | Description |
| ----- | ---- | -------- | ----------- |
| version | string | ✔ | Protocol version (always `"1"`) |
| id | string | ✔ | Unique message ID |
| method | string | ✔ | Always `"session.initialize"` |
| params.application.id | string | ✔ | Application identifier |
| params.audio_codec_offerings[] | array | ✔ | List of available audio codecs |
| params.call.id | string | ✔ | Unique call ID |
| params.call.session_id | string | ✔ | Session identifier |
| params.call.from | string | ✔ | Caller number |
| params.call.to | string | ✔ | Callee number |
| params.metadata | object | ✖ | Optional metadata (e.g., recording consent) |

#### Example Request (inbound to session handler)

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
      "from": "493010001000",
      "to": "493091734928"
    },
    "metadata": {
      "recording_consent": "yes"
    }
  }
}
```

**Response Fields**

| Field               | Type   | Required | Description                           |
| ------------------- | ------ | -------- | ------------------------------------- |
| version             | string | ✔        | Protocol version                      |
| response            | string | ✔        | References original request ID        |
| result.audio\_codec | object | ✔        | Chosen codec (must be from offerings) |

**Example Response**

```json
{
  "version": "1",
  "response": "pD37CkLRXs6iMnBMBzwh2",
  "result": {
    "audio_codec": {
      "id": "L16/8000/1",
      "name": "L16",
      "sample_rate": 8000,
      "bit_depth": 16,
      "channels": 1
    }
  }  
}
```

### session.terminate

The session.terminate request signals the end of a session.
It is typically triggered when a call ends or another termination condition occurs.

| Field         | Type   | Required | Description                               |
| ------------- | ------ | -------- | ----------------------------------------- |
| version       | string | ✔        | Protocol version                          |
| id            | string | ✔        | Unique message ID                         |
| method        | string | ✔        | Always `"session.terminate"`              |
| params.reason | string | ✖        | Reason for termination (e.g., `"hangup"`) |

**Example Request**

```json
{
  "version": "1",
  "id": "vurytmMKlxTBXSIr81YKi",
  "method": "session.terminate",
  "params": {
    "reason": "hangup"
  }
}
```

**Example Response**

```json
{
  "version": "1",
  "response": "vurytmMKlxTBXSIr81YKi",
  "result": {}
}
```

When the session is terminated you will see [call.hangup](#callhangup) event.

## Events

### session.updated

This event is dispatched after a successful session.initialize response and indicates that audio processing has begun.

**Event Fields**

| Field             | Type   | Required | Description                          |
| ----------------- | ------ | -------- | ------------------------------------ |
| version           | string | ✔        | Protocol version                     |
| id                | string | ✔        | Unique event ID                      |
| event             | string | ✔        | Always `"session.updated"`           |
| data.audio\_codec | object | ✔        | Chosen audio codec                   |
| data.metadata     | object | ✖        | Metadata associated with the session |


```json
{
  "version": "1",
  "id": "atjPj9BSH4xtLkPcXqa2z",
  "event": "session.updated",
  "data": {
    "audio_codec": {
      "bit_depth": 16,
      "channels": 1,
      "id": "L16/8000/1",
      "name": "L16",
      "sample_rate": 8000
    },
    "metadata": {
      "application": { "id": "1234" },
      "call": {
        "id": "1234",
        "from": "+4910002000",
        "to": "+4910002000"
      },
      "recording_consent": true
    }
  }
}

```

### call.hangup

`TODO`

**Example Event**

```json
{
  "version": "1",
  "id": "5O3GOxoWMggtNNpYgAlW2",
  "event": "call.hangup",
  "data": {}
}
```

### dtmf

When the session owner’s telephony system receives a DTMF input from the remote peer, the dtmf event is dispatched.

**Event Fields**

| Field             | Type   | Required | Description                               |
| ----------------- | ------ | -------- | ----------------------------------------- |
| version           | string | ✔        | Protocol version                          |
| id                | string | ✔        | Unique event ID                           |
| event             | string | ✔        | Always `"dtmf"`                           |
| data.digit        | string | ✔        | Pressed digit (`"0"-"9"`, `"*"` or `"#"`) |
| data.seq          | int    | ✔        | Sequence number               |
| data.pressed\_at  | int64  | ✔        | Epoch timestamp (ms) when key was pressed |
| data.released\_at | int64  | ✔        | Epoch timestamp (ms) when key was released |

**Example Event**

```json
{
  "version": "1",
  "id": "atjPj9BSH4xtLkPcXqa2z",
  "event": "dtmf",
  "data": {
    "digit": "1",
    "seq": 0,
    "pressed_at": 1753857115250,
    "released_at": 1753857116250
  }
}
```

---
sidebar_position: 2
---

# Protocol

This section defines the application-layer protocol used for real-time voice streaming between the babelforce Cloud (Client) and your application (Server). It covers the transport layer, authentication method, message structure, and keep-alive mechanisms.

---

## Transport

All communication is established over WebSockets secured with TLS (wss://).
This ensures end-to-end encryption, low-latency bidirectional messaging, and persistent connections ideal for real-time voice data exchange.

Other [Transports](./../outlook.md#transport) might come in the future.

## Authentication

The Client (babelforce Cloud) authenticates to your Server using asymmetric authentication with a JWT bearer token.

The token is included in the initial connection request.

You can validate this token using the public key provided by babelforce.

Tokens are signed using a secure algorithm (e.g., RS256), ensuring integrity and authenticity of the Client.

Public key distribution and rotation procedures will be documented separately.

## Message Types

The application-layer protocol defines structured JSON message types exchanged over the WebSocket connection between the **Client** (babelforce Cloud) and the **Server** (your application). All messages share a common schema structure, but their role and context vary based on type.

The protocol defines three core message types exchanged between **Client** and **Server**:

* **Request** – asks the other peer to perform an action
* **Response** – returns the result of a previously received request
* **Event** – sends state updates or notifications without expecting a reply

All messages are JSON-encoded and include a `version` field at the top for forward compatibility.

### Common Envelope

All messages share a standard envelope structure:

| Field      | Type   | Description                                         |
| ---------- | ------ | --------------------------------------------------- |
| `id`       | string | Unique message ID (used for correlation).           |
| `version`  | string | Protocol version, currently always `"1"`.           |
| `method`   | string | For **Request** messages only.                      |
| `response` | string | For **Response** messages only. References request. |
| `event`    | string | For **Event** messages only.                        |
| `params`   | object | Payload for **Requests**.                           |
| `result`   | object | Result data for **Responses**.                      |
| `data`     | object | Payload for **Events**.                             |



### Request

A **Request** initiates an action. It contains:

* `version`: Protocol version (always `"1"` for now)
* `id`: Unique message identifier
* `method`: The operation to perform
* `params`: Any data the operation needs

A **Request** is sent when one peer wants to trigger an operation or query a capability from the other. It always includes an `id` field (used for correlation) and a `method` that identifies the requested action.

#### Example

```json
{
  "version": "1",
  "id": "abc123",
  "method": "my_cool_operation",
  "params": {
    "foo": "bar",
    "baz": "bing"
  }
}
```

### Response

A **Response** is always paired with a Request. It contains:

* `version`: Protocol version
* `response`: ID of the original request
* `result`: Outcome or return data of the request
* `error`: Given when the operation failed

#### Example: Success

```json
{
  "version": "1",
  "response": "abc123",
  "result": {
    "status": "ok"
  }
}
```

#### Example: Error

```json
{
  "version": "1",
  "response": "abc123",
  "error": {
    "code": 404,
    "message": "No such Thing!",
  }
}
```

### Event

An **Event** is an unsolicited message used to notify the other side of a change or update. It contains:

* `version`: Protocol version
* `id`: Unique event ID
* `event`: Name of the event
* `data`: The event payload

#### Example

```json
{
  "version": "1",
  "id": "evt-001",
  "event": "timer.expired",
  "data": {
    "name": "input-timeout"
  }
}
```

---

Note: Transport level Keep-alive

## Keep-Alive

The `ping` method allows either peer to measure **latency**, **one-way delay**, and **clock alignment** by exchanging timestamps through the application layer.

This is in addition to the low-level WebSocket ping/pong mechanism, and provides fine-grained insights into application and network timing.

---

### Request: `ping`

A **ping request** sends a timestamp (`t0`) and optional metadata to the other peer. It may also carry a previous RTT measurement for context.

#### Fields

| Field         | Type   | Required | Description                                                             |
| ------------- | ------ | -------- | ----------------------------------------------------------------------- |
| `version`     | string | ✔        | Protocol version, always `"1"`                                          |
| `id`          | string | ✔        | Unique message ID                                                       |
| `method`      | string | ✔        | Always `"ping"`                                                         |
| `params.t0`   | int64  | ✔        | Epoch timestamp in **milliseconds** representing when the ping was sent |
| `params.rtt`  | int64  | ✖        | Optional RTT (round-trip time) from a previous ping cycle               |
| `params.data` | any    | ✖        | Optional payload, echoed back in the response                           |

#### Example

```json
{
  "version": "1",
  "id": "abc123",
  "method": "ping",
  "params": {
    "t0": 1753857115250,
    "rtt": 12,
    "data": {
      "note": "health-check"
    }
  }
}
```

---

### Response: `ping`

The **ping response** echoes the original `t0` and returns two additional timestamps:

* `t1`: Time the message was received at the transport layer
* `t2`: Time it was handled at the application layer

The **one-way delay (OWD)** is calculated as: `t2 - t0`.

#### Fields

| Field         | Type   | Required | Description                                                |
| ------------- | ------ | -------- | ---------------------------------------------------------- |
| `version`     | string | ✔        | Protocol version                                           |
| `response`    | string | ✔        | References the original `ping` request ID                  |
| `result.t0`   | int64  | ✔        | Echo of the sender's original timestamp (`params.t0`)      |
| `result.t1`   | int64  | ✔        | Time (ms) the message arrived at the transport layer       |
| `result.t2`   | int64  | ✔        | Time (ms) the message was handled at the application layer |
| `result.owd`  | int64  | ✔        | One-way delay (`t2 - t0`) in milliseconds                  |
| `result.data` | any    | ✖        | Echo of the original `params.data`, if provided            |

#### Example

```json
{
  "version": "1",
  "response": "abc123",
  "result": {
    "t0": 1753857115250,
    "t1": 1753857115249,
    "t2": 1753857115250,
    "owd": 0,
    "data": {
      "note": "health-check"
    }
  }
}
```

---

### Timing Diagram

```
Client                                 Server
  |                                      |
  | -- ping { t0 } ------------------->  |
  |                                      | <- t1: transport receive
  |                                      | <- t2: application handle
  | <-- response { t0, t1, t2, owd } --- |
  |                                      |
```

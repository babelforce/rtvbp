


## v1

**session**

- `session_create`: creates a new session
- `events_subscribe`: subscribe to events

**auth**

- bearer token auth, we store it, and share with them
- ip whitelisting on their end

**failures**

- activity detection on our end

**constraints**

- audio is alaw

**functionality**

- we send audio, they send audio
- `move` request must be possible
- `call.hangup` event must be implemented on our side
- `call.hangup` request must be implemented
- `session.update` request (or event?)

## Todo

**Code Generation**

- Make `impl Example for RequestExt` automatically require `Example` for Response type

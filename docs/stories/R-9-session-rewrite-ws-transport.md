---
id: R-9
title: Session rewrite and WebSocket transport port
pillar: SDK
status: backlog
design: docs/designs/go-sdk.md
epic: go-sdk
areas: [sdk-go]
note: blocked on R-7 and R-8; changes runtime semantics but not a single byte on the wire
---

# Session rewrite and WebSocket transport port

## Goal
Rebuild the session on the new seams and fix the runtime semantics that are known-wrong today —
ordering, keepalive, termination, and pending-request leaks — then port the WebSocket transport onto
the new transport interface.

## Acceptance
- [ ] Responses resolve on the reader path and never block; requests and events dispatch through a
      **single serial dispatcher**, so event ordering is guaranteed (a failing-first test asserts a
      `dtmf` burst arrives in order).
- [ ] A handler can issue a nested request without deadlocking (test).
- [ ] Closing a session resolves every pending request with `ErrSessionClosed` instead of leaving it
      to time out (test).
- [ ] `SHC.RespondThenClose` replaces the `OnAfterReply` hooks, which are deleted; a test proves the
      response reaches the peer before the connection closes, for a `terminal` operation.
- [ ] The voice role implements `session.terminate` properly rather than answering 501.
- [ ] One `KeepalivePolicy{Interval, Timeout, MaxMisses}`; a breach surfaces as
      `ErrKeepaliveTimeout`, moves the session to `Failed`, and resolves pending requests. The
      catalog `ping` operation is no longer run automatically.
- [ ] Lifecycle is `Inactive → Connecting → Active → Closing → Closed | Failed`.
- [ ] The session owns the audio ring-buffer pair, exposes `Format()` from the negotiated codec, and
      chunks outbound audio by `Format().PTime` — the hardcoded 320-byte buffer and the dead
      `ChunkSize` option are both gone.
- [ ] The `ws` transport implements the new interface: text frames as the control channel, one
      static duplex `"audio"` media channel over binary frames, flush-on-close, keepalive wiring, and
      subprotocol negotiation (absence means `rtvbp.v1`).
- [ ] `ClearReadBuffer` and the audio observer still work; the load test is `goleak`-clean.

## Progress
- (not started)

## Notes
- Decide the deferred-response API shape here (sentinel error vs. explicit handle) and record it in
  the design doc — it must land together with serial dispatch, not after.
- Reuse the existing gorilla plumbing from the imported tree rather than rewriting the socket layer.
- Nothing in this story may change the wire; R-6/R-7 tests are the guard.

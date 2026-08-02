# RTVBP — vision & principles

This document states *why* RTVBP exists and the principles that decide how it's built. It is the
**tie-breaker** when a design choice is unclear: prefer the option that best serves the north star and
the principles below.

## What RTVBP is

RTVBP (Real-Time Voice Bridge Protocol) connects a **voice-owning peer** (telephony — a live call,
its audio, its call control) to an **application peer** (an AI agent, an IVR, any program that wants
to listen and speak on that call). The two sides exchange typed operations and events plus a
real-time audio stream.

This repository is the **protocol itself**: a machine-readable specification, a generator, and the
SDKs it produces. The defining idea is that the specification is executable — the payload catalog is
authored once as typed source, and every SDK, every document, and every conformance vector is
generated from it. Extending the protocol means editing the spec; nothing else is written by hand.

## North star

**The payload is the invariant; everything around it is a choice.**

A message's shape — its operations, its events, its fields — is the protocol. How that payload is
framed into request/response/event (the **envelope**) and how those frames reach the peer (the
**transport**) are independent, substitutable concerns. Two peers must be able to agree on a
transport and an envelope that suit them — WebSocket today, WebRTC for media with WebSocket for
control, QUIC, or SIP — and still be speaking exactly the same protocol.

## Principles

1. **The spec is the only source of truth.** Operations, events, roles, and envelopes are declared
   once, as typed source. SDK types, dispatch glue, envelope codecs, reference documentation, and
   conformance vectors are all emitted from that declaration. If a thing can be described in the
   spec, it must be described there and generated — never hand-written in N languages. Hand-written
   code is reserved for what genuinely cannot be derived: session plumbing, transports, audio.

2. **Bytes are a contract.** `babelforce.v1` is frozen and must serialize byte-identically to the
   deployed implementation, quirks included. Compatibility is proven mechanically by golden fixtures
   and cross-version interop tests, not by reading code. A wire change is a new catalog, never an
   edit to an existing one.

3. **Any transport × any envelope.** Layers do not leak: the session runtime never sees envelope
   bytes, and a transport never sees a method name. Any (transport, envelope) pair that can carry
   request/response and fire-and-forget events plus a media stream is a valid binding. New bindings
   must not require touching the catalog or the session.

4. **Roles are part of the protocol.** The two peers are symmetric on the network and asymmetric in
   meaning: each offers different operations and emits different events. That asymmetry is declared
   in the spec and shows up as distinct generated interfaces, so an integrator sees exactly what
   their side must implement and what it may call.

5. **Drift cannot merge.** Generated output is committed and CI regenerates it; a diff fails the
   build. Documentation and test vectors are outputs of the same pipeline as the code, so they
   cannot describe a protocol the SDKs don't implement.

## Non-goals

- **Not a general-purpose RPC framework.** RTVBP is scoped to real-time voice sessions; the
  operations are telephony and media operations, not arbitrary services.
- **Not an off-the-shelf codegen consumer.** We do not author OpenAPI/AsyncAPI and run a third-party
  generator. Those may be *emitted* as artifacts, but the generator is ours, so the output is
  idiomatic and the semantics we care about (roles, presence, byte-identity) are first-class.
- **Not a media processing stack.** Codec negotiation and framing are in scope; transcoding,
  recording storage, and speech models are the peers' business.
- **Not backwards-breaking by accident.** We do not "improve" `babelforce.v1` on the wire. Better
  ideas become `babelforce.v2` or a new envelope, selected by negotiation.

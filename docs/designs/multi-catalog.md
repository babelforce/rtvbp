# Design: multi-catalog operation and profile negotiation

**Status:** accepted · **Pillar:** Spec · **Stories:** R-14

## Why

The framework claims to be catalog-agnostic and to support any transport × any envelope.
A claim with exactly one catalog, one envelope, and one transport in the tree is untested — the
abstractions will have quietly grown `babelforce.v1` assumptions, and we will only discover them
when a second catalog is expensive to add.

This epic proves the claim cheaply, without touching the frozen catalog.

## Approach

### Profiles

A **profile** is the triple *(transport, envelope, catalog)*, each independently versioned:

| Component | Id | Today |
|---|---|---|
| Transport | `ws.v1` | WebSocket, text = control, binary = audio |
| Envelope | `classic.v1` | flat JSON, `version:"1"`, discrimination `event → method → response` |
| Catalog | `babelforce.v1` | the frozen payload set |

The legacy profile `rtvbp.v1` names exactly that triple.

### Negotiation

Out-of-band, at connection establishment — the v1 wire is not touched. For WebSocket, the client
offers subprotocols in preference order (`Sec-WebSocket-Protocol: rtvbp.v1`), the server selects one
and echoes it. **Absence of a subprotocol means `rtvbp.v1`**, which is what makes this backward
compatible: current `rtvbp-go` clients send none, and the Rust server already optionally echoes
`rtvbp.v1`.

The per-message `version:"1"` field stays as it is — it is a `classic.v1` envelope constant, not a
negotiation surface. Future envelopes may carry a real handshake; we do not retrofit one into v1.

### The demo catalog

`rtvbp-spec-demo-v1`: a deliberately tiny catalog — one operation, one event — that exists only to be
*different*. The generator emits it exactly as it emits `babelforce.v1` (types, role glue, docs,
vectors), and a Go example serves **both profiles on a single WebSocket endpoint**, dispatching by
negotiated subprotocol.

That example is the proof: two catalogs, one server, one runtime, no special cases. If adding
`demo.v1` requires touching the session, the envelope codec, or the transport, the abstraction is
wrong and this story has found the bug — which is its purpose.

## Alternatives considered

- **Fork `babelforce.v2` as the second catalog.** Real, but expensive and premature: we would be
  designing protocol changes to test a generator. A throwaway catalog isolates the question.
- **Prove catalog-agnosticism only by inspection.** That is precisely the reasoning that let three
  hand-written ports drift apart.
- **Defer to the Rust SDK milestone.** Then the first real test of catalog-agnosticism arrives with
  the first test of language-agnosticism, and a failure is ambiguous.

## Risks & open questions

- Subprotocol names for non-default profiles need a convention (e.g.
  `rtvbp.<catalog>.<envelope>`) — settle it while writing the profiles page in
  [docs-gen](docs-gen.md).
- A server accepting multiple catalogs needs a routing seam between "negotiated profile" and "handler
  set"; keep it in the example rather than the runtime until a second real catalog exists.

## Acceptance / done

`demo.v1` is emitted by the same generator with no catalog-specific code paths; one Go example serves
both `babelforce.v1` and `demo.v1` over a single endpoint selected by subprotocol; a client of each
completes an exchange; the negotiation rules (including the "absence means `rtvbp.v1`" default) are
documented on the generated profiles page.

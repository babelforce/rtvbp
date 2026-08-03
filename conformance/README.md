# Conformance

`babelforce.v1/golden/` is frozen producer authority. The sibling `payloads/`, `envelope/`, and
`scenarios/` trees are generated from the typed spec and must be changed only through
`rtvbp-spec-gen`.

Until R-16 consolidates the repository gate behind `task check`, run the published-version proofs
explicitly:

```sh
(cd tools/capture-rtvbp-go-v0.37.2 && go test ./...)
(cd interop/rtvbp-go-v0.37.2 && go test ./...)
```

The first command proves every common frozen payload and envelope shape against the published
`github.com/babelforce/rtvbp-go v0.37.2`. The second completes a live WebSocket session in both role
directions against that same unmodified module version, including negotiated audio, DTMF, legacy
application ping, termination, and the headerless `rtvbp.v1` compatibility path.

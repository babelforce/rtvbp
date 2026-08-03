# Frozen `babelforce.v1` golden wire fixtures

These 48 compact JSON files are the byte authority for `babelforce.v1`. They have no trailing
newline, and every source-specific capture has an exact inventory and byte-reproduction test.

## Authority and provenance

Forty-two fixtures come from the deployed Go implementation. They are captured by
[`capture-rtvbp-go-v0.40.0`](../../tools/capture-rtvbp-go-v0.40.0/) from
`github.com/babelforce/rtvbp-go v0.40.0`, commit
`9370abb8d18cf3c89837d4d1c63564f6218e354d`: 20 canonical operation payloads, five canonical event
payloads, ten `classic.v1` envelope frames, and seven presence/float payload variants.

Four additive browser-feedback events do not exist in rtvbp-go. Their six payload shapes are
captured by [`capture-private-source.invalid-v0.33.0`](../../tools/capture-private-source.invalid-v0.33.0/)
from the released Rust `rtvbp` crate in `private-source.invalid v0.33.0`, commit
`408b9bc17e925b41a2e9d4fbf97dc93cdbe60b8c`. That tool also exercises the upstream production
`Event::of` → `Message::Event` → `to_json_string` path with deterministic ids, proving event names,
data placement, field order, and `output.transcript.done.text` presence. At the pin, the sole
production bridge callsite sends `text: None`, yielding the canonical `{}` payload. The public
payload type also permits present non-empty and empty strings; both are pinned as variants. Its
GitLab source is private, so a fresh reproduction requires repository read access.

The original 29 v0.40.0 fixtures remain byte-identical. Adding coverage records previously
unpinned behavior; changing any existing fixture byte changes the frozen wire contract and requires
a new catalog rather than an edit to `babelforce.v1`.

## Pinned wire behavior

- Go struct fields serialize in declaration order. Free-form Go maps serialize with lexically
  sorted keys; `session.get` therefore remains a bare, sorted map.
- Required nullable fields emit `null`; optional fields with `omitempty` disappear. The variants
  include every optional payload field in its absent form.
- `output.transcript.done.text` distinguishes absence from a present empty string. Generated Go
  therefore uses an optional string pointer for this Rust-sourced field; both present spellings and
  the production bridge's absent spelling are byte-pinned.
- A request with parameters pins `params` and its position after `method`. Success responses pin the
  distinction between an absent `result` and a typed-nil `"result":null`.
- Error frames pin codes `-1`, `400`, `500`, and `501`, both with and without the legacy `"any"`
  data key. The 501 frame is the deployed response to a reverse application→voice
  `session.terminate`; the canonical `{}` terminate result is produced by the demo application
  handler for the supported voice→application direction.
- `audio.info` pins both Go's integral `float64` spelling (`12800`, not `12800.0`) and a fractional
  spelling (`106.66666666666667`).
- Go's standard JSON decoder ignores unknown object fields. That tolerance is part of compatibility;
  these fixtures do not authorize generated decoders to reject extensions.

## Reproduction

```sh
cd conformance/tools/capture-rtvbp-go-v0.40.0
go run .
go test ./...

cd ../capture-private-source.invalid-v0.33.0
cargo run --locked
cargo test --locked

cd ../capture-rtvbp-go-v0.37.2
go test ./...
```

The last tool captures the 40 fixtures common to rtvbp-go v0.37.2 and v0.40.0 and compares them to
this directory byte-for-byte. The two excluded Go fixtures are `audio.info`, which was added after
v0.37.2; the six fixture shapes for four Rust-only events are outside both Go releases.

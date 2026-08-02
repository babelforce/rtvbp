# Frozen `babelforce.v1` golden wire fixtures

These files are the byte authority for the deployed `babelforce.v1` protocol. They were captured by
[`capture-rtvbp-go-v0.40.0`](../../tools/capture-rtvbp-go-v0.40.0/) from
`github.com/babelforce/rtvbp-go v0.40.0`, commit
`9370abb8d18cf3c89837d4d1c63564f6218e354d`.

The JSON files are compact `encoding/json.Marshal` output with no trailing newline. Payload fixtures
cover every operation request and result and every event's data. Envelope fixtures pin all four
`classic.v1` frame shapes.

These fixtures are frozen. **Changing any JSON byte means changing the wire contract** and therefore
requires a new catalog rather than an edit to `babelforce.v1`.

To reproduce them without using a local `rtvbp-go` checkout:

```sh
cd conformance/tools/capture-rtvbp-go-v0.40.0
go run .
go test ./...
```

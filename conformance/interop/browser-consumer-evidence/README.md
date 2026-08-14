# Maintained browser-consumer migration evidence

This directory records the browser RTVBP implementation that M2 supersedes. It does not copy the
client, identify its private repository, or define wire behavior. The protocol spec and frozen
fixtures remain the only public authority.

The exact source module and its tests are pinned by SHA-256 content digests in `evidence.json`.
Maintainers with access to the consumer can reproduce the pin with `sha256sum`; no private remote,
repository name, source path, branch, commit coordinate, or authentication scheme is published.

## Behavior inventory

**Protocol evidence:** current profile negotiation; classic text control and binary L16 audio on one
WebSocket; an initialization request; initialize response handling; buffer-clear and hangup request
replies; browser-feedback events; little-endian PCM conversion. These are candidates for
generated/runtime support, but the pinned fixtures decide their wire details.

**Deployment policy:** route selection in the URL; caller-injected browser authentication; deployment
identity defaults; same-origin URL resolution. These belong in caller-supplied adapters and examples,
not the universal RTVBP catalog.

**Application convenience:** transcript/action classification, callback shaping, Float32 conversion,
linear device-rate resampling, and UI-friendly URL helpers. Media utilities may be migrated behind an
explicit browser adapter; transcript presentation stays with the application.

**Defects to remove:** version-only parsing, ordinary `JSON.parse` numeric corruption, no structural
envelope validation, only one correlated request, ignored remote errors, success replies for unknown
methods, no timeout/cancellation/close settlement, connection completion before the socket opens,
possible duplicate close callbacks, and no negotiated-profile check. `evidence.json` makes these
gaps executable in the TypeScript foundation tests.

The capture is intentionally disposable after the consumer has migrated. No file under this
directory is an emitter input or a source of generated SDK declarations.

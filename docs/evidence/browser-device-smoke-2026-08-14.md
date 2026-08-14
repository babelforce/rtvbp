# Browser device smoke — 2026-08-14

This is bounded, non-sensitive release evidence for R-37. It contains no endpoint, SDP, candidate,
device-label, credential, or audio recording.

| Item | Evidence |
| --- | --- |
| Command | `npm run smoke:browser-device` from `sdk/typescript` |
| Browser | Google Chrome 150.0.7871.46, headless native media APIs |
| Host class | Linux x86_64, kernel 6.6.144 |
| Node.js | 22.23.1 |
| Profile | `rtvbp.webrtc.v1` |
| Peer | current Go SDK server from this worktree |
| Bound | test timeout 90 seconds; observed completion 4.27 seconds |
| Result | pass |

The smoke used an actual host microphone and audio-output device, not Chrome's fake-media source.
It proved one PCMU WebRTC audio channel, nonzero inbound and outbound RTP packet counts, a remote
track attached to the browser rendering graph, typed control, orderly session shutdown, and the
microphone track reaching `ended`. The automated run verifies signal flow and resource ownership;
it does not claim a human subjective quality score. No captured media was retained.

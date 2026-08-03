---
id: R-20
title: Bounded two-agent OpenAI Realtime acceptance over RTVBP
pillar: Proof
status: backlog
priority: 18
design: docs/designs/conformance.md
epic: conformance
areas: [conformance, sdk-go]
note: follow-up after the first Go tag; OpenAI implementation remains in rtvbp-openai
---

# Bounded two-agent OpenAI Realtime acceptance over RTVBP

## Goal
Extend real-service acceptance beyond a human phone call: play the migrated endpoint through the
existing RTVBP CLI on a local audio device, then prove two OpenAI Realtime agents can exchange audio
through RTVBP without moving OpenAI-specific implementation into the protocol repository.

## Acceptance
- [ ] The existing `rtvbp-demo-client` connects to the deployed `rtvbp-openai` endpoint and plays
      received audio on the configured local speaker; microphone audio reaches the remote agent.
- [ ] A thin two-agent driver lives in the `rtvbp-openai` source repository and connects a second
      OpenAI Realtime session to the RTVBP voice side. No OpenAI-specific runtime enters `rtvbp`.
- [ ] The automated conversation is capped at 60 seconds and six completed turns, with explicit
      cancellation and session termination on every exit path.
- [ ] The proof observes audio in both directions, at least one speech-started/barge-in clear, and a
      clean terminal response before transport close.
- [ ] The run command and bounded evidence are recorded without committing credentials or audio
      containing personal data.

## Progress
- 2026-08-04: Captured from live-acceptance discussion. The existing Go demo CLI already provides
  PortAudio speaker/microphone playback, DTMF injection, and timed hangup; its nested module tests
  pass and the host exposes working PipeWire/PulseAudio input and output devices.

## Notes
- R-15's human/CLI call remains the first release gate. This follow-up must not delay the Go tag.
- The second agent is an external acceptance fixture, not part of the protocol SDK or generator.

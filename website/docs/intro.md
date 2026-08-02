---
sidebar_position: 1
---

# Introduction

**Realtime-Voice-Bridge-Protocol** (`RTVBP`) is a lightweight, event-driven protocol that streams live voice interactions from the babelforce platform to external service endpoints with minimal delay. Designed for extensibility and integration, RTVBP supports a wide range of use cases that depend on real-time access to audio—such as automated language detection, acoustic anomaly monitoring, dynamic tagging, voice-driven workflows, and AI-powered quality assessment.

Built on top of WebSockets secured with TLS, the protocol provides a robust and low-latency transport mechanism. Its design emphasizes ease of implementation, avoiding dependencies on specialized tooling, brokers, or orchestration layers. Developers can focus on the core logic without managing complex infrastructure for stream handling, serialization, or connection balancing.

`RTVBP` streams two types of WebSocket messages: JSON-encoded text messages for metadata and binary messages for the actual audio stream. This separation ensures efficient handling—especially important for voice pipelines—by sidestepping the processing overhead that comes with encoding audio as text. As a result, audio arrives in raw binary format, simplifying integration with systems that perform real-time processing or storage.

---

## What is RTVBP ?

> `RTVBP` is a lightweight session protocol which allows to integrate with babelforce
telephony services.

## Versions

### v1

#### Requests

**Handler**

The following requests can be send by your session handler
and are handled on the babelforce platform (session owner).

- `ping`
- `application.move`
- `call.hangup`
- `audio.buffer.clear`
- **upcoming:** `session.set`
- **upcoming:** `session.get`
- **upcoming:** `recording.start`
- **upcoming:** `recording.stop`

**Session Owner**

The following requests will be send by the session owner and
must be handled by your session handler.

- `session.initialize`
- `session.terminate`

#### Events

**Session Owner**

- `session.updated`
- `call.hangup`
- `dtmf`

**Audio Formats**

- Currently only `PCM16` is supported at a sample rate of `8khz`
- We will send a continuous stream of audio without any gaps
- Peers can sent partial audio as well (You do not have to transmit silence)

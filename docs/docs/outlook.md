---
sidebar_position: 3
---

# Outlook

In future developments of **RTVBP** we have a few things in mind:

## Transport

- **QUIC Transport protocol** could be a better transport protocol for this use case because it offers lower latency and faster connection establishment compared to traditional TCP-based protocols like WebSockets over TLS. Additionally, QUIC’s built-in multiplexing and improved handling of packet loss make it well-suited for real-time audio streaming with minimal disruption.  

## Functionality

- **Recording Control** Allow to control the creation and interruption of audio recordings.
- **Session-Store** Allow to read and write session data to make it available within other parts of the babelforce ecosystem
- **Outbound Calls** Allow to be the initiator of the session. Allow to create outbound calls from your end and control them.


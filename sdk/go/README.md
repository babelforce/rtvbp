# rtvbp-go

> Golang implementation of rtvbp

---

## TODO

**General**

- [x] ping: include previous rtt in next ping message

**Reliability / Failure Handling**

- If ping requests are not answered consider connection to be dead? -> terminate session, reconnect ?
- Session Re-establishment (due to websocket connection loss, or unanswered ping requests)

**Transport**

- [ ] websocket reconnect
- [ ] test quic protocol -> benchmark

**Client**

- [ ] client session must end if server dies or closes the connection


---

## Roadmap

**Audio**

- Support multiple codecs besides PCM16
- Multi-channel (stereo) in case we control a conversation instead of just a voice-bot

**Session**

- Update session to change session and audio behaviour

**Transport**

- quic: Using quic would allow for true independent streams (control stream and multiple audio streams)



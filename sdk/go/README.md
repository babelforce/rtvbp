# rtvbp-go

> Golang implementation of rtvbp

---

## TODO

**General**

- ping: include previous rtt in next ping message

**Transport**

- [ ] websocket reconnect
- [ ] test quic protocol -> benchmark

**Client**

- [ ] client session must end if server dies or closes the connection

**Server**

- configure general timeout per client session
- create example server handler

---

## Roadmap

**Audio**

- Support multiple codecs besides PCM16
- Multi-channel (stereo) in case we control a conversation instead of just a voice-bot

**Session**

- Update session to change session and audio behaviour

**Transport**

- quic: Using quic would allow for true independent streams (control stream and multiple audio streams)



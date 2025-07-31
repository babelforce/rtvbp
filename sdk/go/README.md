# rtvbp-go

> Golang implementation of rtvbp

---

## TODO

**Reliability / Failure Handling**

- If ping requests are not answered consider connection to be dead? -> terminate session, reconnect ?
- Session Re-establishment (due to websocket connection loss, or unanswered ping requests)
- On server shutdown, send reconnect request, then transport layer will disconnect and re-connect with retries
- Connect retries

**Transport**

- [ ] websocket reconnect
- [ ] test quic protocol -> benchmark

**Client**

- [ ] client session must end if server dies or closes the connection

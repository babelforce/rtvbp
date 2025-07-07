---
sidebar_position: 1
---

# Introduction

---

## What is RTVBP ?

> `RTVBP` is a lightweight session protocol which allows to integrate with babelforce
telephony services.

## Versions

### v1

**Supported Actions**

- `application.move`
- `application.hangup`

**Supported Events**

- `session.updated`
- `session.terminated`

**Audio Formats**

- Currently only `PCM16` is supported at a sample rate of `8khz`
- We will send a continuous stream of audio without any gaps
- Peers can sent partial audio as well (You do not have to transmit silence)

# RTVBP DEMO

## Quickstart

### Open AI Realtime

You can test the OpenAI Realtime without using `rtvbp`

```bash
rtvbp-demo openai
```

### Client

```bash
# connect websocket to url, output incomming audio on the speaker, use agent as audio source
rtvbp-demo client --url=ws://localhost:1234 --speaker --source=agent --agent-instructions="you are an angry customer wanting an discount"

# same but use microphone as audio input
rtvbp-demo client --url=ws://localhost:1234 --speaker --source=microphone
```

**Parameters**

- `--speaker`: playback incoming audio on the speaker
- `--source=<agent|microphone>`: will use either microphone or agent as audio source

**Agent Tools**

- `say_goodbye`: Say good bye and end the session

### Server

```bash
rtvbp-demo server --listen=0.0.0.0:1234
```

**TODO**

- provide html example with webrtc

## WebRTC

See: https://platform.openai.com/docs/guides/realtime#connection-details

**TODO**

- move audio to codewandler_audio
- make sure interruption works
- request/response handling
- generate a client for javascript, use in html file with webrtc 

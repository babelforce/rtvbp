# RTVBP - Realtime Voice Bridge Protocol

> A lightweight session protocol for telephony service integration

---

`RTVBP` is a lightweight protocol which allows to integrate with babelforce
telephony services.

It allows an integrator to take control over an ongoing telephony session in the following ways:

- Receive the callers audio data in `PCM16` format
- Send audio data to the caller in `PCM16` format
- Get notified about various events (like `session.terminated`, `call.hangup`, `dtmf.recevied`, etc )
- Execute commands (`session.terminate`, `call.hangup`, `dtmf.send`, `ivr.move`, etc)

**Server**

`Server` is a websocket endpoint provided by an integrator who wants to 
control an ongoing telephony session.

**Client**

`Client` is the session owning instance, which contacts the `Server`
to handover control.

**Event**

`TODO`

**Request/Command**

`TODO`

---

**Flow**

1. A customer calls the `babelforce` platform
2. IVR configuration initiates an `RTVBP` session
3. `babelforce` acts as a client, and will open a websocket connection to a `RTVBP` server
4. Audio data is being exchanged between both peers
5. The `server` will receive important events about the session
6. The `server` is able to send commands to the initiating party to control the call
7. When either side decides the session shall be ended, the session will be closed with `session.terminate`

---

## Getting started

In order to implement such clients we provide a simple testing
client which is able to connect to your server implementation.

### Client

Our test client is able to mimic a live call with a customer
by utilizing OpenAIs realtime voice model.

The best way currently to run the client is by using `cargo`
from within this project:

```bash
# check out the project
git clone https://github.com/babelforce/rtvbp.git
cd rtvpb

# start the client
export OPENAI_KEY=s3cr3t
cargo run --bin rtvbp-demo -- client
```

By default the client will connect to `ws://127.0.0.1:8181`
You can provide various arguments to configure its behaviour

**Example use-case**

```bash
cargo run --bin rtvbp-demo -- client \
  --agent-prompt "you are an angry customer calling for a discount"
```

**Usage**

```bash
Usage: rtvbp-demo client [OPTIONS]

Options:
  -u, --url <URL>              [default: ws://127.0.0.1:8181]
  -t, --token <TOKEN>          Authorization Bearer Token Is set as HTTP header on handshake: `Authorization: Bearer {token}`
      --agent-speed <SPEED>    [default: 1.2]
      --agent-voice <VOICE>    [default: alloy]
      --agent-prompt <PROMPT>  [default: "You are a nice and friendly person wanting to have a nice conversation"]
      --agent-lang <LANG>      [default: en-US]
      --agent-create-response  
  -h, --help                   Print help

```

```bash
# via cargo
cargo run --bin rtvbp-demo -- client
```

**Docker**

You can use docker to run the client.

Note: Unfortunately audio quality suffers when using in docker. This will be improved in the future.

```bash
# via docker
docker run \
    --rm \
    --net host \
    --env OPENAI_KEY=$OPENAI_KEY \
    --device /dev/snd -e AUDIODEV=default \
    --cap-add=sys_nice --ulimit memlock=-1 \
    ghcr.io/babelforce/rtvbp:main \
    client
```

**Start server**

You can start your own websocket endpoint or
use the demo here:

```bash
docker run \
    --rm \
    --net host \
    --env OPENAI_KEY=$OPENAI_KEY \
    ghcr.io/babelforce/rtvbp:main \
    server
```




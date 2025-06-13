# Realtime Voice Bridge Protocol

## Getting started

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

**Start Client**

By default the client connects to `ws://localhost:8181`

```bash
# via cargo
cargo run --bin rtvbp-demo -- client
```

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


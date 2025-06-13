# Realtime Voice-Bridge Protocol

---

**Quickstart**

1. Go to [AsyncAPI Studio](https://studio.asyncapi.com/)
2. Paste the contents of [asyncapi.yaml](./asyncapi.yaml) 

---

#

## Requests

**Request**

```json
{
    "id": "1234",
    "ts": 1234,
    "request": "session_create"
    "data": {
        "capabilities": {}
    }
}
```

**Response**

```json
{
    "id": "1235",
    "ts": 1235,
    "response": "1234"
    "data": {
        "session_id": "xyz"
    }
}
```

## Events

```json
{
    "id": "1234",
    "ts": 1234,
    "event": "session_destroyed",
    "data": {
    
    }
}
```

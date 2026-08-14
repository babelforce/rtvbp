---
sidebar_position: 1
sidebar_label: babelforce Cloud
description: Authenticate babelforce telephony and browser voice peers before accepting an RTVBP session.
---

# babelforce Cloud authentication

This page describes the babelforce Cloud deployment, not a universal RTVBP wire requirement.
The babelforce telephony service acts as the connecting **voice peer** and opens your
application-role WebSocket endpoint.

:::warning Deployment policy, not wire protocol
These JWT and OAuth rules are specific to babelforce Cloud. Other RTVBP deployments may choose a
different admission policy without changing the catalog, envelope, or transport framing.
:::

## Upgrade authentication

The Upgrade request carries a short-lived JWT:

```http
Authorization: Bearer <jwt>
Sec-WebSocket-Protocol: rtvbp.v1
```

Validate the token before accepting the WebSocket. Missing or invalid credentials must fail the
HTTP request with `401 Unauthorized`; no RTVBP session exists until the server returns `101`.

## JWT contract

| Item | Current value | Validation behavior |
| --- | --- | --- |
| Signing algorithm | `RS256` | Require exactly RS256 and verify against the provisioned babelforce RSA public key. |
| `iss` | `auth.babelforce.com` | Required and matched exactly. |
| `sub` | `com.babelforce.svc.telephony.realtime` | Required and matched exactly. |
| `exp` | Short-lived expiry | Required and validated. Production tokens are currently issued for about one hour; do not hard-code that duration. |
| `aud` | babelforce account ID | Required account context. Authorize it against the exact account IDs served by this endpoint; a multi-account endpoint uses an allowlist rather than one fixed value. |
| `iat` | Issued-at time | Emitted and validated when present. |
| `jti` | Unique token ID | Emitted for identification and audit; not currently an authorization decision. |
| JWT header `kid` | `jwt-rsa-2048-v1` | Identifies the current signing key; signature verification remains authoritative. |

After signature, issuer, subject, and expiry validation, authorize `aud` against the account IDs
the endpoint is configured to serve before routing or accessing account data. Do not trust `aud`
from an unvalidated token. A multi-account endpoint still needs an explicit account allowlist; a
valid token for a different babelforce account is not sufficient authorization.

## Public keys and rotation

RTVBP does not define key discovery. Obtain the PEM-encoded babelforce public key through the
deployment's trusted provisioning channel. There is no documented RTVBP JWKS endpoint, so do not
invent one in an integration.

Use `kid` to prepare coordinated rotations, allow the old and new public keys during the agreed
overlap, and remove the retired key afterward. Never ship a babelforce private key, log bearer
tokens, or disable authentication on a public endpoint.

## Go SDK integration

The WebSocket server calls `ServerConfig.AuthHandler` before upgrading. The repository includes a
[tested RS256 validator example](https://github.com/babelforce/rtvbp/tree/main/sdk/go/examples/babelforce-auth)
covering valid tokens, missing credentials, algorithm and signature failures, issuer/subject
mismatches, expiry, and the intentional audience behavior.

```go
validator, err := babelforceauth.NewValidatorPEM(publicKeyPEM)
if err != nil {
    log.Fatal(err)
}

server := ws.NewServer(ws.ServerConfig{
    Addr:        "0.0.0.0:8080",
    Path:        "/rtvbp",
    AuthHandler: validator.AuthHandler,
}, handler)
```

Use `wss://` in production even though the JWT is signed: TLS protects the bearer token and audio
from disclosure in transit.

## Browser OAuth deployment adapter

The telephony JWT contract above applies when babelforce is the connecting voice peer. A browser
cannot add `Authorization` to a native WebSocket Upgrade, so babelforce browser deployments use a
separate OAuth bearer carrier:

```http
Sec-WebSocket-Protocol: rtvbp.v1, bearer.<base64url(UTF-8 access token)>
Origin: https://app.example
```

Use `babelforceBearerSubprotocols(profile, accessToken)` from `@babelforce/rtvbp/browser`; never put a
raw opaque token in the protocol list. The accepting adapter decodes the unpadded base64url value,
validates or introspects the resulting OAuth token, selects account context, and echoes only
`rtvbp.v1` (or the explicitly offered WebRTC profile). The credential carrier is not an RTVBP
catalog operation and must not appear in application logs.

Allowlist exact production origins before accepting the Upgrade, alongside ordinary credential
validation. Do not interpret permissive CORS headers as WebSocket protection, and do not accept a
missing or `null` Origin on a public browser endpoint unless that behavior is an explicit,
separately authenticated deployment requirement.

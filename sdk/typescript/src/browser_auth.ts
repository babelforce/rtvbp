import { SessionError } from "./errors.ts";

const BASE64URL = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

function base64Url(bytes: Uint8Array): string {
  let result = "";
  for (let offset = 0; offset < bytes.byteLength; offset += 3) {
    const first = bytes[offset] ?? 0;
    const second = bytes[offset + 1];
    const third = bytes[offset + 2];
    result += BASE64URL[first >>> 2];
    result += BASE64URL[((first & 0x03) << 4) | ((second ?? 0) >>> 4)];
    if (second !== undefined) {
      result += BASE64URL[((second & 0x0f) << 2) | ((third ?? 0) >>> 6)];
    }
    if (third !== undefined) result += BASE64URL[third & 0x3f];
  }
  return result;
}

/**
 * Encode babelforce's browser deployment credential as an RFC-token-safe subprotocol value.
 * This is deployment policy, not part of RTVBP itself.
 */
export function babelforceBearerSubprotocol(accessToken: string): string {
  if (accessToken.length === 0) {
    throw new SessionError("authentication", "babelforce OAuth access token must not be empty");
  }
  return `bearer.${base64Url(new TextEncoder().encode(accessToken))}`;
}

/** Profile token followed by babelforce's base64url OAuth credential carrier. */
export function babelforceBearerSubprotocols(
  profile: string,
  accessToken: string,
): readonly string[] {
  if (profile.length === 0) throw new SessionError("configuration", "profile token must not be empty");
  return [profile, babelforceBearerSubprotocol(accessToken)];
}

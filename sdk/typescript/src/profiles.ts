import { SessionError } from "./errors.ts";
import { PROFILES } from "./generated/zz_generated_profiles.ts";
import type { MediaFormat } from "./transport.ts";
import { mediaFrameBytes } from "./transport.ts";

const L16_FORMAT = /^l16-([1-9][0-9]*)-16-([1-9][0-9]*)-([1-9][0-9]*)ms$/;

/** Interpret a generated profile's SDK-side L16 media descriptor. */
export function profileMediaFormat(profileToken: string, channel: string): MediaFormat {
  const profile = PROFILES.find((candidate) => candidate.token === profileToken);
  if (profile === undefined) {
    throw new SessionError("profile", `unknown profile '${profileToken}'`);
  }
  const media = profile.media.find((candidate) => candidate.channel === channel);
  if (media === undefined) {
    throw new SessionError("media_unsupported", `profile '${profileToken}' has no '${channel}' channel`);
  }
  const matched = L16_FORMAT.exec(media.sdkFormat);
  if (matched === null) {
    throw new SessionError("media_format", `profile '${profileToken}' does not expose L16 SDK media`);
  }
  const format: MediaFormat = {
    encoding: "L16",
    sampleRate: Number(matched[1]),
    bitDepth: 16,
    channels: Number(matched[2]),
    packetTimeMs: Number(matched[3]),
  };
  mediaFrameBytes(format);
  return format;
}

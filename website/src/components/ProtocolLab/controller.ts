import {
  Handler,
  MemoryTransport,
  Session,
  babelforceV1,
  classicV1,
  profiles,
  type ControlChannel,
  type ControlFrame,
  type MediaChannel,
  type MediaFormat,
  type ReceivedControl,
  type Transport,
  type WireEncodable,
  type WireErrorValue,
} from "@babelforce/rtvbp";
import {
  BrowserAudioDevice,
  BrowserWebRtcTransport,
  babelforceBearerSubprotocols,
  browserWebRtcTransport,
} from "@babelforce/rtvbp/browser";

export type LabProfile = "webrtc" | "websocket";
export type LabRunState = "idle" | "connecting" | "connected" | "ended" | "failed";

export interface TimelineEntry {
  readonly id: number;
  readonly elapsedMs: number;
  readonly from: string;
  readonly to: string;
  readonly kind: "profile" | "request" | "response" | "event" | "media" | "state" | "scenario";
  readonly label: string;
  readonly detail: string;
  readonly raw?: string;
  readonly reference?: string;
}

export interface LabStats {
  readonly codec: string;
  readonly connection: string;
  readonly ice: string;
  readonly candidatePair: string;
  readonly bitrateKbps?: number;
  readonly rttMs?: number;
  readonly jitterMs?: number;
  readonly packetsLost?: number;
}

export interface LabCallbacks {
  readonly timeline: (entry: Omit<TimelineEntry, "id" | "elapsedMs">) => void;
  readonly state: (state: LabRunState, message: string) => void;
  readonly stats: (stats: LabStats) => void;
  readonly levels: (voice: number, application: number) => void;
}

export interface LiveConfig {
  readonly endpoint: string;
  readonly accessToken?: string;
  readonly iceUrls?: readonly string[];
  readonly iceUsername?: string;
  readonly iceCredential?: string;
}

const AUDIO_FORMAT: MediaFormat = {
  encoding: "L16",
  sampleRate: 8_000,
  bitDepth: 16,
  channels: 1,
  packetTimeMs: 20,
};

const AUDIO_CODEC: babelforceV1.AudioCodec = {
  id: "L16/8000/1",
  name: "L16",
  sample_rate: 8_000,
  bit_depth: 16,
  channels: 1,
};

const EMPTY_STATS: LabStats = {
  codec: "Not started",
  connection: "Idle",
  ice: "Idle",
  candidatePair: "Not selected",
};

function waitForAbort(signal: AbortSignal): Promise<void> {
  if (signal.aborted) return Promise.resolve();
  return new Promise((resolve) => signal.addEventListener("abort", () => resolve(), {once: true}));
}

function referenceFor(frame: ControlFrame, requestMethods: Map<string, string>): string | undefined {
  if (frame.kind === "request") return `/docs/reference/babelforce.v1/operations/${frame.method}`;
  if (frame.kind === "event") return `/docs/reference/babelforce.v1/events/${frame.event}`;
  const method = requestMethods.get(frame.correlationId);
  return method === undefined ? undefined : `/docs/reference/babelforce.v1/operations/${method}`;
}

class ObservedControl implements ControlChannel {
  constructor(
    private readonly inner: ControlChannel,
    private readonly from: string,
    private readonly to: string,
    private readonly requestMethods: Map<string, string>,
    private readonly emit: LabCallbacks["timeline"],
  ) {}

  async send(data: string, signal?: AbortSignal): Promise<void> {
    const frame = classicV1.classicV1Envelope.decode(data);
    let label: string;
    let detail: string;
    if (frame.kind === "request") {
      this.requestMethods.set(frame.id, frame.method);
      label = frame.method;
      detail = `request ${frame.id}`;
    } else if (frame.kind === "event") {
      label = frame.event;
      detail = `event ${frame.id}`;
    } else {
      const method = this.requestMethods.get(frame.correlationId);
      label = method === undefined ? "response" : `${method} response`;
      detail = frame.error === undefined
        ? `correlated with ${frame.correlationId}`
        : `${frame.error.code}: ${frame.error.message}`;
    }
    this.emit({
      from: this.from,
      to: this.to,
      kind: frame.kind,
      label,
      detail,
      raw: data,
      reference: referenceFor(frame, this.requestMethods),
    });
    await this.inner.send(data, signal);
  }

  async receive(signal?: AbortSignal): Promise<ReceivedControl> {
    return await this.inner.receive(signal);
  }
}

class ObservedTransport implements Transport {
  readonly control: ControlChannel;
  readonly supportsKeepalive: boolean;

  constructor(
    readonly inner: Transport,
    from: string,
    to: string,
    requestMethods: Map<string, string>,
    emit: LabCallbacks["timeline"],
  ) {
    this.control = new ObservedControl(inner.control, from, to, requestMethods, emit);
    this.supportsKeepalive = inner.supportsKeepalive ?? false;
  }

  async openMedia(id: string, format: MediaFormat, signal?: AbortSignal): Promise<MediaChannel> {
    return await this.inner.openMedia(id, format, signal);
  }

  async acceptMedia(signal?: AbortSignal): Promise<MediaChannel> {
    return await this.inner.acceptMedia(signal);
  }

  async monitor(signal: AbortSignal): Promise<void> {
    if (this.inner.monitor !== undefined) return await this.inner.monitor(signal);
    await waitForAbort(signal);
  }

  async monitorKeepalive(policy: Parameters<NonNullable<Transport["monitorKeepalive"]>>[0], signal: AbortSignal): Promise<void> {
    if (this.inner.monitorKeepalive !== undefined) {
      return await this.inner.monitorKeepalive(policy, signal);
    }
    await waitForAbort(signal);
  }

  async close(): Promise<void> {
    await this.inner.close();
  }
}

interface AudioRuntime {
  readonly context: AudioContext;
  readonly setMuted: (muted: boolean) => void;
  readonly getStats?: () => Promise<RTCStatsReport>;
  readonly connection?: () => {connection: string; ice: string};
  readonly close: () => Promise<void>;
}

function rms(analyser: AnalyserNode, samples: Float32Array): number {
  analyser.getFloatTimeDomainData(samples);
  let total = 0;
  for (const sample of samples) total += sample * sample;
  return Math.min(1, Math.sqrt(total / samples.length) * 7);
}

function pcmuCodecs(): RTCRtpCodec[] {
  return (RTCRtpSender.getCapabilities("audio")?.codecs ?? [])
    .filter((codec) => codec.mimeType.toLowerCase() === "audio/pcmu");
}

async function waitForConnected(peers: readonly RTCPeerConnection[]): Promise<void> {
  const deadline = Date.now() + 7_000;
  while (Date.now() < deadline) {
    if (peers.every((peer) => peer.connectionState === "connected")) return;
    if (peers.some((peer) => peer.connectionState === "failed")) {
      throw new Error("local WebRTC loopback failed");
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error("local WebRTC loopback did not connect");
}

async function createWebRtcAudio(callbacks: LabCallbacks): Promise<AudioRuntime> {
  if (typeof AudioContext === "undefined" || typeof RTCPeerConnection === "undefined") {
    throw new Error("This browser does not expose Web Audio and WebRTC");
  }
  const context = new AudioContext();
  await context.resume();
  const voice = new RTCPeerConnection({iceServers: []});
  const application = new RTCPeerConnection({iceServers: []});
  const peers = [voice, application] as const;
  const nodes: AudioNode[] = [];
  const tracks: MediaStreamTrack[] = [];
  const oscillators: OscillatorNode[] = [];
  const analysers: AnalyserNode[] = [];

  voice.addEventListener("icecandidate", (event) => {
    if (event.candidate !== null) void application.addIceCandidate(event.candidate).catch(() => {});
  });
  application.addEventListener("icecandidate", (event) => {
    if (event.candidate !== null) void voice.addIceCandidate(event.candidate).catch(() => {});
  });

  const attachRemote = (peer: RTCPeerConnection) => {
    peer.addEventListener("track", (event) => {
      const stream = event.streams[0] ?? new MediaStream([event.track]);
      const source = context.createMediaStreamSource(stream);
      const analyser = context.createAnalyser();
      analyser.fftSize = 256;
      source.connect(analyser);
      analyser.connect(context.destination);
      nodes.push(source, analyser);
      analysers.push(analyser);
    });
  };
  attachRemote(voice);
  attachRemote(application);

  const createSender = (peer: RTCPeerConnection, frequency: number, initialGain: number) => {
    const oscillator = context.createOscillator();
    const gain = context.createGain();
    const analyser = context.createAnalyser();
    const destination = context.createMediaStreamDestination();
    oscillator.type = "sine";
    oscillator.frequency.value = frequency;
    gain.gain.value = initialGain;
    analyser.fftSize = 256;
    oscillator.connect(gain);
    gain.connect(analyser);
    analyser.connect(destination);
    const track = destination.stream.getAudioTracks()[0];
    if (track === undefined) throw new Error("browser did not create a local audio track");
    const transceiver = peer.addTransceiver(track, {direction: "sendrecv", streams: [destination.stream]});
    const codecs = pcmuCodecs();
    if (codecs.length > 0 && "setCodecPreferences" in transceiver) transceiver.setCodecPreferences(codecs);
    oscillator.start();
    nodes.push(oscillator, gain, analyser, destination);
    analysers.push(analyser);
    tracks.push(track);
    oscillators.push(oscillator);
    return gain;
  };

  const voiceGain = createSender(voice, 220, 0.035);
  const applicationGain = createSender(application, 330, 0.018);
  const offer = await voice.createOffer();
  await voice.setLocalDescription(offer);
  await application.setRemoteDescription(offer);
  const answer = await application.createAnswer();
  await application.setLocalDescription(answer);
  await voice.setRemoteDescription(answer);
  await waitForConnected(peers);

  const toneTimer = window.setInterval(() => {
    const first = Math.floor(context.currentTime * 1.25) % 2 === 0;
    applicationGain.gain.setTargetAtTime(first ? 0.012 : 0.032, context.currentTime, 0.04);
  }, 500);
  const levelSamples = new Float32Array(256);
  const levelTimer = window.setInterval(() => {
    callbacks.levels(
      analysers[0] === undefined ? 0 : rms(analysers[0], levelSamples),
      analysers[1] === undefined ? 0 : rms(analysers[1], levelSamples),
    );
  }, 120);

  return {
    context,
    setMuted(muted) {
      voiceGain.gain.setTargetAtTime(muted ? 0 : 0.035, context.currentTime, 0.025);
    },
    getStats: async () => await voice.getStats(),
    connection: () => ({connection: voice.connectionState, ice: voice.iceConnectionState}),
    async close() {
      window.clearInterval(toneTimer);
      window.clearInterval(levelTimer);
      callbacks.levels(0, 0);
      for (const oscillator of oscillators) {
        try { oscillator.stop(); } catch { /* Already stopped. */ }
      }
      for (const track of tracks) track.stop();
      for (const peer of peers) peer.close();
      for (const node of nodes) node.disconnect();
      await context.close().catch(() => {});
    },
  };
}

async function createWebSocketAudio(callbacks: LabCallbacks): Promise<AudioRuntime> {
  if (typeof AudioContext === "undefined") throw new Error("This browser does not expose Web Audio");
  const context = new AudioContext();
  await context.resume();
  const voice = context.createOscillator();
  const voiceGain = context.createGain();
  const application = context.createOscillator();
  const applicationGain = context.createGain();
  const analyserVoice = context.createAnalyser();
  const analyserApplication = context.createAnalyser();
  voice.frequency.value = 220;
  application.frequency.value = 330;
  voiceGain.gain.value = 0.025;
  applicationGain.gain.value = 0.015;
  voice.connect(voiceGain).connect(analyserVoice).connect(context.destination);
  application.connect(applicationGain).connect(analyserApplication).connect(context.destination);
  voice.start();
  application.start();
  const samples = new Float32Array(256);
  const levelTimer = window.setInterval(() => {
    callbacks.levels(rms(analyserVoice, samples), rms(analyserApplication, samples));
  }, 120);
  return {
    context,
    setMuted(muted) {
      voiceGain.gain.setTargetAtTime(muted ? 0 : 0.025, context.currentTime, 0.025);
    },
    async close() {
      window.clearInterval(levelTimer);
      callbacks.levels(0, 0);
      voice.stop();
      application.stop();
      await context.close().catch(() => {});
    },
  };
}

function statNumber(report: RTCStats | undefined, name: string): number | undefined {
  if (report === undefined) return undefined;
  const value = (report as unknown as Record<string, unknown>)[name];
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

function statString(report: RTCStats | undefined, name: string): string | undefined {
  if (report === undefined) return undefined;
  const value = (report as unknown as Record<string, unknown>)[name];
  return typeof value === "string" ? value : undefined;
}

function summarizeStats(
  report: RTCStatsReport,
  connection: {connection: string; ice: string},
  previous: {bytes: number; at: number} | undefined,
): {stats: LabStats; sample?: {bytes: number; at: number}} {
  let inbound: RTCStats | undefined;
  let outbound: RTCStats | undefined;
  let pair: RTCStats | undefined;
  const byId = new Map<string, RTCStats>();
  report.forEach((entry) => {
    byId.set(entry.id, entry);
    const mediaKind = statString(entry, "kind") ?? statString(entry, "mediaType");
    if (entry.type === "inbound-rtp" && mediaKind === "audio") inbound = entry;
    if (entry.type === "outbound-rtp" && mediaKind === "audio") outbound = entry;
    if (entry.type === "candidate-pair" && statString(entry, "state") === "succeeded") {
      if ((entry as unknown as Record<string, unknown>).nominated === true || pair === undefined) pair = entry;
    }
  });
  const codecId = inbound === undefined ? undefined : statString(inbound, "codecId");
  const codec = codecId === undefined ? undefined : byId.get(codecId);
  const mime = codec === undefined ? undefined : statString(codec, "mimeType");
  const clock = codec === undefined ? undefined : statNumber(codec, "clockRate");
  const bytes = outbound === undefined ? undefined : statNumber(outbound, "bytesSent");
  const now = performance.now();
  const bitrate = bytes === undefined || previous === undefined || now <= previous.at
    ? undefined
    : ((bytes - previous.bytes) * 8) / (now - previous.at);
  const localId = pair === undefined ? undefined : statString(pair, "localCandidateId");
  const remoteId = pair === undefined ? undefined : statString(pair, "remoteCandidateId");
  const localType = localId === undefined ? undefined : statString(byId.get(localId), "candidateType");
  const remoteType = remoteId === undefined ? undefined : statString(byId.get(remoteId), "candidateType");
  return {
    stats: {
      codec: mime === undefined ? "PCMU / 8 kHz" : `${mime.replace(/^audio\//i, "")} / ${Math.round((clock ?? 8_000) / 1_000)} kHz`,
      connection: connection.connection,
      ice: connection.ice,
      candidatePair: localType === undefined || remoteType === undefined
        ? "Selected local pair"
        : `${localType} ↔ ${remoteType}`,
      ...(bitrate === undefined ? {} : {bitrateKbps: Math.max(0, bitrate)}),
      ...(pair === undefined || statNumber(pair, "currentRoundTripTime") === undefined
        ? {}
        : {rttMs: statNumber(pair, "currentRoundTripTime")! * 1_000}),
      ...(inbound === undefined || statNumber(inbound, "jitter") === undefined
        ? {}
        : {jitterMs: statNumber(inbound, "jitter")! * 1_000}),
      ...(inbound === undefined || statNumber(inbound, "packetsLost") === undefined
        ? {}
        : {packetsLost: statNumber(inbound, "packetsLost")!}),
    },
    ...(bytes === undefined ? {} : {sample: {bytes, at: now}}),
  };
}

function frameTone(frequency: number, phaseOffset: number): Uint8Array {
  const frame = new Uint8Array(320);
  const view = new DataView(frame.buffer);
  for (let sample = 0; sample < 160; sample += 1) {
    const value = Math.round(Math.sin(((sample + phaseOffset) * Math.PI * 2 * frequency) / 8_000) * 4_200);
    view.setInt16(sample * 2, value, true);
  }
  return frame;
}

export class ProtocolLabController {
  private readonly requestMethods = new Map<string, string>();
  private readonly callbacks: LabCallbacks;
  private voiceSession?: Session;
  private applicationSession?: Session;
  private voiceRun?: Promise<void>;
  private applicationRun?: Promise<void>;
  private audio?: AudioRuntime;
  private browserDevice?: BrowserAudioDevice;
  private liveTransport?: BrowserWebRtcTransport;
  private liveTrack?: MediaStreamTrack;
  private mediaAbort?: AbortController;
  private statsTimer?: number;
  private sequence = 0;
  private stopping = false;

  constructor(callbacks: LabCallbacks) {
    this.callbacks = callbacks;
    callbacks.stats(EMPTY_STATS);
  }

  async startSimulation(profile: LabProfile): Promise<void> {
    await this.closeResources();
    this.stopping = false;
    this.requestMethods.clear();
    this.callbacks.state("connecting", "Negotiating the local session…");
    const profileName = profile === "webrtc" ? "rtvbp.webrtc.v1" : "rtvbp.v1";
    this.callbacks.timeline({
      from: "Browser phone",
      to: "Local application",
      kind: "profile",
      label: profileName,
      detail: profile === "webrtc"
        ? "WebSocket control · WebRTC PCMU media"
        : "WebSocket control · L16 binary media",
      reference: "/docs/reference/profiles",
    });

    const [voiceBase, applicationBase] = MemoryTransport.pair();
    const voiceTransport = new ObservedTransport(
      voiceBase,
      "Voice",
      "Application",
      this.requestMethods,
      this.callbacks.timeline,
    );
    const applicationTransport = new ObservedTransport(
      applicationBase,
      "Application",
      "Voice",
      this.requestMethods,
      this.callbacks.timeline,
    );
    this.createSessions(voiceTransport, applicationTransport);
    this.voiceRun = this.voiceSession!.run();
    this.applicationRun = this.applicationSession!.run();
    await Promise.all([this.voiceSession!.ready, this.applicationSession!.ready]);

    const applicationPeer = new babelforceV1.ApplicationPeer(this.voiceSession!);
    await applicationPeer.sessionInitialize({
      application: {id: "protocol-lab"},
      call: {
        id: "call-demo-1",
        session_id: "session-demo-1",
        from: "+12025550100",
        to: "+12025550101",
      },
      audio_codec_offerings: [AUDIO_CODEC],
      metadata: {mode: "browser-only simulation"},
    });
    await new babelforceV1.VoiceEvents(this.voiceSession!).sessionUpdated({audio_codec: AUDIO_CODEC});

    if (profile === "webrtc") {
      this.callbacks.timeline({
        from: "Voice",
        to: "Application",
        kind: "request",
        label: "transport.webrtc.offer",
        detail: "Local SDP/ICE exchanged; raw addressing hidden",
        reference: "/docs/transports/webrtc-websocket#signaling",
      });
      this.audio = await createWebRtcAudio(this.callbacks);
      this.callbacks.timeline({
        from: "Voice",
        to: "Application",
        kind: "media",
        label: "SRTP audio active",
        detail: "Bidirectional PCMU through a local RTCPeerConnection pair",
        reference: "/docs/transports/webrtc-websocket#audio-formats",
      });
      this.startStatsPolling(this.audio);
    } else {
      this.audio = await createWebSocketAudio(this.callbacks);
      await Promise.all([this.voiceSession!.openAudio(AUDIO_FORMAT), this.applicationSession!.acceptAudio()]);
      this.callbacks.timeline({
        from: "Voice",
        to: "Application",
        kind: "media",
        label: "L16 audio active",
        detail: "20 ms, 8 kHz mono frames over the SDK memory channel",
        reference: "/docs/transports/websocket#audio-framing",
      });
      this.startFramePump();
      this.callbacks.stats({
        codec: "L16 / 8 kHz",
        connection: "active",
        ice: "Not applicable",
        candidatePair: "Not applicable",
      });
    }
    await new babelforceV1.ApplicationEvents(this.applicationSession!).outputTranscriptDelta({
      delta: "Hello from the local application.",
    });
    await new babelforceV1.ApplicationEvents(this.applicationSession!).outputTranscriptDone({
      text: "Hello from the local application.",
    });
    this.callbacks.state("connected", "Connected");
  }

  async startLive(config: LiveConfig): Promise<void> {
    await this.closeResources();
    this.stopping = false;
    if (!config.endpoint.startsWith("wss://")) throw new Error("Live endpoints must use wss://");
    this.callbacks.state("connecting", "Requesting media and connecting to your endpoint…");
    this.callbacks.timeline({
      from: "Browser phone",
      to: "Caller-supplied endpoint",
      kind: "profile",
      label: profiles.PROFILE_RTVBP_WEBRTC_V1,
      detail: "Live mode · endpoint and credentials remain in browser memory",
      reference: "/docs/reference/profiles",
    });
    const stream = await navigator.mediaDevices.getUserMedia({audio: true});
    const track = stream.getAudioTracks()[0];
    if (track === undefined) throw new Error("The browser did not provide a microphone track");
    this.liveTrack = track;
    this.browserDevice = new BrowserAudioDevice({stream});
    const iceServers = config.iceUrls === undefined || config.iceUrls.length === 0
      ? []
      : [{
          urls: [...config.iceUrls],
          ...(config.iceUsername === undefined ? {} : {username: config.iceUsername}),
          ...(config.iceCredential === undefined ? {} : {credential: config.iceCredential}),
        }];
    const protocols = config.accessToken === undefined || config.accessToken.length === 0
      ? [profiles.PROFILE_RTVBP_WEBRTC_V1]
      : babelforceBearerSubprotocols(profiles.PROFILE_RTVBP_WEBRTC_V1, config.accessToken);
    const baseFactory = browserWebRtcTransport({
      url: config.endpoint,
      protocols,
      audioDevice: this.browserDevice,
      rtcConfiguration: {iceServers},
    });
    const handler = new Handler({
      adapter: babelforceV1.voiceAdapter(this.voiceHandler(), this.voiceEventHandler()),
      onBegin: async (context) => await context.acceptAudio(),
    });
    this.voiceSession = new Session({
      id: "protocol-lab-live-voice",
      envelope: classicV1.classicV1Envelope,
      handler,
      transportFactory: async (envelope, signal) => {
        const base = await baseFactory(envelope, signal);
        if (!(base instanceof BrowserWebRtcTransport)) {
          throw new Error("WebRTC transport was not selected");
        }
        this.liveTransport = base;
        return new ObservedTransport(
          base,
          "Voice",
          "Application",
          this.requestMethods,
          this.callbacks.timeline,
        );
      },
    });
    this.voiceRun = this.voiceSession.run();
    await this.voiceSession.ready;
    await new babelforceV1.ApplicationPeer(this.voiceSession).sessionInitialize({
      application: {id: "protocol-lab-live"},
      call: {
        id: `browser-${Date.now()}`,
        session_id: `browser-${Date.now()}`,
        from: "browser",
        to: "application",
      },
      audio_codec_offerings: [AUDIO_CODEC],
      metadata: {source: "public protocol lab"},
    });
    this.statsTimer = window.setInterval(() => void this.pollLiveStats(), 1_000);
    await this.pollLiveStats();
    this.callbacks.state("connected", "Connected");
  }

  setMuted(muted: boolean): void {
    this.audio?.setMuted(muted);
    if (this.liveTransport !== undefined) {
      if (this.liveTrack !== undefined) this.liveTrack.enabled = !muted;
    }
  }

  async resumeAudio(): Promise<void> {
    await this.audio?.context.resume();
  }

  async sendDtmf(digit: string): Promise<void> {
    if (this.voiceSession === undefined) return;
    const pressed = Date.now();
    this.sequence += 1;
    await new babelforceV1.VoiceEvents(this.voiceSession).dtmf({
      seq: this.sequence,
      pressed_at: pressed,
      released_at: pressed + 120,
      digit,
    });
  }

  async bargeIn(): Promise<void> {
    if (this.applicationSession === undefined) return;
    await new babelforceV1.ApplicationEvents(this.applicationSession).audioSpeechStarted({origin: "sender"});
    await new babelforceV1.VoicePeer(this.applicationSession).audioBufferClear({});
  }

  async clearBuffer(): Promise<void> {
    if (this.applicationSession !== undefined) {
      await new babelforceV1.VoicePeer(this.applicationSession).audioBufferClear({});
      return;
    }
    this.browserDevice?.clearPlayback();
  }

  async hangup(): Promise<void> {
    if (this.voiceSession !== undefined && this.applicationSession !== undefined) {
      await new babelforceV1.VoicePeer(this.applicationSession).callHangup({reason: "caller"}).catch(() => {});
    } else if (this.voiceSession !== undefined) {
      await new babelforceV1.ApplicationPeer(this.voiceSession)
        .sessionTerminate({reason: "caller"})
        .catch(() => {});
    }
    await this.closeResources();
    this.callbacks.timeline({
      from: "Session",
      to: "Browser",
      kind: "state",
      label: "closed",
      detail: "Sessions, media tracks, timers, and audio resources released",
    });
    this.callbacks.state("ended", "Ended");
  }

  async close(): Promise<void> {
    await this.closeResources();
  }

  private createSessions(voiceTransport: Transport, applicationTransport: Transport): void {
    this.voiceSession = new Session({
      id: "protocol-lab-voice",
      envelope: classicV1.classicV1Envelope,
      transport: voiceTransport,
      handler: new Handler({adapter: babelforceV1.voiceAdapter(this.voiceHandler(), this.voiceEventHandler())}),
    });
    this.applicationSession = new Session({
      id: "protocol-lab-application",
      envelope: classicV1.classicV1Envelope,
      transport: applicationTransport,
      handler: new Handler({
        adapter: babelforceV1.applicationAdapter(
          {
            ping: (_context, request) => this.pingResponse(request),
            sessionInitialize: () => ({audio_codec: AUDIO_CODEC}),
            sessionTerminate: () => ({}),
          },
          {
            audioInfo: async () => {},
            callHangup: async () => {},
            dtmf: async () => {},
            sessionUpdated: async () => {},
          },
        ),
      }),
    });
  }

  private voiceHandler(): babelforceV1.VoiceHandler {
    return {
      applicationMove: async (_context, request) => ({next_application_id: request.application_id}),
      audioBufferClear: async () => ({len: this.voiceSession?.audio.clear() ?? this.browserDevice?.clearPlayback() ?? 0}),
      callHangup: async () => ({}),
      ping: (_context, request) => this.pingResponse(request),
      recordingStart: async () => ({id: "recording-protocol-lab"}),
      recordingStop: async () => ({}),
      sessionGet: async () => ({mode: "protocol-lab"}),
      sessionSet: async () => ({}),
    };
  }

  private voiceEventHandler(): babelforceV1.VoiceEventHandler {
    return {
      agentToolCall: async () => {},
      audioSpeechStarted: async () => {},
      inputTranscript: async () => {},
      outputTranscriptDelta: async () => {},
      outputTranscriptDone: async () => {},
    };
  }

  private pingResponse(request: babelforceV1.PingRequest): babelforceV1.PingResponse {
    const now = Date.now();
    return {
      t0: request.t0,
      t1: now,
      t2: now,
      owd: 0,
      ...(request.data === undefined ? {} : {data: request.data}),
    };
  }

  private startFramePump(): void {
    this.mediaAbort = new AbortController();
    const signal = this.mediaAbort.signal;
    const voice = this.voiceSession!;
    const application = this.applicationSession!;
    void (async () => {
      let phase = 0;
      while (!signal.aborted) {
        try {
          await Promise.all([
            voice.audio.write(frameTone(220, phase), signal),
            application.audio.write(frameTone(330, phase), signal),
          ]);
          await Promise.all([
            application.audio.read(320, signal),
            voice.audio.read(320, signal),
          ]);
          phase += 160;
          await new Promise((resolve) => window.setTimeout(resolve, 100));
        } catch {
          if (!signal.aborted) this.callbacks.state("failed", "The local audio simulation stopped unexpectedly");
          return;
        }
      }
    })();
  }

  private startStatsPolling(audio: AudioRuntime): void {
    let previous: {bytes: number; at: number} | undefined;
    const poll = async () => {
      if (audio.getStats === undefined || audio.connection === undefined) return;
      const summary = summarizeStats(await audio.getStats(), audio.connection(), previous);
      previous = summary.sample;
      this.callbacks.stats(summary.stats);
    };
    void poll();
    this.statsTimer = window.setInterval(() => void poll().catch(() => {}), 1_000);
  }

  private async pollLiveStats(): Promise<void> {
    if (this.liveTransport === undefined) return;
    const report = await this.liveTransport.getStats();
    const summary = summarizeStats(
      report,
      {
        connection: this.liveTransport.connectionState,
        ice: this.liveTransport.connectionState === "connected" ? "connected" : "checking",
      },
      undefined,
    );
    this.callbacks.stats(summary.stats);
  }

  private async closeResources(): Promise<void> {
    if (this.stopping) return;
    this.stopping = true;
    if (this.statsTimer !== undefined) window.clearInterval(this.statsTimer);
    this.statsTimer = undefined;
    this.mediaAbort?.abort();
    this.mediaAbort = undefined;
    await Promise.all([
      this.voiceSession?.close().catch(() => {}),
      this.applicationSession?.close().catch(() => {}),
    ]);
    await Promise.all([
      this.voiceRun?.catch(() => {}),
      this.applicationRun?.catch(() => {}),
    ]);
    await this.audio?.close();
    await this.browserDevice?.close().catch(() => {});
    this.liveTrack?.stop();
    this.voiceSession = undefined;
    this.applicationSession = undefined;
    this.voiceRun = undefined;
    this.applicationRun = undefined;
    this.audio = undefined;
    this.browserDevice = undefined;
    this.liveTransport = undefined;
    this.liveTrack = undefined;
    this.callbacks.levels(0, 0);
    this.callbacks.stats(EMPTY_STATS);
    this.stopping = false;
  }
}

export function encodeScenarioStep(step: Readonly<Record<string, unknown>>): string {
  let frame: ControlFrame;
  if (step.kind === "request") {
    frame = {
      kind: "request",
      id: String(step.id),
      method: String(step.method),
      ...(step.params === undefined ? {} : {params: step.params as WireEncodable}),
    };
  } else if (step.kind === "event") {
    frame = {
      kind: "event",
      id: String(step.id),
      event: String(step.event),
      ...(step.data === undefined ? {} : {data: step.data as WireEncodable}),
    };
  } else {
    frame = {
      kind: "response",
      correlationId: String(step.response),
      ...(step.result === undefined ? {} : {result: step.result as WireEncodable}),
      ...(step.error === undefined ? {} : {error: step.error as WireErrorValue}),
    };
  }
  return classicV1.classicV1Envelope.encode(frame);
}

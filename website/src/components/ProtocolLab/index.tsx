import type {CSSProperties, ReactNode} from "react";
import {useEffect, useMemo, useRef, useState} from "react";
import Link from "@docusaurus/Link";
import Heading from "@theme/Heading";

import bargeInScenario from "../../../../conformance/babelforce.v1/scenarios/barge-in.json";
import initializeScenario from "../../../../conformance/babelforce.v1/scenarios/initialize-updated-dtmf.json";
import pingScenario from "../../../../conformance/babelforce.v1/scenarios/ping.json";
import terminationScenario from "../../../../conformance/babelforce.v1/scenarios/termination.json";
import {
  ProtocolLabController,
  encodeScenarioStep,
  type LabProfile,
  type LabRunState,
  type LabStats,
  type TimelineEntry,
} from "./controller";
import styles from "./styles.module.css";

type ScenarioStep = Readonly<Record<string, unknown>> & {
  readonly kind: "request" | "response" | "event";
  readonly from: string;
  readonly method?: string;
  readonly event?: string;
  readonly id?: string;
  readonly response?: string;
};

interface ScenarioCase {
  readonly key: string;
  readonly title: string;
  readonly description: string;
  readonly steps: readonly ScenarioStep[];
}

interface ScenarioDocument {
  readonly name: string;
  readonly cases: readonly {
    readonly name: string;
    readonly description: string;
    readonly steps: readonly ScenarioStep[];
  }[];
}

const scenarioDocuments = [
  initializeScenario,
  bargeInScenario,
  pingScenario,
  terminationScenario,
] as readonly ScenarioDocument[];

const scenarios: readonly ScenarioCase[] = scenarioDocuments.flatMap((document) =>
  document.cases.map((scenario) => ({
    key: `${document.name}/${scenario.name}`,
    title: `${document.name} · ${scenario.name}`,
    description: scenario.description,
    steps: scenario.steps,
  })),
);

const initialStats: LabStats = {
  codec: "Not started",
  connection: "Idle",
  ice: "Idle",
  candidatePair: "Not selected",
};

const dtmfDigits = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "*", "0", "#"] as const;

function titleCase(value: string): string {
  if (value.length === 0) return value;
  return value[0]!.toUpperCase() + value.slice(1);
}

function displayMetric(value: number | undefined, unit = ""): string {
  return value === undefined ? "—" : `${value.toFixed(value >= 10 ? 0 : 1)}${unit}`;
}

function scenarioLabel(step: ScenarioStep): string {
  if (step.kind === "request") return step.method ?? "request";
  if (step.kind === "event") return step.event ?? "event";
  return "correlated response";
}

function scenarioReference(step: ScenarioStep): string | undefined {
  if (step.kind === "request" && step.method !== undefined) {
    return `/docs/reference/babelforce.v1/operations/${step.method}`;
  }
  if (step.kind === "event" && step.event !== undefined) {
    return `/docs/reference/babelforce.v1/events/${step.event}`;
  }
  return undefined;
}

function Direction({entry}: {entry: TimelineEntry}): ReactNode {
  return (
    <span className={styles.direction}>
      <span>{entry.from}</span>
      <span aria-hidden="true">→</span>
      <span>{entry.to}</span>
    </span>
  );
}

export default function ProtocolLab(): ReactNode {
  const controller = useRef<ProtocolLabController | undefined>(undefined);
  const startedAt = useRef(0);
  const entryId = useRef(0);
  const [mounted, setMounted] = useState(false);
  const [mode, setMode] = useState<"simulation" | "live">("simulation");
  const [profile, setProfile] = useState<LabProfile>("webrtc");
  const [runState, setRunState] = useState<LabRunState>("idle");
  const [statusMessage, setStatusMessage] = useState("Ready — no network or microphone needed");
  const [timeline, setTimeline] = useState<readonly TimelineEntry[]>([]);
  const [stats, setStats] = useState<LabStats>(initialStats);
  const [voiceLevel, setVoiceLevel] = useState(0);
  const [applicationLevel, setApplicationLevel] = useState(0);
  const [muted, setMuted] = useState(false);
  const [showRaw, setShowRaw] = useState(false);
  const [selectedScenario, setSelectedScenario] = useState(scenarios[0]!.key);
  const [scenarioStep, setScenarioStep] = useState(0);
  const [endpoint, setEndpoint] = useState("");
  const [accessToken, setAccessToken] = useState("");
  const [iceUrls, setIceUrls] = useState("");
  const [error, setError] = useState<string>();

  const pushTimeline = (entry: Omit<TimelineEntry, "id" | "elapsedMs">) => {
    entryId.current += 1;
    const elapsedMs = startedAt.current === 0 ? 0 : Math.max(0, performance.now() - startedAt.current);
    setTimeline((current) => [...current, {...entry, id: entryId.current, elapsedMs}]);
  };

  useEffect(() => {
    const lab = new ProtocolLabController({
      timeline: pushTimeline,
      state: (state, message) => {
        setRunState(state);
        setStatusMessage(message);
      },
      stats: setStats,
      levels: (voice, application) => {
        setVoiceLevel(voice);
        setApplicationLevel(application);
      },
    });
    controller.current = lab;
    setMounted(true);
    return () => {
      controller.current = undefined;
      void lab.close();
    };
  }, []);

  const activeScenario = useMemo(
    () => scenarios.find((scenario) => scenario.key === selectedScenario) ?? scenarios[0]!,
    [selectedScenario],
  );
  const isActive = runState === "connecting" || runState === "connected";
  const canInteract = runState === "connected";

  const start = async () => {
    if (controller.current === undefined) return;
    setError(undefined);
    setMuted(false);
    setTimeline([]);
    entryId.current = 0;
    startedAt.current = performance.now();
    try {
      if (mode === "simulation") {
        await controller.current.startSimulation(profile);
      } else {
        await controller.current.startLive({
          endpoint: endpoint.trim(),
          ...(accessToken.length === 0 ? {} : {accessToken}),
          ...(iceUrls.trim().length === 0
            ? {}
            : {iceUrls: iceUrls.split(",").map((url) => url.trim()).filter(Boolean)}),
        });
      }
    } catch (failure) {
      const message = failure instanceof Error ? failure.message : String(failure);
      setError(message);
      setRunState("failed");
      setStatusMessage("Could not start");
      await controller.current.close();
    }
  };

  const hangup = async () => {
    setError(undefined);
    await controller.current?.hangup();
  };

  const toggleMute = () => {
    const next = !muted;
    setMuted(next);
    controller.current?.setMuted(next);
  };

  const replayNext = () => {
    const index = scenarioStep >= activeScenario.steps.length ? 0 : scenarioStep;
    const step = activeScenario.steps[index]!;
    const next = index + 1;
    const raw = encodeScenarioStep(step);
    const from = titleCase(step.from);
    const correlatedRequest = step.kind === "response"
      ? activeScenario.steps.find((candidate) => candidate.id === step.response && candidate.kind === "request")
      : undefined;
    pushTimeline({
      from,
      to: step.from === "voice" ? "Application" : "Voice",
      kind: "scenario",
      label: scenarioLabel(step),
      detail: `generated scenario step ${next}/${activeScenario.steps.length}`,
      raw,
      reference: scenarioReference(step) ?? (correlatedRequest === undefined ? undefined : scenarioReference(correlatedRequest)),
    });
    setScenarioStep(next >= activeScenario.steps.length ? 0 : next);
  };

  return (
    <section className={styles.lab} data-testid="protocol-lab" aria-label="Interactive RTVBP protocol lab">
      <div className={styles.labHeader}>
        <div>
          <p className={styles.kicker}>Runs in this tab</p>
          <Heading as="h2">Browser phone</Heading>
        </div>
        <div className={styles.modeSwitch} aria-label="Lab mode">
          <button
            type="button"
            aria-pressed={mode === "simulation"}
            className={mode === "simulation" ? styles.selected : undefined}
            disabled={isActive}
            onClick={() => setMode("simulation")}
          >
            Safe simulation
          </button>
          <button
            type="button"
            aria-pressed={mode === "live"}
            className={mode === "live" ? styles.selected : undefined}
            disabled={isActive}
            onClick={() => setMode("live")}
          >
            Live endpoint
          </button>
        </div>
      </div>

      <div className={styles.statusBar} role="status" aria-live="polite">
        <span className={`${styles.stateDot} ${styles[`state-${runState}`]}`} aria-hidden="true" />
        <strong>{statusMessage}</strong>
        <span>
          {mode === "simulation"
            ? "Everything stays in your browser"
            : "Only your selected endpoint receives the call"}
        </span>
      </div>

      <div className={styles.workspace}>
        <div className={styles.phoneColumn}>
          {mode === "simulation" ? (
            <fieldset className={styles.profilePicker} disabled={isActive}>
              <legend>Media path</legend>
              <label>
                <input
                  type="radio"
                  name="profile"
                  value="webrtc"
                  checked={profile === "webrtc"}
                  onChange={() => setProfile("webrtc")}
                />
                <span><strong>WebRTC</strong><small>Real local peer connection + PCMU</small></span>
              </label>
              <label>
                <input
                  type="radio"
                  name="profile"
                  value="websocket"
                  checked={profile === "websocket"}
                  onChange={() => setProfile("websocket")}
                />
                <span><strong>WebSocket</strong><small>Deterministic L16 frames</small></span>
              </label>
            </fieldset>
          ) : (
            <div className={styles.liveForm}>
              <label htmlFor="live-endpoint">WebSocket endpoint</label>
              <input
                id="live-endpoint"
                type="url"
                inputMode="url"
                autoComplete="off"
                placeholder="wss://your-endpoint.example/rtvbp"
                value={endpoint}
                disabled={isActive}
                onChange={(event) => setEndpoint(event.target.value)}
              />
              <label htmlFor="live-token">Bearer token <span>optional</span></label>
              <input
                id="live-token"
                type="password"
                autoComplete="off"
                value={accessToken}
                disabled={isActive}
                onChange={(event) => setAccessToken(event.target.value)}
              />
              <label htmlFor="live-ice">STUN/TURN URLs <span>optional, comma-separated</span></label>
              <input
                id="live-ice"
                type="text"
                autoComplete="off"
                placeholder="stun:stun.example:3478"
                value={iceUrls}
                disabled={isActive}
                onChange={(event) => setIceUrls(event.target.value)}
              />
              <p>No value is stored, logged, or built into this site. Live mode requests microphone access.</p>
            </div>
          )}

          <div className={styles.callActions}>
            {!isActive ? (
              <button
                type="button"
                className={styles.callButton}
                disabled={!mounted || (mode === "live" && endpoint.trim().length === 0)}
                onClick={() => void start()}
              >
                <span aria-hidden="true">↗</span>
                {runState === "ended" || runState === "failed" ? "Call again" : "Call"}
              </button>
            ) : (
              <button
                type="button"
                className={styles.hangupButton}
                disabled={runState === "connecting"}
                onClick={() => void hangup()}
              >
                <span aria-hidden="true">×</span>
                Hang up
              </button>
            )}
            <button type="button" className={styles.secondaryButton} onClick={() => void controller.current?.resumeAudio()} disabled={!canInteract}>
              Enable audio
            </button>
          </div>

          {error !== undefined && <p className={styles.error} role="alert">{error}</p>}

          <div className={styles.meters} aria-label="Duplex audio levels">
            <div>
              <span>Caller → app</span>
              <div className={styles.meterTrack}>
                <i
                  data-testid="audio-meter-voice"
                  style={{"--level": `${Math.round(voiceLevel * 100)}%`} as CSSProperties}
                />
              </div>
            </div>
            <div>
              <span>App → caller</span>
              <div className={styles.meterTrack}>
                <i
                  data-testid="audio-meter-application"
                  style={{"--level": `${Math.round(applicationLevel * 100)}%`} as CSSProperties}
                />
              </div>
            </div>
          </div>

          <div className={styles.keypad} aria-label="Phone controls">
            {dtmfDigits.map((digit) => (
              <button
                type="button"
                key={digit}
                aria-label={`Send DTMF ${digit}`}
                disabled={!canInteract}
                onClick={() => void controller.current?.sendDtmf(digit)}
              >
                {digit}
              </button>
            ))}
          </div>

          <div className={styles.controlGrid}>
            <button type="button" disabled={!canInteract} onClick={toggleMute}>
              {muted ? "Unmute" : "Mute"}
            </button>
            <button
              type="button"
              disabled={!canInteract || mode !== "simulation"}
              onClick={() => void controller.current?.bargeIn()}
            >
              Barge in
            </button>
            <button
              type="button"
              aria-label="Clear audio buffer"
              disabled={!canInteract}
              onClick={() => void controller.current?.clearBuffer()}
            >
              Clear buffer
            </button>
          </div>
        </div>

        <div className={styles.inspectorColumn}>
          <div className={styles.inspectorHeader}>
            <div>
              <p className={styles.kicker}>Glass-box view</p>
              <Heading as="h2">Protocol timeline</Heading>
            </div>
            <button type="button" className={styles.rawToggle} aria-pressed={showRaw} onClick={() => setShowRaw(!showRaw)}>
              {showRaw ? "Hide raw payloads" : "Show raw payloads"}
            </button>
          </div>

          <ol className={styles.timeline} aria-label="RTVBP messages and media state">
            {timeline.length === 0 ? (
              <li className={styles.emptyTimeline}>
                <strong>Ready when you are.</strong>
                <span>Call to watch profile negotiation, typed frames, media, and shutdown.</span>
              </li>
            ) : timeline.map((entry) => (
              <li key={entry.id} className={styles.timelineEntry} data-kind={entry.kind}>
                <div className={styles.timelineMeta}>
                  <time>+{(entry.elapsedMs / 1_000).toFixed(2)}s</time>
                  <span>{entry.kind}</span>
                  <Direction entry={entry} />
                </div>
                <div className={styles.timelineBody}>
                  {entry.reference === undefined ? (
                    <strong>{entry.label}</strong>
                  ) : (
                    <Link to={entry.reference}>{entry.label}</Link>
                  )}
                  <p>{entry.detail}</p>
                  {showRaw && entry.raw !== undefined && <code>{entry.raw}</code>}
                </div>
              </li>
            ))}
          </ol>
        </div>
      </div>

      <div className={styles.lowerGrid}>
        <section className={styles.statsPanel} aria-labelledby="webrtc-health">
          <div className={styles.panelHeading}>
            <div>
              <p className={styles.kicker}>Browser-reported</p>
              <Heading id="webrtc-health" as="h2">WebRTC health</Heading>
            </div>
            <span>{profile === "webrtc" || mode === "live" ? "Live statistics" : "Not used by this profile"}</span>
          </div>
          <dl className={styles.statsGrid}>
            <div><dt>Codec</dt><dd>{stats.codec}</dd></div>
            <div><dt>Connection</dt><dd>{titleCase(stats.connection)}</dd></div>
            <div><dt>ICE</dt><dd>{titleCase(stats.ice)}</dd></div>
            <div><dt>Selected pair</dt><dd>{stats.candidatePair}</dd></div>
            <div><dt><abbr title="Estimated outbound media bitrate">Bitrate</abbr></dt><dd>{displayMetric(stats.bitrateKbps, " kbps")}</dd></div>
            <div><dt><abbr title="Current round-trip time">RTT</abbr></dt><dd>{displayMetric(stats.rttMs, " ms")}</dd></div>
            <div><dt><abbr title="Variation in packet arrival time">Jitter</abbr></dt><dd>{displayMetric(stats.jitterMs, " ms")}</dd></div>
            <div><dt>Packets lost</dt><dd>{displayMetric(stats.packetsLost)}</dd></div>
          </dl>
          <p>A dash means this browser did not report the statistic. Candidate addresses and SDP are never displayed.</p>
        </section>

        <section className={styles.scenarioPanel} aria-labelledby="scenario-replay">
          <div className={styles.panelHeading}>
            <div>
              <p className={styles.kicker}>Spec-generated proof</p>
              <Heading id="scenario-replay" as="h2">Generated conformance scenario</Heading>
            </div>
            <span>{scenarioStep + 1} / {activeScenario.steps.length}</span>
          </div>
          <label htmlFor="scenario-select">Scenario</label>
          <select
            id="scenario-select"
            value={selectedScenario}
            onChange={(event) => {
              setSelectedScenario(event.target.value);
              setScenarioStep(0);
            }}
          >
            {scenarios.map((scenario) => <option key={scenario.key} value={scenario.key}>{scenario.title}</option>)}
          </select>
          <p>{activeScenario.description}</p>
          <button type="button" className={styles.scenarioButton} onClick={replayNext}>Next scenario step</button>
          <Link to={`/docs/reference/babelforce.v1/flows/${activeScenario.key.split("/")[0]}`}>
            Open generated flow reference <span aria-hidden="true">→</span>
          </Link>
        </section>
      </div>
    </section>
  );
}

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  AsyncQueue,
  Handler,
  MemoryTransport,
  RemoteError,
  Session,
  babelforceV1,
  classicV1,
  demoV1,
} from "../src/index.ts";
import type { ControlFrame } from "../src/envelope.ts";
import type { RoleAdapter } from "../src/protocol.ts";

interface Scenario {
  readonly name: string;
  readonly roles: Readonly<Record<string, "application" | "voice">>;
  readonly cases: readonly ScenarioCase[];
}

interface ScenarioCase {
  readonly name: string;
  readonly steps: readonly ScenarioStep[];
}

interface ScenarioStep {
  readonly kind: "request" | "response" | "event";
  readonly from: string;
  readonly id?: string;
  readonly method?: string;
  readonly params?: unknown;
  readonly response?: string;
  readonly result?: unknown;
  readonly error?: { readonly code: number; readonly message: string; readonly data?: unknown };
  readonly event?: string;
  readonly data?: unknown;
}

const scenarioPaths = [
  "../../../conformance/babelforce.v1/scenarios/barge-in.json",
  "../../../conformance/babelforce.v1/scenarios/initialize-updated-dtmf.json",
  "../../../conformance/babelforce.v1/scenarios/ping.json",
  "../../../conformance/babelforce.v1/scenarios/termination.json",
  "../../../conformance/demo.v1/scenarios/echo-observed.json",
];

function loadScenario(path: string): Scenario {
  return JSON.parse(readFileSync(new URL(path, import.meta.url), "utf8")) as Scenario;
}

function responseValues(testCase: ScenarioCase): Map<string, unknown> {
  const methods = new Map<string, string>();
  const responses = new Map<string, unknown>();
  for (const step of testCase.steps) {
    if (step.kind === "request") methods.set(step.id!, step.method!);
    if (step.kind === "response" && step.error === undefined) {
      responses.set(methods.get(step.response!)!, step.result);
    }
  }
  return responses;
}

function handlerProxy(responses: Map<string, unknown>): object {
  return new Proxy({}, {
    get(_target, property) {
      if (typeof property !== "string") return undefined;
      const method = property.replaceAll(/[A-Z]/g, (letter) => `.${letter.toLowerCase()}`);
      return async () => structuredClone(responses.get(method));
    },
  });
}

function roleAdapter(
  catalog: "babelforce" | "demo",
  role: "application" | "voice",
  responses: Map<string, unknown>,
  events: AsyncQueue<{ readonly event: string; readonly data: unknown }>,
): RoleAdapter {
  const proxy = handlerProxy(responses);
  let generated: RoleAdapter;
  if (catalog === "demo") {
    generated = role === "application"
      ? demoV1.applicationAdapter(
          proxy as demoV1.ApplicationHandler,
          proxy as demoV1.ApplicationEventHandler,
        )
      : demoV1.voiceAdapter(
          proxy as demoV1.VoiceHandler,
          proxy as demoV1.VoiceEventHandler,
        );
  } else {
    generated = role === "application"
      ? babelforceV1.applicationAdapter(
          proxy as babelforceV1.ApplicationHandler,
          proxy as babelforceV1.ApplicationEventHandler,
        )
      : babelforceV1.voiceAdapter(
          proxy as babelforceV1.VoiceHandler,
          proxy as babelforceV1.VoiceEventHandler,
        );
  }
  return {
    ...generated,
    events: generated.events.map((registration) => ({
      ...registration,
      async handle(context, data) {
        await registration.handle(context, data);
        await events.push({ event: registration.event, data });
      },
    })),
  };
}

async function receive(peer: MemoryTransport): Promise<ControlFrame> {
  const received = await peer.control.receive(AbortSignal.timeout(2_000));
  return classicV1.classicV1Envelope.decode(received.data);
}

async function send(peer: MemoryTransport, frame: ControlFrame): Promise<void> {
  await peer.control.send(classicV1.classicV1Envelope.encode(frame), AbortSignal.timeout(2_000));
}

async function runCase(
  scenario: Scenario,
  testCase: ScenarioCase,
  localName: string,
  localRole: "application" | "voice",
): Promise<void> {
  const [local, peer] = MemoryTransport.pair();
  const events = new AsyncQueue<{ readonly event: string; readonly data: unknown }>(64);
  const session = new Session({
    envelope: classicV1.classicV1Envelope,
    transport: local,
    requestTimeoutMs: 2_000,
    closeTimeoutMs: 2_000,
    handler: new Handler({
      adapter: roleAdapter(
        scenario.name === "echo-observed" ? "demo" : "babelforce",
        localRole,
        responseValues(testCase),
        events,
      ),
    }),
  });
  const done = session.run();
  await session.ready;
  const bindings = new Map<string, string>();
  const pending = new Map<string, Promise<unknown>>();

  for (const step of testCase.steps) {
    const localOrigin = step.from === localName;
    if (step.kind === "request") {
      if (localOrigin) {
        const request = session.request(step.method!, step.params as never);
        const frame = await receive(peer);
        assert.equal(frame.kind, "request");
        assert.equal(frame.method, step.method);
        assert.deepEqual(frame.params, step.params);
        bindings.set(step.id!, frame.id);
        pending.set(step.id!, request);
      } else {
        const id = `peer-${bindings.size + 1}`;
        bindings.set(step.id!, id);
        await send(peer, {
          kind: "request",
          id,
          method: step.method!,
          params: step.params as never,
        });
      }
      continue;
    }
    if (step.kind === "response") {
      const correlationId = bindings.get(step.response!)!;
      if (localOrigin) {
        const frame = await receive(peer);
        assert.equal(frame.kind, "response");
        assert.equal(frame.correlationId, correlationId);
        assert.deepEqual(frame.result, step.result);
        assert.deepEqual(frame.error, step.error);
      } else {
        await send(peer, {
          kind: "response",
          correlationId,
          ...(step.result === undefined ? {} : { result: step.result as never }),
          ...(step.error === undefined ? {} : { error: step.error as never }),
        });
        const outcome = pending.get(step.response!)!;
        if (step.error === undefined) assert.deepEqual(await outcome, step.result);
        else {
          await assert.rejects(
            outcome,
            (error: unknown) => error instanceof RemoteError
              && error.wire.code === step.error!.code
              && error.wire.message === step.error!.message,
          );
        }
        pending.delete(step.response!);
      }
      continue;
    }
    if (localOrigin) {
      await session.notify(step.event!, step.data as never);
      const frame = await receive(peer);
      assert.equal(frame.kind, "event");
      assert.equal(frame.event, step.event);
      assert.deepEqual(frame.data, step.data);
      bindings.set(step.id!, frame.id);
    } else {
      const id = `peer-${bindings.size + 1}`;
      bindings.set(step.id!, id);
      await send(peer, { kind: "event", id, event: step.event!, data: step.data as never });
      assert.deepEqual(await events.shift(AbortSignal.timeout(2_000)), {
        event: step.event,
        data: step.data,
      });
    }
  }
  assert.equal(pending.size, 0);
  if (session.state === "active") await session.close();
  await done;
}

for (const path of scenarioPaths) {
  const scenario = loadScenario(path);
  for (const [localName, localRole] of Object.entries(scenario.roles)) {
    for (const testCase of scenario.cases) {
      test(`${scenario.name}/${testCase.name}/local-${localRole}`, async () => {
        await runCase(scenario, testCase, localName, localRole);
      });
    }
  }
}

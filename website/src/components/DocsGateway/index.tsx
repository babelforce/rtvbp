import type {ReactNode} from "react";
import Link from "@docusaurus/Link";
import Heading from "@theme/Heading";

import styles from "./styles.module.css";

const buildPaths = [
  {
    marker: "TS",
    title: "TypeScript SDK",
    description: "One generated API for Node 22+ and evergreen browsers, including native WebRTC.",
    detail: "@babelforce/rtvbp · 0.1.0",
    to: "/docs/getting-started/typescript",
  },
  {
    marker: "GO",
    title: "Go SDK",
    description: "Build either protocol role with typed peers, WebSocket audio, or Pion WebRTC.",
    detail: "Go module · 0.1.1",
    to: "/docs/getting-started/go",
  },
  {
    marker: "RS",
    title: "Rust SDK",
    description: "Use the same generated contracts through Tokio, WebSocket, and WebRTC runtimes.",
    detail: "Rust crate source · 0.1.0",
    to: "/docs/getting-started/rust",
  },
  {
    marker: "{}",
    title: "Wire protocol",
    description: "Implement a peer directly from generated envelopes, roles, flows, and vectors.",
    detail: "Protocol snapshot · 1.0.0",
    to: "/docs/getting-started/protocol",
  },
] as const;

const understandPaths = [
  {
    title: "Core concepts",
    detail: "Roles, messages, media, and the three independent layers.",
    to: "/docs/concepts",
  },
  {
    title: "Profiles",
    detail: "How peers negotiate a compatible transport, envelope, and catalog.",
    to: "/docs/profiles",
  },
  {
    title: "Transport bindings",
    detail: "Choose WebSocket audio or WebRTC media without changing payloads.",
    to: "/docs/transports/websocket",
  },
] as const;

function Arrow(): ReactNode {
  return <span aria-hidden="true">↗</span>;
}

export default function DocsGateway(): ReactNode {
  return (
    <div className={styles.gateway} data-testid="docs-gateway">
      <header className={styles.hero}>
        <div className={styles.heroCopy}>
          <p className={styles.eyebrow}>Real-Time Voice Bridge Protocol</p>
          <Heading as="h1">The typed bridge between a live call and your application.</Heading>
          <p className={styles.lede}>
            Keep telephony in the voice platform. Give an AI agent, IVR, or audio service a small,
            transport-independent contract for control and duplex media.
          </p>
          <div className={styles.actions}>
            <Link className="button button--primary button--lg" to="/try">Try it out</Link>
            <Link className={styles.textLink} to="/docs/concepts">
              Learn the model <Arrow />
            </Link>
          </div>
          <ul className={styles.proof} aria-label="Protocol status">
            <li><strong>3</strong><span>released SDKs</span></li>
            <li><strong>48</strong><span>frozen wire fixtures</span></li>
            <li><strong>1 spec</strong><span>generates every contract</span></li>
          </ul>
        </div>

        <div className={styles.bridge} aria-label="Voice peer and application peer exchange typed control and duplex audio">
          <div className={styles.peer}>
            <span>Owns the call</span>
            <strong>Voice peer</strong>
            <small>Telephony</small>
          </div>
          <div className={styles.channels} aria-hidden="true">
            <span><i /> typed control <i /></span>
            <span><i /> duplex audio <i /></span>
          </div>
          <div className={styles.peer}>
            <span>Owns the logic</span>
            <strong>Application peer</strong>
            <small>Agent · IVR · service</small>
          </div>
          <div className={styles.profile}>
            <span>Current profiles</span>
            <code>rtvbp.v1</code>
            <code>rtvbp.webrtc.v1</code>
          </div>
        </div>
      </header>

      <section className={styles.section} aria-labelledby="build-boundary">
        <div className={styles.sectionHeading}>
          <div>
            <p className={styles.eyebrow}>Start here</p>
            <Heading id="build-boundary" as="h2">Build from your boundary.</Heading>
          </div>
          <p>Choose an SDK when you want a complete runtime, or the wire path when RTVBP meets another stack.</p>
        </div>
        <div className={styles.buildGrid}>
          {buildPaths.map((path) => (
            <Link className={styles.buildCard} to={path.to} key={path.title}>
              <span className={styles.marker}>{path.marker}</span>
              <Heading as="h3">{path.title}</Heading>
              <p>{path.description}</p>
              <small>{path.detail}</small>
              <strong>Open quickstart <Arrow /></strong>
            </Link>
          ))}
        </div>
      </section>

      <section className={styles.section} aria-labelledby="understand-protocol">
        <div className={styles.sectionHeading}>
          <div>
            <p className={styles.eyebrow}>Understand</p>
            <Heading id="understand-protocol" as="h2">A small model with hard guarantees.</Heading>
          </div>
          <p>The payload is invariant; transport and envelope are negotiated choices. Generated proof keeps them aligned.</p>
        </div>
        <div className={styles.understandGrid}>
          {understandPaths.map((path, index) => (
            <Link className={styles.understandCard} to={path.to} key={path.title}>
              <span>0{index + 1}</span>
              <div><Heading as="h3">{path.title}</Heading><p>{path.detail}</p></div>
              <Arrow />
            </Link>
          ))}
        </div>
      </section>

      <aside className={styles.referenceCallout} aria-label="Generated protocol reference">
        <div>
          <p className={styles.eyebrow}>Need the exact contract?</p>
          <Heading as="h2">Reference generated from the same spec as the SDKs.</Heading>
          <p>Inspect every role, operation, event, envelope, profile, and conformance flow without a hand-maintained gap.</p>
        </div>
        <div className={styles.referenceLinks}>
          <Link to="/docs/reference/babelforce.v1/roles/application">Application role <Arrow /></Link>
          <Link to="/docs/reference/babelforce.v1/roles/voice">Voice role <Arrow /></Link>
          <Link to="/docs/reference/babelforce.v1/flows/initialize-updated-dtmf">Proven flows <Arrow /></Link>
          <Link to="/docs/releases">Releases and verification <Arrow /></Link>
        </div>
      </aside>
    </div>
  );
}

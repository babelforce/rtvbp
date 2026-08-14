import type {ReactNode} from "react";
import Link from "@docusaurus/Link";
import useDocusaurusContext from "@docusaurus/useDocusaurusContext";
import Layout from "@theme/Layout";
import HomepageFeatures from "@site/src/components/HomepageFeatures";
import Heading from "@theme/Heading";

import styles from "./index.module.css";

const paths = [
  {
    label: "TypeScript SDK",
    title: "Connect from Node or a real browser.",
    detail: "Generated role contracts, AudioWorklet, WebSocket, and native WebRTC.",
    to: "/docs/getting-started/typescript",
  },
  {
    label: "Go SDK",
    title: "Build a voice or application peer in Go.",
    detail: "Generated role contracts, typed peers, WebSocket, and WebRTC.",
    to: "/docs/getting-started/go",
  },
  {
    label: "Rust SDK",
    title: "Build the same protocol surface in Rust.",
    detail: "Tokio runtime, generated catalog APIs, and transport parity.",
    to: "/docs/getting-started/rust",
  },
  {
    label: "Wire protocol",
    title: "Implement RTVBP directly from the contract.",
    detail: "Profiles, envelopes, media framing, and generated reference.",
    to: "/docs/getting-started/protocol",
  },
];

function HomepageHeader() {
  return (
    <header className={styles.heroBanner}>
      <div className={`container ${styles.heroGrid}`}>
        <div className={styles.heroCopy}>
          <p className={styles.eyebrow}>Real-Time Voice Bridge Protocol</p>
          <Heading as="h1">One voice protocol. Any application.</Heading>
          <p className={styles.heroSubtitle}>
            RTVBP connects the peer that owns a live call to the application
            that listens, speaks, and controls it—using typed operations and
            events alongside real-time audio.
          </p>
          <div className={styles.buttons}>
            <Link className="button button--primary button--lg" to="/try">
              Try it out
            </Link>
            <Link
              className={`button button--outline button--lg ${styles.heroSecondary}`}
              to="/docs/intro"
            >
              Read the docs
            </Link>
          </div>
          <div className={styles.peerLine} aria-label="Voice peer connects to application peer">
            <span>Voice peer</span>
            <span className={styles.connection} aria-hidden="true">
              <i />
            </span>
            <span>Application peer</span>
          </div>
        </div>

        <aside className={styles.profileCard} aria-label="Current deployed profile">
          <div className={styles.profileHeader}>
            <span className={styles.statusDot} />
            Deployed profile
          </div>
          <Heading as="h2">rtvbp.v1</Heading>
          <dl className={styles.profileLayers}>
            <div>
              <dt>Control</dt>
              <dd>WebSocket</dd>
            </div>
            <div>
              <dt>Media</dt>
              <dd>WebSocket or WebRTC</dd>
            </div>
            <div>
              <dt>Envelope</dt>
              <dd>classic.v1</dd>
            </div>
            <div>
              <dt>Catalog</dt>
              <dd>babelforce.v1</dd>
            </div>
          </dl>
          <p className={styles.profileFootnote}>
            Frozen wire compatibility, guarded by generated fixtures and
            cross-SDK tests.
          </p>
        </aside>
      </div>
    </header>
  );
}

function Quickstarts(): ReactNode {
  return (
    <section className={styles.quickstarts} aria-labelledby="choose-path">
      <div className="container">
        <div className={styles.sectionHeading}>
          <p className={styles.eyebrow}>Choose your path</p>
          <Heading id="choose-path" as="h2">
            Start at your integration boundary.
          </Heading>
        </div>
        <div className={styles.pathGrid}>
          {paths.map((path) => (
            <Link className={styles.pathCard} to={path.to} key={path.label}>
              <span>{path.label}</span>
              <Heading as="h3">{path.title}</Heading>
              <p>{path.detail}</p>
              <strong>Open quickstart <span aria-hidden="true">→</span></strong>
            </Link>
          ))}
        </div>
      </div>
    </section>
  );
}

function Steward(): ReactNode {
  return (
    <section className={styles.steward} aria-labelledby="stewarded-by">
      <div className={`container ${styles.stewardInner}`}>
        <div className={styles.stewardCopy}>
          <p className={styles.eyebrow}>Open protocol, active steward</p>
          <Heading id="stewarded-by" as="h2">
            Built from production voice infrastructure.
          </Heading>
          <p>
            RTVBP is stewarded by babelforce. The deployed protocol remains
            frozen where compatibility matters, while new SDKs, transports,
            and conformance proofs evolve in the open.
          </p>
          <Link to="https://github.com/babelforce/rtvbp">
            Explore the project on GitHub <span aria-hidden="true">→</span>
          </Link>
        </div>
        <Link
          className={styles.stewardLogo}
          to="https://www.babelforce.com/"
          aria-label="Visit babelforce.com"
        >
          <img
            className={styles.logoLight}
            src="img/babelforce-wordmark.svg"
            alt="babelforce"
            width="300"
            height="34"
          />
          <img
            className={styles.logoDark}
            src="img/babelforce-wordmark-white.svg"
            alt="babelforce"
            width="300"
            height="34"
          />
          <span>Voice, built for change.</span>
        </Link>
      </div>
    </section>
  );
}

export default function Home(): ReactNode {
  const {siteConfig} = useDocusaurusContext();
  return (
    <Layout
      title="Real-Time Voice Bridge Protocol"
      description={siteConfig.tagline}
    >
      <HomepageHeader />
      <main>
        <HomepageFeatures />
        <Quickstarts />
        <Steward />
      </main>
    </Layout>
  );
}

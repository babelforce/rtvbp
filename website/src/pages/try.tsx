import type {ReactNode} from "react";
import Layout from "@theme/Layout";
import Heading from "@theme/Heading";
import Link from "@docusaurus/Link";

import ProtocolLab from "@site/src/components/ProtocolLab";
import styles from "./try.module.css";

export default function TryRtvbp(): ReactNode {
  return (
    <Layout
      title="Try RTVBP in your browser"
      description="Make a safe simulated voice call and inspect RTVBP messages, audio, and WebRTC health in your browser."
    >
      <main>
        <header className={styles.hero}>
          <div className="container">
            <p className={styles.eyebrow}>Interactive protocol lab</p>
            <Heading as="h1">See a voice session happen.</Heading>
            <p className={styles.intro}>
              Make a deterministic call entirely inside this tab. Hear both media directions,
              press keys, barge in, and watch generated RTVBP frames travel between a phone and an
              application while the browser reports real WebRTC health.
            </p>
            <div className={styles.assurances} aria-label="Simulation properties">
              <span>No account</span>
              <span>No backend</span>
              <span>No microphone</span>
              <span>No telemetry</span>
            </div>
          </div>
        </header>

        <div className={`container ${styles.labWrap}`}>
          <ProtocolLab />
        </div>

        <section className={styles.explainer} aria-labelledby="what-is-real">
          <div className={`container ${styles.explainerGrid}`}>
            <div>
              <p className={styles.eyebrow}>What is real here?</p>
              <Heading id="what-is-real" as="h2">The SDK, frames, media, and statistics.</Heading>
              <p>
                Simulation mode runs two actual TypeScript SDK sessions, generated role adapters,
                typed peers, the generated <code>classic.v1</code> envelope, and a native local
                WebRTC peer pair. The phone UI is the only simulation layer.
              </p>
            </div>
            <div className={styles.explainerCard}>
              <Heading as="h3">Bring an endpoint when you are ready.</Heading>
              <p>
                Advanced live mode accepts a caller-supplied secure endpoint and optional runtime
                credentials. The site ships with neither and never persists what you enter.
              </p>
              <Link to="/docs/getting-started/typescript">
                Build the browser integration <span aria-hidden="true">→</span>
              </Link>
            </div>
          </div>
        </section>
      </main>
    </Layout>
  );
}

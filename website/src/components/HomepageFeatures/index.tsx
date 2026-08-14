import type {ReactNode} from "react";
import Heading from "@theme/Heading";
import styles from "./styles.module.css";

type FeatureItem = {
  marker: string;
  title: string;
  description: ReactNode;
};

const FeatureList: FeatureItem[] = [
  {
    marker: "01",
    title: "Typed by design",
    description: (
      <>
        Operations, events, roles, envelopes, SDK surfaces, and reference docs
        all derive from one executable specification.
      </>
    ),
  },
  {
    marker: "02",
    title: "Transport independent",
    description: (
      <>
        Keep the same call-control payloads while choosing WebSocket audio,
        WebRTC media, or a future binding.
      </>
    ),
  },
  {
    marker: "03",
    title: "Proven on the wire",
    description: (
      <>
        Frozen fixtures and cross-SDK conformance tests protect deployed{" "}
        <code>babelforce.v1</code> behavior byte for byte.
      </>
    ),
  },
];

function Feature({marker, title, description}: FeatureItem) {
  return (
    <article className={styles.feature}>
      <span className={styles.marker} aria-hidden="true">
        {marker}
      </span>
      <Heading as="h3">{title}</Heading>
      <p>{description}</p>
    </article>
  );
}

export default function HomepageFeatures(): ReactNode {
  return (
    <section className={styles.features} aria-labelledby="why-rtvbp">
      <div className="container">
        <div className={styles.sectionHeading}>
          <p className={styles.eyebrow}>Why RTVBP</p>
          <Heading id="why-rtvbp" as="h2">
            The payload is the invariant.
          </Heading>
          <p>
            Build real-time voice systems without coupling business logic to a
            single media path or hand-maintained protocol client.
          </p>
        </div>
        <div className={styles.grid}>
          {FeatureList.map((props) => (
            <Feature key={props.marker} {...props} />
          ))}
        </div>
      </div>
    </section>
  );
}

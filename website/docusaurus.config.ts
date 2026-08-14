import { themes as prismThemes } from "prism-react-renderer";
import type { Config } from "@docusaurus/types";
import type * as Preset from "@docusaurus/preset-classic";

// This runs in Node.js - Don't use client-side code here (browser APIs, JSX...)

const config: Config = {
  title: "RTVBP — Real-Time Voice Bridge Protocol",
  tagline: "The typed protocol between telephony and real-time applications",
  favicon: "img/babelforce-mark.svg",
  headTags: [
    {
      tagName: "meta",
      attributes: {
        name: "theme-color",
        content: "#181e32",
      },
    },
  ],
  stylesheets: [
    {
      href: "https://fonts.googleapis.com/css2?family=DM+Sans:wght@400;500;600;700&family=Spectral:wght@500;600;700&display=swap",
      type: "text/css",
    },
  ],

  // Future flags, see https://docusaurus.io/docs/api/docusaurus-config#future
  future: {
    v4: true, // Improve compatibility with the upcoming Docusaurus v4
  },

  // Set the production url of your site here
  url: "https://babelforce.github.io",
  // Set the /<baseUrl>/ pathname under which your site is served
  // For GitHub pages deployment, it is often '/<projectName>/'
  baseUrl: "/rtvbp/",

  // GitHub pages deployment config.
  // If you aren't using GitHub pages, you don't need these.
  organizationName: "babelforce", // Usually your GitHub org/user name.
  projectName: "rtvbp", // Usually your repo name.
  deploymentBranch: "main",

  onBrokenLinks: "throw",
  onBrokenMarkdownLinks: "throw",

  markdown: {
    mermaid: true,
  },
  themes: ["@docusaurus/theme-mermaid"],

  // Even if you don't use internationalization, you can use this field to set
  // useful metadata like html lang. For example, if your site is Chinese, you
  // may want to replace "en" with "zh-Hans".
  i18n: {
    defaultLocale: "en",
    locales: ["en"],
  },

  presets: [
    [
      "classic",
      {
        docs: {
          sidebarPath: "./sidebars.ts",
          editUrl: "https://github.com/babelforce/rtvbp/edit/main/website/",
        },
        theme: {
          customCss: "./src/css/custom.css",
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    image: "img/rtvbp-social-card.svg",
    colorMode: {
      defaultMode: "light",
      disableSwitch: false,
      respectPrefersColorScheme: true,
    },
    tableOfContents: {
      minHeadingLevel: 2,
      maxHeadingLevel: 3,
    },
    mermaid: {
      theme: { light: "neutral", dark: "forest" },
    },
    navbar: {
      title: "RTVBP",
      logo: {
        alt: "babelforce",
        src: "img/babelforce-mark.svg",
        width: 28,
        height: 28,
      },
      items: [
        {
          to: "/try",
          label: "Try it out",
          position: "left",
          className: "navbar__try",
        },
        {
          type: "docSidebar",
          sidebarId: "docsSidebar",
          position: "left",
          label: "Docs",
        },
        {
          type: "dropdown",
          label: "Build",
          position: "left",
          items: [
            {
              label: "TypeScript SDK",
              to: "/docs/getting-started/typescript",
            },
            {
              label: "Go SDK",
              to: "/docs/getting-started/go",
            },
            {
              label: "Rust SDK",
              to: "/docs/getting-started/rust",
            },
            {
              label: "Wire protocol",
              to: "/docs/getting-started/protocol",
            },
          ],
        },
        {
          to: "/docs/reference/babelforce.v1/roles/application",
          label: "Reference",
          position: "left",
        },
        {
          href: "https://www.babelforce.com/",
          label: "babelforce",
          position: "right",
        },
        {
          href: "https://github.com/babelforce/rtvbp",
          label: "GitHub",
          position: "right",
        },
      ],
    },
    footer: {
      style: "dark",
      logo: {
        alt: "babelforce",
        src: "img/babelforce-wordmark-white.svg",
        href: "https://www.babelforce.com/",
        width: 194,
        height: 22,
      },
      links: [
        {
          title: "Explore",
          items: [
            {
              label: "Try it out",
              to: "/try",
            },
            {
              label: "Documentation",
              to: "/docs/intro",
            },
            {
              label: "Core concepts",
              to: "/docs/concepts",
            },
            {
              label: "Profiles",
              to: "/docs/profiles",
            },
          ],
        },
        {
          title: "SDKs",
          items: [
            {
              label: "TypeScript",
              to: "/docs/getting-started/typescript",
            },
            {
              label: "Go",
              to: "/docs/getting-started/go",
            },
            {
              label: "Rust",
              to: "/docs/getting-started/rust",
            },
            {
              label: "Wire protocol",
              to: "/docs/getting-started/protocol",
            },
          ],
        },
        {
          title: "Reference",
          items: [
            {
              label: "Application role",
              to: "/docs/reference/babelforce.v1/roles/application",
            },
            {
              label: "Voice role",
              to: "/docs/reference/babelforce.v1/roles/voice",
            },
            {
              label: "Operations",
              to: "/docs/reference/babelforce.v1/operations/session.initialize",
            },
            {
              label: "Proven flows",
              to: "/docs/reference/babelforce.v1/flows/initialize-updated-dtmf",
            },
          ],
        },
        {
          title: "Project",
          items: [
            {
              label: "Source on GitHub",
              href: "https://github.com/babelforce/rtvbp",
            },
            {
              label: "Releases",
              to: "/docs/releases",
            },
            {
              label: "babelforce",
              href: "https://www.babelforce.com/",
            },
          ],
        },
      ],
      copyright: `RTVBP is an open protocol stewarded by babelforce GmbH. © ${new Date().getFullYear()} babelforce GmbH.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ["bash", "go", "rust", "toml"],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;

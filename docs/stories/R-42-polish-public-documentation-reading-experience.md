---
id: R-42
title: Polish the public documentation reading experience
pillar: Integration
status: done
priority: 42
design: docs/designs/public-docs-reading-experience.md
epic: docs-gen
areas: [website, documentation, accessibility, design]
note: make guides and generated reference feel as deliberate as the landing page without touching generated content
---

# Polish the public documentation reading experience

## Goal
Turn the public documentation itself—not the interactive phone—into a cohesive, current, and
comfortable integration workspace. The visual language should carry from the landing page through
guides, quickstarts, diagrams, and generated reference while preserving the generated boundary.

## Acceptance
- [x] Documentation navigation has an intentional audience-first hierarchy, clear labels, useful
      category landing pages, active states, and direct routes to the lab, SDKs, concepts, profiles,
      transports, generated reference, releases, and project source.
- [x] Guide and reference pages share polished typography, spacing, breadcrumbs, sidebars, table of
      contents, code blocks, tables, callouts, diagrams, pagination, edit links, and footer treatment
      in light and dark themes; generated reference files remain byte-untouched.
- [x] The documentation introduction is a useful gateway with a visible **Try it out** route,
      audience cards, a compact protocol mental model, current released SDK status, and a clear next
      step rather than a long undifferentiated article.
- [x] Hand-written public docs are reviewed for stale milestone/release language and navigational
      dead ends, without copying facts that belong to generated reference.
- [x] Responsive behavior keeps prose, navigation, diagrams, tables, and code usable at narrow phone
      widths; keyboard focus, skip navigation, contrast, reduced-motion behavior, and semantic
      landmarks remain explicit.
- [x] Failing-first real-browser tests cover the docs landing route, navigation hierarchy, guide and
      generated-reference presentation, dark mode, keyboard focus, table/code overflow, and mobile
      layout. Typecheck, static build, confidentiality gate, and the full repository gate pass.

## Notes

- The interactive `ProtocolLab` component and controller are deliberately out of scope.
- `website/docs/reference/` is generated and must not be edited. Its presentation may be improved
  through shared theme CSS and wrappers only.

## Progress

- 2026-08-14: Audited the published introduction, TypeScript quickstart, generated operation
  reference, generated flow, desktop sidebars, and mobile rendering. The content is sound but actual
  documentation pages still inherit a mostly stock Docusaurus hierarchy and reading treatment that
  does not match the deliberate public landing page.
- 2026-08-14: Replaced the article-like introduction with a responsive integration gateway; added a
  manual Start/Build/Understand/Reference/Project hierarchy; aligned guide, category, and generated
  reference presentation across light and dark themes; and corrected the public TypeScript release
  and roadmap status. Generated reference files remain unchanged. The failing-first Chrome test now
  covers gateway/navigation, a long quickstart, generated tables, focus, theme switching, and mobile
  overflow, while the existing interactive-lab acceptance stays green.

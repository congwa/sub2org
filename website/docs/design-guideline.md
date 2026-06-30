# sub2org Website Design Guideline

## Inspiration Source

- Dribbble search: `saas dashboard landing page`
- Captured reference screenshot: `/tmp/sub2org-aesthetic-dribbble/dribbble-search-saas-dashboard.png`
- Downloaded reference thumbnails: `/tmp/sub2org-aesthetic-dribbble/shot-02.webp` through `shot-08.webp`

## Extracted Patterns

- SaaS landing pages lead with a visible product surface, not pure marketing copy.
- High-performing shots use a strong hero contrast block, then quieter white content sections.
- Dashboard cards feel more credible when they include system cues: rails, status strips, small labels, and compact density.
- Accent color should identify state and routing, while typography and spacing keep the page calm.
- Motion should be small and functional: hover lift, border emphasis, and subtle signal-line movement.

## sub2org Direction

The visual theme is a browser-worker relay switchboard: dark control surface, signal rails, compact state cards, and operational panels. Copy stays unchanged; visual hierarchy should make the relay architecture feel tangible before the visitor reads every paragraph.

## Implementation Rules

- Use Tailwind CSS utilities for responsive layout.
- Keep global CSS limited to theme tokens and pseudo-element textures.
- Preserve mobile-first layouts with one column by default and richer switchboard visuals on desktop.
- Maintain 44px minimum interactive targets and visible focus states.
- Avoid generic SaaS decoration; every visual detail should suggest routing, workers, streaming, or operations.

# 0001: Application stack and repository layout

Status: accepted (Stage 0)

## Decision

- Tauri 2 for desktop and mobile packaging, with a Rust core.
- Plain Svelte 5 with Vite for the UI, not SvelteKit: Nimata has no URL
  routing needs, and navigation is a small state store that also drives the
  single-column phone layout.
- Tailwind CSS 4 with a handful of CSS-variable color tokens for light and
  dark mode. No component library or theming system.
- A Cargo workspace: `crates/nimata-core` (no Tauri dependency) and
  `src-tauri` (thin shell).
- Toolchains pinned with mise. npm as the package manager.
- TypeScript is pinned to 6.x because svelte-check does not yet support
  TypeScript 7.

## Consequences

- Core logic is testable with plain `cargo test` and reusable by a future
  secondary client.
- Frontend runtime dependencies are limited to `svelte` and
  `@tauri-apps/api`.
- Fonts are the platform's own serif and sans-serif families, so nothing is
  downloaded and the app works offline.

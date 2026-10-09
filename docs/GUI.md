# GUI

The desktop frontend lives in `gui/` and uses Tauri 2 with a deliberately small vanilla TypeScript/CSS surface. It is a thin view over `nero-core`.

## Development

From `gui/`:

```bash
pnpm install
NERO_WORKSPACE=~/notes pnpm tauri dev
```

The workspace can also be discovered from the current working directory when `.nero/` exists.

## Responsibilities

The Rust side owns:

- workspace discovery
- document parsing
- Markdown HTML rendering
- search
- wiki-link resolution
- filesystem operations
- image import and safe local image reads
- file watching

The TypeScript side owns presentation and interaction:

- note browser
- search/command palette
- editor and split preview
- typography
- KaTeX rendering
- keyboard/mouse behavior

The GUI deliberately has no second document format or frontend-side Markdown parser. Split preview asks the same Rust/Comrak renderer used by normal reading mode.

## Assets

Imported images are copied into `assets/` and referenced with normal Markdown. The Rust core returns data URLs for local image reads so the WebView does not need broad filesystem URL permissions.

## Build

```bash
npm ci
npx tsc --noEmit
npm run build
```

Tauri packaging is covered by the GitHub release workflow. See [CI and releases](CI.md).

## Application icons

Tauri's compile-time context expects `gui/src-tauri/icons/icon.png` to exist even when running the development app. Keep the PNG, standard-size PNGs, `.ico`, and `.icns` in version control so development builds and Windows/macOS/Linux bundles do not depend on a locally generated asset. `nero-icon.svg` is the editable vector source for the artwork.

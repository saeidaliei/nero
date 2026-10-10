# GUI

The desktop frontend lives in `gui/` and uses Tauri 2 with a deliberately small vanilla TypeScript/CSS surface. It is a thin view over `nero-core`.

## Development

From `gui/`:

```bash
npm install --include=dev
NERO_WORKSPACE="$HOME/notes" npm run tauri:dev
```

For normal use, run `nero gui` from any directory after configuring a default workspace with `nero workspace create personal` (or `nero workspace use <name>`); Nero passes the selected workspace to the GUI. `NERO_WORKSPACE` can override the selected workspace for scripts. When launching the development app directly, set `NERO_WORKSPACE` as shown above. See [Workspace configuration](WORKSPACES.md).

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
npm install --include=dev
npx tsc --noEmit
npm run build
```

Tauri packaging is covered by the GitHub release workflow. See [CI and releases](CI.md).

## Application icons

Tauri's compile-time context expects `gui/src-tauri/icons/icon.png` to exist even when running the development app. Keep the PNG, standard-size PNGs, `.ico`, and `.icns` in version control so development builds and Windows/macOS/Linux bundles do not depend on a locally generated asset. `nero-icon.svg` is the editable vector source for the artwork.

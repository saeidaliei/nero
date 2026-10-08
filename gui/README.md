# Nero GUI

The first desktop frontend for Nero, built with Tauri 2 + vanilla TypeScript.

It deliberately has no frontend framework. The GUI is a thin view over `nero-core`: Markdown stays on disk, the Rust core owns parsing/search/link resolution, and the webview is responsible for typography and interaction.

## Development

From this directory:

```bash
pnpm install
NERO_WORKSPACE=~/notes pnpm tauri dev
```

The workspace can also be the current working directory when `.nero/` is present.

The desktop frontend uses Comrak's annotated math spans and KaTeX for typesetting. Comrak emits `data-math-style="inline|display"`, which the frontend passes directly to KaTeX. See the project root's `DESIGN.md` for the broader architecture.

## Current scope

- note browser
- Markdown reading
- search palette
- backlinks/context
- create note
- today's note
- live reload when Markdown changes
- KaTeX rendering
- embedded Markdown editor
- split source/preview mode
- syntax-aware highlighting
- smart Markdown list/task continuation
- native image import into `assets/`
- local workspace-safe image rendering

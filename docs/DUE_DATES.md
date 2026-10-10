# Due dates

Due dates are plain Markdown frontmatter. The date is read directly from the Markdown file; no separate task database is needed.

## Add a due date

Add `due: YYYY-MM-DD` to a note's frontmatter:

```markdown
---
title: Send the draft
due: 2026-10-12
---

# Send the draft

- [ ] Send the draft to the editor
```

`nero today` creates/prints the daily note path, then lists notes whose due date is today or earlier. Dates before today are marked `overdue`; future dates are not shown yet. This makes a dated note or todo note surface when it becomes actionable without requiring Nero to remain running.

Dates use the simple ISO format `YYYY-MM-DD`. Invalid `due` values are ignored by the due list so a typo cannot block daily-note access. `nero doctor` remains the place to check workspace health.

When a dated note/todo is complete or no longer relevant, edit the note and remove/change its `due` field. Markdown remains the source of truth; the index does not store separate task state.


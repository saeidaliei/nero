# Reminders (planned)

Nero does not yet deliver scheduled notifications. This document records a deliberately small design so the feature can be implemented without turning Nero into a task-management platform.

## Proposed model

A reminder should remain a normal Markdown note, with optional frontmatter metadata rather than a new database format:

```markdown
---
title: Send the draft
kind: reminder
remind_at: 2026-10-12T09:00:00+02:00
reminder_state: pending
---

# Send the draft

- [ ] Send the draft to the editor
```

The timestamp includes an offset so a reminder keeps a clear instant across daylight-saving changes and machine moves. A reminder is still a note and can be edited by any text editor. The SQLite index remains disposable.

## Proposed first implementation

1. `nero remind add <title> --at <RFC3339 timestamp>` creates a Markdown reminder note.
2. `nero remind list` lists pending, notified, completed, and cancelled reminders.
3. `nero remind daemon` checks due reminders and sends local desktop notifications.
4. A user-level service installer can start the daemon at login on supported platforms, with manual instructions if automatic installation is not appropriate.
5. `nero remind done <note>` and `nero remind cancel <note>` update the Markdown frontmatter.

The daemon must mark notifications atomically and avoid duplicate delivery after restarts as far as the operating system allows. It must explain how to keep reminders running; a notification cannot be delivered if neither Nero nor its service is running.

## Notification providers

**Desktop notifications should come first.** They require no Nero account and keep task details on the user's machine. Start with the platform's notification mechanism, show a small title and summary, and avoid including full private note contents by default.

Telegram or another external messenger should be an optional adapter later, not a requirement. It needs explicit opt-in, a bot token stored in the user's OS credential store or a private config outside workspaces, delivery/retry semantics, and clear disclosure that reminder text leaves the machine. Never put bot tokens in Markdown notes or the workspace backup manifest.

## Non-goals

The first reminder release should not add recurring rules, shared task boards, calendars, or a new task database. Start with one-shot reminders attached to ordinary Markdown notes; expand only after real use shows a need.

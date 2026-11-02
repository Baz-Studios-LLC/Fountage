# Fountage goal — revised 2026-10-14

Bring Fountage by Baz Studios to verified Highland Pro feature parity, targeting macOS, Windows and Linux with a shared Rust engine and an evaluated Slint interface. Keep the public release at v001 until the user explicitly authorizes a version change.

## Immediate phase: validate Slint

Build an isolated Slint prototype alongside the existing SwiftUI/AppKit app. Reuse the actual Rust parsing and analysis engine, writing categories and source semantics. Evaluate a beautiful, platform-appropriate interface in light and dark modes, optional white paper, screenplay indentation and typography, long-manuscript responsiveness, Unicode and bidirectional text, input methods, selection, clipboard, undo/redo, keyboard navigation and accessibility. Measure UI behavior separately from engine timings.

Do not treat a plain text input plus a rendered preview as a finished screenplay editor. Test the hardest editor requirements before committing to a full migration. Document missing framework capabilities and the cost of custom editor work. Preserve the existing Mac app as a working reference and fallback until the replacement passes acceptance gates.

## Migration and parity

After the prototype demonstrates viability, move reusable document behavior and workflows into the Rust core and implement the Slint interface incrementally. Preserve all eight writing categories, familiar comic-script conventions, approved branding, metadata and attachments, save/reopen compatibility, undo behavior and reliability. Existing SwiftUI/AppKit code may be replaced incrementally under this revised direction; it is no longer a permanent platform constraint.

Use docs/HIGHLAND_PARITY.md as the living checklist, verifying requirements against Highland's current official documentation and installed app. Complete partially implemented workflows before marking new areas complete. Cover writing/formatting, navigation, cast/location tools, goals, notes/comments, Shelf/snippets, sprints, templates, title/custom pages, pagination, preview/printing/PDF, revisions, analysis, autocomplete, import/export, Highland compatibility, review sharing, backups and conflict recovery. Treat MCP as an additional Fountage requirement, tracked separately from Highland parity.

Verify macOS, Windows and Linux independently: compilation alone is not runtime verification. Check packaging, file associations, menus/shortcuts, fonts, dialogs, clipboard, drag/drop, accessibility, input methods, printing and persistent document compatibility on each target. Report unavailable platform coverage explicitly.

Make routine decisions autonomously. Preserve manuscripts, unsaved work, unknown metadata and all uncommitted changes. Use isolated QA fixtures. Never force-quit the user's active app. Do not publish, deploy, purchase or message others. Record implementation, tests, UI evidence and remaining limitations in project documentation. Declare completion only after all applicable parity and platform acceptance gates pass, with no known data-loss defects.

## Goal control note

This file is the revised working objective requested by the user. The Codex goal API available in this session exposes status changes only, so this document does not claim that the existing goal's stored objective text was rewritten. The recurring heartbeat remains paused unless explicitly resumed.

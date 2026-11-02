# Screenplay navigation in v001

The screenplay workspace now offers Outline and Notes in its native sidebar. Other document types keep their existing format-specific outlines.

## Behavior

- Outline includes scenes, nested sections, synopses and forced page breaks in source order. Indentation reflects section depth. Only scene headings contribute to the scene count.
- Groups fold with disclosure chevrons or Left/Right when selected; the outline options menu expands or collapses every group. Nested groups retain their independent folded states. This hierarchy also supports other writing types, including comic pages/panels and academic headings.
- Outline search reveals matching headings with their ancestors, regardless of folds. Clearing search restores the folded view. Folding is disabled during search so hidden state does not change unexpectedly.
- Selecting an outline item keeps keyboard focus in the list for continued navigation. Return places focus in the manuscript at that heading. Folding hides navigator rows only: it does not alter source text or move the editor selection.
- Right-click a heading and choose Move Up or Move Down to swap it with the adjacent heading of the same kind and parent. The complete scene or section moves, including its body, notes and nested headings. Moves use one native Undo step. Reordering is disabled during search; synopses and cross-parent moves are not offered. Comic pages/panels use the same hierarchy rules.
- Notes lists bracketed script notes, including inline notes, multiple notes on one line and multiline notes. Each item shows its source line. Searching filters note contents.
- Selecting a note scrolls to and selects its exact source range. Selecting an outline item moves to its source line. Editing clears stale sidebar selection without moving the writing cursor.
- Notes receive subtle highlighting in the manuscript. Omitted text is subdued and excluded from the navigator and cast. It remains visible and editable in the source.
- Counts, outline and editor roles share one Rust parser. The older outline ABI derives its scene-only result from the same analysis.
- Centered action and lyric lines now receive distinct display styles; forced action, page breaks and transition boundaries are distinguished from character cues.

Notes use transient UTF-16 source positions so AppKit selections stay accurate around emoji, accented text and CRLF line endings. These positions are recalculated on edits, not saved as durable comment anchors. All note text remains in the manuscript and survives ordinary project/Fountain saving. The project metadata schema and app release version have not changed.

## Limits

This implements navigation and source-editor behavior, not complete Fountain layout or Highland parity. Note resolution, collaboration, production pagination, full inline-emphasis conformance and side-by-side dual-dialogue output remain unfinished. Screenplay draft totals count whitespace-delimited tokens containing letters/numbers in printed elements (including headings and character cues), excluding title metadata, sections, synopses, notes, omitted text, scene numbers and syntax-only tokens. Selected-passage counts use the same source-positioned words from the full document, so selecting inside a note or omission returns zero. A partially selected counted word counts once; overlapping selections do not double-count it. The parser is permissive about the blank line after a scene heading; it is not a strict syntax validator.

Folds are local to the open window and are not saved in the project. Ordinary body edits retain folds while source positions update. Structural edits (renaming, adding, removing, reordering or changing the depth of headings) reopen groups to avoid applying stale folded states to a different structure. Indistinguishable duplicate headings use their occurrence in source order for identity.

The syntax reference is [Fountain's official specification](https://fountain.io/syntax/), checked October 14, 2026. `examples/The Last Signal.fountain` is an original example with nested sections, synopses, four notes and an omitted scene.

## Verification

Automated tests cover note ranges around Unicode and CRLF, two notes on one line, multiline closing brackets, malformed notes, omissions, hierarchy, forced elements, transition spacing, continued dialogue, style cleanup, project saving and stale selection bounds.

Outline tests additionally cover independent nested folds, search through folded groups, duplicate titles, structural changes, shifted source positions around Unicode, comic hierarchy and skipped heading levels. In a separate QA app, Left/Right folding, search restoration and Return-to-editor focus were verified with a disposable project; the source and word count stayed unchanged.

In the running app, verified the example's three scenes and three cast members, four notes, exact selection of the second note on a line, live note updates, native undo, search filtering and the empty-search state. Visual inspection covered the dark appearance and, after adding the independent paper preference, the Notes sidebar in light appearance. Exact note selection and semantic styling were also checked with white paper inside the dark workspace and when switching back to Match App.

Inline bold, italics, underline and combined styles now render in the screenplay source editor while retaining all source markers. Escaped markers and line boundaries are respected; full delimiter-edge-case and output conformance remain open.

Format → Bold/Italic/Underline (Command-B/I/U) edits Fountain markers with native undo in screenplay manuscripts. Multiline passages are marked per line. Formatted structural lines retain their roles and source positions. Mixed-style toggling and the complete delimiter corpus remain part of the unfinished formatting audit.

Format → Insert Page Break (Command-Return) inserts a Fountain `===` marker with surrounding blank lines and native undo. Page breaks appear under their containing scene/section, are searchable, and Return navigates to the original source line. They move with their containing scene; independent break reordering is disabled. This does not yet implement production pagination.

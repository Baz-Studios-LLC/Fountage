# Writing formats and shareable templates

Fountage by Baz Studios · product decisions, October 14, 2026

## Implemented draft foundation

The Mac app stores a document type separately from its layout preset. Changing either keeps the manuscript text intact. Native paragraph attributes provide indentation; spaces are not inserted into the source. Editor typography is a writing view, not a print proof. Pagination, print page dimensions, page breaks, headers and final PDF/DOCX exports remain separate work.

| Document type | Draft behavior | Built-in layouts |
| --- | --- | --- |
| Screenplay | Fountain scene/character index, dialogue and parenthetical indents | Screenplay |
| Comic Script | PAGE/PANEL structure, uppercase character cues ending in a colon, distinct CAPTION/CAP and SFX labels | Fountage Standard, Lettering Friendly |
| Book | Chapter headings, `#` headings, scene breaks, prose paragraph indentation | Manuscript, Reading Draft |
| Academic Paper | `#` through `######` section headings and prose drafting | Academic Draft |
| Stage Play | Act/scene navigation, character cues and parenthesized directions | Stage Draft |
| Audio Script | Scene/segment navigation, speakers separated from technical cues | Audio Drama, Podcast |
| General Writing | Section headings and plain prose | Free Writing, Outline |
| Poetry & Lyrics | Stanza counts, poem titles and lyric sections; original spacing preserved | Poetry, Song Lyrics |

The user has approved an original house format for comics. **Fountage Standard** is the default: a clear, consistent starting point for new writers and collaborators. See `COMIC_FORMAT.md` for the writing conventions. The lettering-friendly option keeps cues flush left. A DC-specific preset requires a concrete, dated source supplied by a publisher or creator; do not label an invented style as official DC compliance.

The screenplay parser remains partial. Full Fountain behavior, reliable production pagination, dual dialogue and Highland parity remain release requirements.

## Academic standards must be versioned

Persist the style family, edition and variant with every paper. Do not silently change them after an app or template update. The current build records APA 7 student/professional or MLA 9 as a requested target; it does not format citations, produce a compliant title page or claim submission readiness.

Production support must include title-page metadata, headings, running headers/page numbering as appropriate, quotations, tables/figures, in-text citations or notes, and references/works cited. Bibliographic records and citation anchors should be structured, not inferred only from the rendered text. Student and professional APA templates must be distinct. Chicago needs an explicit edition plus notes/bibliography versus author-date. Instructor and university overrides should be explicit, named and saved separately from the base standard.

Required tests include official sample-paper fixtures, edition-specific citation cases, layout comparison, and save/reopen stability. A standard becomes “supported” only after the relevant rules and output checks are implemented. Unsupported imported editions must remain visible and preserved.

## Shareable templates: planned, not yet implemented

The user-facing concept is Templates. A template combines a base document type, semantic elements, page and paragraph rules, optional starter content and optional academic standard. Users should be able to create, duplicate, edit, preview, export and import their own templates. A niche format downloaded online can import directly when distributed as a compatible Fountage template.

Proposed `.fountagetemplate` package:

- Manifest schema version, stable template ID, display name, author, template version, description, source attribution/license and minimum compatible Fountage version.
- Base document type and named semantic roles. First release exposes supported roles and declarative structure rules; arbitrary new grammars require a separately designed extension interface.
- Page size, margins, font choices/fallbacks, paragraph indents and spacing, heading/numbering rules and explicit page-break behavior.
- Optional standard family/edition/variant and clearly identified local overrides.
- Optional starter text and local preview assets. A template export must not include the user's manuscript, notes, sources or project assets unless explicitly selected as starter content.
- Separate light/dark preview colors for on-screen accents. Editor colors should prefer semantic roles; print styling stays independent and readable.

Documents embed a snapshot of the selected template and its version. Updating a library template does not change existing documents. Offer an explicit update with a preview and preserve the original snapshot. Removing a library template must not break existing projects.

Import first validates schema, compatibility, size and resource paths, then previews name/author/version and sample output. Reject unsupported executable content and path traversal; templates are data, not native plugins. Unknown required capabilities produce a clear compatibility message rather than silently changing the output. Handle duplicate IDs/versions explicitly. Export/reimport round trips must preserve supported rules.

A PDF, Word document or webpage is a reference, not automatically a machine-readable template. A future assisted workflow can extract candidate rules, identify missing or ambiguous details, show a sample, and let the user review/save the resulting template. Source URL and publication date should travel with it. Do not promise universal one-click conversion or redistribute source material without permission.

A community library and uploads are later product work: author attribution, licensing, previews, version history, compatibility labels, reporting and moderation. Executable plugins require a separate permission/sandbox and compatibility design; they are not needed for the first template release. No community uploads or template installation are implemented yet.

## Appearance

System follows macOS, with explicit Light and Dark overrides. Use native semantic surfaces and text colors, plus a darker forest accent in Light and a lighter sage accent in Dark. Apply appearance to AppKit as well as SwiftUI, including the editor, dialogs and settings. Check text, insertion caret, selected text, sidebar selection, search, inspector, disabled controls and focus mode at realistic window sizes. Print output must not inherit dark editor colors.

## Primary sources consulted

- [Fountain syntax](https://fountain.io/syntax/) — screenplay source rules.
- [Dark Horse comic script guide](https://images.darkhorse.com/darkhorse08/company/submissions/scriptguide.pdf) — page/panel, lettering and flush-left conventions.
- [Jim Zub on writing comics](https://www.jimzub.com/rambling-about-how-i-write-comics-part-one/) — full-script practice and variation.
- [J. M. DeMatteis on comic scripting](https://www.jmdematteis.com/2013/11/how-do-you-do-that.html) — a creator's DC experience; not evidence of a universal current DC standard.
- [APA paper format](https://apastyle.apa.org/style-grammar-guidelines/paper-format/) and [sample papers](https://apastyle.apa.org/style-grammar-guidelines/paper-format/sample-papers) — seventh edition, student and professional paper distinctions.
- [MLA Handbook, ninth edition](https://style.mla.org/mla-handbook-ninth-edition/) and [citation formats](https://style.mla.org/works-cited/citations-by-format/) — current handbook and core citation elements.
- [Chicago citation guide](https://www.chicagomanualofstyle.org/tools_citationguide.aspx) — two distinct citation systems.

These references inform requirements. Their presence does not establish implemented conformity.

## Verification of this development build

October 14, 2026: 15 Rust tests and 14 Swift tests pass; Rust lint passes. In the running Mac app, checked screenplay dialogue indents, scene/section navigation, comic page/panel/cast counts, System/Light/Dark switching, editor and inspector contrast, sidebar/text selection, and native undo/redo. A Unicode academic draft was created through the UI, saved, inspected on disk, closed and reopened with its MLA 9 target intact. Source content matched exactly; formatting was not inserted into the manuscript. These checks do not validate print output or standards compliance.


## Eight-category chooser

The chooser offers Screenplay, Comic Script, Book, Academic Paper, Stage Play, Audio Script, General Writing, and Poetry & Lyrics. Each selects an editing layout and optional starter. Wider windows show four columns; smaller windows show two columns and allow scrolling. Book retains the serialized format ID `novel` for existing projects. New types use `stageplay`, `audio`, `general`, and `poetry`.

Poetry & Lyrics preserves source line breaks, stanza spacing and indentation, with separate Poetry and Song Lyrics starters. It counts nonempty stanza blocks and indexes # titles and familiar lyric labels. The running app's eight-category chooser was visually checked in Light and Dark; creating Song Lyrics populated four lyric headings and four stanzas. Stage and audio support the documented starter conventions; they are draft modes, not certified publisher or broadcast layouts. Production export and extended grammars remain future work.


Additional primary references: [Dramatists Guild suggested script formats](https://www.dramatistsguild.com/script-formats) and the [BBC radio-drama example](https://downloads.bbc.co.uk/writersroom/scripts/bbcradioscene.pdf). These inform the distinction between acts/scenes, dialogue, stage directions and audio cues; the current draft modes do not claim full compliance with either source's export specifications.

# Fountage by Baz Studios

Product domain: **fountage.com** (registered by the owner). Website work is separate from the application.

A writing studio with a shared Rust engine and a Slint interface targeting macOS, Windows and Linux. The original SwiftUI/AppKit app remains the Mac reference and fallback during migration. Exceptional visual quality and verified Highland Pro feature parity remain the goal, not a claim about this early preview.

## Downloads

Download the **v001 development prerelease** for macOS (Apple Silicon or Intel) or Windows x64 from [GitHub Releases](https://github.com/Baz-Studios-LLC/Fountage/releases). See [preview notes](docs/PREVIEW_RELEASE.md) for installation and current limitations. Linux packaging is not included in this first release.

## Build the shared Slint app

Install the current stable Rust toolchain, then run:

```sh
cargo run --locked --manifest-path prototypes/slint/Cargo.toml
```

On macOS, `Launch Slint Lab.command` builds an isolated app bundle. Release packaging is defined in `.github/workflows/release.yml` and `scripts/package-release.py`; builds use the approved icon and retain v001.

## Release version

The current release is **v001**. It stays at v001 throughout early development; only an explicit instruction from the user authorizes a version change. Future authorized labels follow v002, v003, and so on. `VERSION` supplies the public label and numeric macOS bundle metadata; builds never increment it. Rust's package version is internal engine metadata.

## Build the Swift Mac reference app

Double-click **Launch Fountage.command** in the project root to build and open the app. If Fountage is already running, quit it normally first to load the latest build; the launcher never force-quits an open manuscript. Build errors remain visible in Terminal.

```sh
scripts/run-macos.sh
```

Then open `target/Fountage.app` in Finder. Requires Rust and Xcode 26 or newer with its Command Line Tools configured. The local development bundle is not a signed/notarized distribution. The build compiles `macOS/Resources/Fountage.icon` into the app icon (including appearance variants), generates an older-macOS fallback, and registers the matching `.fountage` project-file icon. An already-running app picks up the new icon on its next launch.

## Swift reference features

- Native document windows and menus through Apple's document system. New documents begin with a type/layout chooser and optional starting outline; existing documents open directly.
- Primary `.fountage` project packages with versioned metadata and assets; direct Fountain editing remains supported. Unknown project data is retained across saves.
- Native NSTextView editing, selection, clipboard, find, spelling and undo support.
- Select a passage to see its word count alongside the total in the footer; clearing the selection restores the total-only display. Screenplay selections and draft totals exclude nonprinting material and syntax-only tokens; other formats still use source counts.
- Editor text size is remembered across documents and launches. The Text Size menu shows the current point size and offers a reset to 16 pt; this is a display preference, not manuscript formatting.
- Format-aware Rust analysis: screenplay scenes and character cues, comic pages/panels/lettering, novel chapters/scene breaks, academic/general section headings, stage acts/scenes, audio segments/speakers, and poetry stanzas/lyric sections.
- Screenplay Outline and Notes sidebar: nested sections, synopses, note search and exact source selection. Outline groups can fold individually or all at once; searching reveals matches with their parents. Left/Right folds the selected group; Return resumes writing. The same navigation works for other writing types, including comic pages/panels. Omitted text stays out of navigation and cast counts.
- Manage Cast & Locations in the inspector saves named reference notes in any Fountage project. Add details for detected names or create manual entries; name edits do not rewrite the manuscript. Search names and note contents, then use arrow keys and Return to open a reference.
- Screenplay Locations inspector groups scene locations and offers a menu to jump to each occurrence. Common time-of-day suffixes and scene numbers are removed from derived labels; unknown suffixes are retained.
- Display-only screenplay dialogue indentation and dedicated draft layouts for all eight writing types. Text remains editable plain source.
- Saved document type, layout, and academic style/edition targets (APA 7 student/professional and MLA 9). Academic citations and standards-compliant output are not implemented.
- Resizable Mac sidebar and inspector, System/Light/Dark appearance choices, focus mode and text sizing. An independent Paper preference offers Match App or White Paper, including a white writing surface inside the dark workspace. Find it in Settings → Appearance or View → Appearance → Paper.
- Optional Typewriter Scrolling keeps the insertion line centered, including the beginning and end of a draft. Manual scrolling stays free for reading. Enable it in View (⌃⌘T) or Settings → Writing; the preference persists across documents and launches and works with all eight writing types and Focus mode.
- Project-wide draft word goals in the inspector: set, edit or remove a target, follow live progress and see when it is reached. Goals persist in `.fountage` projects across all eight writing types. Screenplay goals use manuscript counts excluding nonprinting material; other formats still use source counts. Plain-text files do not store goals.
- Original sample projects in `examples/` for the initial four document types; new modes include built-in starting outlines.

These are early implementations, not a Highland parity claim. Highland import, complete screenplay formatting, professional pagination/PDF export, revisions, and the other features in `docs/HIGHLAND_PARITY.md` are still to be implemented and verified. The original seven-page design PDF remains unavailable.

## Structure

- `macOS/Sources/Fountage`: SwiftUI document application and native AppKit text editor.
- `macOS/Sources/Fountage/ManuscriptAnalysisCache.swift`: one exact-source analysis snapshot shared by navigation and styling; Unicode byte changes invalidate source ranges.
- `macOS/Sources/FountageCore`: C ABI declarations for the Rust engine.
- `src/fountain.rs`: shared source-preserving Fountain analysis for navigation, notes, cast and editor roles.
- `src/document.rs`: legacy scene-only outline adapter over the shared parser.
- `src/writing.rs`: format-aware structure and editor element roles.
- `src/lib.rs`: ownership-safe analysis bridge. Swift decodes JSON and frees Rust allocations.
- `src/storage.rs`: atomic saving utility, tested independently. Native document saving is managed by Apple's document framework.
- `src/main.rs`: archived Iced prototype, enabled only with `--features ice-prototype --bin fountage-prototype`.
- `docs/FILE_FORMAT.md`: documented primary container and researched interoperability requirements.
- `docs/FORMATS_AND_TEMPLATES.md`: format research, academic standards, and planned shareable templates.
- `docs/HIGHLAND_PARITY.md`: feature inventory, evidence and release acceptance gates.
- `docs/FOUNTAIN_NAVIGATION.md`: screenplay navigation behavior, verification and current limits.
- `docs/DEVELOPMENT_LOG.md`: completed improvements and verification notes for continued development.

## Checks

```sh
cargo fmt --check
cargo test --lib
cargo clippy --lib -- -D warnings
swift test --package-path macOS
```

Build the Rust library with `cargo build --release --lib` before compiling Swift. Screenplay draft counts exclude title metadata, outline sections, synopses, notes, omissions, scene numbers and syntax-only tokens. Screenplay selections use the same counted word positions; other formats still use source counts. Character identification remains a heuristic pending the complete parser. Document lifecycle and visual QA are ongoing; keep backups of important scripts during development.

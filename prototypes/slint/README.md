# Fountage Slint Lab — v001

Isolated feasibility prototype, not a replacement for the SwiftUI/AppKit application. Shares the existing Rust core through a path dependency; its own manifest and lockfile leave the production dependency graph unchanged.

Run from the repository root:

```sh
cargo run --manifest-path prototypes/slint/Cargo.toml
# Optional UTF-8 source file, loaded into a scratch buffer:
cargo run --manifest-path prototypes/slint/Cargo.toml -- examples/story.fountain
cargo run --manifest-path prototypes/slint/Cargo.toml -- --benchmark
cargo test --manifest-path prototypes/slint/Cargo.toml
cargo clippy --manifest-path prototypes/slint/Cargo.toml --all-targets -- -D warnings
```

`Launch Slint Lab.command` builds a separate Mac app bundle and opens it. It refuses to replace a running Slint Lab session: save scratch work and quit the lab normally first. `scripts/build-slint-lab.sh` prepares the bundle without launching it. The bundle is `target/Fountage Slint Lab.app` within this directory, uses a distinct identity, and compiles both current layered and legacy approved icon resources. Production document associations are not registered. CLI execution remains available for other platforms and benchmarks.

Launching without a file opens an empty workspace with the eight-category New Draft chooser, plus Open Project and Open Source. Cancel leaves an empty writable draft; no sample screenplay is inserted. Explicit CLI file arguments bypass the chooser; the sample remains only for benchmarks/tests.

## Implemented experiment

- Source editing using Slint TextEdit, with background Rust analysis. One worker processes one snapshot while at most one newer snapshot waits; intermediate waiting snapshots are replaced. Results are revision-checked both before queuing and on UI delivery. Screenplays use a single shared engine snapshot parse for analysis and output.
- Eight writing category choices with shared engine analysis and outline.
- Screenplay semantic preview with rough character/dialogue/parenthetical indents and scene heading weight. Other categories show raw source as explicitly labeled.
- Light/dark controls bound to the toolkit palette; independently selectable white preview paper.
- Save a copy through a native dialog, with atomic no-clobber persistence. Existing destinations are refused, even if the dialog's replacement prompt was accepted.
- Built-in Slint attribution popup.

## Acceptance evidence — 2026-10-14

- macOS debug build and Clippy with warnings denied passed; 5 prototype tests and 47 existing core tests passed. The two added worker tests use channel barriers to verify in-flight stale rejection, newest-source/category coalescing, shutdown, and rechecking results after queued delivery becomes stale. Prototype build and Clippy were rerun after the worker change; the core was unchanged.
- Live macOS app showed source, engine counts/outline and indented preview. Light palette first failed contrast against system dark controls; explicit Palette binding fixed it. Rechecked light appearance and dark appearance with white preview paper visually.
- A changed source character appeared in the live preview and word count. A CUA human-input notification stopped further editing tests; no claim of successful select-all, paste or undo validation. The running scratch buffer was left intact.
- Debug engine plus preview-model construction: 2,980 bytes/168 rows ~3.2ms; 29,800/1,698 ~26.7ms; 89,400/5,098 ~69.3ms. These are synthetic repeated samples, not representative feature-length UI responsiveness or frame-time measurements.

## Not accepted yet

No migration decision. This is source editing beside derived output, not an inline formatted manuscript editor. Existing-project source editing, new-copy publication and project-baseline recovery are experimental. In-place saving, type migration, complete Quit/shutdown protection, pagination, title pages, rich emphasis, dual-column dialogue, scene-number layout, print/PDF and full menus remain open. Heading selection has generated-editor coverage; richer navigation and native interactions do not. Preview uses an eager layout; model/text layout and synchronous recovery/save work still need long-manuscript/large-attachment latency checks. Unicode/CRLF replacement and Undo round trips pass in the simulated backend, not as a guarantee across native input methods/platforms.

IME, bidirectional text, composition, selection/caret offsets, undo/redo, clipboard, screen reader behavior and keyboard focus need hands-on testing. Presence in the accessibility tree does not certify accessibility. Windows and Linux builds, runtime, packaging, fonts and integration remain untested.

## Primary sources

- https://docs.slint.dev/latest/docs/slint/reference/std-widgets/views/textedit/
- https://docs.slint.dev/latest/docs/slint/reference/keyboard-input/textinput/
- https://docs.slint.dev/latest/docs/slint/reference/std-widgets/style/
- https://slint.dev/agreements/slint-royalty-free-license.pdf

Pinned Slint 1.18.1. Royalty-free licensing requires attribution; the prototype includes AboutSlint. This is not a paid-service signup or purchase.

### Background analysis and launcher follow-up

Analysis timing is now separate from save/error status, so preview delivery cannot erase save feedback. New UI behavior has not yet been rechecked live: the prior Slint Lab process remains running with a scratch buffer, so its bundle was deliberately not replaced. The packaging guard was verified against that actual process and refused mutation as intended; launcher shell syntax checks passed. Cross-platform runtime claims remain unverified.

### Scratch close follow-up

Window-close source checkpoint policy added; 8 tests and Clippy pass. Save failure/cancellation and Keep Writing retain the draft, clean/undo-to-checkpoint needs no prompt, successful copy marks the current source saved. Initial native rfd message prompt blocked UI automation and was rejected after live testing. Current source uses a nonblocking Slint overlay, with underlying controls disabled; its live behavior is not yet verified. Native Quit/Dock Quit and OS shutdown remain explicit open gates. Do not rely on this experiment for unsaved manuscripts. The old isolated Close QA native dialog was left for normal user dismissal; do not repeat that build.

### Persistent scratch recovery (2026-10-14)

Each new session creates a unique private folder beneath the platform's local application-data `Fountage Slint Lab/Recovery` directory. Initial source, subsequent editor changes and category changes write one atomic versioned JSON snapshot with exact source and category. Successful writes flush the file before reporting that the recovery copy is current. Failure is shown separately from save and analysis status. Sessions are retained on exit, including after Discard; there is currently no automatic pruning or recovery-management UI.

Recover… opens a validated snapshot into the scratch editor and requests an explicit source-copy save. It refuses to replace a draft that differs from its save checkpoint. Original files and earlier recovery files are never overwritten by recovery loading. Malformed/schema-future/category-invalid recovery data is refused without rewriting it. `dirs` 7.0.0 supplies platform local-data locations: https://docs.rs/dirs/7.0.0/dirs/fn.data_local_dir.html.

11 tests and Clippy -D warnings pass; build succeeds on macOS. Tests cover Unicode, combining characters, NUL and mixed line endings, all eight categories, per-session isolation, survival after drop, invalid updates, malformed/future records and reported write failure. A separate live QA process created a recovery snapshot matching the exact initial sample/category at `~/Library/Application Support/Fountage Slint Lab/Recovery/session-GRqCLk/scratch.json`. The QA UI then reported human activity (Dark switched on); further automated input stopped. Recovery load and the nonblocking close overlay have not yet been exercised live. Original Lab session remained untouched.

Snapshot writes are synchronous on the UI thread to persist accepted edits before a normal immediate exit; large-document and slow-disk latency remain unmeasured. This is a recovery aid, not full crash/power-loss/OS-shutdown acceptance or a production backup system. Directory-entry durability after power loss, disk-full behavior under real load, platform runtime and corrupt-storage repair still need verification. No real manuscripts were used for testing.

### Generated-editor checks and source transaction history

14 tests, Clippy and Mac build pass. Dev-only `i-slint-backend-testing =1.18.1` exercises the actual generated TextEdit without a desktop. Its first selection-replacement test exposed a two-step toolkit undo. The prototype now routes source Undo/Redo through Rust changed-span transactions. Exact original Unicode/CRLF is restored in one action; redo, branching and history reset are covered. The same test also covers disabled source input while the close overlay is open and an isolated test clipboard for copy, paste and cut. No real pasteboard was modified.

History is reset on recovery load and refuses external-source mismatches. Save checkpoint, recovery and analysis notifications run after Undo/Redo. This is not complete undo parity: typing coalescing, retention/memory policy, IME/preedit interactions, native menu integration and native physical shortcut routing remain open. The toolkit test backend measures text with simulated fonts and does not establish visual layout, real bidi movement or accessibility. Public source selection offsets are UTF-8 bytes; engine UTF-16 navigation ranges still need adaptation.

Testing backend docs: https://docs.slint.dev/latest/docs/rust/i_slint_backend_testing/. The internal API is pinned exactly to the toolkit patch version and is not a production runtime dependency.

### Typing groups

16 tests pass. Adjacent scalar typing now coalesces within 750ms, with a 64-character maximum run. Native non-text/modified key events and explicit navigation, Save/Recover/About/category/close/Undo/Redo actions break groups; replacements, paste/cut and multi-character commits stay atomic and separate. Deterministic tests cover time and length limits, plus generated TextEdit typing/Undo/Redo. Mouse-only caret changes, deletion grouping, IME/focus subtleties and history retention are still acceptance gates; no native runtime claim is made by these headless checks.

### Outline navigation

18 tests pass; Clippy and Mac build pass. Heading rows now activate exact source selections. Engine line indexes are converted to UTF-8 ranges with original line endings retained; duplicate headings are distinguished by position. Source/category snapshots and valid range bounds are checked before navigation. Outdated rows cannot select a passage in a changed source.

The generated-editor test selects the second duplicate scene heading after Unicode/CRLF content, verifies navigation alone does not edit, replaces just that heading and undoes it, then confirms stale navigation leaves the caret untouched. Native clicking/tab activation, scrolling/reveal and visual treatment still require live acceptance. Other engine indexes using UTF-16 need a separate adapter. The prototype has not replaced the production Navigator or its broader feature set.

### Workspace appearance and offscreen visual checks

Serif branding and pane headings, quiet platform-style outline list, neutral green surfaces, secondary text, full-width separators and compact layout replace the initial test-bench styling. Window size is now preferred 1280×860 with a 960×640 minimum, rather than fixed. This remains a source/preview experiment, not the complete production writing UI.

To render without touching the desktop:

```sh
FOUNTAGE_LAB_RENDER_DIR=/tmp/fountage-slint-workspace-renders cargo test --manifest-path prototypes/slint/Cargo.toml render_tests::render_workspace_views -- --nocapture
```

The optional renderer uses actual software rasterization and fonts, with mocked animation time, and exports light, dark/white-paper, dark-paper, compact and close-overlay PNGs. Its test entry returns without rendering when the environment variable is absent. 18 logic tests plus that entry pass; the explicit five-state render run passes. Saved inspected artifacts: `../../output/slint-workspace-mfs3atgd/`. Native window chrome, physical resizing/activation, native graphics/font behavior, accessibility and other OS runtime remain unverified.

### Existing .fountage projects and preservation-aware copies

The prototype now accepts a `.fountage` directory package as a CLI argument and exposes Open Project… and Save Project Copy… controls. Loaded project format/preset remain intact; format conversion is disabled. Save copies include metadata, assets and opaque entries through the shared Rust new-copy writer and never overwrite existing destinations. Project source edits remain in the scratch editor until an explicit copy is saved; original packages are never used for replacement saves.

Before adopting an opened project, the prototype creates an independent recovery baseline package. Schema-2 recovery JSON references its fixed sibling `project-baseline.fountage` and holds current source/category. Recovery loads both components and refuses missing/invalid baselines, NUL source or category mismatches instead of degrading to plain text. Earlier schema-1 source-only recovery remains readable. Recovery into a new session preserves a fresh full baseline; old sessions remain available. A draft with unsaved text must be copied before Open Project/Recover replaces it.

22 prototype tests (21 logic tests plus conditional render entry), Clippy and Mac build pass. Preservation workflow tests open/edit/recover/save/reopen with foreign metadata, Shelf-like data and binary attachments, verify original bytes unchanged, and reject blocked/missing baselines and drift. The six-state software render run includes a compact project layout; artifacts are `../../output/slint-project-ui-luph32p0/`.

Native Mac pickers and interactive open/save/recovery/close sequences still need live validation; source/recovery callbacks have not been certified through those dialogs. Large package read/copy, baseline creation and save run synchronously and may stall. Relative external symlink resolution after relocating a package is not certified; non-Unix attachment links are refused by the writer. In-place save/conflict handling, modification timestamps, schema/type migration, full source/metadata Undo and other-platform runtime remain open. User running apps have not been rebuilt or replaced automatically.

### Save checkpoints and Edited

The prototype now distinguishes a saved empty source from a recovered draft that has never been explicitly saved. A canceled/failed copy keeps the checkpoint unchanged; an empty recovered draft blocks replacement/close just like any other unsaved draft. Edited appears in the source header/window title, clears on Undo back to saved text and reappears on Redo. A fixed caption width prevents its appearance moving the pane boundaries.

23 tests pass; build/Clippy pass. The generated editor exercises these source-state transitions. The seven-state software-render run additionally compares actual divider pixel columns between clean/Edited states. Artifacts: `../../output/slint-checkpoint-ui-d_r96171/`. Native modal timing, native title/Quit/close behavior and general metadata dirty state remain unverified. Open/Recover now recheck the draft after pickers return; closing after a copy keeps the window open if the source changed during saving.

### Close confirmation keyboard behavior

The nonblocking close confirmation focuses Keep Writing, wraps Tab/reverse Tab among its three actions, and handles Escape as Keep Writing. Dismissal restores editor focus and selection. Generated-component tests exercise these paths; native keyboard/OS Quit/file-picker lifecycle acceptance remains open. Automated Mac Command chords have produced literal characters in isolated QA and are not certified by the headless shortcut tests.

### Source Find

Find… opens literal manuscript search with Match case, Previous/Next, wrap and match counts. Return searches; Shift+Return searches backwards from the query field. Escape closes Find and leaves source intact. Source editor logical Control+F and Control+G/Shift+G open/navigate (Slint maps to Command on macOS; native synthesized modifier handling remains an open gate). Search navigation selects the actual editable source; typing over it uses the same Undo/checkpoint/recovery path as other edits.

Case-insensitive search uses Unicode simple case folding, with no normalization or multi-character full folding. Hits are nonoverlapping. Query or source changes reset navigation to first/last result. No Replace controls, whole-word/regex mode, selection-seeded search or current-caret start yet. Search scanning/counting is synchronous; large-document latency and complete native/platform acceptance remain open.

### Replacement follow-up

Replace… expands a replacement field. Replace Match edits the last navigated Find match only when source/query/case still match that search. It does not derive a new target from a manually moved caret/selection. Replace All scans the current whole draft for nonoverlapping literal hits. Empty replacement deletes hits; dollars/backslashes have no expansion syntax. Each changed action is one source transaction with Undo/Redo, checkpoint, recovery and preview integration. An already-replaced target cannot be replayed; find again for a new target. Identical replacement makes no edit.

Logical Control+Alt+F exposes replacement from the source editor. Native physical shortcuts/Undo and project lifecycle acceptance remain open. Basic native Match/All/save-copy output has been verified on the synthetic sample; no current-Highland or other-platform parity certification.

### Native Edit menu

Edit menu provides source Undo/Redo, focused clipboard actions, Find/Replace and Next/Previous. Native macOS menu accelerators are now verified for Command-Z/Shift-Z, Command-A, paste, Command-F/G/Shift-G and Command-Option-F on synthetic source. Unicode multiline paste and bulk replacement each undo in one action. This supersedes the earlier command-synthesis failure for these actions in the rebuilt main Lab.

Undo/Redo now routes to separate source, Find and replacement histories according to focus. Native Command-Z/Shift-Z and menu Undo have been verified in both small fields without changing manuscript/dirty/recovery state. Menu clipboard actions also route to the focused field. Selection-aware Cut/Copy enablement, a full desktop menu set, physical layouts/IME, native Cut/Copy and Windows/Linux runtime remain unverified. Software test rendering shows Slint's fallback menu; macOS uses the system menu bar.

### Quit and file-dialog safety

Mac native Quit checks saved-source state via a guarded bridge on the existing pinned winit application delegate. Clean source accepts AppKit termination; unsaved source opens the same nonblocking review as window close/File→Close (Command-W). Save cancellation keeps source open and restores editor focus. File Open Project/Recover/Save Copy are available via menu; no in-place Save yet. File pickers disable competing edits/actions with an RAII guard.

Native synthetic QA verified Command-Q, application-menu Quit, Command-W, Save Copy cancel, exact saved bytes before exit and clean Quit. Dirty native requests use NSTerminateCancel then asynchronous review, so OS logout/restart would require retry after review; those system actions and Dock Quit are not live-certified. Native project quit/reopen/discard and multiwindow/other-platform acceptance remain open. The Mac bridge verifies pinned delegate/layout assumptions and refuses unknown or preexisting termination policies.

### Native project copy/reopen evidence

Isolated comic package now verified in the running Mac app through Open/edit/Undo/Redo/Quit/Save Project Copy/fresh reopen/clean Quit. Exact source, unknown raw metadata, large integer and attachments checked on disk, with original package unchanged. Fixture/evidence: output/slint-project-native-pivvhlag/. Project close review explicitly names project copies and attachment preservation. This covers that fixture only; native recovery, conflicts/in-place saves, all categories and platform-wide acceptance remain open.

### Active document identity and modal menu dispatch

Opened path basename appears in window/editor heading, with Edited and v001 retained. Long labels elide within stable columns. Copy suggestion uses basename plus Copy and project/source extension; saving a copy keeps the active origin label rather than rebinding to that copy. Recovered drafts are labeled explicitly without guessing their original filename.

File menu actions are queued/coalesced outside native menu activation: modal picker display timers could otherwise rebuild a still-borrowed native menu and abort. Native source/project chooser shortcuts, cancellation and clean Command-W now verified in one process. Recovery menu shares dispatch but its complete native workflow remains unverified. No represented-file proxy, in-place Save or broader platform parity claim.

### Native recovery evidence

Native Recover Draft now verified on constructed schema2 comic project snapshots, including an empty source. Recovered drafts require an explicit save even when empty; another project cannot replace them first. Copies retain baseline metadata/attachments exactly. Missing baseline reports how to keep its folder with the snapshot JSON and leaves the current draft intact. Saved-empty copy reopens clean. Fixtures/hash evidence: output/slint-recovery-native-qmk4olx8/.

This is not a power-loss/actual crash recovery test. Session browser/discovery/pruning, native all-category/schema failures, conflicts/in-place save, long assets/source and other-platform acceptance remain open.

### Composition-event evaluation (2026-10-14)

The generated-editor test now injects pinned Slint internal composition events through a test-only i-slint-core dependency. Temporary Japanese preedit/cancel leaves the manuscript and saved checkpoint unchanged; commit replaces the selection as one Undo/Redo transaction. A one-scalar commit stays separate from preceding ordinary typing, and close/picker locks reject composition edits. This is not native IME or preedit-pixel acceptance.

Known limitation: Slint 1.18.1 shared-parley window event dispatch NFC-normalizes incoming text, including composition commits (e + combining acute becomes é). Tests record this behavior explicitly. Existing loaded source remains byte-preserved; exact committed-byte preservation, native candidates/reconversion, Save/Quit during marked text and platform IME coverage remain open.

Native Mac dead-key follow-up: the packaged Lab was exercised with Command-End, Option-E, e. Marked accent rendered while document stayed clean; commit updated source/preview/Edited, native Undo restored original clean sample, Redo and Save Copy produced exact sample+é (output/slint-native-accent-8k7p74qa/verification.json). Escape after another dead key committed standalone acute, not cancellation; Undo restored saved source and clean Quit exited. Native CJK/reconversion and marked-text Save/Quit remain open; no native plain-source reopen in this run. Generated-editor relative-range commit replacement/Undo/Redo now also covered.

### Open source files (2026-10-14)

File → Open Source… (Command-Shift-O on Mac) opens UTF-8 .fountain/.txt/.md without changing the original. Fountain selects Screenplay; text/Markdown retain the selected writing category. Fresh recovery must succeed before adoption; dirty drafts must first be saved as a copy. New source load resets history and package ownership. Invalid UTF-8/NUL/unsupported types are rejected. CLI source loading shares validation. Open Project remains separate.

Native source Save Copy/reopen now verified with Unicode/CRLF Comic text and accented Fountain, alongside invalid UTF-8, dirty guard and empty Markdown. Evidence: output/slint-source-open-w_cbbhvn/verification.json. In-place Save, source associations/Recent/unifiedOpen, native all-category/project-to-source transitions, latency and other platforms remain open. Automated native Command-Z with Undo disabled inserted z, while enabled Undo worked; this remains an explicit native shortcut acceptance issue pending physical/backend investigation.

### Whole-word Find (2026-10-14)

Find's Whole word option filters literal matches using Unicode word boundaries, consistently for navigation and both replacement actions. Changing it invalidates any old replacement target; Replace All remains one document Undo transaction. Letters/marks/digits/underscores count as word characters, hyphens separate words; this is not language-specific CJK segmentation or normalization, and punctuation-only queries generally do not satisfy both boundaries. Default substring search is unchanged.

Native4substring/2whole-name MARA matches, selective bulk replacement, Undo/Redo and exact CRLF save/reopen passed; fixtures/hash report and11render states in output/slint-whole-word-tz9sfzpt/. Find controls use separate option/navigation rows at compact width. Current Pro comparison, selection-seeded Find (std TextEdit lacks public selection offsets), regex/QuickFind/JumpSelection and disabled-shortcut acceptance remain open.

### Regular-expression Find and replacement (2026-10-14)

Enable Regular expression to use Rust regex syntax, e.g. `\[\[(.*?)\]\]` → `**$1**` converts Fountain notes to bold source. Replacement supports numbered/named groups (`$1`, `${name}`), `$0` for whole match and `$$` for a dollar. Unknown references/unclosed braces reject the action; unmatched optional groups are empty, backslashes literal. Plain mode still treats query/replacement literally. Inline multiline/dotall flags are available; look-around and pattern backreferences are unsupported. Invalid/oversize patterns cannot change source. Whole word and Match case share the same validated input snapshot.

31tests/11render states, Clippy and Mac build pass. Native2note conversion, Undo/Redo, invalid pattern/bad capture safety and exact Unicode/CRLF save/reopen verified in output/slint-regex-ervjfwvi/verification.json. Native named/zero-width behavior, performance, token/recents UI, full Highland/Foundation syntax compatibility and other-platform acceptance remain open.

### Find Again without search controls (2026-10-14)

After searching, dismiss Find with Escape/Done and use Command-G / Shift-Command-G to navigate the retained query/options. The bar stays closed; the footer shows match count or No matches. This never edits source or changes saved checkpoint. Hidden navigation revalidates current source/options; replacement remains restricted to visible Find/Replace. Native forward/backward/wrap/no-results and generated-editor non-mutation checks pass (output/slint-find-again-yzzxxlen/verification.json). Currentcaret/selection-driven search and native no-query/disabled-shortcut behavior remain open.

### Current Find modes and wrapping (2026-10-14)

Matching selector now offers Contains, Starts with (word prefix), Whole word. Wrap around is optional, on by default. With wrapping off, Next/Previous stop at the last/first match; enabling it again wraps from the retained position. Matching modes apply to both replacement actions. Search still begins from document start/end after input/source changes, not the current caret.

Native6substring/4prefix counts, no-wrap boundaries, toggle position retention, prefix ReplaceAll/UndoRedo/exact CRLF save/reopen verified.32tests/11render states/build/Clippy pass; evidence output/highland-find-audit-3r4as287/. Current Highland Pro3.3.6 Find controls audited on an isolated imported file; Replace&Find, Option-selected-scope ReplaceAll and broader current Pro search behavior still need implementation/acceptance.

### Replace & Find (2026-10-14)

Replace & Find replaces the current validated match and selects the next match after inserted text, respecting Wrap around. Newly inserted matches are skipped until wrap; adjacent matches moved by deletion remain eligible. Zero-width regex matches progress beyond the same position, and identical replacements advance without creating Undo/recovery writes. Every changed action is one source Undo transaction.

Native two-note capture conversion→next selection→no matches, individual Undo/Redo and exact CRLF/Unicode save/reopen verified (output/slint-replace-find-ak3i101a/verification.json).33tests/11renders/Clippy/build pass. Native edge-case matrix and full current Highland behavioral comparison remain open; currentcaret/manual selection and selected-scope replacement not implemented.

### New Draft and project creation (2026-10-14)

File → New Draft… (Command-N) asks what you are writing across all8 categories. Escape/Cancel keeps the current draft; unsaved source blocks replacement. Selecting a category card stages the choice; Create (or Return on the focused card) creates an empty schema2 Fountage project with its own UUID/timestamps/type/default preset and full recovery baseline. The empty new project requires Save Project Copy to preserve its category/identity. New resets source history and old outline/preview; existing recovery remains intact. Comic defaults to Fountage Standard; Academic is an academic draft, not a certified APA/MLA template.

NativeComic creation by keyboard, cancel/dirty guards, empty Quitreview, empty+edited SaveCopy/UndoRedo/native reopen pass; metadata/source evidence in output/slint-new-project-04qb9xrz/.58core/34prototype tests,13renders/Clippy/build pass. Other7 nativeNew matrix, UI diskfailure/fulltabcycle, preset/title/edition UI, recents, automatic restore and multiwindow flows, metadata/timestamp editing/in-placeSave and other platforms remain open. Startup now uses the empty workspace and chooser described below.

### Empty startup and direct opening (2026-10-14)

No-file launch opens the category chooser over an empty draft. Open Project/Open Source are available inside it; Cancel leaves an empty usable workspace, and Command-N brings choices back. Picker cancellation also returns to the workspace. Synthetic screenplay source is retained for benchmarks/tests only. Explicit CLI file arguments bypass chooser (not native-retested this run).

Native two fresh launches, picker cancellation, chooser project/source loading with clean correct state and unchanged fixtures verified.34tests/15renders/Clippy/build pass; evidence output/slint-startup-3z_vkuk9/. Recents/automatic restore/multiwindow, failure/fullkeyboard matrix and other-platform startup remain open.

### Notes navigation (2026-10-14)

Select Notes in the Navigator view menu to list parsed [[inline notes]]. A row selects the full original note, including multiline text and delimiters, for editing in source. Labels show first nonblank line with truncation/multiline ellipsis; empty state explains syntax. Worker snapshot guards reject stale ranges, and UTF16 offsets translate safely after emoji. All8 category analyses now return notes the shared lexer already recognized; their word-count/preview policies are unchanged.

Native multiline selection/edit/live label update/UndoRedo/exact CRLF save/reopen passed; evidence output/slint-notes-d9h741q3/verification.json.59core/35prototype tests,18render states/Clippy/build pass. Note filtering/search/add shortcuts/full text view, native category/keyboard matrix, current Pro Navigator comparison, review/threaded comments, output-policy parity and performance/platform gates remain open.

### Filter notes (2026-10-14)

Notes view includes a case-insensitive literal filter over the full parsed body, matching/total count and No matching notes feedback. Filtered rows navigate their exact original source range, not their displayed index in the unfiltered list. Escape clears the filter and focuses source. Query has its own Undo/Redo/clipboard routing, separate from manuscript edits/recovery. It stays session UI state.

Native body-only PAUSE match, filtered multiline selection, queryUndoRedo/SelectAll/no-results/Escape and unchanged fixture hashes verified (output/slint-note-filter-c7d7tndd/verification.json).35tests/20renders/Clippy/build pass. Native clipboard/keyboard/category/currentPro matrix, performance and other Notes/editor/platform gates remain open.

### Persistent appearance (2026-10-14)

Dark interface and preview White paper settings are now remembered across launches in app configuration (Mac ~/Library/Application Support/Fountage Slint Lab/appearance.json). They are not manuscript metadata and do not change source/recovery/Edited. No settings file is written until explicit checkbox changes. Future/malformed settings are left intact with status feedback; unknown raw fields survive saves. Defaults remain light+white when no saved choice.

37tests/Clippy/build pass. Native fresh-launch dark+darkpreview then dark+whitepreview restoration verified; final setting is dark interface with white preview (output/slint-appearance-ch0bsihr/verification.json). White paper currently applies to preview; source TextEdit follows interface theme. Follow-system/source-paper styling, native errors/concurrent/multiwindow and other-platform acceptance remain open.


## Manuscript paper surface

White paper now applies to editable source as well as preview under dark interface chrome. ManuscriptEditor wraps public Slint TextInput and ScrollView, retaining app history, selection setters, clipboard, word wrap, focus/selection colors and caret scrolling. Source remains uniform plain text; formatted inline screenplay editing and pagination are not implemented.37tests/20render states, Clippy/Mac build and native long Unicode/CRLF editing, Find selection, UndoRedo/checkpoint and exact SaveCopy/fresh reopen pass (output/slint-paper-editor-AAOo27/). Native IME/accessibility/context-menu and other-platform matrices remain acceptance gates.


## Mac native history shortcuts

A scoped app-local AppKit KeyDown monitor consumes Command-Z/Shift-Command-Z even when the native menu item is disabled, preventing the pinned backend's unmodified z/Z fallthrough. It defers existing focused history actions and accepts AccessKit subclasses of WinitView; native pickers/review and other window views pass through.38tests/Clippy/Mac build and native source/Find/replacement/Notes history, checkpoint and exact SaveCopy/fresh reopen checks pass (output/slint-native-history-SNylGB/). Native picker keyboard routing, menu focus refresh, other disabled shortcuts and physical layouts/IME/platform matrices remain open.


## Native file-dialog Edit actions (partial)

Mac DialogGuard now scopes a native Edit-menu adapter across hosted file dialogs and Slint menu rebuilds. Standard native responder actions/Command equivalents replace app callbacks temporarily; observer removal and original action/target/key/validation restoration precede source unlock. Direct SaveAs SelectAll/type/Undo/Redo, exact SaveCopy/reopen and source history/Cancel restoration are verified (output/slint-panel-keys-JOya0i/).38tests/Clippy/Mac build pass. Automated dialog Command shortcuts still produce no observed field change; physical-keyboard, clipboard, Open-field and full focus/a11y/platform acceptance remain open.


## Editor gate and long-note analysis

Pinned Slint1.18.1 TextInput supports plain input, not styled paragraph runs; actual inline screenplay formatting requires custom editor integration. No production-editor acceptance claimed. Shared hidden-span indexing now counts original UTF16/line positions incrementally, avoiding repeated prefix scans.60core/38prototype tests, Clippy/Mac build and native8000-note final-note selection/edit/Undo pass (output/slint-editor-gates-0GMysX/). `cargo run --release --example benchmark_notes` from repository root measures synthetic engine analysis only (not UI frames); local largest workload1734.21→5.55ms median. Debug engine+model/native UI costs remain separate. Highland's hosted Open panel reproduces the automated Command-A failure too; physical shortcut coverage remains unverified.


## Experimental formatted layout (not installed as input)

`cargo run --manifest-path prototypes/slint/Cargo.toml -- --layout-audit path/to/script.fountain 600` shapes original screenplay source with Parley and reports paragraph/wrapping/hit-test metrics without launching the app, writing source, or printing manuscript content. Width is logical pixels at display scale1; defaults600. Includes provisional native-reference indents/alignment/emphasis, exact LF/CRLF/EOF source ranges and grapheme-safe caret/replacement primitives. Raw Fountain syntax remains visible.42prototype tests/Clippy/Mac build pass; width600/300 and8003paragraph audits at output/slint-formatted-layout-L4YPHe/. Glyph painting, widget/IME/input/selection/a11y/reflow integration and certified print/dual-dialogue/other-format layout are still required; existing working editor remains unchanged.

### Visual migration checkpoint (2026-10-15)

The workspace now follows the Swift reference's navigator / broad manuscript / narrow draft-inspector proportions. Preview is an explicit central view, with Find/navigation returning to editing. Appearance controls live in a neutral popup; Open/Recover remain in File. Actual shared-engine word/cast counts populate the inspector. Light/dark/compact and preview rendering is checked in 23 snapshot states (`output/slint-studio-visual-20261015`). The working input is still plain source; styled editing and full native presentation remain incomplete. Public release remains v001.

### Formatted editing transactions (experimental, not installed in live input)

`cargo run --manifest-path prototypes/slint/Cargo.toml -- --edit-layout SOURCE COMMANDS.json NEW_OUTPUT_DIRECTORY` exercises the shaped editor's real source/selection/history controller without opening a desktop window. Commands are a JSON array, for example:

```json
[
  {"action":"select","anchor":0,"focus":0},
  {"action":"insert","text":"INT. ROOM - DAY\r\n\r\nMARA\r\nHello.\r\n"},
  {"action":"undo"},
  {"action":"redo"},
  {"action":"reflow","width":420}
]
```

Other actions: `point` (`x`, `y`, `extend`, logical document coordinates), `enter`, `deleteBackward`, `deleteForward`, `selectAll`. Source offsets are UTF8 bytes; invalid/grapheme-interior/CRLF-interior endpoints are rejected. The controller guards geometry generations for eventual asynchronous UI dispatch. The CLI sequences commands against the current generation. An existing destination is refused; successful output includes an exclusive exact source copy, before/after PNGs, final SVG and verification JSON. Input file is never written. Whole-source reanalysis per edit, visual keyboard movement, IME, clipboard, accessible text, viewport integration and live widget adoption remain open. Output is experimental editing geometry, not print output or a finished editor.

Formatted command streams also support `{"action":"move","direction":"lineEnd","extend":false}`. Directions currently implemented: `left`, `right`, `up`, `down`, `lineStart`, `lineEnd`, `documentStart`, `documentEnd`; `extend:true` retains the selection anchor. Motions use shaped cursor geometry and preserve soft-wrap/bidi affinity; repeated vertical movement keeps a preferred global x. Reports include `upstream` per command and `caretUpstream` in painting metrics. These are controller commands, not yet physical-key bindings in the live editor. Word/page movement and full platform/bidi/input-method acceptance remain open.

### Try the formatted editor in the workspace

Choose **View → Try Experimental Formatted Editor** in a screenplay draft. This enables real shaped paragraph indentation, scene/emphasis styling, selection/caret painting and basic keyboard/pointer editing on the same draft/history/recovery/save path. **View → Use Standard Source Editor** returns to the standard input. Default is unchanged. Formatting source markers remain visible; this is editing geometry, not paginated print output.

The live adapter supports viewport resizing, caret-follow scrolling and wheel scrolling, Find/source range selections, shared Undo/Redo and mirrored native clipboard selection. It uses direct cached raster frames without PNG compression. IME/preedit and accessible text are **not integrated**, and full native clipboard/context menu, platform shortcut, long-source performance and all-category formatting acceptance remain open. Use the standard editor for writing that needs those capabilities. The opt-in surface is deliberately screenplay-only pending those gates.53prototype tests and27render states currently pass; native exact save/reopen/Undo evidence is in `output/slint-formatted-live-20261015`.

Clipboard follow-up: formatted Copy reads the controller's exact source range; Cut/Paste use controller transactions via a separate unbound clipboard transport. A paste replacement is one production undo step, including multiline Unicode and CRLF. Empty Cut/Copy preserve clipboard content, and empty Paste does not delete a selection. Standard editor mirroring is retained for fallback only. Right-click actions now open on the formatted surface and Cut/Copy follow selection availability. Native Command-A/X/C/V protection is scoped to formatted mode and bypasses file pickers. Invalid formatted geometry blocks input and clears stale frames until a valid viewport returns; the standard editor remains available. Native selected Copy/Cut interchange is still an acceptance gate.55tests/27render states; evidence `output/slint-clipboard-20261015`.

### Formatted layout reuse and measurement

`cargo run --manifest-path prototypes/slint/Cargo.toml -- --benchmark-formatted` emits synthetic controller timings for100/500/1500scenes,3samples per size. It separates cold creation, edit, Undo, resize and fresh viewport painting, and identifies the build profile. It does not measure native input/frame latency or write manuscripts.

Formatted editing now shares immutable paragraph layouts, verifies candidate text/role/styles exactly, and rebreaks glyph shapes for new widths. Live Undo/Open adoption retains context; production History is the sole live history owner. Glyph and selection painting considers visible paragraphs. Evidence in `output/slint-layout-reuse-20261015` includes before/after timings, identical scrolled SelectAll pixels and native exact188KB save/reopen workflow. Cold layout, full-source scans and fresh raster work remain performance gates; the editor remains opt-in.

### Composition groundwork — native IME is still not connected

The formatted controller supports `preedit` (`text`, optional UTF8 `cursor:[anchor,focus]`, optional absolute committed-source `replacement:[a,b]`), `commitComposition` (`text`) and `cancelComposition` in `--edit-layout` command streams. Preedit creates a separate marked display document; the saved source remains committed text. Commit is one source edit; Cancel restores the original display. An empty `preedit` cancels; an explicit empty commit can delete the pending replacement range. These are application-level commands, not native event-unit semantics.

Live adapter callbacks are available for integration and covered by generated-widget tests. Current stock FocusScope still does not deliver native composition, and candidate windows/input-context requests are not connected. The marked renders in `output/slint-composition-20261015` are callback/script-driven QA. Physical IME support, signed/relative offset translation, partial commits and native focus/save policy remain acceptance gates. Standard TextInput remains the default and input-method fallback.

### Focus mode

Use **View → Focus** or the toolbar button (**Command-Shift-F** on macOS) to hide both side panels; repeat to restore them. The same editor stays active, preserving selection and shared Undo/Redo. Preview also supports Focus. This is a per-window presentation state, independent of document contents. Highland-style automatic toolbar hiding is still pending.

### Swift-style New chooser (2026-10-15)

New uses a full-window branded chooser with eight descriptive, illustrated cards and a separate Create action. Four columns at wider window sizes, two at compact sizes; only the card list scrolls so the brand/header/footer stay visible. Click/Space selects a card, Tab moves focus, Return on a card creates that category, and Escape cancels. Open Project/Open Source remain available. Selection alone never replaces the current draft. Layout/preset, academic-edition selection and starter-outline options from the Swift chooser remain separate pending work.

Formatted keyboard progress: the opt-in screenplay editor supports Option-Backspace and Option-forward-Delete on macOS for logical word deletion (Control-modified equivalents in the non-Mac event path, not runtime-certified there). Existing selections delete exactly once, with shared Undo/Redo. Line/paragraph deletion, complete Highland boundary policies and full IME/accessibility acceptance remain pending.

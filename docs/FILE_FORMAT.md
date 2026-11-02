# Fountage project format, version 2

Status: development format. The initial container is implemented; future writing features and their detailed schemas are not. This document distinguishes external facts from Fountage design decisions.

## External specifications checked

- [Highland's published format](https://quoteunquoteapps.com/highland-pro/highland-format): a customized, ZIP-compressed TextBundle. A complete metadata schema is not published there. Extra directories must be retained to avoid losing application data.
- [TextBundle specification](https://textbundle.org/spec/): allows derived extensions and types; defines text, JSON metadata and assets; application metadata belongs in a versioned dictionary keyed by the application's identifier. Other applications' metadata should be preserved.
- [Fountain syntax](https://fountain.io/syntax/): defines scene headings, character cues, dialogue, dual dialogue, forced elements, emphasis, title pages, notes, omissions, sections and synopses. Parsing must preserve intentional whitespace and punctuation. The current index is not a complete implementation of this grammar.
- [Final Draft's export documentation](https://kb.finaldraft.com/hc/en-us/articles/27525594609684-How-do-I-export-a-Final-Draft-file-to-a-different-format-like-RTF-or-TXT): identifies FDX as its standard document format. This research did not establish a complete official public FDX schema. Do not invent one from a small example.

Checked October 14, 2026. These sources inform the implementation but do not prove full interoperability.

## Fountage's own decisions

Primary extension: `.fountage`.

The current on-disk format is an uncompressed directory package. On macOS it appears as one document in Finder, and its contents can be inspected using Show Package Contents. The new Windows/Linux target does not by itself change these on-disk contents or imply that package writing is implemented on those platforms. This avoids re-compressing every asset on each edit. ZIP-based sharing/export can be added separately; the current reader does not accept zipped `.fountage` files.

The UTI is `com.bazstudios.fountage.project`, declared as a derivative of `org.textbundle.package` and `com.apple.package`.

```text
My Screenplay.fountage/
  text.fountain  [screenplays; text.txt for other types]
  info.json
  assets/
  [additional preserved entries]
```

`text.fountain` is the authoritative UTF-8 screenplay. All other document types use `text.txt`; the namespaced `documentFormat` field selects their semantics. `info.json` is JSON. `assets/` contains attachments. Unknown package entries remain intact when a project is opened, edited and saved.

Initial metadata shape:

```json
{
  "version": 2,
  "type": "com.fountain.screenplay",
  "creatorIdentifier": "com.bazstudios.fountage",
  "com.bazstudios.fountage": {
    "version": 2,
    "documentFormat": "screenplay",
    "layoutPreset": "screenplay-standard",
    "sourceFile": "text.fountain",
    "projectID": "a generated UUID",
    "createdAt": "ISO-8601 timestamp",
    "modifiedAt": "ISO-8601 timestamp",
    "authors": [],
    "settings": {},
    "characters": {},
    "notes": [],
    "revisions": [],
    "snippets": [],
    "goals": {}
  }
}
```

The outer version is the TextBundle version. The nested version is Fountage's independent schema version. Reserved collections start empty: their presence does not mean those features are implemented. Define each record schema and migration before populating it. Stable scene IDs, range anchors, revision history and export settings will require explicit schemas and editing semantics; do not use raw line numbers as durable identity.

## Preservation and failure behavior

- Never derive project identity from the filename. Rename keeps the project UUID.
- Store unknown metadata values and additional files, including foreign namespaces, across saves.
- Native document writes use value snapshots so background saving cannot share mutable FileWrapper trees with the editor.
- Reject malformed project identity, missing text/metadata, invalid UTF-8, NUL text and unsupported schema versions with an error.
- Reject NUL in manuscript text before generating any save wrapper, including plain-text/Fountain export. This prevents replacing a readable document with one the reader would reject. Do not silently remove characters; binary attachment contents are unaffected.
- Preserve source line endings and Unicode in serialization. The editor may normalize line endings on edits; test this before claiming byte-preserving edit round trips.
- Plain Fountain export contains screenplay text only. It cannot carry Fountage-only project metadata. Keep the project as the authoritative copy.
- Highland import must create a separate Fountage project and keep the source package unchanged. Preserve unknown imported data; report which features have and have not been translated.
- FDX import/export needs real fixtures, XML parser hardening and per-feature fidelity checks before claiming compatibility.

## Implemented verification

Automated Swift tests cover project serialization, Unicode Fountain export, stable identity, unknown metadata, opaque extra entries and assets, rejection of future versions, and the Swift-to-Rust outline bridge. Additional UI tests cover native open/edit/undo and scene navigation. Production recovery, sync-conflict behavior, revision anchoring and format migrations remain outstanding.

## Document types and migration

Schema 2 adds `documentFormat`, `layoutPreset` and `sourceFile`. Values and built-in presets are defined in `DocumentFormat.swift`. Screenplay uses outer `type: com.fountain.screenplay`; all other document types use `public.plain-text`. Plain `.txt` opens as a novel draft until saved into a project and assigned a type. Only projects persist settings.

The reader accepts schema 1 screenplays. Saving migrates them to schema 2, preserving project identity and unknown data. Changing type preserves the source text, resets the layout to that type's default, and updates the primary source filename on save. It removes the former primary manuscript entry. A pre-existing attachment with the new manuscript filename causes an error rather than being overwritten. Invalid type/preset combinations and unexpected source paths are rejected.

Optional `academicStyle` in the Fountage namespace records an intended standard:

```json
{"family": "apa", "edition": 7, "variant": "student"}
```

Edition and variant are explicit and never follow a moving “latest” alias. The current picker offers APA 7 student, APA 7 professional and MLA 9 (empty variant). Other well-formed values remain intact when opening/saving. This is metadata only: the current renderer does not apply APA/MLA citation or submission rules. Unknown fields in an existing academicStyle dictionary survive changing the known settings. Instructor overrides, bibliography records and citation anchors still require schema design.

Automated checks now also cover all built-in types/presets, schema 1 migration, filename conversion, attachment collision protection, persisted academic editions, and visual screenplay indents without source mutation.


The eight-category chooser adds schema-2 values `stageplay`, `audio`, `general` and `poetry`. Their source remains `text.txt`. Book retains `novel`; Fountage Standard retains the preset ID `comic-full-script`. These public label changes require no source or metadata migration. The app release label remains v001 and is independent of metadata schema 2.

## Draft word goal

Schema 2 optionally stores `goals.draftWords` in the Fountage namespace, for example `"goals": {"draftWords": 50000}`. It is a whole-number target from 1 through 10,000,000 for the entire manuscript, independent of writing category. An absent value means no target. Removing a target deletes only `draftWords`; other goal metadata remains intact. No schema migration or public release version change is needed for this optional field.

The reader ignores unsupported values (including booleans, fractions, strings and out-of-range numbers) for display but preserves them on save unless the user explicitly changes the goal. The target persists with project metadata; current progress is recomputed from the Rust engine's source word count. It can fall below the target again when text is removed. This is not a daily/session goal and counts still include source markup. Fountain and plain-text export remain exact source text and cannot carry this metadata.

## Optional cast/location reference cards

Within `com.bazstudios.fountage`, `referenceCards` is an optional JSON object keyed by stable UUID strings. Each known record has `kind` (`character` or `location`), `name`, and `notes` strings. These manually saved records are separate from derived manuscript indexes and available in every project type. Name edits do not rename manuscript text; manuscript renames do not migrate records automatically. Removing saved details never removes source text.

Updates preserve unknown fields on a known record and unknown records in the collection. An unsupported collection shape is retained and disables editing. Plain Fountain/text exports cannot carry these project-only details. This optional addition uses the existing schema version 2 and public release v001.

## Cross-platform Rust snapshot reader (evaluation)

`fountage_core::project::Project` reads the current directory package into an immutable entry snapshot, validates schema 1/2 identity/category/preset/container/source metadata and provides source-only editing in memory. All eight current categories and 13 presets are recognized. The snapshot retains raw `info.json` bytes; source replacement never reserializes metadata, so foreign fields, formatting and opaque numeric values are retained verbatim. The parsed JSON view is for inspection, not a new serialization authority. Existing schema 1 stays schema 1 in this API; no migration is silently performed.

Additional file contents, directory entries, native filename representations and symlink targets are retained in memory. Stable attachment symlinks are captured without following them. Primary manuscript/metadata entries must be regular files. Unsupported filesystem objects fail rather than disappear. This does not preserve arbitrary filesystem permissions, ACLs, extended attributes or resource forks, and it is not a transactional snapshot against concurrent filesystem mutation.

The module now exposes exclusive new-copy publication, described below. It has no replacement-save, type-conversion or format-migration API and is wired experimentally to Slint project opening/new-copy saving; native dialog acceptance is still pending. Do not claim full cross-platform package save or interoperability from these read checks. A read-only structural probe is available:

```sh
cargo run --example inspect_project -- /absolute/path/Example.fountage
```

Verification: 52 core tests pass (five new project tests), with coverage across categories/presets, exact metadata/attachment retention, schema 1, source Unicode/line endings, rejection cases and stable symlink handling. A Swift-created synthetic QA project was read successfully and SHA-256 file manifests matched before/after. No real manuscripts were used. macOS rejects arbitrary non-UTF8 filesystem names; their retention is tested in the in-memory tree on this host, not as an on-disk macOS filename claim. Windows/Linux runtime and package publication remain open acceptance gates.

## Exclusive new-copy publication (evaluation)

`Project::save_copy(destination)` writes a complete staged directory alongside a new `.fountage` destination, flushes regular-file content, then publishes with an exclusive rename. It refuses existing files, empty/nonempty directories and dangling links. It uses pinned `renamore` 0.3.2's `rename_exclusive`; unsupported exclusive-rename filesystems return an error rather than falling back to an overwrite-prone exists-check/rename sequence. See [the dependency's platform/API contract](https://docs.rs/renamore/0.3.2/renamore/fn.rename_exclusive.html). Its platform operations are `renamex_np(RENAME_EXCL)` on Apple platforms, `renameat2(RENAME_NOREPLACE)` on Linux, and [MoveFileExW without replacement flags](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw) on Windows. Only macOS behavior has been executed here.

All entry names are validated before staging to prevent writing outside the staging tree. Files are created exclusively; case/filename collisions fail instead of replacing another entry. A staging write failure does not publish the destination. Normal failure cleanup removes the writer's own temporary tree; interrupted/failed cleanup may leave a staging directory for later recovery.

Copy preserves project identity, schema and raw metadata exactly, including timestamps. It does not migrate schema 1 or update modification metadata. Unix symlink targets are reproduced without dereferencing. Non-Unix copies containing symlinks currently fail before publication rather than silently dropping them. Filesystem attributes/ACLs/resource forks and full power-loss durability are not implemented. In particular, file-content flushing and atomic publication do not prove directory-entry durability across power failure.

57 core tests pass: all eight categories/13 presets and schema 1 publish/reopen with opaque-entry equality, existing destinations remain intact, simulated late staging failure leaves no published copy, and two concurrent publications produce exactly one complete winner. Root/prototype Clippy and prototype logic tests pass. Slint now exposes experimental project controls (see below). In-place save, external-change detection, migration and Windows/Linux runtime are not certified by this addition.

## Slint project recovery baseline (evaluation)

Project UI integration keeps format/preset fixed and saves complete new package copies. Opening an existing project must first establish a separate full baseline in the prototype's local recovery session. Recovery schema 2 pairs `scratch.json` (`source`, `category`, `project: true`) with a fixed sibling `project-baseline.fountage`; schema 1 remains source-only. Missing baselines and source/category validation failures abort opening/recovery before the current editor draft is adopted. Recovered project copies preserve unknown metadata and attachment bytes; saving remains exclusive new-copy publication. This is separate from Fountage package schema 2 and public v001.

Native dialogs, in-place saving, migration/timestamps, external-link relocation, concurrent changes and large-package responsiveness remain unverified. Recovery sessions are retained without pruning. Cross-platform runtime and full document-system parity are not established.

## Shared Rust creation (2026-10-14)

Project::new(Format) creates a new schema2 directory-package snapshot with UUIDv4 identity, RFC3339 UTC creation/modification timestamps, creatorIdentifier, correct documentFormat/sourceFile/type, the category's first existing layout preset, empty source and assets directory, and empty reserved collections. Existing packages never pass through this constructor. New creation does not migrate existing metadata, infer an academic edition or implement reserved features. Project::save_copy remains exclusive publication; source-only editing preserves original metadata bytes, including the initial timestamps, until a separate metadata/timestamp save protocol is implemented. All8 new categories roundtrip through the shared package writer/reader in tests. Slint creates a complete recovery baseline before adopting a new project.


## Independent Shelf ordering

The version-1 Shelf record retains `order` as the normal order of item identities. An optional `pinnedOrder` array stores the display order of pinned items independently. Without that field, pinned items use their existing relative base order. Unpinned items always use `order`; rearranging pins therefore cannot change where an item returns when it is unpinned. Current Rust and Swift implementations read and preserve this optional field. Document and public release version numbers do not change for this additive record field.

Both arrays contain unique strings. Unknown, dangling and currently unpinned identities in `pinnedOrder` are retained as reserved identities, while only supported pinned items participate in the displayed pinned group. New items cannot claim identities reserved by either array. Removing a known item removes only that identity from both arrays. Malformed `pinnedOrder` values disable Shelf writes rather than being normalized or discarded. Other unknown record fields remain opaque.

The Slint checklist-only view moves checklists within their own pinned/unpinned group and kind. It exchanges selected item identities in the relevant order array while preserving the slots of invisible notes, snippets and unknown entries. The complete expected known-item order and metadata snapshot are checked before adoption. UI pin/order history and recovery retain the complete project tree.


### Shelf type visibility

The optional version-1 Shelf `visibility` object stores Boolean `note`, `snippet` and `checklist` entries. Missing entries mean shown. Slint’s individual type action changes only that entry; Show All/Hide All changes these three implemented types in one metadata transaction. The setting does not delete or reorder records, change their pin state, or alter manuscript/output text. Unknown visibility keys and values remain opaque. A non-object visibility value or non-Boolean known key disables Shelf editing rather than silently replacing it.

Visibility is saved with the project, supports metadata Undo/Redo and publishes a complete recovery snapshot before adoption. Both the Swift Shelf sheet/inspector and Slint Shelf apply these settings. Swift validates Boolean flags through JSON decoding so numeric 0/1 cannot be mistaken for valid flags. Swift native save updates the known `modifiedAt` timestamp and may create the standard empty `assets` directory; these writer behaviors are distinct from visibility changes. Public version and document schema stay unchanged.


### Quick Note Entry draft

The optional version-1 Shelf `quickEntry` object has `text` (String, default empty) and `visible` (Boolean, default true). Unknown fields remain opaque. A malformed known value makes Shelf writes unavailable rather than discarding the draft. Null characters are invalid in known text. This draft is separate from manuscript source and stored Shelf records. The Swift native inspector binds its multiline field to this document metadata, so ordinary project save/autosave/versions include unfinished input. Slint now presents a multiline Shelf field with guarded capture, independent visibility, metadata history and recovery; failed writes retain pending input until retry. Normal typing coalescing and full accessibility/input-method/performance acceptance remain open.

Shared operations set the exact draft against an expected byte-identical previous string, change entry visibility without clearing text, or capture the draft into a new unpinned `note` titled `Note`. Capture creates the record, prepends its identity to normal `order`, and clears draft text in one candidate transaction. Existing normal-order identities retain relative order; pinnedOrder and all prior records/unknown fields stay unchanged. Empty/whitespace-only capture is a no-op; stale/duplicate/invalid captures reject atomically. Stored item-type visibility is independent of entry visibility. Swift document Undo restores both the prior draft and prior record set after capture. Public/document versions remain unchanged.


### Slint metadata recovery revisions (internal schema 3)

Recovery schema 3 is internal prototype storage, separate from `.fountage` package schema 2, Shelf schema 1 and public v001. It contains `schema: 3`, `project: true`, the exact current `source`, `category`, and `project_info` (a String containing the entire raw current `info.json`). Each `metadata-*.json` revision sits beside one shared immutable `project-baseline.fountage` in the same recovery session directory. Keep the baseline and revision files together when moving or archiving recovery data. Revision files never contain an arbitrary baseline path.

The baseline retains all original attachments, extra entries and link targets. Recovery replaces only its raw info.json and manuscript from the chosen revision, validates the complete project and requires matching identity/category/document schema before adoption. Source editing of the current revision keeps its metadata overlay. Existing source-only schema 1 and full-project schema 2 snapshots remain readable; older app builds reject schema 3 rather than silently dropping the metadata overlay. New immutable revision publication uses exclusive temporary-file publication and fsync before project/history/checkpoint adoption.

Reuse requires the same recovery root, project identity/category/schema and unchanged non-source/non-metadata entries. The app checks cached baseline file stamps (kind, size, modification time, link target and Unix file identity). A changed/missing baseline or changed attachments causes a fresh full baseline copied from the trusted candidate; a failed publication adopts nothing. Source updates against a changed shared baseline report failure and keep the draft available for saving. These stamps detect ordinary filesystem changes; they are not cryptographic tamper resistance or a power-failure guarantee. Recovery sessions/revisions remain retained without automatic pruning.

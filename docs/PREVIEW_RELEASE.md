# Fountage v001 — development preview

Fountage by Baz Studios is a writing studio under active development. This prerelease uses the shared Rust engine and the Slint interface; the Swift macOS app remains a development reference. It is not yet at Highland feature parity.

## Downloads

- macOS arm64: Apple Silicon Macs, macOS 14 or newer.
- macOS x64: Intel Macs, macOS 14 or newer.
- Windows x64: 64-bit Windows 10/11. Run the per-user setup installer, or extract the entire portable ZIP and run Fountage.exe.

Mac downloads contain Fountage.app. The preview is ad-hoc signed, not Developer ID signed or notarized. Windows binaries are not Authenticode signed. Operating-system trust prompts may appear. Release assets include SHA-256 checksums.

## What to expect

Eight writing categories, starting layouts, source editing, an experimental formatted screenplay surface, structure navigation, title-page and scene-number tools, Shelf notes/snippets/checklists/Quick Notes, light/dark modes and independent white paper. The public version stays v001 until the owner authorizes a change.

Saving currently creates a new copy. Keep original manuscripts and backups; use copies while testing. Projects contain source, metadata and attachments. On Windows, project packages containing symbolic-link attachments are rejected rather than saved with missing data. Ordinary projects without such links are supported. The Slint preview does not yet provide the native Swift app's full pagination, printing/PDF, goals or document-system features.

Launch the app, then use Open Project to select a .fountage project (a package on macOS, a folder on Windows). Finder/Explorer double-click document opening is not implemented in this preview.

## Verification limits

Build/test logs are available in GitHub Actions. Generated-editor tests do not prove native input-method, screen-reader or performance parity. Windows release checks cover compilation, core tests, ordinary project creation/save/reopen and actual window startup/close; they do not certify every editing workflow. Mac and Windows behavior must be verified independently. See docs/HIGHLAND_PARITY.md in the repository for the living requirements and limitations.

Source and issues: https://github.com/Baz-Studios-LLC/Fountage

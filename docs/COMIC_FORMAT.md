# Fountage Standard for comic scripts

Fountage by Baz Studios

A consistent starting point for writing a comic and sharing it with an artist or letterer. Fountage Standard is a house style built from familiar full-script conventions. Its purpose is consistency and approachability. It should remain immediately recognizable to writers, artists and letterers who use other comic-script layouts.

## Writing conventions

1. Begin with the title, issue number (if applicable), and author.
2. Mark each story page with `PAGE 1`, `PAGE 2`, and so on. An optional panel count, such as `PAGE 1 (2 panels)`, helps the artist plan the page.
3. Number panels within each page, starting again at `Panel 1` on a new page.
4. Put the artist's description immediately beneath the panel heading. Describe the moment to draw: setting, characters, visible action, and relevant visual details.
5. Introduce dialogue with the character's name in uppercase followed by a colon. Put the spoken words on the next line. Keep each intended balloon in its own block, in reading order.
6. Use `CAPTION:` for caption text and `SFX:` for drawn sound effects. Put the actual lettering on the next line.
7. Separate descriptions, lettering blocks, and panel headings with blank lines. Keep a cue and its lettering together without an intervening blank line.

A story-page heading describes a page in the comic. It is separate from the physical page count of a printed script.

## Example

```text
Title: Signal
Issue: 1
Author: Baz Studios

PAGE 1 (2 panels)

Panel 1
A silent observatory beneath a sky full of stars. One window glows.

CAPTION:
Three minutes before dawn.

Panel 2
Mara leans toward a radio. Its needle begins to move.

MARA:
There you are.

SFX:
CLICK

PAGE 2 (1 panel)

Panel 1
Beyond the glass, every star goes out.

MARA:
That was not the plan.
```

## Current app behavior

The Comic Script chooser selects Fountage Standard by default. The optional starting outline demonstrates these elements. Page and panel headings populate the sidebar. Dialogue, captions and sound effects use visually distinct cues, with lettering blocks indented consistently. Character counts track character cues and exclude captions and sound effects. Lettering Friendly offers a flush-left alternative.

The app also recognizes some alternate spellings and numbered cues, but the example above is the recommended house convention. The current parser does not validate page/panel numbering, check the stated panel count, assign balloon numbers automatically, or paginate a production script. Those are future features. Existing projects retain their original source text.

Implementation note: the persisted preset ID remains `comic-full-script`, so existing projects select the same layout after the public label becomes Fountage Standard. This change remains part of app release v001.


## Familiarity and compatibility requirements

Keep ordinary page/panel headings and readable cue labels. No custom tags or proprietary source grammar should be required. Accept common variants where they can be interpreted reliably, while preserving the original source. Continue accepting CAP and CAPTION, character delivery modifiers such as `(whisper)` or `(OP)`, numeric or spelled-out page numbers within the parser's supported range, and optional numbered lettering cues.

[Dark Horse's published guide](https://images.darkhorse.com/darkhorse08/company/submissions/scriptguide.pdf) supports the same basic breakdown: story pages, numbered panels, descriptions, uppercase speaker labels, and separate dialogue/SFX/caption blocks in reading order. Its flush-left lettering convention is available as Lettering Friendly. Its requirements for page headers and physical page breaks belong in export work; they are not implemented by the draft editor.

[J. M. DeMatteis describes a full-script approach learned from his editor at DC](https://www.jmdematteis.com/2013/11/how-do-you-do-that.html), while emphasizing variation between writers. Use this as evidence of familiar structural conventions, not as certification of a current company-wide DC specification.

Validation should include common off-panel/whisper labels, caption aliases, numbered lettering items and source-preserving interchange. Match specific publisher requirements in separately researched templates. Automatic lettering numbering, print pagination and universal publisher import remain future work.

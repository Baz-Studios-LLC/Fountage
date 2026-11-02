//! Stateful formatted-editor transactions, isolated from the production TextInput.
//! A generation tags shaped geometry: stale pointer/edit requests never alter a draft.
use crate::{
    editor_layout::{Context, Document, Position},
    editor_paint::{self, Config, Painting},
    history::History,
};
use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub anchor: usize,
    pub focus: usize,
}
impl Selection {
    fn range(self) -> std::ops::Range<usize> {
        self.anchor.min(self.focus)..self.anchor.max(self.focus)
    }
}
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Motion {
    Left,
    Right,
    WordLeft,
    WordRight,
    Up,
    Down,
    LineStart,
    LineEnd,
    DocumentStart,
    DocumentEnd,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase", deny_unknown_fields)]
pub enum Command {
    Select {
        anchor: usize,
        focus: usize,
    },
    SelectVisual {
        anchor: usize,
        focus: usize,
        anchor_upstream: bool,
        focus_upstream: bool,
    },
    Point {
        x: f32,
        y: f32,
        extend: bool,
    },
    Insert {
        text: String,
    },
    Enter,
    DeleteBackward,
    DeleteForward,
    DeleteWord {
        forward: bool,
    },
    SelectAll,
    Move {
        direction: Motion,
        extend: bool,
    },
    Page {
        pixels: f32,
        extend: bool,
    },
    Undo,
    Redo,
    Reflow {
        width: f32,
    },
    Preedit {
        text: String,
        cursor: Option<[usize; 2]>,
        replacement: Option<[usize; 2]>,
    },
    CommitComposition {
        text: String,
    },
    CancelComposition,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    StaleGeometry,
    InvalidEndpoint,
    InvalidGeometry,
    CompositionActive,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Change {
    pub source_changed: bool,
    pub generation: u64,
}

pub struct Editor {
    context: Context,
    document: Document,
    selection: Selection,
    history: History,
    record_history: bool,
    generation: u64,
    focus_upstream: bool,
    anchor_upstream: bool,
    preferred_x: Option<f32>,
    composition: Option<crate::editor_composition::Marked>,
}
impl Editor {
    pub fn new(source: &str, width: f32) -> Result<Self, Error> {
        Self::with_history(source, width, true)
    }
    pub fn external_history(source: &str, width: f32) -> Result<Self, Error> {
        Self::with_history(source, width, false)
    }
    fn with_history(source: &str, width: f32, record_history: bool) -> Result<Self, Error> {
        let mut context = Context::default();
        let document = context
            .shape(source, width)
            .map_err(|_| Error::InvalidGeometry)?;
        let mut history = History::default();
        if record_history {
            history.reset(source);
        }
        Ok(Self {
            context,
            document,
            selection: Selection {
                anchor: 0,
                focus: 0,
            },
            history,
            record_history,
            generation: 0,
            focus_upstream: false,
            anchor_upstream: false,
            preferred_x: None,
            composition: None,
        })
    }
    /// Adopt an external document transaction while retaining safe immutable shapes.
    /// The application supplies its own Undo/Redo selection after this succeeds.
    pub fn adopt(&mut self, source: &str, width: f32) -> Result<(), Error> {
        let document = self
            .context
            .reshape(source, width, Some(&self.document))
            .map_err(|_| Error::InvalidGeometry)?;
        self.document = document;
        self.composition = None;
        self.history
            .reset(if self.record_history { source } else { "" });
        self.selection = Selection {
            anchor: 0,
            focus: 0,
        };
        self.focus_upstream = false;
        self.anchor_upstream = false;
        self.preferred_x = None;
        self.generation += 1;
        Ok(())
    }
    pub fn display_document(&self) -> &Document {
        self.composition
            .as_ref()
            .map(|c| &c.document)
            .unwrap_or(&self.document)
    }
    pub fn display_selection(&self) -> Selection {
        self.composition
            .as_ref()
            .map(|c| c.selection)
            .unwrap_or(self.selection)
    }
    pub fn display_position(&self) -> Position {
        if self.composition.is_some() {
            Position {
                byte: self.display_selection().focus,
                upstream: false,
            }
        } else {
            self.focus_position()
        }
    }
    pub fn ime_preedit(&self) -> Option<(&str, usize)> {
        self.composition
            .as_ref()
            .map(|c| (c.text.as_str(), c.replacement.start))
    }
    pub fn marked_range(&self) -> Option<[usize; 2]> {
        self.composition.as_ref().map(|c| c.underline)
    }
    pub fn composing(&self) -> bool {
        self.composition.is_some()
    }
    fn cancel_composition(&mut self) {
        if self.composition.take().is_some() {
            self.generation += 1;
        }
    }
    pub fn document(&self) -> &Document {
        &self.document
    }
    pub fn source(&self) -> &str {
        self.document.source()
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn selection(&self) -> Selection {
        self.selection
    }
    pub fn history_selection(&self) -> crate::history::SelectionState {
        crate::history::SelectionState {
            anchor: self.selection.anchor,
            focus: self.selection.focus,
            anchor_upstream: self.anchor_upstream,
            focus_upstream: self.focus_upstream,
        }
    }
    pub fn focus_position(&self) -> Position {
        Position {
            byte: self.selection.focus,
            upstream: self.focus_upstream,
        }
    }
    fn place(&mut self, position: Position, extend: bool) {
        self.history.boundary();
        if !extend {
            self.selection.anchor = position.byte;
            self.anchor_upstream = position.upstream;
        }
        self.selection.focus = position.byte;
        self.focus_upstream = position.upstream;
    }
    fn move_caret(&mut self, direction: Motion, extend: bool) -> Result<(), Error> {
        let position = self.focus_position();
        let vertical = matches!(direction, Motion::Up | Motion::Down);
        let target = if !extend
            && !self.selection.range().is_empty()
            && matches!(direction, Motion::Left | Motion::Right)
        {
            let anchor = Position {
                byte: self.selection.anchor,
                upstream: self.anchor_upstream,
            };
            let a = self
                .document
                .caret_at(anchor)
                .ok_or(Error::InvalidEndpoint)?
                .rect;
            let b = self
                .document
                .caret_at(position)
                .ok_or(Error::InvalidEndpoint)?
                .rect;
            let anchor_first = (a.y0, a.x0) < (b.y0, b.x0);
            if anchor_first == matches!(direction, Motion::Left) {
                anchor
            } else {
                position
            }
        } else {
            match direction {
                Motion::WordLeft | Motion::WordRight => self
                    .document
                    .word(position, matches!(direction, Motion::WordRight))
                    .ok_or(Error::InvalidEndpoint)?,
                Motion::Left | Motion::Right => self
                    .document
                    .horizontal(position, matches!(direction, Motion::Right))
                    .ok_or(Error::InvalidEndpoint)?,
                Motion::Up | Motion::Down => {
                    let x = self.preferred_x.unwrap_or(
                        self.document
                            .caret_at(position)
                            .ok_or(Error::InvalidEndpoint)?
                            .rect
                            .x0 as f32,
                    );
                    self.preferred_x = Some(x);
                    self.document
                        .vertical(position, matches!(direction, Motion::Down), x)
                        .ok_or(Error::InvalidEndpoint)?
                }
                Motion::LineStart | Motion::LineEnd => self
                    .document
                    .line_edge(position, matches!(direction, Motion::LineEnd))
                    .ok_or(Error::InvalidEndpoint)?,
                Motion::DocumentStart => Position::default(),
                Motion::DocumentEnd => Position {
                    byte: self.source().len(),
                    upstream: true,
                },
            }
        };
        if !vertical {
            self.preferred_x = None;
        }
        self.place(target, extend);
        Ok(())
    }
    pub fn history_available(&self) -> (bool, bool) {
        self.history.availability(self.source())
    }
    pub fn paint(&self, dark: bool) -> Result<Painting, String> {
        editor_paint::paint(
            self.display_document(),
            &Config {
                width: self.document.width + 64.,
                dark,
                caret: Some(self.display_selection().focus),
                caret_upstream: self.display_position().upstream,
                selection: Some([
                    self.display_selection().anchor,
                    self.display_selection().focus,
                ]),
                marked: self.marked_range(),
                ..Config::default()
            },
        )
    }
    fn valid(&self, byte: usize) -> bool {
        self.document.caret(byte).is_some_and(|c| c.byte == byte)
    }
    fn select(&mut self, anchor: usize, focus: usize) -> Result<(), Error> {
        if !self.valid(anchor) || !self.valid(focus) {
            return Err(Error::InvalidEndpoint);
        }
        self.history.boundary();
        self.selection = Selection { anchor, focus };
        self.focus_upstream = false;
        self.anchor_upstream = false;
        self.preferred_x = None;
        Ok(())
    }
    fn replace(&mut self, range: std::ops::Range<usize>, text: &str) -> Result<bool, Error> {
        let source = self
            .document
            .replace(range.start, range.end, text)
            .ok_or(Error::InvalidEndpoint)?;
        if source == self.source() {
            return Ok(false);
        }
        let document = self
            .context
            .reshape(&source, self.document.width, Some(&self.document))
            .map_err(|_| Error::InvalidGeometry)?;
        // Inserting a combining mark or ZWJ can join its neighbours. Never leave
        // the next deletion at an interior Unicode boundary after reanalysis.
        let requested = range.start + text.len();
        let requested = document.boundaries()[document
            .boundaries()
            .partition_point(|&b| b <= requested)
            .saturating_sub(1)];
        let caret = document
            .caret(requested)
            .ok_or(Error::InvalidEndpoint)?
            .byte;
        self.history.boundary(); // a single replacement/paste/deletion is one transaction
        if self.record_history {
            self.history
                .observe_visual_selection(self.document.source(), self.history_selection());
            self.history.record_visual_selected(
                &source,
                crate::history::SelectionState::plain(caret, caret),
            );
        }
        self.document = document;
        self.selection = Selection {
            anchor: caret,
            focus: caret,
        };
        self.focus_upstream = false;
        self.anchor_upstream = false;
        self.preferred_x = None;
        self.generation += 1;
        Ok(true)
    }
    fn step_history(&mut self, redo: bool) -> Result<bool, Error> {
        if !self.record_history {
            return Ok(false);
        }
        // Stage history too: failed shaping must not consume an undo entry.
        let mut history = self.history.clone();
        let Some((source, selection)) = history.step_visual(self.source(), redo) else {
            return Ok(false);
        };
        let document = self
            .context
            .reshape(&source, self.document.width, Some(&self.document))
            .map_err(|_| Error::InvalidGeometry)?;
        let anchor = document
            .caret_at(Position {
                byte: selection.anchor,
                upstream: selection.anchor_upstream,
            })
            .ok_or(Error::InvalidEndpoint)?
            .byte;
        let focus = document
            .caret_at(Position {
                byte: selection.focus,
                upstream: selection.focus_upstream,
            })
            .ok_or(Error::InvalidEndpoint)?
            .byte;
        self.history = history;
        self.document = document;
        self.selection = Selection { anchor, focus };
        self.focus_upstream = selection.focus_upstream;
        self.anchor_upstream = selection.anchor_upstream;
        self.preferred_x = None;
        self.generation += 1;
        Ok(true)
    }
    fn newline(&self) -> &'static str {
        // Preserve the local paragraph's ending, then nearest preceding ending.
        // Paste stays byte-exact; only a newly typed Return uses this convention.
        let position = self.selection.range().start;
        if let Some(p) = self
            .document
            .paragraphs
            .iter()
            .find(|p| p.source.start <= position && position <= p.content.end)
        {
            let ending = &self.source()[p.content.end..p.source.end];
            if ending == "\r\n" {
                return "\r\n";
            }
            if ending == "\n" {
                return "\n";
            }
        }
        if let Some(index) = self.source()[..position].rfind('\n')
            && index > 0
            && self.source().as_bytes()[index - 1] == b'\r'
        {
            return "\r\n";
        }
        "\n"
    }
    pub fn apply(&mut self, expected_generation: u64, command: Command) -> Result<Change, Error> {
        if expected_generation != self.generation {
            return Err(Error::StaleGeometry);
        }
        if self.composing()
            && !matches!(
                command,
                Command::Preedit { .. }
                    | Command::CommitComposition { .. }
                    | Command::CancelComposition
                    | Command::Reflow { .. }
                    | Command::Undo
            )
        {
            return Err(Error::CompositionActive);
        }
        let source_changed = match command {
            Command::Preedit {
                text,
                cursor,
                replacement,
            } => {
                if text.is_empty() {
                    self.cancel_composition();
                } else {
                    let replacement = replacement
                        .map(|[a, b]| a.min(b)..a.max(b))
                        .or_else(|| self.composition.as_ref().map(|c| c.replacement.clone()))
                        .unwrap_or(self.selection.range());
                    let previous = self.composition.as_ref().map(|c| &c.document);
                    let marked = crate::editor_composition::Marked::build(
                        &mut self.context,
                        &self.document,
                        replacement,
                        text,
                        cursor,
                        self.document.width,
                        previous,
                    )?;
                    self.history.boundary();
                    self.composition = Some(marked);
                    self.generation += 1;
                }
                false
            }
            Command::CommitComposition { text } => {
                let range = self
                    .composition
                    .as_ref()
                    .map(|c| c.replacement.clone())
                    .unwrap_or(self.selection.range());
                let requested = range.start + text.len();
                let changed = self.replace(range, &text)?;
                if !changed {
                    let byte = self.document.boundaries()[self
                        .document
                        .boundaries()
                        .partition_point(|&b| b <= requested)
                        .saturating_sub(1)];
                    let caret = self
                        .document
                        .caret(byte)
                        .ok_or(Error::InvalidEndpoint)?
                        .byte;
                    self.select(caret, caret)?;
                }
                self.cancel_composition();
                changed
            }
            Command::CancelComposition => {
                self.cancel_composition();
                false
            }
            Command::Select { anchor, focus } => {
                self.select(anchor, focus)?;
                false
            }
            Command::SelectVisual {
                anchor,
                focus,
                anchor_upstream,
                focus_upstream,
            } => {
                self.select(anchor, focus)?;
                self.anchor_upstream = anchor_upstream;
                self.focus_upstream = focus_upstream;
                false
            }
            Command::Point { x, y, extend } => {
                let position = self
                    .document
                    .hit_position(x, y)
                    .ok_or(Error::InvalidGeometry)?;
                self.place(position, extend);
                self.preferred_x = None;
                false
            }
            Command::Move { direction, extend } => {
                self.move_caret(direction, extend)?;
                false
            }
            Command::Page { pixels, extend } => {
                if !pixels.is_finite() || pixels.abs() < 1. {
                    return Err(Error::InvalidGeometry);
                }
                let caret = self
                    .document
                    .caret_at(self.focus_position())
                    .ok_or(Error::InvalidEndpoint)?
                    .rect;
                let x = self.preferred_x.unwrap_or(caret.x0 as f32);
                // Whole visual lines avoid half-line rounding drift when paging back.
                let line_height = (caret.y1 - caret.y0).max(1.) as f32;
                let distance =
                    (pixels.abs() / line_height).floor().max(1.) * line_height * pixels.signum();
                let y =
                    ((caret.y0 + caret.y1) as f32 * 0.5 + distance).clamp(0., self.document.height);
                let target = self
                    .document
                    .hit_position(x, y)
                    .ok_or(Error::InvalidGeometry)?;
                self.preferred_x = Some(x);
                self.place(target, extend);
                false
            }
            Command::SelectAll => {
                self.select(0, self.source().len())?;
                false
            }
            Command::Insert { text } => self.replace(self.selection.range(), &text)?,
            Command::Enter => self.replace(self.selection.range(), self.newline())?,
            Command::DeleteBackward | Command::DeleteForward => {
                let mut range = self.selection.range();
                if range.is_empty() {
                    let boundaries = self.document.boundaries();
                    let index = boundaries
                        .binary_search(&range.start)
                        .map_err(|_| Error::InvalidEndpoint)?;
                    if matches!(command, Command::DeleteBackward) && index > 0 {
                        range.start = boundaries[index - 1];
                    }
                    if matches!(command, Command::DeleteForward) && index + 1 < boundaries.len() {
                        range.end = boundaries[index + 1];
                    }
                }
                self.replace(range, "")?
            }
            Command::DeleteWord { forward } => {
                let mut range = self.selection.range();
                if range.is_empty() {
                    let target = self
                        .document
                        .deletion_word_boundary(self.focus_position(), forward)
                        .ok_or(Error::InvalidEndpoint)?;
                    range = target.min(range.start)..target.max(range.end);
                }
                self.replace(range, "")?
            }
            Command::Undo => {
                if self.composing() {
                    self.cancel_composition();
                    false
                } else {
                    self.step_history(false)?
                }
            }
            Command::Redo => self.step_history(true)?,
            Command::Reflow { width } => {
                if !width.is_finite() {
                    return Err(Error::InvalidGeometry);
                }
                if width != self.document.width {
                    let source = self.source().to_owned();
                    let document = self
                        .context
                        .reshape(&source, width, Some(&self.document))
                        .map_err(|_| Error::InvalidGeometry)?;
                    let composition = self
                        .composition
                        .as_ref()
                        .map(|c| {
                            crate::editor_composition::Marked::build(
                                &mut self.context,
                                &document,
                                c.replacement.clone(),
                                c.text.clone(),
                                c.cursor,
                                width,
                                Some(&c.document),
                            )
                        })
                        .transpose()?;
                    self.document = document;
                    self.composition = composition;
                    self.preferred_x = None;
                    self.generation += 1;
                }
                false
            }
        };
        Ok(Change {
            source_changed,
            generation: self.generation,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn apply(editor: &mut Editor, command: Command) -> Change {
        editor.apply(editor.generation(), command).unwrap()
    }
    fn select(editor: &mut Editor, anchor: usize, focus: usize) {
        apply(editor, Command::Select { anchor, focus });
    }
    #[test]
    fn styled_edit_reflow_history_and_exclusive_save_preserve_exact_source() {
        let source = "Title: QA\r\n\r\nINT. ROOM - DAY\r\n\r\nMARA\r\nZoë _waits_ beside the antenna.\r\n\r\nEXT. ROOF - NIGHT\r\n";
        let mut editor = Editor::new(source, 600.).unwrap();
        let start = source.find("MARA").unwrap();
        let end = source.find("EXT.").unwrap();
        select(&mut editor, end, start);
        apply(
            &mut editor,
            Command::Insert {
                text: "NINA\r\nA _new_ transmission arrives.\r\n\r\n".into(),
            },
        );
        let edited = editor.source().to_owned();
        assert!(
            editor
                .document
                .paragraphs
                .iter()
                .any(|p| p.kind == fountage_core::writing::Kind::Character
                    && p.x > 0.
                    && &edited[p.content.clone()] == "NINA")
        );
        let selection = editor.selection();
        let generation = editor.generation();
        assert!(!apply(&mut editor, Command::Reflow { width: 300. }).source_changed);
        assert_eq!(editor.selection(), selection);
        assert_eq!(editor.source(), edited);
        assert_eq!(editor.generation(), generation + 1);
        let picture = editor.paint(false).unwrap();
        assert!(picture.metrics["underlineRuns"].as_u64().unwrap() > 0);
        assert_eq!(editor.history_available(), (true, false));
        apply(&mut editor, Command::Undo);
        assert_eq!(editor.source(), source);
        apply(&mut editor, Command::Redo);
        assert_eq!(editor.source(), edited);
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("Edited.fountain");
        crate::save_copy(&path, editor.source()).unwrap();
        assert!(crate::save_copy(&path, "must never overwrite").is_err());
        let reopened = crate::source_draft::read(&path).unwrap();
        assert_eq!(reopened, edited);
        let editor = Editor::new(&reopened, 300.).unwrap();
        assert_eq!(editor.source(), edited);
        assert_eq!(editor.history_available(), (false, false));
    }
    #[test]
    fn deletion_treats_combining_zwj_emoji_and_crlf_as_indivisible() {
        for cluster in ["e\u{301}", "👩‍💻", "🇯🇵", "\r\n", "🌙", "क्‍ष"] {
            let source = format!("a{cluster}b");
            let mut editor = Editor::new(&source, 600.).unwrap();
            select(&mut editor, 1 + cluster.len(), 1 + cluster.len());
            apply(&mut editor, Command::DeleteBackward);
            assert_eq!(editor.source(), "ab", "backspace: {cluster:?}");
            assert_eq!(
                editor.selection(),
                Selection {
                    anchor: 1,
                    focus: 1
                }
            );
            apply(&mut editor, Command::Undo);
            assert_eq!(editor.source(), source);
            select(&mut editor, 1, 1);
            apply(&mut editor, Command::DeleteForward);
            assert_eq!(editor.source(), "ab", "forward delete: {cluster:?}");
        }
        let mut editor = Editor::new("e", 600.).unwrap();
        select(&mut editor, 1, 1);
        apply(
            &mut editor,
            Command::Insert {
                text: "\u{301}".into(),
            },
        );
        assert_eq!(editor.source(), "e\u{301}");
        apply(&mut editor, Command::DeleteBackward);
        assert_eq!(editor.source(), "");
        apply(&mut editor, Command::Undo);
        assert_eq!(editor.source(), "e\u{301}");
        apply(&mut editor, Command::Undo);
        assert_eq!(editor.source(), "e");
    }
    #[test]
    fn stale_geometry_invalid_requests_and_reflow_never_mutate_text_or_history() {
        let source = "Zoë e\u{301} 👩‍💻\r\n";
        let mut editor = Editor::new(source, 600.).unwrap();
        for offset in [
            source.find('ë').unwrap() + 1,
            source.find("e\u{301}").unwrap() + 1,
            source.find("👩").unwrap() + 4,
            source.len() - 1,
            usize::MAX,
        ] {
            assert_eq!(
                editor.apply(
                    0,
                    Command::Select {
                        anchor: 0,
                        focus: offset
                    }
                ),
                Err(Error::InvalidEndpoint)
            );
        }
        assert_eq!(
            editor.apply(
                0,
                Command::Point {
                    x: f32::NAN,
                    y: 0.,
                    extend: false
                }
            ),
            Err(Error::InvalidGeometry)
        );
        for width in [0., f32::NAN, 10_001.] {
            assert_eq!(
                editor.apply(0, Command::Reflow { width }),
                Err(Error::InvalidGeometry)
            );
        }
        assert_eq!(editor.source(), source);
        assert_eq!(
            editor.selection(),
            Selection {
                anchor: 0,
                focus: 0
            }
        );
        assert_eq!(editor.history_available(), (false, false));
        apply(&mut editor, Command::Reflow { width: 300. });
        assert_eq!(
            editor.apply(
                0,
                Command::Insert {
                    text: "stale".into()
                }
            ),
            Err(Error::StaleGeometry)
        );
        assert_eq!(editor.source(), source);
        assert_eq!(editor.history_available(), (false, false));
        apply(&mut editor, Command::Undo);
        assert_eq!(editor.source(), source);
    }
    #[test]
    fn pointer_drag_uses_indented_shaping_and_preserves_anchor() {
        let source = "INT. ROOM - DAY\n\nMARA\nA long line of dialogue that wraps over several shaped lines in this narrow column.\n";
        let mut editor = Editor::new(source, 300.).unwrap();
        let start = source.find("MARA").unwrap();
        let end = source.find("dialogue").unwrap();
        let a = editor.document.caret(start).unwrap().rect;
        let b = editor.document.caret(end).unwrap().rect;
        apply(
            &mut editor,
            Command::Point {
                x: a.x0 as f32 + 0.01,
                y: (a.y0 + 8.) as f32,
                extend: false,
            },
        );
        apply(
            &mut editor,
            Command::Point {
                x: b.x0 as f32 + 0.01,
                y: (b.y0 + 8.) as f32,
                extend: true,
            },
        );
        assert_eq!(
            editor.selection(),
            Selection {
                anchor: start,
                focus: end
            }
        );
        assert!(editor.document.selection_rects(start, end).unwrap().len() > 2);
        assert_eq!(editor.source(), source);
        apply(&mut editor, Command::DeleteBackward);
        assert_eq!(
            editor.source(),
            format!("{}{}", &source[..start], &source[end..])
        );
        apply(&mut editor, Command::Undo);
        assert_eq!(editor.source(), source);
    }
    #[test]
    fn return_uses_local_endings_paste_is_exact_and_new_edits_invalidate_redo() {
        let source = "one\r\ntwo\nthree";
        let mut editor = Editor::new(source, 600.).unwrap();
        select(&mut editor, 3, 3);
        apply(&mut editor, Command::Enter);
        assert_eq!(editor.source(), "one\r\n\r\ntwo\nthree");
        apply(&mut editor, Command::Undo);
        let position = source.find("two").unwrap() + 3;
        select(&mut editor, position, position);
        apply(&mut editor, Command::Enter);
        assert_eq!(editor.source(), "one\r\ntwo\n\nthree");
        apply(&mut editor, Command::Undo);
        apply(&mut editor, Command::SelectAll);
        let paste = "# Exact paste\r\n\n世界\r\n";
        apply(&mut editor, Command::Insert { text: paste.into() });
        assert_eq!(editor.source(), paste);
        assert_eq!(editor.history_available(), (true, false));
        apply(&mut editor, Command::Redo);
        assert_eq!(editor.source(), paste);
        apply(&mut editor, Command::Undo);
        assert_eq!(editor.source(), source);
    }
}

#[cfg(test)]
mod motion_tests {
    use super::*;
    fn run(editor: &mut Editor, command: Command) {
        editor.apply(editor.generation(), command).unwrap();
    }
    fn move_to(editor: &mut Editor, direction: Motion, extend: bool) {
        run(editor, Command::Move { direction, extend });
    }
    #[test]
    fn soft_wrap_affinity_survives_line_end_and_right_then_left() {
        let source = "INT. ROOM - DAY\r\n\r\nMARA\r\nOne two three four five six seven eight nine ten eleven twelve thirteen.\r\n";
        let mut editor = Editor::new(source, 300.).unwrap();
        let start = source.find("One").unwrap();
        run(
            &mut editor,
            Command::Select {
                anchor: start,
                focus: start,
            },
        );
        move_to(&mut editor, Motion::LineEnd, false);
        let edge = editor.focus_position();
        assert!(edge.upstream);
        assert!(edge.byte > start && edge.byte < source.len() - 2);
        let above = editor.document.caret_at(edge).unwrap().rect;
        move_to(&mut editor, Motion::Right, false);
        let next = editor.focus_position();
        assert_eq!(
            next.byte, edge.byte,
            "wrap crossing changes affinity before consuming a character"
        );
        assert!(!next.upstream);
        let below = editor.document.caret_at(next).unwrap().rect;
        assert!(below.y0 > above.y0);
        move_to(&mut editor, Motion::Left, false);
        assert_eq!(editor.focus_position(), edge);
        assert_eq!(editor.paint(false).unwrap().metrics["caretUpstream"], true);
        assert_eq!(editor.source(), source);
        assert_eq!(editor.history_available(), (false, false));
    }
    #[test]
    fn visual_traversal_crosses_rtl_runs_without_splitting_graphemes_or_crlf() {
        let source = "!ab שלום العربية cd e\u{301} 👩‍💻\r\nNext\r\n";
        let mut editor = Editor::new(source, 600.).unwrap();
        let mut seen = std::collections::BTreeSet::new();
        let mut had_reverse_source_step = false;
        for _ in 0..200 {
            let before = editor.focus_position();
            assert!(
                seen.insert((before.byte, before.upstream)),
                "visual navigation loop at {before:?}"
            );
            move_to(&mut editor, Motion::Right, false);
            let after = editor.focus_position();
            assert!(
                editor
                    .document
                    .boundaries()
                    .binary_search(&after.byte)
                    .is_ok()
            );
            assert!(editor.valid(after.byte));
            had_reverse_source_step |= after.byte < before.byte;
            if before == after {
                break;
            }
        }
        assert_eq!(editor.selection.focus, source.len());
        assert!(
            had_reverse_source_step,
            "RTL movement must follow glyph order, not byte order"
        );
        assert_eq!(editor.source(), source);
        assert_eq!(editor.history_available(), (false, false));
    }
    #[test]
    fn vertical_navigation_retains_column_across_short_lines_and_indents() {
        let source = "!A long body line with enough words to choose a late column.\r\nx\r\n!Another long line with enough words to restore the original column.\r\n\r\nMARA\r\nDialogue with enough words to wrap across several narrow lines.\r\n";
        let mut editor = Editor::new(source, 600.).unwrap();
        let start = source.find("enough").unwrap();
        run(
            &mut editor,
            Command::Select {
                anchor: start,
                focus: start,
            },
        );
        let original = editor
            .document
            .caret_at(editor.focus_position())
            .unwrap()
            .rect;
        move_to(&mut editor, Motion::Down, false);
        let short = editor
            .document
            .caret_at(editor.focus_position())
            .unwrap()
            .rect;
        assert!(short.x0 < original.x0);
        move_to(&mut editor, Motion::Down, false);
        let restored = editor
            .document
            .caret_at(editor.focus_position())
            .unwrap()
            .rect;
        assert!(
            (restored.x0 - original.x0).abs() < 10.,
            "restore preferred x: {original:?} -> {restored:?}"
        );
        move_to(&mut editor, Motion::Up, true);
        move_to(&mut editor, Motion::Up, true);
        assert_eq!(editor.selection.focus, start);
        assert!(editor.selection.anchor > start);
        assert_eq!(editor.source(), source);
        assert_eq!(editor.history_available(), (false, false));
        move_to(&mut editor, Motion::DocumentEnd, false);
        assert_eq!(editor.selection.focus, source.len());
        move_to(&mut editor, Motion::DocumentStart, true);
        assert_eq!(
            editor.selection,
            Selection {
                anchor: source.len(),
                focus: 0
            }
        );
        run(
            &mut editor,
            Command::Insert {
                text: "Replacement\r\n".into(),
            },
        );
        assert_eq!(editor.source(), "Replacement\r\n");
        run(&mut editor, Command::Undo);
        assert_eq!(editor.source(), source);
    }
    #[test]
    fn pointer_affinity_and_selection_collapse_follow_visible_endpoints() {
        let source =
            "!A first line\r\n\r\nMARA\r\nOne two three four five six seven eight nine ten.\r\n";
        let mut editor = Editor::new(source, 300.).unwrap();
        let start = source.find("One").unwrap();
        run(
            &mut editor,
            Command::Select {
                anchor: start,
                focus: start,
            },
        );
        move_to(&mut editor, Motion::LineEnd, false);
        let end = editor.focus_position();
        let rect = editor.document.caret_at(end).unwrap().rect;
        run(
            &mut editor,
            Command::Point {
                x: rect.x0 as f32 - 0.1,
                y: (rect.y0 + rect.y1) as f32 * 0.5,
                extend: false,
            },
        );
        assert_eq!(editor.focus_position(), end);
        move_to(&mut editor, Motion::LineStart, true);
        assert_eq!(editor.selection.anchor, end.byte);
        assert_eq!(editor.selection.focus, start);
        move_to(&mut editor, Motion::Right, false);
        assert_eq!(
            editor.focus_position(),
            end,
            "Right collapses reversed selection to its visible end"
        );
        assert_eq!(editor.selection.anchor, end.byte);
    }
}

/// Synthetic debug-profile controller timings, not native frame/input latency.
pub fn benchmark() -> serde_json::Value {
    use std::time::Instant;
    let mut results = Vec::new();
    for scenes in [100, 500, 1500] {
        let mut source = String::new();
        for i in 0..scenes {
            source.push_str(&format!("INT. OBSERVATORY {i} - NIGHT\r\n\r\nMARA\r\nZoë watches a _signal_ cross the sky above station {i}, then returns to the café.\r\n\r\n"));
        }
        let mut samples = Vec::new();
        for _ in 0..3 {
            let t = Instant::now();
            let mut editor = Editor::new(&source, 600.).unwrap();
            let cold = t.elapsed().as_secs_f64() * 1000.;
            let end = source.rfind("café").unwrap() + "café".len();
            editor
                .apply(
                    editor.generation(),
                    Command::Select {
                        anchor: end,
                        focus: end,
                    },
                )
                .unwrap();
            let t = Instant::now();
            editor
                .apply(
                    editor.generation(),
                    Command::Insert {
                        text: " tonight".into(),
                    },
                )
                .unwrap();
            let edit = t.elapsed().as_secs_f64() * 1000.;
            let shaped = editor.document().shaped_paragraphs;
            let reused = editor.document().reused_paragraphs;
            let t = Instant::now();
            let painting = editor_paint::paint_live(
                editor.document(),
                &Config {
                    caret: Some(0),
                    ..Config::default()
                },
            )
            .unwrap();
            let paint = t.elapsed().as_secs_f64() * 1000.;
            let t = Instant::now();
            editor.apply(editor.generation(), Command::Undo).unwrap();
            let undo = t.elapsed().as_secs_f64() * 1000.;
            assert_eq!(editor.source(), source);
            let t = Instant::now();
            editor
                .apply(editor.generation(), Command::Reflow { width: 420. })
                .unwrap();
            let resize = t.elapsed().as_secs_f64() * 1000.;
            samples.push(serde_json::json!({"coldMs":cold,"editMs":edit,"paintMs":paint,"undoMs":undo,"resizeMs":resize,"resizeShapedParagraphs":editor.document().shaped_paragraphs,"resizeRewrappedParagraphs":editor.document().rewrapped_paragraphs,"editShapedParagraphs":shaped,"editReusedParagraphs":reused,"visibleParagraphs":painting.metrics["visibleParagraphs"],"paragraphs":editor.document().paragraphs.len()}));
        }
        let mut medians = serde_json::Map::new();
        for key in ["coldMs", "editMs", "paintMs", "undoMs", "resizeMs"] {
            let mut values: Vec<_> = samples.iter().map(|s| s[key].as_f64().unwrap()).collect();
            values.sort_by(f64::total_cmp);
            medians.insert(key.into(), values[1].into());
        }
        results.push(serde_json::json!({"scenes":scenes,"sourceBytes":source.len(),"median":medians,"samples":samples}));
    }
    serde_json::json!({"profile":if cfg!(debug_assertions) {"debug"} else {"release"},"nativeUILatency":false,"paintViewport":"first page, fresh live raster, no PNG","runs":results})
}

#[cfg(test)]
mod adoption_tests {
    use super::*;
    #[test]
    fn external_history_adoption_reuses_shapes_and_rejects_stale_geometry() {
        let source = "INT. ROOM - DAY\r\n\r\nMARA\r\nAn _old_ signal.\r\n";
        let mut editor = Editor::external_history(source, 600.).unwrap();
        let end = source.find("signal").unwrap();
        editor
            .apply(
                0,
                Command::Select {
                    anchor: end,
                    focus: end,
                },
            )
            .unwrap();
        editor
            .apply(
                0,
                Command::Insert {
                    text: "new ".into(),
                },
            )
            .unwrap();
        assert_eq!(editor.document().shaped_paragraphs, 1);
        assert!(editor.document().reused_paragraphs > 0);
        assert_eq!(
            editor.history_available(),
            (false, false),
            "application owns this editor's history"
        );
        let generation = editor.generation();
        editor.adopt(source, 600.).unwrap();
        assert_eq!(editor.document().shaped_paragraphs, 1);
        assert!(editor.document().reused_paragraphs > 0);
        assert_eq!(
            editor.selection(),
            Selection {
                anchor: 0,
                focus: 0
            }
        );
        assert_eq!(
            editor.apply(generation, Command::DeleteForward),
            Err(Error::StaleGeometry)
        );
        assert_eq!(editor.source(), source);
        let generation = editor.generation();
        assert_eq!(
            editor.adopt("must not be adopted", f32::NAN),
            Err(Error::InvalidGeometry)
        );
        assert_eq!(editor.generation(), generation);
        assert_eq!(editor.source(), source);
    }
}

#[cfg(test)]
mod extended_navigation_tests {
    use super::*;
    use unicode_segmentation::UnicodeSegmentation;
    fn run(editor: &mut Editor, command: Command) {
        editor.apply(editor.generation(), command).unwrap();
    }
    #[test]
    fn visual_word_motion_preserves_graphemes_and_can_replace_with_one_undo() {
        let source = "Café e\u{301} waits.\r\n\r\nשלום مرحبا world 🌙";
        let mut editor = Editor::new(source, 340.).unwrap();
        run(
            &mut editor,
            Command::Move {
                direction: Motion::WordRight,
                extend: true,
            },
        );
        assert_eq!(
            editor.selection(),
            Selection {
                anchor: 0,
                focus: "Café".len()
            }
        );
        run(
            &mut editor,
            Command::Insert {
                text: "Moon".into(),
            },
        );
        assert_eq!(editor.source(), source.replacen("Café", "Moon", 1));
        run(&mut editor, Command::Undo);
        assert_eq!(editor.source(), source);
        run(
            &mut editor,
            Command::Move {
                direction: Motion::DocumentStart,
                extend: false,
            },
        );
        let mut seen = std::collections::HashSet::new();
        for _ in 0..80 {
            let position = editor.focus_position();
            assert!(
                position.byte == source.len()
                    || source
                        .grapheme_indices(true)
                        .any(|(byte, _)| byte == position.byte)
            );
            assert!(
                seen.insert((position.byte, position.upstream)),
                "word navigation must not loop"
            );
            run(
                &mut editor,
                Command::Move {
                    direction: Motion::WordRight,
                    extend: false,
                },
            );
            if editor.focus_position() == position {
                break;
            }
        }
        assert!(editor.selection().focus >= source.find("שלום").unwrap());
        assert_eq!(editor.source(), source);
    }
    #[test]
    fn page_motion_tracks_shaped_lines_and_rejects_invalid_or_stale_geometry() {
        let source = "A small sentence with many words. ".repeat(100);
        let mut editor = Editor::new(&source, 300.).unwrap();
        let start = editor
            .document
            .caret_at(editor.focus_position())
            .unwrap()
            .rect;
        run(
            &mut editor,
            Command::Page {
                pixels: 120.,
                extend: false,
            },
        );
        let caret = editor
            .document
            .caret_at(editor.focus_position())
            .unwrap()
            .rect;
        assert!((caret.y0 - start.y0 - 120.).abs() < 30.);
        assert!(editor.selection().focus > 0);
        assert_eq!(editor.history_available(), (false, false));
        let selection = editor.selection();
        for pixels in [f32::NAN, f32::INFINITY, 0., 0.5] {
            assert_eq!(
                editor.apply(
                    editor.generation(),
                    Command::Page {
                        pixels,
                        extend: true
                    }
                ),
                Err(Error::InvalidGeometry)
            );
            assert_eq!(editor.selection(), selection);
        }
        assert_eq!(
            editor.apply(
                editor.generation() + 1,
                Command::Page {
                    pixels: 120.,
                    extend: true
                }
            ),
            Err(Error::StaleGeometry)
        );
        run(
            &mut editor,
            Command::Page {
                pixels: -120.,
                extend: false,
            },
        );
        assert_eq!(
            editor.selection().focus,
            0,
            "start={start:?}, down={caret:?}, back={:?}",
            editor
                .document
                .caret_at(editor.focus_position())
                .unwrap()
                .rect
        );
        run(
            &mut editor,
            Command::Page {
                pixels: 120.,
                extend: true,
            },
        );
        let end = editor.selection().focus;
        assert_eq!(editor.selection().anchor, 0);
        run(
            &mut editor,
            Command::Insert {
                text: "Replacement".into(),
            },
        );
        assert_eq!(editor.source(), format!("Replacement{}", &source[end..]));
        run(&mut editor, Command::Undo);
        assert_eq!(editor.source(), source);
    }
}

#[cfg(test)]
mod word_deletion_tests {
    use super::*;
    fn run(editor: &mut Editor, command: Command) -> Change {
        editor.apply(editor.generation(), command).unwrap()
    }
    #[test]
    fn word_deletion_is_logical_grapheme_safe_atomic_and_source_preserving_on_undo() {
        let source = "Café e\u{301} waits.\r\n\r\nשלום עולם مرحبا 🌙";
        let mut editor = Editor::new(source, 500.).unwrap();
        run(
            &mut editor,
            Command::Select {
                anchor: 5,
                focus: 5,
            },
        );
        run(&mut editor, Command::DeleteWord { forward: false });
        assert_eq!(editor.source(), &source[5..]);
        run(&mut editor, Command::Undo);
        assert_eq!(editor.source(), source);
        run(
            &mut editor,
            Command::Select {
                anchor: 0,
                focus: 0,
            },
        );
        run(&mut editor, Command::DeleteWord { forward: true });
        assert_eq!(editor.source(), &source["Café".len()..]);
        run(&mut editor, Command::Undo);
        let accent = source.find("e\u{301}").unwrap();
        run(
            &mut editor,
            Command::Select {
                anchor: accent + 3,
                focus: accent,
            },
        );
        run(&mut editor, Command::DeleteWord { forward: false });
        assert_eq!(editor.source(), source.replacen("e\u{301}", "", 1));
        run(&mut editor, Command::Undo);
        let word = source.find("עולם").unwrap();
        run(
            &mut editor,
            Command::Select {
                anchor: word + "עולם".len(),
                focus: word + "עולם".len(),
            },
        );
        run(&mut editor, Command::DeleteWord { forward: false });
        assert_eq!(editor.source(), source.replacen("עולם", "", 1));
        run(&mut editor, Command::Undo);
        let paragraph = source.find("שלום").unwrap();
        run(
            &mut editor,
            Command::Select {
                anchor: paragraph,
                focus: paragraph,
            },
        );
        run(&mut editor, Command::DeleteWord { forward: false });
        assert_eq!(
            editor.source(),
            format!("{}{}", &source[..paragraph - 2], &source[paragraph..])
        );
        run(&mut editor, Command::Undo);
        assert_eq!(editor.source(), source);
        run(&mut editor, Command::Redo);
        assert_eq!(
            editor.source(),
            format!("{}{}", &source[..paragraph - 2], &source[paragraph..])
        );
    }
    #[test]
    fn word_deletion_at_edges_or_during_stale_and_composing_states_never_discards_text() {
        let source = "Zoë 🌙";
        let mut editor = Editor::new(source, 500.).unwrap();
        assert!(!run(&mut editor, Command::DeleteWord { forward: false }).source_changed);
        assert_eq!(editor.history_available(), (false, false));
        run(
            &mut editor,
            Command::Select {
                anchor: source.len(),
                focus: source.len(),
            },
        );
        assert!(!run(&mut editor, Command::DeleteWord { forward: true }).source_changed);
        assert_eq!(
            editor.apply(
                editor.generation() + 1,
                Command::DeleteWord { forward: false }
            ),
            Err(Error::StaleGeometry)
        );
        run(
            &mut editor,
            Command::Preedit {
                text: "かな".into(),
                cursor: None,
                replacement: None,
            },
        );
        assert_eq!(
            editor.apply(editor.generation(), Command::DeleteWord { forward: false }),
            Err(Error::CompositionActive)
        );
        assert_eq!(editor.source(), source);
        assert!(editor.composing());
    }
}

#[cfg(test)]
mod history_selection_tests {
    use super::*;
    fn run(editor: &mut Editor, command: Command) {
        editor.apply(editor.generation(), command).unwrap();
    }
    #[test]
    fn controller_history_restores_directed_selection_and_redo_caret() {
        let source = "🌙\r\n\r\nINT. CAFÉ - DAY\r\n\r\n[[Third 世界 note]]\r\n";
        let start = source.find("[[").unwrap();
        let end = source.find("]]").unwrap() + 2;
        for reverse in [false, true] {
            let mut editor = Editor::new(source, 500.).unwrap();
            let selected = if reverse {
                Selection {
                    anchor: end,
                    focus: start,
                }
            } else {
                Selection {
                    anchor: start,
                    focus: end,
                }
            };
            run(
                &mut editor,
                Command::Select {
                    anchor: selected.anchor,
                    focus: selected.focus,
                },
            );
            let replacement = "[[Revised 世界 note]]";
            run(
                &mut editor,
                Command::Insert {
                    text: replacement.into(),
                },
            );
            let expected = source.replacen("[[Third 世界 note]]", replacement, 1);
            assert_eq!(editor.source(), expected);
            run(&mut editor, Command::Undo);
            assert_eq!(editor.source(), source);
            assert_eq!(editor.selection(), selected);
            run(&mut editor, Command::Redo);
            assert_eq!(editor.source(), expected);
            let caret = start + replacement.len();
            assert_eq!(
                editor.selection(),
                Selection {
                    anchor: caret,
                    focus: caret
                }
            );
            run(&mut editor, Command::Undo);
            run(&mut editor, Command::Insert { text: "X".into() });
            assert_eq!(
                editor.source(),
                source.replacen("[[Third 世界 note]]", "X", 1)
            );
            assert!(
                !editor.history_available().1,
                "new replacement invalidates redo"
            );
        }
    }
    #[test]
    fn controller_history_preserves_grapheme_and_crlf_boundaries_after_deletion_and_reflow() {
        let source = "e\u{301} 👩‍🚀\r\nשלום עולם\r\n";
        let start = source.find('👩').unwrap();
        let end = source.find("עולם").unwrap();
        let mut editor = Editor::new(source, 500.).unwrap();
        run(
            &mut editor,
            Command::Select {
                anchor: end,
                focus: start,
            },
        );
        run(&mut editor, Command::DeleteBackward);
        assert_eq!(
            editor.source(),
            format!("{}{}", &source[..start], &source[end..])
        );
        run(&mut editor, Command::Reflow { width: 330. });
        run(&mut editor, Command::Undo);
        assert_eq!(editor.source(), source);
        assert_eq!(
            editor.selection(),
            Selection {
                anchor: end,
                focus: start
            }
        );
        run(&mut editor, Command::Redo);
        assert_eq!(
            editor.selection(),
            Selection {
                anchor: start,
                focus: start
            }
        );
    }
}

#[cfg(test)]
mod history_affinity_tests {
    use super::*;
    fn run(editor: &mut Editor, command: Command) {
        editor.apply(editor.generation(), command).unwrap();
    }
    #[test]
    fn undo_restores_visual_wrap_side_for_caret_and_both_selection_directions() {
        let source = "INT. ROOM - DAY\r\n\r\nMARA\r\nOne two three four five six seven eight nine ten eleven twelve thirteen.\r\n";
        for mode in 0..3 {
            let mut editor = Editor::new(source, 300.).unwrap();
            let start = source.find("One").unwrap();
            run(
                &mut editor,
                Command::Select {
                    anchor: start,
                    focus: start,
                },
            );
            run(
                &mut editor,
                Command::Move {
                    direction: Motion::LineEnd,
                    extend: mode == 1,
                },
            );
            let edge = editor.focus_position();
            assert!(edge.upstream);
            if mode == 2 {
                run(
                    &mut editor,
                    Command::Move {
                        direction: Motion::LineStart,
                        extend: true,
                    },
                );
            }
            let selection = editor.selection();
            let position = editor.focus_position();
            let rectangle = editor.document().caret_at(position).unwrap().rect;
            run(&mut editor, Command::Insert { text: "X".into() });
            run(&mut editor, Command::Undo);
            assert_eq!(editor.source(), source);
            assert_eq!(editor.selection(), selection);
            assert_eq!(editor.focus_position(), position);
            assert_eq!(
                editor
                    .document()
                    .caret_at(editor.focus_position())
                    .unwrap()
                    .rect,
                rectangle
            );
            if mode == 2 {
                run(
                    &mut editor,
                    Command::Move {
                        direction: Motion::Right,
                        extend: false,
                    },
                );
                assert_eq!(editor.focus_position(), edge);
            }
        }
    }
}

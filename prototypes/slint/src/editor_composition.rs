//! Uncommitted composition is a separate display document, never manuscript text.
use crate::{
    editor_layout::{Context, Document},
    editor_session::{Error, Selection},
};
use std::ops::Range;

pub struct Marked {
    pub replacement: Range<usize>,
    pub text: String,
    pub cursor: Option<[usize; 2]>,
    pub document: Document,
    pub selection: Selection,
    pub underline: [usize; 2],
}
impl Marked {
    pub fn build(
        context: &mut Context,
        committed: &Document,
        replacement: Range<usize>,
        text: String,
        cursor: Option<[usize; 2]>,
        width: f32,
        previous: Option<&Document>,
    ) -> Result<Self, Error> {
        let preview = committed
            .replace(replacement.start, replacement.end, &text)
            .ok_or(Error::InvalidEndpoint)?;
        let [a, b] = cursor.unwrap_or([text.len(), text.len()]);
        if !text.is_char_boundary(a) || !text.is_char_boundary(b) {
            return Err(Error::InvalidEndpoint);
        }
        let document = context
            .reshape(&preview, width, previous.or(Some(committed)))
            .map_err(|_| Error::InvalidGeometry)?;
        let floor = |byte| {
            document.boundaries()[document
                .boundaries()
                .partition_point(|&p| p <= byte)
                .saturating_sub(1)]
        };
        let ceil = |byte| {
            document.boundaries()[document
                .boundaries()
                .partition_point(|&p| p < byte)
                .min(document.boundaries().len() - 1)]
        };
        let normalize = |byte| {
            document
                .caret(floor(byte))
                .map(|c| c.byte)
                .ok_or(Error::InvalidEndpoint)
        };
        let selection = Selection {
            anchor: normalize(replacement.start + a)?,
            focus: normalize(replacement.start + b)?,
        };
        let end = ceil(replacement.start + text.len());
        let start_index = document.boundaries().partition_point(|&b| b < end);
        let end = document.boundaries()[start_index..]
            .iter()
            .copied()
            .find(|&b| document.caret(b).is_some_and(|c| c.byte == b))
            .ok_or(Error::InvalidEndpoint)?;
        let underline = [normalize(replacement.start)?, end];
        Ok(Self {
            replacement,
            text,
            cursor,
            document,
            selection,
            underline,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::editor_session::{Command, Editor, Error};
    fn apply(e: &mut Editor, c: Command) {
        e.apply(e.generation(), c).unwrap();
    }
    const SOURCE: &str = "INT. ROOM - DAY\r\n\r\nMARA\r\nAn old signal.\r\n";
    fn selected() -> Editor {
        let mut e = Editor::new(SOURCE, 600.).unwrap();
        let a = SOURCE.find("old").unwrap();
        apply(
            &mut e,
            Command::Select {
                anchor: a + 3,
                focus: a,
            },
        );
        e
    }
    #[test]
    fn preedit_is_visible_but_never_enters_source_history_or_saved_copy() {
        let mut e = selected();
        let selected = e.selection();
        for text in ["に", "にほん", "日本"] {
            apply(
                &mut e,
                Command::Preedit {
                    text: text.into(),
                    cursor: Some([0, text.len()]),
                    replacement: None,
                },
            );
            assert_eq!(e.source(), SOURCE);
            assert_eq!(e.selection(), selected);
            assert_eq!(e.history_available(), (false, false));
            assert!(e.display_document().source().contains(text));
            assert!(
                e.paint(false).unwrap().metrics["markedRects"]
                    .as_u64()
                    .unwrap()
                    > 0
            );
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("While composing.fountain");
        crate::save_copy(&path, e.source()).unwrap();
        assert_eq!(crate::source_draft::read(&path).unwrap(), SOURCE);
        apply(&mut e, Command::CancelComposition);
        assert!(!e.composing());
        assert_eq!(e.display_document().source(), SOURCE);
        assert_eq!(e.selection(), selected);
        assert_eq!(e.paint(false).unwrap().metrics["markedRects"], 0);
        apply(
            &mut e,
            Command::Preedit {
                text: "にほん".into(),
                cursor: None,
                replacement: None,
            },
        );
        apply(
            &mut e,
            Command::CommitComposition {
                text: "日本語".into(),
            },
        );
        let expected = SOURCE.replace("old", "日本語");
        assert_eq!(e.source(), expected);
        assert!(!e.composing());
        apply(&mut e, Command::Undo);
        assert_eq!(e.source(), SOURCE);
        apply(&mut e, Command::Redo);
        assert_eq!(e.source(), expected);
    }
    #[test]
    fn invalid_and_stale_events_preserve_pending_state_and_reflow_does_not_commit() {
        let mut e = selected();
        let old = e.generation();
        apply(
            &mut e,
            Command::Preedit {
                text: "にほん".into(),
                cursor: Some([0, 3]),
                replacement: None,
            },
        );
        let generation = e.generation();
        let preview = e.display_document().source().to_owned();
        assert_eq!(
            e.apply(
                old,
                Command::CommitComposition {
                    text: "stale".into()
                }
            ),
            Err(Error::StaleGeometry)
        );
        assert_eq!(
            e.apply(
                generation,
                Command::Preedit {
                    text: "日本".into(),
                    cursor: Some([1, 1]),
                    replacement: None
                }
            ),
            Err(Error::InvalidEndpoint)
        );
        assert_eq!(
            e.apply(
                generation,
                Command::Insert {
                    text: "must not edit a preview".into()
                }
            ),
            Err(Error::CompositionActive)
        );
        assert_eq!(e.generation(), generation);
        assert_eq!(e.display_document().source(), preview);
        apply(&mut e, Command::Reflow { width: 300. });
        assert!(e.composing());
        assert_eq!(e.display_document().width, 300.);
        assert_eq!(e.source(), SOURCE);
        assert_eq!(e.history_available(), (false, false));
        assert_eq!(
            e.adopt("must not adopt", f32::NAN),
            Err(Error::InvalidGeometry)
        );
        assert!(e.composing());
        assert_eq!(e.source(), SOURCE);
        e.adopt(SOURCE, 300.).unwrap();
        assert!(!e.composing());
    }
    #[test]
    fn combining_preedit_and_undo_cancel_never_split_original_text() {
        let mut e = Editor::new("e\r\n", 600.).unwrap();
        apply(
            &mut e,
            Command::Select {
                anchor: 1,
                focus: 1,
            },
        );
        apply(
            &mut e,
            Command::Preedit {
                text: "\u{301}".into(),
                cursor: None,
                replacement: None,
            },
        );
        assert_eq!(e.display_document().source(), "e\u{301}\r\n");
        assert!(e.paint(false).is_ok());
        apply(
            &mut e,
            Command::CommitComposition {
                text: "\u{301}".into(),
            },
        );
        assert_eq!(e.source(), "e\u{301}\r\n");
        apply(
            &mut e,
            Command::Preedit {
                text: "かな".into(),
                cursor: None,
                replacement: None,
            },
        );
        apply(&mut e, Command::Undo);
        assert!(!e.composing());
        assert_eq!(e.source(), "e\u{301}\r\n");
        apply(&mut e, Command::Undo);
        assert_eq!(e.source(), "e\r\n");
        let mut e = selected();
        apply(
            &mut e,
            Command::Preedit {
                text: "old".into(),
                cursor: None,
                replacement: None,
            },
        );
        apply(&mut e, Command::CommitComposition { text: "old".into() });
        assert_eq!(e.source(), SOURCE);
        assert_eq!(e.selection().anchor, e.selection().focus);
        assert_eq!(e.history_available(), (false, false));
    }
}

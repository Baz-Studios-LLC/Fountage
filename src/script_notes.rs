//! Source-preserving script-note insertion/toggling. Notes remain ordinary text.
use crate::formatting::Edit;
use std::ops::Range;

pub fn edit(source: &str, anchor: usize, focus: usize) -> Result<Option<Edit>, &'static str> {
    let (mut start, mut end) = (anchor.min(focus), anchor.max(focus));
    for byte in [start, end] {
        if !source.is_char_boundary(byte)
            || (byte > 0
                && source.as_bytes().get(byte - 1) == Some(&b'\r')
                && source.as_bytes().get(byte) == Some(&b'\n'))
        {
            return Err("Invalid source selection. No text was changed.");
        }
    }
    if start != end
        && start >= 2
        && source[..start].ends_with("[[")
        && source[end..].starts_with("]]")
    {
        start -= 2;
        end += 2;
    }
    let analysis = crate::fountain::analyze(source);
    let (a, b) = (
        source[..start].encode_utf16().count(),
        source[..end].encode_utf16().count(),
    );
    let overlaps = |x: usize, y: usize| {
        if a == b {
            x < a && a < y
        } else {
            x < b && y > a
        }
    };
    if analysis
        .omissions
        .iter()
        .any(|r| overlaps(r.start, r.start + r.length))
    {
        return Err("The selection overlaps omitted text. No note was added.");
    }
    // Parser ranges are ordered. Convert only requested endpoints, in one pass.
    let (mut byte, mut units) = (0, 0);
    let mut to_byte = |target| {
        while units < target {
            let c = source[byte..].chars().next().unwrap();
            byte += c.len_utf8();
            units += c.len_utf16();
        }
        byte
    };
    let spans: Vec<Range<usize>> = analysis
        .notes
        .iter()
        .map(|r| to_byte(r.start)..to_byte(r.start + r.length))
        .collect();
    let touched: Vec<_> = spans
        .iter()
        .filter(|r| {
            if start == end {
                r.start < start && start < r.end
            } else {
                r.start < end && r.end > start
            }
        })
        .collect();
    if touched.iter().any(|r| r.start < start || r.end > end) {
        return Err(
            "Select the whole note to remove its brackets. Nested notes would change what is hidden.",
        );
    }
    let mut cursor = start;
    let mut only_notes = !touched.is_empty();
    for span in &touched {
        only_notes &= source[cursor..span.start].trim().is_empty();
        cursor = span.end;
    }
    only_notes &= source[cursor..end].trim().is_empty();
    let replacement;
    let (selection_start, selection_end);
    if only_notes {
        let mut text = String::new();
        cursor = start;
        for span in touched {
            text.push_str(&source[cursor..span.start]);
            text.push_str(&source[span.start + 2..span.end - 2]);
            cursor = span.end;
        }
        text.push_str(&source[cursor..end]);
        selection_start = start;
        selection_end = start + text.len();
        replacement = text;
    } else {
        if !touched.is_empty()
            || source[start..end].contains("[[")
            || source[start..end].contains("]]")
        {
            return Err(
                "The selection contains note brackets. Select complete notes on their own to remove them.",
            );
        }
        if start == end {
            replacement = "[[]]".to_owned();
            selection_start = start + 2;
            selection_end = selection_start;
        } else {
            let mut text = String::new();
            let mut first = None;
            let mut last = 0;
            for line in source[start..end].split_inclusive('\n') {
                let body = line
                    .strip_suffix("\r\n")
                    .or_else(|| line.strip_suffix('\n'))
                    .unwrap_or(line);
                if body.is_empty() {
                    text.push_str(line);
                    continue;
                }
                text.push_str("[[");
                first.get_or_insert(text.len());
                text.push_str(body);
                last = text.len();
                text.push_str("]]");
                text.push_str(&line[body.len()..]);
            }
            let Some(first) = first else {
                return Ok(None);
            };
            selection_start = start + first;
            selection_end = start + last;
            replacement = text;
        }
    }
    let mut changed = source.to_owned();
    changed.replace_range(start..end, &replacement);
    let (anchor, focus) = if anchor > focus {
        (selection_end, selection_start)
    } else {
        (selection_start, selection_end)
    };
    Ok(Some(Edit {
        source: changed,
        anchor,
        focus,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inline_multiline_whitespace_and_reverse_selection_match_installed_highland() {
        for (source, expected) in [
            ("Zoë 🌙", "[[Zoë 🌙]]"),
            (
                "  First  \r\n\r\n  Second ",
                "[[  First  ]]\r\n\r\n[[  Second ]]",
            ),
            ("First\nSecond", "[[First]]\n[[Second]]"),
        ] {
            let marked = edit(source, source.len(), 0).unwrap().unwrap();
            assert_eq!(marked.source, expected);
            assert!(marked.anchor > marked.focus);
            let restored = edit(&marked.source, marked.anchor, marked.focus)
                .unwrap()
                .unwrap();
            assert_eq!(restored.source, source);
        }
        let source = "Alpha Zoë 🌙 omega.";
        let start = source.find("Zoë").unwrap();
        let marked = edit(source, start, start + "Zoë 🌙".len())
            .unwrap()
            .unwrap();
        assert_eq!(marked.source, "Alpha [[Zoë 🌙]] omega.");
        assert_eq!(&marked.source[marked.anchor..marked.focus], "Zoë 🌙");
        let restored = edit(&marked.source, marked.anchor, marked.focus)
            .unwrap()
            .unwrap();
        assert_eq!(restored.source, source);
        let caret = edit(source, source.len(), source.len()).unwrap().unwrap();
        assert_eq!(caret.source, "Alpha Zoë 🌙 omega.[[]]");
        assert_eq!(caret.focus, source.len() + 2);
    }
    #[test]
    fn existing_multiline_notes_are_removable_and_unsafe_nesting_is_atomic() {
        let source = "Before [[First\r\n  \r\nSecond]] after";
        let a = source.find("[[").unwrap();
        let b = source.find("]]").unwrap() + 2;
        assert_eq!(
            edit(source, a + 2, b - 2).unwrap().unwrap().source,
            "Before First\r\n  \r\nSecond after"
        );
        assert!(edit(source, a + 3, b - 3).is_err());
        assert!(edit(source, a + 3, a + 3).is_err());
        assert!(edit(source, 0, source.len()).is_err());
        assert!(edit("/* omitted */", 3, 6).is_err());
        assert!(edit("literal ]]", 0, 10).is_err());
        assert!(edit("🌙", 1, 4).is_err());
        assert!(edit("a\r\nb", 0, 2).is_err());
        assert!(edit("x", 0, 9).is_err());
        assert!(edit("\r\n", 0, 2).unwrap().is_none());
    }
    #[test]
    fn inserted_notes_are_indexed_in_all_categories_and_excluded_from_screenplay_output() {
        let source = "INT. ROOM - DAY\r\n\r\nThe secret is here.\r\n";
        let a = source.find("secret").unwrap();
        let marked = edit(source, a, a + 6).unwrap().unwrap();
        for code in 0..8 {
            let analysis = crate::writing::analyze(
                &marked.source,
                crate::writing::Format::from_code(code).unwrap(),
            );
            assert_eq!(analysis.notes.len(), 1);
            assert_eq!(analysis.notes[0].text, "secret");
        }
        let snapshot = crate::output::snapshot(&marked.source);
        assert!(
            snapshot
                .output
                .body
                .iter()
                .all(|line| !line.text.contains("secret"))
        );
        assert_eq!(
            snapshot.analysis.words + 1,
            crate::fountain::analyze(source).words
        );
    }
}

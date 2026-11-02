//! Line-local Fountain formatting commands. All coordinates refer to original UTF-8 source.
#[derive(Debug, Clone, Copy)]
pub enum Style {
    Bold,
    Italic,
    Underline,
}
impl Style {
    pub fn from_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::Bold),
            1 => Some(Self::Italic),
            2 => Some(Self::Underline),
            _ => None,
        }
    }
    fn marker(self) -> &'static str {
        match self {
            Self::Bold => "**",
            Self::Italic => "*",
            Self::Underline => "_",
        }
    }
}
#[derive(Debug)]
pub struct Edit {
    pub source: String,
    pub anchor: usize,
    pub focus: usize,
}
/// Toggle each nonempty selected line, preserving whitespace and all other bytes.
/// A caret inserts a matching pair and places the caret between the delimiters.
pub fn edit(
    source: &str,
    anchor: usize,
    focus: usize,
    style: Style,
) -> Result<Option<Edit>, &'static str> {
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
    let marker = style.marker();
    let width = marker.len();
    let symbol = marker.as_bytes()[0];
    // A selection inside existing paired markers toggles those markers too.
    // Odd star runs distinguish italic from an existing bold pair.
    if start != end {
        let left = source.as_bytes()[..start]
            .iter()
            .rev()
            .take_while(|&&b| b == symbol)
            .count();
        let right = source.as_bytes()[end..]
            .iter()
            .take_while(|&&b| b == symbol)
            .count();
        let escaped = source.as_bytes()[..start - left]
            .iter()
            .rev()
            .take_while(|&&b| b == b'\\')
            .count()
            % 2
            == 1;
        if !escaped
            && left >= width
            && right >= width
            && (!matches!(style, Style::Italic) || (left % 2 == 1 && right % 2 == 1))
        {
            start -= width;
            end += width;
        }
    }
    if start == end {
        let mut changed = source.to_owned();
        changed.insert_str(start, &marker.repeat(2));
        return Ok(Some(Edit {
            source: changed,
            anchor: start + width,
            focus: start + width,
        }));
    }
    let selected = &source[start..end];
    let wrapped = |body: &str| {
        body.len() > width * 2
            && body.starts_with(marker)
            && body.ends_with(marker)
            && (!matches!(style, Style::Italic)
                || (body.bytes().take_while(|&b| b == b'*').count() % 2 == 1
                    && body.bytes().rev().take_while(|&b| b == b'*').count() % 2 == 1))
    };
    let bodies: Vec<_> = selected
        .split('\n')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if bodies.is_empty() {
        return Ok(None);
    }
    let removing = bodies.iter().all(|s| wrapped(s));
    let replacement = selected
        .split('\n')
        .map(|line| {
            let body = line.trim();
            if body.is_empty() {
                return line.to_owned();
            }
            let prefix = line.len() - line.trim_start().len();
            let suffix = prefix + body.len();
            let changed = if removing {
                body[width..body.len() - width].to_owned()
            } else {
                format!("{marker}{body}{marker}")
            };
            format!("{}{changed}{}", &line[..prefix], &line[suffix..])
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut changed = source.to_owned();
    changed.replace_range(start..end, &replacement);
    let result_end = start + replacement.len();
    let (anchor, focus) = if anchor > focus {
        (result_end, start)
    } else {
        (start, result_end)
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
    fn multiline_unicode_roundtrip_preserves_line_endings_edges_and_selection_direction() {
        let original = "Title: Keep\r\n\r\n  Zoë 🌙\r\n\r\nSecond line.  \r\n[[private]]";
        let start = original.find("  Zoë").unwrap();
        let end = original.find("[[").unwrap();
        for style in [Style::Bold, Style::Italic, Style::Underline] {
            let marked = edit(original, end, start, style).unwrap().unwrap();
            assert!(marked.anchor > marked.focus);
            assert!(marked.source.starts_with(&original[..start]));
            assert!(marked.source.ends_with(&original[end..]));
            let expected = format!(
                "  {0}Zoë 🌙{0}\r\n\r\n{0}Second line.{0}  \r\n",
                style.marker()
            );
            assert_eq!(&marked.source[marked.focus..marked.anchor], expected);
            let restored = edit(&marked.source, marked.anchor, marked.focus, style)
                .unwrap()
                .unwrap();
            assert_eq!(restored.source, original);
            assert_eq!((restored.anchor, restored.focus), (end, start));
        }
    }
    #[test]
    fn nested_markers_inner_selection_and_caret_follow_swift_reference() {
        for (source, start, end, style, expected) in [
            ("**bold**", 0, 8, Style::Italic, "***bold***"),
            ("***both***", 0, 10, Style::Italic, "**both**"),
            ("***both***", 0, 10, Style::Bold, "*both*"),
            ("A **word**.", 4, 8, Style::Bold, "A word."),
            ("**bold**", 2, 6, Style::Italic, "***bold***"),
            ("\\**word**", 3, 7, Style::Bold, "\\****word****"),
        ] {
            assert_eq!(
                edit(source, start, end, style).unwrap().unwrap().source,
                expected
            );
        }
        let caret = edit("Hi", 2, 2, Style::Bold).unwrap().unwrap();
        assert_eq!(caret.source, "Hi****");
        assert_eq!((caret.anchor, caret.focus), (4, 4));
        assert!(edit(" \t\r\n", 0, 4, Style::Bold).unwrap().is_none());
        for (source, a, f) in [("🌙", 1, 4), ("a\r\nb", 0, 2), ("abc", 0, 9)] {
            assert!(edit(source, a, f, Style::Bold).is_err());
        }
    }
    #[test]
    fn full_script_toggles_preserve_semantic_structure_and_rendered_text() {
        let source = "Title: Café\r\n\r\n# Act One\r\n\r\nINT. CAFÉ - DAY\r\n\r\n@Zoë\r\n(quietly)\r\nHello 🌙.\r\n\r\n> CUT TO:";
        let baseline = crate::output::snapshot(source);
        let signature = |document: &crate::output::Document| {
            document
                .body
                .iter()
                .map(|line| (line.kind, line.source_line, line.text.clone()))
                .collect::<Vec<_>>()
        };
        for style in [Style::Bold, Style::Italic, Style::Underline] {
            let marked = edit(source, 0, source.len(), style).unwrap().unwrap();
            let snapshot = crate::output::snapshot(&marked.source);
            assert_eq!(signature(&snapshot.output), signature(&baseline.output));
            assert_eq!(snapshot.analysis.characters, baseline.analysis.characters);
            assert_eq!(snapshot.analysis.locations, baseline.analysis.locations);
            assert_eq!(
                edit(&marked.source, marked.anchor, marked.focus, style)
                    .unwrap()
                    .unwrap()
                    .source,
                source
            );
        }
    }
}

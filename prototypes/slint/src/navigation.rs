//! Heading line indexes map directly to UTF-8 source ranges, including CRLF and Unicode.
use std::ops::Range;
/// Translate only parser-requested UTF-16 endpoints in one source pass.
/// Endpoints inside a surrogate pair or outside source are rejected.
pub fn note_ranges(
    source: &str,
    notes: &[fountage_core::writing::ScriptNote],
) -> Vec<Option<Range<usize>>> {
    utf16_ranges(source, notes.iter().map(|note| (note.start, note.length)))
}
/// Map requested UTF-16 start/length pairs without indexing every source character.
pub fn utf16_ranges(
    source: &str,
    ranges: impl IntoIterator<Item = (usize, usize)>,
) -> Vec<Option<Range<usize>>> {
    let ranges: Vec<_> = ranges.into_iter().collect();
    let mut endpoints = std::collections::BTreeMap::<usize, Vec<(usize, bool)>>::new();
    let mut mapped = vec![(None, None); ranges.len()];
    for (index, &(start, length)) in ranges.iter().enumerate() {
        let Some(end) = start.checked_add(length) else {
            continue;
        };
        endpoints.entry(start).or_default().push((index, true));
        endpoints.entry(end).or_default().push((index, false));
    }
    let mut utf16 = 0;
    for (byte, width) in source
        .char_indices()
        .map(|(b, c)| (b, c.len_utf16()))
        .chain(std::iter::once((source.len(), 0)))
    {
        if endpoints.is_empty() {
            break;
        }
        if let Some(targets) = endpoints.remove(&utf16) {
            for (index, start) in targets {
                if start {
                    mapped[index].0 = Some(byte);
                } else {
                    mapped[index].1 = Some(byte);
                }
            }
        }
        utf16 += width;
    }
    mapped
        .into_iter()
        .map(|(start, end)| Some(start?..end?))
        .collect()
}
pub fn line_ranges(source: &str) -> Vec<Range<usize>> {
    let mut start = 0;
    source
        .split_inclusive('\n')
        .map(|line| {
            let end = start + line.trim_end_matches(['\r', '\n']).len();
            let range = start..end;
            start += line.len();
            range
        })
        .collect()
}
pub fn guarded_range(
    source: &str,
    analyzed: &str,
    category: i32,
    analyzed_category: i32,
    start: i32,
    end: i32,
) -> Option<Range<usize>> {
    if source != analyzed || category != analyzed_category {
        return None;
    }
    let start = usize::try_from(start).ok()?;
    let end = usize::try_from(end).ok()?;
    if start > end || source.get(start..end).is_none() {
        return None;
    }
    Some(start..end)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn note_endpoints_map_unicode_once_and_reject_surrogate_and_overflow_ranges() {
        let source = "🌙 Café 世界\r\n";
        let notes = [
            (3, 4),
            (1, 1),
            (0, 2),
            (999, 1),
            (usize::MAX, 1),
            (source.encode_utf16().count(), 0),
        ]
        .map(|(start, length)| fountage_core::writing::ScriptNote {
            line: 0,
            start,
            length,
            text: String::new(),
        });
        assert_eq!(
            note_ranges(source, &notes),
            vec![
                Some(5..10),
                None,
                Some(0..4),
                None,
                None,
                Some(source.len()..source.len())
            ]
        );
    }
    #[test]
    fn exact_lines_after_emoji_with_repeated_headings_and_mixed_endings() {
        let source = "🌙\r\nINT. CAFÉ - DAY\n\nINT. CAFÉ - DAY";
        let ranges = line_ranges(source);
        assert_eq!(&source[ranges[1].clone()], "INT. CAFÉ - DAY");
        assert_eq!(&source[ranges[3].clone()], "INT. CAFÉ - DAY");
        assert_ne!(ranges[1].start, ranges[3].start);
        assert_eq!(ranges[3].end, source.len());
    }
    #[test]
    fn stale_category_invalid_utf8_and_out_of_bounds_are_rejected() {
        let source = "🌙\r\nINT. ROOM - DAY";
        for (text, category, start, end) in [
            ("changed", 0, 0, 4),
            (source, 1, 0, 4),
            (source, 0, 1, 4),
            (source, 0, -1, 4),
            (source, 0, 4, 999),
            (source, 0, 4, 0),
        ] {
            assert!(guarded_range(text, source, category, 0, start, end).is_none());
        }
        assert_eq!(guarded_range(source, source, 0, 0, 0, 4), Some(0..4));
    }
}

//! Derived, line-local Fountain emphasis ranges. Offsets always refer to source.
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Emphasis {
    pub start: usize,
    pub length: usize,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    #[serde(skip)]
    pub markers: [std::ops::Range<usize>; 2],
}

struct Delimiter {
    start: usize,
    remaining: usize,
    marker: u8,
}

pub fn parse(source: &str, visible: &str) -> Vec<Emphasis> {
    let mut result = Vec::new();
    let mut offset = 0;
    let mut utf16_offset = 0;
    for line in visible.split_inclusive('\n') {
        let bytes = line.as_bytes();
        let mut openers: Vec<Delimiter> = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            let marker = bytes[i];
            if marker == b'\\' {
                // Escaped punctuation is literal. Two backslashes escape each other.
                i += 1;
                if i < bytes.len() {
                    i += line[i..].chars().next().unwrap().len_utf8();
                }
                continue;
            }
            if marker != b'*' && marker != b'_' {
                i += line[i..].chars().next().unwrap().len_utf8();
                continue;
            }
            let start = i;
            while i < bytes.len() && bytes[i] == marker {
                i += 1;
            }
            let mut remaining = i - start;
            let mut consumed = 0;
            let can_close = line[..start]
                .chars()
                .next_back()
                .is_some_and(|c| !c.is_whitespace());
            let can_open = line[i..].chars().next().is_some_and(|c| !c.is_whitespace());
            if can_close {
                while remaining > 0 {
                    let Some(index) = openers.iter().rposition(|d| d.marker == marker) else {
                        break;
                    };
                    let opener = &mut openers[index];
                    let width = if marker == b'*' && remaining >= 2 && opener.remaining >= 2 {
                        2
                    } else {
                        1
                    };
                    let a = offset + opener.start + opener.remaining;
                    let b = offset + start + consumed;
                    if a >= b {
                        break;
                    }
                    result.push(Emphasis {
                        start: utf16_offset + source[offset..a].encode_utf16().count(),
                        length: source[a..b].encode_utf16().count(),
                        bold: marker == b'*' && width == 2,
                        italic: marker == b'*' && width == 1,
                        underline: marker == b'_',
                        markers: [a - width..a, b..b + width],
                    });
                    opener.remaining -= width;
                    remaining -= width;
                    consumed += width;
                    if opener.remaining == 0 {
                        openers.remove(index);
                    }
                }
            }
            if remaining > 0 && can_open {
                openers.push(Delimiter {
                    start: start + consumed,
                    remaining,
                    marker,
                });
            }
        }
        utf16_offset += source[offset..offset + line.len()].encode_utf16().count();
        offset += line.len();
    }
    result
}

// A derived classification string only. Keep source line boundaries and literal,
// unmatched or escaped markers; remove only delimiters actually paired above.
pub fn classification(source: &str, visible: &str, spans: &[Emphasis]) -> (String, Vec<usize>) {
    let mut removed = vec![false; visible.len()];
    for span in spans {
        for range in &span.markers {
            removed[range.clone()].fill(true);
        }
    }
    let mut source_positions = vec![0; source.len()];
    let mut utf16 = 0;
    for (index, c) in source.char_indices() {
        source_positions[index..index + c.len_utf8()].fill(utf16);
        utf16 += c.len_utf16();
    }
    let mut text = String::new();
    let mut positions = Vec::new();
    for (index, c) in visible.char_indices() {
        if !removed[index] {
            text.push(c);
            positions.extend_from_slice(&source_positions[index..index + c.len_utf8()]);
        }
    }
    (text, positions)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn combined_and_nested_styles_have_source_ranges() {
        let s = "🌙 _A **bold** and *italic* phrase_ ***both***";
        let spans = parse(s, s);
        assert_eq!(spans.len(), 5);
        assert_eq!(spans.iter().filter(|s| s.bold).count(), 2);
        assert_eq!(spans.iter().filter(|s| s.italic).count(), 2);
        assert_eq!(spans.iter().filter(|s| s.underline).count(), 1);
        let units: Vec<u16> = s.encode_utf16().collect();
        assert_eq!(
            String::from_utf16(&units[spans[0].start..spans[0].start + spans[0].length]).unwrap(),
            "bold"
        );
    }
    #[test]
    fn literal_escapes_whitespace_and_line_breaks() {
        for s in [
            r"\*literal\*",
            "* open *",
            "*unclosed",
            "*first\nsecond*",
            "*first\r\nsecond*",
            "Dial *69 and then *23",
        ] {
            assert!(parse(s, s).is_empty(), "{s}");
        }
        let s = r"**\*9765\***";
        let spans = parse(s, s);
        assert_eq!(spans.len(), 1);
        assert!(spans[0].bold);
    }
}

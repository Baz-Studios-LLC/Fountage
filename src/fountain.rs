//! Source-preserving Fountain structure shared by navigation, cast counts and editor styling.
//! Layout/pagination and side-by-side dual dialogue are separate work.
use crate::writing::{Analysis, Element, Heading, Kind, ScriptNote, SourceRange};

struct HiddenSpan {
    start: usize,
    end: usize,
    kind: Kind,
}

// Hidden spans arrive in source order. Count each intervening segment once,
// rather than rescanning the entire prefix for every note/marker/omission.
#[derive(Default)]
struct SourceCursor {
    byte: usize,
    utf16: usize,
    line: usize,
}
impl SourceCursor {
    fn advance(&mut self, source: &str, end: usize) {
        debug_assert!(end >= self.byte);
        for c in source[self.byte..end].chars() {
            self.utf16 += c.len_utf16();
            self.line += usize::from(c == '\n');
        }
        self.byte = end;
    }
    fn range(&mut self, source: &str, start: usize, end: usize) -> (SourceRange, usize) {
        self.advance(source, start);
        let first = self.utf16;
        let line = self.line;
        self.advance(source, end);
        (
            SourceRange {
                start: first,
                length: self.utf16 - first,
            },
            line,
        )
    }
}

// Replace hidden bytes with spaces while preserving line endings and byte positions.
// Work on a derived copy only; the original source is never rewritten.
fn visible_source(source: &str, result: &mut Analysis) -> (String, Vec<HiddenSpan>) {
    let mut visible = source.as_bytes().to_vec();
    let mut spans = Vec::new();
    let bytes = source.as_bytes();
    let mut cursor = 0;
    let mut positions = SourceCursor::default();
    while cursor + 1 < bytes.len() {
        let (end, kind) =
            if bytes[cursor..].starts_with(b"/*") {
                let end = source[cursor + 2..]
                    .find("*/")
                    .map_or(source.len(), |end| cursor + 2 + end + 2);
                result
                    .omissions
                    .push(positions.range(source, cursor, end).0);
                (end, Kind::Omitted)
            } else if bytes[cursor..].starts_with(b"[[") {
                let rest = &source[cursor + 2..];
                let Some(close) = rest.find("]]") else {
                    cursor += 2;
                    continue;
                };
                let content = &rest[..close];
                // An actually empty line ends the search. A two-space line can join a note.
                if content.split_inclusive('\n').skip(1).any(|line| {
                    line.ends_with('\n') && line.trim_end_matches(['\r', '\n']).is_empty()
                }) {
                    cursor += 2;
                    continue;
                }
                let end = cursor + 2 + close + 2;
                let (range, line) = positions.range(source, cursor, end);
                result.notes.push(ScriptNote {
                    line,
                    start: range.start,
                    length: range.length,
                    text: content.trim().to_owned(),
                });
                (end, Kind::Note)
            } else if bytes[cursor..].starts_with(b"{{%m")
                && bytes[..cursor]
                    .iter()
                    .rev()
                    .take_while(|b| **b == b'\\')
                    .count()
                    % 2
                    == 0
            {
                let rest = &source[cursor + 4..];
                let Some(close) = rest.find("}}") else {
                    cursor += 4;
                    continue;
                };
                let content = &rest[..close];
                let name = if content.is_empty() {
                    Some("")
                } else {
                    content.strip_prefix(':').map(str::trim)
                };
                let Some(name) = name.filter(|_| !content.contains(['\r', '\n'])) else {
                    cursor += 4;
                    continue;
                };
                let end = cursor + 4 + close + 2;
                let (range, line) = positions.range(source, cursor, end);
                result.markers.push(ScriptNote {
                    line,
                    start: range.start,
                    length: range.length,
                    text: name.to_owned(),
                });
                (end, Kind::Marker)
            } else {
                cursor += 1;
                continue;
            };
        for byte in &mut visible[cursor..end] {
            if !matches!(*byte, b'\r' | b'\n') {
                *byte = b' ';
            }
        }
        spans.push(HiddenSpan {
            start: cursor,
            end,
            kind,
        });
        cursor = end;
    }
    // Each replaced span is delimited at UTF-8 boundaries; remaining bytes are unchanged.
    (
        String::from_utf8(visible).expect("masked Fountain remains UTF-8"),
        spans,
    )
}

/// Reuse directive recognition without applying Fountain note/omission semantics
/// to other writing categories. Only marker bytes are masked for classification.
pub(crate) fn marker_source(
    source: &str,
) -> (
    String,
    Vec<ScriptNote>,
    Vec<ScriptNote>,
    usize,
    Vec<crate::todo::Todo>,
) {
    let mut result = Analysis::default();
    let (full_visible, spans) = visible_source(source, &mut result);
    let todos = crate::todo::parse(source, &full_visible);
    let mut masked = source.as_bytes().to_vec();
    let mut counted = String::new();
    let mut cursor = 0;
    for span in spans.iter().filter(|span| span.kind == Kind::Marker) {
        masked[span.start..span.end].fill(b' ');
        counted.push_str(&source[cursor..span.start]);
        cursor = span.end;
    }
    counted.push_str(&source[cursor..]);
    (
        String::from_utf8(masked).expect("masked markers remain UTF-8"),
        result.markers,
        result.notes,
        counted.trim().encode_utf16().count(),
        todos,
    )
}

fn title_key(line: &str) -> bool {
    let Some((key, _)) = line.split_once(':') else {
        return false;
    };
    matches!(
        key.to_lowercase().as_str(),
        "title"
            | "credit"
            | "author"
            | "authors"
            | "source"
            | "draft date"
            | "contact"
            | "copyright"
            | "notes"
    )
}

fn is_scene(line: &str) -> bool {
    let upper = line.to_uppercase();
    ["INT", "EXT", "EST", "INT./EXT", "INT/EXT", "EXT/INT", "I/E"]
        .iter()
        .any(|prefix| {
            upper
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with(['.', ' ']))
        })
        || line
            .strip_prefix('.')
            .is_some_and(|rest| rest.chars().next().is_some_and(char::is_alphanumeric))
}

// Conservative derived location names: retain ambiguous suffixes rather than
// guessing that part of a location is a time of day. Never alter manuscript text.
fn location_name(line: &str) -> Option<String> {
    let upper = line.trim_start_matches('.').to_uppercase();
    let mut name = upper.as_str();
    for prefix in [
        "INT./EXT.",
        "INT/EXT.",
        "EXT/INT.",
        "I/E.",
        "INT.",
        "EXT.",
        "EST.",
        "INT/EXT ",
        "EXT/INT ",
        "I/E ",
        "INT ",
        "EXT ",
        "EST ",
    ] {
        if let Some(rest) = name.strip_prefix(prefix) {
            name = rest.trim();
            break;
        }
    }
    if let Some(without_end) = name.strip_suffix('#')
        && let Some((body, number)) = without_end.rsplit_once(" #")
        && !number.is_empty()
        && !number.contains('#')
    {
        name = body.trim();
    }
    for separator in [" - ", " – ", " — "] {
        if let Some((body, time)) = name.rsplit_once(separator)
            && matches!(
                time.trim(),
                "DAY"
                    | "NIGHT"
                    | "DAWN"
                    | "DUSK"
                    | "MORNING"
                    | "AFTERNOON"
                    | "EVENING"
                    | "CONTINUOUS"
                    | "LATER"
                    | "SAME TIME"
                    | "MOMENTS LATER"
            )
        {
            name = body.trim();
            break;
        }
    }
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    (!name.is_empty()).then_some(name)
}

fn character_name(line: &str) -> Option<&str> {
    let name = line.trim_start_matches('@').trim_end_matches('^').trim();
    let base = name.split(" (").next().unwrap_or(name).trim();
    if base.chars().any(char::is_alphabetic)
        && (line.starts_with('@') || base == base.to_uppercase())
        && !line.starts_with(['!', '#', '=', '>', '[', '~', '.'])
        && !line.ends_with(':')
    {
        Some(base)
    } else {
        None
    }
}

pub fn analyze(source: &str) -> Analysis {
    let mut result = Analysis::default();
    let (visible, hidden) = visible_source(source, &mut result);
    result.todos = crate::todo::parse(source, &visible);
    result.emphasis = crate::emphasis::parse(source, &visible);
    let (visible, word_positions) =
        crate::emphasis::classification(source, &visible, &result.emphasis);
    let lines: Vec<&str> = visible.lines().collect();
    let original: Vec<&str> = source.lines().collect();
    let mut dialogue = false;
    let mut in_title = lines.first().is_some_and(|line| title_key(line.trim()));
    let mut section_depth = 0;
    let mut previous_level: u8 = 0;
    let mut byte_offset = 0;
    let mut hidden_cursor = 0;
    // split_inclusive retains LF/CRLF byte lengths for annotation classification.
    for (i, raw_with_ending) in source.split_inclusive('\n').enumerate() {
        let original_line = original.get(i).copied().unwrap_or("");
        let visible_line = lines.get(i).copied().unwrap_or("");
        let line = visible_line.trim();
        let start = byte_offset;
        byte_offset += raw_with_ending.len();
        while hidden
            .get(hidden_cursor)
            .is_some_and(|span| span.end <= start)
        {
            hidden_cursor += 1;
        }
        if line.is_empty() {
            if let Some(span) = hidden
                .get(hidden_cursor)
                .filter(|span| span.start < byte_offset && span.end > start)
            {
                result.elements.push(Element {
                    line: i,
                    kind: span.kind,
                });
            } else if dialogue && original_line == "  " {
                result.elements.push(Element {
                    line: i,
                    kind: Kind::Dialogue,
                });
            } else {
                dialogue = false;
                in_title = false;
            }
            continue;
        }
        if in_title {
            let indented = original_line.starts_with("   ") || original_line.starts_with('\t');
            if title_key(line)
                || indented
                || line.split_once(':').is_some_and(|(key, _)| {
                    !key.is_empty() && key.chars().all(|c| c.is_alphabetic() || c == ' ')
                })
            {
                result.elements.push(Element {
                    line: i,
                    kind: Kind::Title,
                });
                continue;
            }
            in_title = false;
        }
        let starts_block = i == 0 || lines[i - 1].trim().is_empty();
        let followed = lines.get(i + 1).is_some_and(|line| !line.trim().is_empty());
        let kind;
        if result.todos.iter().any(|todo| todo.line == i) {
            dialogue = false;
            kind = Kind::Todo;
        } else if line.starts_with('!') {
            dialogue = false;
            kind = Kind::Body;
        } else if line.chars().all(|c| c == '=') && line.len() >= 3 {
            dialogue = false;
            kind = Kind::PageBreak;
            result.headings.push(Heading {
                line: i,
                heading: "Page Break".into(),
                kind: Kind::PageBreak,
                level: previous_level.saturating_add(1),
            });
        } else if line.starts_with('#') {
            dialogue = false;
            section_depth = line.bytes().take_while(|c| *c == b'#').count().min(255) as u8;
            previous_level = section_depth.saturating_sub(1);
            let text = line.trim_start_matches('#').trim();
            if !text.is_empty() {
                result.headings.push(Heading {
                    line: i,
                    heading: text.into(),
                    kind: Kind::Section,
                    level: previous_level,
                });
            }
            kind = Kind::Section;
        } else if let Some(synopsis) = line.strip_prefix('=').filter(|text| !text.starts_with('='))
        {
            dialogue = false;
            if !synopsis.trim().is_empty() {
                result.headings.push(Heading {
                    line: i,
                    heading: synopsis.trim().into(),
                    kind: Kind::Synopsis,
                    level: previous_level.saturating_add(1),
                });
            }
            kind = Kind::Synopsis;
        } else if starts_block && !dialogue && is_scene(line) {
            dialogue = false;
            result.units += 1;
            if let Some(location) = location_name(line) {
                result.locations.entry(location).or_default().push(i);
            }
            previous_level = section_depth;
            result.headings.push(Heading {
                line: i,
                heading: line.strip_prefix('.').unwrap_or(line).into(),
                kind: Kind::Scene,
                level: section_depth,
            });
            kind = Kind::Scene;
        } else if line.starts_with('>') {
            dialogue = false;
            kind = if line.ends_with('<') {
                Kind::Centered
            } else {
                Kind::Transition
            };
        } else if line.starts_with('~') {
            dialogue = false;
            kind = Kind::Lyric;
        } else if !dialogue
            && starts_block
            && !followed
            && visible_line.ends_with("TO:")
            && line == line.to_uppercase()
        {
            dialogue = false;
            kind = Kind::Transition;
        } else if line.starts_with('@')
            && followed
            && let Some(name) = character_name(line)
        {
            dialogue = true;
            *result.characters.entry(name.into()).or_default() += 1;
            kind = Kind::Character;
        } else if dialogue {
            kind = if line.starts_with('(') && line.ends_with(')') {
                Kind::Parenthetical
            } else {
                Kind::Dialogue
            };
        } else if (starts_block || line.starts_with('@'))
            && followed
            && let Some(name) = character_name(line)
        {
            dialogue = true;
            *result.characters.entry(name.into()).or_default() += 1;
            kind = Kind::Character;
        } else {
            kind = Kind::Body;
        }
        result.elements.push(Element { line: i, kind });
    }
    for element in &result.elements {
        if matches!(
            element.kind,
            Kind::Title
                | Kind::Section
                | Kind::Synopsis
                | Kind::Note
                | Kind::Omitted
                | Kind::PageBreak
        ) {
            continue;
        }
        let mut text = lines.get(element.line).copied().unwrap_or("").trim();
        if element.kind == Kind::Todo {
            text = text.get(3..).unwrap_or("").trim_start();
        }
        if element.kind == Kind::Scene
            && let Some(without_end) = text.strip_suffix('#')
            && let Some((body, number)) = without_end.rsplit_once(" #")
            && !number.is_empty()
            && number
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '.')
        {
            text = body;
        }
        // Whitespace-delimited words with letters/numbers. Syntax-only tokens,
        // punctuation and emoji are not words; names and printed headings count.
        for token in text.split_whitespace() {
            let mut letters = token.char_indices().filter(|(_, c)| c.is_alphanumeric());
            let Some((first, first_char)) = letters.next() else {
                continue;
            };
            let (last, last_char) = letters.next_back().unwrap_or((first, first_char));
            let base = token.as_ptr() as usize - visible.as_ptr() as usize;
            let start = word_positions[base + first];
            let end = word_positions[base + last] + last_char.len_utf16();
            result.word_ranges.push(SourceRange {
                start,
                length: end - start,
            });
        }
    }
    result.words = result.word_ranges.len();
    // Highland character goals count UTF-16 source units, including markup and
    // interior separators, after removing notes, boneyards and synopsis text.
    let mut excluded: Vec<(usize, usize)> =
        hidden.iter().map(|span| (span.start, span.end)).collect();
    let synopsis_lines: std::collections::BTreeSet<_> = result
        .elements
        .iter()
        .filter(|element| element.kind == Kind::Synopsis)
        .map(|element| element.line)
        .collect();
    let mut offset = 0;
    for (line, text) in source.split_inclusive('\n').enumerate() {
        if synopsis_lines.contains(&line) {
            excluded.push((offset, offset + text.trim_end_matches(['\r', '\n']).len()));
        }
        offset += text.len();
    }
    excluded.sort_unstable();
    let mut visible = String::new();
    let mut cursor = 0;
    for (start, end) in excluded {
        if start > cursor {
            visible.push_str(&source[cursor..start]);
        }
        cursor = cursor.max(end);
    }
    visible.push_str(&source[cursor..]);
    result.character_count = visible.trim().encode_utf16().count();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dense_mixed_hidden_spans_keep_original_unicode_ranges_and_lines() {
        let mut source = String::from("Title: Zoë 🌙\r\n\r\n");
        let mut notes = Vec::new();
        let mut markers = Vec::new();
        let mut omissions = Vec::new();
        for i in 0..256 {
            source.push_str("日本語 e\u{301} 🌙\r\n");
            let start = source.len();
            let note = format!("[[Note {i}: café\r\n  \n世界 🌙]]");
            notes.push((start, start + note.len()));
            source.push_str(&note);
            source.push_str(" gap ");
            let start = source.len();
            let omitted = "/* omitted 🌙\r\n[[not a note]] {{%m: hidden}} */";
            omissions.push((start, start + omitted.len()));
            source.push_str(omitted);
            source.push('\n');
            let start = source.len();
            let marker = format!("{{{{%m: Beat {i} 🌙}}}}");
            markers.push((start, start + marker.len()));
            source.push_str(&marker);
            source.push_str("\r\n");
        }
        let original = source.clone();
        let analysis = analyze(&source);
        assert_eq!(source, original);
        assert_eq!(analysis.notes.len(), notes.len());
        assert_eq!(analysis.markers.len(), markers.len());
        assert_eq!(analysis.omissions.len(), omissions.len());
        for (actual, (start, end)) in analysis
            .notes
            .iter()
            .zip(notes)
            .chain(analysis.markers.iter().zip(markers))
        {
            assert_eq!(actual.start, source[..start].encode_utf16().count());
            assert_eq!(actual.length, source[start..end].encode_utf16().count());
            assert_eq!(
                actual.line,
                source[..start].bytes().filter(|b| *b == b'\n').count()
            );
        }
        for (actual, (start, end)) in analysis.omissions.iter().zip(omissions) {
            assert_eq!(actual.start, source[..start].encode_utf16().count());
            assert_eq!(actual.length, source[start..end].encode_utf16().count());
        }
    }
    #[test]
    fn markers_preserve_utf16_ranges_and_ignore_hidden_escaped_or_malformed_directives() {
        let source = "Title: Zoë\r\n\r\nINT. ROOM - DAY{{%m: Café 🌙}}\r\n\r\n{{%m}} {{%m: second}}\r\n\r\n[[{{%m: private}}]]\r\n/*{{%m: omitted}}*/\r\n\\{{%m: literal}}\r\n{{%more}} {{%m broken}} {{%m: multi\r\nline}}";
        let result = analyze(source);
        assert_eq!(
            result
                .markers
                .iter()
                .map(|m| m.text.as_str())
                .collect::<Vec<_>>(),
            ["Café 🌙", "", "second"]
        );
        let units: Vec<_> = source.encode_utf16().collect();
        for (marker, expected) in
            result
                .markers
                .iter()
                .zip(["{{%m: Café 🌙}}", "{{%m}}", "{{%m: second}}"])
        {
            assert_eq!(
                String::from_utf16(&units[marker.start..marker.start + marker.length]).unwrap(),
                expected
            );
        }
        assert_eq!(
            result.markers.iter().map(|m| m.line).collect::<Vec<_>>(),
            [2, 4, 4]
        );
        assert_eq!(result.headings[0].heading, "INT. ROOM - DAY");
        assert_eq!(result.units, 1);
    }
    #[test]
    fn marker_only_lines_do_not_become_dialogue_or_printable_content() {
        let source =
            "INT. ROOM - DAY\n\nMARA{{%m: cue}}\nHello.\n{{%m: inside}}\nStill speaking.\n\n{{%m}}";
        let result = analyze(source);
        assert_eq!(result.characters.get("MARA"), Some(&1));
        assert_eq!(result.words, 7);
        let output = crate::output::screenplay(source);
        let text = output
            .body
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!text.contains("%m"));
        assert!(text.contains("MARA\nHello.\nStill speaking."));
        assert!(!output.body.iter().any(|line| line.kind == Kind::Marker));
        assert_eq!(
            result.character_count,
            "INT. ROOM - DAY\n\nMARA\nHello.\n\nStill speaking."
                .encode_utf16()
                .count()
        );
    }
    #[test]
    fn character_goals_match_installed_highland_fixtures() {
        for (source, expected) in [
            ("A b 🌙\n\n[[private]]", 6),
            (
                "Title: Test\n\nA b 🌙\n\nC\n\n# Section\n\n= Summary\n\n/*hidden*/",
                33,
            ),
            (
                "**bold**\n\nPlain [[private]] end\n\n= Summary\n\n/*hidden*/",
                20,
            ),
            ("A\n\n[[private]]\n\nB", 6),
            (" [[private]] /*hidden*/ ", 0),
            ("  e\u{301} 🌙\r\nC  ", 8),
        ] {
            assert_eq!(analyze(source).character_count, expected, "{source:?}");
        }
    }
    fn kind_at(analysis: &Analysis, line: usize) -> Kind {
        analysis
            .elements
            .iter()
            .find(|element| element.line == line)
            .unwrap()
            .kind
    }
    #[test]
    fn draft_words_exclude_nonprinting_material_and_markup() {
        let body = "INT. CAFÉ - DAY #12A#\n\n@Zoë ^\nHello **world** [[private note]].\n\n> THE END <\n\n~Sing a song.";
        // INT. CAFÉ DAY + Zoë + Hello world + THE END + Sing a song
        assert_eq!(analyze(body).words, 11);
        let decorated = format!(
            "Title: Not part of draft\nAuthor: Writer\n\n# Outline section\n\n= Planning synopsis\n\n{body}\n\n===\n\n/* INT. OMITTED - DAY\n\nGHOST\nNot printed. */"
        );
        assert_eq!(analyze(&decorated).words, 11);
        assert_eq!(analyze(&decorated.replace('\n', "\r\n")).words, 11);
        assert_eq!(analyze("[[Only a note]]").words, 0);
        assert_eq!(analyze("... — 🌙 !").words, 0);
        assert_eq!(analyze("Hello, 世界. It's 2026.").words, 4);
        assert_eq!(analyze("INT. ROOM - DAY #1.#").words, 3);
    }

    #[test]
    fn formatted_structure_keeps_roles_indexes_and_source_offsets() {
        let plain = "Title: Café\n\n# Act One\n\nINT. CAFÉ - DAY #1#\n\n@Zoë (V.O.)\n(quietly)\nHello 🌙.\n\n> CUT TO:\n\n.The roof\n\n= A synopsis";
        for marker in ["*", "**", "_", "***", "_**"] {
            let closing: String = marker.chars().rev().collect();
            for newline in ["\n", "\r\n"] {
                let formatted = plain
                    .lines()
                    .map(|line| {
                        if line.is_empty() {
                            String::new()
                        } else {
                            format!("{marker}{line}{closing}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(newline);
                let original = analyze(plain);
                let actual = analyze(&formatted);
                assert_eq!(actual.units, original.units);
                assert_eq!(actual.characters, original.characters);
                assert_eq!(actual.locations, original.locations);
                assert_eq!(
                    actual
                        .headings
                        .iter()
                        .map(|h| (&h.heading, h.line, h.kind, h.level))
                        .collect::<Vec<_>>(),
                    original
                        .headings
                        .iter()
                        .map(|h| (&h.heading, h.line, h.kind, h.level))
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    actual
                        .elements
                        .iter()
                        .map(|e| (e.line, e.kind))
                        .collect::<Vec<_>>(),
                    original
                        .elements
                        .iter()
                        .map(|e| (e.line, e.kind))
                        .collect::<Vec<_>>()
                );
                let utf16: Vec<u16> = formatted.encode_utf16().collect();
                for span in actual.emphasis {
                    assert!(
                        String::from_utf16(&utf16[span.start..span.start + span.length]).is_ok()
                    );
                }
            }
        }
    }

    #[test]
    fn formatted_notes_keep_original_utf16_ranges() {
        let source = "**INT. CAFÉ - DAY**\r\n\r\n🌙 **Action [[note]] follows.**\r\n\r\n/* **INT. OMITTED - DAY** */";
        let a = analyze(source);
        assert_eq!(a.units, 1);
        assert_eq!(a.notes.len(), 1);
        let units: Vec<u16> = source.encode_utf16().collect();
        let note = &a.notes[0];
        assert_eq!(
            String::from_utf16(&units[note.start..note.start + note.length]).unwrap(),
            "[[note]]"
        );
    }

    #[test]
    fn locations_group_visible_scenes_without_losing_source_positions() {
        let source = "INT. Café - DAY #1#\r\n\r\nAction.\r\n\r\nEXT. CAFÉ – NIGHT\r\n\r\n/*\r\nINT. HIDDEN - DAY\r\n*/\r\n\r\nINT./EXT. CAR - CONTINUOUS\r\n\r\n.The moon\r\n";
        let a = analyze(source);
        assert_eq!(a.locations["CAFÉ"], vec![0, 4]);
        assert_eq!(a.locations["CAR"], vec![10]);
        assert_eq!(a.locations["THE MOON"], vec![12]);
        assert_eq!(a.locations.len(), 3);
        assert_eq!(
            location_name("INT. ROOM - WEST WING"),
            Some("ROOM - WEST WING".into())
        );
        assert_eq!(location_name("INT.  ROOM  -  DAY"), Some("ROOM".into()));
    }

    #[test]
    fn outline_contains_nested_sections_scenes_and_synopses() {
        let a = analyze(
            "# Act One\n\n= An arrival\n\n## The signal\n\nINT. STATION - NIGHT #1A#\n\n= Mara hears it.\n\n===\n\n# Act Two\n\n.The roof\n",
        );
        assert_eq!(a.units, 2);
        assert_eq!(
            a.headings.iter().map(|h| h.level).collect::<Vec<_>>(),
            [0, 1, 1, 2, 3, 3, 0, 1]
        );
        assert_eq!(a.headings[3].heading, "INT. STATION - NIGHT #1A#");
        assert_eq!(kind_at(&a, 10), Kind::PageBreak);
    }
    #[test]
    fn inline_and_multiline_notes_have_exact_utf16_ranges() {
        let source = "EXT. CAFÉ - DAY\r\n\r\nZoë sees 🌙. [[Check the time.]] And [[ask Éli.]]\r\n\r\n[[Long note\r\n  \r\ncontinued.]]\r\n";
        let a = analyze(source);
        assert_eq!(a.notes.len(), 3);
        assert_eq!(a.notes[0].line, 2);
        assert_eq!(a.notes[1].line, 2);
        assert_ne!(a.notes[0].start, a.notes[1].start);
        let units: Vec<u16> = source.encode_utf16().collect();
        for note in &a.notes {
            let selected =
                String::from_utf16(&units[note.start..note.start + note.length]).unwrap();
            assert!(selected.starts_with("[[") && selected.ends_with("]]"));
            assert_eq!(selected[2..selected.len() - 2].trim(), note.text);
        }
        assert_eq!(a.notes[2].text, "Long note\r\n  \r\ncontinued.");
    }
    #[test]
    fn notes_and_boneyards_do_not_create_phantom_structure() {
        let a = analyze(
            "[[# Not an act\nINT. NOT A SCENE\nGHOST\nBoo.]]\n\n/*\n# Hidden\n\nINT. OMITTED\n\nGHOST\nBoo.\n[[Invisible note]]\n*/\n\nINT. REAL - DAY\n\nMARA\nHello /* hidden */there.",
        );
        assert_eq!(a.units, 1);
        assert_eq!(a.headings.len(), 1);
        assert_eq!(a.characters.len(), 1);
        assert_eq!(a.characters.get("MARA"), Some(&1));
        assert_eq!(a.notes.len(), 1);
        assert_eq!(a.omissions.len(), 2);
        assert_eq!(a.elements.last().unwrap().kind, Kind::Dialogue);
    }
    #[test]
    fn unclosed_note_cannot_swallow_a_later_scene() {
        let a = analyze("[[unfinished\n\nEXT. ROAD - DAY\n\nMARA\nStill here.\n\n]]");
        assert!(a.notes.is_empty());
        assert_eq!(a.units, 1);
        assert_eq!(a.characters.get("MARA"), Some(&1));
    }
    #[test]
    fn note_closing_brackets_can_be_on_their_own_line() {
        let a = analyze("[[A note\nwith two lines.\n]]\n\nEXT. ROAD - DAY\n");
        assert_eq!(a.notes.len(), 1);
        assert_eq!(a.notes[0].text, "A note\nwith two lines.");
        assert_eq!(a.units, 1);
    }
    #[test]
    fn joined_dialogue_and_forced_cues_keep_their_context() {
        let a = analyze(
            "MARA\nReading a heading:\n  \nINT. THIS IS SPOKEN\n@Zoë\nMe too.\n  \nCUT TO:\n",
        );
        assert_eq!(a.units, 0);
        assert_eq!(kind_at(&a, 3), Kind::Dialogue);
        assert_eq!(kind_at(&a, 4), Kind::Character);
        assert_eq!(kind_at(&a, 7), Kind::Dialogue);
        assert_eq!(a.characters.len(), 2);
    }
    #[test]
    fn forced_elements_do_not_become_character_cues() {
        let a = analyze(
            "!THE ROOM\nFalls quiet.\n\n~A SONG\n~Another line\n\n>THE END<\n\n>Fade out.\n\n===",
        );
        assert!(a.characters.is_empty());
        assert_eq!(kind_at(&a, 0), Kind::Body);
        assert_eq!(kind_at(&a, 3), Kind::Lyric);
        assert_eq!(kind_at(&a, 6), Kind::Centered);
        assert_eq!(kind_at(&a, 8), Kind::Transition);
    }
    #[test]
    fn transitions_respect_surrounding_lines_and_trailing_spaces() {
        let a = analyze("CUT TO:\n\nAction.\n\nCUT TO: \n\nACTION:\n\nCUT TO:\nThis is action.");
        assert_eq!(kind_at(&a, 0), Kind::Transition);
        for line in [4, 6, 8] {
            assert_eq!(kind_at(&a, line), Kind::Body);
        }
        assert!(a.characters.is_empty());
    }
    #[test]
    fn two_spaces_keep_dialogue_connected_and_titles_stay_out_of_cast() {
        let a = analyze(
            "Title:\n   THE SIGNAL\nAuthor: Baz Studios\nCustom Field: Kept\n\nINT. ROOM - DAY\n\n@Zoë\nOne.\n  \nTWO.\n(quietly)\nThree.\n\nAction.",
        );
        assert_eq!(kind_at(&a, 1), Kind::Title);
        assert_eq!(kind_at(&a, 3), Kind::Title);
        assert_eq!(kind_at(&a, 9), Kind::Dialogue);
        assert_eq!(kind_at(&a, 10), Kind::Dialogue);
        assert_eq!(kind_at(&a, 11), Kind::Parenthetical);
        assert_eq!(kind_at(&a, 14), Kind::Body);
        assert_eq!(a.characters.len(), 1);
    }
}

//! Shared semantic screenplay output. Pagination consumes these lines; source is never rewritten.
use crate::writing::{Analysis, Kind};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct Run {
    pub start: usize,
    pub length: usize,
    pub style: Style,
}
#[derive(Debug, Clone, Serialize)]
pub struct Line {
    pub source_line: usize,
    pub kind: Kind,
    pub text: String,
    pub runs: Vec<Run>,
    pub scene_number: Option<String>,
    pub section_level: Option<usize>,
    pub dual_dialogue: bool,
    pub todo_completed: Option<bool>,
    #[serde(skip)]
    pub number_source: Option<std::ops::Range<usize>>,
    #[serde(skip)]
    pub number_insertion: Option<usize>,
}
#[derive(Debug, Serialize)]
pub struct TitleField {
    pub key: String,
    // Unknown keys remain metadata, never silently printed as body text.
    pub placement: &'static str,
    pub values: Vec<Line>,
}
#[derive(Debug, Default, Serialize)]
pub struct Document {
    pub title: Vec<Line>,
    pub title_fields: Vec<TitleField>,
    pub body: Vec<Line>,
    pub outline: Vec<Line>,
}
#[derive(Clone)]
struct Atom {
    c: char,
    source_start: usize,
    escaped: bool,
    style: Style,
}
fn trim(atoms: &mut Vec<Atom>) {
    while atoms.last().is_some_and(|a| a.c.is_whitespace()) {
        atoms.pop();
    }
    let first = atoms
        .iter()
        .position(|a| !a.c.is_whitespace())
        .unwrap_or(atoms.len());
    atoms.drain(..first);
}
fn line(source_line: usize, kind: Kind, mut atoms: Vec<Atom>) -> Line {
    let mut scene_number = None;
    let mut number_source = None;
    let mut dual_dialogue = false;
    let mut section_level = None;
    if kind != Kind::Body {
        trim(&mut atoms);
    }
    let first = atoms.iter().position(|a| !a.c.is_whitespace());
    if let Some(i) = first {
        let c = atoms[i].c;
        if !atoms[i].escaped
            && matches!(
                (kind, c),
                (Kind::Body, '!')
                    | (Kind::Scene, '.')
                    | (Kind::Character, '@')
                    | (Kind::Transition, '>')
                    | (Kind::Centered, '>')
                    | (Kind::Lyric, '~')
                    | (Kind::Synopsis, '=')
            )
        {
            atoms.remove(i);
        }
    }
    if kind == Kind::Section {
        let depth = atoms
            .iter()
            .take_while(|atom| atom.c == '#' && !atom.escaped)
            .count();
        if depth > 0 {
            atoms.drain(..depth);
            section_level = Some(depth);
        }
    }
    if kind == Kind::Centered && atoms.last().is_some_and(|a| a.c == '<') {
        atoms.pop();
    }
    if kind == Kind::Character && atoms.last().is_some_and(|a| a.c == '^') {
        atoms.pop();
        dual_dialogue = true;
    }
    if kind == Kind::Scene {
        let text: String = atoms.iter().map(|a| a.c).collect();
        if let Some(without_end) = text.strip_suffix('#')
            && let Some((body, number)) = without_end.rsplit_once(" #")
            && !number.is_empty()
            && number
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '.')
        {
            let open = body.chars().count() + 1;
            if !atoms[open].escaped && !atoms.last().unwrap().escaped {
                number_source =
                    Some(atoms[open].source_start..atoms.last().unwrap().source_start + 1);
                scene_number = Some(number.into());
                atoms.truncate(body.chars().count());
            }
        }
    }
    if kind == Kind::PageBreak {
        atoms.clear();
    }
    if kind != Kind::Body {
        trim(&mut atoms);
    }
    let number_insertion = if kind == Kind::Scene {
        atoms
            .last()
            .map(|atom| atom.source_start + atom.c.len_utf16())
    } else {
        None
    };
    let todo_completed = if kind == Kind::Todo {
        let completed = atoms.get(1).is_some_and(|atom| atom.c != ' ');
        let source_start = atoms.first().map_or(0, |atom| atom.source_start);
        atoms.drain(..3.min(atoms.len()));
        trim(&mut atoms);
        atoms.insert(
            0,
            Atom {
                c: ' ',
                source_start,
                escaped: false,
                style: Style::default(),
            },
        );
        atoms.insert(
            0,
            Atom {
                c: if completed { '✓' } else { '☐' },
                source_start,
                escaped: false,
                style: Style::default(),
            },
        );
        Some(completed)
    } else {
        None
    };
    let mut text = String::new();
    let mut runs: Vec<Run> = Vec::new();
    let mut position = 0;
    for atom in atoms {
        let len = atom.c.len_utf16();
        if let Some(last) = runs.last_mut().filter(|r| r.style == atom.style) {
            last.length += len;
        } else {
            runs.push(Run {
                start: position,
                length: len,
                style: atom.style,
            });
        }
        text.push(atom.c);
        position += len;
    }
    Line {
        source_line,
        kind,
        text,
        runs,
        scene_number,
        section_level,
        dual_dialogue,
        todo_completed,
        number_source,
        number_insertion,
    }
}

// Slice at a UTF-8 boundary, rebasing rendered UTF-16 style runs.
fn slice_line(line: &Line, start: usize) -> Line {
    let offset = line.text[..start].encode_utf16().count();
    let mut value = line.clone();
    value.text = line.text[start..].into();
    value.runs = line
        .runs
        .iter()
        .filter_map(|run| {
            let end = run.start + run.length;
            (end > offset).then(|| Run {
                start: run.start.saturating_sub(offset),
                length: end - run.start.max(offset),
                style: run.style,
            })
        })
        .collect();
    value
}

#[derive(Debug, Serialize)]
pub struct Snapshot {
    pub analysis: Analysis,
    pub output: Document,
}

/// Editing indexes and printable output from exactly one analysis of the source.
pub fn snapshot(source: &str) -> Snapshot {
    let analysis = crate::fountain::analyze(source);
    let output = build(source, &analysis);
    Snapshot { analysis, output }
}

pub fn screenplay(source: &str) -> Document {
    let analysis = crate::fountain::analyze(source);
    build(source, &analysis)
}
fn build(source: &str, analysis: &Analysis) -> Document {
    let mut hidden = vec![false; source.encode_utf16().count()];
    for range in &analysis.omissions {
        hidden[range.start..range.start + range.length].fill(true);
    }
    for note in &analysis.notes {
        hidden[note.start..note.start + note.length].fill(true);
    }
    for marker in &analysis.markers {
        hidden[marker.start..marker.start + marker.length].fill(true);
    }
    let mut markers = vec![false; source.len()];
    let mut events: BTreeMap<usize, [i32; 3]> = BTreeMap::new();
    for span in &analysis.emphasis {
        for range in &span.markers {
            markers[range.clone()].fill(true);
        }
        let traits = [span.bold as i32, span.italic as i32, span.underline as i32];
        for (i, value) in traits.into_iter().enumerate() {
            events.entry(span.start).or_default()[i] += value;
            events.entry(span.start + span.length).or_default()[i] -= value;
        }
    }
    let kinds: BTreeMap<_, _> = analysis.elements.iter().map(|e| (e.line, e.kind)).collect();
    let mut events = events.into_iter().peekable();
    let mut traits = [0; 3];
    let mut position = 0;
    let mut byte_offset = 0;
    let mut result = Document::default();
    for (source_line, raw) in source.split_inclusive('\n').enumerate() {
        let kind = kinds.get(&source_line).copied().unwrap_or(Kind::Body);
        let mut atoms = Vec::new();
        let mut chars = raw.char_indices().peekable();
        let mut escaped = false;
        for (index, c) in &mut chars {
            while events.peek().is_some_and(|(at, _)| *at <= position) {
                let (_, values) = events.next().unwrap();
                for i in 0..3 {
                    traits[i] += values[i];
                }
            }
            let omit =
                hidden.get(position).copied().unwrap_or(false) || markers[byte_offset + index];
            let style = Style {
                bold: traits[0] > 0,
                italic: traits[1] > 0,
                underline: traits[2] > 0,
            };
            let source_start = position;
            position += c.len_utf16();
            if omit || c == '\n' || c == '\r' {
                continue;
            }
            if !escaped && c == '\\' {
                // Only punctuation escapes lose their backslash, not paths such as C:\temp.
                let next = raw[index + 1..].chars().next();
                if next.is_some_and(|c| c.is_ascii_punctuation()) {
                    escaped = true;
                    continue;
                }
            }
            let literal = escaped;
            escaped = false;
            if c == '\t' {
                atoms.extend((0..4).map(|_| Atom {
                    c: ' ',
                    style,
                    escaped: literal,
                    source_start,
                }));
            } else {
                atoms.push(Atom {
                    c,
                    style,
                    escaped: literal,
                    source_start,
                });
            }
        }
        byte_offset += raw.len();
        if matches!(kind, Kind::Note | Kind::Omitted | Kind::Marker) {
            continue;
        }
        let output = line(source_line, kind, atoms);
        match kind {
            Kind::Title => {
                let indented = raw.starts_with("   ") || raw.starts_with('\t');
                if !indented && let Some((key, value)) = output.text.split_once(':') {
                    let key = key.trim().to_lowercase();
                    let placement = match key.as_str() {
                        "title" | "credit" | "author" | "authors" | "source" => "center",
                        "contact" | "draft date" => "lowerLeft",
                        _ => "metadata",
                    };
                    let start = output.text.len() - value.trim_start().len();
                    let value = slice_line(&output, start);
                    result.title_fields.push(TitleField {
                        key,
                        placement,
                        values: if value.text.is_empty() {
                            vec![]
                        } else {
                            vec![value]
                        },
                    });
                } else if let Some(field) = result.title_fields.last_mut() {
                    field.values.push(output.clone());
                }
                result.title.push(output);
            }
            Kind::Section | Kind::Synopsis => result.outline.push(output),
            _ => result.body.push(output),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checklist_output_keeps_completion_and_emphasis_offsets_after_replacing_syntax() {
        let output = screenplay(
            "[ ] Open **Zoë** 🌙 [[private]]\r\n[x] Completed _task_\r\n[X] Done\r\n[ ]",
        );
        let todos: Vec<_> = output
            .body
            .iter()
            .filter(|line| line.kind == Kind::Todo)
            .collect();
        assert_eq!(
            todos
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>(),
            ["☐ Open Zoë 🌙", "✓ Completed task", "✓ Done", "☐ "]
        );
        assert_eq!(
            todos
                .iter()
                .map(|line| line.todo_completed)
                .collect::<Vec<_>>(),
            [Some(false), Some(true), Some(true), Some(false)]
        );
        let bold = todos[0].runs.iter().find(|run| run.style.bold).unwrap();
        assert_eq!(bold.start, 7);
        assert_eq!(bold.length, 3);
        assert_eq!(todos[0].source_line, 0);
    }
    #[test]
    fn outline_output_has_clean_text_levels_and_rendered_style_offsets() {
        for newline in ["\n", "\r\n"] {
            let source = format!(
                "## _**Zoë 世界**_{newline}{newline}= *The signal* [[private]]{newline}{newline}/* # Omitted */"
            );
            let doc = screenplay(&source);
            assert_eq!(doc.outline.len(), 2);
            assert_eq!(doc.outline[0].text, "Zoë 世界");
            assert_eq!(doc.outline[0].section_level, Some(2));
            assert_eq!(doc.outline[0].runs[0].start, 0);
            assert_eq!(
                doc.outline[0].runs[0].length,
                "Zoë 世界".encode_utf16().count()
            );
            assert!(doc.outline[0].runs[0].style.bold && doc.outline[0].runs[0].style.underline);
            assert_eq!(doc.outline[1].text, "The signal");
            assert_eq!(doc.outline[1].section_level, None);
            assert!(doc.outline[1].runs[0].style.italic);
            assert_eq!(doc.outline[1].source_line, 2);
        }
    }
    #[test]
    fn escaped_action_marker_is_printed_literally() {
        let doc = screenplay(r"\!Keep this punctuation.");
        assert_eq!(doc.body[0].text, "!Keep this punctuation.");
    }
    #[test]
    fn output_removes_source_controls_and_keeps_semantics() {
        let source = "Title: Example\n\n# Act One\n\n= Outline only\n\n**INT. ROOM - DAY #1A#**\n\n@Zoë ^\n(quietly)\nHello **world** [[private]].\n\n> THE END <\n\n~Sing.\n\n===\n\n/* Secret */";
        let doc = screenplay(source);
        assert_eq!(doc.title[0].text, "Title: Example");
        assert_eq!(doc.outline.len(), 2);
        let scene = doc.body.iter().find(|l| l.kind == Kind::Scene).unwrap();
        assert_eq!(scene.text, "INT. ROOM - DAY");
        assert_eq!(scene.scene_number.as_deref(), Some("1A"));
        let cue = doc.body.iter().find(|l| l.kind == Kind::Character).unwrap();
        assert_eq!(cue.text, "Zoë");
        assert!(cue.dual_dialogue);
        assert!(
            doc.body
                .iter()
                .any(|l| l.kind == Kind::Centered && l.text == "THE END")
        );
        assert!(
            doc.body
                .iter()
                .any(|l| l.kind == Kind::PageBreak && l.text.is_empty())
        );
        assert!(
            !doc.body
                .iter()
                .any(|l| l.text.contains("private") || l.text.contains("Secret"))
        );
    }
    #[test]
    fn output_style_ranges_match_unicode_after_removing_markers_and_escapes() {
        for newline in ["\n", "\r\n"] {
            let source = format!("!🌙 _**Zoë**_ says \\*hello\\*.{newline}{newline}~A song");
            let doc = screenplay(&source);
            let line = &doc.body[0];
            assert_eq!(line.text, "🌙 Zoë says *hello*.");
            let units: Vec<u16> = line.text.encode_utf16().collect();
            let bold = line.runs.iter().find(|r| r.style.bold).unwrap();
            assert!(bold.style.underline);
            assert_eq!(
                String::from_utf16(&units[bold.start..bold.start + bold.length]).unwrap(),
                "Zoë"
            );
            assert_eq!(
                line.runs.iter().map(|r| r.length).sum::<usize>(),
                units.len()
            );
            assert!(
                doc.body
                    .iter()
                    .any(|l| l.kind == Kind::Lyric && l.text == "A song")
            );
        }
    }
}

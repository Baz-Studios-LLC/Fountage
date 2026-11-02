//! Derived, format-specific editing structure. This is not a production paginator.
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Format {
    Screenplay,
    Comic,
    Novel,
    Academic,
    Stageplay,
    Audio,
    General,
    Poetry,
}
impl Format {
    /// Stable identifiers shared with the Swift reference and package reader.
    /// Selecting a preset records intent; it does not certify output-template parity.
    pub fn layout_presets(self) -> &'static [&'static str] {
        match self {
            Self::Screenplay => &["screenplay-standard"],
            Self::Comic => &["comic-full-script", "comic-lettering-friendly"],
            Self::Novel => &["novel-manuscript", "novel-reading"],
            Self::Academic => &["academic-draft"],
            Self::Stageplay => &["stage-draft"],
            Self::Audio => &["audio-drama", "audio-podcast"],
            Self::General => &["general-draft", "general-outline"],
            Self::Poetry => &["poetry-standard", "poetry-lyrics"],
        }
    }
    pub fn starter_for_preset(self, preset: &str) -> Result<&'static str, &'static str> {
        if !self.layout_presets().contains(&preset) {
            return Err("Unsupported layout preset for writing category.");
        }
        Ok(match preset {
            "audio-podcast" => {
                "Title: Untitled Podcast\nAuthor: \n\nSEGMENT 1 - INTRODUCTION\n\nMUSIC:\nOpening theme, then fade under.\n\nHOST:\nWelcome to the show.\n\nSEGMENT 2 - CONVERSATION\n\nHOST:\nThe first question.\n\nGUEST:\nThe first answer.\n"
            }
            "poetry-lyrics" => {
                "Title: Untitled Song\nAuthor: \n\nVERSE 1\nThe first line of your verse.\nThe next line.\n\nCHORUS\nThe words we come back to.\n\nVERSE 2\nA new thought begins.\n\nBRIDGE\nA change in direction.\n"
            }
            "general-outline" => {
                "Title: Untitled Outline\nAuthor: \n\n# The idea\n\nDescribe the central idea.\n\n## Key points\n\n- First point\n- Second point\n\n# Next steps\n\nWhat happens next?\n"
            }
            _ => self.starter(),
        })
    }
    /// Familiar, editable starting manuscripts shared by desktop clients.
    /// Academic starters are drafting structure, not citation-style compliance.
    pub fn starter(self) -> &'static str {
        match self {
            Self::Screenplay => {
                "Title: Untitled Screenplay\nAuthor: \n\nINT. ROOM - DAY\n\nDescribe the action.\n\nCHARACTER\nThe first line of dialogue.\n"
            }
            Self::Comic => {
                "Title: Untitled Comic\nIssue: 1\nAuthor: \n\nPAGE 1 (2 panels)\n\nPanel 1\nDescribe one drawable moment: the setting, characters, and action.\n\nCHARACTER:\nThe first line of dialogue.\n\nCAPTION:\nA moment earlier.\n\nPanel 2\nDescribe the next moment.\n\nSFX:\nCLICK\n"
            }
            Self::Novel => {
                "Title: Untitled Book\nAuthor: \n\nChapter One\n\nBegin your story here.\n\nThe next paragraph.\n\n***\n\nA new scene begins.\n"
            }
            Self::Stageplay => {
                "Title: Untitled Play\nAuthor: \n\nACT I\n\nSCENE 1\n\n(A room at dawn. Describe the setting and opening action.)\n\nCHARACTER\nThe first line of dialogue.\n\n(An action between lines.)\n\nCHARACTER\nThe next line of dialogue.\n"
            }
            Self::Audio => {
                "Title: Untitled Audio Drama\nAuthor: \n\nSCENE 1 - THE ARRIVAL\n\nSFX:\nFootsteps approach, then stop.\n\nCHARACTER:\nThe first line of dialogue.\n\nMUSIC:\nA brief transition.\n\nSCENE 2 - A NEW VOICE\n\nCHARACTER:\nThe next line of dialogue.\n"
            }
            Self::General => {
                "Title: Untitled Draft\nAuthor: \n\n# An idea\n\nBegin writing here.\n\n# A new direction\n\nContinue your thought.\n"
            }
            Self::Poetry => {
                "Title: Untitled Collection\nAuthor: \n\n# First Poem\n\nBegin a line here.\nLet the next line find its rhythm.\n\nA new stanza begins.\n"
            }
            Self::Academic => {
                "Title: Untitled Paper\nAuthor: \n\n# Introduction\n\nIntroduce your question and central argument.\n\n# Discussion\n\nDevelop your argument and record your sources.\n\n# Conclusion\n\nBring your findings together.\n"
            }
        }
    }
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            0 => Some(Self::Screenplay),
            1 => Some(Self::Comic),
            2 => Some(Self::Novel),
            3 => Some(Self::Academic),
            4 => Some(Self::Stageplay),
            5 => Some(Self::Audio),
            6 => Some(Self::General),
            7 => Some(Self::Poetry),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Body,
    Title,
    Scene,
    Character,
    Dialogue,
    Parenthetical,
    Transition,
    Note,
    Section,
    Page,
    Panel,
    Balloon,
    Caption,
    SoundEffect,
    Chapter,
    SceneBreak,
    Heading,
    Act,
    Verse,
    Synopsis,
    Centered,
    Lyric,
    PageBreak,
    Omitted,
    Marker,
    Todo,
}
#[derive(Debug, Serialize)]
pub struct Element {
    pub line: usize,
    pub kind: Kind,
}
#[derive(Debug, Serialize)]
pub struct Heading {
    pub line: usize,
    pub heading: String,
    pub kind: Kind,
    pub level: u8,
}
#[derive(Debug, Serialize)]
pub struct ScriptNote {
    pub line: usize,
    // UTF-16 offsets match AppKit's text selection coordinates, including CRLF and emoji.
    pub start: usize,
    pub length: usize,
    pub text: String,
}
#[derive(Debug, Serialize)]
pub struct SourceRange {
    pub start: usize,
    pub length: usize,
}
#[derive(Debug, Default, Serialize)]
pub struct Analysis {
    pub emphasis: Vec<crate::emphasis::Emphasis>,
    pub headings: Vec<Heading>,
    pub notes: Vec<ScriptNote>,
    pub markers: Vec<ScriptNote>,
    pub todos: Vec<crate::todo::Todo>,
    pub omissions: Vec<SourceRange>,
    pub characters: BTreeMap<String, usize>,
    pub locations: BTreeMap<String, Vec<usize>>,
    pub words: usize,
    pub character_count: usize,
    pub word_ranges: Vec<SourceRange>,
    pub elements: Vec<Element>,
    pub units: usize,
    pub panels: usize,
}
fn metadata(line: &str) -> bool {
    let Some((key, _)) = line.split_once(':') else {
        return false;
    };
    matches!(
        key.to_lowercase().as_str(),
        "title" | "author" | "authors" | "credit" | "draft date" | "contact" | "source" | "issue"
    )
}
fn numbered_heading(line: &str, prefix: &str) -> bool {
    let upper = line.to_uppercase();
    let Some(rest) = upper.strip_prefix(prefix) else {
        return false;
    };
    let token = rest
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_end_matches(['.', ':', '-']);
    !token.is_empty()
        && (token.chars().next().is_some_and(|c| c.is_ascii_digit())
            || matches!(
                token,
                "ONE"
                    | "TWO"
                    | "THREE"
                    | "FOUR"
                    | "FIVE"
                    | "SIX"
                    | "SEVEN"
                    | "EIGHT"
                    | "NINE"
                    | "TEN"
                    | "ELEVEN"
                    | "TWELVE"
                    | "THIRTEEN"
                    | "FOURTEEN"
                    | "FIFTEEN"
                    | "SIXTEEN"
                    | "SEVENTEEN"
                    | "EIGHTEEN"
                    | "NINETEEN"
                    | "TWENTY"
            ))
}
fn comic_cue(line: &str) -> Option<(Kind, String)> {
    let (label, _) = line.split_once(':')?;
    // Full scripts sometimes number lettering items: `1. MARA: ...`.
    let label = label.trim_start_matches(|c: char| {
        c.is_ascii_digit() || c == '.' || c == ')' || c.is_whitespace()
    });
    let base = label.split(" (").next().unwrap_or(label).trim();
    if base.is_empty() || !base.chars().any(char::is_alphabetic) || base != base.to_uppercase() {
        return None;
    }
    Some((
        match base {
            "SFX" | "SOUND" | "SOUND EFFECT" => Kind::SoundEffect,
            "CAP" | "CAPTION" | "CAPTIONS" => Kind::Caption,
            _ => Kind::Balloon,
        },
        base.to_owned(),
    ))
}
fn markdown_heading(line: &str) -> Option<(u8, &str)> {
    let level = line.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&level) || line.as_bytes().get(level) != Some(&b' ') {
        return None;
    }
    let title = line[level + 1..].trim();
    if title.is_empty() {
        None
    } else {
        Some(((level - 1) as u8, title))
    }
}
fn dramatic_heading(line: &str, prefix: &str) -> bool {
    if numbered_heading(line, prefix) {
        return true;
    }
    let upper = line.to_uppercase();
    let token = upper
        .strip_prefix(prefix)
        .unwrap_or("")
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_end_matches(['.', ':']);
    !token.is_empty()
        && token
            .chars()
            .all(|c| matches!(c, 'I' | 'V' | 'X' | 'L' | 'C'))
}
fn verse_heading(line: &str) -> bool {
    let label = line.trim_matches(['[', ']']).to_uppercase();
    numbered_heading(&label, "VERSE ")
        || numbered_heading(&label, "CHORUS ")
        || matches!(
            label.as_str(),
            "VERSE" | "CHORUS" | "PRE-CHORUS" | "BRIDGE" | "REFRAIN" | "INTRO" | "OUTRO"
        )
}
fn stage_character<'a>(lines: &[&'a str], index: usize) -> Option<&'a str> {
    let line = lines[index].trim();
    if index > 0 && !lines[index - 1].trim().is_empty() {
        return None;
    }
    if lines
        .get(index + 1)
        .is_none_or(|line| line.trim().is_empty())
    {
        return None;
    }
    let name = line
        .trim_start_matches('@')
        .split(" (")
        .next()
        .unwrap_or("")
        .trim();
    if name.chars().any(char::is_alphabetic)
        && (line.starts_with('@') || name == name.to_uppercase())
    {
        Some(name)
    } else {
        None
    }
}
pub fn analyze(source: &str, format: Format) -> Analysis {
    if format == Format::Screenplay {
        return crate::fountain::analyze(source);
    }
    let (visible, markers, notes, character_count, todos) = crate::fountain::marker_source(source);
    let visible = crate::todo::mask_states(&visible, &todos);
    let lines: Vec<&str> = visible.lines().collect();
    let mut result = Analysis {
        words: visible.split_whitespace().count(),
        character_count,
        markers,
        notes,
        todos,
        ..Analysis::default()
    };
    // Preserve the category’s whitespace-token policy while sharing exact
    // source ranges with selection counting. Marker masks never shift offsets.
    let mut positions = vec![0; source.len() + 1];
    let mut utf16 = 0;
    for (start, c) in source.char_indices() {
        positions[start..start + c.len_utf8()].fill(utf16);
        utf16 += c.len_utf16();
    }
    positions[source.len()] = utf16;
    result.word_ranges = visible
        .split_whitespace()
        .map(|token| {
            let start = token.as_ptr() as usize - visible.as_ptr() as usize;
            SourceRange {
                start: positions[start],
                length: positions[start + token.len()] - positions[start],
            }
        })
        .collect();
    let mut dialogue = false;
    let mut in_title = lines.first().is_some_and(|line| metadata(line.trim()));
    let mut after_break = true;
    let mut has_act = false;
    let mut audio_technical = false;
    let mut in_stanza = false;
    for (i, raw) in lines.iter().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            if result.markers.iter().any(|marker| marker.line == i) {
                result.elements.push(Element {
                    line: i,
                    kind: Kind::Marker,
                });
                continue;
            }
            dialogue = false;
            in_title = false;
            in_stanza = false;
            continue;
        }
        let kind = if result.todos.iter().any(|todo| todo.line == i) {
            dialogue = false;
            Kind::Todo
        } else if in_title && (metadata(line) || raw.starts_with([' ', '\t'])) {
            Kind::Title
        } else {
            match format {
                Format::Screenplay => unreachable!("screenplays use the shared Fountain parser"),
                Format::Comic => {
                    if numbered_heading(line, "PAGE ") {
                        dialogue = false;
                        result.units += 1;
                        result.headings.push(Heading {
                            line: i,
                            heading: line.into(),
                            kind: Kind::Page,
                            level: 0,
                        });
                        Kind::Page
                    } else if numbered_heading(line, "PANEL ") {
                        dialogue = false;
                        result.panels += 1;
                        result.headings.push(Heading {
                            line: i,
                            heading: line.into(),
                            kind: Kind::Panel,
                            level: 1,
                        });
                        Kind::Panel
                    } else if let Some((kind, name)) = comic_cue(line) {
                        dialogue = true;
                        if kind == Kind::Balloon {
                            *result.characters.entry(name).or_default() += 1;
                        }
                        kind
                    } else if dialogue {
                        Kind::Dialogue
                    } else {
                        Kind::Body
                    }
                }
                Format::Academic | Format::General => {
                    if let Some((level, title)) = markdown_heading(line) {
                        result.units += 1;
                        result.headings.push(Heading {
                            line: i,
                            heading: title.into(),
                            kind: Kind::Heading,
                            level,
                        });
                        Kind::Heading
                    } else {
                        Kind::Body
                    }
                }
                Format::Stageplay => {
                    if dramatic_heading(line, "ACT ") {
                        dialogue = false;
                        has_act = true;
                        result.headings.push(Heading {
                            line: i,
                            heading: line.into(),
                            kind: Kind::Act,
                            level: 0,
                        });
                        Kind::Act
                    } else if dramatic_heading(line, "SCENE ") {
                        dialogue = false;
                        result.units += 1;
                        result.headings.push(Heading {
                            line: i,
                            heading: line.into(),
                            kind: Kind::Scene,
                            level: u8::from(has_act),
                        });
                        Kind::Scene
                    } else if line.starts_with('(') && line.ends_with(')') {
                        Kind::Parenthetical
                    } else if let Some(name) = stage_character(&lines, i) {
                        dialogue = true;
                        *result.characters.entry(name.into()).or_default() += 1;
                        Kind::Character
                    } else if dialogue {
                        Kind::Dialogue
                    } else {
                        Kind::Body
                    }
                }
                Format::Audio => {
                    if dramatic_heading(line, "SCENE ") || numbered_heading(line, "SEGMENT ") {
                        dialogue = false;
                        result.units += 1;
                        result.headings.push(Heading {
                            line: i,
                            heading: line.into(),
                            kind: Kind::Scene,
                            level: 0,
                        });
                        Kind::Scene
                    } else if let Some((level, title)) = markdown_heading(line) {
                        dialogue = false;
                        result.units += 1;
                        result.headings.push(Heading {
                            line: i,
                            heading: title.into(),
                            kind: Kind::Heading,
                            level,
                        });
                        Kind::Heading
                    } else if let Some((kind, name)) = comic_cue(line) {
                        dialogue = true;
                        audio_technical = kind != Kind::Balloon
                            || matches!(name.as_str(), "MUSIC" | "AMBIENCE" | "FX");
                        if audio_technical {
                            Kind::SoundEffect
                        } else {
                            *result.characters.entry(name).or_default() += 1;
                            Kind::Character
                        }
                    } else if line.starts_with('(') && line.ends_with(')') {
                        Kind::Parenthetical
                    } else if dialogue {
                        if audio_technical {
                            Kind::Note
                        } else {
                            Kind::Dialogue
                        }
                    } else {
                        Kind::Body
                    }
                }
                Format::Poetry => {
                    if let Some((level, title)) = markdown_heading(line) {
                        in_stanza = false;
                        result.headings.push(Heading {
                            line: i,
                            heading: title.into(),
                            kind: Kind::Heading,
                            level,
                        });
                        Kind::Heading
                    } else if verse_heading(line) {
                        in_stanza = false;
                        result.headings.push(Heading {
                            line: i,
                            heading: line.into(),
                            kind: Kind::Verse,
                            level: 0,
                        });
                        Kind::Verse
                    } else {
                        if !in_stanza {
                            result.units += 1;
                            in_stanza = true;
                        }
                        Kind::Body
                    }
                }
                Format::Novel => {
                    if line.starts_with("# ")
                        || line.to_uppercase().starts_with("CHAPTER ")
                        || matches!(line.to_uppercase().as_str(), "PROLOGUE" | "EPILOGUE")
                    {
                        result.units += 1;
                        after_break = true;
                        result.headings.push(Heading {
                            line: i,
                            heading: line.trim_start_matches("# ").into(),
                            kind: Kind::Chapter,
                            level: 0,
                        });
                        Kind::Chapter
                    } else if matches!(line, "***" | "* * *" | "---" | "#") {
                        after_break = true;
                        Kind::SceneBreak
                    } else if after_break {
                        after_break = false;
                        Kind::Section
                    } else {
                        Kind::Body
                    }
                }
            }
        };
        result.elements.push(Element { line: i, kind });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn markers_work_in_every_category_without_changing_its_structure() {
        for (format, source) in [
            (Format::Comic, "PAGE 1\r\n\r\nPANEL 1\r\nArt."),
            (Format::Novel, "CHAPTER ONE\r\n\r\nProse."),
            (Format::Academic, "# Abstract\r\n\r\nResearch."),
            (
                Format::Stageplay,
                "ACT ONE\r\n\r\nSCENE ONE\r\n\r\nMARA\r\nHello.",
            ),
            (Format::Audio, "SCENE ONE\r\n\r\nMARA: Hello."),
            (Format::General, "# Ideas\r\n\r\nWriting."),
            (Format::Poetry, "# Verse\r\n\r\nQuiet lines."),
        ] {
            let marked = source.replacen("\r\n", "{{%m: Zoë 🌙}}\r\n", 1) + "\r\n{{%m}}";
            let plain = analyze(source, format);
            let result = analyze(&marked, format);
            assert_eq!(result.markers.len(), 2);
            assert_eq!(result.markers[0].text, "Zoë 🌙");
            assert_eq!(result.words, plain.words);
            assert_eq!(result.character_count, plain.character_count);
            assert_eq!(result.units, plain.units);
            assert_eq!(result.panels, plain.panels);
            assert_eq!(result.characters, plain.characters);
            assert_eq!(
                serde_json::to_value(&result.headings).unwrap(),
                serde_json::to_value(&plain.headings).unwrap()
            );
            let units: Vec<_> = marked.encode_utf16().collect();
            for marker in result.markers {
                let directive =
                    String::from_utf16(&units[marker.start..marker.start + marker.length]).unwrap();
                assert!(directive.starts_with("{{%m"));
            }
        }
    }
    #[test]
    fn non_screenplay_marker_masking_preserves_other_existing_markup() {
        let source = "# Heading\n\n[[{{%m: private}}]] /*{{%m: omitted}}*/\n\n{{%m: public}}";
        let result = analyze(source, Format::General);
        assert_eq!(result.markers.len(), 1);
        assert_eq!(result.markers[0].text, "public");
        let original = "# Heading\n\n[[{{%m: private}}]] /*{{%m: omitted}}*/";
        assert_eq!(result.character_count, original.encode_utf16().count());
        assert_eq!(result.words, original.split_whitespace().count());
    }
    #[test]
    fn comic_pages_panels_and_lettering_stay_distinct() {
        let a = analyze(
            "Title: Test\nIssue: 1\n\nPAGE ONE (2 panels)\n\nPanel 1. A door.\n\n1. MARA: Hello.\nCAPTION: Earlier.\nSFX: BOOM\n\nPanel 2\nMARA (whisper): Run.\n\nPAGE 2\nPanel 1\n2) CAP:\nLater.\n\n3. ELI (OP):\nOkay.",
            Format::Comic,
        );
        assert_eq!((a.units, a.panels), (2, 3));
        assert_eq!(a.characters.get("MARA"), Some(&2));
        assert_eq!(a.characters.get("ELI"), Some(&1));
        assert_eq!(a.characters.len(), 2);
        assert_eq!(a.headings[1].level, 1);
        assert_eq!(
            a.elements
                .iter()
                .filter(|e| e.kind == Kind::Caption)
                .count(),
            2
        );
        assert_eq!(a.elements.last().unwrap().kind, Kind::Dialogue);
    }
    #[test]
    fn novels_have_chapters_and_no_screenplay_characters() {
        let source = "Title: Test\n\nChapter One\n\nINT. ROOM - DAY\n\nALICE\nSaid nothing.\n\n***\n\nA new scene.\n\n# Chapter Two\n\nThe end.";
        let a = analyze(source, Format::Novel);
        assert_eq!(a.units, 2);
        assert!(a.characters.is_empty());
        assert_eq!(a.headings[1].heading, "Chapter Two");
        assert!(a.elements.iter().any(|e| e.kind == Kind::SceneBreak));
    }
    #[test]
    fn screenplay_dialogue_gets_structural_roles() {
        let a = analyze(
            "INT. ROOM - DAY\n\nALICE\n(quietly)\nHello.\n\nAction resumes.",
            Format::Screenplay,
        );
        assert_eq!(
            a.elements.iter().map(|e| e.kind).collect::<Vec<_>>(),
            [
                Kind::Scene,
                Kind::Character,
                Kind::Parenthetical,
                Kind::Dialogue,
                Kind::Body
            ]
        );
    }
    #[test]
    fn academic_sections_do_not_imply_a_citation_standard() {
        let a = analyze(
            "Title: Paper\n\n# Introduction\n\nEvidence (Smith, 2020).\n\n## Context\n\nA paragraph.\n\n# Conclusion",
            Format::Academic,
        );
        assert_eq!(a.units, 3);
        assert_eq!(a.headings[1].level, 1);
        assert_eq!(a.headings[1].heading, "Context");
        assert!(a.characters.is_empty());
    }
    #[test]
    fn prose_is_not_a_comic_heading() {
        let a = analyze(
            "Page layouts vary.\nPanel discussions are fun.",
            Format::Comic,
        );
        assert!(a.headings.is_empty());
        assert!(Format::from_code(99).is_none());
    }
    #[test]
    fn stage_acts_scenes_and_directions_do_not_become_characters() {
        let a = analyze(
            "Title: A Play\n\nACT I\n\nSCENE 1\n\n(A quiet room.)\n\nMARA\nHello.\n\nACT II\n\nSCENE II\n\n@Zoë\n(softly)\nWelcome.",
            Format::Stageplay,
        );
        assert_eq!(a.units, 2);
        assert_eq!(
            a.headings.iter().map(|h| h.kind).collect::<Vec<_>>(),
            [Kind::Act, Kind::Scene, Kind::Act, Kind::Scene]
        );
        assert_eq!(a.headings[1].level, 1);
        assert_eq!(a.characters.len(), 2);
        assert_eq!(a.characters.get("Zoë"), Some(&1));
        assert!(a.elements.iter().any(|e| e.kind == Kind::Parenthetical));
    }
    #[test]
    fn audio_technical_cues_are_not_speakers() {
        let a = analyze(
            "SEGMENT 1 - OPENING\n\nMUSIC:\nTheme music.\n\nHOST:\nWelcome.\n\nSFX:\nA door.\n\nGUEST (quietly):\nHello.\n\nSCENE 2\n\nHOST: Ready?",
            Format::Audio,
        );
        assert_eq!(a.units, 2);
        assert_eq!(a.characters.len(), 2);
        assert_eq!(a.characters.get("HOST"), Some(&2));
        assert_eq!(a.characters.get("GUEST"), Some(&1));
        assert!(a.elements.iter().any(|e| e.kind == Kind::Note));
    }
    #[test]
    fn inline_notes_retain_exact_ranges_in_every_category() {
        let source = "🌙 [[Repeat Café]]\r\n/* [[hidden]] */\r\n[[Repeat Café]]";
        for code in 0..8 {
            let analysis = analyze(source, Format::from_code(code).unwrap());
            assert_eq!(analysis.notes.len(), 2);
            let utf16 = source.encode_utf16().collect::<Vec<_>>();
            for note in analysis.notes {
                assert_eq!(note.text, "Repeat Café");
                assert_eq!(
                    String::from_utf16(&utf16[note.start..note.start + note.length]).unwrap(),
                    "[[Repeat Café]]"
                );
            }
        }
    }
    #[test]
    fn poetry_counts_stanzas_and_preserves_source_line_positions() {
        let source = "Title: Song\n\n[VERSE 1]\n  A line,\n    another.\n\nCHORUS\n世界\nagain\n\n# Second Poem\n\nONE\nTWO\n";
        let a = analyze(source, Format::Poetry);
        assert_eq!(a.units, 3);
        assert_eq!(
            a.headings.iter().map(|h| h.line).collect::<Vec<_>>(),
            [2, 6, 10]
        );
        assert!(a.characters.is_empty());
        assert_eq!(
            a.elements.iter().find(|e| e.line == 12).unwrap().kind,
            Kind::Body
        );
    }
    #[test]
    fn general_writing_uses_sections_instead_of_screenplay_rules() {
        let a = analyze(
            "# Notes\n\nINT. ROOM - DAY\n\nALICE\nHello.\n\n## Next steps\n\nA thought.",
            Format::General,
        );
        assert_eq!(a.units, 2);
        assert!(a.characters.is_empty());
        assert_eq!(a.headings[1].level, 1);
    }
}

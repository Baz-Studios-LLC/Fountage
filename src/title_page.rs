//! Source-preserving edits of the six standard Fountain title-page fields.
use crate::writing::{Format, Kind};
use std::collections::BTreeSet;
pub const LABELS: [&str; 6] = [
    "Title",
    "Credit",
    "Author",
    "Source",
    "Draft date",
    "Contact",
];
const COMPLEX: &str =
    "This title page uses syntax the form cannot safely update. Edit its fields in the manuscript.";
const INVALID: &str = "Use consecutive lines within each field. Blank lines end a Fountain title page; null characters are not supported.";
struct Block {
    key: String,
    label: String,
    range: std::ops::Range<usize>,
    value: String,
}
pub struct Draft {
    source: String,
    blocks: Vec<Block>,
    end: usize,
    newline: &'static str,
    pub values: [String; 6],
}
fn canonical(label: &str) -> String {
    let key = label.trim().to_lowercase();
    if key == "authors" {
        "author".into()
    } else {
        key
    }
}
impl Draft {
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn parse(source: &str) -> Result<Self, &'static str> {
        if source.contains('\0') {
            return Err(INVALID);
        }
        let analysis = crate::writing::analyze(source, Format::Screenplay);
        let titles: BTreeSet<_> = analysis
            .elements
            .iter()
            .filter(|e| e.kind == Kind::Title)
            .map(|e| e.line)
            .collect();
        let mut blocks: Vec<Block> = Vec::new();
        let mut offset = 0;
        let mut end = 0;
        for (index, raw) in source.split_inclusive('\n').enumerate() {
            let start = offset;
            offset += raw.len();
            let text = raw
                .trim_end_matches('\n')
                .strip_suffix('\r')
                .unwrap_or(raw.trim_end_matches('\n'));
            if !titles.contains(&index) {
                if !blocks.is_empty() && titles.last().is_some_and(|last| index < *last) {
                    return Err(COMPLEX);
                }
                continue;
            }
            if start != end {
                return Err(COMPLEX);
            }
            let indented = text.starts_with("   ") || text.starts_with('\t');
            if !indented && let Some((label, value)) = text.split_once(':') {
                let label = label.trim();
                if label.is_empty() || !label.chars().all(|c| c.is_alphabetic() || c == ' ') {
                    return Err(COMPLEX);
                }
                blocks.push(Block {
                    key: canonical(label),
                    label: label.into(),
                    range: start..offset,
                    value: value.trim().into(),
                });
            } else if indented && !blocks.is_empty() {
                let block = blocks.last_mut().unwrap();
                if !block.value.is_empty() {
                    block.value.push('\n');
                }
                block.value.push_str(text.trim());
                block.range.end = offset;
            } else {
                return Err(COMPLEX);
            }
            end = offset;
        }
        let values = LABELS.map(|label| {
            blocks
                .iter()
                .filter(|b| b.key == canonical(label) && !b.value.is_empty())
                .map(|b| b.value.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        });
        Ok(Self {
            source: source.into(),
            blocks,
            end,
            newline: if source.contains("\r\n") {
                "\r\n"
            } else {
                "\n"
            },
            values,
        })
    }
    pub fn edit(&self, values: &[String; 6]) -> Result<Option<String>, &'static str> {
        let changed: BTreeSet<_> = (0..6).filter(|&i| values[i] != self.values[i]).collect();
        if changed.is_empty() {
            return Ok(None);
        }
        let render = |index: usize, label: &str| -> Result<String, &'static str> {
            let value = values[index].replace("\r\n", "\n");
            let value = value.trim_matches('\n');
            if value.contains(['\0', '\r']) {
                return Err(INVALID);
            }
            if value.is_empty() {
                return Ok(String::new());
            }
            let lines: Vec<_> = value.split('\n').collect();
            if lines.iter().any(|line| line.trim().is_empty()) {
                return Err(INVALID);
            }
            Ok(if lines.len() == 1 {
                format!("{label}: {}{}", lines[0], self.newline)
            } else {
                format!(
                    "{label}:{}{}",
                    self.newline,
                    lines
                        .iter()
                        .map(|line| format!("    {line}{}", self.newline))
                        .collect::<String>()
                )
            })
        };
        let mut replacement = String::new();
        let mut emitted = BTreeSet::new();
        for block in &self.blocks {
            let index = LABELS
                .iter()
                .position(|label| canonical(label) == block.key);
            if let Some(index) = index.filter(|i| changed.contains(i)) {
                if emitted.insert(index) {
                    replacement.push_str(&render(index, &block.label)?);
                }
            } else {
                replacement.push_str(&self.source[block.range.clone()]);
            }
        }
        for index in changed {
            if !emitted.contains(&index) {
                let rendered = render(index, LABELS[index])?;
                if !rendered.is_empty() && !replacement.is_empty() && !replacement.ends_with('\n') {
                    replacement.push_str(self.newline);
                }
                replacement.push_str(&rendered);
            }
        }
        if !replacement.is_empty() {
            let first = canonical(replacement.split(':').next().unwrap());
            if ![
                "title",
                "credit",
                "author",
                "source",
                "draft date",
                "contact",
                "copyright",
                "notes",
            ]
            .contains(&first.as_str())
            {
                replacement = format!("Title:{}{replacement}", self.newline);
            }
            if self.end == 0 {
                replacement.push_str(self.newline);
            }
        }
        replacement.push_str(&self.source[self.end..]);
        Ok((replacement != self.source).then_some(replacement))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_changed_known_blocks_are_rewritten_with_body_and_custom_bytes_preserved() {
        for nl in ["\n", "\r\n"] {
            let header = format!(
                "Title: **Old**{nl}Credit: Written by{nl}Authors: Zoë{nl}    世界{nl}Custom Field:  keep  {nl}\tOpaque 🌙{nl}Draft date: Yesterday{nl}"
            );
            let body = format!("{nl}INT. CAFÉ - DAY #1#{nl}{nl}MARA{nl}Hello. [[private]]{nl}");
            let source = format!("{header}{body}");
            let draft = Draft::parse(&source).unwrap();
            assert_eq!(draft.values[0], "**Old**");
            assert_eq!(draft.values[2], "Zoë\n世界");
            assert!(draft.edit(&draft.values).unwrap().is_none());
            let mut values = draft.values.clone();
            values[0] = "The Moon 🌙".into();
            values[5] = "Zoë\nwriter@example.invalid".into();
            let updated = draft.edit(&values).unwrap().unwrap();
            assert!(updated.ends_with(&body));
            assert!(updated.contains(&format!(
                "Authors: Zoë{nl}    世界{nl}Custom Field:  keep  {nl}\tOpaque 🌙{nl}"
            )));
            assert!(updated.contains(&format!(
                "Contact:{nl}    Zoë{nl}    writer@example.invalid{nl}"
            )));
            let reopened = Draft::parse(&updated).unwrap();
            assert_eq!(reopened.values, values);
            let output = crate::output::screenplay(&updated);
            assert!(
                output
                    .title_fields
                    .iter()
                    .any(|field| field.key == "custom field")
            );
            assert!(
                output
                    .body
                    .iter()
                    .any(|line| line.text == "INT. CAFÉ - DAY")
            );
        }
    }
    #[test]
    fn new_headers_aliases_duplicates_and_custom_anchor_remain_parseable() {
        let body = "INT. ROOM - DAY\r\n\r\nAction. 🌙\r\n";
        let draft = Draft::parse(body).unwrap();
        let mut values = draft.values.clone();
        values[0] = "Café".into();
        let updated = draft.edit(&values).unwrap().unwrap();
        assert_eq!(updated, format!("Title: Café\r\n\r\n{body}"));
        let source = "Title: Old\nCustom Field: Opaque\nAuthor: A\nAuthors: B\n\nINT. ROOM - DAY";
        let draft = Draft::parse(source).unwrap();
        assert_eq!(draft.values[2], "A\nB");
        let mut values = draft.values.clone();
        values[0].clear();
        values[2] = "C".into();
        let updated = draft.edit(&values).unwrap().unwrap();
        assert_eq!(
            updated,
            "Title:\nCustom Field: Opaque\nAuthor: C\n\nINT. ROOM - DAY"
        );
        assert_eq!(Draft::parse(&updated).unwrap().values[2], "C");
        let draft = Draft::parse("").unwrap();
        assert!(draft.edit(&draft.values).unwrap().is_none());
        let draft = Draft::parse("Title: Old").unwrap();
        let mut values = draft.values.clone();
        values[2] = "A".into();
        assert_eq!(
            draft.edit(&values).unwrap().unwrap(),
            "Title: Old\nAuthor: A\n"
        );
    }
    #[test]
    fn invalid_values_or_hidden_header_gaps_fail_without_mutation() {
        assert!(Draft::parse("Title: Name\n/* hidden */\nAuthor: A\n\nINT. ROOM - DAY").is_err());
        assert!(Draft::parse("Title: Name\0").is_err());
        let source = "Title: Original\n\nINT. ROOM - DAY\n";
        let draft = Draft::parse(source).unwrap();
        for bad in ["A\n\nB", "A\rB", "A\0B", "A\n  \nB"] {
            let mut values = draft.values.clone();
            values[0] = bad.into();
            assert!(draft.edit(&values).is_err());
            assert_eq!(draft.source(), source);
        }
    }
}

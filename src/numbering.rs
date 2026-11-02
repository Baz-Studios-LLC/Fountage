//! Source-preserving, atomic scene numbering edits. All coordinates are UTF-16.
use crate::writing::Kind;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub struct Patch {
    pub start: usize,
    pub length: usize,
    pub replacement: String,
}
#[derive(Debug, Serialize)]
pub struct Change {
    pub start: usize,
    pub length: usize,
    pub replacement: String,
    pub patches: Vec<Patch>,
}
#[derive(Serialize)]
pub struct Response {
    pub change: Option<Change>,
    pub error: Option<&'static str>,
}

pub fn response(source: &str, start: u32, remove: bool) -> Response {
    match edit(source, start, remove) {
        Ok(change) => Response {
            change,
            error: None,
        },
        Err(error) => Response {
            change: None,
            error: Some(error),
        },
    }
}
pub fn edit(source: &str, start: u32, remove: bool) -> Result<Option<Change>, &'static str> {
    edit_range(source, start, remove, None)
}
pub fn edit_range(
    source: &str,
    start: u32,
    remove: bool,
    scope: Option<std::ops::Range<usize>>,
) -> Result<Option<Change>, &'static str> {
    if source.contains('\0') || start == 0 {
        return Err("The source or starting scene number is invalid. No text was changed.");
    }
    let mut boundaries = BTreeMap::new();
    let mut units = 0;
    for (byte, character) in source.char_indices() {
        boundaries.insert(units, byte);
        units += character.len_utf16();
    }
    boundaries.insert(units, source.len());
    if scope.as_ref().is_some_and(|r| {
        r.start >= r.end || !boundaries.contains_key(&r.start) || !boundaries.contains_key(&r.end)
    }) {
        return Err("Select a valid text range. No text was changed.");
    }
    let mut position = 0;
    let lines: Vec<_> = source
        .split_inclusive('\n')
        .map(|line| {
            let begin = position;
            position += line.encode_utf16().count();
            begin..position
        })
        .collect();
    let output = crate::output::screenplay(source);
    let mut patches = Vec::new();
    for (index, line) in output
        .body
        .iter()
        .filter(|line| {
            line.kind == Kind::Scene
                && scope.as_ref().is_none_or(|scope| {
                    let heading = &lines[line.source_line];
                    heading.start < scope.end && heading.end > scope.start
                })
        })
        .enumerate()
    {
        let expected = if remove {
            String::new()
        } else {
            start
                .checked_add(u32::try_from(index).map_err(|_| "Too many scenes to number.")?)
                .ok_or("The scene numbering range is too large. No text was changed.")?
                .to_string()
        };
        if !remove && line.scene_number.as_deref() == Some(&expected) {
            continue;
        }
        if let Some(range) = &line.number_source {
            let raw = &source[boundaries[&range.start]..boundaries[&range.end]];
            if raw != format!("#{}#", line.scene_number.as_deref().unwrap_or("")) {
                return Err(
                    "A scene number contains formatting or hidden text. Edit that number in the manuscript before numbering again. No text was changed.",
                );
            }
            let mut beginning = range.start;
            if remove
                && beginning > 0
                && boundaries
                    .get(&(beginning - 1))
                    .is_some_and(|byte| source.as_bytes()[*byte] == b' ')
            {
                beginning -= 1;
            }
            patches.push(Patch {
                start: beginning,
                length: range.end - beginning,
                replacement: if remove {
                    String::new()
                } else {
                    format!("#{expected}#")
                },
            });
        } else if !remove && let Some(at) = line.number_insertion {
            patches.push(Patch {
                start: at,
                length: 0,
                replacement: format!(" #{expected}#"),
            });
        }
    }
    let Some(first) = patches.first() else {
        return Ok(None);
    };
    let start = first.start;
    let end = patches.last().unwrap().start + patches.last().unwrap().length;
    let byte_start = boundaries[&start];
    let mut replacement = source[byte_start..boundaries[&end]].to_owned();
    for patch in patches.iter().rev() {
        replacement.replace_range(
            boundaries[&patch.start] - byte_start
                ..boundaries[&(patch.start + patch.length)] - byte_start,
            &patch.replacement,
        );
    }
    Ok(Some(Change {
        start,
        length: end - start,
        replacement,
        patches,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn apply(source: &str, remove: bool) -> String {
        let Some(change) = edit(source, 1, remove).unwrap() else {
            return source.into();
        };
        let units: Vec<u16> = source.encode_utf16().collect();
        String::from_utf16(
            &[
                &units[..change.start],
                &change.replacement.encode_utf16().collect::<Vec<_>>(),
                &units[change.start + change.length..],
            ]
            .concat(),
        )
        .unwrap()
    }
    #[test]
    fn add_remove_preserve_source_notes_unicode_and_line_endings() {
        for nl in ["\n", "\r\n"] {
            let source = format!(
                "Title: Zoë{nl}{nl}**INT. CAFÉ - DAY** [[private]]{nl}{nl}Action 🌙.{nl}{nl}.The roof{nl}{nl}/* INT. OMITTED - DAY */"
            );
            let numbered = apply(&source, false);
            assert!(numbered.contains("**INT. CAFÉ - DAY #1#** [[private]]"));
            assert!(numbered.contains(".The roof #2#"));
            assert!(edit(&numbered, 1, false).unwrap().is_none());
            assert_eq!(apply(&numbered, true), source);
        }
    }
    #[test]
    fn renumber_custom_labels_and_note_suffix() {
        let source = "INT. ROOM - DAY #1A# [[keep]]\n\nAction.\n\nEXT. ROOF - NIGHT #Z-2#";
        assert_eq!(
            apply(source, false),
            "INT. ROOM - DAY #1# [[keep]]\n\nAction.\n\nEXT. ROOF - NIGHT #2#"
        );
        assert_eq!(
            apply(source, true),
            "INT. ROOM - DAY [[keep]]\n\nAction.\n\nEXT. ROOF - NIGHT"
        );
    }
    #[test]
    fn unsafe_number_markup_rejects_atomically() {
        assert!(
            edit(
                "INT. ROOM - DAY\n\nAction.\n\nEXT. ROOF - NIGHT #**Z**#",
                1,
                false
            )
            .is_err()
        );
        assert!(edit("INT. ROOM - DAY #1[[note]]#", 1, true).is_err());
        assert!(edit("INT. ROOM - DAY", 0, false).is_err());
        assert!(
            edit(
                "INT. ROOM - DAY\n\nAction.\n\nEXT. ROOF - NIGHT",
                u32::MAX,
                false
            )
            .is_err()
        );
        assert!(edit("Action.", 1, false).unwrap().is_none());
    }
    #[test]
    fn range_validation_and_outside_unsafe_numbers() {
        let source = "INT. FIRST - DAY #**A**#\n\n🌙 waits.\n\nEXT. LAST - DAY";
        let begin = source[..source.find("EXT.").unwrap()]
            .encode_utf16()
            .count();
        let end = source.encode_utf16().count();
        let change = edit_range(source, 23, false, Some(begin..end))
            .unwrap()
            .unwrap();
        assert_eq!(change.replacement, " #23#");
        assert!(edit_range(source, 1, false, Some(0..end)).is_err());
        assert!(edit_range(source, 1, false, Some(end..end)).is_err());
        assert!(edit_range(source, 1, false, Some(0..end + 1)).is_err());
        let emoji = source[..source.find('🌙').unwrap()].encode_utf16().count();
        assert!(edit_range(source, 1, false, Some(emoji + 1..end)).is_err());
        assert!(edit("INT. A - DAY #1#\n\nEXT. B - DAY #2#", u32::MAX, true).is_ok());
    }
}

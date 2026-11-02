//! Adapt shared UTF-16 scene-number patches to original UTF-8 editor coordinates.
use crate::navigation;
pub struct Plan {
    pub source: String,
    pub anchor: usize,
    pub focus: usize,
}
pub fn plan(
    source: &str,
    anchor: usize,
    focus: usize,
    first: u32,
    remove: bool,
    selected_only: bool,
) -> Result<Option<Plan>, String> {
    let to_units = |byte| {
        source
            .get(..byte)
            .map(|s| s.encode_utf16().count())
            .ok_or_else(|| "Invalid source selection. No text was changed.".to_owned())
    };
    let a = to_units(anchor)?;
    let f = to_units(focus)?;
    let scope = selected_only.then_some(a.min(f)..a.max(f));
    let Some(change) = fountage_core::numbering::edit_range(source, first, remove, scope)
        .map_err(str::to_owned)?
    else {
        return Ok(None);
    };
    let range = navigation::utf16_ranges(source, [(change.start, change.length)])
        .pop()
        .flatten()
        .ok_or("Invalid scene number range.")?;
    let mapped = |position: usize| -> usize {
        let mut value = position;
        for patch in &change.patches {
            if position < patch.start {
                break;
            }
            let size = patch.replacement.encode_utf16().count();
            if position < patch.start + patch.length {
                return value - (position - patch.start) + (position - patch.start).min(size);
            }
            value = value - patch.length + size;
        }
        value
    };
    let mut updated = source.to_owned();
    updated.replace_range(range, &change.replacement);
    let ranges = navigation::utf16_ranges(&updated, [(mapped(a), 0), (mapped(f), 0)]);
    let anchor = ranges[0]
        .as_ref()
        .ok_or("Invalid resulting selection.")?
        .start;
    let focus = ranges[1]
        .as_ref()
        .ok_or("Invalid resulting selection.")?
        .start;
    Ok(Some(Plan {
        source: updated,
        anchor,
        focus,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scoped_numbers_map_directed_selection_and_preserve_all_other_bytes() {
        let source = "Title: Zoë 🌙\r\n\r\nINT. FIRST - DAY #A#\r\n\r\nINT. CAFÉ - DAY [[keep]]\r\n\r\nAction 世界.\r\n\r\nEXT. ROOF - NIGHT #old#\r\n\r\n/*\r\nINT. HIDDEN - DAY\r\n*/\r\n\r\nINT. LAST - DAY #Z#\r\n";
        let start = source.find("CAFÉ").unwrap();
        let end = source.find("#old#").unwrap() + 5;
        let result = plan(source, end, start, 42, false, true).unwrap().unwrap();
        let expected = source
            .replace("DAY [[keep]]", "DAY #42# [[keep]]")
            .replace("#old#", "#43#");
        assert_eq!(result.source, expected);
        assert!(result.anchor > result.focus);
        assert_eq!(
            &result.source[result.focus..result.anchor],
            &expected[start..expected.find("#43#").unwrap() + 4]
        );
        assert!(
            plan(&result.source, result.anchor, result.focus, 42, false, true)
                .unwrap()
                .is_none()
        );
        let removed = plan(&result.source, result.anchor, result.focus, 1, true, true)
            .unwrap()
            .unwrap();
        assert_eq!(removed.source, source.replace(" #old#", ""));
        assert!(plan(source, source.find('🌙').unwrap() + 1, 1, 1, false, false).is_err());
        assert!(plan(source, start, start, 1, false, true).is_err());
        assert!(
            plan(
                "INT. ROOM - DAY\n\nEXT. ROOF - DAY #**X**#",
                0,
                0,
                1,
                false,
                false
            )
            .is_err()
        );
        assert!(plan("INT. A - DAY\n\nEXT. B - DAY", 0, 0, u32::MAX, false, false).is_err());
        assert!(
            plan("Action only.", 0, 0, 1, false, false)
                .unwrap()
                .is_none()
        );
    }
}

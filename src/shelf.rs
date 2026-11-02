//! Swift-compatible independent Shelf records. Unknown records and fields remain opaque.
use crate::{project::NAMESPACE, writing::Format};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;

const UNSUPPORTED: &str = "This Shelf uses an unsupported format. Its data was left unchanged.";
const CHANGED: &str = "The Shelf changed. Reopen it before editing.";
const INVALID: &str = "Enter a title and remove null characters before saving.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Note,
    Snippet,
    Checklist,
}
impl Kind {
    fn visibility_key(self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::Snippet => "snippet",
            Self::Checklist => "checklist",
        }
    }
}
/// Missing visibility settings mean shown. Unknown settings remain opaque.
pub fn visible(metadata: &Value, kind: Kind) -> Result<bool, &'static str> {
    Ok(record(metadata)?
        .and_then(|s| s.get("visibility"))
        .and_then(|v| v.get(kind.visibility_key()))
        .and_then(Value::as_bool)
        .unwrap_or(true))
}
/// Quick entry is a project draft, separate from saved Shelf notes and source.
pub fn quick_text(metadata: &Value) -> Result<&str, &'static str> {
    Ok(record(metadata)?
        .and_then(|s| s.get("quickEntry"))
        .and_then(|q| q.get("text"))
        .and_then(Value::as_str)
        .unwrap_or(""))
}
pub fn quick_visible(metadata: &Value) -> Result<bool, &'static str> {
    Ok(record(metadata)?
        .and_then(|s| s.get("quickEntry"))
        .and_then(|q| q.get("visible"))
        .and_then(Value::as_bool)
        .unwrap_or(true))
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub kind: Kind,
    pub title: String,
    pub content: String,
    pub pinned: bool,
}
impl Item {
    pub fn new(kind: Kind, title: String, content: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            title,
            content,
            pinned: false,
        }
    }
    fn validate(&self) -> Result<(), &'static str> {
        if self.id.is_empty()
            || self.id.contains('\0')
            || self.title.trim().is_empty()
            || self.title.contains('\0')
            || self.content.contains('\0')
        {
            Err(INVALID)
        } else {
            Ok(())
        }
    }
}
/// Plan a Shelf insertion against the exact source/selection captured by the UI.
/// Coordinates are UTF-8 bytes. Reversed selections are preserved by callers for
/// Undo; the inserted text replaces their complete range without normalizing it.
pub fn selection_range(
    source: &str,
    expected: &str,
    anchor: usize,
    focus: usize,
) -> Result<std::ops::Range<usize>, &'static str> {
    let boundary = |at: usize| {
        source.is_char_boundary(at)
            && !(at > 0
                && at < source.len()
                && source.as_bytes()[at - 1] == b'\r'
                && source.as_bytes()[at] == b'\n')
    };
    if source != expected || source.contains('\0') || !boundary(anchor) || !boundary(focus) {
        return Err(
            "The manuscript or selection changed. Select the text again before inserting from the Shelf.",
        );
    }
    Ok(anchor.min(focus)..anchor.max(focus))
}
pub fn insertion(
    source: &str,
    expected: &str,
    anchor: usize,
    focus: usize,
    content: &str,
) -> Result<crate::formatting::Edit, &'static str> {
    let range = selection_range(source, expected, anchor, focus)?;
    if content.contains('\0') {
        return Err("Remove null characters from Shelf text before inserting.");
    }
    let caret = range
        .start
        .checked_add(content.len())
        .ok_or("The inserted text is too large.")?;
    let mut updated = source.to_owned();
    updated.replace_range(range, content);
    Ok(crate::formatting::Edit {
        source: updated,
        anchor: caret,
        focus: caret,
    })
}

#[derive(Debug, Clone)]
pub enum Operation {
    SetQuickText {
        expected: String,
        text: String,
    },
    SetQuickVisible(bool),
    CaptureQuickNote {
        expected: String,
        id: String,
    },
    /// None changes every implemented type, retaining future visibility keys.
    SetVisibility {
        kind: Option<Kind>,
        visible: bool,
    },
    Add(Item),
    Update {
        expected: Item,
        item: Item,
    },
    Remove(Item),
    Move {
        expected: Item,
        order: Vec<String>,
        down: bool,
    },
    MoveWithinKind {
        expected: Item,
        order: Vec<String>,
        down: bool,
    },
    ToggleTask {
        expected: Item,
        start: usize,
    },
    MoveTask {
        expected: Item,
        start: usize,
        down: bool,
    },
    RemoveTask {
        expected: Item,
        start: usize,
    },
    UpdateTask {
        expected: Item,
        start: usize,
        text: String,
    },
    AppendTask {
        expected: Item,
        text: String,
    },
}
fn record(metadata: &Value) -> Result<Option<&Map<String, Value>>, &'static str> {
    let root = metadata.as_object().ok_or(UNSUPPORTED)?;
    let Some(own) = root.get(NAMESPACE) else {
        return Ok(None);
    };
    let own = own.as_object().ok_or(UNSUPPORTED)?;
    let Some(shelf) = own.get("shelf") else {
        return Ok(None);
    };
    let shelf = shelf.as_object().ok_or(UNSUPPORTED)?;
    if shelf.get("version").and_then(Value::as_u64) != Some(1)
        || shelf.get("items").and_then(Value::as_object).is_none()
    {
        return Err(UNSUPPORTED);
    }
    let order = shelf
        .get("order")
        .and_then(Value::as_array)
        .ok_or(UNSUPPORTED)?;
    let mut seen = BTreeSet::new();
    if !order
        .iter()
        .all(|v| v.as_str().is_some_and(|id| seen.insert(id)))
    {
        return Err(UNSUPPORTED);
    }
    if let Some(pinned) = shelf.get("pinnedOrder") {
        let pinned = pinned.as_array().ok_or(UNSUPPORTED)?;
        let mut seen = BTreeSet::new();
        if !pinned
            .iter()
            .all(|v| v.as_str().is_some_and(|id| seen.insert(id)))
        {
            return Err(UNSUPPORTED);
        }
    }
    if let Some(visibility) = shelf.get("visibility") {
        let visibility = visibility.as_object().ok_or(UNSUPPORTED)?;
        for kind in [Kind::Note, Kind::Snippet, Kind::Checklist] {
            if visibility
                .get(kind.visibility_key())
                .is_some_and(|v| !v.is_boolean())
            {
                return Err(UNSUPPORTED);
            }
        }
    }
    if let Some(quick) = shelf.get("quickEntry") {
        let quick = quick.as_object().ok_or(UNSUPPORTED)?;
        if quick
            .get("text")
            .is_some_and(|v| v.as_str().is_none_or(|s| s.contains('\0')))
            || quick.get("visible").is_some_and(|v| !v.is_boolean())
        {
            return Err(UNSUPPORTED);
        }
    }
    Ok(Some(shelf))
}
/// Known records in saved order, pinned first. Unknown/malformed items are retained
/// in metadata even though the supported view does not offer them for editing.
pub fn items(metadata: &Value) -> Result<Vec<Item>, &'static str> {
    let Some(shelf) = record(metadata)? else {
        return Ok(Vec::new());
    };
    let records = shelf["items"].as_object().unwrap();
    let mut ids: Vec<String> = shelf["order"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    let known: BTreeSet<_> = ids.iter().cloned().collect();
    let mut extra: Vec<_> = records
        .keys()
        .filter(|id| !known.contains(*id))
        .cloned()
        .collect();
    extra.sort();
    ids.extend(extra);
    let mut decoded: Vec<Item> = ids
        .iter()
        .filter_map(|id| {
            let mut value = records.get(id)?.as_object()?.clone();
            value.insert("id".into(), Value::String(id.clone()));
            serde_json::from_value(Value::Object(value)).ok()
        })
        .collect();
    let pinned: Vec<_> = shelf
        .get("pinnedOrder")
        .and_then(Value::as_array)
        .map(|ids| ids.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    decoded.sort_by_key(|item| {
        (
            !item.pinned,
            if item.pinned {
                pinned
                    .iter()
                    .position(|id| *id == item.id)
                    .unwrap_or(usize::MAX)
            } else {
                usize::MAX
            },
        )
    }); // Stable sort preserves base order for unranked items.
    Ok(decoded)
}
fn current<'a>(items: &'a [Item], expected: &Item) -> Result<&'a Item, &'static str> {
    items
        .iter()
        .find(|i| i.id == expected.id && *i == expected)
        .ok_or(CHANGED)
}
fn write_item(shelf: &mut Map<String, Value>, item: &Item, new: bool) -> Result<(), &'static str> {
    item.validate()?;
    let records = shelf.get_mut("items").unwrap().as_object_mut().unwrap();
    let mut value = records
        .get(&item.id)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    value.insert("kind".into(), serde_json::to_value(item.kind).unwrap());
    value.insert("title".into(), Value::String(item.title.clone()));
    value.insert("content".into(), Value::String(item.content.clone()));
    value.insert("pinned".into(), Value::Bool(item.pinned));
    records.insert(item.id.clone(), Value::Object(value));
    if new {
        shelf
            .get_mut("order")
            .unwrap()
            .as_array_mut()
            .unwrap()
            .push(Value::String(item.id.clone()));
    }
    Ok(())
}
fn is_line_break(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r' | '\u{000b}' | '\u{000c}' | '\u{0085}' | '\u{2028}' | '\u{2029}'
    )
}
fn append_task(content: &str, text: &str) -> Result<String, &'static str> {
    let label = text.trim_matches(|c: char| c.is_whitespace() && !is_line_break(c));
    if label.is_empty() || label.chars().any(|c| c == '\0' || is_line_break(c)) {
        return Err("Enter a task on one line.");
    }
    let ending = if content
        .find('\n')
        .is_some_and(|p| p > 0 && content.as_bytes()[p - 1] == b'\r')
    {
        "\r\n"
    } else {
        "\n"
    };
    let separator = if content.is_empty() || content.ends_with('\n') {
        ""
    } else {
        ending
    };
    let start = content.encode_utf16().count() + separator.encode_utf16().count();
    let candidate = format!("{content}{separator}[ ] {label}{ending}");
    if !crate::writing::analyze(&candidate, Format::General)
        .todos
        .iter()
        .any(|t| t.start == start && t.text == label)
    {
        return Err("The new task would be hidden by markup. No Shelf data was changed.");
    }
    Ok(candidate)
}
/// Return a complete metadata candidate without mutating the input. Only known
/// fields of the requested Shelf item change; unknown data is retained in place.
pub fn apply(metadata: &Value, operation: &Operation) -> Result<Option<Value>, &'static str> {
    let before = items(metadata)?;
    let mut shelf = record(metadata)?.cloned().unwrap_or_else(|| {
        json!({"version":1,"items":{},"order":[]})
            .as_object()
            .unwrap()
            .clone()
    });
    match operation {
        Operation::SetQuickText { expected, text } => {
            if quick_text(metadata)? != expected {
                return Err(CHANGED);
            }
            if text.contains('\0') {
                return Err(INVALID);
            }
            if text == expected {
                return Ok(None);
            }
            shelf
                .entry("quickEntry")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or(UNSUPPORTED)?
                .insert("text".into(), text.clone().into());
        }
        Operation::SetQuickVisible(shown) => {
            if quick_visible(metadata)? == *shown {
                return Ok(None);
            }
            shelf
                .entry("quickEntry")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or(UNSUPPORTED)?
                .insert("visible".into(), (*shown).into());
        }
        Operation::CaptureQuickNote { expected, id } => {
            if quick_text(metadata)? != expected {
                return Err(CHANGED);
            }
            if expected.trim().is_empty() {
                return Ok(None);
            }
            let item = Item {
                id: id.clone(),
                kind: Kind::Note,
                title: "Note".into(),
                content: expected.clone(),
                pinned: false,
            };
            let mut updated = apply(metadata, &Operation::Add(item))?.ok_or(CHANGED)?;
            updated[NAMESPACE]["shelf"]["quickEntry"]["text"] = "".into();
            let order = updated[NAMESPACE]["shelf"]["order"]
                .as_array_mut()
                .ok_or(UNSUPPORTED)?;
            order.retain(|value| value.as_str() != Some(id));
            order.insert(0, id.clone().into());
            return Ok(Some(updated));
        }
        Operation::SetVisibility {
            kind,
            visible: show,
        } => {
            let kinds: Vec<_> = [Kind::Note, Kind::Snippet, Kind::Checklist]
                .into_iter()
                .filter(|k| kind.is_none_or(|wanted| wanted == *k))
                .collect();
            if kinds.iter().all(|k| visible(metadata, *k) == Ok(*show)) {
                return Ok(None);
            }
            let settings = shelf
                .entry("visibility")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or(UNSUPPORTED)?;
            for kind in kinds {
                settings.insert(kind.visibility_key().into(), (*show).into());
            }
        }
        Operation::Add(item) => {
            item.validate()?;
            if shelf["items"].as_object().unwrap().contains_key(&item.id)
                || shelf["order"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v.as_str() == Some(&item.id))
            {
                return Err(CHANGED);
            }
            if shelf
                .get("pinnedOrder")
                .and_then(Value::as_array)
                .is_some_and(|ids| ids.iter().any(|v| v.as_str() == Some(&item.id)))
            {
                return Err(CHANGED);
            }
            write_item(&mut shelf, item, true)?;
        }
        Operation::Update { expected, item } => {
            current(&before, expected)?;
            if item.id != expected.id {
                return Err(CHANGED);
            }
            item.validate()?;
            if expected == item {
                return Ok(None);
            }
            write_item(&mut shelf, item, false)?;
        }
        Operation::Remove(expected) => {
            current(&before, expected)?;
            shelf
                .get_mut("items")
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(&expected.id);
            shelf
                .get_mut("order")
                .unwrap()
                .as_array_mut()
                .unwrap()
                .retain(|id| id.as_str() != Some(&expected.id));
            if let Some(order) = shelf.get_mut("pinnedOrder").and_then(Value::as_array_mut) {
                order.retain(|id| id.as_str() != Some(&expected.id));
            }
        }
        Operation::Move {
            expected,
            order,
            down,
        }
        | Operation::MoveWithinKind {
            expected,
            order,
            down,
        } => {
            current(&before, expected)?;
            if before.iter().map(|i| &i.id).ne(order.iter()) {
                return Err(CHANGED);
            }
            let group: Vec<_> = before
                .iter()
                .filter(|i| {
                    i.pinned == expected.pinned
                        && (!matches!(operation, Operation::MoveWithinKind { .. })
                            || i.kind == expected.kind)
                })
                .collect();
            let at = group.iter().position(|i| i.id == expected.id).unwrap();
            let next = if *down {
                at.checked_add(1).filter(|n| *n < group.len())
            } else {
                at.checked_sub(1)
            };
            let Some(next) = next else { return Ok(None) };
            let order_key = if expected.pinned {
                "pinnedOrder"
            } else {
                "order"
            };
            let mut ids: Vec<String> = shelf
                .get(order_key)
                .and_then(Value::as_array)
                .map(|v| {
                    v.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_else(|| {
                    before
                        .iter()
                        .filter(|i| i.pinned)
                        .map(|i| i.id.clone())
                        .collect()
                });
            let mut extra: Vec<_> = if expected.pinned {
                before
                    .iter()
                    .filter(|i| i.pinned && !ids.contains(&i.id))
                    .map(|i| i.id.clone())
                    .collect()
            } else {
                shelf["items"]
                    .as_object()
                    .unwrap()
                    .keys()
                    .filter(|id| !ids.contains(id))
                    .cloned()
                    .collect()
            };
            if !expected.pinned {
                extra.sort();
            }
            ids.extend(extra);
            let first = ids.iter().position(|id| *id == expected.id).unwrap();
            let second = ids.iter().position(|id| *id == group[next].id).unwrap();
            ids.swap(first, second);
            shelf.insert(order_key.into(), serde_json::to_value(ids).unwrap());
        }
        Operation::ToggleTask { expected, start } => {
            current(&before, expected)?;
            if expected.kind != Kind::Checklist {
                return Err(CHANGED);
            }
            let mut item = expected.clone();
            item.content = crate::todo::toggle(&item.content, Format::General, *start)?.source;
            write_item(&mut shelf, &item, false)?;
        }
        Operation::MoveTask {
            expected,
            start,
            down,
        } => {
            current(&before, expected)?;
            if expected.kind != Kind::Checklist {
                return Err(CHANGED);
            }
            let Some(edit) =
                crate::todo::move_task(&expected.content, Format::General, *start, *down)?
            else {
                return Ok(None);
            };
            let mut item = expected.clone();
            item.content = edit.source;
            write_item(&mut shelf, &item, false)?;
        }
        Operation::RemoveTask { expected, start } => {
            current(&before, expected)?;
            if expected.kind != Kind::Checklist {
                return Err(CHANGED);
            }
            let mut item = expected.clone();
            item.content = crate::todo::remove(&item.content, Format::General, *start)?.source;
            write_item(&mut shelf, &item, false)?;
        }
        Operation::UpdateTask {
            expected,
            start,
            text,
        } => {
            current(&before, expected)?;
            if expected.kind != Kind::Checklist {
                return Err(CHANGED);
            }
            let Some(edit) = crate::todo::update(&expected.content, Format::General, *start, text)?
            else {
                return Ok(None);
            };
            let mut item = expected.clone();
            item.content = edit.source;
            write_item(&mut shelf, &item, false)?;
        }
        Operation::AppendTask { expected, text } => {
            current(&before, expected)?;
            if expected.kind != Kind::Checklist {
                return Err(CHANGED);
            }
            let mut item = expected.clone();
            item.content = append_task(&item.content, text)?;
            write_item(&mut shelf, &item, false)?;
        }
    }
    let mut updated = metadata.clone();
    let root = updated.as_object_mut().ok_or(UNSUPPORTED)?;
    let own = root
        .entry(NAMESPACE)
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or(UNSUPPORTED)?;
    own.insert("shelf".into(), Value::Object(shelf));
    Ok(Some(updated))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{Entry, Project};
    use std::collections::BTreeMap;
    #[test]
    fn quick_draft_and_capture_preserve_exact_text_and_unknown_fields_atomically() {
        let mut original = fixture();
        original[NAMESPACE]["shelf"]["quickEntry"] = json!({"future":{"keep":true}});
        assert_eq!(quick_text(&original), Ok(""));
        assert_eq!(quick_visible(&original), Ok(true));
        let text = "  Quick Zoë 🌙\r\nSecond 世界\r\n";
        let drafted = apply(
            &original,
            &Operation::SetQuickText {
                expected: "".into(),
                text: text.into(),
            },
        )
        .unwrap()
        .unwrap();
        let mut expected = original.clone();
        expected[NAMESPACE]["shelf"]["quickEntry"]["text"] = text.into();
        assert_eq!(drafted, expected);
        assert_eq!(
            apply(
                &drafted,
                &Operation::SetQuickText {
                    expected: text.into(),
                    text: text.into()
                }
            )
            .unwrap(),
            None
        );
        assert!(
            apply(
                &drafted,
                &Operation::SetQuickText {
                    expected: "stale".into(),
                    text: "lost".into()
                }
            )
            .is_err()
        );
        assert!(
            apply(
                &drafted,
                &Operation::SetQuickText {
                    expected: text.into(),
                    text: "bad\0".into()
                }
            )
            .is_err()
        );
        let hidden = apply(&drafted, &Operation::SetQuickVisible(false))
            .unwrap()
            .unwrap();
        assert_eq!(quick_text(&hidden), Ok(text));
        assert_eq!(
            apply(&hidden, &Operation::SetQuickVisible(false)).unwrap(),
            None
        );
        assert!(
            apply(
                &hidden,
                &Operation::CaptureQuickNote {
                    expected: text.into(),
                    id: "a".into()
                }
            )
            .is_err()
        );
        assert!(
            apply(
                &hidden,
                &Operation::CaptureQuickNote {
                    expected: "stale".into(),
                    id: "fresh".into()
                }
            )
            .is_err()
        );
        let captured = apply(
            &hidden,
            &Operation::CaptureQuickNote {
                expected: text.into(),
                id: "fresh".into(),
            },
        )
        .unwrap()
        .unwrap();
        let mut expected = hidden.clone();
        expected[NAMESPACE]["shelf"]["quickEntry"]["text"] = "".into();
        expected[NAMESPACE]["shelf"]["items"]["fresh"] =
            json!({"kind":"note","title":"Note","content":text,"pinned":false});
        expected[NAMESPACE]["shelf"]["order"]
            .as_array_mut()
            .unwrap()
            .insert(0, "fresh".into());
        assert_eq!(captured, expected);
        assert_eq!(quick_visible(&captured), Ok(false));
        assert!(
            apply(
                &captured,
                &Operation::CaptureQuickNote {
                    expected: text.into(),
                    id: "second".into()
                }
            )
            .is_err()
        );
        assert_eq!(
            apply(
                &captured,
                &Operation::CaptureQuickNote {
                    expected: "".into(),
                    id: "second".into()
                }
            )
            .unwrap(),
            None
        );
    }
    #[test]
    fn quick_entry_malformed_known_values_are_read_only() {
        for value in [
            json!(false),
            json!([]),
            json!({"text":1}),
            json!({"text":null}),
            json!({"text":"nul\0"}),
            json!({"visible":0}),
        ] {
            let mut metadata = fixture();
            metadata[NAMESPACE]["shelf"]["quickEntry"] = value;
            assert!(quick_text(&metadata).is_err());
            assert!(quick_visible(&metadata).is_err());
            assert!(apply(&metadata, &Operation::SetQuickVisible(true)).is_err());
        }
    }
    #[test]
    fn visibility_defaults_noops_and_known_changes_preserve_items_and_future_settings() {
        let mut original = fixture();
        original[NAMESPACE]["shelf"]["visibility"] = json!({"future-tool":{"keep":[true,null]}});
        for kind in [Kind::Note, Kind::Snippet, Kind::Checklist] {
            assert_eq!(visible(&original, kind), Ok(true));
        }
        assert_eq!(
            apply(
                &original,
                &Operation::SetVisibility {
                    kind: None,
                    visible: true
                }
            )
            .unwrap(),
            None
        );
        let hidden = apply(
            &original,
            &Operation::SetVisibility {
                kind: Some(Kind::Snippet),
                visible: false,
            },
        )
        .unwrap()
        .unwrap();
        let mut expected = original.clone();
        expected[NAMESPACE]["shelf"]["visibility"]["snippet"] = false.into();
        assert_eq!(hidden, expected);
        let all = apply(
            &hidden,
            &Operation::SetVisibility {
                kind: None,
                visible: false,
            },
        )
        .unwrap()
        .unwrap();
        for kind in [Kind::Note, Kind::Snippet, Kind::Checklist] {
            assert_eq!(visible(&all, kind), Ok(false));
            expected[NAMESPACE]["shelf"]["visibility"][kind.visibility_key()] = false.into();
        }
        assert_eq!(all, expected);
        assert_eq!(items(&all), items(&original));
        assert_eq!(
            apply(
                &all,
                &Operation::SetVisibility {
                    kind: None,
                    visible: false
                }
            )
            .unwrap(),
            None
        );
        let shown = apply(
            &all,
            &Operation::SetVisibility {
                kind: None,
                visible: true,
            },
        )
        .unwrap()
        .unwrap();
        for kind in [Kind::Note, Kind::Snippet, Kind::Checklist] {
            expected[NAMESPACE]["shelf"]["visibility"][kind.visibility_key()] = true.into();
        }
        assert_eq!(shown, expected);
        let empty = json!({"foreign":7});
        assert_eq!(
            apply(
                &empty,
                &Operation::SetVisibility {
                    kind: None,
                    visible: true
                }
            )
            .unwrap(),
            None
        );
        assert_eq!(visible(&empty, Kind::Note), Ok(true));
    }
    #[test]
    fn malformed_known_visibility_is_read_only_and_unknown_keys_remain_opaque() {
        for value in [
            json!(false),
            json!([]),
            json!({"note":null}),
            json!({"snippet":"false"}),
            json!({"checklist":1}),
        ] {
            let mut metadata = fixture();
            metadata[NAMESPACE]["shelf"]["visibility"] = value;
            let before = metadata.clone();
            assert!(visible(&metadata, Kind::Note).is_err());
            assert!(
                apply(
                    &metadata,
                    &Operation::SetVisibility {
                        kind: None,
                        visible: true
                    }
                )
                .is_err()
            );
            assert_eq!(metadata, before);
        }
    }
    fn fixture() -> Value {
        json!({"foreign":{"keep":[true,null,18446744073709551615u64]},NAMESPACE:{"other":{"retain":true},"shelf":{"version":1,"future":{"keep":[1,2]},"order":["dangling","a","future","b"],"items":{
            "a":{"kind":"checklist","title":"Revision plan","content":"[ ] Review Zoë 🌙 [[keep]]\r\n[ ] Send draft\r\n","pinned":false,"future":{"color":"violet"},"id":"ignored-but-retained"},
            "b":{"kind":"note","title":"Research","content":"Keep raw notes","pinned":true,"future":7},
            "c":{"kind":"snippet","title":"Cut scene","content":"INT. ROOM - DAY\r\n","pinned":false},
            "future":{"kind":"future-tool","binary":"opaque","nested":{"keep":true}},
            "malformed":{"kind":"note","content":"Missing title, retained"}
        }}}})
    }
    #[test]
    fn known_view_and_shelf_transactions_preserve_opaque_data_and_identity() {
        let metadata = fixture();
        let original = metadata.clone();
        let before = items(&metadata).unwrap();
        assert_eq!(
            before.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
            ["b", "a", "c"]
        );
        assert_eq!(metadata, original);
        let checklist = before[1].clone();
        let toggled = apply(
            &metadata,
            &Operation::ToggleTask {
                expected: checklist.clone(),
                start: 0,
            },
        )
        .unwrap()
        .unwrap();
        let mut expected = metadata.clone();
        expected[NAMESPACE]["shelf"]["items"]["a"]["content"] =
            json!(checklist.content.replacen("[ ]", "[x]", 1));
        assert_eq!(toggled, expected);
        let changed = items(&toggled).unwrap().remove(1);
        assert!(
            apply(
                &toggled,
                &Operation::ToggleTask {
                    expected: checklist.clone(),
                    start: 0
                }
            )
            .is_err()
        );
        let appended = apply(
            &toggled,
            &Operation::AppendTask {
                expected: changed.clone(),
                text: "  Contact artist 世界  ".into(),
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            items(&appended).unwrap()[1].content,
            format!("{}[ ] Contact artist 世界\r\n", changed.content)
        );
        for text in [
            "",
            "\nBad",
            "bad\0task",
            "bad\rline",
            "bad\u{0085}line",
            "bad\u{2028}line",
            "[[private]]",
            "/* hidden",
        ] {
            assert!(
                apply(
                    &toggled,
                    &Operation::AppendTask {
                        expected: changed.clone(),
                        text: text.into()
                    }
                )
                .is_err()
            );
        }
        let moved = apply(
            &metadata,
            &Operation::Move {
                expected: checklist.clone(),
                order: before.iter().map(|i| i.id.clone()).collect(),
                down: true,
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            items(&moved)
                .unwrap()
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            ["b", "c", "a"]
        );
        assert_eq!(
            moved[NAMESPACE]["shelf"]["items"],
            metadata[NAMESPACE]["shelf"]["items"]
        );
        assert!(
            moved[NAMESPACE]["shelf"]["order"]
                .as_array()
                .unwrap()
                .contains(&json!("dangling"))
        );
        assert!(
            apply(
                &metadata,
                &Operation::Move {
                    expected: before[0].clone(),
                    order: before.iter().map(|i| i.id.clone()).collect(),
                    down: true
                }
            )
            .unwrap()
            .is_none()
        );
        let mut updated = checklist.clone();
        updated.title = "New title".into();
        updated.pinned = true;
        let updated_meta = apply(
            &metadata,
            &Operation::Update {
                expected: checklist.clone(),
                item: updated,
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            updated_meta[NAMESPACE]["shelf"]["items"]["a"]["future"],
            metadata[NAMESPACE]["shelf"]["items"]["a"]["future"]
        );
        assert!(
            apply(
                &metadata,
                &Operation::Update {
                    expected: checklist.clone(),
                    item: checklist.clone()
                }
            )
            .unwrap()
            .is_none()
        );
        let removed = apply(&metadata, &Operation::Remove(checklist.clone()))
            .unwrap()
            .unwrap();
        let mut expected = metadata.clone();
        expected[NAMESPACE]["shelf"]["items"]
            .as_object_mut()
            .unwrap()
            .remove("a");
        expected[NAMESPACE]["shelf"]["order"]
            .as_array_mut()
            .unwrap()
            .retain(|id| id != &json!("a"));
        assert_eq!(removed, expected);
        let mut reserved = Item::new(Kind::Checklist, "Plan".into(), "".into());
        reserved.id = "dangling".into();
        assert!(apply(&metadata, &Operation::Add(reserved)).is_err());
        assert_eq!(metadata, original);
    }
    #[test]
    fn unknown_schemas_and_invalid_changes_fail_atomically() {
        let metadata = fixture();
        let item = items(&metadata).unwrap()[1].clone();
        for value in [
            json!({"version":2,"items":{},"order":[]}),
            json!({"version":true,"items":{},"order":[]}),
            json!({"version":1,"items":[],"order":[]}),
            json!({"version":1,"items":{},"order":["x","x"]}),
            json!({"version":1,"items":{},"order":[3]}),
        ] {
            let mut candidate = metadata.clone();
            candidate[NAMESPACE]["shelf"] = value;
            let before = candidate.clone();
            assert!(items(&candidate).is_err());
            assert!(apply(&candidate, &Operation::Add(item.clone())).is_err());
            assert_eq!(candidate, before);
        }
        for (title, content) in [(" ", ""), ("bad\0title", ""), ("Plan", "bad\0content")] {
            assert!(
                apply(
                    &metadata,
                    &Operation::Add(Item::new(Kind::Checklist, title.into(), content.into()))
                )
                .is_err()
            );
        }
        let mut pending = item.clone();
        pending.content = "/* unfinished".into();
        let pending_meta = apply(
            &metadata,
            &Operation::Update {
                expected: item,
                item: pending.clone(),
            },
        )
        .unwrap()
        .unwrap();
        assert!(
            apply(
                &pending_meta,
                &Operation::AppendTask {
                    expected: pending,
                    text: "Visible task".into()
                }
            )
            .is_err()
        );
    }
    #[test]
    fn every_project_category_roundtrips_independent_checklists_without_source_changes() {
        let directory = tempfile::tempdir().unwrap();
        for category in 0..8 {
            let mut project = Project::new(Format::from_code(category).unwrap()).unwrap();
            project
                .replace_source("Title: Unchanged manuscript\r\n\r\nZoë 🌙 waits.\r\n")
                .unwrap();
            let mut entries = project.entries().clone();
            let mut metadata = project.metadata().clone();
            metadata["foreign"] = fixture()["foreign"].clone();
            metadata[NAMESPACE]["shelf"] = fixture()[NAMESPACE]["shelf"].clone();
            entries.insert(
                "info.json".into(),
                Entry::File(serde_json::to_vec_pretty(&metadata).unwrap()),
            );
            entries.insert(
                "assets".into(),
                Entry::Directory(BTreeMap::from([(
                    "private.bin".into(),
                    Entry::File(vec![0, 255, 1]),
                )])),
            );
            // Windows currently rejects link-bearing packages atomically. Its
            // rejection is covered in project_copy; ordinary package lifecycle
            // coverage must still run for every writing category there.
            #[cfg(unix)]
            entries.insert(
                "external-link".into(),
                Entry::SymbolicLink("../unfollowed".into()),
            );
            project = Project::from_entries(entries).unwrap();
            let baseline = project.clone();
            let original = directory
                .path()
                .join(format!("original-{category}.fountage"));
            project.save_copy(&original).unwrap();
            let item = items(project.metadata()).unwrap()[1].clone();
            assert!(
                project
                    .edit_shelf(
                        &metadata,
                        &Operation::ToggleTask {
                            expected: item,
                            start: 0
                        }
                    )
                    .unwrap()
            );
            assert_eq!(project.source(), baseline.source());
            assert_eq!(project.project_id(), baseline.project_id());
            assert_eq!(project.preset(), baseline.preset());
            for (name, entry) in baseline.entries() {
                if name != "info.json" {
                    assert_eq!(project.entries().get(name), Some(entry));
                }
            }
            let current = project.clone();
            assert!(
                project
                    .edit_shelf(
                        &metadata,
                        &Operation::Remove(items(current.metadata()).unwrap()[1].clone())
                    )
                    .is_err()
            );
            assert_eq!(project.entries(), current.entries());
            let same = items(project.metadata()).unwrap()[1].clone();
            let meta = project.metadata().clone();
            assert!(
                !project
                    .edit_shelf(
                        &meta,
                        &Operation::Update {
                            expected: same.clone(),
                            item: same
                        }
                    )
                    .unwrap()
            );
            assert_eq!(
                project.entries(),
                current.entries(),
                "no-op must retain original JSON bytes"
            );
            let copy = directory.path().join(format!("copy-{category}.fountage"));
            project.save_copy(&copy).unwrap();
            let reopened = Project::read(&copy).unwrap();
            assert_eq!(reopened.entries(), project.entries());
            assert_eq!(
                Project::read(&original).unwrap().entries(),
                baseline.entries()
            );
            if let Some(output) = std::env::var_os("FOUNTAGE_SHELF_FIXTURES") {
                std::fs::create_dir_all(&output).unwrap();
                project
                    .save_copy(
                        &std::path::PathBuf::from(output)
                            .join(format!("Rust Shelf {category}.fountage")),
                    )
                    .unwrap();
            }
        }
    }
}

#[cfg(test)]
mod interoperability_tests {
    use super::*;
    #[test]
    fn new_checklist_lifecycle_never_changes_manuscript_fields() {
        let base = json!({"foreign":{"keep":true}});
        let item = Item::new(Kind::Checklist, "Plan 世界".into(), "".into());
        assert!(uuid::Uuid::parse_str(&item.id).is_ok());
        let added = apply(&base, &Operation::Add(item)).unwrap().unwrap();
        let item = items(&added).unwrap().remove(0);
        let appended = apply(
            &added,
            &Operation::AppendTask {
                expected: item,
                text: "Review Zoë 🌙".into(),
            },
        )
        .unwrap()
        .unwrap();
        let item = items(&appended).unwrap().remove(0);
        assert_eq!(item.content, "[ ] Review Zoë 🌙\n");
        let completed = apply(
            &appended,
            &Operation::ToggleTask {
                expected: item,
                start: 0,
            },
        )
        .unwrap()
        .unwrap();
        let item = items(&completed).unwrap().remove(0);
        assert_eq!(item.content, "[x] Review Zoë 🌙\n");
        let removed = apply(&completed, &Operation::Remove(item))
            .unwrap()
            .unwrap();
        assert!(items(&removed).unwrap().is_empty());
        assert_eq!(removed["foreign"], base["foreign"]);
    }
    #[test]
    fn swift_written_shelf_projects_are_readable_without_loss() {
        let Some(output) = std::env::var_os("FOUNTAGE_SWIFT_SHELF_FIXTURES") else {
            return;
        };
        let rust = std::env::var_os("FOUNTAGE_SHELF_REFERENCE")
            .expect("Set Rust reference fixture directory");
        for category in 0..8 {
            let project = crate::project::Project::read(
                &std::path::PathBuf::from(&output).join(format!("Swift Shelf {category}.fountage")),
            )
            .unwrap();
            let reference = crate::project::Project::read(
                &std::path::PathBuf::from(&rust).join(format!("Rust Shelf {category}.fountage")),
            )
            .unwrap();
            assert_eq!(project.source(), reference.source());
            for (name, entry) in reference.entries() {
                if name != "info.json" {
                    assert_eq!(project.entries().get(name), Some(entry));
                }
            }
            let checklist = items(project.metadata())
                .unwrap()
                .into_iter()
                .find(|i| i.id == "a")
                .unwrap();
            assert_eq!(
                checklist.content,
                "[ ] Review Zoë 🌙 [[keep]]\r\n[ ] Send draft\r\n"
            );
            let mut expected = reference.metadata().clone();
            expected[NAMESPACE]["shelf"]["items"]["a"]["content"] = json!(checklist.content);
            expected[NAMESPACE]["modifiedAt"] = project.metadata()[NAMESPACE]["modifiedAt"].clone();
            assert_eq!(project.metadata(), &expected);
        }
    }
}

#[cfg(test)]
mod legacy_tests {
    use super::*;
    #[test]
    fn legacy_project_shelf_edits_keep_schema_and_unknown_fields_while_future_schemas_reject() {
        let base = crate::project::Project::new(Format::Screenplay).unwrap();
        let mut metadata = base.metadata().clone();
        metadata[NAMESPACE]["version"] = json!(1);
        let mut entries = base.entries().clone();
        entries.insert(
            "info.json".into(),
            crate::project::Entry::File(serde_json::to_vec_pretty(&metadata).unwrap()),
        );
        let mut project = crate::project::Project::from_entries(entries).unwrap();
        let item = Item::new(Kind::Checklist, "Legacy Plan".into(), "".into());
        project
            .edit_shelf(&metadata, &Operation::Add(item))
            .unwrap();
        assert_eq!(project.schema(), 1);
        assert_eq!(project.metadata()[NAMESPACE]["version"], json!(1));
        let current = items(project.metadata()).unwrap().remove(0);
        let expected = project.metadata().clone();
        project
            .edit_shelf(
                &expected,
                &Operation::AppendTask {
                    expected: current,
                    text: "\u{2003}Review Zoë\u{a0}".into(),
                },
            )
            .unwrap();
        assert_eq!(
            items(project.metadata()).unwrap()[0].content,
            "[ ] Review Zoë\n"
        );
        let mut metadata = project.metadata().clone();
        metadata[NAMESPACE]["shelf"]["version"] = json!(2);
        let mut entries = project.entries().clone();
        entries.insert(
            "info.json".into(),
            crate::project::Entry::File(serde_json::to_vec_pretty(&metadata).unwrap()),
        );
        let mut future = crate::project::Project::from_entries(entries.clone()).unwrap();
        let item = Item::new(Kind::Note, "Future".into(), "".into());
        assert!(future.edit_shelf(&metadata, &Operation::Add(item)).is_err());
        assert_eq!(future.entries(), &entries);
    }
}

#[cfg(test)]
mod pin_order_tests {
    use super::*;
    #[test]
    fn pinned_order_is_independent_of_base_and_kind_moves_preserve_hidden_slots() {
        let mut value = json!({NAMESPACE:{"shelf":{"version":1,"order":["a","future","note","b","snippet","c"],"items":{
            "a":{"kind":"checklist","title":"A","content":"[ ] A","pinned":false,"opaque":7},
            "b":{"kind":"checklist","title":"B","content":"[X] B","pinned":false},
            "c":{"kind":"checklist","title":"C","content":"[ ] C","pinned":false},
            "note":{"kind":"note","title":"Note","content":"Keep","pinned":false},
            "snippet":{"kind":"snippet","title":"Snippet","content":"Keep","pinned":false},
            "future":{"kind":"future","keep":true}}}}});
        let original = value.clone();
        let action = |v: &Value, id: &str, down: bool| Operation::MoveWithinKind {
            expected: items(v).unwrap().into_iter().find(|i| i.id == id).unwrap(),
            order: items(v).unwrap().into_iter().map(|i| i.id).collect(),
            down,
        };
        value = apply(&value, &action(&value, "a", true)).unwrap().unwrap();
        assert_eq!(
            value[NAMESPACE]["shelf"]["order"],
            json!(["b", "future", "note", "a", "snippet", "c"])
        );
        assert_eq!(
            value[NAMESPACE]["shelf"]["items"],
            original[NAMESPACE]["shelf"]["items"]
        );
        for id in ["a", "b"] {
            let expected = items(&value)
                .unwrap()
                .into_iter()
                .find(|i| i.id == id)
                .unwrap();
            let mut item = expected.clone();
            item.pinned = true;
            value = apply(&value, &Operation::Update { expected, item })
                .unwrap()
                .unwrap();
        }
        let base = value[NAMESPACE]["shelf"]["order"].clone();
        value = apply(&value, &action(&value, "a", false)).unwrap().unwrap();
        assert_eq!(value[NAMESPACE]["shelf"]["order"], base);
        assert_eq!(value[NAMESPACE]["shelf"]["pinnedOrder"], json!(["a", "b"]));
        assert_eq!(
            items(&value)
                .unwrap()
                .iter()
                .filter(|i| i.kind == Kind::Checklist)
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        let expected = items(&value)
            .unwrap()
            .into_iter()
            .find(|i| i.id == "a")
            .unwrap();
        let mut item = expected.clone();
        item.pinned = false;
        value = apply(&value, &Operation::Update { expected, item })
            .unwrap()
            .unwrap();
        assert_eq!(
            items(&value)
                .unwrap()
                .iter()
                .filter(|i| i.kind == Kind::Checklist)
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            ["b", "a", "c"]
        );
        assert_eq!(value[NAMESPACE]["shelf"]["order"], base);
        value[NAMESPACE]["shelf"]["pinnedOrder"] = json!(["future-pin", "b", "a"]);
        let mut new = Item::new(Kind::Checklist, "Reserved".into(), "".into());
        new.id = "future-pin".into();
        assert!(apply(&value, &Operation::Add(new)).is_err());
        let expected = items(&value)
            .unwrap()
            .into_iter()
            .find(|i| i.id == "b")
            .unwrap();
        value = apply(&value, &Operation::Remove(expected))
            .unwrap()
            .unwrap();
        assert_eq!(
            value[NAMESPACE]["shelf"]["pinnedOrder"],
            json!(["future-pin", "a"])
        );
        for bad in [json!(true), json!(["a", "a"]), json!([3])] {
            let mut broken = value.clone();
            broken[NAMESPACE]["shelf"]["pinnedOrder"] = bad;
            assert!(items(&broken).is_err());
        }
    }
}

#[cfg(test)]
mod pinned_interop_tests {
    use super::*;
    #[test]
    fn swift_unpinned_projects_preserve_pinned_order_base_order_and_other_data() {
        let (Some(input), Some(output)) = (
            std::env::var_os("FOUNTAGE_PIN_FIXTURES"),
            std::env::var_os("FOUNTAGE_SWIFT_PIN_FIXTURES"),
        ) else {
            return;
        };
        for code in 0..8 {
            let before = crate::project::Project::read(
                &std::path::PathBuf::from(&input).join(format!("Pinned {code}.fountage")),
            )
            .unwrap();
            let after = crate::project::Project::read(
                &std::path::PathBuf::from(&output).join(format!("Swift Pinned {code}.fountage")),
            )
            .unwrap();
            assert_eq!(after.source(), before.source());
            assert_eq!(after.format(), before.format());
            for (path, entry) in before.entries() {
                if path != std::ffi::OsStr::new("info.json") {
                    assert_eq!(after.entries().get(path), Some(entry));
                }
            }
            let mut expected = before.metadata().clone();
            for id in ["plan", "later"] {
                expected[NAMESPACE]["shelf"]["items"][id]["pinned"] = false.into();
            }
            let mut actual = after.metadata().clone();
            actual[NAMESPACE]["modifiedAt"] = expected[NAMESPACE]["modifiedAt"].clone();
            assert_eq!(actual, expected);
            assert_eq!(
                items(after.metadata())
                    .unwrap()
                    .iter()
                    .filter(|i| i.kind == Kind::Checklist)
                    .map(|i| i.id.as_str())
                    .collect::<Vec<_>>(),
                ["later", "plan", "last"]
            );
        }
    }
}

#[cfg(test)]
mod insertion_tests {
    use super::*;
    #[test]
    fn exact_utf8_insertions_replace_directed_ranges_without_normalizing_bytes() {
        let source = "Title: Zoë\r\n\r\n🌙 世界 e\u{301} ending\r\n";
        let start = source.find('🌙').unwrap();
        let end = source.find(" ending").unwrap();
        let replacement = "Zoë\r\nAn alternate ending. 🌙\n";
        for (anchor, focus) in [(start, end), (end, start)] {
            let edit = insertion(source, source, anchor, focus, replacement).unwrap();
            assert_eq!(
                edit.source,
                format!("{}{}{}", &source[..start], replacement, &source[end..])
            );
            assert_eq!(
                (edit.anchor, edit.focus),
                (start + replacement.len(), start + replacement.len())
            );
        }
        for at in [0, start, source.len()] {
            let edit = insertion(source, source, at, at, replacement).unwrap();
            assert_eq!(
                edit.source,
                format!("{}{}{}", &source[..at], replacement, &source[at..])
            );
        }
        let same = insertion(source, source, end, start, &source[start..end]).unwrap();
        assert_eq!(same.source, source);
        assert_eq!(same.anchor, end);
        let remove = insertion(source, source, start, end, "").unwrap();
        assert_eq!(
            remove.source,
            format!("{}{}", &source[..start], &source[end..])
        );
    }
    #[test]
    fn stale_null_overflow_and_split_scalar_or_crlf_endpoints_reject_atomically() {
        let source = "Zoë 🌙\r\n世界";
        for (a, f) in [
            (3, 4),
            (5, 6),
            (10, 10),
            (source.len() + 1, 0),
            (usize::MAX, 0),
        ] {
            assert!(insertion(source, source, a, f, "keep").is_err(), "{a} {f}");
        }
        assert!(insertion(source, "changed", 0, 0, "x").is_err());
        assert!(insertion(source, source, 0, 0, "x\0").is_err());
        assert!(insertion("x\0", "x\0", 0, 0, "x").is_err());
    }
}

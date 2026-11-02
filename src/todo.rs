//! Source-backed checklist items. No task text or completion state is stored in metadata.
use serde::Serialize;
#[derive(Debug, Serialize)]
pub struct Todo {
    pub line: usize,
    pub start: usize,
    pub length: usize,
    pub state_start: usize,
    pub completed: bool,
    pub text: String,
}
pub fn parse(source: &str, visible: &str) -> Vec<Todo> {
    let mut result = Vec::new();
    let mut position = 0;
    for (line, (raw, masked)) in source
        .split_inclusive('\n')
        .zip(visible.split_inclusive('\n'))
        .enumerate()
    {
        let body = raw.trim_end_matches(['\r', '\n']);
        let shown = masked.trim_end_matches(['\r', '\n']);
        let indent = shown.len() - shown.trim_start_matches([' ', '\t']).len();
        let text = &shown[indent..];
        let bytes = text.as_bytes();
        if body
            .get(..indent)
            .is_some_and(|s| s.chars().all(|c| c == ' ' || c == '\t'))
            && bytes.len() >= 3
            && bytes[0] == b'['
            && bytes[2] == b']'
            && matches!(bytes[1], b' ' | b'x' | b'X')
            && (bytes.len() == 3 || matches!(bytes[3], b' ' | b'\t'))
        {
            result.push(Todo {
                line,
                start: position,
                length: body.encode_utf16().count(),
                state_start: position + body[..indent].encode_utf16().count() + 1,
                completed: bytes[1] != b' ',
                text: text[3..].trim().to_owned(),
            });
        }
        position += raw.encode_utf16().count();
    }
    result
}
/// Remove only recognized checkbox syntax from derived word-token input.
pub fn mask_states(visible: &str, todos: &[Todo]) -> String {
    let lines: std::collections::BTreeSet<_> = todos.iter().map(|t| t.line).collect();
    let mut bytes = visible.as_bytes().to_vec();
    let mut offset = 0;
    for (line, raw) in visible.split_inclusive('\n').enumerate() {
        if lines.contains(&line) {
            let indent = raw.len() - raw.trim_start_matches([' ', '\t']).len();
            bytes[offset + indent..offset + indent + 3].fill(b' ');
        }
        offset += raw.len();
    }
    String::from_utf8(bytes).expect("ASCII checkbox masking preserves UTF-8")
}

/// Toggle a currently recognized task at an exact UTF-8 line start. Reparse
/// rather than trusting a stale UI checkbox or allowing edits in hidden text.
pub fn toggle(
    source: &str,
    format: crate::writing::Format,
    start: usize,
) -> Result<crate::formatting::Edit, &'static str> {
    let prefix = source
        .get(..start)
        .ok_or("The task changed. Refresh the list and try again.")?;
    let position = prefix.encode_utf16().count();
    let analysis = crate::writing::analyze(source, format);
    let task = analysis
        .todos
        .iter()
        .find(|task| task.start == position)
        .ok_or("The task changed. Refresh the list and try again.")?;
    let line = source[start..]
        .split('\n')
        .next()
        .unwrap_or("")
        .trim_end_matches('\r');
    let state = start
        + line
            .bytes()
            .take_while(|byte| matches!(byte, b' ' | b'\t'))
            .count()
        + 1;
    let mut updated = source.to_owned();
    updated.replace_range(state..state + 1, if task.completed { " " } else { "x" });
    Ok(crate::formatting::Edit {
        source: updated,
        anchor: start,
        focus: start + line.len(),
    })
}

/// Convert complete selected lines to tasks. At a blank caret, insert a new
/// empty task and place the caret after its prefix, ready for typing.
pub fn insert(
    source: &str,
    format: crate::writing::Format,
    anchor: usize,
    focus: usize,
) -> Result<Option<crate::formatting::Edit>, &'static str> {
    let error = "The selection cannot become a visible to-do list. No text was changed.";
    if source.contains('\0') {
        return Err(error);
    }
    for byte in [anchor, focus] {
        if !source.is_char_boundary(byte)
            || (byte > 0
                && source.as_bytes().get(byte - 1) == Some(&b'\r')
                && source.as_bytes().get(byte) == Some(&b'\n'))
        {
            return Err(error);
        }
    }
    let low = anchor.min(focus);
    let high = anchor.max(focus);
    let start = source.as_bytes()[..low]
        .iter()
        .rposition(|b| *b == b'\n')
        .map_or(0, |p| p + 1);
    let last = if high > low { high - 1 } else { high };
    let end = source.as_bytes()[last..]
        .iter()
        .position(|b| *b == b'\n')
        .map_or(source.len(), |p| last + p + 1);
    let original = &source[start..end];
    let blank_caret = anchor == focus && original.trim().is_empty();
    let mut replacement = String::new();
    let mut expected = Vec::new();
    let mut units = source[..start].encode_utf16().count();
    // Empty EOF has no split_inclusive item.
    let lines: Vec<_> = if original.is_empty() {
        vec![""]
    } else {
        original.split_inclusive('\n').collect()
    };
    for raw in lines {
        let line = raw.trim_end_matches(['\r', '\n']);
        let indent = line
            .bytes()
            .take_while(|b| matches!(b, b' ' | b'\t'))
            .count();
        let body = &line[indent..];
        let existing =
            body.as_bytes().get(..3).is_some_and(|b| {
                b[0] == b'[' && matches!(b[1], b' ' | b'x' | b'X') && b[2] == b']'
            }) && (body.len() == 3 || matches!(body.as_bytes()[3], b' ' | b'\t'));
        if (!line.trim().is_empty() || blank_caret) && !existing {
            expected.push(units);
            replacement.push_str(&line[..indent]);
            replacement.push_str("[ ] ");
            replacement.push_str(&raw[indent..]);
            units += raw.encode_utf16().count() + 4;
        } else {
            replacement.push_str(raw);
            units += raw.encode_utf16().count();
        }
    }
    if replacement == original {
        return Ok(None);
    }
    let mut updated = source.to_owned();
    updated.replace_range(start..end, &replacement);
    let visible: std::collections::BTreeSet<_> = crate::writing::analyze(&updated, format)
        .todos
        .iter()
        .map(|t| t.start)
        .collect();
    if !expected.iter().all(|p| visible.contains(p)) {
        return Err(error);
    }
    let (a, f) = if blank_caret {
        let indent = original
            .bytes()
            .take_while(|b| matches!(b, b' ' | b'\t'))
            .count();
        (start + indent + 4, start + indent + 4)
    } else if anchor > focus {
        (start + replacement.len(), start)
    } else {
        (start, start + replacement.len())
    };
    Ok(Some(crate::formatting::Edit {
        source: updated,
        anchor: a,
        focus: f,
    }))
}

/// Smart Return for a recognized source task. Non-task, selected, hidden or
/// invalid input falls through to the editor's ordinary Return behavior.
pub fn newline(
    source: &str,
    format: crate::writing::Format,
    anchor: usize,
    focus: usize,
) -> Option<crate::formatting::Edit> {
    if anchor != focus
        || !source.is_char_boundary(focus)
        || (focus > 0
            && source.as_bytes().get(focus - 1) == Some(&b'\r')
            && source.as_bytes().get(focus) == Some(&b'\n'))
    {
        return None;
    }
    let start = source.as_bytes()[..focus]
        .iter()
        .rposition(|b| *b == b'\n')
        .map_or(0, |p| p + 1);
    let end = source[start..]
        .find('\n')
        .map_or(source.len(), |p| start + p);
    let end = if source.as_bytes().get(end.wrapping_sub(1)) == Some(&b'\r') {
        end - 1
    } else {
        end
    };
    let line = &source[start..end];
    let indent = line
        .bytes()
        .take_while(|b| matches!(b, b' ' | b'\t'))
        .count();
    // Ordinary Return stays cheap: parse the manuscript only for a checkbox
    // candidate on the current physical line.
    if !matches!(line.get(indent..indent + 3), Some("[ ]" | "[x]" | "[X]")) || source.contains('\0')
    {
        return None;
    }
    let units = source[..focus].encode_utf16().count();
    let analysis = crate::writing::analyze(source, format);
    let annotations = crate::fountain::analyze(source);
    if annotations
        .notes
        .iter()
        .chain(&annotations.markers)
        .any(|r| r.start <= units && units < r.start + r.length)
        || annotations
            .omissions
            .iter()
            .any(|r| r.start <= units && units < r.start + r.length)
    {
        return None;
    }
    analysis
        .todos
        .iter()
        .find(|t| units >= t.state_start + 2 && units <= t.start + t.length)?;
    let ending = if source[end..].starts_with("\r\n") {
        "\r\n"
    } else if source[end..].starts_with('\n') {
        "\n"
    } else if source
        .find('\n')
        .is_some_and(|p| p > 0 && source.as_bytes()[p - 1] == b'\r')
    {
        "\r\n"
    } else {
        "\n"
    };
    let mut updated = source.to_owned();
    let caret = if line.get(indent + 3..)?.trim().is_empty() {
        // Keep the original physical ending: replacing the empty task with an
        // additional ending leaves the blank paragraph that ends the list.
        updated.replace_range(start..end, ending);
        start + ending.len()
    } else {
        let insertion = format!("{ending}{}[ ] ", &line[..indent]);
        updated.insert_str(focus, &insertion);
        focus + insertion.len()
    };
    Some(crate::formatting::Edit {
        source: updated,
        anchor: caret,
        focus: caret,
    })
}

struct TaskLine {
    body: std::ops::Range<usize>,
    line: std::ops::Range<usize>,
    completed: bool,
}
fn task_line(
    source: &str,
    format: crate::writing::Format,
    start: usize,
) -> Result<TaskLine, &'static str> {
    let error = "The task changed or shares markup with another line. No text was changed.";
    if source.contains('\0') {
        return Err(error);
    }
    let units = source.get(..start).ok_or(error)?.encode_utf16().count();
    let analysis = crate::writing::analyze(source, format);
    let task = analysis
        .todos
        .iter()
        .find(|t| t.start == units)
        .ok_or(error)?;
    let end_units = task.start + task.length;
    let crosses = |a: usize, length: usize| {
        a < end_units && a + length > units && (a < units || a + length > end_units)
    };
    let annotations = crate::fountain::analyze(source);
    if annotations
        .notes
        .iter()
        .chain(&annotations.markers)
        .any(|r| crosses(r.start, r.length))
        || annotations
            .omissions
            .iter()
            .any(|r| crosses(r.start, r.length))
    {
        return Err(error);
    }
    let line = source[start..]
        .split('\n')
        .next()
        .unwrap_or("")
        .trim_end_matches('\r');
    let end = start + line.len();
    let indent = line
        .bytes()
        .take_while(|b| matches!(b, b' ' | b'\t'))
        .count();
    let mut body = start + indent + 3;
    while body < end && matches!(source.as_bytes()[body], b' ' | b'\t') {
        body += 1;
    }
    Ok(TaskLine {
        body: body..end,
        line: start..end,
        completed: task.completed,
    })
}
/// Exact editable tail, including inline notes and trailing whitespace. Never
/// derive an editable value from a masked display label, which would lose notes.
pub fn content(
    source: &str,
    format: crate::writing::Format,
    start: usize,
) -> Result<&str, &'static str> {
    Ok(&source[task_line(source, format, start)?.body])
}
/// Replace one task's raw content, preserving its prefix, completion state,
/// indentation, line ending and all surrounding bytes. Cross-line markup rejects.
pub fn update(
    source: &str,
    format: crate::writing::Format,
    start: usize,
    text: &str,
) -> Result<Option<crate::formatting::Edit>, &'static str> {
    let line = task_line(source, format, start)?;
    if text.chars().any(|c| {
        matches!(
            c,
            '\0' | '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{85}' | '\u{2028}' | '\u{2029}'
        )
    }) {
        return Err("Keep the task on one line. No text was changed.");
    }
    if &source[line.body.clone()] == text {
        return Ok(None);
    }
    let prefix_only = line.body.start
        == line.line.start
            + source[line.line.clone()]
                .bytes()
                .take_while(|b| matches!(b, b' ' | b'\t'))
                .count()
            + 3;
    let replacement = if prefix_only && !text.is_empty() {
        format!(" {text}")
    } else {
        text.to_owned()
    };
    let mut candidate = source.to_owned();
    candidate.replace_range(line.body, &replacement);
    let updated = task_line(&candidate, format, start)?;
    if updated.completed != line.completed {
        return Err("The task changed. No text was changed.");
    }
    Ok(Some(crate::formatting::Edit {
        source: candidate,
        anchor: start,
        focus: updated.line.end,
    }))
}
/// Move a visible task past its previous/next visible task. Exchange complete
/// physical line bodies: separators stay in their original slots, including a
/// missing final newline. Intervening prose is never moved with a task.
pub fn move_task(
    source: &str,
    format: crate::writing::Format,
    start: usize,
    down: bool,
) -> Result<Option<crate::formatting::Edit>, &'static str> {
    let selected = task_line(source, format, start)?;
    let error = "The task move would change surrounding markup. No text was changed.";
    let analysis = crate::writing::analyze(source, format);
    let units = source[..start].encode_utf16().count();
    let index = analysis
        .todos
        .iter()
        .position(|t| t.start == units)
        .ok_or(error)?;
    let neighbor_index = if down {
        index.checked_add(1).filter(|i| *i < analysis.todos.len())
    } else {
        index.checked_sub(1)
    };
    let Some(neighbor_index) = neighbor_index else {
        return Ok(None);
    };
    let neighbor_line = analysis.todos[neighbor_index].line;
    let neighbor_start: usize = source
        .split_inclusive('\n')
        .take(neighbor_line)
        .map(str::len)
        .sum();
    let neighbor = task_line(source, format, neighbor_start)?;
    let (first, last) = if down {
        (&selected.line, &neighbor.line)
    } else {
        (&neighbor.line, &selected.line)
    };
    let first_text = &source[first.clone()];
    let last_text = &source[last.clone()];
    let middle = &source[first.end..last.start];
    let mut candidate = String::with_capacity(source.len());
    candidate.push_str(&source[..first.start]);
    candidate.push_str(last_text);
    candidate.push_str(middle);
    candidate.push_str(first_text);
    candidate.push_str(&source[last.end..]);
    if candidate == source {
        return Ok(None);
    }
    let second_start = first.start + last_text.len() + middle.len();
    task_line(&candidate, format, first.start)?;
    task_line(&candidate, format, second_start)?;
    let signature = |text: &str, tasks: &[Todo]| {
        let lines: Vec<_> = text.split('\n').collect();
        tasks
            .iter()
            .map(|task| {
                (
                    lines[task.line].trim_end_matches('\r').to_owned(),
                    task.completed,
                )
            })
            .collect::<Vec<_>>()
    };
    let mut expected = signature(source, &analysis.todos);
    expected.swap(index, neighbor_index);
    let updated = crate::writing::analyze(&candidate, format);
    if signature(&candidate, &updated.todos) != expected {
        return Err(error);
    }
    let anchor = if down { second_start } else { first.start };
    let focus = anchor + selected.line.len();
    Ok(Some(crate::formatting::Edit {
        source: candidate,
        anchor,
        focus,
    }))
}
/// Remove a complete visible task line, including its physical line ending.
/// Cross-line annotations must be edited in source to preserve their scope.
pub fn remove(
    source: &str,
    format: crate::writing::Format,
    start: usize,
) -> Result<crate::formatting::Edit, &'static str> {
    task_line(source, format, start)?;
    let end = source[start..]
        .find('\n')
        .map_or(source.len(), |p| start + p + 1);
    let mut updated = source.to_owned();
    updated.replace_range(start..end, "");
    Ok(crate::formatting::Edit {
        source: updated,
        anchor: start,
        focus: start,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn exact_ranges_states_and_hidden_contents() {
        let source = "# Work\r\n\r\n  [ ] Zoë 🌙 [[private]]\r\n[x] Done\r\n[X] Also done\r\n[[\r\n[ ] Hidden\r\n]]\r\n/*\r\n[x] Omitted\r\n*/\r\n[?] Not a task\r\n[ ]joined\r\n\\[ ] Literal";
        let analysis = crate::fountain::analyze(source);
        assert_eq!(analysis.todos.len(), 3);
        assert_eq!(
            analysis
                .todos
                .iter()
                .map(|t| t.completed)
                .collect::<Vec<_>>(),
            [false, true, true]
        );
        assert_eq!(analysis.todos[0].text, "Zoë 🌙");
        let units: Vec<_> = source.encode_utf16().collect();
        for item in analysis.todos {
            assert!(matches!(units[item.state_start], 32 | 120 | 88));
            assert!(
                String::from_utf16(&units[item.start..item.start + item.length])
                    .unwrap()
                    .contains("] ")
            );
        }
    }
    #[test]
    fn every_category_recognizes_tasks_and_completion_does_not_add_a_word() {
        for format in [
            crate::writing::Format::Screenplay,
            crate::writing::Format::Comic,
            crate::writing::Format::Novel,
            crate::writing::Format::Academic,
            crate::writing::Format::Stageplay,
            crate::writing::Format::Audio,
            crate::writing::Format::General,
            crate::writing::Format::Poetry,
        ] {
            let open = crate::writing::analyze("[ ] Review Zoë 🌙\r\n\r\n[x] Send draft", format);
            let done = crate::writing::analyze("[x] Review Zoë 🌙\r\n\r\n[x] Send draft", format);
            assert_eq!(open.todos.len(), 2);
            assert_eq!(open.words, done.words);
            assert_eq!(
                open.words,
                if format == crate::writing::Format::Screenplay {
                    4
                } else {
                    5
                }
            );
        }
    }
}

#[cfg(test)]
mod toggle_tests {
    #[test]
    fn toggle_changes_only_state_in_every_category_and_rejects_hidden_or_stale_positions() {
        let source = "Title: Tasks\r\n\r\n  [ ] Zoë 🌙 [[private]]\r\n[X] Done\r\n[[\r\n[ ] Hidden\r\n]]\r\n/*\r\n[ ] Omitted\r\n*/\r\n";
        let start = source.find("  [ ]").unwrap();
        for code in 0..8 {
            let format = crate::writing::Format::from_code(code).unwrap();
            let edit = super::toggle(source, format, start).unwrap();
            assert_eq!(edit.source, source.replacen("[ ]", "[x]", 1));
            assert_eq!(
                &edit.source[edit.anchor..edit.focus],
                "  [x] Zoë 🌙 [[private]]"
            );
            assert_eq!(
                super::toggle(&edit.source, format, start).unwrap().source,
                source
            );
            let done = super::toggle(source, format, source.find("[X]").unwrap()).unwrap();
            assert!(done.source.contains("[ ] Done\r\n"));
            for invalid in [
                start + 1,
                source.find("ë").unwrap() + 1,
                source.find("[ ] Hidden").unwrap(),
                source.find("[ ] Omitted").unwrap(),
                source.len() + 1,
            ] {
                assert!(super::toggle(source, format, invalid).is_err());
            }
        }
    }
}

#[cfg(test)]
mod insert_tests {
    #[test]
    fn selected_lines_match_swift_conversion_without_losing_direction_or_bytes() {
        let source = "Before\r\n  First 🌙\r\n\r\n\tSecond\r\n[x] Done\r\nAfter";
        let start = source.find("  First").unwrap();
        let end = source.find("After").unwrap();
        for code in 0..8 {
            let format = crate::writing::Format::from_code(code).unwrap();
            for (a, f) in [(start, end), (end, start)] {
                let edit = super::insert(source, format, a, f).unwrap().unwrap();
                assert_eq!(
                    edit.source,
                    "Before\r\n  [ ] First 🌙\r\n\r\n\t[ ] Second\r\n[x] Done\r\nAfter"
                );
                assert_eq!(edit.anchor > edit.focus, a > f);
                assert!(
                    super::insert(&edit.source, format, edit.anchor, edit.focus)
                        .unwrap()
                        .is_none()
                );
            }
            let caret = super::insert("\t\r\n", format, 1, 1).unwrap().unwrap();
            assert_eq!(caret.source, "\t[ ] \r\n");
            assert_eq!((caret.anchor, caret.focus), (5, 5));
            let empty = super::insert("", format, 0, 0).unwrap().unwrap();
            assert_eq!(empty.source, "[ ] ");
            assert_eq!((empty.anchor, empty.focus), (4, 4));
            for (source, a, f) in [
                ("🌙", 1, 1),
                ("x\r\n", 2, 2),
                ("x", 2, 2),
                ("x\0", 0, 0),
                ("/*\nHidden\n*/", 3, 9),
                ("[[\nHidden\n]]", 3, 9),
            ] {
                assert!(super::insert(source, format, a, f).is_err());
            }
        }
    }
}

#[cfg(test)]
mod newline_tests {
    #[test]
    fn task_return_matches_swift_split_continue_exit_and_mixed_endings() {
        for code in 0..8 {
            let format = crate::writing::Format::from_code(code).unwrap();
            for ending in ["\n", "\r\n"] {
                for state in [" ", "x", "X"] {
                    let source = format!("# Work{ending}{ending}\t[{state}] Zoë 🌙 waits.");
                    let at = source.find(" waits").unwrap();
                    let split = super::newline(&source, format, at, at).unwrap();
                    assert_eq!(
                        split.source,
                        source.replacen(" waits.", &format!("{ending}\t[ ]  waits."), 1)
                    );
                    let next = super::newline(&source, format, source.len(), source.len()).unwrap();
                    assert_eq!(next.source, format!("{source}{ending}\t[ ] "));
                    let exit =
                        super::newline(&next.source, format, next.focus, next.focus).unwrap();
                    assert_eq!(exit.source, format!("{source}{ending}{ending}"));
                }
            }
            let mixed = "Before\n\t[ ] Task\r\nAfter";
            let at = mixed.find("Task").unwrap() + 4;
            assert_eq!(
                super::newline(mixed, format, at, at).unwrap().source,
                "Before\n\t[ ] Task\r\n\t[ ] \r\nAfter"
            );
            for source in [
                "Normal prose.",
                "/*\n[ ] Hidden\n*/",
                "[[\n[ ] Hidden\n]]",
                "\\[ ] Literal",
                "[?] Not a task",
            ] {
                assert!(super::newline(source, format, source.len(), source.len()).is_none());
            }
            let source = "[ ] Task [[private]] {{%m: name}}";
            for word in ["private", "name"] {
                let at = source.find(word).unwrap() + 1;
                assert!(super::newline(source, format, at, at).is_none());
            }
            assert!(super::newline(source, format, 4, 8).is_none());
            assert!(super::newline("[ ] 🌙", format, 5, 5).is_none());
            let omitted = "[ ] Task /*private*/";
            let at = omitted.find("private").unwrap() + 1;
            assert!(super::newline(omitted, format, at, at).is_none());
            let annotated = "[ ] [[keep]]";
            assert_eq!(
                super::newline(annotated, format, annotated.len(), annotated.len())
                    .unwrap()
                    .source,
                "[ ] [[keep]]\n[ ] "
            );
        }
    }
}

#[cfg(test)]
mod removal_tests {
    #[test]
    fn removes_only_task_and_ending_and_refuses_cross_line_annotations() {
        for code in 0..8 {
            let format = crate::writing::Format::from_code(code).unwrap();
            let source = "Before 🌙\r\n\t[X] Review Zoë [[private]]\r\nAfter\r\n";
            let start = source.find("\t[X]").unwrap();
            let edit = super::remove(source, format, start).unwrap();
            assert_eq!(edit.source, "Before 🌙\r\nAfter\r\n");
            assert_eq!((edit.anchor, edit.focus), (start, start));
            assert_eq!(super::remove("[ ] Last 🌙", format, 0).unwrap().source, "");
            for source in [
                "[ ] Work [[private\ncontinued]]\nAfter",
                "[ ] Work /*private\ncontinued*/\nAfter",
                "[[\n[ ] Hidden\n]]",
                "/*\n[ ] Hidden\n*/",
                "Normal text",
            ] {
                let at = source.find("[ ]").unwrap_or(0);
                assert!(
                    super::remove(source, format, at).is_err(),
                    "category {code}: {source:?}"
                );
            }
            assert!(super::remove(source, format, start + 1).is_err());
            assert!(super::remove(source, format, source.find('ë').unwrap() + 1).is_err());
        }
    }
    #[test]
    fn edits_raw_task_tail_without_losing_prefix_state_notes_or_surrounding_bytes_all_formats() {
        for code in 0..8 {
            let format = crate::writing::Format::from_code(code).unwrap();
            for state in [" ", "x", "X"] {
                for ending in ["\n", "\r\n"] {
                    let before = format!(
                        "Zoë 🌙{ending}\t[{state}]  Review [[private]]  {ending}[ ] Next{ending}"
                    );
                    let start = before.find('\t').unwrap();
                    assert_eq!(
                        super::content(&before, format, start).unwrap(),
                        "Review [[private]]  "
                    );
                    assert!(
                        super::update(&before, format, start, "Review [[private]]  ")
                            .unwrap()
                            .is_none()
                    );
                    let edit = super::update(&before, format, start, "Updated 世界 [[private]]")
                        .unwrap()
                        .unwrap();
                    assert_eq!(
                        edit.source,
                        format!(
                            "Zoë 🌙{ending}\t[{state}]  Updated 世界 [[private]]{ending}[ ] Next{ending}"
                        )
                    );
                    assert_eq!(edit.anchor, start);
                    assert!(edit.source.is_char_boundary(edit.focus));
                }
            }
            let edit = super::update("[X]", format, 0, "Ready").unwrap().unwrap();
            assert_eq!(edit.source, "[X] Ready");
            assert_eq!(
                super::update("[ ] Old", format, 0, "")
                    .unwrap()
                    .unwrap()
                    .source,
                "[ ] "
            );
            for invalid in ["two\nlines", "bad\0input", "\u{2028}", "\r", "\u{85}"] {
                assert!(super::update("[ ] Old", format, 0, invalid).is_err());
            }
            for hidden in ["[[\n[ ] Hidden\n]]", "/*\n[ ] Hidden\n*/"] {
                assert!(super::update(hidden, format, hidden.find("[ ]").unwrap(), "New").is_err());
            }
            assert!(super::update("[ ] A [[cross\nline]]\n[ ] Next", format, 0, "New").is_err());
            assert!(super::update("[ ] A\nline]]\n[ ] Next", format, 0, "New [[cross").is_err());
            assert!(super::update("[ ] A\nline */\n[ ] Next", format, 0, "New /*").is_err());
            assert!(super::update("[ ] A", format, 1, "Wrong start").is_err());
        }
    }
}

#[cfg(test)]
mod move_tests {
    use super::*;
    #[test]
    fn move_preserves_bodies_notes_completion_separators_and_prose_in_every_format() {
        for code in 0..8 {
            let format = crate::writing::Format::from_code(code).unwrap();
            for (one, two, end) in [
                ("\n", "\n", "\n"),
                ("\r\n", "\r\n", "\r\n"),
                ("\r\n", "\n", ""),
            ] {
                let source = format!(
                    "Zoë 🌙{one}\t[ ] Review [[note]]  {one}Prose stays{two}[X]\tSend 世界{end}"
                );
                let first = source.find('\t').unwrap();
                assert!(move_task(&source, format, first, false).unwrap().is_none());
                let moved = move_task(&source, format, first, true).unwrap().unwrap();
                assert_eq!(
                    moved.source,
                    format!(
                        "Zoë 🌙{one}[X]\tSend 世界{one}Prose stays{two}\t[ ] Review [[note]]  {end}"
                    )
                );
                assert_eq!(
                    &moved.source[moved.anchor..moved.focus],
                    "\t[ ] Review [[note]]  "
                );
                assert!(
                    move_task(&moved.source, format, moved.anchor, true)
                        .unwrap()
                        .is_none()
                );
                let restored = move_task(&moved.source, format, moved.anchor, false)
                    .unwrap()
                    .unwrap();
                assert_eq!(restored.source, source);
                assert_eq!(restored.anchor, first);
                assert!(
                    move_task("[ ] Duplicate\n[ ] Duplicate", format, 0, true)
                        .unwrap()
                        .is_none()
                );
                assert!(move_task("[X]", format, 0, true).unwrap().is_none());
            }
        }
    }
    #[test]
    fn invalid_hidden_or_crossing_markup_cannot_be_reordered() {
        for code in 0..8 {
            let format = crate::writing::Format::from_code(code).unwrap();
            for source in [
                "[ ] A [[cross\nline]]\n[ ] B",
                "[ ] A\n[ ] B [[cross\nline]]",
                "[ ] A ]]\n[ ] B [[\n",
            ] {
                assert!(
                    move_task(source, format, 0, true).is_err(),
                    "{code}: {source:?}"
                );
            }
            let source = "Zoë 🌙\n[ ] A\n[ ] B";
            assert!(move_task(source, format, source.find('ë').unwrap() + 1, true).is_err());
            assert!(move_task(source, format, source.find("[ ]").unwrap() + 1, true).is_err());
            for source in ["[[\n[ ] Hidden\n]]", "/*\n[ ] Hidden\n*/"] {
                assert!(move_task(source, format, source.find("[ ]").unwrap(), true).is_err());
            }
        }
    }
}

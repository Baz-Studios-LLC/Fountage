//! Source transactions above the toolkit's two-step selection-replacement undo.
//! Store changed spans, not a full manuscript per keystroke. Offsets are UTF-8 bytes.
// Keep generated-editor timing tests independent of filesystem and CI scheduling.
#[cfg(test)]
thread_local! { static TEST_TIME_MS: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) }; }
#[cfg(test)]
pub(crate) fn set_test_time(milliseconds: Option<u64>) {
    TEST_TIME_MS.with(|time| time.set(milliseconds));
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionState {
    pub anchor: usize,
    pub focus: usize,
    pub anchor_upstream: bool,
    pub focus_upstream: bool,
}
impl SelectionState {
    pub fn plain(anchor: usize, focus: usize) -> Self {
        Self {
            anchor,
            focus,
            anchor_upstream: false,
            focus_upstream: false,
        }
    }
}
#[derive(Debug, Clone)]
pub struct ProjectChange {
    pub project_id: String,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}
#[derive(Debug, Clone)]
struct Edit {
    start: usize,
    removed: String,
    inserted: String,
    before_selection: Option<SelectionState>,
    after_selection: Option<SelectionState>,
    project: Option<ProjectChange>,
}
#[derive(Clone)]
pub struct History {
    source: String,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    started: std::time::Instant,

    typing: bool,
    previous_typing: bool,
    last_edit_ms: u64,
    selection: Option<SelectionState>,
}
impl Default for History {
    fn default() -> Self {
        Self {
            source: String::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            started: std::time::Instant::now(),

            typing: false,
            previous_typing: false,
            last_edit_ms: 0,
            selection: None,
        }
    }
}
impl History {
    pub(crate) fn undo_depth(&self) -> usize {
        self.undo.len()
    }
    pub fn availability(&self, source: &str) -> (bool, bool) {
        if source != self.source {
            return (false, false);
        }
        (!self.undo.is_empty(), !self.redo.is_empty())
    }

    pub fn synchronize(&mut self, source: &str) {
        if self.source != source {
            self.reset(source);
        }
    }
    pub fn discard_redo(&mut self) {
        self.redo.clear();
    }
    pub fn boundary(&mut self) {
        self.typing = false;
        self.previous_typing = false;
    }
    pub fn input(&mut self, text: &str) {
        self.typing = text.chars().count() == 1
            && text
                .chars()
                .all(|c| !c.is_control() && !('\u{f700}'..='\u{f7ff}').contains(&c));
        if !self.typing {
            self.previous_typing = false;
        }
    }
    pub fn reset(&mut self, source: &str) {
        self.boundary();
        self.source = source.to_owned();
        self.selection = None;
        self.undo.clear();
        self.redo.clear();
    }
    pub fn observe_selection(&mut self, source: &str, anchor: usize, focus: usize) {
        self.observe_visual_selection(source, SelectionState::plain(anchor, focus));
    }
    pub fn observe_visual_selection(&mut self, source: &str, selection: SelectionState) {
        if source == self.source
            && source.is_char_boundary(selection.anchor)
            && source.is_char_boundary(selection.focus)
        {
            if self.selection.is_some_and(|previous| previous != selection) {
                self.boundary();
            }
            self.selection = Some(selection);
        }
    }
    #[cfg(test)]
    pub fn record_selected(&mut self, source: &str, anchor: usize, focus: usize) {
        self.record_visual_selected(source, SelectionState::plain(anchor, focus));
    }
    pub fn record_visual_selected(&mut self, source: &str, selection: SelectionState) {
        if self.source == source {
            self.typing = false;
            self.observe_visual_selection(source, selection);
            return;
        }
        self.record(source);
        if source.is_char_boundary(selection.anchor) && source.is_char_boundary(selection.focus) {
            self.selection = Some(selection);
            if let Some(edit) = self.undo.last_mut() {
                edit.after_selection = self.selection;
            }
        }
    }
    pub fn record(&mut self, source: &str) {
        let milliseconds = self.started.elapsed().as_millis().min(u64::MAX as u128) as u64;
        #[cfg(test)]
        let milliseconds = TEST_TIME_MS.with(|time| time.get()).unwrap_or(milliseconds);
        self.record_at(source, milliseconds);
    }
    fn record_at(&mut self, source: &str, milliseconds: u64) {
        if self.source == source {
            self.typing = false;
            return;
        }
        let start = self
            .source
            .chars()
            .zip(source.chars())
            .take_while(|(a, b)| a == b)
            .map(|(c, _)| c.len_utf8())
            .sum::<usize>();
        let suffix = self.source[start..]
            .chars()
            .rev()
            .zip(source[start..].chars().rev())
            .take_while(|(a, b)| a == b)
            .map(|(c, _)| c.len_utf8())
            .sum::<usize>();
        let edit = Edit {
            start,
            removed: self.source[start..self.source.len() - suffix].to_owned(),
            inserted: source[start..source.len() - suffix].to_owned(),
            before_selection: self.selection,
            after_selection: None,
            project: None,
        };
        let merge = self.typing
            && self.previous_typing
            && milliseconds.saturating_sub(self.last_edit_ms) <= 750
            && edit.removed.is_empty()
            && edit.inserted.chars().count() == 1
            && self.undo.last().is_some_and(|previous| {
                previous.project.is_none()
                    && previous.removed.is_empty()
                    && previous.start + previous.inserted.len() == edit.start
                    && previous.inserted.chars().count() < 64
            });
        if merge {
            self.undo
                .last_mut()
                .unwrap()
                .inserted
                .push_str(&edit.inserted);
            self.undo.last_mut().unwrap().after_selection = None;
        } else {
            self.undo.push(edit);
        }
        self.previous_typing =
            self.typing && self.undo.last().is_some_and(|e| e.removed.is_empty());
        self.typing = false;
        self.last_edit_ms = milliseconds;
        self.redo.clear();
        self.source = source.to_owned();
        self.selection = None;
    }
    pub fn step(&mut self, current: &str, redo: bool) -> Option<(String, usize, usize)> {
        self.step_visual(current, redo)
            .map(|(source, selection)| (source, selection.anchor, selection.focus))
    }
    pub fn project_change(&self, current: &str, redo: bool) -> Option<ProjectChange> {
        if current != self.source {
            return None;
        }
        (if redo {
            self.redo.last()
        } else {
            self.undo.last()
        })?
        .project
        .clone()
    }
    pub fn record_project(
        &mut self,
        before: &str,
        after: &str,
        selection: SelectionState,
        next: SelectionState,
        project: ProjectChange,
    ) -> bool {
        if self.source != before
            || !before.is_char_boundary(selection.anchor)
            || !before.is_char_boundary(selection.focus)
            || !after.is_char_boundary(next.anchor)
            || !after.is_char_boundary(next.focus)
        {
            return false;
        }
        self.boundary();
        self.observe_visual_selection(before, selection);
        if before == after {
            self.undo.push(Edit {
                start: 0,
                removed: String::new(),
                inserted: String::new(),
                before_selection: Some(selection),
                after_selection: Some(next),
                project: Some(project),
            });
            self.redo.clear();
            self.selection = Some(next);
        } else {
            self.record_visual_selected(after, next);
            self.undo.last_mut().unwrap().project = Some(project);
        }
        self.boundary();
        true
    }
    pub fn step_project(
        &mut self,
        current: &str,
        redo: bool,
    ) -> Option<(String, SelectionState, ProjectChange)> {
        let project = self.project_change(current, redo)?;
        let (source, selection) = self.step_inner(current, redo)?;
        Some((source, selection, project))
    }
    pub fn step_visual(&mut self, current: &str, redo: bool) -> Option<(String, SelectionState)> {
        // Composite entries must be restored with their project metadata/recovery.
        if self.project_change(current, redo).is_some() {
            return None;
        }
        self.step_inner(current, redo)
    }
    fn step_inner(&mut self, current: &str, redo: bool) -> Option<(String, SelectionState)> {
        self.boundary();
        // Never replay an edit into an externally replaced manuscript.
        if current != self.source {
            return None;
        }
        let (from, to) = if redo {
            (&mut self.redo, &mut self.undo)
        } else {
            (&mut self.undo, &mut self.redo)
        };
        let edit = from.pop()?;
        let (old, new) = if redo {
            (&edit.removed, &edit.inserted)
        } else {
            (&edit.inserted, &edit.removed)
        };
        self.source
            .replace_range(edit.start..edit.start + old.len(), new);
        let anchor = if redo {
            edit.start + new.len()
        } else {
            edit.start
        };
        let focus = edit.start + new.len();
        let selection = if redo {
            edit.after_selection
        } else {
            edit.before_selection
        }
        .unwrap_or(SelectionState::plain(anchor, focus));
        self.selection = Some(selection);
        to.push(edit);
        Some((self.source.clone(), selection))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_selection_survives_minimal_diff_and_noop_or_stale_observations() {
        let before = "🌙\r\n[[Third 世界 note]]\r\n";
        let after = "🌙\r\n[[Revised 世界 note]]\r\n";
        let start = before.find("[[").unwrap();
        let end = before.find("]]").unwrap() + 2;
        let caret = after.find("]]").unwrap() + 2;
        let mut history = History::default();
        history.reset(before);
        history.observe_selection(before, end, start);
        history.observe_selection("stale", 0, 0);
        history.observe_selection(before, 1, 2); // inside emoji bytes: rejected
        history.record_selected(after, caret, caret);
        history.record_selected(after, 0, 0); // no-op must not rewrite prior edit's after-selection
        assert_eq!(history.undo.len(), 1);
        assert_eq!(history.undo[0].removed, "Thir", "keep compact text diff");
        assert_eq!(
            history.step(after, false).unwrap(),
            (before.into(), end, start)
        );
        assert_eq!(
            history.step(before, true).unwrap(),
            (after.into(), caret, caret)
        );
        history.reset("");
        history.observe_selection("", 0, 0);
        for value in ["Z", "Zo", "Zoë"] {
            history.input(value.chars().last().unwrap().to_string().as_str());
            history.record_selected(value, value.len(), value.len());
        }
        assert_eq!(history.step("Zoë", false).unwrap(), ("".into(), 0, 0));
        assert_eq!(history.step("", true).unwrap(), ("Zoë".into(), 4, 4));
    }
    #[test]
    fn typing_merges_only_within_time_position_and_explicit_boundaries() {
        let mut h = History::default();
        h.reset("");
        h.input("Z");
        h.record_at("Z", 0);
        h.input("o");
        h.record_at("Zo", 50);
        h.input("ë");
        h.record_at("Zoë", 100);
        h.input("🌙");
        h.record_at("Zoë🌙", 1000); // A pause starts a new run.
        assert_eq!(h.step("Zoë🌙", false).unwrap().0, "Zoë");
        assert_eq!(h.step("Zoë", false).unwrap().0, "");
        h.reset("");
        h.input("a");
        h.record_at("a", 0);
        h.boundary();
        h.input("b");
        h.record_at("ab", 1);
        assert_eq!(h.step("ab", false).unwrap().0, "a");
        h.input("paste");
        h.record_at("apaste", 2);
        h.input("!");
        h.record_at("apaste!", 3);
        assert_eq!(h.step("apaste!", false).unwrap().0, "apaste");
        assert_eq!(h.step("apaste", false).unwrap().0, "a");
    }
    #[test]
    fn long_uninterrupted_typing_is_split_without_losing_any_edits() {
        let mut h = History::default();
        for count in 1..=65 {
            h.input("a");
            h.record_at(&"a".repeat(count), count as u64);
        }
        assert_eq!(h.step(&"a".repeat(65), false).unwrap().0, "a".repeat(64));
        assert_eq!(h.step(&"a".repeat(64), false).unwrap().0, "");
        assert_eq!(h.step("", true).unwrap().0, "a".repeat(64));
        assert_eq!(h.step(&"a".repeat(64), true).unwrap().0, "a".repeat(65));
    }
    #[test]
    fn replacement_is_atomic_branches_clear_redo_and_external_source_is_guarded() {
        let before = "Zoë 🌙 e\u{301} 世界\r\n";
        let after = "Zoë ✨ مرحبًا\r\n";
        let mut h = History::default();
        h.reset(before);
        h.record(after);
        assert!(h.step("unrelated", false).is_none());
        assert_eq!(h.step(after, false).unwrap().0, before);
        assert_eq!(h.step(before, true).unwrap().0, after);
        h.step(after, false).unwrap();
        h.record("new branch");
        assert!(h.step("new branch", true).is_none());
        assert_eq!(h.step("new branch", false).unwrap().0, before);
        h.reset("recovered");
        assert!(h.step("recovered", false).is_none());
    }
    #[test]
    fn all_scalar_boundaries_and_empty_edits_round_trip() {
        let cases = ["", "🌙", "e\u{301}", "世界\r\n", "مرحبا", "\0"];
        for before in cases {
            for after in cases {
                let mut h = History::default();
                h.reset(before);
                h.record(after);
                if before == after {
                    assert!(h.step(after, false).is_none());
                    continue;
                }
                let (s, a, b) = h.step(after, false).unwrap();
                assert_eq!(s, before);
                assert!(s.is_char_boundary(a) && s.is_char_boundary(b));
                let (s, a, b) = h.step(before, true).unwrap();
                assert_eq!(s, after);
                assert!(s.is_char_boundary(a) && s.is_char_boundary(b));
            }
        }
    }
}

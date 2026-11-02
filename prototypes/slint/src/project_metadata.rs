//! Independent project metadata history. Recovery is published before adoption.
//! Source history remains separate; metadata Undo always uses the live manuscript.
use crate::{history::SelectionState, recovery, session};
use fountage_core::{
    project::{Entry, Project},
    shelf::Operation,
};
use std::path::Path;

#[derive(Clone, Copy)]
struct QuickSelection {
    before: SelectionState,
    after: SelectionState,
    typing: bool,
}
pub struct QuickEdit<'a> {
    pub expected: &'a serde_json::Value,
    pub operation: &'a Operation,
    pub root: &'a Path,
    pub after: SelectionState,
}
struct EditIntent<'a> {
    expected: &'a serde_json::Value,
    operation: &'a Operation,
    root: &'a Path,
    quick: Option<SelectionState>,
}
#[derive(Clone)]
struct Change {
    project_id: String,
    before: Vec<u8>,
    after: Vec<u8>,
    quick: Option<QuickSelection>,
}
#[derive(Default)]
pub struct History {
    undo: Vec<Change>,
    redo: Vec<Change>,
    observed: Option<(String, Vec<u8>)>,
    edit_epoch: u64,
    recovery: Option<recovery::Recovery>,
    quick_typing: crate::history::History,
    quick_before: Option<SelectionState>,
    restored_quick_selection: Option<SelectionState>,
}
pub(crate) fn info(project: &Project) -> Result<Vec<u8>, String> {
    match project.entries().get(std::ffi::OsStr::new("info.json")) {
        Some(Entry::File(bytes)) => Ok(bytes.clone()),
        _ => Err("Project metadata is unavailable. No changes were adopted.".into()),
    }
}
impl History {
    pub fn quick_input(&mut self, current: &str, input: &str, selection: SelectionState) {
        self.quick_typing.synchronize(current);
        self.quick_typing
            .observe_visual_selection(current, selection);
        self.quick_typing.input(input);
        self.quick_before = Some(selection);
    }
    pub fn quick_selection(&mut self, current: &str, selection: SelectionState) {
        self.quick_typing
            .observe_visual_selection(current, selection);
        self.quick_before = Some(selection);
    }
    pub fn quick_boundary(&mut self) {
        self.quick_typing.boundary();
    }
    pub fn restored_quick_selection(&self) -> Option<SelectionState> {
        self.restored_quick_selection
    }
    pub fn attach_recovery(&mut self, current: Option<&recovery::Recovery>) {
        self.recovery = current.cloned();
    }
    pub fn edit_epoch(&self) -> u64 {
        self.edit_epoch
    }
    pub fn reset(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.observed = None;
        self.recovery = None;
        self.quick_typing.reset("");
        self.quick_before = None;
        self.restored_quick_selection = None;
    }
    pub fn adopt_external(&mut self, project: &Project) {
        self.quick_typing
            .reset(fountage_core::shelf::quick_text(project.metadata()).unwrap_or(""));
        self.quick_before = None;
        self.restored_quick_selection = None;
        self.observe(project);
    }
    fn observe(&mut self, project: &Project) {
        self.observed = info(project)
            .ok()
            .map(|bytes| (project.project_id().to_owned(), bytes));
    }
    pub fn availability(&self, project: &Project) -> (bool, bool) {
        let Ok(current) = info(project) else {
            return (false, false);
        };
        let matches = |frame: &Change, bytes: &[u8]| {
            frame.project_id == project.project_id() && bytes == current
        };
        (
            self.undo.last().is_some_and(|f| matches(f, &f.after)),
            self.redo.last().is_some_and(|f| matches(f, &f.before)),
        )
    }
    pub fn edit(
        &mut self,
        project: &mut Project,
        source: &str,
        checkpoint: &mut session::Checkpoint,
        expected: &serde_json::Value,
        operation: &Operation,
        root: &Path,
    ) -> Result<Option<recovery::Recovery>, String> {
        self.apply_edit(
            project,
            source,
            checkpoint,
            EditIntent {
                expected,
                operation,
                root,
                quick: None,
            },
        )
    }
    pub fn edit_quick(
        &mut self,
        project: &mut Project,
        source: &str,
        checkpoint: &mut session::Checkpoint,
        request: QuickEdit<'_>,
    ) -> Result<Option<recovery::Recovery>, String> {
        self.apply_edit(
            project,
            source,
            checkpoint,
            EditIntent {
                expected: request.expected,
                operation: request.operation,
                root: request.root,
                quick: Some(request.after),
            },
        )
    }
    fn apply_edit(
        &mut self,
        project: &mut Project,
        source: &str,
        checkpoint: &mut session::Checkpoint,
        intent: EditIntent<'_>,
    ) -> Result<Option<recovery::Recovery>, String> {
        let EditIntent {
            expected,
            operation,
            root,
            quick,
        } = intent;
        if !checkpoint.tracks_metadata(Some(project.metadata())) {
            return Err(
                "The checkpoint does not match the current project. No Shelf edit was adopted."
                    .into(),
            );
        }
        if (!self.undo.is_empty() || !self.redo.is_empty())
            && self.observed.as_ref().is_none_or(|(id, bytes)| {
                id != project.project_id() || info(project).as_ref().ok() != Some(bytes)
            })
        {
            return Err(
                "The project changed. Reset metadata history after adopting a different document."
                    .into(),
            );
        }
        let mut candidate = project.clone();
        candidate.replace_source(source)?;
        if !candidate.edit_shelf(expected, operation)? {
            return Ok(None);
        }
        let mut typing = self.quick_typing.clone();
        let mut merge = false;
        let selection = if let Some(after) = quick {
            let old =
                fountage_core::shelf::quick_text(project.metadata()).map_err(str::to_owned)?;
            let new =
                fountage_core::shelf::quick_text(candidate.metadata()).map_err(str::to_owned)?;
            let before = self.quick_before.unwrap_or(SelectionState::plain(0, 0));
            if !old.is_char_boundary(before.anchor)
                || !old.is_char_boundary(before.focus)
                || !new.is_char_boundary(after.anchor)
                || !new.is_char_boundary(after.focus)
            {
                return Err("Quick Note selection changed. No history edit was adopted.".into());
            }
            let plain_typing = matches!(operation, Operation::SetQuickText { .. });
            if plain_typing {
                typing.synchronize(old);
                typing.observe_visual_selection(old, before);
                let depth = typing.undo_depth();
                typing.record_visual_selected(new, after);
                merge = depth > 0
                    && typing.undo_depth() == depth
                    && self.undo.last().is_some_and(|frame| {
                        frame.quick.is_some_and(|q| q.typing)
                            && frame.after == info(project).unwrap_or_default()
                    });
            } else {
                typing.reset(new);
            }
            Some(QuickSelection {
                before,
                after,
                typing: plain_typing,
            })
        } else {
            typing.reset(fountage_core::shelf::quick_text(candidate.metadata()).unwrap_or(""));
            None
        };
        let change = Change {
            project_id: project.project_id().to_owned(),
            before: info(project)?,
            after: info(&candidate)?,
            quick: selection,
        };
        // Reuse an intact immutable baseline for metadata-only revisions.
        // Any publication failure leaves project, history and checkpoint intact.
        let recovery = recovery::Recovery::for_metadata(root, &candidate, self.recovery.as_ref())?;
        self.recovery = Some(recovery.clone());
        checkpoint.set_metadata(Some(candidate.metadata()));
        *project = candidate;
        self.observe(project);
        self.quick_typing = typing;
        self.quick_before = selection.map(|q| q.after);
        self.restored_quick_selection = None;
        if merge {
            let previous = self.undo.last_mut().unwrap();
            previous.after = change.after;
            previous.quick.as_mut().unwrap().after = selection.unwrap().after;
        } else {
            self.undo.push(change);
        }
        self.edit_epoch = self.edit_epoch.wrapping_add(1);
        self.redo.clear();
        Ok(Some(recovery))
    }
    pub fn undo(
        &mut self,
        project: &mut Project,
        source: &str,
        checkpoint: &mut session::Checkpoint,
        root: &Path,
    ) -> Result<Option<recovery::Recovery>, String> {
        self.restore(project, source, checkpoint, root, false)
    }
    pub fn redo(
        &mut self,
        project: &mut Project,
        source: &str,
        checkpoint: &mut session::Checkpoint,
        root: &Path,
    ) -> Result<Option<recovery::Recovery>, String> {
        self.restore(project, source, checkpoint, root, true)
    }
    fn restore(
        &mut self,
        project: &mut Project,
        source: &str,
        checkpoint: &mut session::Checkpoint,
        root: &Path,
        redo: bool,
    ) -> Result<Option<recovery::Recovery>, String> {
        self.restored_quick_selection = None;
        if !checkpoint.tracks_metadata(Some(project.metadata())) {
            return Err("The checkpoint does not match the current project. Metadata history was not adopted.".into());
        }
        let Some(change) = (if redo {
            self.redo.last()
        } else {
            self.undo.last()
        }) else {
            return Ok(None);
        };
        let (expected, replacement) = if redo {
            (&change.before, &change.after)
        } else {
            (&change.after, &change.before)
        };
        if project.project_id() != change.project_id || info(project)? != *expected {
            return Err("The project changed. Metadata history was not applied.".into());
        }
        let restored_selection = change.quick.map(|q| if redo { q.after } else { q.before });
        let mut entries = project.entries().clone();
        entries.insert("info.json".into(), Entry::File(replacement.clone()));
        let mut candidate = Project::from_entries(entries)?;
        candidate.replace_source(source)?;
        let recovery = recovery::Recovery::for_metadata(root, &candidate, self.recovery.as_ref())?;
        self.recovery = Some(recovery.clone());
        checkpoint.set_metadata(Some(candidate.metadata()));
        *project = candidate;
        self.adopt_external(project);
        self.restored_quick_selection = restored_selection;
        if redo {
            self.undo.push(self.redo.pop().unwrap());
        } else {
            self.redo.push(self.undo.pop().unwrap());
        }
        Ok(Some(recovery))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fountage_core::{project::NAMESPACE, shelf::Item, writing::Format};
    fn fixture(category: u32) -> Project {
        let mut project = Project::new(Format::from_code(category).unwrap()).unwrap();
        project
            .replace_source("Title: Metadata QA\r\n\r\nZoë 🌙 waits.\r\n")
            .unwrap();
        let mut metadata = project.metadata().clone();
        metadata["foreign"] = serde_json::json!({"keep":[true,null,18446744073709551615u64]});
        metadata[NAMESPACE]["shelf"] = serde_json::json!({"version":1,"future":{"keep":true},"order":["list","future"],"items":{"list":{"kind":"checklist","title":"Plan","content":"[ ] Review Zoë 🌙 [[keep]]\r\n","pinned":false,"future":7},"future":{"kind":"future-tool","keep":"opaque"}}});
        let mut entries = project.entries().clone();
        entries.insert(
            "info.json".into(),
            Entry::File(serde_json::to_vec_pretty(&metadata).unwrap()),
        );
        entries.insert("asset.bin".into(), Entry::File(vec![0, 255, 1]));
        entries.insert("link".into(), Entry::SymbolicLink("../unfollowed".into()));
        Project::from_entries(entries).unwrap()
    }
    fn item(project: &Project) -> Item {
        fountage_core::shelf::items(project.metadata())
            .unwrap()
            .remove(0)
    }
    fn checkpoint(project: &Project) -> session::Checkpoint {
        let mut saved = session::Checkpoint::saved(project.source());
        saved.adopt_saved(project.source(), Some(project.metadata()));
        saved
    }
    fn assert_recovery(recovery: recovery::Recovery, project: &Project) {
        let path = recovery.path().to_owned();
        drop(recovery);
        let snapshot = recovery::read(&path).unwrap();
        let restored = recovery::project_from_snapshot(&path, &snapshot)
            .unwrap()
            .unwrap();
        assert_eq!(restored.entries(), project.entries());
    }
    #[test]
    fn metadata_history_recovery_and_save_baselines_preserve_live_source_in_all_categories() {
        let directory = tempfile::tempdir().unwrap();
        for category in 0..8 {
            let mut project = fixture(category);
            let baseline = project.clone();
            let source = project.source().to_owned();
            let original = directory
                .path()
                .join(format!("original-{category}.fountage"));
            project.save_copy(&original).unwrap();
            let mut saved = checkpoint(&project);
            let mut history = History::default();
            let op = Operation::ToggleTask {
                expected: item(&project),
                start: 0,
            };
            let metadata = project.metadata().clone();
            let recovery = history
                .edit(
                    &mut project,
                    &source,
                    &mut saved,
                    &metadata,
                    &op,
                    directory.path(),
                )
                .unwrap()
                .unwrap();
            assert_eq!(project.source(), source);
            assert!(!saved.is_clean(&source));
            assert_eq!(history.availability(&project), (true, false));
            assert_recovery(recovery, &project);
            let copy = directory.path().join(format!("saved-{category}.fountage"));
            project.save_copy(&copy).unwrap();
            saved.mark_saved(&source);
            assert!(saved.is_clean(&source));
            let typed = format!("{source}Unrelated new manuscript text 世界\r\n");
            let recovery = history
                .undo(&mut project, &typed, &mut saved, directory.path())
                .unwrap()
                .unwrap();
            assert_eq!(project.source(), typed);
            assert_eq!(info(&project).unwrap(), info(&baseline).unwrap());
            assert!(!saved.is_clean(&typed));
            assert_recovery(recovery, &project);
            let recovery = history
                .redo(&mut project, &typed, &mut saved, directory.path())
                .unwrap()
                .unwrap();
            assert_eq!(project.source(), typed);
            assert!(!saved.is_clean(&typed));
            assert_recovery(recovery, &project);
            saved.mark_saved(&typed);
            assert!(saved.is_clean(&typed));
            history
                .undo(&mut project, &typed, &mut saved, directory.path())
                .unwrap();
            assert!(!saved.is_clean(&typed), "Undo after save must be dirty");
            history
                .redo(&mut project, &typed, &mut saved, directory.path())
                .unwrap();
            assert!(
                saved.is_clean(&typed),
                "Redo to saved metadata must be clean"
            );
            history
                .undo(&mut project, &typed, &mut saved, directory.path())
                .unwrap();
            let metadata = project.metadata().clone();
            let op = Operation::AppendTask {
                expected: item(&project),
                text: "Send draft".into(),
            };
            history
                .edit(
                    &mut project,
                    &typed,
                    &mut saved,
                    &metadata,
                    &op,
                    directory.path(),
                )
                .unwrap();
            assert_eq!(history.availability(&project), (true, false));
            assert!(
                history
                    .redo(&mut project, &typed, &mut saved, directory.path())
                    .unwrap()
                    .is_none()
            );
            assert_eq!(
                Project::read(&original).unwrap().entries(),
                baseline.entries()
            );
            assert_eq!(Project::read(&copy).unwrap().source(), source);
            if let Some(output) = std::env::var_os("FOUNTAGE_METADATA_FIXTURES") {
                let output = std::path::PathBuf::from(output);
                std::fs::create_dir_all(&output).unwrap();
                baseline
                    .save_copy(&output.join(format!("Original {category}.fountage")))
                    .unwrap();
                Project::read(&copy)
                    .unwrap()
                    .save_copy(&output.join(format!("Saved {category}.fountage")))
                    .unwrap();
                let recovery = recovery::Recovery::for_project(
                    &output.join(format!("Recovery {category}")),
                    &project,
                )
                .unwrap();
                let snapshot = recovery::read(recovery.path()).unwrap();
                assert_eq!(
                    recovery::project_from_snapshot(recovery.path(), &snapshot)
                        .unwrap()
                        .unwrap()
                        .entries(),
                    project.entries()
                );
            }
        }
    }
    #[test]
    fn failures_stale_projects_and_invalid_source_never_adopt_or_consume_history() {
        let directory = tempfile::tempdir().unwrap();
        let blocked = directory.path().join("blocked");
        std::fs::write(&blocked, b"not a folder").unwrap();
        let mut project = fixture(0);
        let baseline = project.clone();
        let source = project.source().to_owned();
        let mut saved = checkpoint(&project);
        let checkpoint_before = saved.clone();
        let mut history = History::default();
        let metadata = project.metadata().clone();
        let op = Operation::ToggleTask {
            expected: item(&project),
            start: 0,
        };
        assert!(
            history
                .edit(&mut project, &source, &mut saved, &metadata, &op, &blocked)
                .is_err()
        );
        assert_eq!(project.entries(), baseline.entries());
        assert_eq!(saved, checkpoint_before);
        assert_eq!(history.availability(&project), (false, false));
        assert!(
            history
                .edit(
                    &mut project,
                    "bad\0source",
                    &mut saved,
                    &metadata,
                    &op,
                    directory.path()
                )
                .is_err()
        );
        assert_eq!(project.entries(), baseline.entries());
        history
            .edit(
                &mut project,
                &source,
                &mut saved,
                &metadata,
                &op,
                directory.path(),
            )
            .unwrap();
        let changed = project.clone();
        let checkpoint_changed = saved.clone();
        let count = std::fs::read_dir(directory.path()).unwrap().count();
        let metadata = project.metadata().clone();
        let same = item(&project);
        assert!(
            history
                .edit(
                    &mut project,
                    &source,
                    &mut saved,
                    &metadata,
                    &Operation::Update {
                        expected: same.clone(),
                        item: same
                    },
                    directory.path()
                )
                .unwrap()
                .is_none()
        );
        assert_eq!(
            std::fs::read_dir(directory.path()).unwrap().count(),
            count,
            "no-op must not publish recovery"
        );
        assert_eq!(project.entries(), changed.entries());
        assert_eq!(saved, checkpoint_changed);

        assert!(
            history
                .undo(&mut project, &source, &mut saved, &blocked)
                .is_err()
        );
        assert_eq!(project.entries(), changed.entries());
        assert_eq!(saved, checkpoint_changed);
        assert_eq!(history.availability(&project), (true, false));
        history
            .undo(&mut project, &source, &mut saved, directory.path())
            .unwrap();
        let before_redo = project.clone();
        let checkpoint_redo = saved.clone();
        assert!(
            history
                .redo(&mut project, &source, &mut saved, &blocked)
                .is_err()
        );
        assert_eq!(project.entries(), before_redo.entries());
        assert_eq!(saved, checkpoint_redo);
        assert_eq!(history.availability(&project), (false, true));
        let mut other = fixture(0);
        let mut other_saved = checkpoint(&other);
        let other_before = other.clone();
        assert_eq!(history.availability(&other), (false, false));
        assert!(
            history
                .redo(&mut other, &source, &mut other_saved, directory.path())
                .is_err()
        );
        assert_eq!(other.entries(), other_before.entries());
        let other_metadata = other.metadata().clone();
        let op = Operation::ToggleTask {
            expected: item(&other),
            start: 0,
        };
        assert!(
            history
                .edit(
                    &mut other,
                    &source,
                    &mut other_saved,
                    &other_metadata,
                    &op,
                    directory.path()
                )
                .is_err()
        );
        history.reset();
        assert!(
            history
                .edit(
                    &mut other,
                    &source,
                    &mut other_saved,
                    &other_metadata,
                    &op,
                    directory.path()
                )
                .unwrap()
                .is_some()
        );
        let saved_snapshot = other_saved.clone();
        other_saved.set_metadata(None);
        assert!(
            history
                .undo(&mut other, &source, &mut other_saved, directory.path())
                .is_err()
        );
        other_saved = saved_snapshot;
        let metadata = other.metadata().clone();
        let mut external = item(&other);
        external.title = "External metadata change".into();
        other
            .edit_shelf(
                &metadata,
                &Operation::Update {
                    expected: item(&other),
                    item: external,
                },
            )
            .unwrap();
        other_saved.set_metadata(Some(other.metadata()));
        let external_state = other.clone();
        assert!(!history.availability(&other).0);
        assert!(
            history
                .undo(&mut other, &source, &mut other_saved, directory.path())
                .is_err()
        );
        assert_eq!(other.entries(), external_state.entries());
        assert_eq!(std::fs::read(blocked).unwrap(), b"not a folder");
    }
}

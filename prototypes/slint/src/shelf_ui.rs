//! Project-only Shelf item controls; all mutations publish recovery before adoption.
use crate::{
    DraftSession, ShelfRow, Studio, navigation, project_metadata, reflect_checkpoint,
    update_recovery,
};
use fountage_core::{
    project::Project,
    shelf::{self, Item, Kind, Operation},
    writing::{self, Format},
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    path::{Path, PathBuf},
    rc::Rc,
};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Default)]
struct View {
    metadata: Option<serde_json::Value>,
    project_id: String,
    actions: Vec<Option<(Item, usize)>>,
    selection: Option<(String, i32, i32, bool, bool)>,
}
struct Draft {
    metadata: serde_json::Value,
    original: Option<Item>,
    task: bool,
    task_start: Option<usize>,
    item: Item,
    fields: [crate::history::History; 2],
}
#[derive(Clone, Copy, PartialEq)]
enum EditorKind {
    Checklist,
    AppendTask,
    EditTask,
}
#[derive(Clone, Copy)]
enum HistoryTarget {
    Quick,
    Metadata,
    Query,
    Field(usize),
    Composing,
}
enum PendingAction {
    QuickCapture(String),
    History(HistoryTarget, bool),
    Focus {
        controls: Option<(i32, i32)>,
        backward: bool,
    },
    Apply,
    ReviewRemoval,
    Clipboard(usize, i32),
}
struct PendingCommand {
    session: i32,
    project_id: String,
    draft: bool,
    action: PendingAction,
}
struct DrainGuard<'a>(&'a Cell<bool>);
impl Drop for DrainGuard<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}
struct Controller {
    session: DraftSession,
    history: RefCell<project_metadata::History>,
    query_history: RefCell<crate::history::History>,
    view: RefCell<View>,
    draft: RefCell<Option<Draft>>,
    pending: RefCell<VecDeque<PendingCommand>>,
    draining: Cell<bool>,
    writing_quick: Cell<bool>,
    root: Box<dyn Fn() -> Result<PathBuf, String>>,
}
type ShelfContents = (Vec<ShelfRow>, Vec<Option<(Item, usize)>>, String);
fn selected_kind(choice: i32) -> Kind {
    match choice {
        1 => Kind::Note,
        2 => Kind::Snippet,
        _ => Kind::Checklist,
    }
}
fn kind_label(choice: i32) -> &'static str {
    match choice {
        1 => "note",
        2 => "snippet",
        _ => "checklist",
    }
}
fn rows(project: &Project, query: &str, choice: i32) -> Result<ShelfContents, String> {
    let items = shelf::items(project.metadata()).map_err(str::to_owned)?;
    let mut rows = Vec::new();
    let mut actions = Vec::new();
    let (mut lists, mut open) = (0, 0);
    let checklists: Vec<_> = items
        .into_iter()
        .filter(|i| i.kind == selected_kind(choice))
        .collect();
    let total = checklists.len();
    if !shelf::visible(project.metadata(), selected_kind(choice)).map_err(str::to_owned)? {
        return Ok((
            Vec::new(),
            Vec::new(),
            format!("{}s hidden · {total} stored", kind_label(choice)),
        ));
    }
    let query = query.trim().to_lowercase();
    let filtered = !query.is_empty();
    let checklists: Vec<_> = checklists
        .into_iter()
        .filter(|item| {
            !filtered
                || item.title.to_lowercase().contains(&query)
                || item.content.to_lowercase().contains(&query)
        })
        .collect();
    let has_pins = checklists.iter().any(|i| i.pinned);
    for (position, item) in checklists.iter().enumerate() {
        lists += 1;
        rows.push(ShelfRow {
            item_id: item.id.clone().into(),
            title: item.title.clone().into(),
            pinned: item.pinned,
            can_move_up: !filtered
                && position > 0
                && checklists[position - 1].pinned == item.pinned,
            can_move_down: !filtered
                && position + 1 < checklists.len()
                && checklists[position + 1].pinned == item.pinned,
            group_label: if has_pins && position == 0 {
                "PINNED"
            } else if has_pins && !item.pinned && position > 0 && checklists[position - 1].pinned {
                match choice {
                    1 => "NOTES",
                    2 => "SNIPPETS",
                    _ => "CHECKLISTS",
                }
            } else {
                ""
            }
            .into(),
            ..Default::default()
        });
        actions.push(None);
        if item.kind != Kind::Checklist {
            rows.push(ShelfRow {
                text: if item.content.is_empty() {
                    "No text yet.".into()
                } else {
                    item.content.clone().into()
                },
                ..Default::default()
            });
            actions.push(None);
            continue;
        }
        let tasks = writing::analyze(&item.content, Format::General).todos;
        let ranges = navigation::line_ranges(&item.content);
        if tasks.is_empty() {
            rows.push(ShelfRow {
                text: "No tasks in this checklist.".into(),
                ..Default::default()
            });
            actions.push(None);
        }
        let count = tasks.len();
        for (position, task) in tasks.into_iter().enumerate() {
            if !task.completed {
                open += 1;
            }
            let start = ranges
                .get(task.line)
                .ok_or("The checklist changed. Reopen the Shelf.")?
                .start;
            rows.push(ShelfRow {
                text: if task.text.is_empty() {
                    "Untitled task".into()
                } else {
                    task.text.into()
                },
                task: true,
                can_move_up: position > 0,
                can_move_down: position + 1 < count,
                completed: task.completed,
                ..Default::default()
            });
            actions.push(Some((item.clone(), start)));
        }
    }
    Ok((
        rows,
        actions,
        if choice == 0 {
            format!(
                "{} · {open} open {}",
                if filtered {
                    format!("{lists} of {total} checklists")
                } else {
                    format!(
                        "{lists} {}",
                        if lists == 1 {
                            "checklist"
                        } else {
                            "checklists"
                        }
                    )
                },
                if open == 1 { "task" } else { "tasks" }
            )
        } else if filtered {
            format!("{lists} of {total} {}s", kind_label(choice))
        } else {
            format!(
                "{lists} {}{}",
                kind_label(choice),
                if lists == 1 { "" } else { "s" }
            )
        },
    ))
}
impl Controller {
    fn history_target(ui: &Studio) -> Option<HistoryTarget> {
        if !ui.get_shelf_open() || !ui.get_file_dialog_open() || ui.get_confirm_close() {
            return None;
        }
        if ui.get_shelf_quick_focused() {
            return Some(if ui.get_shelf_quick_composing() {
                HistoryTarget::Composing
            } else {
                HistoryTarget::Quick
            });
        }
        if ui.get_shelf_query_focused() {
            return Some(if ui.get_shelf_query_composing() {
                HistoryTarget::Composing
            } else {
                HistoryTarget::Query
            });
        }
        if !ui.get_shelf_draft_open() {
            return Some(HistoryTarget::Metadata);
        }
        if ui.get_shelf_draft_composing() {
            return Some(HistoryTarget::Composing);
        }
        usize::try_from(ui.get_shelf_draft_field_index())
            .ok()
            .filter(|index| *index < 2 && (*index == 0 || !ui.get_shelf_draft_task()))
            .map(HistoryTarget::Field)
    }
    // Native admission reads state and records intent only. No UI writes,
    // disk operations, parsing or history execution inside AppKit's monitor.
    fn enqueue(&self, ui: &Studio, action: PendingAction) -> bool {
        if !ui.get_shelf_open() || !ui.get_file_dialog_open() || ui.get_confirm_close() {
            return false;
        }
        let Some(project_id) = self
            .session
            .project
            .borrow()
            .as_ref()
            .map(|p| p.project_id().to_owned())
        else {
            return false;
        };
        self.pending.borrow_mut().push_back(PendingCommand {
            session: ui.get_shelf_session(),
            project_id,
            draft: ui.get_shelf_draft_open(),
            action,
        });
        true
    }
    fn logical_control(&self, ui: &Studio) -> i32 {
        // Earlier admitted Tabs may not have reached Slint yet. Later commands
        // target their resulting control, not the still-visible old field.
        let queued = {
            let pending = self.pending.borrow();
            pending.iter().rev().find_map(|command| {
                if command.session == ui.get_shelf_session()
                    && command.draft
                    && let PendingAction::Focus {
                        controls: Some((_, next)),
                        ..
                    } = command.action
                {
                    return Some(next);
                }
                None
            })
        };
        queued.unwrap_or_else(|| ui.get_shelf_draft_control_index())
    }
    fn queue_history(&self, ui: &Studio, redo: bool) -> bool {
        let target = if ui.get_shelf_draft_open() && !ui.get_shelf_draft_composing() {
            match self.logical_control(ui) {
                0 => Some(HistoryTarget::Field(0)),
                1 if !ui.get_shelf_draft_task() => Some(HistoryTarget::Field(1)),
                _ => None,
            }
        } else {
            Self::history_target(ui)
        };
        target.is_some_and(|target| self.enqueue(ui, PendingAction::History(target, redo)))
    }
    fn queue_focus(&self, ui: &Studio, backward: bool) -> bool {
        if ui.get_shelf_draft_composing()
            || ui.get_shelf_query_composing()
            || ui.get_shelf_quick_composing()
        {
            return self.enqueue(ui, PendingAction::History(HistoryTarget::Composing, false));
        }
        let controls = if ui.get_shelf_draft_open() {
            let previous = self.logical_control(ui);
            let order = if ui.get_shelf_draft_removing() {
                vec![2, 3]
            } else if ui.get_shelf_draft_task_edit() {
                vec![0, 4, 2, 3]
            } else if ui.get_shelf_draft_task() {
                vec![0, 2, 3]
            } else if ui.get_shelf_draft_existing() {
                vec![0, 1, 4, 2, 3]
            } else {
                vec![0, 1, 2, 3]
            };
            let next = if let Some(index) = order.iter().position(|i| *i == previous) {
                order[if backward {
                    (index + order.len() - 1) % order.len()
                } else {
                    (index + 1) % order.len()
                }]
            } else if backward {
                3
            } else {
                0
            };
            Some((previous, next))
        } else {
            None
        };
        self.enqueue(ui, PendingAction::Focus { controls, backward })
    }
    fn queue_apply(&self, ui: &Studio) -> bool {
        if ui.get_shelf_quick_focused() {
            return self.enqueue(
                ui,
                if ui.get_shelf_quick_composing() {
                    PendingAction::History(HistoryTarget::Composing, false)
                } else {
                    PendingAction::QuickCapture(ui.get_shelf_quick_text().to_string())
                },
            );
        }
        if !ui.get_shelf_draft_open() {
            return false;
        }
        self.enqueue(
            ui,
            if ui.get_shelf_draft_composing() {
                PendingAction::History(HistoryTarget::Composing, false)
            } else if !ui.get_shelf_draft_removing() && self.logical_control(ui) == 4 {
                PendingAction::ReviewRemoval
            } else {
                PendingAction::Apply
            },
        )
    }
    fn queue_clipboard(&self, ui: &Studio, kind: i32) -> bool {
        if ui.get_shelf_quick_focused() && (0..=3).contains(&kind) {
            return self.enqueue(
                ui,
                if ui.get_shelf_quick_composing() && kind != 1 {
                    PendingAction::History(HistoryTarget::Composing, false)
                } else {
                    PendingAction::Clipboard(3, kind)
                },
            );
        }
        if ui.get_shelf_query_focused() && (0..=3).contains(&kind) {
            return if ui.get_shelf_query_composing() && kind != 1 {
                self.enqueue(ui, PendingAction::History(HistoryTarget::Composing, false))
            } else {
                self.enqueue(ui, PendingAction::Clipboard(2, kind))
            };
        }
        if !ui.get_shelf_draft_open() || !(0..=3).contains(&kind) {
            return false;
        }
        if ui.get_shelf_draft_composing() && kind != 1 {
            return self.enqueue(ui, PendingAction::History(HistoryTarget::Composing, false));
        }
        let index = self.logical_control(ui);
        if index != 0 && (index != 1 || ui.get_shelf_draft_task()) {
            return false;
        }
        self.enqueue(ui, PendingAction::Clipboard(index as usize, kind))
    }
    fn flush_history(&self, ui: &Studio) -> bool {
        if self.draining.replace(true) {
            return true;
        }
        let _guard = DrainGuard(&self.draining);
        let mut refocus = true;
        loop {
            let Some(command) = self.pending.borrow_mut().pop_front() else {
                return true;
            };
            let same_project = self
                .session
                .project
                .borrow()
                .as_ref()
                .is_some_and(|p| p.project_id() == command.project_id);
            if !ui.get_shelf_open()
                || !ui.get_file_dialog_open()
                || ui.get_confirm_close()
                || ui.get_shelf_session() != command.session
                || !same_project
                || command.draft != ui.get_shelf_draft_open()
            {
                continue;
            }
            let success = match command.action {
                PendingAction::QuickCapture(expected) => self.quick_capture(ui, Some(&expected)),
                PendingAction::History(target, redo) => {
                    self.execute_history(ui, target, redo, refocus)
                }
                PendingAction::Focus {
                    controls: Some((previous, next)),
                    ..
                } => {
                    if ui.get_shelf_draft_control_index() == previous {
                        ui.invoke_focus_shelf_draft_control(next);
                    } else {
                        refocus = false;
                    } // Preserve a later pointer/focus action.
                    true
                }
                PendingAction::Focus {
                    controls: None,
                    backward,
                } => {
                    self.focus_move(ui, backward);
                    true
                }
                PendingAction::ReviewRemoval => {
                    self.request_removal(ui);
                    true
                }
                PendingAction::Apply => {
                    self.draft_action(ui, true);
                    !ui.get_shelf_draft_open()
                }
                PendingAction::Clipboard(index, kind) => {
                    if index == 3 {
                        ui.invoke_shelf_quick_clipboard(kind);
                        kind == 1 || kind == 3 || !ui.get_shelf_quick_pending()
                    } else if index == 2 {
                        ui.invoke_shelf_query_clipboard(kind);
                        true
                    } else if ui.get_shelf_draft_composing() && kind != 1 {
                        self.blocked(ui);
                        false
                    } else {
                        ui.invoke_shelf_field_clipboard(index as i32, kind);
                        true
                    }
                }
            };
            if !success {
                self.pending.borrow_mut().clear();
                let error = ui.get_shelf_error();
                ui.set_shelf_error(
                    format!(
                        "{error} Pending shortcuts stopped; later queued commands were not applied."
                    )
                    .into(),
                );
                return false;
            }
        }
    }
    fn execute_history(
        &self,
        ui: &Studio,
        target: HistoryTarget,
        redo: bool,
        refocus: bool,
    ) -> bool {
        if self.quick_blocked(ui) {
            return false;
        }
        match target {
            HistoryTarget::Quick => {
                let success = self.execute_history(ui, HistoryTarget::Metadata, redo, false);
                if success && refocus && ui.get_shelf_quick_visible() {
                    let selection = self.history.borrow().restored_quick_selection();
                    ui.invoke_focus_shelf_quick();
                    if let Some(selection) = selection {
                        ui.invoke_select_shelf_quick(
                            selection.anchor as i32,
                            selection.focus as i32,
                        );
                    }
                }
                success
            }
            HistoryTarget::Query => {
                if ui.get_shelf_query_composing() {
                    self.blocked(ui);
                    return false;
                }
                let refocus = refocus && ui.get_shelf_query_focused();
                let value = ui.get_shelf_query();
                let step = self.query_history.borrow_mut().step(value.as_str(), redo);
                if let Some((text, anchor, focus)) = step {
                    ui.set_shelf_query(text.into());
                    self.refresh(ui);
                    if refocus {
                        ui.invoke_focus_shelf_search();
                        ui.invoke_select_shelf_search(anchor as i32, focus as i32);
                        // The native backend may release focus while destroying old
                        // result rows after this callback. Restore only this unchanged
                        // search scope; a subsequent click/Tab changes focus-position.
                        let weak = ui.as_weak();
                        let session = ui.get_shelf_session();
                        let query = ui.get_shelf_query();
                        slint::Timer::single_shot(std::time::Duration::from_millis(1), move || {
                            if let Some(ui) = weak.upgrade()
                                && ui.get_shelf_open()
                                && !ui.get_shelf_draft_open()
                                && ui.get_shelf_session() == session
                                && ui.get_shelf_focus_position() == -5
                                && ui.get_shelf_query() == query
                            {
                                ui.invoke_select_shelf_search(anchor as i32, focus as i32);
                            }
                        });
                    }
                }
                true
            }
            HistoryTarget::Composing => {
                self.blocked(ui);
                false
            }
            HistoryTarget::Field(index) => self.field_history_at(ui, index, redo, refocus),
            HistoryTarget::Metadata => {
                let labels = |ui: &Studio| {
                    ui.get_shelf_rows()
                        .iter()
                        .map(|row| (row.title, row.text))
                        .collect::<Vec<_>>()
                };
                let before = labels(ui);
                let available = self
                    .session
                    .project
                    .borrow()
                    .as_ref()
                    .map(|p| self.history.borrow().availability(p))
                    .unwrap_or_default();
                if !(if redo { available.1 } else { available.0 }) && self.project_history(ui, redo)
                {
                    return ui.get_shelf_error().is_empty();
                }
                let success = self.mutate(ui, |history, project, checkpoint, root| {
                    if redo {
                        history.redo(project, ui.get_source().as_str(), checkpoint, root)
                    } else {
                        history.undo(project, ui.get_source().as_str(), checkpoint, root)
                    }
                });
                // History can replace/reorder the task under the focused control.
                // Preserve the prior Done-focus behavior for changed labels/order.
                if success && labels(ui) != before {
                    ui.invoke_finish_shelf_draft(-1);
                }
                success
            }
        }
    }
    fn reflect_metadata_status(&self, ui: &Studio, project: &Project) {
        {
            let mut view = self.view.borrow_mut();
            view.metadata = Some(project.metadata().clone());
            view.project_id = project.project_id().to_owned();
        }
        if !ui.get_shelf_quick_pending() {
            ui.set_shelf_quick_text(shelf::quick_text(project.metadata()).unwrap_or("").into());
        }
        ui.set_shelf_quick_visible(shelf::quick_visible(project.metadata()).unwrap_or(true));
        let available = self.history.borrow().availability(project);
        ui.set_shelf_can_undo(
            available.0
                || self
                    .session
                    .history
                    .borrow()
                    .project_change(ui.get_source().as_str(), false)
                    .is_some(),
        );
        ui.set_shelf_can_redo(
            available.1
                || self
                    .session
                    .history
                    .borrow()
                    .project_change(ui.get_source().as_str(), true)
                    .is_some(),
        );
        if (ui.get_shelf_focus_position() == -2 && !available.0)
            || (ui.get_shelf_focus_position() == -3 && !available.1)
        {
            ui.set_shelf_focus_position(-1);
        }
    }
    fn refresh_quick_status(&self, ui: &Studio) {
        if let Some(project) = self.session.project.borrow().as_ref() {
            self.reflect_metadata_status(ui, project);
        }
    }
    fn refresh(&self, ui: &Studio) {
        let project = self.session.project.borrow();
        let Some(project) = project.as_ref() else {
            return;
        };
        self.reflect_metadata_status(ui, project);
        let query_availability = self
            .query_history
            .borrow()
            .availability(ui.get_shelf_query().as_str());
        ui.set_shelf_query_can_undo(query_availability.0);
        ui.set_shelf_query_can_redo(query_availability.1);
        match rows(project, ui.get_shelf_query().as_str(), ui.get_shelf_kind()) {
            Ok((mut rows, actions, status)) => {
                let selection = self.view.borrow().selection.clone();
                ui.set_shelf_replace_selection(
                    selection.as_ref().is_some_and(|(_, a, f, _, _)| a != f),
                );
                let items = shelf::items(project.metadata()).unwrap_or_default();
                let valid_selection = !ui.get_preview_open()
                    && selection.as_ref().is_some_and(|(source, a, f, _, _)| {
                        usize::try_from(*a)
                            .ok()
                            .zip(usize::try_from(*f).ok())
                            .is_some_and(|(a, f)| {
                                shelf::selection_range(ui.get_source().as_str(), source, a, f)
                                    .is_ok()
                            })
                    });
                ui.set_shelf_can_capture(
                    valid_selection && selection.as_ref().is_some_and(|(_, a, f, _, _)| a != f),
                );
                for row in &mut rows {
                    row.can_insert = valid_selection
                        && items
                            .iter()
                            .find(|i| i.id == row.item_id.as_str())
                            .is_some_and(|item| {
                                !item.content.is_empty() && !item.content.contains('\0')
                            });
                }
                let model = ui.get_shelf_rows();
                let same_shape = model.row_count() == rows.len()
                    && rows.iter().enumerate().all(|(index, row)| {
                        model.row_data(index).is_some_and(|old| {
                            old.item_id == row.item_id
                                && old.task == row.task
                                && old.title.is_empty() == row.title.is_empty()
                                && old.text.is_empty() == row.text.is_empty()
                        })
                    });
                if same_shape {
                    // A changed task label/order does not change the row's control
                    // structure. Retain components: destroying the Button delivering
                    // Return can clear the replacement row's newly requested focus.
                    for (index, row) in rows.into_iter().enumerate() {
                        model.set_row_data(index, row);
                    }
                } else {
                    ui.set_shelf_rows(ModelRc::new(VecModel::from(rows)));
                    if ui.get_shelf_open() && !ui.get_shelf_draft_open() {
                        ui.set_shelf_focus_position(-1);
                    }
                }
                let mut view = self.view.borrow_mut();
                view.metadata = Some(project.metadata().clone());
                view.project_id = project.project_id().to_owned();
                view.actions = actions;
                ui.set_shelf_kind_visible(
                    shelf::visible(project.metadata(), selected_kind(ui.get_shelf_kind()))
                        .unwrap_or(true),
                );
                ui.set_shelf_editable(true);
                ui.set_shelf_status(status.into());
            }
            Err(error) => {
                ui.set_shelf_rows(ModelRc::new(VecModel::default()));
                ui.set_shelf_editable(false);
                ui.set_shelf_can_capture(false);
                ui.set_shelf_status("Shelf data kept unchanged".into());
                ui.set_shelf_error(error.into());
                self.view.borrow_mut().actions.clear();
            }
        }
    }
    fn open(&self, ui: &Studio) {
        if !ui.get_project_mode()
            || !ui.get_can_toggle_preview()
            || self.session.project.borrow().is_none()
        {
            return;
        }
        ui.set_shelf_session(ui.get_shelf_session().wrapping_add(1));
        self.view.borrow_mut().selection = Some((
            ui.get_source().to_string(),
            ui.get_source_anchor(),
            ui.get_source_focus(),
            ui.get_formatted_anchor_upstream(),
            ui.get_formatted_focus_upstream(),
        ));
        ui.set_shelf_error("".into());
        self.refresh(ui);
        ui.set_file_dialog_open(true);
        ui.set_shelf_open(true);
    }
    fn owns(&self, ui: &Studio, project: &Project) -> bool {
        let view = self.view.borrow();
        ui.get_shelf_open()
            && ui.get_file_dialog_open()
            && !ui.get_confirm_close()
            && view.project_id == project.project_id()
            && view.metadata.as_ref() == Some(project.metadata())
    }
    fn mutate(
        &self,
        ui: &Studio,
        edit: impl FnOnce(
            &mut project_metadata::History,
            &mut Project,
            &mut crate::session::Checkpoint,
            &Path,
        ) -> Result<Option<crate::recovery::Recovery>, String>,
    ) -> bool {
        if !ui.get_shelf_open() || !ui.get_file_dialog_open() || ui.get_confirm_close() {
            return false;
        }
        if !self.writing_quick.get() && self.quick_blocked(ui) {
            return false;
        }
        let edit_epoch = self.history.borrow().edit_epoch();
        let result = (|| {
            let mut project = self.session.project.borrow_mut();
            let project = project
                .as_mut()
                .ok_or("Open a project to edit its Shelf.")?;
            if !self.owns(ui, project) {
                return Err("The project changed. Reopen the Shelf before editing.".into());
            }
            self.history
                .borrow_mut()
                .attach_recovery(self.session.recovery.borrow().as_ref().ok());
            edit(
                &mut self.history.borrow_mut(),
                project,
                &mut self.session.checkpoint.borrow_mut(),
                &(self.root)()?,
            )
        })();
        let success = result.is_ok();
        match result {
            Ok(Some(recovery)) => {
                if self.history.borrow().edit_epoch() != edit_epoch {
                    self.session.history.borrow_mut().discard_redo();
                    crate::reflect_history(ui, &self.session.history.borrow());
                }
                *self.session.recovery.borrow_mut() = Ok(recovery);
                reflect_checkpoint(ui, &self.session.checkpoint.borrow());
                update_recovery(ui, &self.session.recovery.borrow());
                ui.set_shelf_error("".into());
                ui.set_status("Shelf updated · Save a Copy to keep project changes".into());
            }
            Ok(None) => ui.set_shelf_error("".into()),
            Err(error) => ui.set_shelf_error(error.into()),
        }
        if self.writing_quick.get() {
            self.refresh_quick_status(ui);
        } else {
            self.refresh(ui);
        }
        success
    }
    fn focus_move(&self, ui: &Studio, backward: bool) {
        if !ui.get_shelf_open() || !ui.get_file_dialog_open() {
            return;
        }
        if !self.flush_history(ui) {
            return;
        }
        if ui.get_shelf_query_composing() || ui.get_shelf_quick_composing() {
            self.blocked(ui);
            return;
        }
        if ui.get_shelf_draft_open() {
            ui.invoke_move_shelf_draft_focus(backward);
            return;
        }
        let rows = ui.get_shelf_rows();
        let mut targets = vec![-17, -7, -8, -9];
        if ui.get_shelf_editable() {
            targets.push(-4);
        }
        if ui.get_shelf_can_capture() {
            targets.extend([-10, -11]);
        }
        if ui.get_shelf_editable() {
            targets.extend([-12, -13, -14]);
        }
        targets.push(-5);
        if !ui.get_shelf_query().is_empty() {
            targets.push(-6);
        }
        if ui.get_shelf_quick_visible() {
            targets.push(-15);
            if !ui.get_shelf_quick_text().is_empty() || ui.get_shelf_quick_pending() {
                targets.push(-16);
            }
        }
        if ui.get_shelf_editable() {
            for index in 0..rows.row_count() {
                if let Some(row) = rows.row_data(index) {
                    if !row.item_id.is_empty() {
                        targets.push((5 * rows.row_count() + index) as i32);
                        if row.can_move_up {
                            targets.push((6 * rows.row_count() + index) as i32);
                        }
                        if row.can_move_down {
                            targets.push((7 * rows.row_count() + index) as i32);
                        }
                        targets.push(index as i32);
                        if row.can_insert {
                            targets.push((8 * rows.row_count() + index) as i32);
                        }
                    } else if row.task {
                        targets.push(index as i32);
                    }
                    if row.task {
                        if row.can_move_up {
                            targets.push((3 * rows.row_count() + index) as i32);
                        }
                        if row.can_move_down {
                            targets.push((4 * rows.row_count() + index) as i32);
                        }
                        targets.push((2 * rows.row_count() + index) as i32);
                    }
                    if !row.item_id.is_empty() && ui.get_shelf_kind() == 0 {
                        targets.push((rows.row_count() + index) as i32);
                    }
                }
            }
        }
        if ui.get_shelf_can_undo() {
            targets.push(-2);
        }
        if ui.get_shelf_can_redo() {
            targets.push(-3);
        }
        targets.push(-1);
        let position = if ui.get_shelf_quick_focused() {
            -15
        } else if ui.get_shelf_query_focused() {
            -5
        } else {
            ui.get_shelf_focus_position()
        };
        let current = targets
            .iter()
            .position(|index| *index == position)
            .unwrap_or(targets.len() - 1);
        let next = if backward {
            (current + targets.len() - 1) % targets.len()
        } else {
            (current + 1) % targets.len()
        };
        ui.set_shelf_focus_position(targets[next]);
    }
    fn toggle(&self, ui: &Studio, index: i32) {
        if ui.get_shelf_draft_open() {
            return;
        }
        let action = usize::try_from(index)
            .ok()
            .and_then(|i| self.view.borrow().actions.get(i).cloned().flatten());
        let Some((item, start)) = action else {
            return;
        };
        if !self.flush_history(ui) {
            return;
        }
        ui.set_shelf_focus_position(index);
        let Some(expected) = self.view.borrow().metadata.clone() else {
            return;
        };
        self.mutate(ui, |history, project, checkpoint, root| {
            history.edit(
                project,
                ui.get_source().as_str(),
                checkpoint,
                &expected,
                &Operation::ToggleTask {
                    expected: item,
                    start,
                },
                root,
            )
        });
    }
    fn reflect_fields(&self, ui: &Studio) {
        if !ui.get_shelf_draft_open() {
            return;
        }
        let available = self
            .draft
            .borrow()
            .as_ref()
            .and_then(|draft| {
                usize::try_from(ui.get_shelf_draft_field_index())
                    .ok()
                    .filter(|i| *i < 2)
                    .map(|i| {
                        draft.fields[i].availability(
                            if i == 0 {
                                ui.get_shelf_draft_title()
                            } else {
                                ui.get_shelf_draft_content()
                            }
                            .as_str(),
                        )
                    })
            })
            .unwrap_or((false, false));
        ui.set_shelf_can_undo(available.0);
        ui.set_shelf_can_redo(available.1);
    }
    fn project_history(&self, ui: &Studio, redo: bool) -> bool {
        if self
            .session
            .history
            .borrow()
            .project_change(ui.get_source().as_str(), redo)
            .is_none()
        {
            return false;
        }
        if ui.get_confirm_close()
            || (ui.get_file_dialog_open() && (!ui.get_shelf_open() || ui.get_shelf_draft_open()))
        {
            return true;
        }
        let result = (|| {
            let mut history = self.session.history.borrow().clone();
            let (source, selection, frame) = history
                .step_project(ui.get_source().as_str(), redo)
                .ok_or("The source history changed.")?;
            let (expected, replacement) = if redo {
                (&frame.before, &frame.after)
            } else {
                (&frame.after, &frame.before)
            };
            let project = self.session.project.borrow();
            let project = project
                .as_ref()
                .ok_or("Open the original project before restoring this Shelf capture.")?;
            if project.project_id() != frame.project_id
                || project_metadata::info(project)? != *expected
                || !self
                    .session
                    .checkpoint
                    .borrow()
                    .tracks_metadata(Some(project.metadata()))
            {
                return Err("Undo the later Shelf changes before restoring this capture. Nothing was changed.".into());
            }
            let mut entries = project.entries().clone();
            entries.insert(
                "info.json".into(),
                fountage_core::project::Entry::File(replacement.clone()),
            );
            let mut candidate = Project::from_entries(entries)?;
            candidate.replace_source(&source)?;
            let recovery = crate::recovery::Recovery::for_project(&(self.root)()?, &candidate)?;
            Ok::<_, String>((history, source, selection, candidate, recovery))
        })();
        match result {
            Err(error) => {
                ui.set_shelf_error(error.clone().into());
                ui.set_status(error.into());
            }
            Ok((history, source, selection, candidate, recovery)) => {
                self.session
                    .checkpoint
                    .borrow_mut()
                    .set_metadata(Some(candidate.metadata()));
                self.history.borrow_mut().adopt_external(&candidate);
                *self.session.project.borrow_mut() = Some(candidate);
                *self.session.recovery.borrow_mut() = Ok(recovery);
                *self.session.history.borrow_mut() = history;
                ui.set_source(source.clone().into());
                // Notify the real source/checkpoint/preview pipeline without adding
                // a second history entry: history already contains the restored source.
                ui.invoke_editor_edited(
                    source.into(),
                    selection.anchor as i32,
                    selection.focus as i32,
                );
                crate::reflect_history(ui, &self.session.history.borrow());
                reflect_checkpoint(ui, &self.session.checkpoint.borrow());
                update_recovery(ui, &self.session.recovery.borrow());
                ui.set_shelf_error("".into());
                if ui.get_shelf_open() {
                    self.refresh(ui);
                    ui.invoke_finish_shelf_list_change();
                } else {
                    ui.invoke_restore_source_selection(
                        selection.anchor as i32,
                        selection.focus as i32,
                        selection.anchor_upstream,
                        selection.focus_upstream,
                    );
                }
            }
        }
        true
    }
    fn capture(&self, ui: &Studio, cut: bool) {
        if self.quick_blocked(ui) {
            return;
        }
        if !ui.get_shelf_open()
            || !ui.get_file_dialog_open()
            || ui.get_shelf_draft_open()
            || ui.get_confirm_close()
            || ui.get_preview_open()
            || !ui.get_shelf_can_capture()
        {
            return;
        }
        let (metadata, selection) = {
            let view = self.view.borrow();
            (view.metadata.clone(), view.selection.clone())
        };
        let (Some(metadata), Some((source, anchor, focus, au, fu))) = (metadata, selection) else {
            return;
        };
        if !self.flush_history(ui) || !ui.get_shelf_open() || ui.get_shelf_draft_open() {
            return;
        }
        let result = (|| {
            let a = usize::try_from(anchor).map_err(|_| "Invalid selection.")?;
            let f = usize::try_from(focus).map_err(|_| "Invalid selection.")?;
            let range = shelf::selection_range(ui.get_source().as_str(), &source, a, f)
                .map_err(str::to_owned)?;
            if range.is_empty() {
                return Err("Select manuscript text before capturing to the Shelf.".into());
            }
            let content = source[range.clone()].to_owned();
            let title = content
                .lines()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("Selected text")
                .trim()
                .graphemes(true)
                .take(60)
                .collect();
            let item = Item::new(Kind::Snippet, title, content);
            let next_source = if cut {
                shelf::insertion(&source, &source, a, f, "")
                    .map_err(str::to_owned)?
                    .source
            } else {
                source.clone()
            };
            let before = crate::history::SelectionState {
                anchor: a,
                focus: f,
                anchor_upstream: au,
                focus_upstream: fu,
            };
            let next = if cut {
                crate::history::SelectionState::plain(range.start, range.start)
            } else {
                before
            };
            let project = self.session.project.borrow();
            let project = project
                .as_ref()
                .ok_or("Open a project before capturing text.")?;
            if !self.owns(ui, project)
                || project.metadata() != &metadata
                || !self
                    .session
                    .checkpoint
                    .borrow()
                    .tracks_metadata(Some(&metadata))
            {
                return Err("The project changed. Reopen the Shelf before capturing text.".into());
            }
            let mut candidate = project.clone();
            candidate.replace_source(&next_source)?;
            candidate.edit_shelf(&metadata, &Operation::Add(item))?;
            let frame = crate::history::ProjectChange {
                project_id: project.project_id().to_owned(),
                before: project_metadata::info(project)?,
                after: project_metadata::info(&candidate)?,
            };
            let mut history = self.session.history.borrow().clone();
            if !history.record_project(&source, &next_source, before, next, frame) {
                return Err("The source history changed. Nothing was captured.".into());
            }
            let recovery = crate::recovery::Recovery::for_project(&(self.root)()?, &candidate)?;
            Ok::<_, String>((candidate, history, recovery, next_source, next))
        })();
        let (candidate, history, recovery, source, selection) = match result {
            Ok(value) => value,
            Err(error) => {
                ui.set_shelf_error(error.into());
                return;
            }
        };
        self.session
            .checkpoint
            .borrow_mut()
            .set_metadata(Some(candidate.metadata()));
        self.history.borrow_mut().adopt_external(&candidate);
        *self.session.project.borrow_mut() = Some(candidate);
        *self.session.recovery.borrow_mut() = Ok(recovery);
        *self.session.history.borrow_mut() = history;
        ui.set_shelf_session(ui.get_shelf_session().wrapping_add(1));
        ui.set_shelf_open(false);
        ui.set_file_dialog_open(false);
        ui.set_source(source.clone().into());
        ui.invoke_editor_edited(
            source.into(),
            selection.anchor as i32,
            selection.focus as i32,
        );
        crate::reflect_history(ui, &self.session.history.borrow());
        reflect_checkpoint(ui, &self.session.checkpoint.borrow());
        update_recovery(ui, &self.session.recovery.borrow());
        ui.invoke_restore_source_selection(
            selection.anchor as i32,
            selection.focus as i32,
            selection.anchor_upstream,
            selection.focus_upstream,
        );
        ui.set_status(
            if cut {
                "Cut selection to Shelf · Undo restores source and removes the snippet"
            } else {
                "Copied selection to Shelf · Undo removes the snippet"
            }
            .into(),
        );
    }
    fn insert(&self, ui: &Studio, index: i32) {
        if self.quick_blocked(ui) {
            return;
        }
        if !ui.get_shelf_open()
            || !ui.get_file_dialog_open()
            || ui.get_shelf_draft_open()
            || ui.get_confirm_close()
            || ui.get_preview_open()
        {
            return;
        }
        let Some(row) = usize::try_from(index)
            .ok()
            .and_then(|i| ui.get_shelf_rows().row_data(i))
        else {
            return;
        };
        if !row.can_insert || row.item_id.is_empty() {
            return;
        }
        let (metadata, selection) = {
            let view = self.view.borrow();
            (view.metadata.clone(), view.selection.clone())
        };
        let (Some(metadata), Some((source, anchor, focus, au, fu))) = (metadata, selection) else {
            return;
        };
        let Some(item) = shelf::items(&metadata)
            .ok()
            .and_then(|items| items.into_iter().find(|i| i.id == row.item_id.as_str()))
        else {
            return;
        };
        if !self.flush_history(ui) || !ui.get_shelf_open() || ui.get_shelf_draft_open() {
            return;
        }
        let result = (|| {
            let project = self.session.project.borrow();
            let project = project
                .as_ref()
                .ok_or("Open a project before inserting from the Shelf.")?;
            if !self.owns(ui, project)
                || project.metadata() != &metadata
                || !self
                    .session
                    .checkpoint
                    .borrow()
                    .tracks_metadata(Some(&metadata))
            {
                return Err("The project changed. Reopen the Shelf before inserting.".into());
            }
            let a = usize::try_from(anchor).map_err(|_| "The manuscript selection is invalid.")?;
            let f = usize::try_from(focus).map_err(|_| "The manuscript selection is invalid.")?;
            let edit = shelf::insertion(ui.get_source().as_str(), &source, a, f, &item.content)
                .map_err(str::to_owned)?;
            if edit.source.len() > i32::MAX as usize {
                return Err("The inserted text is too large for the editor.".into());
            }
            if edit.source == source {
                return Ok((edit, None));
            }
            let mut candidate = project.clone();
            candidate.replace_source(&edit.source)?;
            let recovery = crate::recovery::Recovery::for_project(&(self.root)()?, &candidate)?;
            Ok::<_, String>((edit, Some((candidate, recovery))))
        })();
        let (edit, candidate) = match result {
            Ok(result) => result,
            Err(error) => {
                ui.set_shelf_error(error.into());
                return;
            }
        };
        let changed = candidate.is_some();
        if let Some((candidate, recovery)) = candidate {
            *self.session.project.borrow_mut() = Some(candidate);
            *self.session.recovery.borrow_mut() = Ok(recovery);
            self.session.history.borrow_mut().boundary();
            self.session.history.borrow_mut().observe_visual_selection(
                &source,
                crate::history::SelectionState {
                    anchor: anchor as usize,
                    focus: focus as usize,
                    anchor_upstream: au,
                    focus_upstream: fu,
                },
            );
        }
        self.pending.borrow_mut().clear();
        ui.set_shelf_session(ui.get_shelf_session().wrapping_add(1));
        ui.set_shelf_open(false);
        ui.set_file_dialog_open(false);
        if changed {
            ui.set_source(edit.source.clone().into());
            ui.invoke_editor_edited(edit.source.into(), edit.anchor as i32, edit.focus as i32);
            self.session.history.borrow_mut().boundary();
            reflect_checkpoint(ui, &self.session.checkpoint.borrow());
            update_recovery(ui, &self.session.recovery.borrow());
        }
        ui.invoke_restore_source_selection(edit.anchor as i32, edit.focus as i32, false, false);
        ui.set_status(format!("Inserted from Shelf: {}", item.title).into());
    }
    fn quick_blocked(&self, ui: &Studio) -> bool {
        if ui.get_shelf_quick_pending() {
            ui.set_shelf_error("Quick Note draft is not stored yet. Retry storing it before closing or changing the Shelf. You can copy the text.".into());
            true
        } else if ui.get_shelf_quick_composing() {
            self.blocked(ui);
            true
        } else {
            false
        }
    }
    fn quick_input(&self, ui: &Studio, input: &str) {
        if !ui.get_shelf_quick_focused() || ui.get_shelf_quick_pending() || !self.flush_history(ui)
        {
            return;
        }
        let text = ui.get_shelf_quick_text();
        let selection = crate::history::SelectionState::plain(
            ui.get_shelf_quick_anchor().max(0) as usize,
            ui.get_shelf_quick_focus().max(0) as usize,
        );
        if text.is_char_boundary(selection.anchor) && text.is_char_boundary(selection.focus) {
            self.history
                .borrow_mut()
                .quick_input(text.as_str(), input, selection);
        }
    }
    fn quick_selection(&self, ui: &Studio, text: &str, anchor: i32, focus: i32) {
        if !ui.get_shelf_quick_focused() || anchor < 0 || focus < 0 {
            return;
        }
        let matches = self
            .session
            .project
            .borrow()
            .as_ref()
            .is_some_and(|p| shelf::quick_text(p.metadata()) == Ok(text));
        if matches
            && text.is_char_boundary(anchor as usize)
            && text.is_char_boundary(focus as usize)
        {
            self.history.borrow_mut().quick_selection(
                text,
                crate::history::SelectionState::plain(anchor as usize, focus as usize),
            );
        }
    }
    fn quick_edited(&self, ui: &Studio, value: &str) -> bool {
        if !ui.get_shelf_open()
            || ui.get_shelf_draft_open()
            || !ui.get_shelf_editable()
            || ui.get_shelf_quick_composing()
        {
            return false;
        }
        let Some(metadata) = self.view.borrow().metadata.clone() else {
            return false;
        };
        let Ok(expected) = shelf::quick_text(&metadata) else {
            return false;
        };
        let operation = Operation::SetQuickText {
            expected: expected.to_owned(),
            text: value.to_owned(),
        };
        let flushed = self.flush_history(ui);
        ui.set_shelf_quick_text(value.into());
        ui.set_shelf_quick_pending(true);
        if !flushed {
            return false;
        }
        self.writing_quick.set(true);
        let success = self.mutate(ui, |history, project, checkpoint, root| {
            history.edit_quick(
                project,
                ui.get_source().as_str(),
                checkpoint,
                project_metadata::QuickEdit {
                    expected: &metadata,
                    operation: &operation,
                    root,
                    after: crate::history::SelectionState::plain(
                        ui.get_shelf_quick_anchor().max(0) as usize,
                        ui.get_shelf_quick_focus().max(0) as usize,
                    ),
                },
            )
        });
        self.writing_quick.set(false);
        if success {
            ui.set_shelf_quick_pending(false);
            self.refresh_quick_status(ui);
        }
        success
    }
    fn quick_capture(&self, ui: &Studio, expected: Option<&str>) -> bool {
        if !ui.get_shelf_open()
            || ui.get_shelf_draft_open()
            || !ui.get_shelf_editable()
            || !ui.get_shelf_quick_visible()
        {
            return false;
        }
        if ui.get_shelf_quick_composing() {
            self.blocked(ui);
            return false;
        }
        if expected.is_some_and(|old| old != ui.get_shelf_quick_text().as_str()) {
            ui.set_shelf_error(
                "Quick Note changed before the shortcut ran. Your draft was kept.".into(),
            );
            return false;
        }
        // Retry stores the pending draft only; it never unexpectedly captures it.
        if ui.get_shelf_quick_pending() {
            self.history.borrow_mut().quick_boundary();
            return self.quick_edited(ui, ui.get_shelf_quick_text().as_str());
        }
        let text = ui.get_shelf_quick_text().to_string();
        let Some(metadata) = self.view.borrow().metadata.clone() else {
            return false;
        };
        if !self.flush_history(ui) {
            return false;
        }
        if text != ui.get_shelf_quick_text().as_str() {
            ui.set_shelf_error("Quick Note changed before capture. Your draft was kept.".into());
            return false;
        }
        let id = Item::new(Kind::Note, "Note".into(), text.clone()).id;
        let operation = Operation::CaptureQuickNote { expected: text, id };
        if self.mutate(ui, |history, project, checkpoint, root| {
            history.edit_quick(
                project,
                ui.get_source().as_str(),
                checkpoint,
                project_metadata::QuickEdit {
                    expected: &metadata,
                    operation: &operation,
                    root,
                    after: crate::history::SelectionState::plain(0, 0),
                },
            )
        }) {
            ui.invoke_focus_shelf_quick();
            true
        } else {
            false
        }
    }
    fn visibility(&self, ui: &Studio, action: i32) {
        if !ui.get_shelf_open()
            || !ui.get_file_dialog_open()
            || !ui.get_shelf_editable()
            || ui.get_shelf_draft_open()
            || ui.get_confirm_close()
            || !(0..=3).contains(&action)
        {
            return;
        }
        if ui.get_shelf_query_composing() {
            self.blocked(ui);
            return;
        }
        let Some(metadata) = self.view.borrow().metadata.clone() else {
            return;
        };
        let kind = selected_kind(ui.get_shelf_kind());
        let Ok(shown) = shelf::visible(&metadata, kind) else {
            return;
        };
        let operation = if action == 3 {
            Operation::SetQuickVisible(!shelf::quick_visible(&metadata).unwrap_or(true))
        } else {
            Operation::SetVisibility {
                kind: if action == 0 { Some(kind) } else { None },
                visible: if action == 0 { !shown } else { action == 1 },
            }
        };
        if !self.flush_history(ui) || !ui.get_shelf_open() || ui.get_shelf_draft_open() {
            return;
        }
        if self.mutate(ui, |history, project, checkpoint, root| {
            history.edit(
                project,
                ui.get_source().as_str(),
                checkpoint,
                &metadata,
                &operation,
                root,
            )
        }) {
            ui.invoke_finish_shelf_list_change();
        }
    }
    fn list_action(&self, ui: &Studio, index: i32, action: i32) {
        if !ui.get_shelf_open()
            || !ui.get_file_dialog_open()
            || !ui.get_shelf_editable()
            || ui.get_shelf_draft_open()
            || ui.get_confirm_close()
            || !(0..=2).contains(&action)
        {
            return;
        }
        let Some(row) = usize::try_from(index)
            .ok()
            .and_then(|i| ui.get_shelf_rows().row_data(i))
        else {
            return;
        };
        if row.item_id.is_empty() || (action != 0 && !ui.get_shelf_query().trim().is_empty()) {
            return;
        }
        let Some(metadata) = self.view.borrow().metadata.clone() else {
            return;
        };
        let Ok(items) = shelf::items(&metadata) else {
            return;
        };
        let Some(item) = items
            .iter()
            .find(|i| i.id == row.item_id.as_str() && i.kind == selected_kind(ui.get_shelf_kind()))
            .cloned()
        else {
            return;
        };
        if !self.flush_history(ui) || !ui.get_shelf_open() || ui.get_shelf_draft_open() {
            return;
        }
        let operation = if action == 0 {
            let mut updated = item.clone();
            updated.pinned = !updated.pinned;
            Operation::Update {
                expected: item,
                item: updated,
            }
        } else {
            Operation::MoveWithinKind {
                expected: item,
                order: items.into_iter().map(|i| i.id).collect(),
                down: action == 2,
            }
        };
        if self.mutate(ui, |history, project, checkpoint, root| {
            history.edit(
                project,
                ui.get_source().as_str(),
                checkpoint,
                &metadata,
                &operation,
                root,
            )
        }) && self.view.borrow().metadata.as_ref() != Some(&metadata)
        {
            // The checklist may move across groups and change the row structure.
            // Done is stable outside the repeater, so keyboard focus stays usable.
            ui.invoke_finish_shelf_list_change();
        }
    }
    fn move_task(&self, ui: &Studio, index: i32, down: bool) {
        if !ui.get_shelf_open()
            || !ui.get_file_dialog_open()
            || !ui.get_shelf_editable()
            || ui.get_shelf_draft_open()
            || ui.get_confirm_close()
        {
            return;
        }
        let (metadata, action) = {
            let view = self.view.borrow();
            (
                view.metadata.clone(),
                usize::try_from(index)
                    .ok()
                    .and_then(|i| view.actions.get(i).cloned().flatten()),
            )
        };
        let (Some(metadata), Some((item, start))) = (metadata, action) else {
            return;
        };
        if !self.flush_history(ui) || !ui.get_shelf_open() || ui.get_shelf_draft_open() {
            return;
        }
        let plan = match fountage_core::todo::move_task(&item.content, Format::General, start, down)
        {
            Ok(plan) => plan,
            Err(error) => {
                ui.set_shelf_error(error.into());
                return;
            }
        };
        let operation = Operation::MoveTask {
            expected: item.clone(),
            start,
            down,
        };
        if !self.mutate(ui, |history, project, checkpoint, root| {
            history.edit(
                project,
                ui.get_source().as_str(),
                checkpoint,
                &metadata,
                &operation,
                root,
            )
        }) {
            return;
        }
        if let Some(plan) = plan {
            let moved = self.view.borrow().actions.iter().position(|action| {
                action
                    .as_ref()
                    .is_some_and(|(i, offset)| i.id == item.id && *offset == plan.anchor)
            });
            if let Some(index) = moved {
                let rows = ui.get_shelf_rows();
                if let Some(row) = rows.row_data(index) {
                    let control = if down && row.can_move_down {
                        4
                    } else if !down && row.can_move_up {
                        3
                    } else {
                        2
                    };
                    let position = (control * rows.row_count() + index) as i32;
                    ui.invoke_finish_shelf_draft(position);
                }
            }
        }
    }
    fn begin_edit(&self, ui: &Studio, index: i32, kind: EditorKind) {
        if self.quick_blocked(ui) {
            return;
        }
        if !ui.get_shelf_open()
            || !ui.get_file_dialog_open()
            || !ui.get_shelf_editable()
            || ui.get_shelf_draft_open()
            || ui.get_confirm_close()
        {
            return;
        }
        let task = kind != EditorKind::Checklist;
        if task && ui.get_shelf_kind() != 0 {
            return;
        }
        let requested_task = if kind == EditorKind::EditTask {
            usize::try_from(index)
                .ok()
                .and_then(|i| self.view.borrow().actions.get(i).cloned().flatten())
        } else {
            None
        };
        let requested_id = usize::try_from(index)
            .ok()
            .and_then(|i| ui.get_shelf_rows().row_data(i))
            .map(|row| row.item_id);
        if !self.flush_history(ui) {
            return;
        }
        let Some(metadata) = self.view.borrow().metadata.clone() else {
            return;
        };
        if task && index < 0 {
            return;
        }
        let original = if index == -1 {
            None
        } else {
            let Some(id) = requested_task
                .as_ref()
                .map(|(item, _)| item.id.clone().into())
                .or(requested_id)
            else {
                return;
            };
            let Some(item) = shelf::items(&metadata).ok().and_then(|items| {
                items
                    .into_iter()
                    .find(|i| i.id == id.as_str() && i.kind == selected_kind(ui.get_shelf_kind()))
            }) else {
                return;
            };
            Some(item)
        };
        let item = original.clone().unwrap_or_else(|| {
            Item::new(
                selected_kind(ui.get_shelf_kind()),
                format!("Untitled {}", kind_label(ui.get_shelf_kind())),
                "".into(),
            )
        });
        let task_start = if kind == EditorKind::EditTask {
            let Some((expected, start)) = requested_task else {
                return;
            };
            if expected != item {
                ui.set_shelf_error("The checklist changed. Select the task again.".into());
                return;
            }
            Some(start)
        } else {
            None
        };
        let mut fields = std::array::from_fn(|_| crate::history::History::default());
        let title = if let Some(start) = task_start {
            match fountage_core::todo::content(&item.content, Format::General, start) {
                Ok(text) => text,
                Err(error) => {
                    ui.set_shelf_error(error.into());
                    return;
                }
            }
        } else if task {
            ""
        } else {
            &item.title
        };
        let content = if task { "" } else { &item.content };
        fields[0].reset(title);
        fields[1].reset(content);
        ui.set_shelf_draft_removal_preview(if task { title } else { &item.content }.into());
        ui.set_shelf_draft_task_count(
            i32::try_from(writing::analyze(&item.content, Format::General).todos.len())
                .unwrap_or(i32::MAX),
        );
        ui.set_shelf_draft_kind(match item.kind {
            Kind::Checklist => 0,
            Kind::Note => 1,
            Kind::Snippet => 2,
        });
        ui.set_shelf_draft_removing(false);
        ui.set_shelf_draft_title(title.into());
        ui.set_shelf_draft_content(content.into());
        ui.set_shelf_draft_task(task);
        ui.set_shelf_draft_task_edit(task_start.is_some());
        ui.set_shelf_draft_target(item.title.clone().into());
        ui.set_shelf_draft_existing(original.is_some());
        *self.draft.borrow_mut() = Some(Draft {
            metadata,
            original,
            task,
            task_start,
            item,
            fields,
        });
        ui.set_shelf_error("".into());
        ui.set_shelf_session(ui.get_shelf_session().wrapping_add(1));
        ui.set_shelf_draft_open(true);
        ui.invoke_focus_shelf_draft();
        self.reflect_fields(ui);
    }
    fn field_input(&self, ui: &Studio, index: i32, value: &str, edited: bool) {
        if !ui.get_shelf_draft_open() || !ui.get_shelf_open() {
            return;
        }
        if !edited {
            self.flush_history(ui);
        }
        if let Some(draft) = self.draft.borrow_mut().as_mut()
            && let Ok(index) = usize::try_from(index)
            && let Some(field) = draft.fields.get_mut(index)
        {
            if edited {
                field.record(value);
            } else {
                field.input(value);
            }
        }
        self.reflect_fields(ui);
    }
    fn field_boundary(&self, ui: &Studio) {
        self.flush_history(ui);
        if let Some(draft) = self.draft.borrow_mut().as_mut() {
            for field in &mut draft.fields {
                field.boundary();
            }
        }
        self.reflect_fields(ui);
    }
    fn field_history_at(&self, ui: &Studio, index: usize, redo: bool, allow_refocus: bool) -> bool {
        if !ui.get_shelf_draft_open() || index >= 2 || (index == 1 && ui.get_shelf_draft_task()) {
            return true;
        }
        if ui.get_shelf_draft_composing() {
            self.blocked(ui);
            return false;
        }
        let refocus = allow_refocus && ui.get_shelf_draft_field_index() == index as i32;
        let value = if index == 0 {
            ui.get_shelf_draft_title()
        } else {
            ui.get_shelf_draft_content()
        };
        let step = self
            .draft
            .borrow_mut()
            .as_mut()
            .and_then(|d| d.fields[index].step(value.as_str(), redo));
        if let Some((value, anchor, focus)) = step {
            if index == 0 {
                ui.set_shelf_draft_title(value.into());
            } else {
                ui.set_shelf_draft_content(value.into());
            }
            // The command belongs to its original field. A later click/Tab may
            // have moved focus; never redirect focus or Undo to that new field.
            if refocus {
                ui.invoke_select_shelf_draft_field(index as i32, anchor as i32, focus as i32);
            }
        }
        self.reflect_fields(ui);
        true
    }
    fn blocked(&self, ui: &Studio) {
        ui.set_shelf_error("Finish composing text before saving or changing focus.".into());
    }
    fn request_removal(&self, ui: &Studio) {
        if !ui.get_shelf_open()
            || !ui.get_shelf_draft_open()
            || !ui.get_file_dialog_open()
            || !self
                .draft
                .borrow()
                .as_ref()
                .is_some_and(|d| d.original.is_some() && (!d.task || d.task_start.is_some()))
            || ui.get_confirm_close()
        {
            return;
        }
        let session = ui.get_shelf_session();
        if !self.flush_history(ui) {
            return;
        }
        if ui.get_shelf_session() != session
            || !ui.get_shelf_draft_open()
            || ui.get_shelf_draft_removing()
        {
            return;
        }
        if ui.get_shelf_draft_composing() {
            self.blocked(ui);
            return;
        }
        ui.set_shelf_draft_removing(true);
        ui.set_shelf_error("".into());
        ui.set_shelf_session(ui.get_shelf_session().wrapping_add(1));
        ui.invoke_focus_shelf_draft_control(2);
    }
    fn draft_action(&self, ui: &Studio, apply: bool) {
        if !ui.get_shelf_draft_open()
            || !ui.get_shelf_open()
            || !ui.get_file_dialog_open()
            || ui.get_confirm_close()
        {
            return;
        }
        if !self.flush_history(ui) {
            return;
        }
        if ui.get_shelf_draft_composing() {
            self.blocked(ui);
            return;
        }
        if !apply && ui.get_shelf_draft_removing() {
            ui.set_shelf_draft_removing(false);
            ui.set_shelf_error("".into());
            ui.set_shelf_session(ui.get_shelf_session().wrapping_add(1));
            ui.invoke_focus_shelf_draft();
            return;
        }
        if apply
            && ui.get_shelf_draft_removing()
            && !self
                .draft
                .borrow()
                .as_ref()
                .is_some_and(|d| d.original.is_some() && (!d.task || d.task_start.is_some()))
        {
            ui.set_shelf_error("The selection changed. Reopen the editor before deleting.".into());
            return;
        }
        if apply {
            let Some((expected, operation)) = self.draft.borrow().as_ref().map(|draft| {
                let mut item = draft.item.clone();
                item.title = ui.get_shelf_draft_title().to_string();
                item.content = ui.get_shelf_draft_content().to_string();
                let op = if ui.get_shelf_draft_removing() && !draft.task {
                    Operation::Remove(draft.item.clone())
                } else if let Some(start) = draft.task_start {
                    if ui.get_shelf_draft_removing() {
                        Operation::RemoveTask {
                            expected: draft.item.clone(),
                            start,
                        }
                    } else {
                        Operation::UpdateTask {
                            expected: draft.item.clone(),
                            start,
                            text: ui.get_shelf_draft_title().to_string(),
                        }
                    }
                } else if draft.task {
                    Operation::AppendTask {
                        expected: draft.item.clone(),
                        text: ui.get_shelf_draft_title().to_string(),
                    }
                } else if let Some(original) = &draft.original {
                    Operation::Update {
                        expected: original.clone(),
                        item,
                    }
                } else {
                    Operation::Add(item)
                };
                (draft.metadata.clone(), op)
            }) else {
                return;
            };
            if !self.mutate(ui, |history, project, checkpoint, root| {
                history.edit(
                    project,
                    ui.get_source().as_str(),
                    checkpoint,
                    &expected,
                    &operation,
                    root,
                )
            }) {
                self.reflect_fields(ui);
                return;
            }
        }
        let task_id = self
            .draft
            .borrow()
            .as_ref()
            .filter(|draft| draft.task)
            .map(|draft| {
                (
                    draft.item.id.clone(),
                    if ui.get_shelf_draft_removing() {
                        None
                    } else {
                        draft.task_start
                    },
                )
            });
        *self.draft.borrow_mut() = None;
        ui.set_shelf_draft_open(false);
        ui.set_shelf_draft_removing(false);
        ui.set_shelf_session(ui.get_shelf_session().wrapping_add(1));
        ui.set_shelf_draft_title("".into());
        ui.set_shelf_draft_content("".into());
        ui.set_shelf_error("".into());
        self.refresh(ui);
        let rows = ui.get_shelf_rows();
        let focus = task_id
            .and_then(|(id, start)| {
                if let Some(start) = start {
                    return self
                        .view
                        .borrow()
                        .actions
                        .iter()
                        .position(|action| {
                            action
                                .as_ref()
                                .is_some_and(|(item, offset)| item.id == id && *offset == start)
                        })
                        .map(|index| 2 * rows.row_count() + index);
                }
                (0..rows.row_count())
                    .find(|i| {
                        rows.row_data(*i)
                            .is_some_and(|row| row.item_id == id.as_str())
                    })
                    .map(|index| rows.row_count() + index)
            })
            .map(|index| index as i32)
            .unwrap_or(-4);
        ui.invoke_finish_shelf_draft(focus);
    }
    fn history(&self, ui: &Studio, redo: bool) {
        if !self.flush_history(ui) {
            return;
        }
        if let Some(target) = Self::history_target(ui) {
            self.execute_history(ui, target, redo, true);
        }
    }
    fn close(&self, ui: &Studio) {
        self.history.borrow_mut().quick_boundary();
        if self.quick_blocked(ui) {
            return;
        }
        if !self.flush_history(ui) {
            return;
        }
        if !ui.get_shelf_open() || !ui.get_file_dialog_open() {
            return;
        }
        if ui.get_shelf_draft_open() {
            self.draft_action(ui, false);
            return;
        }
        ui.set_shelf_open(false);
        ui.set_file_dialog_open(false);
        let view = self.view.borrow();
        let same_project = self
            .session
            .project
            .borrow()
            .as_ref()
            .is_some_and(|p| p.project_id() == view.project_id);
        if let Some((source, anchor, focus, au, fu)) = &view.selection
            && same_project
            && source == ui.get_source().as_str()
        {
            ui.invoke_finish_shelf(*anchor, *focus, *au, *fu);
            return;
        }
        ui.invoke_focus_source();
    }
    fn reset(&self, ui: &Studio) {
        ui.set_shelf_quick_pending(false);
        ui.set_shelf_quick_text("".into());
        ui.set_shelf_quick_visible(true);
        self.pending.borrow_mut().clear();
        ui.set_shelf_session(ui.get_shelf_session().wrapping_add(1));
        *self.draft.borrow_mut() = None;
        ui.set_shelf_draft_open(false);
        ui.set_shelf_draft_task(false);
        ui.set_shelf_kind(0);
        ui.set_shelf_replace_selection(false);
        ui.set_shelf_can_capture(false);
        ui.set_shelf_draft_kind(0);
        ui.set_shelf_draft_task_edit(false);
        ui.set_shelf_draft_removing(false);
        ui.set_shelf_draft_removal_preview("".into());
        ui.set_shelf_draft_task_count(0);
        ui.set_shelf_draft_target("".into());
        ui.set_shelf_draft_title("".into());
        ui.set_shelf_draft_content("".into());
        self.history.borrow_mut().reset();
        self.query_history.borrow_mut().reset("");
        ui.set_shelf_query("".into());
        ui.set_shelf_query_can_undo(false);
        ui.set_shelf_query_can_redo(false);
        *self.view.borrow_mut() = View::default();
        ui.set_shelf_open(false);
        ui.set_shelf_rows(ModelRc::new(VecModel::default()));
        ui.set_shelf_can_undo(false);
        ui.set_shelf_can_redo(false);
        ui.set_shelf_editable(false);
        ui.set_shelf_status("".into());
        ui.set_shelf_error("".into());
        // file-dialog-open may belong to the document adoption's native picker.
    }
}
pub(super) fn install(
    ui: &Studio,
    session: DraftSession,
    root: impl Fn() -> Result<PathBuf, String> + 'static,
) {
    let controller = Rc::new(Controller {
        session,
        history: RefCell::default(),
        query_history: RefCell::default(),
        view: RefCell::default(),
        draft: RefCell::default(),
        pending: RefCell::default(),
        draining: Cell::new(false),
        writing_quick: Cell::new(false),
        root: Box::new(root),
    });
    controller.reset(ui);
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_project_history_action(move |redo| {
        weak.upgrade()
            .is_some_and(|ui| c.project_history(&ui, redo))
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_quick_input(move |value| {
        if let Some(ui) = weak.upgrade() {
            c.quick_input(&ui, value.as_str());
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_quick_selection(move |value, anchor, focus| {
        if let Some(ui) = weak.upgrade() {
            c.quick_selection(&ui, value.as_str(), anchor, focus);
        }
    });
    let c = controller.clone();
    ui.on_shelf_quick_boundary(move || {
        c.history.borrow_mut().quick_boundary();
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_quick_edited(move |value| {
        if let Some(ui) = weak.upgrade() {
            c.quick_edited(&ui, value.as_str());
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_quick_capture(move || {
        if let Some(ui) = weak.upgrade() {
            c.quick_capture(&ui, None);
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_visibility_requested(move |action| {
        if let Some(ui) = weak.upgrade() {
            c.visibility(&ui, action);
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_capture(move |cut| {
        if let Some(ui) = weak.upgrade() {
            c.capture(&ui, cut);
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_kind_requested(move |kind| {
        if let Some(ui) = weak.upgrade() {
            if !(0..=2).contains(&kind)
                || !ui.get_shelf_open()
                || ui.get_shelf_draft_open()
                || ui.get_shelf_query_composing()
                || c.quick_blocked(&ui)
            {
                return;
            }
            if !c.flush_history(&ui) || !ui.get_shelf_open() || ui.get_shelf_draft_open() {
                return;
            }
            ui.set_shelf_session(ui.get_shelf_session().wrapping_add(1));
            ui.set_shelf_kind(kind);
            ui.set_shelf_error("".into());
            c.refresh(&ui);
            ui.invoke_finish_shelf_list_change();
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_query_input(move |value| {
        if let Some(ui) = weak.upgrade() {
            if !c.flush_history(&ui) {
                return;
            }
            c.query_history.borrow_mut().input(value.as_str());
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_query_edited(move |value| {
        if let Some(ui) = weak.upgrade() {
            if !ui.get_shelf_open() || ui.get_shelf_draft_open() {
                return;
            }
            let focused = ui.get_shelf_query_focused();
            c.query_history.borrow_mut().record(value.as_str());
            c.refresh(&ui);
            if focused {
                ui.invoke_focus_shelf_search();
            }
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_queue_history(move |redo| {
        weak.upgrade().is_some_and(|ui| c.queue_history(&ui, redo))
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_queue_focus(move |backward| {
        weak.upgrade()
            .is_some_and(|ui| c.queue_focus(&ui, backward))
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_queue_apply(move || weak.upgrade().is_some_and(|ui| c.queue_apply(&ui)));
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_queue_clipboard(move |kind| {
        weak.upgrade()
            .is_some_and(|ui| c.queue_clipboard(&ui, kind))
    });

    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_flush_history(move || weak.upgrade().is_none_or(|ui| c.flush_history(&ui)));

    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_insert(move |index| {
        if let Some(ui) = weak.upgrade() {
            c.insert(&ui, index);
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_edit(move |index| {
        if let Some(ui) = weak.upgrade() {
            c.begin_edit(&ui, index, EditorKind::Checklist);
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_add_task(move |index| {
        if let Some(ui) = weak.upgrade() {
            c.begin_edit(&ui, index, EditorKind::AppendTask);
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_edit_task(move |index| {
        if let Some(ui) = weak.upgrade() {
            c.begin_edit(&ui, index, EditorKind::EditTask);
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_remove_requested(move || {
        if let Some(ui) = weak.upgrade() {
            c.request_removal(&ui);
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_list_action(move |index, action| {
        if let Some(ui) = weak.upgrade() {
            c.list_action(&ui, index, action);
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_move_task(move |index, down| {
        if let Some(ui) = weak.upgrade() {
            c.move_task(&ui, index, down);
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_draft_action(move |apply| {
        if let Some(ui) = weak.upgrade() {
            c.draft_action(&ui, apply);
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_draft_edited(move |index, value| {
        if let Some(ui) = weak.upgrade() {
            c.field_input(&ui, index, value.as_str(), true);
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_draft_input(move |index, value| {
        if let Some(ui) = weak.upgrade() {
            c.field_input(&ui, index, value.as_str(), false);
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_draft_boundary(move || {
        if let Some(ui) = weak.upgrade() {
            c.field_boundary(&ui);
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_draft_focused(move || {
        if let Some(ui) = weak.upgrade() {
            c.field_boundary(&ui);
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_draft_blocked(move || {
        if let Some(ui) = weak.upgrade() {
            c.blocked(&ui);
        }
    });

    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_focus_move(move |backward| {
        if let Some(ui) = weak.upgrade() {
            c.focus_move(&ui, backward);
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_requested(move || {
        if let Some(ui) = weak.upgrade() {
            c.open(&ui);
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_toggle(move |index| {
        if let Some(ui) = weak.upgrade() {
            c.toggle(&ui, index);
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_history(move |redo| {
        if let Some(ui) = weak.upgrade() {
            c.history(&ui, redo);
        }
    });
    let weak = ui.as_weak();
    let c = controller.clone();
    ui.on_shelf_close(move || {
        if let Some(ui) = weak.upgrade() {
            c.close(&ui);
        }
    });
    let weak = ui.as_weak();
    ui.on_shelf_reset(move || {
        if let Some(ui) = weak.upgrade() {
            controller.reset(&ui);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{project_draft, recovery, session};
    use fountage_core::project::{Entry, NAMESPACE};
    use slint::platform::{Key, WindowEvent};
    fn fixture(category: u32) -> Project {
        let mut p = Project::new(Format::from_code(category).unwrap()).unwrap();
        p.replace_source("Title: Shelf UI\r\n\r\nZoë 🌙 世界 waits.\r\n")
            .unwrap();
        let mut metadata = p.metadata().clone();
        metadata["foreign"] = serde_json::json!({"keep":18446744073709551615u64});
        metadata[NAMESPACE]["shelf"] = serde_json::json!({"version":1,"order":["note","plan","future"],"items":{"plan":{"kind":"checklist","title":"Revision plan","content":"[ ] Review Zoë 🌙 [[private]]\r\n[x] Send draft\r\n","pinned":false,"unknown":7},"note":{"kind":"note","title":"Keep","content":"Opaque note","pinned":true},"future":{"kind":"future","opaque":true}}});
        let mut entries = p.entries().clone();
        entries.insert(
            "info.json".into(),
            Entry::File(serde_json::to_vec(&metadata).unwrap()),
        );
        entries.insert("attachment.bin".into(), Entry::File(vec![0, 255, 1]));
        entries.insert("link".into(), Entry::SymbolicLink("../unfollowed".into()));
        Project::from_entries(entries).unwrap()
    }
    fn setup(project: Project, root: PathBuf) -> (Studio, DraftSession) {
        let ui = Studio::new().unwrap();
        ui.set_source(project.source().into());
        ui.set_category(recovery::category_code(project.format()));
        ui.set_project_mode(true);
        let mut saved = session::Checkpoint::saved(project.source());
        saved.adopt_saved(project.source(), Some(project.metadata()));
        let checkpoint = Rc::new(RefCell::new(saved));
        let baseline = recovery::Recovery::for_project(&root, &project).unwrap();
        let recovery = Rc::new(RefCell::new(Ok(baseline)));
        let saved = checkpoint.clone();
        let recovery_edits = recovery.clone();
        let edits = crate::install_editor_history(&ui, move |ui| {
            crate::reflect_checkpoint(ui, &saved.borrow());
            crate::update_recovery(ui, &recovery_edits.borrow());
        });
        let state = DraftSession {
            checkpoint,
            history: edits,
            project: Rc::new(RefCell::new(Some(project))),
            recovery,
        };
        install(&ui, state.clone(), move || Ok(root.clone()));
        ui.show().unwrap();
        (ui, state)
    }
    fn checkbox(ui: &Studio) -> i_slint_backend_testing::ElementHandle {
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(ui, "Review Zoë 🌙")
            .find(|e| e.accessible_checkable() == Some(true))
            .expect("real checklist checkbox")
    }
    fn key(ui: &Studio, text: slint::SharedString) {
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        ui.window()
            .dispatch_event(WindowEvent::KeyReleased { text });
    }
    fn undo(ui: &Studio, redo: bool) {
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Control.into(),
        });
        if redo {
            ui.window().dispatch_event(WindowEvent::KeyPressed {
                text: Key::Shift.into(),
            });
        }
        key(ui, "z".into());
        if redo {
            ui.window().dispatch_event(WindowEvent::KeyReleased {
                text: Key::Shift.into(),
            });
        }
        ui.window().dispatch_event(WindowEvent::KeyReleased {
            text: Key::Control.into(),
        });
    }
    #[test]
    fn shelf_visibility_all_categories_undo_recovery_save_reopen_and_hidden_record_preservation() {
        i_slint_backend_testing::init_no_event_loop();
        for category in 0..8 {
            let root = tempfile::tempdir().unwrap();
            let original = lists_fixture(category);
            let (ui, state) = setup(original.clone(), root.path().into());
            ui.invoke_show_shelf();
            ui.invoke_shelf_kind_requested(1);
            i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                &ui,
                "Hide Shelf notes",
            )
            .next()
            .unwrap()
            .invoke_accessible_default_action();
            assert!(!ui.get_shelf_kind_visible());
            assert_eq!(ui.get_shelf_rows().row_count(), 0);
            assert_eq!(ui.get_shelf_focus_position(), -1);
            let note_hidden = state.project.borrow().as_ref().unwrap().clone();
            let mut expected = original.metadata().clone();
            expected[NAMESPACE]["shelf"]["visibility"] = serde_json::json!({"note":false});
            assert_eq!(note_hidden.metadata(), &expected);
            undo(&ui, false);
            assert!(!ui.get_dirty());
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            assert!(ui.get_shelf_kind_visible());
            undo(&ui, true);
            ui.invoke_shelf_visibility_requested(2);
            let hidden = state.project.borrow().as_ref().unwrap().clone();
            assert_eq!(
                shelf::items(hidden.metadata()),
                shelf::items(original.metadata())
            );
            let recovery_path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
            ui.invoke_shelf_visibility_requested(2);
            assert_eq!(
                state.recovery.borrow().as_ref().unwrap().path(),
                recovery_path
            );
            for choice in 0..3 {
                ui.invoke_shelf_kind_requested(choice);
                assert!(!ui.get_shelf_kind_visible());
                assert_eq!(ui.get_shelf_rows().row_count(), 0);
            }
            let snapshot = recovery::read(&recovery_path).unwrap();
            assert_eq!(
                recovery::project_from_snapshot(&recovery_path, &snapshot)
                    .unwrap()
                    .unwrap()
                    .entries(),
                hidden.entries()
            );
            let path = root.path().join("Hidden.fountage");
            project_draft::save_copy(&hidden, ui.get_source().as_str(), &path).unwrap();
            if let Some(directory) = std::env::var_os("FOUNTAGE_EXPORT_VISIBILITY_FIXTURES") {
                std::fs::create_dir_all(&directory).unwrap();
                let target = PathBuf::from(directory).join(format!("Hidden {category}.fountage"));
                project_draft::save_copy(&hidden, ui.get_source().as_str(), &target).unwrap();
            }
            let reopened = Project::read(&path).unwrap();
            assert_eq!(reopened.entries(), hidden.entries());
            for (path, entry) in original.entries() {
                if path != std::ffi::OsStr::new("info.json") {
                    assert_eq!(reopened.entries().get(path), Some(entry));
                }
            }
            undo(&ui, false);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                note_hidden.entries()
            );
            undo(&ui, true);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                hidden.entries()
            );
            ui.invoke_shelf_visibility_requested(1);
            for choice in 0..3 {
                ui.invoke_shelf_kind_requested(choice);
                assert!(ui.get_shelf_kind_visible());
                assert!(ui.get_shelf_rows().row_count() > 0);
            }
            assert_eq!(ui.get_source(), original.source());
            assert_eq!(
                shelf::items(state.project.borrow().as_ref().unwrap().metadata()),
                shelf::items(original.metadata())
            );
            ui.hide().unwrap();
            let (ui, _) = setup(reopened, root.path().into());
            ui.invoke_show_shelf();
            assert!(!ui.get_shelf_kind_visible());
            assert_eq!(ui.get_shelf_rows().row_count(), 0);
            assert!(!ui.get_shelf_can_undo());
            ui.hide().unwrap();
        }
    }
    #[test]
    fn swift_quick_draft_survives_slint_shelf_edits_save_and_complete_recovery() {
        let Some(directory) = std::env::var_os("FOUNTAGE_QUICK_NOTE_FIXTURES") else {
            return;
        };
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original =
            Project::read(&PathBuf::from(directory).join("Swift Quick Note Draft.fountage"))
                .unwrap();
        assert_eq!(
            shelf::quick_text(original.metadata()).unwrap(),
            "Quick note Zoë 🌙\r\nSecond line 世界\r\n"
        );
        let quick = original.metadata()[NAMESPACE]["shelf"]["quickEntry"].clone();
        let (ui, state) = setup(original.clone(), root.path().into());
        ui.invoke_show_shelf();
        let index = ui
            .get_shelf_rows()
            .iter()
            .position(|row| !row.item_id.is_empty())
            .unwrap() as i32;
        ui.invoke_shelf_list_action(index, 0);
        let changed = state.project.borrow().as_ref().unwrap().clone();
        assert_eq!(changed.metadata()[NAMESPACE]["shelf"]["quickEntry"], quick);
        assert_eq!(changed.source(), original.source());
        let path = root.path().join("Retained Quick Draft.fountage");
        project_draft::save_copy(&changed, ui.get_source().as_str(), &path).unwrap();
        assert_eq!(Project::read(&path).unwrap().entries(), changed.entries());
        let recovery_path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
        let snapshot = recovery::read(&recovery_path).unwrap();
        assert_eq!(
            recovery::project_from_snapshot(&recovery_path, &snapshot)
                .unwrap()
                .unwrap()
                .entries(),
            changed.entries()
        );
        if let Some(output) = std::env::var_os("FOUNTAGE_QUICK_NOTE_RECOVERY_OUTPUT") {
            let archive =
                recovery::Recovery::for_project(&PathBuf::from(output), &changed).unwrap();
            let snapshot = recovery::read(archive.path()).unwrap();
            assert_eq!(
                recovery::project_from_snapshot(archive.path(), &snapshot)
                    .unwrap()
                    .unwrap()
                    .entries(),
                changed.entries()
            );
        }
        undo(&ui, false);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        ui.hide().unwrap();
    }
    #[test]
    fn swift_visibility_interop_preserves_project_and_applies_mixed_flags() {
        let (Some(input), Some(output)) = (
            std::env::var_os("FOUNTAGE_VISIBILITY_FIXTURES"),
            std::env::var_os("FOUNTAGE_SWIFT_VISIBILITY_FIXTURES"),
        ) else {
            return;
        };
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        for category in 0..8 {
            let before =
                Project::read(&PathBuf::from(&input).join(format!("Hidden {category}.fountage")))
                    .unwrap();
            let after = Project::read(
                &PathBuf::from(&output).join(format!("Swift Mixed {category}.fountage")),
            )
            .unwrap();
            let mut expected = before.metadata().clone();
            expected[NAMESPACE]["shelf"]["visibility"] =
                serde_json::json!({"note":false,"snippet":true,"checklist":true});
            // Swift's normal document writer updates its known modifiedAt field.
            let timestamp = after.metadata()[NAMESPACE]["modifiedAt"].as_str().unwrap();
            assert!(timestamp.contains('T') && timestamp.ends_with('Z'));
            expected[NAMESPACE]["modifiedAt"] = timestamp.into();
            assert_eq!(after.metadata(), &expected);
            assert_eq!(after.source(), before.source());
            assert_eq!(after.format(), before.format());
            for (path, entry) in before.entries() {
                if path != std::ffi::OsStr::new("info.json") {
                    assert_eq!(after.entries().get(path), Some(entry));
                }
            }
            let (ui, _) = setup(after, root.path().join(category.to_string()));
            ui.invoke_show_shelf();
            assert!(ui.get_shelf_kind_visible());
            assert!(ui.get_shelf_rows().row_count() > 0);
            ui.invoke_shelf_kind_requested(1);
            assert!(!ui.get_shelf_kind_visible());
            assert_eq!(ui.get_shelf_rows().row_count(), 0);
            ui.invoke_shelf_kind_requested(2);
            assert!(ui.get_shelf_kind_visible());
            assert!(ui.get_shelf_rows().row_count() > 0);
            ui.hide().unwrap();
        }
    }
    #[test]
    fn shelf_visibility_failed_recovery_stale_pending_history_and_forms_preserve_state() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = lists_fixture(0);
        let (ui, state) = setup(original.clone(), root.path().into());
        ui.invoke_show_shelf();
        ui.invoke_shelf_visibility_requested(2);
        assert!(ui.invoke_shelf_queue_history(false));
        ui.invoke_shelf_visibility_requested(0);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        assert!(!ui.get_shelf_error().is_empty());
        assert!(ui.get_shelf_kind_visible());
        new_action(&ui);
        draft_text(&ui, 0, "Keep unsaved form");
        ui.invoke_shelf_visibility_requested(2);
        assert_eq!(ui.get_shelf_draft_title(), "Keep unsaved form");
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        ui.invoke_shelf_draft_action(false);
        ui.invoke_shelf_close();
        let blocked = root.path().join("blocked");
        std::fs::write(&blocked, "keep").unwrap();
        install(&ui, state.clone(), move || Ok(blocked.clone()));
        ui.invoke_show_shelf();
        let recovery_path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
        ui.invoke_shelf_visibility_requested(2);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        assert_eq!(
            state.recovery.borrow().as_ref().unwrap().path(),
            recovery_path
        );
        assert!(ui.get_shelf_kind_visible());
        assert!(!ui.get_shelf_can_undo());
        assert!(!ui.get_shelf_error().is_empty());
        ui.hide().unwrap();
    }
    #[test]
    fn capture_title_limit_preserves_complete_graphemes_and_content() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let mut original = lists_fixture(0);
        let family = "👩‍👩‍👧‍👦";
        let source = format!("\r\n {}{family}e\u{301} next line\r\n", "x".repeat(59));
        original.replace_source(&source).unwrap();
        let (ui, state) = setup(original, root.path().into());
        ui.invoke_select_source(0, source.len() as i32);
        ui.invoke_show_shelf();
        ui.invoke_shelf_capture(false);
        let captured = state.project.borrow().as_ref().unwrap().clone();
        let item = shelf::items(captured.metadata())
            .unwrap()
            .into_iter()
            .find(|i| i.content == source)
            .unwrap();
        assert_eq!(item.title, format!("{}{family}", "x".repeat(59)));
        assert_eq!(item.title.graphemes(true).count(), 60);
        assert_eq!(ui.get_source(), source);
        undo(&ui, false);
        assert_eq!(ui.get_source(), source);
        assert!(!ui.get_dirty());
        ui.hide().unwrap();
    }
    #[test]
    fn capture_menu_and_shelf_history_preserve_clipboard_and_plain_or_formatted_selection() {
        i_slint_backend_testing::init_no_event_loop();
        for formatted in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let original = lists_fixture(0);
            let source = original.source().to_owned();
            let (ui, state) = setup(original.clone(), root.path().into());
            if formatted {
                crate::formatted_live::install(&ui);
                ui.set_formatted_active(true);
            }
            let start = source.find("Zoë").unwrap();
            i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
            ui.invoke_select_source(start as i32, source.len() as i32);
            assert_eq!(
                (ui.get_source_anchor(), ui.get_source_focus()),
                (start as i32, source.len() as i32),
                "formatted={formatted}"
            );
            let platform = i_slint_core::window::WindowInner::from_pub(ui.window())
                .context()
                .platform();
            platform.set_clipboard_text(
                "Keep clipboard",
                i_slint_core::platform::Clipboard::DefaultClipboard,
            );
            assert!(ui.get_can_edit_source());
            ui.invoke_capture_selection_to_shelf(false);
            assert!(!ui.get_shelf_open());
            assert_eq!(ui.get_source(), source);
            assert!(ui.get_dirty(), "formatted={formatted}");
            assert_eq!(
                platform
                    .clipboard_text(i_slint_core::platform::Clipboard::DefaultClipboard)
                    .as_deref(),
                Some("Keep clipboard")
            );
            let copied = state.project.borrow().as_ref().unwrap().clone();
            ui.invoke_show_shelf();
            assert!(ui.get_shelf_can_undo());
            ui.invoke_shelf_history(false);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            assert!(!ui.get_dirty());
            ui.invoke_shelf_history(true);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                copied.entries()
            );
            ui.invoke_shelf_close();
            undo(&ui, false);
            ui.invoke_select_source(start as i32, source.len() as i32);
            ui.invoke_capture_selection_to_shelf(true);
            assert_eq!(ui.get_source(), &source[..start]);
            undo(&ui, false);
            assert_eq!(ui.get_source(), source);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            ui.hide().unwrap();
        }
    }
    #[test]
    fn capture_stale_empty_preview_and_new_metadata_branch_preserve_source_and_redo_guards() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = lists_fixture(0);
        let source = original.source().to_owned();
        let (ui, state) = setup(original.clone(), root.path().into());
        ui.invoke_select_source(0, 0);
        ui.invoke_capture_selection_to_shelf(true);
        assert!(!ui.get_shelf_open());
        ui.invoke_show_shelf();
        assert!(!ui.get_shelf_can_capture());
        ui.invoke_shelf_capture(true);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        ui.invoke_shelf_close();
        ui.invoke_select_source(0, source.len() as i32);
        ui.set_preview_open(true);
        ui.invoke_show_shelf();
        assert!(!ui.get_shelf_can_capture());
        ui.invoke_shelf_capture(false);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        ui.invoke_shelf_close();
        ui.set_preview_open(false);
        ui.invoke_select_source(0, source.len() as i32);
        ui.invoke_show_shelf();
        ui.set_source(format!("{source}external").into());
        ui.invoke_shelf_capture(true);
        assert_eq!(ui.get_source(), format!("{source}external"));
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        ui.set_source(source.clone().into());
        ui.invoke_shelf_close();
        ui.invoke_select_source(0, source.len() as i32);
        ui.invoke_show_shelf();
        ui.invoke_shelf_capture(true);
        undo(&ui, false);
        assert!(ui.get_can_redo());
        ui.invoke_show_shelf();
        ui.invoke_shelf_list_action(list_index(&ui, "plan"), 0);
        ui.invoke_shelf_close();
        assert!(
            !ui.get_can_redo(),
            "New metadata edit invalidates old capture Redo branch"
        );
        let changed = state.project.borrow().as_ref().unwrap().clone();
        undo(&ui, true);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            changed.entries()
        );
        assert_eq!(ui.get_source(), source);
        ui.hide().unwrap();
    }
    #[test]
    fn capture_copy_and_cut_are_one_source_metadata_undo_with_save_recovery_all_categories() {
        i_slint_backend_testing::init_no_event_loop();
        for category in 0..8 {
            for cut in [false, true] {
                let root = tempfile::tempdir().unwrap();
                let original = lists_fixture(category);
                let source = original.source().to_owned();
                let start = source.find("Zoë").unwrap();
                let end = source.len();
                let content = &source[start..end];
                let (ui, state) = setup(original.clone(), root.path().into());
                ui.invoke_select_source(end as i32, start as i32);
                ui.invoke_show_shelf();
                assert!(ui.get_shelf_can_capture());
                i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                    &ui,
                    if cut {
                        "Cut Selection to Shelf"
                    } else {
                        "Copy Selection to Shelf"
                    },
                )
                .next()
                .unwrap()
                .invoke_accessible_default_action();
                assert!(!ui.get_shelf_open() && !ui.get_file_dialog_open());
                let captured = state.project.borrow().as_ref().unwrap().clone();
                let item = shelf::items(captured.metadata())
                    .unwrap()
                    .into_iter()
                    .find(|i| {
                        !shelf::items(original.metadata())
                            .unwrap()
                            .iter()
                            .any(|old| old.id == i.id)
                    })
                    .unwrap();
                assert_eq!(item.kind, Kind::Snippet);
                assert_eq!(item.content, content);
                let mut expected_metadata = original.metadata().clone();
                expected_metadata[NAMESPACE]["shelf"]["items"][&item.id] = serde_json::json!({"kind":"snippet","title":item.title,"content":content,"pinned":false});
                expected_metadata[NAMESPACE]["shelf"]["order"]
                    .as_array_mut()
                    .unwrap()
                    .push(item.id.clone().into());
                assert_eq!(captured.metadata(), &expected_metadata);
                let expected = if cut {
                    source[..start].to_owned()
                } else {
                    source.clone()
                };
                assert_eq!(ui.get_source(), expected);
                assert!(ui.get_dirty() && ui.get_can_undo());
                assert!(
                    state
                        .history
                        .borrow()
                        .project_change(&expected, false)
                        .is_some()
                );
                let mut source_only = state.history.borrow().clone();
                assert!(
                    source_only.step_visual(&expected, false).is_none(),
                    "Text-only undo cannot split a capture"
                );
                undo(&ui, false);
                assert_eq!(ui.get_source(), source);
                assert_eq!(
                    (ui.get_source_anchor(), ui.get_source_focus()),
                    (end as i32, start as i32)
                );
                assert_eq!(
                    state.project.borrow().as_ref().unwrap().entries(),
                    original.entries()
                );
                assert!(!ui.get_dirty());
                undo(&ui, true);
                assert_eq!(ui.get_source(), expected);
                assert_eq!(
                    state.project.borrow().as_ref().unwrap().entries(),
                    captured.entries()
                );
                state.checkpoint.borrow_mut().mark_saved(&expected);
                crate::reflect_checkpoint(&ui, &state.checkpoint.borrow());
                undo(&ui, false);
                assert!(ui.get_dirty());
                undo(&ui, true);
                assert!(!ui.get_dirty());
                let path = root.path().join("Captured.fountage");
                project_draft::save_copy(&captured, &expected, &path).unwrap();
                assert_eq!(Project::read(&path).unwrap().entries(), captured.entries());
                let recovery_path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
                let snapshot = recovery::read(&recovery_path).unwrap();
                assert_eq!(
                    recovery::project_from_snapshot(&recovery_path, &snapshot)
                        .unwrap()
                        .unwrap()
                        .entries(),
                    captured.entries()
                );
                for (path, entry) in original.entries() {
                    if path != std::ffi::OsStr::new("info.json")
                        && path
                            != std::ffi::OsStr::new(
                                original.metadata()[NAMESPACE]["sourceFile"]
                                    .as_str()
                                    .unwrap(),
                            )
                    {
                        assert_eq!(captured.entries().get(path), Some(entry));
                    }
                }
                ui.hide().unwrap();
            }
        }
    }
    #[test]
    fn capture_typing_and_later_shelf_edits_cannot_split_or_consume_composite_history() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = lists_fixture(0);
        let source = original.source().to_owned();
        let (ui, state) = setup(original.clone(), root.path().into());
        ui.invoke_show_shelf();
        ui.invoke_shelf_toggle(1);
        let before_capture = state.project.borrow().as_ref().unwrap().clone();
        ui.invoke_shelf_close();
        ui.invoke_select_source(source.len() as i32, source.find("Zoë").unwrap() as i32);
        ui.invoke_show_shelf();
        ui.invoke_shelf_capture(true);
        let captured = state.project.borrow().as_ref().unwrap().clone();
        let after = ui.get_source().to_string();
        key(&ui, "!".into());
        assert_eq!(ui.get_source(), format!("{after}!"));
        undo(&ui, false);
        assert_eq!(ui.get_source(), after);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().metadata(),
            captured.metadata()
        );
        undo(&ui, false);
        assert_eq!(ui.get_source(), source);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            before_capture.entries()
        );
        undo(&ui, true);
        assert_eq!(ui.get_source(), after);
        undo(&ui, true);
        assert_eq!(ui.get_source(), format!("{after}!"));
        undo(&ui, false); // Return to capture result before adding a Shelf note.
        ui.invoke_show_shelf();
        ui.invoke_shelf_kind_requested(1);
        new_action(&ui);
        draft_text(&ui, 0, "Later note");
        draft_text(&ui, 1, "Keep later metadata");
        apply_action(&ui);
        let later = state.project.borrow().as_ref().unwrap().clone();
        ui.invoke_shelf_close();
        undo(&ui, false); // Must reject capture Undo while later metadata exists.
        assert_eq!(ui.get_source(), after);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            later.entries()
        );
        assert!(!ui.get_shelf_error().is_empty());
        ui.invoke_show_shelf();
        ui.invoke_shelf_history(false);
        ui.invoke_shelf_close();
        assert_eq!(
            state.project.borrow().as_ref().unwrap().metadata(),
            captured.metadata()
        );
        undo(&ui, false);
        assert_eq!(ui.get_source(), source);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            before_capture.entries()
        );
        ui.invoke_show_shelf();
        ui.invoke_shelf_history(false);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        ui.invoke_shelf_history(true);
        ui.invoke_shelf_close();
        undo(&ui, true);
        assert_eq!(ui.get_source(), after);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            captured.entries()
        );
        ui.hide().unwrap();
    }
    #[test]
    fn failed_capture_and_composite_undo_redo_recovery_do_not_adopt_partial_changes() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = lists_fixture(0);
        let source = original.source().to_owned();
        let (ui, state) = setup(original.clone(), root.path().into());
        let blocked = root.path().join("blocked");
        std::fs::write(&blocked, "keep").unwrap();
        let fail = Rc::new(Cell::new(true));
        let switch = fail.clone();
        let good = root.path().to_owned();
        install(&ui, state.clone(), move || {
            Ok(if switch.get() {
                blocked.clone()
            } else {
                good.clone()
            })
        });
        ui.invoke_select_source(source.len() as i32, source.find("Zoë").unwrap() as i32);
        ui.invoke_show_shelf();
        let recovery_path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
        ui.invoke_shelf_capture(true);
        assert!(ui.get_shelf_open());
        assert_eq!(ui.get_source(), source);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        assert!(!ui.get_can_undo());
        assert_eq!(
            state.recovery.borrow().as_ref().unwrap().path(),
            recovery_path
        );
        fail.set(false);
        ui.invoke_shelf_capture(true);
        let captured = state.project.borrow().as_ref().unwrap().clone();
        let after = ui.get_source().to_string();
        let recovery_path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
        fail.set(true);
        undo(&ui, false);
        assert_eq!(ui.get_source(), after);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            captured.entries()
        );
        assert_eq!(
            state.recovery.borrow().as_ref().unwrap().path(),
            recovery_path
        );
        fail.set(false);
        undo(&ui, false);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        fail.set(true);
        undo(&ui, true);
        assert_eq!(ui.get_source(), source);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        fail.set(false);
        undo(&ui, true);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            captured.entries()
        );
        ui.hide().unwrap();
    }
    #[test]
    fn insertion_from_each_shelf_kind_replaces_selection_and_restores_it_on_undo_all_categories() {
        i_slint_backend_testing::init_no_event_loop();
        for category in 0..8 {
            for kind in 0..3 {
                let root = tempfile::tempdir().unwrap();
                let original = lists_fixture(category);
                let source = original.source().to_owned();
                let (ui, state) = setup(original.clone(), root.path().into());
                let a = source.find("Zoë").unwrap();
                let f = source.find(" waits").unwrap();
                ui.invoke_select_source(f as i32, a as i32);
                ui.invoke_show_shelf();
                ui.invoke_shelf_kind_requested(kind);
                let id = match kind {
                    1 => "note",
                    2 => "snippet",
                    _ => "plan",
                };
                let item = shelf::items(original.metadata())
                    .unwrap()
                    .into_iter()
                    .find(|i| i.id == id)
                    .unwrap();
                let expected = shelf::insertion(&source, &source, f, a, &item.content).unwrap();
                let index = list_index(&ui, id);
                assert!(
                    ui.get_shelf_rows()
                        .row_data(index as usize)
                        .unwrap()
                        .can_insert
                );
                assert!(ui.get_shelf_replace_selection());
                ui.invoke_shelf_insert(index);
                assert!(!ui.get_shelf_open() && !ui.get_file_dialog_open());
                assert_eq!(ui.get_source(), expected.source);
                assert!(ui.get_dirty() && ui.get_can_undo());
                assert_eq!(
                    (ui.get_source_anchor(), ui.get_source_focus()),
                    (expected.anchor as i32, expected.focus as i32)
                );
                assert_eq!(
                    state.project.borrow().as_ref().unwrap().metadata(),
                    original.metadata()
                );
                undo(&ui, false);
                assert_eq!(ui.get_source(), source);
                assert_eq!(
                    (ui.get_source_anchor(), ui.get_source_focus()),
                    (f as i32, a as i32)
                );
                assert!(!ui.get_dirty());
                undo(&ui, true);
                assert_eq!(ui.get_source(), expected.source);
                let project = state.project.borrow().as_ref().unwrap().clone();
                let path = root.path().join("Inserted.fountage");
                project_draft::save_copy(&project, ui.get_source().as_str(), &path).unwrap();
                let reopened = Project::read(&path).unwrap();
                assert_eq!(reopened.source(), expected.source);
                let recovery_path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
                let snapshot = recovery::read(&recovery_path).unwrap();
                assert_eq!(
                    recovery::project_from_snapshot(&recovery_path, &snapshot)
                        .unwrap()
                        .unwrap()
                        .entries(),
                    reopened.entries()
                );
                for (path, entry) in original.entries() {
                    if path
                        != std::ffi::OsStr::new(
                            original.metadata()[NAMESPACE]["sourceFile"]
                                .as_str()
                                .unwrap(),
                        )
                    {
                        assert_eq!(reopened.entries().get(path), Some(entry));
                    }
                }
                ui.hide().unwrap();
            }
        }
    }
    #[test]
    fn insertion_of_identical_text_needs_no_recovery_and_formatted_insert_keeps_source_undo() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let mut original = lists_fixture(0);
        original.replace_source("Opaque note").unwrap();
        let (ui, state) = setup(original.clone(), root.path().into());
        let blocked = root.path().join("blocked");
        std::fs::write(&blocked, "keep").unwrap();
        install(&ui, state.clone(), move || Ok(blocked.clone()));
        ui.invoke_select_source(11, 0);
        ui.invoke_show_shelf();
        ui.invoke_shelf_kind_requested(1);
        let recovery_path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
        ui.invoke_shelf_insert(list_index(&ui, "note"));
        assert!(!ui.get_shelf_open());
        assert_eq!(ui.get_source(), "Opaque note");
        assert!(!ui.get_dirty() && !ui.get_can_undo());
        assert_eq!(
            state.recovery.borrow().as_ref().unwrap().path(),
            recovery_path
        );
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        ui.hide().unwrap();

        let original = lists_fixture(0);
        let (ui, state) = setup(original.clone(), root.path().join("formatted"));
        crate::formatted_live::install(&ui);
        ui.set_formatted_active(true);
        ui.invoke_select_source(0, 0);
        ui.invoke_show_shelf();
        ui.invoke_shelf_kind_requested(2);
        assert!(!ui.get_shelf_replace_selection());
        ui.invoke_shelf_insert(list_index(&ui, "snippet"));
        assert_eq!(
            ui.get_source(),
            format!("Opaque snippet{}", original.source())
        );
        undo(&ui, false);
        assert_eq!(ui.get_source(), original.source());
        assert!(!ui.get_dirty());
        assert_eq!(
            state.project.borrow().as_ref().unwrap().metadata(),
            original.metadata()
        );
        ui.hide().unwrap();
    }
    #[test]
    fn insertion_preview_empty_form_and_pending_metadata_undo_guards_do_not_retarget() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = lists_fixture(0);
        let (ui, state) = setup(original.clone(), root.path().into());
        ui.set_preview_open(true);
        ui.invoke_show_shelf();
        let index = list_index(&ui, "plan");
        assert!(
            !ui.get_shelf_rows()
                .row_data(index as usize)
                .unwrap()
                .can_insert
        );
        ui.invoke_shelf_insert(index);
        assert!(ui.get_shelf_open());
        assert_eq!(ui.get_source(), original.source());
        ui.invoke_shelf_close();
        ui.set_preview_open(false);
        ui.invoke_select_source(0, 0);
        ui.invoke_show_shelf();
        let empty = list_index(&ui, "last");
        assert!(
            !ui.get_shelf_rows()
                .row_data(empty as usize)
                .unwrap()
                .can_insert
        );
        ui.invoke_shelf_insert(empty);
        assert!(ui.get_shelf_open());
        ui.invoke_shelf_edit(index);
        ui.invoke_shelf_insert(index);
        assert!(ui.get_shelf_draft_open());
        ui.invoke_shelf_draft_action(false);
        ui.invoke_shelf_list_action(list_index(&ui, "plan"), 0);
        assert!(ui.invoke_shelf_queue_history(false));
        ui.invoke_shelf_insert(list_index(&ui, "plan"));
        assert!(ui.get_shelf_open());
        assert!(!ui.get_shelf_error().is_empty());
        assert_eq!(ui.get_source(), original.source());
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        assert!(!ui.get_dirty() && !ui.get_can_undo());
        ui.hide().unwrap();
    }
    #[test]
    fn insertion_rejects_stale_source_metadata_and_failed_recovery_without_consuming_source_history()
     {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = lists_fixture(0);
        let (ui, state) = setup(original.clone(), root.path().into());
        let source = original.source().to_owned();
        ui.invoke_select_source(0, 0);
        ui.invoke_show_shelf();
        ui.set_source(format!("{source}External text").into());
        ui.invoke_shelf_insert(list_index(&ui, "plan"));
        assert!(ui.get_shelf_open());
        assert!(!ui.get_shelf_error().is_empty());
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        ui.set_source(source.clone().into());
        ui.invoke_shelf_close();
        let blocked = root.path().join("blocked");
        std::fs::write(&blocked, "keep").unwrap();
        install(&ui, state.clone(), move || Ok(blocked.clone()));
        ui.invoke_select_source(0, 0);
        ui.invoke_show_shelf();
        let recovery = state.recovery.borrow().as_ref().unwrap().path().to_owned();
        ui.invoke_shelf_insert(list_index(&ui, "plan"));
        assert!(ui.get_shelf_open());
        assert!(!ui.get_shelf_error().is_empty());
        assert_eq!(ui.get_source(), source);
        assert!(!ui.get_dirty() && !ui.get_can_undo());
        assert_eq!(state.recovery.borrow().as_ref().unwrap().path(), recovery);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        ui.invoke_shelf_close();
        install(&ui, state.clone(), {
            let root = root.path().to_owned();
            move || Ok(root.clone())
        });
        ui.invoke_show_shelf();
        let mut changed = original.clone();
        changed
            .edit_shelf(
                original.metadata(),
                &Operation::Add(Item::new(
                    Kind::Note,
                    "External metadata".into(),
                    "keep".into(),
                )),
            )
            .unwrap();
        state
            .checkpoint
            .borrow_mut()
            .set_metadata(Some(changed.metadata()));
        *state.project.borrow_mut() = Some(changed.clone());
        ui.invoke_shelf_insert(list_index(&ui, "plan"));
        assert!(ui.get_shelf_open());
        assert!(!ui.get_shelf_error().is_empty());
        assert_eq!(ui.get_source(), source);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            changed.entries()
        );
        ui.hide().unwrap();
    }
    #[test]
    fn independent_checklists_toggle_history_save_recover_and_restore_selection_in_all_categories()
    {
        i_slint_backend_testing::init_no_event_loop();
        let directory = tempfile::tempdir().unwrap();
        for category in 0..8 {
            let original = fixture(category);
            let source = original.source().to_owned();
            let (ui, state) = setup(
                original.clone(),
                directory.path().join(category.to_string()),
            );
            ui.invoke_select_source(18, 3);
            ui.invoke_show_shelf();
            assert!(ui.get_shelf_open() && ui.get_file_dialog_open());
            assert_eq!(ui.get_shelf_status(), "1 checklist · 1 open task");
            assert_eq!(checkbox(&ui).accessible_checked(), Some(false));
            for position in [-17, -7, -8, -9, -4, -12, -13, -14, -5, -15] {
                key(&ui, Key::Tab.into());
                assert_eq!(
                    ui.get_shelf_focus_position(),
                    position,
                    "Tab follows type selectors, New, visibility controls, then Search"
                );
            }
            key(&ui, Key::Tab.into()); // Pin the sole checklist.
            assert_eq!(
                ui.get_shelf_focus_position(),
                5 * ui.get_shelf_rows().row_count() as i32
            );
            key(&ui, Key::Tab.into()); // Edit.
            key(&ui, Key::Tab.into()); // Add Task.
            key(&ui, Key::Tab.into());
            assert_eq!(ui.get_shelf_focus_position(), 1, "Tab stays inside Shelf");
            key(&ui, " ".into());
            assert_eq!(checkbox(&ui).accessible_checked(), Some(true));
            key(&ui, " ".into());
            assert_eq!(
                checkbox(&ui).accessible_checked(),
                Some(false),
                "row refresh retains checkbox keyboard focus"
            );
            checkbox(&ui).invoke_accessible_default_action();
            assert_eq!(checkbox(&ui).accessible_checked(), Some(true));
            assert_eq!(ui.get_source(), source);
            assert!(ui.get_dirty());
            assert!(ui.get_shelf_can_undo());
            assert!(
                !ui.get_can_undo(),
                "metadata toggle never enters source history"
            );
            let current = state.project.borrow().as_ref().unwrap().clone();
            let saved = directory.path().join(format!("Saved {category}.fountage"));
            project_draft::save_copy(&current, &source, &saved).unwrap();
            assert_eq!(Project::read(&saved).unwrap().entries(), current.entries());
            let baseline = state.recovery.borrow().as_ref().unwrap().path().to_owned();
            let snapshot = recovery::read(&baseline).unwrap();
            assert_eq!(
                recovery::project_from_snapshot(&baseline, &snapshot)
                    .unwrap()
                    .unwrap()
                    .entries(),
                current.entries()
            );
            for (path, value) in original.entries() {
                if path != std::ffi::OsStr::new("info.json") {
                    assert_eq!(current.entries().get(path), Some(value));
                }
            }
            state.checkpoint.borrow_mut().mark_saved(&source);
            crate::reflect_checkpoint(&ui, &state.checkpoint.borrow());
            assert!(!ui.get_dirty());
            undo(&ui, false);
            assert_eq!(checkbox(&ui).accessible_checked(), Some(false));
            assert!(
                ui.get_dirty(),
                "Undo after Save changes metadata checkpoint"
            );
            undo(&ui, true);
            assert_eq!(checkbox(&ui).accessible_checked(), Some(true));
            assert!(!ui.get_dirty(), "Redo to saved metadata is clean");
            key(&ui, Key::Escape.into());
            assert!(!ui.get_shelf_open() && !ui.get_file_dialog_open());
            assert_eq!((ui.get_source_anchor(), ui.get_source_focus()), (18, 3));
            ui.invoke_toggle_preview();
            ui.invoke_show_shelf();
            ui.invoke_shelf_close();
            assert!(ui.get_preview_open(), "Shelf must retain Preview mode");
            ui.invoke_show_shelf();
            assert!(
                ui.get_shelf_can_undo(),
                "reopening same document retains history"
            );
            ui.invoke_shelf_reset();
            assert!(
                ui.get_file_dialog_open(),
                "reset cannot release a native picker lock"
            );
            ui.set_file_dialog_open(false);
            // An adopted copy can have the same project ID. Explicit reset is required.
            *state.project.borrow_mut() = Some(original.clone());
            state
                .checkpoint
                .borrow_mut()
                .adopt_saved(&source, Some(original.metadata()));
            ui.invoke_show_shelf();
            assert!(!ui.get_shelf_can_undo());
            ui.invoke_perform_history(false);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            ui.hide().unwrap();
        }
    }
    #[test]
    fn failed_or_stale_checklist_writes_reset_optimistic_widget_without_consuming_history() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let (ui, state) = setup(fixture(0), root.path().join("baseline"));
        let blocked = root.path().join("blocked");
        std::fs::write(&blocked, "keep").unwrap();
        install(&ui, state.clone(), move || Ok(blocked.clone()));
        let original = state.project.borrow().as_ref().unwrap().clone();
        let baseline = state.recovery.borrow().as_ref().unwrap().path().to_owned();
        ui.invoke_show_shelf();
        checkbox(&ui).invoke_accessible_default_action();
        assert_eq!(checkbox(&ui).accessible_checked(), Some(false));
        assert!(!ui.get_shelf_error().is_empty());
        assert!(!ui.get_dirty() && !ui.get_shelf_can_undo());
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        assert_eq!(state.recovery.borrow().as_ref().unwrap().path(), baseline);
        ui.invoke_shelf_toggle(-1);
        ui.invoke_shelf_toggle(999);
        assert!(!ui.get_shelf_can_undo());
        // Replace the adopted document while an old panel snapshot exists.
        *state.project.borrow_mut() = Some(fixture(1));
        checkbox(&ui).invoke_accessible_default_action();
        assert!(ui.get_shelf_error().contains("changed"));
        assert_eq!(checkbox(&ui).accessible_checked(), Some(false));
    }
    #[test]
    fn source_only_future_shelf_and_modal_composition_guards_are_read_only() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let project = fixture(0);
        let mut metadata = project.metadata().clone();
        metadata[NAMESPACE]["shelf"]["version"] = 99.into();
        let mut entries = project.entries().clone();
        entries.insert(
            "info.json".into(),
            Entry::File(serde_json::to_vec(&metadata).unwrap()),
        );
        let future = Project::from_entries(entries).unwrap();
        let (ui, state) = setup(future.clone(), root.path().to_owned());
        for guard in 0..4 {
            ui.set_project_mode(guard != 0);
            ui.set_confirm_close(guard == 1);
            ui.set_file_dialog_open(guard == 2);
            ui.set_formatted_composing(guard == 3);
            ui.invoke_show_shelf();
            assert!(!ui.get_shelf_open());
        }
        ui.set_formatted_composing(false);
        ui.set_confirm_close(false);
        ui.set_file_dialog_open(false);
        ui.invoke_show_shelf();
        assert!(ui.get_shelf_open());
        assert!(!ui.get_shelf_editable());
        assert!(ui.get_shelf_error().contains("unsupported"));
        ui.invoke_shelf_toggle(1);
        ui.invoke_perform_history(false);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            future.entries()
        );
        assert!(!ui.get_dirty());
        assert_eq!(
            state
                .history
                .borrow()
                .availability(ui.get_source().as_str()),
            (false, false)
        );
    }
    fn draft_text(ui: &Studio, index: i32, value: &str) {
        let old = if index == 0 {
            ui.get_shelf_draft_title()
        } else {
            ui.get_shelf_draft_content()
        };
        ui.invoke_select_shelf_draft_field(index, 0, old.len() as i32);
        if value.contains('\r') {
            i_slint_core::window::WindowInner::from_pub(ui.window())
                .context()
                .platform()
                .set_clipboard_text(value, i_slint_core::platform::Clipboard::DefaultClipboard);
            ui.invoke_clipboard_action(2);
        } else {
            key(ui, value.into());
        }
        assert_eq!(
            if index == 0 {
                ui.get_shelf_draft_title()
            } else {
                ui.get_shelf_draft_content()
            },
            value,
            "actual focused field accepts typing"
        );
    }
    fn new_action(ui: &Studio) {
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(
            ui,
            match ui.get_shelf_kind() {
                1 => "New Note…",
                2 => "New Snippet…",
                _ => "New Checklist…",
            },
        )
        .next()
        .unwrap()
        .invoke_accessible_default_action();
    }
    fn apply_action(ui: &Studio) {
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(
            ui,
            if ui.get_shelf_draft_removing() {
                if ui.get_shelf_draft_task() {
                    "Remove Task"
                } else {
                    match ui.get_shelf_draft_kind() {
                        1 => "Delete Note",
                        2 => "Delete Snippet",
                        _ => "Delete Checklist",
                    }
                }
            } else if ui.get_shelf_draft_task() {
                if ui.get_shelf_draft_task_edit() {
                    "Save Task"
                } else {
                    "Add Task"
                }
            } else if ui.get_shelf_draft_existing() {
                match ui.get_shelf_draft_kind() {
                    1 => "Save Note",
                    2 => "Save Snippet",
                    _ => "Save Checklist",
                }
            } else {
                match ui.get_shelf_draft_kind() {
                    1 => "Create Note",
                    2 => "Create Snippet",
                    _ => "Create Checklist",
                }
            },
        )
        .next()
        .unwrap()
        .invoke_accessible_default_action();
    }
    #[test]
    fn notes_and_snippets_create_edit_pin_delete_undo_recover_and_save_all_categories() {
        i_slint_backend_testing::init_no_event_loop();
        for category in 0..8 {
            for kind in [1, 2] {
                let root = tempfile::tempdir().unwrap();
                let original = lists_fixture(category);
                let (ui, state) = setup(original.clone(), root.path().into());
                ui.invoke_show_shelf();
                ui.invoke_shelf_kind_requested(kind);
                assert_eq!(ui.get_shelf_kind(), kind);
                assert_eq!(
                    visible_list_ids(&ui),
                    if kind == 1 {
                        vec!["note"]
                    } else {
                        vec!["snippet"]
                    }
                );
                new_action(&ui);
                assert_eq!(ui.get_shelf_draft_kind(), kind);
                draft_text(&ui, 0, "Zoë 🌙 世界 research");
                let content = "[ ] Keep this as plain text.\r\nA second line [[raw note]].\r\n";
                draft_text(&ui, 1, content);
                ui.invoke_shelf_kind_requested(0);
                assert_eq!(
                    ui.get_shelf_kind(),
                    kind,
                    "Cannot switch away from an active form"
                );
                apply_action(&ui);
                let added = state.project.borrow().as_ref().unwrap().clone();
                let item = shelf::items(added.metadata())
                    .unwrap()
                    .into_iter()
                    .find(|i| i.title == "Zoë 🌙 世界 research")
                    .unwrap();
                assert_eq!(item.kind, selected_kind(kind));
                assert_eq!(item.content, content);
                assert!(
                    !ui.get_shelf_rows().iter().any(|r| r.task),
                    "Stored note/snippet text is not an independent checklist"
                );
                search_text(&ui, "research");
                assert_eq!(
                    visible_list_ids(&ui).as_slice(),
                    std::slice::from_ref(&item.id)
                );
                let index = list_index(&ui, &item.id);
                ui.invoke_shelf_add_task(index);
                ui.invoke_shelf_edit_task(index);
                assert!(
                    !ui.get_shelf_draft_open(),
                    "Task commands cannot reinterpret note text"
                );
                ui.invoke_shelf_list_action(index, 0);
                assert!(
                    shelf::items(state.project.borrow().as_ref().unwrap().metadata())
                        .unwrap()
                        .iter()
                        .find(|i| i.id == item.id)
                        .unwrap()
                        .pinned
                );
                ui.invoke_shelf_edit(list_index(&ui, &item.id));
                draft_text(&ui, 0, "Revised research");
                draft_text(&ui, 1, "Revised 🌙\r\n");
                apply_action(&ui);
                let edited = state.project.borrow().as_ref().unwrap().clone();
                ui.invoke_shelf_edit(list_index(&ui, &item.id));
                draft_text(&ui, 0, "Unsaved title kept");
                ui.invoke_shelf_remove_requested();
                assert!(ui.get_shelf_draft_removing());
                assert_eq!(ui.get_shelf_draft_removal_preview(), "Revised 🌙\r\n");
                ui.invoke_shelf_draft_action(false);
                assert_eq!(ui.get_shelf_draft_title(), "Unsaved title kept");
                ui.invoke_shelf_remove_requested();
                apply_action(&ui);
                let deleted = state.project.borrow().as_ref().unwrap().clone();
                assert!(
                    !shelf::items(deleted.metadata())
                        .unwrap()
                        .iter()
                        .any(|i| i.id == item.id)
                );
                undo(&ui, false);
                assert_eq!(
                    state.project.borrow().as_ref().unwrap().entries(),
                    edited.entries()
                );
                undo(&ui, true);
                assert_eq!(
                    state.project.borrow().as_ref().unwrap().entries(),
                    deleted.entries()
                );
                let path = root
                    .path()
                    .join(format!("Shelf {category}-{kind}.fountage"));
                project_draft::save_copy(&deleted, deleted.source(), &path).unwrap();
                assert_eq!(Project::read(&path).unwrap().entries(), deleted.entries());
                let recovery_path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
                let snapshot = recovery::read(&recovery_path).unwrap();
                assert_eq!(
                    recovery::project_from_snapshot(&recovery_path, &snapshot)
                        .unwrap()
                        .unwrap()
                        .entries(),
                    deleted.entries()
                );
                assert_eq!(deleted.source(), original.source());
                for (path, entry) in original.entries() {
                    if path != std::ffi::OsStr::new("info.json") {
                        assert_eq!(deleted.entries().get(path), Some(entry));
                    }
                }
                // Removing the newly created item returns every saved field to the baseline,
                // except the optional pin-order field created by the pin operation (if any).
                let mut expected = original.metadata().clone();
                if deleted.metadata()[NAMESPACE]["shelf"]
                    .get("pinnedOrder")
                    .is_some()
                {
                    expected[NAMESPACE]["shelf"]["pinnedOrder"] = serde_json::json!([]);
                }
                assert_eq!(deleted.metadata(), &expected);
                ui.hide().unwrap();
            }
        }
    }
    #[test]
    fn shelf_item_kind_transitions_and_failed_note_writes_preserve_scope_and_drafts() {
        i_slint_backend_testing::init_no_event_loop();
        for kind in [1, 2] {
            let root = tempfile::tempdir().unwrap();
            let original = lists_fixture(0);
            let (ui, state) = setup(original.clone(), root.path().into());
            ui.invoke_show_shelf();
            search_text(&ui, "revision");
            assert!(ui.invoke_shelf_queue_history(false));
            let old_session = ui.get_shelf_session();
            ui.invoke_shelf_kind_requested(kind);
            assert_ne!(ui.get_shelf_session(), old_session);
            assert!(ui.get_shelf_query().is_empty());
            i_slint_backend_testing::testing_backend::mock_elapsed_time(2);
            assert!(
                !ui.get_shelf_query_focused(),
                "Old query correction cannot steal type-switch focus"
            );
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            ui.invoke_shelf_close();
            let blocked = root.path().join("blocked");
            std::fs::write(&blocked, "keep").unwrap();
            install(&ui, state.clone(), move || Ok(blocked.clone()));
            ui.invoke_show_shelf();
            ui.invoke_shelf_kind_requested(kind);
            new_action(&ui);
            draft_text(&ui, 0, "Retain unsaved note or snippet");
            draft_text(&ui, 1, "Zoë 🌙 text");
            let recovery_path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
            apply_action(&ui);
            assert!(ui.get_shelf_draft_open());
            assert_eq!(ui.get_shelf_draft_title(), "Retain unsaved note or snippet");
            assert_eq!(ui.get_shelf_draft_content(), "Zoë 🌙 text");
            assert!(!ui.get_shelf_error().is_empty());
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            assert_eq!(
                state.recovery.borrow().as_ref().unwrap().path(),
                recovery_path
            );
            assert!(!ui.get_dirty());
            ui.hide().unwrap();
        }
    }
    #[test]
    fn checklist_forms_create_edit_cancel_field_history_and_metadata_history_across_all_categories()
    {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        for category in 0..8 {
            let original = fixture(category);
            let source = original.source().to_owned();
            let (ui, state) = setup(
                original.clone(),
                root.path().join(format!("recovery-{category}")),
            );
            ui.invoke_select_source(18, 3);
            ui.invoke_show_shelf();
            new_action(&ui);
            assert!(ui.get_shelf_draft_open() && !ui.get_shelf_draft_existing());
            draft_text(&ui, 0, "Launch Zoë 🌙 世界");
            undo(&ui, false);
            assert_eq!(ui.get_shelf_draft_title(), "Untitled checklist");
            undo(&ui, true);
            assert_eq!(ui.get_shelf_draft_title(), "Launch Zoë 🌙 世界");
            let content =
                "[ ] Review Zoë 🌙 [[private]]\r\n[x] Send draft\r\nKeep this freeform note.\r\n";
            draft_text(&ui, 1, content);
            assert!(
                !ui.get_dirty(),
                "staged fields never modify the saved project"
            );
            ui.invoke_shelf_draft_action(false);
            assert!(!ui.get_shelf_draft_open() && ui.get_shelf_open());
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            assert!(!ui.get_shelf_can_undo());
            new_action(&ui);
            draft_text(&ui, 0, "Launch Zoë 🌙 世界");
            draft_text(&ui, 1, content);
            apply_action(&ui);
            assert!(!ui.get_shelf_draft_open());
            assert!(ui.get_dirty() && ui.get_shelf_can_undo());
            assert_eq!(ui.get_source(), source);
            assert!(
                !ui.get_can_undo(),
                "form history is separate from source history"
            );
            let created = state.project.borrow().as_ref().unwrap().clone();
            let new_item = shelf::items(created.metadata())
                .unwrap()
                .into_iter()
                .find(|i| i.title == "Launch Zoë 🌙 世界")
                .unwrap();
            assert_eq!(new_item.content, content);
            undo(&ui, false);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            undo(&ui, true);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                created.entries()
            );
            let edit = i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                &ui,
                "Edit checklist Revision plan",
            )
            .next()
            .unwrap();
            edit.invoke_accessible_default_action();
            assert!(ui.get_shelf_draft_existing());
            draft_text(&ui, 0, "Revised plan");
            let revised = "[x] Edited task 🌙 [[keep]]\r\n[ ] New task\r\n";
            draft_text(&ui, 1, revised);
            // Buttons own no field history: Undo cannot touch committed metadata.
            ui.invoke_move_shelf_draft_focus(false);
            assert_eq!(ui.get_shelf_draft_field_index(), -1);
            ui.invoke_perform_history(false);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                created.entries()
            );
            apply_action(&ui);
            assert!(!ui.get_shelf_draft_open());
            let edited = state.project.borrow().as_ref().unwrap().clone();
            assert_eq!(
                edited.metadata()[NAMESPACE]["shelf"]["items"]["plan"]["unknown"],
                7
            );
            assert_eq!(
                edited.metadata()[NAMESPACE]["shelf"]["items"]["plan"]["content"],
                revised
            );
            assert_eq!(edited.metadata()["foreign"], original.metadata()["foreign"]);
            for (path, entry) in original.entries() {
                if path != std::ffi::OsStr::new("info.json") {
                    assert_eq!(edited.entries().get(path), Some(entry));
                }
            }
            let path = root.path().join(format!("Forms {category}.fountage"));
            project_draft::save_copy(&edited, &source, &path).unwrap();
            assert_eq!(Project::read(&path).unwrap().entries(), edited.entries());
            state.checkpoint.borrow_mut().mark_saved(&source);
            crate::reflect_checkpoint(&ui, &state.checkpoint.borrow());
            undo(&ui, false);
            assert!(ui.get_dirty());
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                created.entries()
            );
            undo(&ui, true);
            assert!(!ui.get_dirty());
            let recovery_path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
            let snapshot = recovery::read(&recovery_path).unwrap();
            assert_eq!(
                recovery::project_from_snapshot(&recovery_path, &snapshot)
                    .unwrap()
                    .unwrap()
                    .entries(),
                edited.entries()
            );
            // Saving an unchanged existing form is a byte-preserving no-op.
            let index = (0..ui.get_shelf_rows().row_count())
                .find(|i| ui.get_shelf_rows().row_data(*i).unwrap().title == "Revised plan")
                .unwrap();
            ui.invoke_shelf_edit(index as i32);
            apply_action(&ui);
            assert!(!ui.get_dirty());
            assert_eq!(
                state.recovery.borrow().as_ref().unwrap().path(),
                recovery_path
            );
            ui.invoke_shelf_close();
            assert_eq!((ui.get_source_anchor(), ui.get_source_focus()), (18, 3));
            ui.hide().unwrap();
        }
    }
    #[test]
    fn checklist_form_validation_failure_and_stale_save_keep_draft_and_project() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = fixture(0);
        let (ui, state) = setup(original.clone(), root.path().join("recovery"));
        ui.invoke_show_shelf();
        new_action(&ui);
        draft_text(&ui, 0, "   ");
        apply_action(&ui);
        assert!(ui.get_shelf_draft_open());
        assert!(!ui.get_shelf_error().is_empty());
        assert_eq!(ui.get_shelf_draft_title(), "   ");
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        draft_text(&ui, 0, "Keep draft");
        draft_text(&ui, 1, "[ ] Keep all text");
        // A foreign metadata change invalidates the exact full-metadata draft snapshot.
        let mut changed = original.clone();
        changed
            .edit_shelf(
                original.metadata(),
                &Operation::Add(Item::new(Kind::Note, "External note".into(), "keep".into())),
            )
            .unwrap();
        state
            .checkpoint
            .borrow_mut()
            .set_metadata(Some(changed.metadata()));
        *state.project.borrow_mut() = Some(changed.clone());
        apply_action(&ui);
        assert!(ui.get_shelf_draft_open());
        assert_eq!(ui.get_shelf_draft_content(), "[ ] Keep all text");
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            changed.entries()
        );
        apply_action(&ui); // Refresh cannot silently rebase an old form.
        assert!(ui.get_shelf_draft_open());
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            changed.entries()
        );
        ui.invoke_shelf_draft_action(false);
        ui.invoke_shelf_close();
        let blocked = root.path().join("blocked");
        std::fs::write(&blocked, "keep").unwrap();
        install(&ui, state.clone(), move || Ok(blocked.clone()));
        ui.invoke_show_shelf();
        new_action(&ui);
        draft_text(&ui, 0, "Unsaved form");
        draft_text(&ui, 1, "[ ] Never lose this");
        apply_action(&ui);
        assert!(ui.get_shelf_draft_open());
        assert_eq!(ui.get_shelf_draft_title(), "Unsaved form");
        assert_eq!(ui.get_shelf_draft_content(), "[ ] Never lose this");
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            changed.entries()
        );
        ui.invoke_shelf_draft_action(false);
        assert!(!ui.get_shelf_draft_open());
    }
    #[test]
    fn checklist_form_composition_guards_save_cancel_history_and_focus() {
        use i_slint_core::input::{InternalKeyEvent, KeyEvent, KeyEventType};
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = fixture(0);
        let (ui, state) = setup(original.clone(), root.path().to_owned());
        ui.invoke_show_shelf();
        new_action(&ui);
        ui.window()
            .dispatch_event(WindowEvent::internal(InternalKeyEvent {
                key_event: KeyEvent::default(),
                event_type: KeyEventType::UpdateComposition,
                preedit_text: "日本".into(),
                ..Default::default()
            }));
        assert!(ui.get_shelf_draft_composing());
        ui.invoke_shelf_draft_action(true);
        ui.invoke_shelf_draft_action(false);
        ui.invoke_perform_history(false);
        ui.invoke_move_shelf_draft_focus(false);
        assert!(ui.get_shelf_draft_open());
        assert_eq!(ui.get_shelf_draft_field_index(), 0);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        assert!(!ui.get_shelf_error().is_empty());
        ui.window()
            .dispatch_event(WindowEvent::internal(InternalKeyEvent {
                key_event: KeyEvent::default(),
                event_type: KeyEventType::CommitComposition,
                preedit_text: "".into(),
                ..Default::default()
            }));
        ui.invoke_shelf_draft_action(false);
        assert!(!ui.get_shelf_draft_open());
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
    }
    fn add_task_action(ui: &Studio) {
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(
            ui,
            "Add task to Revision plan",
        )
        .next()
        .expect("checklist Add Task control")
        .invoke_accessible_default_action();
    }
    #[test]
    fn direct_task_entry_cancel_return_field_history_metadata_history_and_persistence_all_categories()
     {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        for category in 0..8 {
            let original = fixture(category);
            let source = original.source().to_owned();
            let (ui, state) = setup(
                original.clone(),
                root.path().join(format!("recovery-{category}")),
            );
            ui.invoke_select_source(18, 3);
            ui.invoke_show_shelf();
            add_task_action(&ui);
            assert!(ui.get_shelf_draft_open() && ui.get_shelf_draft_task());
            assert_eq!(ui.get_shelf_draft_target(), "Revision plan");
            assert_eq!(ui.get_shelf_draft_title(), "");
            assert_eq!(ui.get_shelf_draft_field_index(), 0);
            draft_text(&ui, 0, "Cancel this task 🌙");
            undo(&ui, false);
            assert_eq!(ui.get_shelf_draft_title(), "");
            undo(&ui, true);
            assert_eq!(ui.get_shelf_draft_title(), "Cancel this task 🌙");
            key(&ui, Key::Escape.into());
            assert!(!ui.get_shelf_draft_open() && ui.get_shelf_open());
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            assert!(!ui.get_shelf_can_undo());
            add_task_action(&ui);
            draft_text(&ui, 0, "  Check Zoë 🌙 世界  ");
            key(&ui, Key::Tab.into());
            assert_eq!(
                ui.get_shelf_draft_field_index(),
                -1,
                "Task Tab goes to Cancel, skipping hidden checklist text"
            );
            key(&ui, Key::Tab.into());
            key(&ui, Key::Tab.into());
            assert_eq!(
                ui.get_shelf_draft_field_index(),
                0,
                "Task focus cycles through three controls"
            );
            key(&ui, Key::Return.into());
            assert!(!ui.get_shelf_draft_open());
            let added = state.project.borrow().as_ref().unwrap().clone();
            let content =
                "[ ] Review Zoë 🌙 [[private]]\r\n[x] Send draft\r\n[ ] Check Zoë 🌙 世界\r\n";
            let mut expected = original.metadata().clone();
            expected[NAMESPACE]["shelf"]["items"]["plan"]["content"] = content.into();
            assert_eq!(added.metadata(), &expected);
            assert_eq!(added.source(), source);
            assert!(ui.get_dirty() && !ui.get_can_undo());
            assert_eq!(
                ui.get_shelf_focus_position(),
                ui.get_shelf_rows().row_count() as i32,
                "return focus targets same checklist Add Task"
            );
            i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
            key(&ui, Key::Return.into());
            assert!(
                ui.get_shelf_draft_open(),
                "Return can reopen Add Task for the same checklist"
            );
            assert_eq!(ui.get_shelf_draft_title(), "");
            draft_text(&ui, 0, "Second task");
            key(&ui, Key::Return.into());
            let twice = state.project.borrow().as_ref().unwrap().clone();
            undo(&ui, false);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                added.entries()
            );
            undo(&ui, true);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                twice.entries()
            );
            let path = root.path().join(format!("Tasks {category}.fountage"));
            project_draft::save_copy(&twice, &source, &path).unwrap();
            assert_eq!(Project::read(&path).unwrap().entries(), twice.entries());
            state.checkpoint.borrow_mut().mark_saved(&source);
            crate::reflect_checkpoint(&ui, &state.checkpoint.borrow());
            assert!(!ui.get_dirty());
            undo(&ui, false);
            assert!(ui.get_dirty());
            undo(&ui, true);
            assert!(!ui.get_dirty());
            let recovery_path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
            let snapshot = recovery::read(&recovery_path).unwrap();
            assert_eq!(
                recovery::project_from_snapshot(&recovery_path, &snapshot)
                    .unwrap()
                    .unwrap()
                    .entries(),
                twice.entries()
            );
            for (path, value) in original.entries() {
                if path != std::ffi::OsStr::new("info.json") {
                    assert_eq!(twice.entries().get(path), Some(value));
                }
            }
            ui.invoke_shelf_close();
            assert_eq!((ui.get_source_anchor(), ui.get_source_focus()), (18, 3));
            ui.hide().unwrap();
        }
    }
    #[test]
    fn direct_task_invalid_markup_stale_and_failed_recovery_keep_input_without_edits() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = fixture(0);
        let (ui, state) = setup(original.clone(), root.path().join("recovery"));
        ui.invoke_show_shelf();
        add_task_action(&ui);
        key(&ui, Key::Return.into());
        assert!(ui.get_shelf_draft_open());
        assert!(ui.get_shelf_error().contains("one line"));
        draft_text(&ui, 0, "[[hidden task]]");
        key(&ui, Key::Return.into());
        assert!(ui.get_shelf_draft_open());
        assert_eq!(ui.get_shelf_draft_title(), "[[hidden task]]");
        assert!(ui.get_shelf_error().contains("hidden"));
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        // Multiline/null input at the API boundary must reject, even if a native
        // single-line input would normally filter it before dispatch.
        for invalid in ["first\nsecond", "bad\0task", "\u{2028}", "  "] {
            ui.set_shelf_draft_title(invalid.into());
            ui.invoke_shelf_draft_action(true);
            assert!(ui.get_shelf_draft_open());
            assert_eq!(ui.get_shelf_draft_title(), invalid);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
        }
        ui.invoke_shelf_draft_action(false);
        ui.invoke_shelf_close();
        let blocked = root.path().join("blocked");
        std::fs::write(&blocked, "keep").unwrap();
        install(&ui, state.clone(), move || Ok(blocked.clone()));
        ui.invoke_show_shelf();
        add_task_action(&ui);
        draft_text(&ui, 0, "Keep draft on failure");
        key(&ui, Key::Return.into());
        assert!(ui.get_shelf_draft_open());
        assert_eq!(ui.get_shelf_draft_title(), "Keep draft on failure");
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        assert!(!ui.get_dirty());
        let changed = fixture(1);
        *state.project.borrow_mut() = Some(changed.clone());
        ui.invoke_shelf_draft_action(true);
        assert!(ui.get_shelf_draft_open());
        assert!(ui.get_shelf_error().contains("changed"));
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            changed.entries()
        );
    }
    #[test]
    fn direct_task_composition_blocks_bare_return_cancel_history_and_tab() {
        use i_slint_core::input::{InternalKeyEvent, KeyEvent, KeyEventType};
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = fixture(0);
        let (ui, state) = setup(original.clone(), root.path().to_owned());
        ui.invoke_show_shelf();
        add_task_action(&ui);
        ui.window()
            .dispatch_event(WindowEvent::internal(InternalKeyEvent {
                key_event: KeyEvent::default(),
                event_type: KeyEventType::UpdateComposition,
                preedit_text: "日本".into(),
                ..Default::default()
            }));
        assert!(ui.get_shelf_draft_composing());
        key(&ui, Key::Return.into());
        key(&ui, Key::Tab.into());
        ui.invoke_shelf_draft_action(false);
        ui.invoke_perform_history(false);
        assert!(ui.get_shelf_draft_open());
        assert_eq!(ui.get_shelf_draft_field_index(), 0);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        ui.window()
            .dispatch_event(WindowEvent::internal(InternalKeyEvent {
                key_event: KeyEvent::default(),
                event_type: KeyEventType::CommitComposition,
                preedit_text: "".into(),
                ..Default::default()
            }));
        ui.invoke_shelf_draft_action(false);
        assert!(!ui.get_shelf_draft_open());
    }
    #[test]
    fn accepted_native_history_finishes_before_close_and_never_replays_afterward_all_categories() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        for category in 0..8 {
            let original = fixture(category);
            let source = original.source().to_owned();
            let (ui, state) = setup(original.clone(), root.path().join(category.to_string()));
            ui.invoke_select_source(18, 3);
            ui.invoke_show_shelf();
            checkbox(&ui).invoke_accessible_default_action();
            let saved = state.project.borrow().as_ref().unwrap().clone();
            let path = root.path().join(format!("Saved-{category}.fountage"));
            project_draft::save_copy(&saved, &source, &path).unwrap();
            state.checkpoint.borrow_mut().mark_saved(&source);
            crate::reflect_checkpoint(&ui, &state.checkpoint.borrow());
            ui.invoke_perform_history(false);
            assert!(ui.get_dirty());
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            // This is the actual admission callback used synchronously in the
            // AppKit event monitor; it must not execute history or mutate UI.
            assert!(ui.invoke_shelf_queue_history(true));
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            assert!(ui.get_dirty());
            assert!(ui.get_shelf_can_redo());
            // Escape reaches the controller before its scheduled event-loop drain.
            ui.invoke_shelf_close();
            assert!(!ui.get_shelf_open() && !ui.get_file_dialog_open());
            assert!(
                !ui.get_dirty(),
                "accepted Redo must restore saved checkpoint before close"
            );
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                saved.entries()
            );
            assert_eq!((ui.get_source_anchor(), ui.get_source_focus()), (18, 3));
            assert!(
                ui.invoke_shelf_flush_history(),
                "later native wakeup is a no-op"
            );
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                saved.entries()
            );
            assert_eq!(Project::read(&path).unwrap().entries(), saved.entries());
            let recovery_path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
            let snapshot = recovery::read(&recovery_path).unwrap();
            assert_eq!(
                recovery::project_from_snapshot(&recovery_path, &snapshot)
                    .unwrap()
                    .unwrap()
                    .entries(),
                saved.entries()
            );
            ui.invoke_show_shelf();
            // Multiple accepted commands execute in their admission order once.
            assert!(ui.invoke_shelf_queue_history(false));
            assert!(ui.invoke_shelf_queue_history(true));
            assert!(ui.invoke_shelf_queue_history(false));
            ui.invoke_shelf_close();
            assert!(ui.get_dirty());
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            ui.invoke_shelf_flush_history();
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            assert_eq!(ui.get_source(), source);
            assert!(!ui.get_can_undo());
            ui.hide().unwrap();
        }
    }
    #[test]
    fn accepted_field_history_finishes_before_save_or_focus_change_without_retargeting() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = fixture(0);
        let (ui, state) = setup(original.clone(), root.path().to_owned());
        ui.invoke_show_shelf();
        ui.invoke_shelf_edit(0);
        draft_text(&ui, 0, "Edited title");
        assert!(ui.invoke_shelf_queue_history(false));
        assert_eq!(
            ui.get_shelf_draft_title(),
            "Edited title",
            "admission only records intent"
        );
        ui.invoke_select_shelf_draft_field(1, 0, 0);
        // The native UI wakeup may occur after a pointer focus change. It must
        // still edit the admitted field and leave the later focus untouched.
        assert!(ui.invoke_shelf_flush_history());
        assert_eq!(ui.get_shelf_draft_title(), "Revision plan");
        assert_eq!(
            ui.get_shelf_draft_field_index(),
            1,
            "earlier field Undo cannot steal later field focus"
        );
        assert_eq!(
            ui.get_shelf_draft_content(),
            "[ ] Review Zoë 🌙 [[private]]\r\n[x] Send draft\r\n"
        );
        ui.invoke_select_shelf_draft_field(0, 0, 0);
        assert!(ui.invoke_shelf_queue_history(true));
        apply_action(&ui); // accepted Redo must precede reading values for Save.
        assert!(!ui.get_shelf_draft_open());
        assert_eq!(
            state.project.borrow().as_ref().unwrap().metadata()[NAMESPACE]["shelf"]["items"]["plan"]
                ["title"],
            "Edited title"
        );
        ui.invoke_shelf_flush_history();
        assert_eq!(ui.get_source(), original.source());
        // A queued Undo also precedes typing, so it cannot consume later text.
        ui.invoke_shelf_edit(0);
        draft_text(&ui, 0, "Temporary");
        assert!(ui.invoke_shelf_queue_history(false));
        key(&ui, "Z".into());
        assert!(ui.get_shelf_draft_title().as_str().ends_with('Z'));
        assert_ne!(ui.get_shelf_draft_title(), "TemporaryZ");
        ui.invoke_shelf_draft_action(false);
    }
    #[test]
    fn failed_pending_metadata_history_blocks_close_and_keeps_project_checkpoint_and_recovery() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = fixture(0);
        let (ui, state) = setup(original, root.path().join("baseline"));
        let fail = Rc::new(Cell::new(false));
        let failed = fail.clone();
        let calls = Rc::new(Cell::new(0));
        let observed = calls.clone();
        let path = root.path().join("commands");
        install(&ui, state.clone(), move || {
            observed.set(observed.get() + 1);
            if failed.get() {
                Err("Controlled recovery failure".into())
            } else {
                Ok(path.clone())
            }
        });
        ui.invoke_show_shelf();
        checkbox(&ui).invoke_accessible_default_action();
        let saved = state.project.borrow().as_ref().unwrap().clone();
        state
            .checkpoint
            .borrow_mut()
            .mark_saved(ui.get_source().as_str());
        crate::reflect_checkpoint(&ui, &state.checkpoint.borrow());
        let recovery_path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
        fail.set(true);
        let before = calls.get();
        assert!(ui.invoke_shelf_queue_history(false));
        assert!(ui.invoke_shelf_queue_history(true));
        assert_eq!(
            calls.get(),
            before,
            "native admission performs no IO or recovery callback"
        );
        ui.invoke_shelf_close();
        assert!(
            ui.get_shelf_open() && ui.get_file_dialog_open(),
            "failed Undo cannot be hidden by closing"
        );
        assert!(ui.get_shelf_error().contains("Controlled recovery failure"));
        assert!(
            ui.get_shelf_error()
                .contains("later queued commands were not applied")
        );
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            saved.entries()
        );
        assert_eq!(
            state.recovery.borrow().as_ref().unwrap().path(),
            recovery_path
        );
        assert!(!ui.get_dirty());
        assert!(ui.get_shelf_can_undo());
        let error = ui.get_shelf_error();
        assert!(ui.invoke_shelf_flush_history());
        assert_eq!(
            ui.get_shelf_error(),
            error,
            "later scheduled drains must not erase the failure"
        );
        fail.set(false);
        assert!(ui.invoke_shelf_queue_history(false));
        ui.invoke_shelf_close();
        assert!(!ui.get_shelf_open());
        assert!(ui.get_dirty());
    }
    #[test]
    fn pending_history_is_invalidated_by_document_reset_even_with_same_project_id() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = fixture(0);
        let (ui, state) = setup(original, root.path().to_owned());
        ui.invoke_show_shelf();
        checkbox(&ui).invoke_accessible_default_action();
        let adopted = state.project.borrow().as_ref().unwrap().clone();
        assert!(ui.invoke_shelf_queue_history(false));
        ui.invoke_shelf_reset();
        *state.project.borrow_mut() = Some(adopted.clone());
        state
            .checkpoint
            .borrow_mut()
            .adopt_saved(adopted.source(), Some(adopted.metadata()));
        ui.set_file_dialog_open(false);
        ui.invoke_show_shelf();
        assert!(ui.invoke_shelf_flush_history());
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            adopted.entries()
        );
        assert!(!ui.get_shelf_can_undo());
        ui.invoke_shelf_close();
        assert!(
            !ui.invoke_shelf_queue_history(false),
            "source context cannot admit Shelf history"
        );
    }
    #[test]
    fn composing_history_cannot_become_an_undo_after_the_composition_commits() {
        use i_slint_core::input::{InternalKeyEvent, KeyEvent, KeyEventType};
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let (ui, _) = setup(fixture(0), root.path().to_owned());
        ui.invoke_show_shelf();
        add_task_action(&ui);
        draft_text(&ui, 0, "Keep this");
        ui.window()
            .dispatch_event(WindowEvent::internal(InternalKeyEvent {
                key_event: KeyEvent::default(),
                event_type: KeyEventType::UpdateComposition,
                preedit_text: "日本".into(),
                ..Default::default()
            }));
        assert!(ui.get_shelf_draft_composing());
        assert!(ui.invoke_shelf_queue_history(false));
        ui.window()
            .dispatch_event(WindowEvent::internal(InternalKeyEvent {
                key_event: KeyEvent::default(),
                event_type: KeyEventType::CommitComposition,
                preedit_text: "".into(),
                ..Default::default()
            }));
        let value = ui.get_shelf_draft_title();
        assert!(!ui.invoke_shelf_flush_history());
        assert_eq!(
            ui.get_shelf_draft_title(),
            value,
            "an IME-blocked command stays blocked after commit"
        );
        assert!(ui.get_shelf_draft_open());
    }
    #[test]
    fn queued_tab_history_clipboard_and_apply_use_logical_field_order() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = fixture(0);
        let (ui, state) = setup(original.clone(), root.path().to_owned());
        ui.invoke_show_shelf();
        ui.invoke_shelf_edit(0);
        draft_text(&ui, 0, "New title");
        let old_content = ui.get_shelf_draft_content();
        draft_text(&ui, 1, "[ ] Temporary text");
        ui.invoke_select_shelf_draft_field(0, 0, 9);
        assert!(ui.invoke_shelf_queue_focus(false));
        assert!(ui.invoke_shelf_queue_history(false));
        assert_eq!(
            ui.get_shelf_draft_field_index(),
            0,
            "admission leaves focus and text unchanged"
        );
        assert_eq!(ui.get_shelf_draft_content(), "[ ] Temporary text");
        assert!(ui.invoke_shelf_flush_history());
        assert_eq!(ui.get_shelf_draft_field_index(), 1);
        assert_eq!(
            ui.get_shelf_draft_content(),
            old_content,
            "Tab then Undo belongs to the new content field"
        );
        assert_eq!(
            ui.get_shelf_draft_title(),
            "New title",
            "earlier title field must not receive Undo"
        );
        ui.invoke_select_shelf_draft_field(0, 0, 9);
        // Isolated backend clipboard only: copy title, Tab, replace content,
        // Undo paste, then apply. All commands are admitted before dispatch.
        assert!(ui.invoke_shelf_queue_clipboard(1));
        assert!(ui.invoke_shelf_queue_focus(false));
        assert!(ui.invoke_shelf_queue_clipboard(3));
        assert!(ui.invoke_shelf_queue_clipboard(2));
        assert!(ui.invoke_shelf_flush_history());
        assert_eq!(ui.get_shelf_draft_content(), "New title");
        assert_eq!(ui.get_shelf_draft_field_index(), 1);
        assert!(ui.invoke_shelf_queue_history(false));
        assert!(ui.invoke_shelf_queue_apply());
        assert!(ui.invoke_shelf_flush_history());
        assert!(!ui.get_shelf_draft_open());
        let project = state.project.borrow();
        let project = project.as_ref().unwrap();
        assert_eq!(
            project.metadata()[NAMESPACE]["shelf"]["items"]["plan"]["title"],
            "New title"
        );
        assert_eq!(
            project.metadata()[NAMESPACE]["shelf"]["items"]["plan"]["content"],
            old_content.as_str()
        );
        assert_eq!(project.source(), original.source());
    }
    #[test]
    fn queued_focus_cannot_steal_later_pointer_focus_or_apply_to_another_form() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let (ui, _) = setup(fixture(0), root.path().to_owned());
        ui.invoke_show_shelf();
        ui.invoke_shelf_edit(0);
        draft_text(&ui, 0, "Keep edited title");
        draft_text(&ui, 1, "[ ] Edited content");
        ui.invoke_select_shelf_draft_field(0, 0, 0);
        assert!(ui.invoke_shelf_queue_focus(false));
        assert!(ui.invoke_shelf_queue_history(false));
        // A later pointer action focuses Save before the UI wakeup.
        ui.invoke_focus_shelf_draft_control(3);
        assert!(ui.invoke_shelf_flush_history());
        assert_eq!(ui.get_shelf_draft_control_index(), 3);
        assert_eq!(ui.get_shelf_draft_title(), "Keep edited title");
        assert_eq!(
            ui.get_shelf_draft_content(),
            "[ ] Review Zoë 🌙 [[private]]\r\n[x] Send draft\r\n"
        );
        ui.invoke_shelf_draft_action(false);
        ui.invoke_shelf_edit(0);
        assert!(ui.invoke_shelf_flush_history());
        assert_eq!(ui.get_shelf_draft_title(), "Revision plan");
    }
    #[test]
    fn pending_apply_validation_failure_stops_batch_and_keeps_form_values() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = fixture(0);
        let (ui, state) = setup(original.clone(), root.path().to_owned());
        ui.invoke_show_shelf();
        new_action(&ui);
        draft_text(&ui, 0, "   ");
        assert!(ui.invoke_shelf_queue_apply());
        assert!(ui.invoke_shelf_queue_history(false));
        ui.invoke_shelf_close();
        assert!(ui.get_shelf_open() && ui.get_shelf_draft_open());
        assert_eq!(ui.get_shelf_draft_title(), "   ");
        assert!(
            ui.get_shelf_error()
                .contains("later queued commands were not applied")
        );
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        assert!(ui.invoke_shelf_flush_history());
        assert_eq!(ui.get_shelf_draft_title(), "   ");
    }
    #[test]
    fn direct_task_edit_preserves_raw_notes_state_undo_recovery_and_copy_all_categories() {
        i_slint_backend_testing::init_no_event_loop();
        for category in 0..8 {
            let root = tempfile::tempdir().unwrap();
            let original = fixture(category);
            let (ui, state) = setup(original.clone(), root.path().into());
            ui.invoke_show_shelf();
            let edit = i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                &ui,
                "Edit task Review Zoë 🌙",
            )
            .next()
            .expect("accessible task edit action");
            edit.invoke_accessible_default_action();
            assert!(ui.get_shelf_draft_open() && ui.get_shelf_draft_task_edit());
            assert_eq!(ui.get_shelf_draft_title(), "Review Zoë 🌙 [[private]]");
            let baseline = state.recovery.borrow().as_ref().unwrap().path().to_owned();
            apply_action(&ui); // Exact no-op must not publish a new recovery or dirty the draft.
            assert!(!ui.get_shelf_draft_open() && !ui.get_dirty());
            assert_eq!(state.recovery.borrow().as_ref().unwrap().path(), baseline);
            assert!(!ui.get_shelf_can_undo());
            ui.invoke_shelf_edit_task(1);
            draft_text(&ui, 0, "Updated 世界 [[private]]");
            undo(&ui, false);
            assert_eq!(ui.get_shelf_draft_title(), "Review Zoë 🌙 [[private]]");
            undo(&ui, true);
            assert_eq!(ui.get_shelf_draft_title(), "Updated 世界 [[private]]");
            ui.invoke_shelf_draft_action(false);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            ui.invoke_shelf_edit_task(1);
            draft_text(&ui, 0, "Updated 世界 [[private]]");
            apply_action(&ui);
            assert!(!ui.get_shelf_draft_open());
            let changed = state.project.borrow().as_ref().unwrap().clone();
            let mut expected = original.metadata().clone();
            expected[NAMESPACE]["shelf"]["items"]["plan"]["content"] =
                "[ ] Updated 世界 [[private]]\r\n[x] Send draft\r\n".into();
            assert_eq!(changed.metadata(), &expected);
            assert_eq!(changed.source(), original.source());
            assert!(ui.get_dirty() && !ui.get_can_undo());
            assert_eq!(
                ui.get_shelf_focus_position(),
                2 * ui.get_shelf_rows().row_count() as i32 + 1
            );
            undo(&ui, false);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            undo(&ui, true);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                changed.entries()
            );
            let path = root.path().join(format!("Edited {category}.fountage"));
            project_draft::save_copy(&changed, changed.source(), &path).unwrap();
            assert_eq!(Project::read(&path).unwrap().entries(), changed.entries());
            let recovery_path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
            let snapshot = recovery::read(&recovery_path).unwrap();
            assert_eq!(
                recovery::project_from_snapshot(&recovery_path, &snapshot)
                    .unwrap()
                    .unwrap()
                    .entries(),
                changed.entries()
            );
            for (path, entry) in original.entries() {
                if path != std::ffi::OsStr::new("info.json") {
                    assert_eq!(changed.entries().get(path), Some(entry));
                }
            }
            ui.hide().unwrap();
        }
    }
    #[test]
    fn task_edit_invalid_cross_line_stale_and_failed_publish_keep_form_and_original() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = fixture(0);
        let (ui, state) = setup(original.clone(), root.path().into());
        ui.invoke_show_shelf();
        ui.invoke_shelf_edit_task(1);
        for text in ["bad\nline", "bad\0input", "New /*"] {
            ui.set_shelf_draft_title(text.into());
            apply_action(&ui);
            assert!(ui.get_shelf_draft_open());
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            assert!(!ui.get_dirty());
            assert_eq!(
                state
                    .history
                    .borrow()
                    .availability(ui.get_source().as_str()),
                (false, false)
            );
        }
        ui.invoke_shelf_draft_action(false);
        ui.invoke_shelf_edit_task(1);
        draft_text(&ui, 0, "Valid task");
        let mut changed = original.clone();
        let metadata = original.metadata().clone();
        let item = shelf::items(&metadata)
            .unwrap()
            .into_iter()
            .find(|i| i.id == "plan")
            .unwrap();
        changed
            .edit_shelf(
                &metadata,
                &Operation::AppendTask {
                    expected: item,
                    text: "External change".into(),
                },
            )
            .unwrap();
        state
            .checkpoint
            .borrow_mut()
            .set_metadata(Some(changed.metadata()));
        *state.project.borrow_mut() = Some(changed.clone());
        apply_action(&ui);
        assert!(ui.get_shelf_draft_open());
        assert_eq!(ui.get_shelf_draft_title(), "Valid task");
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            changed.entries()
        );
        ui.invoke_shelf_draft_action(false);
        ui.invoke_shelf_close();
        let blocked = root.path().join("blocked");
        std::fs::write(&blocked, "keep").unwrap();
        install(&ui, state.clone(), move || Ok(blocked.clone()));
        ui.invoke_show_shelf();
        ui.invoke_shelf_edit_task(1);
        draft_text(&ui, 0, "Keep unsaved task");
        apply_action(&ui);
        assert!(ui.get_shelf_draft_open());
        assert_eq!(ui.get_shelf_draft_title(), "Keep unsaved task");
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            changed.entries()
        );
        assert!(!ui.get_shelf_error().is_empty());
        ui.hide().unwrap();
    }
    fn removal_request(ui: &Studio) {
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(ui, "Remove Task…")
            .next()
            .unwrap()
            .invoke_accessible_default_action();
    }
    fn checklist_deletion_request(ui: &Studio) {
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(ui, "Delete Checklist…")
            .next()
            .unwrap()
            .invoke_accessible_default_action();
    }
    fn search_text(ui: &Studio, value: &str) {
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        ui.invoke_select_shelf_search(0, ui.get_shelf_query().len() as i32);
        assert!(ui.get_shelf_query_focused(), "Search field owns input");
        ui.invoke_clipboard_action(3);
        if value.is_empty() {
            ui.invoke_clipboard_action(0);
        } else {
            i_slint_core::window::WindowInner::from_pub(ui.window())
                .context()
                .platform()
                .set_clipboard_text(value, i_slint_core::platform::Clipboard::DefaultClipboard);
            ui.invoke_clipboard_action(2);
        }
        assert_eq!(ui.get_shelf_query(), value);
    }
    #[test]
    fn shelf_search_matches_title_raw_task_text_and_unicode_without_mutating_any_project() {
        i_slint_backend_testing::init_no_event_loop();
        for category in 0..8 {
            let root = tempfile::tempdir().unwrap();
            let original = lists_fixture(category);
            let (ui, state) = setup(original.clone(), root.path().into());
            ui.invoke_show_shelf();
            let baseline = state.recovery.borrow().as_ref().unwrap().path().to_owned();
            for (query, ids) in [
                ("REVISION", vec!["plan"]),
                ("zoë", vec!["plan"]),
                ("private", vec!["plan"]),
                ("LATER 🌙", vec!["later"]),
                ("unmatched shelf qa", vec![]),
                ("  final  ", vec!["last"]),
                ("", vec!["plan", "later", "last"]),
            ] {
                search_text(&ui, query);
                assert_eq!(visible_list_ids(&ui), ids);
                assert_eq!(
                    state.project.borrow().as_ref().unwrap().entries(),
                    original.entries()
                );
                assert_eq!(state.recovery.borrow().as_ref().unwrap().path(), baseline);
                assert!(!ui.get_dirty() && !ui.get_shelf_can_undo());
            }
            search_text(&ui, "Later plan");
            assert_eq!(ui.get_shelf_status(), "1 of 3 checklists · 0 open tasks");
            let index = list_index(&ui, "later");
            assert!(
                !ui.get_shelf_rows()
                    .row_data(index as usize)
                    .unwrap()
                    .can_move_up
            );
            ui.invoke_shelf_list_action(index, 1);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            ui.invoke_shelf_edit(index);
            draft_text(&ui, 0, "Renamed checklist");
            apply_action(&ui);
            assert!(visible_list_ids(&ui).is_empty());
            undo(&ui, false);
            assert_eq!(visible_list_ids(&ui), ["later"]);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            assert!(!ui.get_dirty());
            ui.invoke_shelf_close();
            ui.invoke_show_shelf();
            assert_eq!(
                ui.get_shelf_query(),
                "Later plan",
                "session search survives closing Shelf"
            );
            ui.invoke_shelf_reset();
            assert!(ui.get_shelf_query().is_empty());
            ui.hide().unwrap();
        }
    }
    #[test]
    fn shelf_search_typing_and_composition_keep_query_and_project_histories_separate() {
        use i_slint_core::input::{InternalKeyEvent, KeyEvent, KeyEventType};
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = lists_fixture(0);
        let (ui, state) = setup(original.clone(), root.path().into());
        ui.invoke_show_shelf();
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        ui.invoke_focus_shelf_search();
        assert!(ui.get_shelf_query_focused());
        for character in "later".chars() {
            key(&ui, character.to_string().into());
        }
        assert_eq!(ui.get_shelf_query(), "later");
        assert_eq!(visible_list_ids(&ui), ["later"]);
        ui.window()
            .dispatch_event(WindowEvent::internal(InternalKeyEvent {
                key_event: KeyEvent::default(),
                event_type: KeyEventType::UpdateComposition,
                preedit_text: "日本".into(),
                ..Default::default()
            }));
        assert!(ui.get_shelf_query_composing());
        assert!(ui.invoke_shelf_queue_history(false));
        ui.invoke_shelf_queue_focus(false);
        assert!(!ui.invoke_shelf_flush_history());
        assert_eq!(ui.get_shelf_query(), "later");
        assert!(ui.get_shelf_query_focused());
        ui.window()
            .dispatch_event(WindowEvent::internal(InternalKeyEvent {
                key_event: {
                    let mut event = KeyEvent::default();
                    event.text = "日本".into();
                    event
                },
                event_type: KeyEventType::CommitComposition,
                preedit_text: "".into(),
                ..Default::default()
            }));
        assert!(!ui.get_shelf_query_composing());
        assert_eq!(ui.get_shelf_query(), "later日本");
        assert!(visible_list_ids(&ui).is_empty());
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        assert!(!ui.get_dirty());
        ui.hide().unwrap();
    }
    #[test]
    fn shelf_search_history_clipboard_and_escape_never_undo_project_changes() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = lists_fixture(0);
        let (ui, state) = setup(original, root.path().into());
        ui.invoke_show_shelf();
        ui.invoke_shelf_toggle(1);
        let changed = state.project.borrow().as_ref().unwrap().clone();
        search_text(&ui, "later");
        assert!(ui.get_shelf_query_can_undo());
        assert!(ui.invoke_shelf_queue_history(false));
        assert!(ui.invoke_shelf_flush_history());
        assert!(ui.get_shelf_query().is_empty());
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            changed.entries()
        );
        // A later Tab owns focus even if native result destruction schedules
        // the earlier query-history correction for the next event-loop turn.
        ui.invoke_shelf_focus_move(false);
        let next_control = ui.get_shelf_focus_position();
        i_slint_backend_testing::testing_backend::mock_elapsed_time(2);
        assert_eq!(ui.get_shelf_focus_position(), next_control);
        assert!(!ui.get_shelf_query_focused());
        ui.invoke_focus_shelf_search();
        assert!(ui.invoke_shelf_queue_history(true));
        ui.invoke_shelf_flush_history();
        assert_eq!(ui.get_shelf_query(), "later");
        ui.invoke_select_shelf_search(0, 5);
        assert!(ui.invoke_shelf_queue_clipboard(0));
        ui.invoke_shelf_flush_history();
        assert!(ui.get_shelf_query().is_empty());
        assert!(ui.invoke_shelf_queue_clipboard(2));
        ui.invoke_shelf_flush_history();
        assert_eq!(ui.get_shelf_query(), "later");
        key(&ui, Key::Tab.into());
        assert_eq!(ui.get_shelf_focus_position(), -6);
        key(&ui, Key::Return.into());
        assert!(ui.get_shelf_query().is_empty() && ui.get_shelf_query_focused());
        search_text(&ui, "later");
        key(&ui, Key::Escape.into());
        assert!(ui.get_shelf_open() && ui.get_shelf_query().is_empty());
        key(&ui, Key::Escape.into());
        assert!(!ui.get_shelf_open());
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            changed.entries()
        );
        ui.hide().unwrap();
    }
    #[test]
    fn checklist_deletion_review_keyboard_cancel_history_recovery_and_copy_all_categories() {
        i_slint_backend_testing::init_no_event_loop();
        for category in 0..8 {
            let root = tempfile::tempdir().unwrap();
            let base = lists_fixture(category);
            let mut metadata = base.metadata().clone();
            metadata[NAMESPACE]["shelf"]["items"]["plan"]["pinned"] = true.into();
            metadata[NAMESPACE]["shelf"]["pinnedOrder"] =
                serde_json::json!(["future-pin", "plan", "note", "later"]);
            let mut entries = base.entries().clone();
            entries.insert(
                "info.json".into(),
                Entry::File(serde_json::to_vec(&metadata).unwrap()),
            );
            let original = Project::from_entries(entries).unwrap();
            let (ui, state) = setup(original.clone(), root.path().into());
            ui.invoke_show_shelf();
            ui.invoke_shelf_edit(list_index(&ui, "plan"));
            draft_text(&ui, 0, "Unsaved title");
            draft_text(&ui, 1, "[ ] Unsaved content");
            ui.invoke_focus_shelf_draft();
            key(&ui, Key::Tab.into());
            assert_eq!(
                ui.get_shelf_draft_control_index(),
                1,
                "Tab reaches checklist content"
            );
            key(&ui, Key::Tab.into());
            assert_eq!(ui.get_shelf_draft_control_index(), 4);
            key(&ui, Key::Return.into());
            assert!(ui.get_shelf_draft_removing());
            assert!(!ui.get_shelf_draft_task());
            assert_eq!(ui.get_shelf_draft_task_count(), 2);
            assert_eq!(
                ui.get_shelf_draft_removal_preview(),
                "[ ] Review Zoë 🌙 [[private]]\r\n[x] Send draft\r\n"
            );
            assert_eq!(ui.get_shelf_draft_control_index(), 2);
            key(&ui, Key::Return.into());
            assert!(!ui.get_shelf_draft_removing() && ui.get_shelf_draft_open());
            assert_eq!(ui.get_shelf_draft_title(), "Unsaved title");
            assert_eq!(ui.get_shelf_draft_content(), "[ ] Unsaved content");
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            checklist_deletion_request(&ui);
            key(&ui, Key::Tab.into());
            key(&ui, Key::Return.into());
            assert!(!ui.get_shelf_draft_open());
            assert_eq!(ui.get_shelf_focus_position(), -4);
            let deleted = state.project.borrow().as_ref().unwrap().clone();
            let mut expected = metadata;
            expected[NAMESPACE]["shelf"]["items"]
                .as_object_mut()
                .unwrap()
                .remove("plan");
            expected[NAMESPACE]["shelf"]["order"] =
                serde_json::json!(["note", "future", "snippet", "later", "last"]);
            expected[NAMESPACE]["shelf"]["pinnedOrder"] =
                serde_json::json!(["future-pin", "note", "later"]);
            assert_eq!(deleted.metadata(), &expected);
            assert_eq!(visible_list_ids(&ui), ["later", "last"]);
            undo(&ui, false);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            assert!(!ui.get_dirty());
            undo(&ui, true);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                deleted.entries()
            );
            state.checkpoint.borrow_mut().mark_saved(deleted.source());
            crate::reflect_checkpoint(&ui, &state.checkpoint.borrow());
            undo(&ui, false);
            assert!(ui.get_dirty());
            undo(&ui, true);
            assert!(!ui.get_dirty());
            let path = root.path().join(format!("Deleted {category}.fountage"));
            project_draft::save_copy(&deleted, deleted.source(), &path).unwrap();
            assert_eq!(Project::read(&path).unwrap().entries(), deleted.entries());
            let recovery_path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
            let snapshot = recovery::read(&recovery_path).unwrap();
            assert_eq!(
                recovery::project_from_snapshot(&recovery_path, &snapshot)
                    .unwrap()
                    .unwrap()
                    .entries(),
                deleted.entries()
            );
            for (path, entry) in original.entries() {
                if path != std::ffi::OsStr::new("info.json") {
                    assert_eq!(deleted.entries().get(path), Some(entry));
                }
            }
            ui.hide().unwrap();
        }
    }
    #[test]
    fn checklist_deletion_final_list_retains_hidden_items_and_new_action() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = fixture(0);
        let (ui, state) = setup(original.clone(), root.path().into());
        ui.invoke_show_shelf();
        ui.invoke_shelf_edit(0);
        checklist_deletion_request(&ui);
        apply_action(&ui);
        assert!(visible_list_ids(&ui).is_empty());
        assert_eq!(ui.get_shelf_focus_position(), -4);
        let deleted = state.project.borrow().as_ref().unwrap().clone();
        assert_eq!(
            deleted.metadata()[NAMESPACE]["shelf"]["items"]["note"],
            original.metadata()[NAMESPACE]["shelf"]["items"]["note"]
        );
        assert_eq!(
            deleted.metadata()[NAMESPACE]["shelf"]["items"]["future"],
            original.metadata()[NAMESPACE]["shelf"]["items"]["future"]
        );
        ui.invoke_shelf_edit(-1);
        assert!(ui.get_shelf_draft_open());
        assert!(!ui.get_shelf_draft_existing());
        ui.invoke_shelf_remove_requested();
        assert!(!ui.get_shelf_draft_removing());
        ui.invoke_shelf_draft_action(false);
        undo(&ui, false);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        ui.hide().unwrap();
    }
    #[test]
    fn reviewed_task_removal_cancel_undo_redo_last_task_and_persistence_all_categories() {
        i_slint_backend_testing::init_no_event_loop();
        for category in 0..8 {
            let root = tempfile::tempdir().unwrap();
            let original = fixture(category);
            let (ui, state) = setup(original.clone(), root.path().into());
            ui.invoke_show_shelf();
            ui.invoke_shelf_edit_task(1);
            draft_text(&ui, 0, "Unsaved edit kept on cancel");
            assert!(ui.invoke_shelf_queue_focus(false));
            assert!(ui.invoke_shelf_queue_apply());
            assert!(ui.invoke_shelf_flush_history());
            assert!(ui.get_shelf_draft_removing());
            assert_eq!(
                ui.get_shelf_draft_removal_preview(),
                "Review Zoë 🌙 [[private]]"
            );
            assert_eq!(
                ui.get_shelf_draft_control_index(),
                2,
                "Cancel is initially focused"
            );
            key(&ui, "ignored".into());
            ui.invoke_clipboard_action(3);
            assert_eq!(ui.get_shelf_draft_title(), "Unsaved edit kept on cancel");
            key(&ui, Key::Escape.into());
            assert!(!ui.get_shelf_draft_removing() && ui.get_shelf_draft_open());
            assert_eq!(ui.get_shelf_draft_title(), "Unsaved edit kept on cancel");
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            removal_request(&ui);
            key(&ui, Key::Tab.into());
            assert_eq!(ui.get_shelf_draft_control_index(), 3);
            key(&ui, Key::Return.into());
            assert!(!ui.get_shelf_draft_open() && !ui.get_shelf_draft_removing());
            let first = state.project.borrow().as_ref().unwrap().clone();
            let mut expected = original.metadata().clone();
            expected[NAMESPACE]["shelf"]["items"]["plan"]["content"] = "[x] Send draft\r\n".into();
            assert_eq!(first.metadata(), &expected);
            assert_eq!(
                ui.get_shelf_focus_position(),
                ui.get_shelf_rows().row_count() as i32
            );
            undo(&ui, false);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            assert!(!ui.get_dirty());
            undo(&ui, true);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                first.entries()
            );
            ui.invoke_shelf_edit_task(1);
            removal_request(&ui);
            apply_action(&ui);
            let empty = state.project.borrow().as_ref().unwrap().clone();
            expected[NAMESPACE]["shelf"]["items"]["plan"]["content"] = "".into();
            assert_eq!(
                empty.metadata(),
                &expected,
                "removing final task retains checklist and opaque fields"
            );
            assert_eq!(empty.source(), original.source());
            state.checkpoint.borrow_mut().mark_saved(empty.source());
            crate::reflect_checkpoint(&ui, &state.checkpoint.borrow());
            undo(&ui, false);
            assert!(ui.get_dirty());
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                first.entries()
            );
            undo(&ui, true);
            assert!(!ui.get_dirty());
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                empty.entries()
            );
            let path = root.path().join(format!("Removed {category}.fountage"));
            project_draft::save_copy(&empty, empty.source(), &path).unwrap();
            assert_eq!(Project::read(&path).unwrap().entries(), empty.entries());
            let recovery_path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
            let snapshot = recovery::read(&recovery_path).unwrap();
            assert_eq!(
                recovery::project_from_snapshot(&recovery_path, &snapshot)
                    .unwrap()
                    .unwrap()
                    .entries(),
                empty.entries()
            );
            for (path, entry) in original.entries() {
                if path != std::ffi::OsStr::new("info.json") {
                    assert_eq!(empty.entries().get(path), Some(entry));
                }
            }
            ui.hide().unwrap();
        }
    }
    #[test]
    fn removal_failure_stale_review_and_queued_old_form_commands_cannot_remove_another_task() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = fixture(0);
        let (ui, state) = setup(original.clone(), root.path().into());
        ui.invoke_show_shelf();
        ui.invoke_shelf_edit_task(1);
        assert!(ui.invoke_shelf_queue_apply()); // Old intent to save task must not become a removal.
        removal_request(&ui); // Drains exact no-op save, ending form.
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        // If flushing ended the old form, the request must not open an orphan review.
        assert!(!ui.get_shelf_draft_open());
        assert!(!ui.get_shelf_draft_removing());
        ui.invoke_shelf_edit_task(1);
        removal_request(&ui);
        let mut changed = original.clone();
        let metadata = original.metadata().clone();
        let item = shelf::items(&metadata)
            .unwrap()
            .into_iter()
            .find(|i| i.id == "plan")
            .unwrap();
        changed
            .edit_shelf(
                &metadata,
                &Operation::AppendTask {
                    expected: item,
                    text: "External task".into(),
                },
            )
            .unwrap();
        state
            .checkpoint
            .borrow_mut()
            .set_metadata(Some(changed.metadata()));
        *state.project.borrow_mut() = Some(changed.clone());
        for _ in 0..2 {
            apply_action(&ui);
            assert!(ui.get_shelf_draft_open() && ui.get_shelf_draft_removing());
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                changed.entries()
            );
        }
        ui.invoke_shelf_draft_action(false); // Cancel review, then form.
        ui.invoke_shelf_draft_action(false);
        ui.invoke_shelf_close();
        let blocked = root.path().join("blocked");
        std::fs::write(&blocked, "keep").unwrap();
        install(&ui, state.clone(), move || Ok(blocked.clone()));
        ui.invoke_show_shelf();
        ui.invoke_shelf_edit_task(1);
        removal_request(&ui);
        let baseline = state.recovery.borrow().as_ref().unwrap().path().to_owned();
        apply_action(&ui);
        assert!(ui.get_shelf_draft_removing() && ui.get_shelf_draft_open());
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            changed.entries()
        );
        assert_eq!(state.recovery.borrow().as_ref().unwrap().path(), baseline);
        assert!(!ui.get_shelf_error().is_empty());
        ui.hide().unwrap();
    }
    #[test]
    fn checklist_deletion_stale_review_recovery_failure_and_old_save_are_safe() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = fixture(0);
        let (ui, state) = setup(original.clone(), root.path().into());
        ui.invoke_show_shelf();
        ui.invoke_shelf_edit(0);
        assert!(ui.invoke_shelf_queue_apply()); // An already accepted save must not turn into a deletion.
        checklist_deletion_request(&ui); // Drains exact no-op save, ending form.
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        // If flushing ended the old form, the request must not open an orphan review.
        assert!(!ui.get_shelf_draft_open());
        assert!(!ui.get_shelf_draft_removing());
        ui.invoke_shelf_edit(0);
        checklist_deletion_request(&ui);
        let mut changed = original.clone();
        let metadata = original.metadata().clone();
        let item = shelf::items(&metadata)
            .unwrap()
            .into_iter()
            .find(|i| i.id == "plan")
            .unwrap();
        changed
            .edit_shelf(
                &metadata,
                &Operation::AppendTask {
                    expected: item,
                    text: "External task".into(),
                },
            )
            .unwrap();
        state
            .checkpoint
            .borrow_mut()
            .set_metadata(Some(changed.metadata()));
        *state.project.borrow_mut() = Some(changed.clone());
        for _ in 0..2 {
            apply_action(&ui);
            assert!(ui.get_shelf_draft_open() && ui.get_shelf_draft_removing());
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                changed.entries()
            );
        }
        ui.invoke_shelf_draft_action(false); // Cancel review, then form.
        ui.invoke_shelf_draft_action(false);
        ui.invoke_shelf_close();
        let blocked = root.path().join("blocked");
        std::fs::write(&blocked, "keep").unwrap();
        install(&ui, state.clone(), move || Ok(blocked.clone()));
        ui.invoke_show_shelf();
        ui.invoke_shelf_edit(0);
        checklist_deletion_request(&ui);
        let baseline = state.recovery.borrow().as_ref().unwrap().path().to_owned();
        apply_action(&ui);
        assert!(ui.get_shelf_draft_removing() && ui.get_shelf_draft_open());
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            changed.entries()
        );
        assert_eq!(state.recovery.borrow().as_ref().unwrap().path(), baseline);
        assert!(!ui.get_shelf_error().is_empty());
        ui.hide().unwrap();
    }
    #[test]
    fn task_moves_boundary_noop_identity_focus_undo_recovery_and_copy_all_categories() {
        i_slint_backend_testing::init_no_event_loop();
        for category in 0..8 {
            let root = tempfile::tempdir().unwrap();
            let original = fixture(category);
            let (ui, state) = setup(original.clone(), root.path().into());
            ui.invoke_show_shelf();
            let baseline = state.recovery.borrow().as_ref().unwrap().path().to_owned();
            ui.invoke_shelf_move_task(1, false);
            assert!(!ui.get_dirty() && !ui.get_shelf_can_undo());
            assert_eq!(state.recovery.borrow().as_ref().unwrap().path(), baseline);
            let up = i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                &ui,
                "Move task up Review Zoë 🌙",
            )
            .next()
            .unwrap();
            assert_eq!(up.accessible_enabled(), Some(false));
            i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                &ui,
                "Move task down Review Zoë 🌙",
            )
            .next()
            .unwrap()
            .invoke_accessible_default_action();
            let moved = state.project.borrow().as_ref().unwrap().clone();
            let mut expected = original.metadata().clone();
            expected[NAMESPACE]["shelf"]["items"]["plan"]["content"] =
                "[x] Send draft\r\n[ ] Review Zoë 🌙 [[private]]\r\n".into();
            assert_eq!(moved.metadata(), &expected);
            assert_eq!(moved.source(), original.source());
            assert_eq!(
                ui.get_shelf_focus_position(),
                2 * ui.get_shelf_rows().row_count() as i32 + 2,
                "last-row move focuses the same task Edit, not a disabled Down button"
            );
            undo(&ui, false);
            assert!(!ui.get_dirty());
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            undo(&ui, true);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                moved.entries()
            );
            let path = root.path().join(format!("Moved {category}.fountage"));
            project_draft::save_copy(&moved, moved.source(), &path).unwrap();
            assert_eq!(Project::read(&path).unwrap().entries(), moved.entries());
            let recovery_path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
            let snapshot = recovery::read(&recovery_path).unwrap();
            assert_eq!(
                recovery::project_from_snapshot(&recovery_path, &snapshot)
                    .unwrap()
                    .unwrap()
                    .entries(),
                moved.entries()
            );
            state.checkpoint.borrow_mut().mark_saved(moved.source());
            crate::reflect_checkpoint(&ui, &state.checkpoint.borrow());
            undo(&ui, false);
            assert!(ui.get_dirty());
            undo(&ui, true);
            assert!(!ui.get_dirty());
            i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                &ui,
                "Move task up Review Zoë 🌙",
            )
            .next()
            .unwrap()
            .invoke_accessible_default_action();
            assert_eq!(
                state.project.borrow().as_ref().unwrap().metadata(),
                original.metadata()
            );
            assert_eq!(
                ui.get_shelf_focus_position(),
                2 * ui.get_shelf_rows().row_count() as i32 + 1
            );
            for (path, entry) in original.entries() {
                if path != std::ffi::OsStr::new("info.json") {
                    assert_eq!(moved.entries().get(path), Some(entry));
                }
            }
            ui.hide().unwrap();
        }
    }
    #[test]
    fn task_move_refuses_old_row_after_pending_undo_stale_metadata_and_failed_recovery() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = fixture(0);
        let (ui, state) = setup(original.clone(), root.path().into());
        ui.invoke_show_shelf();
        ui.invoke_shelf_move_task(1, true);
        assert!(ui.invoke_shelf_queue_history(false));
        ui.invoke_shelf_move_task(2, false); // Old row must not retarget after queued Undo changed its contents.
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        assert!(!ui.get_shelf_error().is_empty());
        let mut metadata = original.metadata().clone();
        metadata["foreign"]["added"] = true.into();
        let mut entries = original.entries().clone();
        entries.insert(
            "info.json".into(),
            Entry::File(serde_json::to_vec(&metadata).unwrap()),
        );
        let changed = Project::from_entries(entries).unwrap();
        *state.project.borrow_mut() = Some(changed.clone());
        ui.invoke_shelf_move_task(1, true);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            changed.entries()
        );
        ui.invoke_shelf_close();
        let blocked = root.path().join("blocked");
        std::fs::write(&blocked, "keep").unwrap();
        install(&ui, state.clone(), move || Ok(blocked.clone()));
        ui.invoke_show_shelf();
        let baseline = state.recovery.borrow().as_ref().unwrap().path().to_owned();
        ui.invoke_shelf_move_task(1, true);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            changed.entries()
        );
        assert_eq!(state.recovery.borrow().as_ref().unwrap().path(), baseline);
        assert!(!ui.get_shelf_error().is_empty());
        ui.invoke_shelf_edit_task(1);
        draft_text(&ui, 0, "Keep unsaved field");
        ui.invoke_shelf_move_task(1, true);
        assert_eq!(ui.get_shelf_draft_title(), "Keep unsaved field");
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            changed.entries()
        );
        ui.hide().unwrap();
    }
    #[test]
    fn repeated_keyboard_task_moves_retain_control_focus_and_edit_the_same_task_at_boundary() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = fixture(0);
        let mut metadata = original.metadata().clone();
        metadata[NAMESPACE]["shelf"]["items"]["plan"]["content"] =
            "[ ] First [[private]]\r\n[x] Second\r\n[ ] Third\r\n[ ] Fourth\r\n".into();
        let mut entries = original.entries().clone();
        entries.insert(
            "info.json".into(),
            Entry::File(serde_json::to_vec(&metadata).unwrap()),
        );
        let (ui, state) = setup(Project::from_entries(entries).unwrap(), root.path().into());
        ui.invoke_show_shelf();
        i_slint_backend_testing::testing_backend::mock_elapsed_time(2);
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(
            &ui,
            "Move task down First",
        )
        .next()
        .unwrap()
        .invoke_accessible_default_action();
        i_slint_backend_testing::testing_backend::mock_elapsed_time(2);
        i_slint_backend_testing::testing_backend::mock_elapsed_time(2);
        for expected in [
            "[x] Second\r\n[ ] Third\r\n[ ] First [[private]]\r\n[ ] Fourth\r\n",
            "[x] Second\r\n[ ] Third\r\n[ ] Fourth\r\n[ ] First [[private]]\r\n",
        ] {
            key(&ui, Key::Return.into());
            i_slint_backend_testing::testing_backend::mock_elapsed_time(2);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().metadata()[NAMESPACE]["shelf"]["items"]["plan"]
                    ["content"],
                expected
            );
        }
        key(&ui, Key::Return.into());
        assert!(ui.get_shelf_draft_open());
        assert_eq!(ui.get_shelf_draft_title(), "First [[private]]");
        ui.invoke_shelf_draft_action(false);
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(&ui, "Move task up First")
            .next()
            .unwrap()
            .invoke_accessible_default_action();
        i_slint_backend_testing::testing_backend::mock_elapsed_time(2);
        key(&ui, Key::Return.into());
        i_slint_backend_testing::testing_backend::mock_elapsed_time(2);
        key(&ui, Key::Return.into());
        i_slint_backend_testing::testing_backend::mock_elapsed_time(2);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().metadata(),
            &metadata
        );
        key(&ui, Key::Return.into());
        assert!(ui.get_shelf_draft_open());
        assert_eq!(ui.get_shelf_draft_title(), "First [[private]]");
        ui.hide().unwrap();
    }
    fn lists_fixture(category: u32) -> Project {
        let original = fixture(category);
        let mut metadata = original.metadata().clone();
        metadata[NAMESPACE]["shelf"]["items"]["later"] = serde_json::json!({"kind":"checklist","title":"Later plan","content":"[X] Later 🌙\r\n","pinned":false,"opaque":7});
        metadata[NAMESPACE]["shelf"]["items"]["last"] = serde_json::json!({"kind":"checklist","title":"Final plan","content":"","pinned":false});
        metadata[NAMESPACE]["shelf"]["items"]["snippet"] = serde_json::json!({"kind":"snippet","title":"Keep snippet","content":"Opaque snippet","pinned":false});
        metadata[NAMESPACE]["shelf"]["order"] =
            serde_json::json!(["note", "plan", "future", "snippet", "later", "last"]);
        let mut entries = original.entries().clone();
        entries.insert(
            "info.json".into(),
            Entry::File(serde_json::to_vec(&metadata).unwrap()),
        );
        Project::from_entries(entries).unwrap()
    }
    fn list_index(ui: &Studio, id: &str) -> i32 {
        ui.get_shelf_rows()
            .iter()
            .position(|r| r.item_id == id)
            .unwrap() as i32
    }
    fn visible_list_ids(ui: &Studio) -> Vec<String> {
        ui.get_shelf_rows()
            .iter()
            .filter(|r| !r.item_id.is_empty())
            .map(|r| r.item_id.to_string())
            .collect()
    }
    #[test]
    fn shelf_record_pin_groups_order_unpin_return_history_recovery_and_copy_all_categories() {
        i_slint_backend_testing::init_no_event_loop();
        for (choice, kind) in [(0, "checklist"), (1, "note"), (2, "snippet")] {
            for category in 0..8 {
                let root = tempfile::tempdir().unwrap();
                let baseline = lists_fixture(category);
                let original = if choice == 0 {
                    baseline
                } else {
                    let mut metadata = baseline.metadata().clone();
                    for id in ["plan", "later", "last"] {
                        metadata[NAMESPACE]["shelf"]["items"][id]["kind"] = kind.into();
                    }
                    // Interleave opaque records and both other known kinds. Their order
                    // slots and unknown payloads must survive every visible move.
                    metadata[NAMESPACE]["shelf"]["items"]["note"]["kind"] = "checklist".into();
                    metadata[NAMESPACE]["shelf"]["items"]["snippet"]["kind"] =
                        if choice == 1 { "snippet" } else { "note" }.into();
                    let mut entries = baseline.entries().clone();
                    entries.insert(
                        "info.json".into(),
                        Entry::File(serde_json::to_vec(&metadata).unwrap()),
                    );
                    Project::from_entries(entries).unwrap()
                };
                let (ui, state) = setup(original.clone(), root.path().into());
                ui.invoke_show_shelf();
                ui.invoke_shelf_kind_requested(choice);
                let baseline = state.recovery.borrow().as_ref().unwrap().path().to_owned();
                ui.invoke_shelf_list_action(list_index(&ui, "plan"), 1);
                assert!(!ui.get_dirty());
                assert_eq!(state.recovery.borrow().as_ref().unwrap().path(), baseline);
                i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                    &ui,
                    &format!("Pin {kind} Later plan"),
                )
                .next()
                .unwrap()
                .invoke_accessible_default_action();
                assert_eq!(visible_list_ids(&ui), ["later", "plan", "last"]);
                assert_eq!(
                    ui.get_shelf_rows().row_data(0).unwrap().group_label,
                    "PINNED"
                );
                assert_eq!(ui.get_shelf_focus_position(), -1);
                let pinned = state.project.borrow().as_ref().unwrap().clone();
                let mut expected = original.metadata().clone();
                expected[NAMESPACE]["shelf"]["items"]["later"]["pinned"] = true.into();
                assert_eq!(pinned.metadata(), &expected);
                undo(&ui, false);
                assert_eq!(
                    state.project.borrow().as_ref().unwrap().entries(),
                    original.entries()
                );
                assert!(!ui.get_dirty());
                undo(&ui, true);
                assert_eq!(
                    state.project.borrow().as_ref().unwrap().entries(),
                    pinned.entries()
                );
                ui.invoke_shelf_list_action(list_index(&ui, "later"), 0);
                assert_eq!(
                    state.project.borrow().as_ref().unwrap().metadata(),
                    original.metadata()
                );
                ui.invoke_shelf_list_action(list_index(&ui, "plan"), 2);
                expected = original.metadata().clone();
                expected[NAMESPACE]["shelf"]["order"] =
                    serde_json::json!(["note", "later", "future", "snippet", "plan", "last"]);
                assert_eq!(
                    state.project.borrow().as_ref().unwrap().metadata(),
                    &expected,
                    "skip invisible notes/snippets without moving their slots"
                );
                for id in ["plan", "later"] {
                    ui.invoke_shelf_list_action(list_index(&ui, id), 0);
                }
                assert_eq!(visible_list_ids(&ui), ["later", "plan", "last"]);
                ui.invoke_shelf_list_action(list_index(&ui, "plan"), 1);
                assert_eq!(visible_list_ids(&ui), ["plan", "later", "last"]);
                let reordered = state.project.borrow().as_ref().unwrap().clone();
                assert_eq!(
                    reordered.metadata()[NAMESPACE]["shelf"]["order"],
                    expected[NAMESPACE]["shelf"]["order"]
                );
                let path = root.path().join("Pinned.fountage");
                project_draft::save_copy(&reordered, reordered.source(), &path).unwrap();
                assert_eq!(Project::read(&path).unwrap().entries(), reordered.entries());
                if choice == 0
                    && let Some(directory) = std::env::var_os("FOUNTAGE_PIN_FIXTURES")
                {
                    std::fs::create_dir_all(&directory).unwrap();
                    let target =
                        PathBuf::from(directory).join(format!("Pinned {category}.fountage"));
                    project_draft::save_copy(&reordered, reordered.source(), &target).unwrap();
                }
                let recovery_path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
                let snapshot = recovery::read(&recovery_path).unwrap();
                assert_eq!(
                    recovery::project_from_snapshot(&recovery_path, &snapshot)
                        .unwrap()
                        .unwrap()
                        .entries(),
                    reordered.entries()
                );
                state.checkpoint.borrow_mut().mark_saved(reordered.source());
                crate::reflect_checkpoint(&ui, &state.checkpoint.borrow());
                undo(&ui, false);
                assert!(ui.get_dirty());
                undo(&ui, true);
                assert!(!ui.get_dirty());
                for id in ["plan", "later"] {
                    ui.invoke_shelf_list_action(list_index(&ui, id), 0);
                }
                assert_eq!(
                    visible_list_ids(&ui),
                    ["later", "plan", "last"],
                    "unpin returns to base order after rearranging pins"
                );
                let result = state.project.borrow().as_ref().unwrap().clone();
                assert_eq!(result.source(), original.source());
                for (path, entry) in original.entries() {
                    if path != std::ffi::OsStr::new("info.json") {
                        assert_eq!(result.entries().get(path), Some(entry));
                    }
                }
                assert_eq!(
                    result.metadata()[NAMESPACE]["shelf"]["items"]["note"],
                    original.metadata()[NAMESPACE]["shelf"]["items"]["note"]
                );
                assert_eq!(
                    result.metadata()[NAMESPACE]["shelf"]["items"]["snippet"],
                    original.metadata()[NAMESPACE]["shelf"]["items"]["snippet"]
                );
                ui.hide().unwrap();
            }
        }
    }

    #[test]
    fn checklist_management_stale_pending_history_failed_recovery_and_forms_are_guarded() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = lists_fixture(0);
        let (ui, state) = setup(original.clone(), root.path().into());
        ui.invoke_show_shelf();
        ui.invoke_shelf_list_action(list_index(&ui, "later"), 0);
        assert!(ui.invoke_shelf_queue_history(false));
        ui.invoke_shelf_list_action(list_index(&ui, "later"), 0); // frozen snapshot must not toggle the restored original.
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        assert!(!ui.get_shelf_error().is_empty());
        ui.invoke_shelf_edit(list_index(&ui, "plan"));
        draft_text(&ui, 0, "Keep unsaved title");
        ui.invoke_shelf_list_action(0, 0);
        assert_eq!(ui.get_shelf_draft_title(), "Keep unsaved title");
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        ui.invoke_shelf_draft_action(false);
        ui.invoke_shelf_close();
        let blocked = root.path().join("blocked");
        std::fs::write(&blocked, "keep").unwrap();
        install(&ui, state.clone(), move || Ok(blocked.clone()));
        ui.invoke_show_shelf();
        let recovery = state.recovery.borrow().as_ref().unwrap().path().to_owned();
        for action in [0, 2] {
            ui.invoke_shelf_list_action(list_index(&ui, "plan"), action);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            assert_eq!(state.recovery.borrow().as_ref().unwrap().path(), recovery);
            assert!(!ui.get_shelf_error().is_empty());
        }
        ui.hide().unwrap();
    }
    #[test]
    fn quick_notes_all_categories_capture_history_visibility_recovery_and_reopen() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        for category in 0..8 {
            let original = fixture(category);
            let source = original.source().to_owned();
            let (ui, state) = setup(
                original.clone(),
                root.path().join(format!("recovery-{category}")),
            );
            ui.invoke_show_shelf();
            i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
            ui.invoke_focus_shelf_quick();
            assert!(ui.get_shelf_quick_focused());
            let draft = "Quick Zoë 🌙\r\nSecond line 世界 e\u{301}\r\n";
            ui.set_shelf_quick_text(draft.into());
            ui.invoke_shelf_quick_edited(draft.into());
            assert!(!ui.get_shelf_quick_pending());
            assert_eq!(
                shelf::quick_text(state.project.borrow().as_ref().unwrap().metadata()),
                Ok(draft)
            );
            let before_capture = state.project.borrow().as_ref().unwrap().clone();
            assert!(ui.invoke_shelf_queue_apply());
            assert!(ui.invoke_shelf_flush_history());
            let captured = state.project.borrow().as_ref().unwrap().clone();
            assert_eq!(ui.get_shelf_quick_text(), "");
            let items = shelf::items(captured.metadata()).unwrap();
            let note = items.iter().find(|item| item.content == draft).unwrap();
            assert_eq!(note.kind, Kind::Note);
            assert_eq!(note.title, "Note");
            assert_eq!(captured.metadata()[NAMESPACE]["shelf"]["order"][0], note.id);
            assert_eq!(captured.source(), source);
            ui.invoke_shelf_history(false);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                before_capture.entries()
            );
            assert_eq!(ui.get_shelf_quick_text().as_str(), draft);
            ui.invoke_shelf_history(true);
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                captured.entries()
            );
            let pending = "Keep this unfinished idea 🌙\r\n世界";
            ui.set_shelf_quick_text(pending.into());
            ui.invoke_shelf_quick_edited(pending.into());
            ui.invoke_shelf_visibility_requested(3);
            assert!(!ui.get_shelf_quick_visible());
            assert_eq!(ui.get_shelf_quick_text().as_str(), pending);
            ui.invoke_shelf_history(false);
            assert!(ui.get_shelf_quick_visible());
            ui.invoke_shelf_history(true);
            assert!(!ui.get_shelf_quick_visible());
            let saved = state.project.borrow().as_ref().unwrap().clone();
            for (path, entry) in original.entries() {
                if path != std::ffi::OsStr::new("info.json") {
                    assert_eq!(saved.entries().get(path), Some(entry));
                }
            }
            assert_eq!(saved.metadata()["foreign"], original.metadata()["foreign"]);
            for id in ["note", "plan", "future"] {
                assert_eq!(
                    saved.metadata()[NAMESPACE]["shelf"]["items"][id],
                    original.metadata()[NAMESPACE]["shelf"]["items"][id]
                );
            }
            let copy = root.path().join(format!("Quick {category}.fountage"));
            project_draft::save_copy(&saved, &source, &copy).unwrap();
            let reopened = Project::read(&copy).unwrap();
            assert_eq!(reopened.entries(), saved.entries());
            let recovery_path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
            let snapshot = recovery::read(&recovery_path).unwrap();
            assert_eq!(
                recovery::project_from_snapshot(&recovery_path, &snapshot)
                    .unwrap()
                    .unwrap()
                    .entries(),
                saved.entries()
            );
            ui.invoke_shelf_close();
            assert!(!ui.get_shelf_open());
            ui.hide().unwrap();
            let (ui, _) = setup(reopened, root.path().join(format!("reopened-{category}")));
            ui.invoke_show_shelf();
            assert!(!ui.get_shelf_quick_visible());
            assert_eq!(ui.get_shelf_quick_text().as_str(), pending);
            assert!(!ui.get_shelf_can_undo());
            ui.hide().unwrap();
        }
    }
    #[test]
    fn failed_quick_write_retains_text_and_blocks_close_history_and_other_edits_until_retry() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = fixture(0);
        let (ui, state) = setup(original.clone(), root.path().join("valid"));
        let blocked = root.path().join("blocked");
        std::fs::write(&blocked, b"keep").unwrap();
        let failing = Rc::new(Cell::new(true));
        let flag = failing.clone();
        let good = root.path().join("retry");
        install(&ui, state.clone(), move || {
            Ok(if flag.get() {
                blocked.clone()
            } else {
                good.clone()
            })
        });
        ui.invoke_show_shelf();
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        ui.invoke_focus_shelf_quick();
        key(&ui, "A pending idea 🌙".into());
        assert!(ui.get_shelf_quick_pending());
        assert_eq!(ui.get_shelf_quick_text(), "A pending idea 🌙");
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        ui.invoke_shelf_close();
        assert!(ui.get_shelf_open());
        ui.invoke_shelf_visibility_requested(3);
        assert!(ui.get_shelf_quick_visible());
        ui.invoke_shelf_history(false);
        assert_eq!(ui.get_shelf_quick_text(), "A pending idea 🌙");
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        failing.set(false);
        ui.invoke_shelf_quick_capture(); // Retry persists, without capturing a Note.
        assert!(!ui.get_shelf_quick_pending());
        assert_eq!(
            shelf::items(state.project.borrow().as_ref().unwrap().metadata())
                .unwrap()
                .len(),
            shelf::items(original.metadata()).unwrap().len()
        );
        ui.invoke_shelf_quick_capture();
        assert_eq!(ui.get_shelf_quick_text(), "");
        ui.invoke_shelf_history(false);
        assert_eq!(ui.get_shelf_quick_text(), "A pending idea 🌙");
        ui.invoke_shelf_close();
        assert!(!ui.get_shelf_open());
        ui.hide().unwrap();
    }
    #[test]
    fn quick_entry_preedit_clipboard_and_stale_queued_capture_preserve_committed_text() {
        use i_slint_core::input::{InternalKeyEvent, KeyEvent, KeyEventType};
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let original = fixture(0);
        let (ui, state) = setup(original.clone(), root.path().join("recovery"));
        ui.invoke_show_shelf();
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        ui.invoke_focus_shelf_quick();
        let compose = |preedit: &str, committed: &str, commit: bool| {
            let mut key_event = KeyEvent::default();
            key_event.text = committed.into();
            ui.window()
                .dispatch_event(WindowEvent::internal(InternalKeyEvent {
                    key_event,
                    event_type: if commit {
                        KeyEventType::CommitComposition
                    } else {
                        KeyEventType::UpdateComposition
                    },
                    preedit_text: preedit.into(),
                    ..Default::default()
                }));
        };
        compose("にほん", "", false);
        assert!(ui.get_shelf_quick_composing());
        ui.invoke_shelf_quick_capture();
        ui.invoke_shelf_close();
        assert!(ui.get_shelf_open());
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            original.entries()
        );
        compose("", "日本 🌙", true);
        assert_eq!(ui.get_shelf_quick_text(), "日本 🌙");
        assert!(!ui.get_shelf_quick_pending());
        assert!(ui.invoke_shelf_queue_clipboard(3));
        assert!(ui.invoke_shelf_queue_clipboard(1));
        assert!(ui.invoke_shelf_queue_clipboard(2));
        assert!(ui.invoke_shelf_flush_history());
        assert_eq!(ui.get_shelf_quick_text(), "日本 🌙");
        assert!(ui.invoke_shelf_queue_clipboard(3));
        assert!(ui.invoke_shelf_queue_clipboard(0));
        assert!(ui.invoke_shelf_flush_history());
        assert_eq!(ui.get_shelf_quick_text(), "");
        assert_eq!(
            shelf::quick_text(state.project.borrow().as_ref().unwrap().metadata()),
            Ok("")
        );
        assert!(ui.invoke_shelf_queue_clipboard(2));
        assert!(ui.invoke_shelf_flush_history());
        assert_eq!(ui.get_shelf_quick_text(), "日本 🌙");
        assert!(ui.invoke_shelf_queue_apply());
        ui.set_shelf_quick_text("newer input 世界".into());
        ui.invoke_shelf_quick_edited("newer input 世界".into());
        assert!(ui.get_shelf_quick_pending());
        assert_eq!(ui.get_shelf_quick_text(), "newer input 世界");
        assert_eq!(
            shelf::quick_text(state.project.borrow().as_ref().unwrap().metadata()),
            Ok("日本 🌙")
        );
        ui.invoke_shelf_quick_capture(); // Retry, not capture.
        assert!(!ui.get_shelf_quick_pending());
        assert_eq!(
            shelf::quick_text(state.project.borrow().as_ref().unwrap().metadata()),
            Ok("newer input 世界")
        );
        ui.invoke_shelf_close();
        assert!(!ui.get_shelf_open());
        ui.hide().unwrap();
    }
    #[test]
    fn quick_entry_recovery_cost_probe() {
        if std::env::var_os("FOUNTAGE_QUICK_COST_PROBE").is_none() {
            return;
        }
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let mut entries = fixture(0).entries().clone();
        entries.insert(
            "large-attachment.bin".into(),
            Entry::File(vec![137; 4 * 1024 * 1024]),
        );
        let project = Project::from_entries(entries).unwrap();
        let (ui, _) = setup(project, root.path().join("recovery"));
        ui.invoke_show_shelf();
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        ui.invoke_focus_shelf_quick();
        let mut timings = Vec::new();
        for letter in "Quick idea 🌙".chars() {
            let start = std::time::Instant::now();
            key(&ui, letter.to_string().into());
            timings.push(start.elapsed().as_secs_f64() * 1000.0);
            assert!(!ui.get_shelf_quick_pending());
        }
        eprintln!(
            "QUICK_COST {}",
            serde_json::json!({"attachmentBytes":4 * 1024 * 1024,"millisecondsPerCommittedEdit":timings,"scope":"Testing backend, synchronous recovery path; not native end-to-end input latency"})
        );
        ui.hide().unwrap();
    }
    #[test]
    fn quick_typing_and_history_reuse_one_baseline_with_complete_independent_revisions() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let mut entries = fixture(0).entries().clone();
        entries.insert(
            "large-asset.bin".into(),
            Entry::File(vec![89; 4 * 1024 * 1024]),
        );
        let original = Project::from_entries(entries).unwrap();
        let (ui, state) = setup(original.clone(), root.path().join("recovery"));
        let base = state
            .recovery
            .borrow()
            .as_ref()
            .unwrap()
            .path()
            .parent()
            .unwrap()
            .to_owned();
        let base_bytes = std::fs::read(base.join("scratch.json")).unwrap();
        let mut paths = std::collections::BTreeSet::new();
        ui.invoke_show_shelf();
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        ui.invoke_focus_shelf_quick();
        for character in "Quick 世界 🌙".chars() {
            key(&ui, character.to_string().into());
            assert!(!ui.get_shelf_quick_pending());
            let path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
            assert_eq!(path.parent(), Some(base.as_path()));
            assert!(paths.insert(path.clone()));
            assert!(std::fs::metadata(&path).unwrap().len() < 65536);
            let snapshot = recovery::read(&path).unwrap();
            assert_eq!(snapshot.schema, 3);
            assert_eq!(
                recovery::project_from_snapshot(&path, &snapshot)
                    .unwrap()
                    .unwrap()
                    .entries(),
                state.project.borrow().as_ref().unwrap().entries()
            );
        }
        assert_eq!(
            std::fs::read(base.join("scratch.json")).unwrap(),
            base_bytes
        );
        assert_eq!(
            Project::read(&base.join("project-baseline.fountage"))
                .unwrap()
                .entries(),
            original.entries()
        );
        let final_text = ui.get_shelf_quick_text();
        let final_project = state.project.borrow().as_ref().unwrap().clone();
        let final_path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
        let final_bytes = std::fs::read(&final_path).unwrap();
        ui.invoke_shelf_history(false);
        assert_ne!(ui.get_shelf_quick_text(), final_text);
        ui.invoke_shelf_history(true);
        assert_eq!(ui.get_shelf_quick_text(), final_text);
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            final_project.entries()
        );
        assert_eq!(std::fs::read(final_path).unwrap(), final_bytes);
        assert_eq!(
            state.recovery.borrow().as_ref().unwrap().path().parent(),
            Some(base.as_path())
        );
        assert_eq!(
            std::fs::read_dir(root.path().join("recovery"))
                .unwrap()
                .count(),
            1,
            "one baseline directory, not a full copy per character"
        );
        ui.hide().unwrap();
    }
    #[test]
    fn quick_note_typing_groups_and_directed_selection_restore_across_all_categories() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        for category in 0..8 {
            let original = fixture(category);
            let (ui, state) = setup(
                original.clone(),
                root.path().join(format!("recovery-{category}")),
            );
            crate::history::set_test_time(Some(0));
            ui.invoke_show_shelf();
            i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
            ui.invoke_focus_shelf_quick();
            let phrase = "Zoë 🌙 世界";
            for character in phrase.chars() {
                key(&ui, character.to_string().into());
            }
            assert_eq!(ui.get_shelf_quick_text(), phrase);
            assert_eq!(ui.get_shelf_quick_focus() as usize, phrase.len());
            ui.invoke_shelf_history(false);
            assert_eq!(
                ui.get_shelf_quick_text(),
                "",
                "one Undo removes an adjacent typing run"
            );
            assert_eq!(
                (ui.get_shelf_quick_anchor(), ui.get_shelf_quick_focus()),
                (0, 0)
            );
            assert_eq!(
                state.project.borrow().as_ref().unwrap().entries(),
                original.entries()
            );
            ui.invoke_shelf_history(true);
            assert_eq!(ui.get_shelf_quick_text(), phrase);
            assert_eq!(
                (
                    ui.get_shelf_quick_anchor() as usize,
                    ui.get_shelf_quick_focus() as usize
                ),
                (phrase.len(), phrase.len())
            );
            let end = "Zoë 🌙".len();
            ui.invoke_select_shelf_quick(end as i32, 2);
            key(&ui, "X".into());
            let replaced = ui.get_shelf_quick_text().to_string();
            assert_eq!(replaced, "ZoX 世界");
            ui.invoke_shelf_history(false);
            assert_eq!(ui.get_shelf_quick_text(), phrase);
            assert_eq!(
                (ui.get_shelf_quick_anchor(), ui.get_shelf_quick_focus()),
                (end as i32, 2)
            );
            ui.invoke_shelf_history(true);
            assert_eq!(ui.get_shelf_quick_text(), replaced);
            assert_eq!(
                (ui.get_shelf_quick_anchor(), ui.get_shelf_quick_focus()),
                (3, 3)
            );
            ui.invoke_shelf_quick_capture();
            assert_eq!(ui.get_shelf_quick_text(), "");
            ui.invoke_shelf_history(false);
            assert_eq!(ui.get_shelf_quick_text(), replaced);
            assert_eq!(
                (ui.get_shelf_quick_anchor(), ui.get_shelf_quick_focus()),
                (3, 3)
            );
            ui.invoke_shelf_history(true);
            assert_eq!(ui.get_shelf_quick_text(), "");
            assert_eq!(
                (ui.get_shelf_quick_anchor(), ui.get_shelf_quick_focus()),
                (0, 0)
            );
            let saved = state.project.borrow().as_ref().unwrap().clone();
            assert_eq!(saved.source(), original.source());
            let copy = root.path().join(format!("Grouped {category}.fountage"));
            project_draft::save_copy(&saved, ui.get_source().as_str(), &copy).unwrap();
            assert_eq!(Project::read(&copy).unwrap().entries(), saved.entries());
            let path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
            assert_eq!(
                recovery::project_from_snapshot(&path, &recovery::read(&path).unwrap())
                    .unwrap()
                    .unwrap()
                    .entries(),
                saved.entries()
            );
            ui.hide().unwrap();
            crate::history::set_test_time(None);
        }
    }
    #[test]
    fn quick_note_return_clipboard_and_focus_are_distinct_typing_boundaries() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let (ui, _) = setup(fixture(0), root.path().into());
        crate::history::set_test_time(Some(0));
        ui.invoke_show_shelf();
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        ui.invoke_focus_shelf_quick();
        for c in "First".chars() {
            key(&ui, c.to_string().into());
        }
        key(&ui, Key::Return.into());
        for c in "Next".chars() {
            key(&ui, c.to_string().into());
        }
        assert_eq!(ui.get_shelf_quick_text(), "First\nNext");
        ui.invoke_shelf_history(false);
        assert_eq!(ui.get_shelf_quick_text(), "First\n");
        ui.invoke_shelf_history(false);
        assert_eq!(ui.get_shelf_quick_text(), "First");
        ui.invoke_shelf_history(false);
        assert_eq!(ui.get_shelf_quick_text(), "");
        for _ in 0..3 {
            ui.invoke_shelf_history(true);
        }
        assert_eq!(ui.get_shelf_quick_text(), "First\nNext");
        ui.invoke_focus_shelf_search();
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        ui.invoke_focus_shelf_quick();
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        key(&ui, "!".into());
        ui.invoke_shelf_history(false);
        assert_eq!(ui.get_shelf_quick_text(), "First\nNext");
        let end = ui.get_shelf_quick_text().len() as i32;
        ui.invoke_select_shelf_quick(end, 0);
        ui.invoke_clipboard_action(0);
        assert_eq!(ui.get_shelf_quick_text(), "");
        ui.invoke_shelf_history(false);
        assert_eq!(ui.get_shelf_quick_text(), "First\nNext");
        assert_eq!(
            (ui.get_shelf_quick_anchor(), ui.get_shelf_quick_focus()),
            (end, 0)
        );
        ui.invoke_shelf_history(true);
        ui.invoke_clipboard_action(2);
        assert_eq!(ui.get_shelf_quick_text(), "First\nNext");
        ui.invoke_shelf_history(false);
        assert_eq!(ui.get_shelf_quick_text(), "");
        ui.hide().unwrap();
        crate::history::set_test_time(None);
    }
    #[test]
    fn quick_note_failed_group_update_and_retry_preserve_history_and_selection() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let good = root.path().join("recovery");
        let (ui, state) = setup(fixture(0), good.clone());
        let blocked = root.path().join("blocked");
        std::fs::write(&blocked, "keep").unwrap();
        let failing = Rc::new(Cell::new(false));
        let flag = failing.clone();
        install(&ui, state.clone(), move || {
            Ok(if flag.get() {
                blocked.clone()
            } else {
                good.clone()
            })
        });
        crate::history::set_test_time(Some(0));
        ui.invoke_show_shelf();
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        ui.invoke_focus_shelf_quick();
        for c in "abc".chars() {
            key(&ui, c.to_string().into());
        }
        let before = state.project.borrow().as_ref().unwrap().clone();
        let recovery_path = state.recovery.borrow().as_ref().unwrap().path().to_owned();
        failing.set(true);
        key(&ui, "d".into());
        assert!(ui.get_shelf_quick_pending());
        assert_eq!(ui.get_shelf_quick_text(), "abcd");
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            before.entries()
        );
        assert_eq!(
            state.recovery.borrow().as_ref().unwrap().path(),
            recovery_path
        );
        ui.invoke_shelf_history(false);
        assert_eq!(ui.get_shelf_quick_text(), "abcd");
        failing.set(false);
        ui.invoke_shelf_quick_capture();
        assert!(!ui.get_shelf_quick_pending());
        ui.invoke_shelf_history(false);
        assert_eq!(
            ui.get_shelf_quick_text(),
            "abc",
            "Retry is separate from the earlier typing group"
        );
        assert_eq!(
            (ui.get_shelf_quick_anchor(), ui.get_shelf_quick_focus()),
            (3, 3)
        );
        ui.invoke_shelf_history(false);
        assert_eq!(ui.get_shelf_quick_text(), "");
        ui.invoke_shelf_history(true);
        ui.invoke_shelf_history(true);
        assert_eq!(ui.get_shelf_quick_text(), "abcd");
        let committed = state.project.borrow().as_ref().unwrap().clone();
        failing.set(true);
        ui.invoke_shelf_history(false);
        assert_eq!(ui.get_shelf_quick_text(), "abcd");
        assert_eq!(
            (ui.get_shelf_quick_anchor(), ui.get_shelf_quick_focus()),
            (4, 4)
        );
        assert_eq!(
            state.project.borrow().as_ref().unwrap().entries(),
            committed.entries()
        );
        failing.set(false);
        ui.invoke_shelf_history(false);
        assert_eq!(ui.get_shelf_quick_text(), "abc");
        ui.hide().unwrap();
        crate::history::set_test_time(None);
    }
    #[test]
    fn quick_note_typing_limits_and_idle_pause_start_new_undo_groups() {
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let (ui, _) = setup(fixture(0), root.path().into());
        crate::history::set_test_time(Some(0));
        ui.invoke_show_shelf();
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        ui.invoke_focus_shelf_quick();
        for _ in 0..70 {
            key(&ui, "a".into());
        }
        ui.invoke_shelf_history(false);
        assert_eq!(ui.get_shelf_quick_text(), "a".repeat(64).as_str());
        ui.invoke_shelf_history(false);
        assert_eq!(ui.get_shelf_quick_text(), "");
        key(&ui, "b".into());
        crate::history::set_test_time(Some(800));
        key(&ui, "c".into());
        ui.invoke_shelf_history(false);
        assert_eq!(ui.get_shelf_quick_text(), "b");
        ui.invoke_shelf_history(false);
        assert_eq!(ui.get_shelf_quick_text(), "");
        ui.hide().unwrap();
        crate::history::set_test_time(None);
    }
    #[test]
    fn quick_note_ime_commit_is_separate_from_adjacent_typing() {
        use i_slint_core::input::{InternalKeyEvent, KeyEvent, KeyEventType};
        i_slint_backend_testing::init_no_event_loop();
        let root = tempfile::tempdir().unwrap();
        let (ui, _) = setup(fixture(0), root.path().into());
        crate::history::set_test_time(Some(0));
        ui.invoke_show_shelf();
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        ui.invoke_focus_shelf_quick();
        for c in "ab".chars() {
            key(&ui, c.to_string().into());
        }
        ui.window()
            .dispatch_event(WindowEvent::internal(InternalKeyEvent {
                key_event: KeyEvent::default(),
                event_type: KeyEventType::UpdateComposition,
                preedit_text: "に".into(),
                ..Default::default()
            }));
        assert!(ui.get_shelf_quick_composing());
        let mut event = KeyEvent::default();
        event.text = "日".into();
        ui.window()
            .dispatch_event(WindowEvent::internal(InternalKeyEvent {
                key_event: event,
                event_type: KeyEventType::CommitComposition,
                ..Default::default()
            }));
        assert!(!ui.get_shelf_quick_composing());
        assert_eq!(ui.get_shelf_quick_text(), "ab日");
        key(&ui, "c".into());
        ui.invoke_shelf_history(false);
        assert_eq!(ui.get_shelf_quick_text(), "ab日");
        ui.invoke_shelf_history(false);
        assert_eq!(ui.get_shelf_quick_text(), "ab");
        ui.invoke_shelf_history(false);
        assert_eq!(ui.get_shelf_quick_text(), "");
        // Disabled Undo must not replay a prior cursor restoration.
        ui.invoke_shelf_history(false);
        assert_eq!(
            (ui.get_shelf_quick_anchor(), ui.get_shelf_quick_focus()),
            (0, 0)
        );
        ui.hide().unwrap();
        crate::history::set_test_time(None);
    }
}

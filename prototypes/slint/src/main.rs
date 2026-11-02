#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use fountage_core::{
    output,
    writing::{self, Format, Kind},
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::RefCell, io::Write, path::Path, rc::Rc, time::Instant};
mod appearance;
mod bitmap_glyph;
mod document_label;
mod editor_composition;
mod editor_layout;
mod editor_paint;
mod editor_session;
#[cfg(test)]
mod editor_tests;
mod find;
mod formatted_live;
mod history;
#[cfg(target_os = "macos")]
mod keyboard_modifiers;
#[cfg(target_os = "macos")]
mod macos_history;
#[cfg(target_os = "macos")]
mod macos_panel;
#[cfg(target_os = "macos")]
mod macos_quit;
mod navigation;
mod preview_text;
mod project_draft;
pub mod project_metadata;
mod recovery;
#[cfg(test)]
mod render_tests;
mod scene_numbering;
mod session;
mod shelf_ui;
mod source_draft;
mod window_chrome;
// Exercise the exact vendored hard-break emission helper in the app regression suite.
#[cfg(test)]
#[path = "../vendor/i-slint-core/textlayout/sharedparley/accessibility/separator.rs"]
mod accessible_separator_regression;
mod worker;
slint::include_modules!();
#[derive(Debug)]
struct PlainLine {
    text: String,
    indent: f32,
    strong: bool,
    runs: Vec<output::Run>,
}
#[derive(Debug)]
struct PlainHeading {
    text: String,
    body: String,
    start: usize,
    end: usize,
}
struct NavigationData {
    manuscript_title: String,
    words: usize,
    cast: Vec<String>,
    outline: Vec<PlainHeading>,
    notes: Vec<PlainHeading>,
    tasks: Vec<PlainHeading>,
    task_count: usize,
    completed_tasks: usize,
    locations: Vec<PlainHeading>,
    location_count: usize,
}
const SAMPLE: &str = "Title: The Last Signal\nAuthor: Baz Studios\n\n# Act One\n\nINT. OBSERVATORY - NIGHT\n\nZoë watches a pale light cross the sky.\n\nMARA\nThere hasn't been a transmission in years.\n\nVOICE (V.O.)\nThen I suppose you weren't expecting me.\n\nEXT. STATION ROOF - NIGHT\n\nThe antenna turns slowly against the stars.\n";
fn derived(source: &str, category: i32) -> (String, NavigationData, Vec<PlainLine>) {
    let (a, screenplay) = if category == 0 {
        let snapshot = output::snapshot(source);
        (snapshot.analysis, Some(snapshot.output))
    } else {
        (
            writing::analyze(
                source,
                Format::from_code(category as u32).unwrap_or(Format::Screenplay),
            ),
            None,
        )
    };
    let summary = format!(
        "{} words\n{} headings · {} characters",
        a.words,
        a.headings.len(),
        a.characters.len()
    );
    let ranges = navigation::line_ranges(source);
    let outline = a
        .headings
        .iter()
        .filter_map(|h| {
            let range = ranges.get(h.line)?;
            Some(PlainHeading {
                text: h.heading.clone(),
                body: String::new(),
                start: range.start,
                end: range.end,
            })
        })
        .collect();
    let locations = a
        .locations
        .iter()
        .flat_map(|(name, occurrences)| {
            let ranges = &ranges;
            occurrences
                .iter()
                .enumerate()
                .filter_map(move |(index, line)| {
                    let range = ranges.get(*line)?;
                    Some(PlainHeading {
                        text: format!("{name} · {}/{}", index + 1, occurrences.len()),
                        body: source.get(range.clone())?.to_owned(),
                        start: range.start,
                        end: range.end,
                    })
                })
        })
        .collect();
    let manuscript_title = screenplay
        .as_ref()
        .and_then(|document| {
            document
                .title_fields
                .iter()
                .find(|field| field.key == "title")
        })
        .map(|field| {
            field
                .values
                .iter()
                .map(|line| line.text.trim())
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    let lines = if let Some(screenplay) = screenplay {
        screenplay
            .body
            .into_iter()
            .map(|l| PlainLine {
                text: l.text,
                runs: l.runs,
                indent: match l.kind {
                    Kind::Character => 110.,
                    Kind::Dialogue => 55.,
                    Kind::Parenthetical => 80.,
                    _ => 0.,
                },
                strong: matches!(l.kind, Kind::Scene | Kind::Section),
            })
            .collect()
    } else {
        source
            .lines()
            .map(|l| PlainLine {
                text: l.to_owned(),
                indent: 0.,
                strong: false,
                runs: Vec::new(),
            })
            .collect()
    };
    let tasks = a
        .todos
        .iter()
        .filter_map(|task| {
            let range = ranges.get(task.line)?;
            Some(PlainHeading {
                text: format!(
                    "[{}] {}",
                    if task.completed { "x" } else { " " },
                    if task.text.is_empty() {
                        "Untitled task"
                    } else {
                        &task.text
                    }
                ),
                body: task.text.clone(),
                start: range.start,
                end: range.end,
            })
        })
        .collect();
    let notes = a
        .notes
        .iter()
        .zip(navigation::note_ranges(source, &a.notes))
        .filter_map(|(note, range)| {
            let range = range?;
            let first = note
                .text
                .lines()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("Empty note")
                .trim();
            let mut label = first.chars().take(72).collect::<String>();
            if first.chars().count() > 72 || note.text.lines().count() > 1 {
                label.push('…');
            }
            Some(PlainHeading {
                text: label,
                body: note.text.clone(),
                start: range.start,
                end: range.end,
            })
        })
        .collect();
    (
        summary,
        NavigationData {
            manuscript_title,
            words: a.words,
            cast: a
                .characters
                .iter()
                .map(|(name, count)| format!("{name}    {count}"))
                .collect(),
            outline,
            notes,
            tasks,
            task_count: a.todos.len(),
            completed_tasks: a.todos.iter().filter(|task| task.completed).count(),
            locations,
            location_count: a.locations.len(),
        },
        lines,
    )
}
fn navigation_models(
    rows: Vec<PlainHeading>,
) -> (ModelRc<slint::StandardListViewItem>, ModelRc<OutlineRow>) {
    let rows = rows
        .into_iter()
        .filter_map(|h| {
            Some(OutlineRow {
                text: h.text.into(),
                body: h.body.into(),
                start_byte: i32::try_from(h.start).ok()?,
                end_byte: i32::try_from(h.end).ok()?,
            })
        })
        .collect::<Vec<_>>();
    let labels = rows
        .iter()
        .map(|row| slint::StandardListViewItem::from(row.text.clone()))
        .collect::<Vec<_>>();
    (
        ModelRc::new(VecModel::from(labels)),
        ModelRc::new(VecModel::from(rows)),
    )
}
fn apply(
    ui: &Studio,
    summary: String,
    navigation: NavigationData,
    lines: Vec<PlainLine>,
    milliseconds: f64,
) {
    ui.set_manuscript_title(navigation.manuscript_title.into());
    ui.set_word_count(navigation.words.to_string().into());
    ui.set_cast(ModelRc::new(VecModel::from(
        navigation
            .cast
            .into_iter()
            .map(slint::SharedString::from)
            .collect::<Vec<_>>(),
    )));
    ui.set_summary(summary.into());
    ui.set_outline_selected(-1);
    let (labels, rows) = navigation_models(navigation.outline);
    ui.set_outline_labels(labels);
    ui.set_outline_rows(rows);
    let (labels, rows) = navigation_models(navigation.notes);
    ui.set_note_labels(labels);
    ui.set_note_rows(rows);
    refresh_note_filter(ui);
    ui.set_task_status(
        format!(
            "{} of {} completed",
            navigation.completed_tasks, navigation.task_count
        )
        .into(),
    );
    let (labels, rows) = navigation_models(navigation.tasks);
    ui.set_task_labels(labels);
    ui.set_task_rows(rows);
    ui.set_location_status(
        format!(
            "{} locations · {} scenes",
            navigation.location_count,
            navigation.locations.len()
        )
        .into(),
    );
    let (labels, rows) = navigation_models(navigation.locations);
    ui.set_location_labels(labels);
    ui.set_location_rows(rows);
    let mut preview_error = None;
    ui.set_lines(ModelRc::new(VecModel::from(
        lines
            .into_iter()
            .map(|l| {
                let styled =
                    preview_text::styled(&l.text, &l.runs, l.strong).unwrap_or_else(|error| {
                        preview_error = Some(error);
                        slint::StyledText::from_plain_text(&l.text)
                    });
                PreviewLine {
                    text: l.text.into(),
                    styled,
                    indent: l.indent,
                }
            })
            .collect::<Vec<_>>(),
    )));
    ui.set_preview_error(preview_error.unwrap_or("").into());
    ui.set_analysis_status(
        format!("Engine analysis {:.1} ms · background worker", milliseconds).into(),
    );
}
// Never replace the user's original or an existing destination, even after a dialog confirms replacement.
fn save_copy(path: &Path, source: &str) -> Result<(), String> {
    if source.contains('\0') {
        return Err("Manuscript contains NUL; no copy was saved.".into());
    }
    let parent = path.parent().ok_or("No parent directory")?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    tmp.write_all(source.as_bytes())
        .map_err(|e| e.to_string())?;
    tmp.as_file().sync_all().map_err(|e| e.to_string())?;
    tmp.persist_noclobber(path).map_err(|e| e.to_string())?;
    Ok(())
}
fn install_file_actions(ui: &Studio) {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let weak = ui.as_weak();
    let pending = Arc::new(AtomicBool::new(false));
    ui.on_file_action(move |action| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if (if action == 3 {
            close_blocked(&ui)
        } else {
            ui.get_confirm_close() || ui.get_file_dialog_open()
        }) || pending.swap(true, Ordering::SeqCst)
        {
            return;
        }
        let queued = pending.clone();
        // Native menu dispatch holds the backend's menu borrow. A modal picker
        // can run display-link timers and rebuild that menu; run only after the
        // activation callback has returned and released its borrow.
        if let Err(error) = weak.upgrade_in_event_loop(move |ui| {
            if if action == 3 {
                !close_blocked(&ui)
            } else {
                !ui.get_confirm_close() && !ui.get_file_dialog_open()
            } {
                dispatch_file_action(&ui, action);
            }
            queued.store(false, Ordering::SeqCst);
        }) {
            pending.store(false, Ordering::SeqCst);
            ui.set_status(format!("File action could not start: {error}").into());
        }
    });
}
// Called only after native menu activation has released its backend borrow.
// Recheck Save eligibility because composition/modal state can change in queue.
fn dispatch_file_action(ui: &Studio, action: i32) {
    if action == 2 && !ui.get_can_save_copy() {
        return;
    }
    ui.invoke_formatted_finish();
    if action == 2 {
        ui.invoke_history_boundary();
        ui.invoke_field_boundary();
    }
    match action {
        0 => ui.invoke_open_project(),
        1 => ui.invoke_recover(),
        2 => ui.invoke_save_copy(),
        3 => ui.invoke_request_close(),
        4 => ui.invoke_open_source(),
        5 => ui.invoke_new_draft(),
        _ => (),
    }
}
// The chooser stages options only. It must not act like a native file picker
// during Quit/Close; the underlying draft still uses the usual checkpoint.
fn close_blocked(ui: &Studio) -> bool {
    ui.get_confirm_close()
        || (ui.get_file_dialog_open() && !ui.get_new_draft_open())
        || ui.get_title_page_open()
        || ui.get_numbering_open()
}
fn close_response(ui: &Studio, checkpoint: &session::Checkpoint) -> slint::CloseRequestResponse {
    if close_blocked(ui) {
        return slint::CloseRequestResponse::KeepWindowShown;
    }
    if ui.get_new_draft_open() {
        ui.set_new_draft_open(false);
        ui.set_file_dialog_open(false);
    }
    ui.invoke_formatted_finish();
    if checkpoint.is_clean(ui.get_source().as_str()) {
        slint::CloseRequestResponse::HideWindow
    } else {
        ui.invoke_history_boundary();
        ui.invoke_field_boundary();
        ui.set_confirm_close(true);
        slint::CloseRequestResponse::KeepWindowShown
    }
}
fn install_close_requests(ui: &Studio, checkpoint: Rc<RefCell<session::Checkpoint>>) {
    let weak = ui.as_weak();
    let saved = checkpoint.clone();
    ui.on_can_terminate(move || {
        weak.upgrade().is_some_and(|ui| {
            !close_blocked(&ui)
                && !ui.get_formatted_composing()
                && saved.borrow().is_clean(ui.get_source().as_str())
        })
    });
    let weak = ui.as_weak();
    let saved = checkpoint.clone();
    ui.window().on_close_requested(move || {
        weak.upgrade()
            .map_or(slint::CloseRequestResponse::KeepWindowShown, |ui| {
                close_response(&ui, &saved.borrow())
            })
    });
    let weak = ui.as_weak();
    ui.on_request_close(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if !matches!(
            close_response(&ui, &checkpoint.borrow()),
            slint::CloseRequestResponse::HideWindow
        ) {
            return;
        }
        if let Err(error) = ui.hide() {
            ui.set_status(format!("Window could not close: {error}").into());
        }
    });
}
struct DialogGuard {
    ui: slint::Weak<Studio>,
    #[cfg(target_os = "macos")]
    menu: macos_panel::PanelMenu,
}
impl DialogGuard {
    fn new(ui: &Studio) -> Self {
        ui.set_file_dialog_open(true);
        Self {
            ui: ui.as_weak(),
            #[cfg(target_os = "macos")]
            menu: macos_panel::PanelMenu::install(),
        }
    }
}
impl Drop for DialogGuard {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        self.menu.restore();
        if let Some(ui) = self.ui.upgrade() {
            ui.set_file_dialog_open(false);
            ui.invoke_focus_source();
        }
    }
}
fn save_with_dialog(
    ui: &Studio,
    source: &str,
    project: &Option<fountage_core::project::Project>,
) -> bool {
    let _dialog = DialogGuard::new(ui);
    let Some(path) = rfd::FileDialog::new()
        .set_title(if project.is_some() {
            "Save a new project copy"
        } else {
            "Save a source copy"
        })
        .set_file_name(document_label::copy_filename(
            ui.get_document_name().as_str(),
            project.is_some(),
            ui.get_category() == 0,
        ))
        .save_file()
    else {
        return false;
    };
    if let Some(project) = project {
        return match project_draft::save_copy(project, source, &path) {
            Ok(()) => {
                ui.set_status(format!("Saved project copy: {}", path.display()).into());
                true
            }
            Err(error) => {
                ui.set_status(format!("Project copy not saved: {error}").into());
                false
            }
        };
    }
    if !matches!(
        path.extension().and_then(|v| v.to_str()),
        Some("fountain" | "txt" | "md")
    ) {
        ui.set_status("Copy not saved. Choose a .fountain, .txt or .md destination.".into());
        return false;
    }
    match save_copy(&path, source) {
        Ok(()) => {
            ui.set_status(format!("Saved copy: {}", path.display()).into());
            true
        }
        Err(e) => {
            ui.set_status(format!("Copy not saved: {e}. Choose a new filename.").into());
            false
        }
    }
}
fn refresh_note_filter(ui: &Studio) {
    use slint::Model;
    let query = ui.get_note_query();
    let matcher = if query.is_empty() {
        None
    } else {
        match regex::RegexBuilder::new(&regex::escape(query.as_str()))
            .case_insensitive(true)
            .size_limit(2 * 1024 * 1024)
            .build()
        {
            Ok(pattern) => Some(pattern),
            Err(_) => {
                ui.set_visible_note_rows(ModelRc::new(VecModel::from(Vec::<OutlineRow>::new())));
                ui.set_visible_note_labels(ModelRc::new(VecModel::from(Vec::<
                    slint::StandardListViewItem,
                >::new())));
                ui.set_note_filter_status("Filter is too long or complex".into());
                ui.set_outline_selected(-1);
                return;
            }
        }
    };
    let rows = ui.get_note_rows();
    let visible = rows
        .iter()
        .filter(|row| {
            matcher
                .as_ref()
                .is_none_or(|pattern| pattern.is_match(row.body.as_str()))
        })
        .collect::<Vec<_>>();
    let count = visible.len();
    let labels = visible
        .iter()
        .map(|row| slint::StandardListViewItem::from(row.text.clone()))
        .collect::<Vec<_>>();
    ui.set_outline_selected(-1);
    ui.set_visible_note_rows(ModelRc::new(VecModel::from(visible)));
    ui.set_visible_note_labels(ModelRc::new(VecModel::from(labels)));
    ui.set_note_filter_status(format!("{count} of {} notes", rows.row_count()).into());
}
fn install_task_removal(ui: &Studio) {
    let pending = Rc::new(RefCell::new(
        None::<(String, i32, usize, history::SelectionState)>,
    ));
    let weak = ui.as_weak();
    let request = pending.clone();
    ui.on_request_task_removal(move || {
        use slint::Model;
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if !ui.get_can_toggle_task() {
            return;
        }
        let Some(row) = usize::try_from(ui.get_outline_selected())
            .ok()
            .and_then(|i| ui.get_task_rows().row_data(i))
        else {
            return;
        };
        let source = ui.get_source().to_string();
        let Some(range) = navigation::guarded_range(
            &source,
            ui.get_outline_source().as_str(),
            ui.get_category(),
            ui.get_outline_category(),
            row.start_byte,
            row.end_byte,
        ) else {
            return;
        };
        let Some(format) = Format::from_code(ui.get_category() as u32) else {
            return;
        };
        if let Err(error) = fountage_core::todo::remove(&source, format, range.start) {
            ui.set_status(error.into());
            if ui.get_formatted_active() {
                ui.set_formatted_status(error.into());
            }
            return;
        }
        let (Ok(anchor), Ok(focus)) = (
            usize::try_from(ui.get_source_anchor()),
            usize::try_from(ui.get_source_focus()),
        ) else {
            return;
        };
        *request.borrow_mut() = Some((
            source,
            ui.get_category(),
            range.start,
            history::SelectionState {
                anchor,
                focus,
                anchor_upstream: ui.get_formatted_anchor_upstream(),
                focus_upstream: ui.get_formatted_focus_upstream(),
            },
        ));
        ui.set_task_removal_label(row.body);
        ui.invoke_history_boundary();
        ui.set_file_dialog_open(true);
        ui.set_task_removal_open(true);
    });
    let weak = ui.as_weak();
    ui.on_task_removal_action(move |remove| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if !ui.get_task_removal_open() || !ui.get_file_dialog_open() || ui.get_confirm_close() {
            return;
        }
        let Some((source, category, start, selection)) = pending.borrow_mut().take() else {
            return;
        };
        ui.set_task_removal_open(false);
        ui.set_file_dialog_open(false);
        if ui.get_source().as_str() != source || ui.get_category() != category {
            ui.set_status("The manuscript changed. No task was removed.".into());
            ui.invoke_focus_source();
            return;
        }
        ui.invoke_restore_source_selection(
            selection.anchor as i32,
            selection.focus as i32,
            selection.anchor_upstream,
            selection.focus_upstream,
        );
        if !remove {
            return;
        }
        match fountage_core::todo::remove(
            &source,
            Format::from_code(category as u32).unwrap(),
            start,
        ) {
            Ok(edit) => apply_source_edit(
                &ui,
                &source,
                edit,
                "Removed to-do · Undo restores the complete line",
            ),
            Err(error) => ui.set_status(error.into()),
        }
    });
}
fn install_outline_navigation(ui: &Studio) {
    install_task_removal(ui);
    let weak = ui.as_weak();
    ui.on_toggle_task(move || {
        use slint::Model;
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if !ui.get_can_toggle_task() {
            return;
        }
        let Some(row) = usize::try_from(ui.get_outline_selected())
            .ok()
            .and_then(|i| ui.get_task_rows().row_data(i))
        else {
            return;
        };
        let source = ui.get_source().to_string();
        let range = navigation::guarded_range(
            &source,
            ui.get_outline_source().as_str(),
            ui.get_category(),
            ui.get_outline_category(),
            row.start_byte,
            row.end_byte,
        );
        let result = range
            .ok_or("Tasks are updating. Select the item again when analysis finishes.")
            .and_then(|range| {
                fountage_core::todo::toggle(
                    &source,
                    Format::from_code(ui.get_category() as u32)
                        .ok_or("Unsupported writing category.")?,
                    range.start,
                )
            });
        match result {
            Ok(edit) => apply_source_edit(&ui, &source, edit, "Updated to-do"),
            Err(error) => ui.set_status(error.into()),
        }
    });
    let weak = ui.as_weak();
    ui.on_note_filter_changed(move || {
        if let Some(ui) = weak.upgrade() {
            refresh_note_filter(&ui);
        }
    });
    let weak = ui.as_weak();
    ui.on_navigate(move |index| {
        if let Some(ui) = weak.upgrade() {
            navigate_to(&ui, index, true);
        }
    });
    let weak = ui.as_weak();
    ui.on_browse_navigation(move |index| {
        if let Some(ui) = weak.upgrade() {
            navigate_to(&ui, index, false);
        }
    });
}
fn navigate_to(ui: &Studio, index: i32, activate: bool) {
    use slint::Model;
    if ui.get_confirm_close() || ui.get_file_dialog_open() {
        return;
    }
    let Some(row) = usize::try_from(index)
        .ok()
        .and_then(|i| match ui.get_navigation_mode() {
            0 => ui.get_outline_rows().row_data(i),
            1 => ui.get_visible_note_rows().row_data(i),
            2 if ui.get_category() == 0 => ui.get_location_rows().row_data(i),
            3 => ui.get_task_rows().row_data(i),
            _ => None,
        })
    else {
        return;
    };
    if let Some(range) = navigation::guarded_range(
        ui.get_source().as_str(),
        ui.get_outline_source().as_str(),
        ui.get_category(),
        ui.get_outline_category(),
        row.start_byte,
        row.end_byte,
    ) {
        if activate {
            ui.invoke_select_source(range.start as i32, range.end as i32);
        } else {
            ui.invoke_browse_source(range.start as i32, range.end as i32);
        }
    } else {
        ui.set_status(
            "Navigation is updating. Select the item again when analysis finishes.".into(),
        );
    }
}
fn field_source(ui: &Studio, kind: i32) -> slint::SharedString {
    if kind == 1 {
        ui.get_find_query()
    } else if kind == 2 {
        ui.get_replacement()
    } else {
        ui.get_note_query()
    }
}
fn reflect_field_history(ui: &Studio, fields: &[history::History; 3]) {
    let (undo, redo) = fields[0].availability(ui.get_find_query().as_str());
    ui.set_query_can_undo(undo);
    ui.set_query_can_redo(redo);
    let (undo, redo) = fields[1].availability(ui.get_replacement().as_str());
    ui.set_replacement_can_undo(undo);
    ui.set_replacement_can_redo(redo);
    let (undo, redo) = fields[2].availability(ui.get_note_query().as_str());
    ui.set_note_query_can_undo(undo);
    ui.set_note_query_can_redo(redo);
}
fn install_field_history(ui: &Studio) {
    let fields = Rc::new(RefCell::new([
        history::History::default(),
        history::History::default(),
        history::History::default(),
    ]));
    fields.borrow_mut()[0].reset(ui.get_find_query().as_str());
    fields.borrow_mut()[1].reset(ui.get_replacement().as_str());
    fields.borrow_mut()[2].reset(ui.get_note_query().as_str());
    reflect_field_history(ui, &fields.borrow());
    let weak = ui.as_weak();
    let histories = fields.clone();
    ui.on_field_sync(move || {
        if let Some(ui) = weak.upgrade() {
            let mut fields = histories.borrow_mut();
            fields[0].synchronize(ui.get_find_query().as_str());
            fields[1].synchronize(ui.get_replacement().as_str());
            fields[2].synchronize(ui.get_note_query().as_str());
            reflect_field_history(&ui, &fields);
        }
    });
    let histories = fields.clone();
    ui.on_field_boundary(move || {
        for field in histories.borrow_mut().iter_mut() {
            field.boundary();
        }
    });
    let weak = ui.as_weak();
    let histories = fields.clone();
    ui.on_field_input(move |kind, text| {
        if !(1..=3).contains(&kind) {
            return;
        }
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if ui.get_confirm_close() || ui.get_file_dialog_open() {
            return;
        }
        let mut fields = histories.borrow_mut();
        let field = &mut fields[(kind - 1) as usize];
        field.synchronize(field_source(&ui, kind).as_str());
        field.input(text.as_str());
    });
    let weak = ui.as_weak();
    let histories = fields.clone();
    ui.on_field_edited(move |kind, value| {
        if !(1..=3).contains(&kind) {
            return;
        }
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if ui.get_confirm_close() || ui.get_file_dialog_open() {
            return;
        }
        let mut fields = histories.borrow_mut();
        fields[(kind - 1) as usize].record(value.as_str());
        reflect_field_history(&ui, &fields);
    });
    let weak = ui.as_weak();
    ui.on_field_history_action(move |kind, redo| {
        if !(1..=3).contains(&kind) {
            return;
        }
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if ui.get_confirm_close() || ui.get_file_dialog_open() || kind != ui.get_history_context() {
            return;
        }
        let result =
            fields.borrow_mut()[(kind - 1) as usize].step(field_source(&ui, kind).as_str(), redo);
        if let Some((value, anchor, focus)) = result {
            if kind == 1 {
                ui.set_find_query(value.into());
            } else if kind == 2 {
                ui.set_replacement(value.into());
            } else {
                ui.set_note_query(value.into());
            }
            ui.invoke_select_field(kind, anchor as i32, focus as i32);
        }
        reflect_field_history(&ui, &fields.borrow());
    });
}
fn find_options(ui: &Studio) -> find::Options {
    find::Options {
        match_case: ui.get_find_match_case(),
        whole_word: ui.get_find_mode() == 2,
        starts_with: ui.get_find_mode() == 1,
        wrap: ui.get_find_wrap(),
        regex: ui.get_find_regex(),
    }
}
fn install_appearance(ui: &Studio, path: Result<std::path::PathBuf, String>) {
    match path
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|path| appearance::load(path))
    {
        Ok(value) => {
            ui.set_dark(value.dark);
            ui.set_white_paper(value.white_paper);
        }
        Err(error) => ui.set_status(format!("Appearance preference not loaded: {error}").into()),
    }
    let weak = ui.as_weak();
    ui.on_appearance_changed(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if ui.get_confirm_close() || ui.get_file_dialog_open() {
            return;
        }
        let value = appearance::Appearance {
            dark: ui.get_dark(),
            white_paper: ui.get_white_paper(),
        };
        match path
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|path| appearance::save(path, &value))
        {
            Ok(()) => {
                if ui.get_status().starts_with("Appearance preference") {
                    ui.set_status("Appearance preference saved".into());
                }
            }
            Err(error) => ui.set_status(format!("Appearance preference not saved: {error}").into()),
        }
    });
}
#[derive(Clone)]
struct DraftSession {
    checkpoint: Rc<RefCell<session::Checkpoint>>,
    history: Rc<RefCell<history::History>>,
    project: Rc<RefCell<Option<fountage_core::project::Project>>>,
    recovery: Rc<RefCell<Result<recovery::Recovery, String>>>,
}
fn install_new_draft(
    ui: &Studio,
    session: DraftSession,
    root: impl Fn() -> Result<std::path::PathBuf, String> + 'static,
    notify: impl Fn(&Studio) + 'static,
) {
    let weak = ui.as_weak();
    let saved = session.checkpoint.clone();
    ui.on_new_draft(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if ui.get_confirm_close() || ui.get_file_dialog_open() {
            return;
        }
        if !saved.borrow().is_clean(ui.get_source().as_str()) {
            ui.set_status("Save a copy of the current draft before starting a new project.".into());
            return;
        }
        ui.invoke_history_boundary();
        ui.set_new_show_open_actions(false);
        ui.set_file_dialog_open(true);
        ui.set_new_draft_open(true);
    });
    let weak = ui.as_weak();
    ui.on_new_category(move |category| {
        let Some(ui) = weak.upgrade() else { return; };
        if !ui.get_new_draft_open() || !ui.get_file_dialog_open() || ui.get_confirm_close() { return; }
        if category == -2 || category == -3 {
            ui.set_new_draft_open(false);
            ui.set_file_dialog_open(false);
            // Release chooser locks before entering the shared deferred picker path.
            ui.invoke_file_action(if category == -2 { 0 } else { 4 });
            return;
        }
        if category != -1 {
            if !session.checkpoint.borrow().is_clean(ui.get_source().as_str()) {
                ui.set_status("The current draft changed; it was kept open.".into());
            } else {
                let result = root().and_then(|root| project_draft::create(category, ui.get_new_include_starter(), ui.get_new_academic_style(), ui.get_new_layout_choice(), &root));
                match result {
                    Ok((project, baseline)) => {
                        let source = project.source().to_owned();
                        let label = ["screenplay", "comic", "novel", "academic draft", "stageplay", "audio draft", "general draft", "poetry"] [category as usize];
                        ui.set_academic_style_label(project.academic_style_label().unwrap_or("").into());
                        session.checkpoint.borrow_mut().set_metadata(Some(project.metadata()));
                        *session.project.borrow_mut() = Some(project);
                        ui.invoke_shelf_reset();
                        *session.recovery.borrow_mut() = Ok(baseline);
                        session.history.borrow_mut().reset(&source);
                        session.checkpoint.borrow_mut().require_save();
                        ui.set_project_mode(true);
                        ui.set_document_name(format!("Untitled {label}").into());
                        ui.set_category(category);
                        ui.set_source(source.clone().into());
                        // Clear the previous document immediately while the
                        // background worker receives the new session revision.
                        let (summary, headings, lines) = derived(&source, category);
                        ui.set_outline_source(source.into());
                        ui.set_outline_category(category);
                        apply(&ui, summary, headings, lines, 0.);
                        ui.set_find_open(false);
                        ui.set_replace_open(false);
                        reflect_history(&ui, &session.history.borrow());
                        reflect_checkpoint(&ui, &session.checkpoint.borrow());
                        update_recovery(&ui, &session.recovery.borrow());
                        ui.set_status("New project · Save a project copy to keep your draft and writing category".into());
                        notify(&ui);
                    }
                    Err(error) => {
                        ui.set_status(format!("New project could not be created: {error}").into());
                        // Keep staged options available for retry. Never adopt a
                        // source-only draft when complete recovery publication fails.
                        return;
                    },
                }
            }
        }
        ui.set_new_draft_open(false);
        ui.set_file_dialog_open(false);
        ui.invoke_focus_source();
    });
}
fn title_values(ui: &Studio) -> [slint::SharedString; 6] {
    [
        ui.get_title_page_title(),
        ui.get_title_page_credit(),
        ui.get_title_page_author(),
        ui.get_title_page_source(),
        ui.get_title_page_date(),
        ui.get_title_page_contact(),
    ]
}
fn reflect_title_history(ui: &Studio, histories: &[history::History; 6]) {
    let available = usize::try_from(ui.get_title_field_index())
        .ok()
        .filter(|i| *i < 6)
        .map(|i| histories[i].availability(title_values(ui)[i].as_str()))
        .unwrap_or((false, false));
    ui.set_title_can_undo(available.0);
    ui.set_title_can_redo(available.1);
}
fn install_title_page(ui: &Studio) {
    let fields = Rc::new(RefCell::new(std::array::from_fn::<_, 6, _>(|_| {
        history::History::default()
    })));
    let weak = ui.as_weak();
    let histories = fields.clone();
    ui.on_title_field_focus_changed(move || {
        if let Some(ui) = weak.upgrade() {
            reflect_title_history(&ui, &histories.borrow());
        }
    });
    let histories = fields.clone();
    ui.on_title_field_input(move |index, text| {
        if let Ok(index) = usize::try_from(index)
            && let Some(field) = histories.borrow_mut().get_mut(index)
        {
            field.input(text.as_str());
        }
    });
    let histories = fields.clone();
    ui.on_title_field_boundary(move |index| {
        if let Ok(index) = usize::try_from(index)
            && let Some(field) = histories.borrow_mut().get_mut(index)
        {
            field.boundary();
        }
    });
    let weak = ui.as_weak();
    let histories = fields.clone();
    ui.on_title_field_edited(move |index, value| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if !ui.get_title_page_open() || !ui.get_title_page_ready() {
            return;
        }
        if let Ok(index) = usize::try_from(index)
            && let Some(field) = histories.borrow_mut().get_mut(index)
        {
            field.record(value.as_str());
        }
        reflect_title_history(&ui, &histories.borrow());
    });
    let weak = ui.as_weak();
    let histories = fields.clone();
    ui.on_title_field_history(move |redo| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if !ui.get_title_page_input_focused()
            || ui.get_confirm_close()
            || ui.get_title_page_composing()
        {
            return;
        }
        let Ok(index) = usize::try_from(ui.get_title_field_index()) else {
            return;
        };
        if index >= 6 {
            return;
        }
        let step = histories.borrow_mut()[index].step(title_values(&ui)[index].as_str(), redo);
        if let Some((value, anchor, focus)) = step {
            match index {
                0 => ui.set_title_page_title(value.into()),
                1 => ui.set_title_page_credit(value.into()),
                2 => ui.set_title_page_author(value.into()),
                3 => ui.set_title_page_source(value.into()),
                4 => ui.set_title_page_date(value.into()),
                _ => ui.set_title_page_contact(value.into()),
            }
            ui.invoke_select_title_field(index as i32, anchor as i32, focus as i32);
        }
        reflect_title_history(&ui, &histories.borrow());
    });
    let request = Rc::new(RefCell::new(
        None::<(fountage_core::title_page::Draft, i32, i32)>,
    ));
    let weak = ui.as_weak();
    let snapshot = request.clone();
    ui.on_title_page_requested(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if ui.get_category() != 0 || ui.get_confirm_close() || ui.get_file_dialog_open() {
            return;
        }
        let parsed = fountage_core::title_page::Draft::parse(ui.get_source().as_str());
        let (values, ready, error) = match parsed {
            Ok(draft) => {
                let values = draft.values.clone();
                *snapshot.borrow_mut() =
                    Some((draft, ui.get_source_anchor(), ui.get_source_focus()));
                (values, true, "")
            }
            Err(error) => {
                snapshot.borrow_mut().take();
                (std::array::from_fn(|_| String::new()), false, error)
            }
        };
        ui.set_title_page_title(values[0].clone().into());
        ui.set_title_page_credit(values[1].clone().into());
        ui.set_title_page_author(values[2].clone().into());
        ui.set_title_page_source(values[3].clone().into());
        ui.set_title_page_date(values[4].clone().into());
        ui.set_title_page_contact(values[5].clone().into());
        for (field, value) in fields.borrow_mut().iter_mut().zip(&values) {
            field.reset(value);
        }
        ui.set_title_can_undo(false);
        ui.set_title_can_redo(false);
        ui.set_title_page_error(error.into());
        ui.set_title_page_ready(ready);
        ui.set_file_dialog_open(true);
        ui.set_title_page_open(true);
    });
    let weak = ui.as_weak();
    ui.on_title_page_action(move |apply| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if !ui.get_title_page_open() || !ui.get_file_dialog_open() || ui.get_confirm_close() {
            return;
        }
        if !apply {
            ui.set_title_page_open(false);
            ui.set_file_dialog_open(false);
            if let Some((draft, anchor, focus)) = request.borrow_mut().take() {
                if ui.get_source().as_str() == draft.source() {
                    ui.invoke_select_source(anchor, focus);
                } else {
                    ui.invoke_focus_source();
                }
            } else {
                ui.invoke_focus_source();
            }
            return;
        }
        if ui.get_title_page_composing() {
            ui.set_title_page_error("Finish composing text before applying.".into());
            return;
        }
        let values = [
            ui.get_title_page_title(),
            ui.get_title_page_credit(),
            ui.get_title_page_author(),
            ui.get_title_page_source(),
            ui.get_title_page_date(),
            ui.get_title_page_contact(),
        ]
        .map(|value| value.to_string());
        let result = {
            let request = request.borrow();
            let Some((draft, anchor, focus)) = request.as_ref() else {
                return;
            };
            if ui.get_source().as_str() != draft.source() || ui.get_category() != 0 {
                ui.set_title_page_error(
                    "The manuscript changed. Cancel and reopen Title Page.".into(),
                );
                return;
            }
            draft
                .edit(&values)
                .map(|updated| (updated, draft.source().to_owned(), *anchor, *focus))
        };
        match result {
            Err(error) => ui.set_title_page_error(error.into()),
            Ok((updated, before, anchor, focus)) => {
                request.borrow_mut().take();
                ui.set_title_page_open(false);
                ui.set_file_dialog_open(false);
                if let Some(updated) = updated {
                    ui.invoke_history_boundary();
                    ui.invoke_history_selection(before.into(), anchor, focus);
                    ui.set_source(updated.clone().into());
                    ui.invoke_editor_edited(updated.into(), 0, 0);
                    ui.invoke_history_boundary();
                    ui.invoke_select_source(0, 0);
                    ui.set_status("Updated title page".into());
                } else {
                    ui.invoke_select_source(anchor, focus);
                }
            }
        }
    });
}
fn apply_source_edit(
    ui: &Studio,
    source: &str,
    edit: fountage_core::formatting::Edit,
    status: &str,
) {
    let (anchor, focus) = (ui.get_source_anchor(), ui.get_source_focus());
    ui.invoke_history_boundary();
    if ui.get_formatted_active() {
        ui.invoke_history_visual_selection(
            source.into(),
            anchor,
            focus,
            ui.get_formatted_anchor_upstream(),
            ui.get_formatted_focus_upstream(),
        );
    } else {
        ui.invoke_history_selection(source.into(), anchor, focus);
    }
    ui.set_source(edit.source.clone().into());
    ui.invoke_editor_edited(edit.source.into(), edit.anchor as i32, edit.focus as i32);
    ui.invoke_history_boundary();
    ui.invoke_select_source(edit.anchor as i32, edit.focus as i32);
    ui.set_status(status.into());
}

fn install_formatting(ui: &Studio) {
    let weak = ui.as_weak();
    ui.on_task_newline(move || {
        let Some(ui) = weak.upgrade() else {
            return false;
        };
        if !ui.get_can_edit_source() {
            return false;
        }
        let (Ok(anchor), Ok(focus), Some(format)) = (
            usize::try_from(ui.get_source_anchor()),
            usize::try_from(ui.get_source_focus()),
            Format::from_code(ui.get_category() as u32),
        ) else {
            return false;
        };
        let source = ui.get_source().to_string();
        let Some(edit) = fountage_core::todo::newline(&source, format, anchor, focus) else {
            return false;
        };
        apply_source_edit(&ui, &source, edit, "Updated to-do list");
        true
    });
    let weak = ui.as_weak();
    ui.on_insert_tasks(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if !ui.get_can_edit_source() {
            return;
        }
        let (Ok(anchor), Ok(focus), Some(format)) = (
            usize::try_from(ui.get_source_anchor()),
            usize::try_from(ui.get_source_focus()),
            Format::from_code(ui.get_category() as u32),
        ) else {
            return;
        };
        let source = ui.get_source().to_string();
        match fountage_core::todo::insert(&source, format, anchor, focus) {
            Ok(Some(edit)) => apply_source_edit(&ui, &source, edit, "Created to-do list"),
            Ok(None) => ui.set_status("Selected lines are already tasks · No text changed".into()),
            Err(error) => {
                ui.set_status(error.into());
                if ui.get_formatted_active() {
                    ui.set_formatted_status(error.into());
                }
            }
        }
    });
    let weak = ui.as_weak();
    ui.on_note_source(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if !ui.get_can_edit_source() {
            return;
        }
        let (Ok(anchor), Ok(focus)) = (
            usize::try_from(ui.get_source_anchor()),
            usize::try_from(ui.get_source_focus()),
        ) else {
            return;
        };
        let source = ui.get_source().to_string();
        match fountage_core::script_notes::edit(&source, anchor, focus) {
            Ok(Some(edit)) => apply_source_edit(&ui, &source, edit, "Updated script notes"),
            Ok(None) => ui.set_status("No note text selected · No text changed".into()),
            Err(error) => {
                ui.set_status(error.into());
                if ui.get_formatted_active() {
                    ui.set_formatted_status(error.into());
                }
            }
        }
    });
    let weak = ui.as_weak();
    ui.on_format_source(move |code| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if !ui.get_can_format_source() {
            return;
        }
        let Some(style) = fountage_core::formatting::Style::from_code(code) else {
            return;
        };
        let (Ok(anchor), Ok(focus)) = (
            usize::try_from(ui.get_source_anchor()),
            usize::try_from(ui.get_source_focus()),
        ) else {
            return;
        };
        let source = ui.get_source().to_string();
        match fountage_core::formatting::edit(&source, anchor, focus, style) {
            Ok(Some(edit)) => {
                apply_source_edit(&ui, &source, edit, "Updated Fountain formatting");
            }
            Ok(None) => ui.set_status("Select some text to format · No text changed".into()),
            Err(error) => {
                ui.set_status(error.into());
                if ui.get_formatted_active() {
                    ui.set_formatted_status(error.into());
                }
            }
        }
    });
}

fn install_scene_numbering(ui: &Studio) {
    let request = Rc::new(RefCell::new(None::<(String, usize, usize)>));
    let weak = ui.as_weak();
    let snapshot = request.clone();
    ui.on_numbering_requested(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if ui.get_category() != 0 || ui.get_confirm_close() || ui.get_file_dialog_open() {
            return;
        }
        let source = ui.get_source().to_string();
        let (Ok(anchor), Ok(focus)) = (
            usize::try_from(ui.get_source_anchor()),
            usize::try_from(ui.get_source_focus()),
        ) else {
            return;
        };
        ui.set_numbering_selection_available(anchor != focus);
        ui.set_numbering_selected_only(false);
        ui.set_numbering_start("1".into());
        ui.set_numbering_error("".into());
        *snapshot.borrow_mut() = Some((source, anchor, focus));
        ui.set_file_dialog_open(true);
        ui.set_numbering_open(true);
    });
    let weak = ui.as_weak();
    ui.on_numbering_action(move |action| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if !ui.get_numbering_open() || !ui.get_file_dialog_open() || ui.get_confirm_close() {
            return;
        }
        let Some((source, anchor, focus)) = request.borrow().clone() else {
            return;
        };
        if action == -1 {
            ui.set_numbering_open(false);
            ui.set_file_dialog_open(false);
            request.borrow_mut().take();
            if ui.get_source().as_str() == source {
                ui.invoke_select_source(anchor as i32, focus as i32);
            } else {
                ui.invoke_focus_source();
            }
            return;
        }
        if !(0..=1).contains(&action) {
            return;
        }
        if ui.get_source().as_str() != source || ui.get_category() != 0 {
            ui.set_numbering_error(
                "The draft changed. Cancel and open Scene Numbers again.".into(),
            );
            return;
        }
        let first = if action == 1 {
            Ok(1)
        } else {
            ui.get_numbering_start()
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|v| *v > 0)
                .ok_or("Enter a starting number from 1 to 4294967295.".to_owned())
        };
        let result = first.and_then(|first| {
            scene_numbering::plan(
                &source,
                anchor,
                focus,
                first,
                action == 1,
                ui.get_numbering_selected_only(),
            )
        });
        match result {
            Err(error) => ui.set_numbering_error(error.into()),
            Ok(plan) => {
                ui.set_numbering_open(false);
                ui.set_file_dialog_open(false);
                request.borrow_mut().take();
                if let Some(plan) = plan {
                    ui.invoke_history_boundary();
                    ui.invoke_history_selection(source.into(), anchor as i32, focus as i32);
                    ui.set_source(plan.source.clone().into());
                    ui.invoke_editor_edited(
                        plan.source.into(),
                        plan.anchor as i32,
                        plan.focus as i32,
                    );
                    ui.invoke_history_boundary();
                    ui.invoke_select_source(plan.anchor as i32, plan.focus as i32);
                    ui.set_status(
                        if action == 1 {
                            "Removed scene numbers"
                        } else {
                            "Numbered scene headings"
                        }
                        .into(),
                    );
                } else {
                    ui.invoke_select_source(anchor as i32, focus as i32);
                    ui.set_status("Scene numbers already up to date · No text changed".into());
                }
            }
        }
    });
}
fn install_find(ui: &Studio) {
    install_field_history(ui);
    let weak = ui.as_weak();
    let search = Rc::new(RefCell::new(find::Search::default()));
    let invalidated_search = search.clone();
    let invalidated_weak = ui.as_weak();
    ui.on_find_invalidated(move || {
        let Some(ui) = invalidated_weak.upgrade() else {
            return;
        };
        if !invalidated_search.borrow().has_input(
            ui.get_source().as_str(),
            ui.get_find_query().as_str(),
            find_options(&ui),
        ) {
            ui.set_find_can_replace(false);
            ui.set_find_status(
                if ui.get_find_query().is_empty() {
                    "Enter text to search"
                } else {
                    "Press Return to search current draft"
                }
                .into(),
            );
        }
    });
    let replace_search = search.clone();
    let replace_weak = ui.as_weak();
    let replace_handler = Rc::new(move |all: bool, advance: bool| {
        let Some(ui) = replace_weak.upgrade() else {
            return;
        };
        if ui.get_confirm_close()
            || ui.get_file_dialog_open()
            || !ui.get_find_open()
            || !ui.get_replace_open()
        {
            return;
        }
        let source = ui.get_source();
        let query = ui.get_find_query();
        let replacement = ui.get_replacement();
        let plan = if all {
            find::replace_all(
                source.as_str(),
                query.as_str(),
                find_options(&ui),
                replacement.as_str(),
            )
        } else {
            replace_search.borrow().replace_current(
                source.as_str(),
                query.as_str(),
                find_options(&ui),
                replacement.as_str(),
            )
        };
        replace_search
            .borrow_mut()
            .replaced(source.as_str(), query.as_str(), find_options(&ui));
        ui.set_find_can_replace(false);
        match plan {
            Ok(Some(plan)) => {
                let changed = plan.source != source.as_str();
                if changed {
                    // One source transaction, including checkpoint/recovery/preview.
                    ui.invoke_history_boundary();
                    replace_search.borrow_mut().replaced(
                        &plan.source,
                        query.as_str(),
                        find_options(&ui),
                    );
                    ui.set_source(plan.source.clone().into());
                    ui.invoke_edited(plan.source.clone().into());
                    ui.invoke_history_boundary();
                    ui.invoke_select_source(plan.selection.start as i32, plan.selection.end as i32);
                }
                ui.set_find_can_replace(false);
                let outcome = if changed {
                    format!(
                        "Replaced {} {}",
                        plan.count,
                        if plan.count == 1 { "match" } else { "matches" }
                    )
                } else {
                    "No text changes needed".to_owned()
                };
                ui.set_find_status(outcome.clone().into());
                if advance {
                    let next = replace_search.borrow_mut().continue_after(
                        &plan.source,
                        query.as_str(),
                        find_options(&ui),
                        plan.selection.end,
                        plan.first_match_empty,
                    );
                    match next {
                        Ok(Some(found)) => {
                            if let (Ok(start), Ok(end)) = (
                                i32::try_from(found.range.start),
                                i32::try_from(found.range.end),
                            ) {
                                ui.invoke_select_source(start, end);
                                ui.set_find_can_replace(true);
                                ui.set_find_status(
                                    format!(
                                        "{outcome} · {} of {} matches",
                                        found.ordinal, found.count
                                    )
                                    .into(),
                                );
                            } else {
                                ui.set_find_status(
                                    "Next match exceeds the editor's supported offset range."
                                        .into(),
                                );
                            }
                        }
                        Ok(None) => ui.set_find_status(format!("{outcome} · No matches").into()),
                        Err(error) => ui.set_find_status(format!("{outcome} · {error}").into()),
                    }
                }
            }
            Ok(None) => {
                ui.set_find_can_replace(false);
                ui.set_find_status("No matches".into());
            }
            Err(error) => {
                ui.set_find_can_replace(false);
                ui.set_find_status(error.into());
            }
        }
    });
    let replace_only = replace_handler.clone();
    ui.on_replace_match(move |all| replace_only(all, false));
    ui.on_replace_and_find(move || replace_handler(false, true));
    ui.on_find_step(move |backwards| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if ui.get_confirm_close() || ui.get_file_dialog_open() {
            return;
        }
        let source = ui.get_source();
        let query = ui.get_find_query();
        ui.set_find_can_replace(false);
        match search.borrow_mut().step(
            source.as_str(),
            query.as_str(),
            find_options(&ui),
            backwards,
        ) {
            Ok(Some(found)) => {
                let (Ok(start), Ok(end)) = (
                    i32::try_from(found.range.start),
                    i32::try_from(found.range.end),
                ) else {
                    ui.set_find_status(
                        "Match is beyond the editor's supported offset range.".into(),
                    );
                    return;
                };
                ui.set_find_can_replace(true);
                ui.set_find_status(format!("{} of {} matches", found.ordinal, found.count).into());
                ui.invoke_select_source(start, end);
            }
            Ok(None) => ui.set_find_status(
                if query.is_empty() {
                    "Enter text to search"
                } else {
                    "No matches"
                }
                .into(),
            ),
            Err(error) => ui.set_find_status(error.into()),
        }
        if !ui.get_find_open() {
            ui.set_status(format!("Find · {}", ui.get_find_status()).into());
        }
    });
}
fn reflect_checkpoint(ui: &Studio, checkpoint: &session::Checkpoint) {
    ui.set_dirty(!checkpoint.is_clean(ui.get_source().as_str()));
}
fn reflect_history(ui: &Studio, history: &history::History) {
    let (undo, redo) = history.availability(ui.get_source().as_str());
    ui.set_can_undo(undo);
    ui.set_can_redo(redo);
}
fn install_editor_history(
    ui: &Studio,
    changed: impl Fn(&Studio) + 'static,
) -> Rc<RefCell<history::History>> {
    let history = Rc::new(RefCell::new(history::History::default()));
    history.borrow_mut().reset(ui.get_source().as_str());
    reflect_history(ui, &history.borrow());
    let input_history = history.clone();
    ui.on_history_input(move |text| input_history.borrow_mut().input(text.as_str()));
    let boundary_history = history.clone();
    ui.on_history_boundary(move || boundary_history.borrow_mut().boundary());
    let changed = Rc::new(changed);
    let selections = history.clone();
    ui.on_history_selection(move |source, anchor, focus| {
        if let (Ok(anchor), Ok(focus)) = (usize::try_from(anchor), usize::try_from(focus)) {
            selections
                .borrow_mut()
                .observe_selection(source.as_str(), anchor, focus);
        }
    });
    let selections = history.clone();
    ui.on_history_visual_selection(move |source, a, f, au, fu| {
        if let (Ok(anchor), Ok(focus)) = (usize::try_from(a), usize::try_from(f)) {
            selections.borrow_mut().observe_visual_selection(
                source.as_str(),
                history::SelectionState {
                    anchor,
                    focus,
                    anchor_upstream: au,
                    focus_upstream: fu,
                },
            );
        }
    });
    let weak = ui.as_weak();
    let edits = history.clone();
    let notify = changed.clone();
    let record = Rc::new(
        move |source: slint::SharedString, selection: Option<history::SelectionState>| {
            if let Some(selection) = selection {
                edits
                    .borrow_mut()
                    .record_visual_selected(source.as_str(), selection);
            } else {
                edits.borrow_mut().record(source.as_str());
            }
            if let Some(ui) = weak.upgrade() {
                reflect_history(&ui, &edits.borrow());
                notify(&ui);
            }
        },
    );
    let plain = record.clone();
    ui.on_edited(move |source| plain(source, None));
    let visual_record = record.clone();
    ui.on_editor_visual_edited(move |source, a, f, au, fu| {
        visual_record(
            source,
            usize::try_from(a)
                .ok()
                .zip(usize::try_from(f).ok())
                .map(|(anchor, focus)| history::SelectionState {
                    anchor,
                    focus,
                    anchor_upstream: au,
                    focus_upstream: fu,
                }),
        );
    });
    ui.on_editor_edited(move |source, anchor, focus| {
        record(
            source,
            usize::try_from(anchor)
                .ok()
                .zip(usize::try_from(focus).ok())
                .map(|(a, f)| history::SelectionState::plain(a, f)),
        )
    });
    let weak = ui.as_weak();
    let edits = history.clone();
    ui.on_history_action(move |redo| {
        if let Some(ui) = weak.upgrade() {
            if ui.get_confirm_close() || ui.get_file_dialog_open() {
                return;
            }
            if ui.invoke_project_history_action(redo) {
                reflect_history(&ui, &edits.borrow());
                return;
            }
            let result = edits
                .borrow_mut()
                .step_visual(ui.get_source().as_str(), redo);
            if let Some((source, selection)) = result {
                ui.set_source(source.into());
                ui.invoke_restore_source_selection(
                    selection.anchor as i32,
                    selection.focus as i32,
                    selection.anchor_upstream,
                    selection.focus_upstream,
                );
                changed(&ui);
            }
            reflect_history(&ui, &edits.borrow());
        }
    });
    history
}
fn update_recovery(ui: &Studio, recovery: &Result<recovery::Recovery, String>) {
    let status = match recovery {
        Ok(session) => match session.write(ui.get_source().as_str(), ui.get_category()) {
            Ok(()) => "Recovery copy is current".to_owned(),
            Err(error) => format!("Recovery unavailable: {error}. Save a copy."),
        },
        Err(error) => format!("Recovery unavailable: {error}. Save a copy."),
    };
    ui.set_recovery_status(status.into());
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arg = std::env::args().nth(1);
    if arg.as_deref() == Some("--benchmark-formatted") {
        println!(
            "{}",
            serde_json::to_string_pretty(&editor_session::benchmark())?
        );
        return Ok(());
    }
    if arg.as_deref() == Some("--edit-layout") {
        let path = std::env::args()
            .nth(2)
            .ok_or("--edit-layout requires SOURCE COMMANDS.json NEW_OUTPUT_DIRECTORY")?;
        let commands = std::env::args().nth(3).ok_or("Missing command JSON")?;
        let destination = std::env::args()
            .nth(4)
            .ok_or("Missing new output directory")?;
        let source = source_draft::read(Path::new(&path))?;
        let commands: Vec<editor_session::Command> =
            serde_json::from_slice(&std::fs::read(commands)?)?;
        let mut editor =
            editor_session::Editor::new(&source, 600.).map_err(|e| format!("Editor: {e:?}"))?;
        let before = editor.paint(false)?;
        let mut changes = Vec::new();
        for (index, command) in commands.into_iter().enumerate() {
            let result = editor
                .apply(editor.generation(), command)
                .map_err(|e| format!("Command {index}: {e:?}"))?;
            let selection = editor.selection();
            changes.push(serde_json::json!({"command":index,"generation":result.generation,"sourceChanged":result.source_changed,"sourceBytes":editor.source().len(),"anchor":selection.anchor,"focus":selection.focus,"upstream":editor.focus_position().upstream}));
        }
        let after = editor.paint(false)?;
        let dark = editor.paint(true)?;
        let (undo, redo) = editor.history_available();
        let report = serde_json::json!({"experimental":true,"editableWidgetInstalled":false,"commands":changes,"canUndo":undo,"canRedo":redo,"finalPainting":after.metrics});
        std::fs::create_dir(&destination)?;
        let destination = Path::new(&destination);
        save_copy(&destination.join("Edited Copy.fountain"), editor.source())?;
        std::fs::write(destination.join("before.png"), before.png)?;
        std::fs::write(destination.join("after.png"), after.png)?;
        std::fs::write(destination.join("after-dark.png"), dark.png)?;
        std::fs::write(destination.join("after.svg"), after.svg)?;
        std::fs::write(
            destination.join("verification.json"),
            serde_json::to_vec_pretty(&report)?,
        )?;
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    if arg.as_deref() == Some("--paint-layout") {
        let path = std::env::args()
            .nth(2)
            .ok_or("--paint-layout requires SOURCE OUTPUT_DIRECTORY [CONFIG.json]")?;
        let destination = std::env::args()
            .nth(3)
            .ok_or("Missing new output directory")?;
        let config = match std::env::args().nth(4) {
            Some(path) => serde_json::from_slice(&std::fs::read(path)?)?,
            None => editor_paint::Config::default(),
        };
        let source = source_draft::read(Path::new(&path))?;
        config.validate()?;
        let document = editor_layout::Context::default().shape(&source, config.width - 64.)?;
        let result = editor_paint::paint(&document, &config)?;
        std::fs::create_dir(&destination)?; // Never overwrite an existing artifact directory.
        let destination = Path::new(&destination);
        std::fs::write(destination.join("layout.svg"), result.svg)?;
        std::fs::write(destination.join("layout.png"), result.png)?;
        std::fs::write(
            destination.join("metrics.json"),
            serde_json::to_vec_pretty(&result.metrics)?,
        )?;
        println!("{}", serde_json::to_string_pretty(&result.metrics)?);
        return Ok(());
    }
    if arg.as_deref() == Some("--layout-audit") {
        let path = std::env::args()
            .nth(2)
            .ok_or("--layout-audit requires a UTF-8 source file")?;
        let source = source_draft::read(Path::new(&path))?;
        let width = std::env::args()
            .nth(3)
            .map(|s| s.parse::<f32>())
            .transpose()?
            .unwrap_or(600.);
        println!(
            "{}",
            serde_json::to_string_pretty(&editor_layout::audit(&source, width)?)?
        );
        return Ok(());
    }
    if arg.as_deref() == Some("--benchmark") {
        for scenes in [10, 100, 300] {
            let source = SAMPLE.repeat(scenes);
            let start = Instant::now();
            let (_, _, lines) = derived(&source, 0);
            println!(
                "{} bytes / {} preview lines: {:.1} ms engine+model (not UI frame time)",
                source.len(),
                lines.len(),
                start.elapsed().as_secs_f64() * 1000.
            );
        }
        return Ok(());
    }
    let initial_name = arg
        .as_deref()
        .map(|path| document_label::from_path(Path::new(path)))
        .unwrap_or_else(|| "Untitled draft".into());
    let choose_on_launch = arg.is_none();
    let mut initial_project = None;
    let source = match arg {
        Some(p) => {
            let path = Path::new(&p);
            if path.extension().and_then(|v| v.to_str()) == Some("fountage") {
                let project = fountage_core::project::Project::read(path)?;
                let source = project.source().to_owned();
                initial_project = Some(project);
                source
            } else {
                source_draft::read(path)?
            }
        }
        None => String::new(),
    };
    window_chrome::select_backend()?;
    let ui = Studio::new()?;
    window_chrome::install(&ui);
    #[cfg(target_os = "macos")]
    keyboard_modifiers::install(&ui);
    formatted_live::install(&ui);
    ui.set_document_name(initial_name.into());
    install_file_actions(&ui);
    install_outline_navigation(&ui);
    install_find(&ui);
    install_formatting(&ui);
    install_scene_numbering(&ui);
    install_title_page(&ui);
    let checkpoint = Rc::new(RefCell::new(session::Checkpoint::saved(&source)));
    checkpoint
        .borrow_mut()
        .adopt_saved(&source, initial_project.as_ref().map(|p| p.metadata()));
    ui.set_source(source.into());
    let initial_recovery = if let Some(project) = &initial_project {
        ui.set_academic_style_label(project.academic_style_label().unwrap_or("").into());
        ui.set_project_mode(true);
        ui.set_status("Project draft · Save a Copy preserves metadata and attachments".into());
        ui.set_category(recovery::category_code(project.format()));
        Ok(recovery::Recovery::for_project(
            &recovery::directory()?,
            project,
        )?)
    } else {
        recovery::directory().and_then(|dir| recovery::Recovery::create(&dir))
    };
    let project = Rc::new(RefCell::new(initial_project));
    let recovery = Rc::new(RefCell::new(initial_recovery));
    update_recovery(&ui, &recovery.borrow());
    if let Ok(session) = recovery.borrow().as_ref() {
        eprintln!("Scratch recovery: {}", session.path().display());
    }
    let weak = ui.as_weak();
    let worker = Rc::new(worker::Worker::new(move |request, latest| {
        let start = Instant::now();
        let (summary, outline, lines) = derived(&request.source, request.category);
        let milliseconds = start.elapsed().as_secs_f64() * 1000.;
        if !worker::is_current(&latest, request.revision) {
            return;
        }
        let _ = weak.upgrade_in_event_loop(move |ui| {
            // Recheck after delivery: typing can supersede an already queued result.
            if worker::is_current(&latest, request.revision) {
                ui.set_outline_source(request.source.into());
                ui.set_outline_category(request.category);
                apply(&ui, summary, outline, lines, milliseconds);
            }
        });
    })?);
    worker.submit(ui.get_source().to_string(), ui.get_category());
    let edits = worker.clone();
    let saved = checkpoint.clone();
    let recovery_edits = recovery.clone();
    let history = install_editor_history(&ui, move |ui| {
        let source = ui.get_source();
        let kind = if ui.get_project_mode() {
            "Project draft"
        } else {
            "Scratch draft"
        };
        reflect_checkpoint(ui, &saved.borrow());
        let state = if saved.borrow().is_clean(source.as_str()) {
            "No unsaved text changes"
        } else {
            "Unsaved edits · Save a Copy to keep changes"
        };
        ui.set_status(format!("{kind} · {state}").into());
        ui.set_analysis_status("Updating preview…".into());
        update_recovery(ui, &recovery_edits.borrow());
        edits.submit(source.to_string(), ui.get_category());
    });
    let weak = ui.as_weak();
    let recovery_format = recovery.clone();
    let format_worker = worker.clone();
    let format_project = project.clone();
    ui.on_format_changed(move |category| {
        if let Some(ui) = weak.upgrade() {
            if let Some(project) = format_project.borrow().as_ref() {
                ui.set_category(recovery::category_code(project.format()));
                return;
            }
            ui.set_analysis_status("Updating preview…".into());
            update_recovery(&ui, &recovery_format.borrow());
            format_worker.submit(ui.get_source().to_string(), category);
        }
    });
    shelf_ui::install(
        &ui,
        DraftSession {
            checkpoint: checkpoint.clone(),
            history: history.clone(),
            project: project.clone(),
            recovery: recovery.clone(),
        },
        recovery::directory,
    );
    let new_worker = worker.clone();
    install_new_draft(
        &ui,
        DraftSession {
            checkpoint: checkpoint.clone(),
            history: history.clone(),
            project: project.clone(),
            recovery: recovery.clone(),
        },
        recovery::directory,
        move |ui| {
            new_worker.submit(ui.get_source().to_string(), ui.get_category());
        },
    );
    let weak = ui.as_weak();
    let source_project = project.clone();
    let source_recovery = recovery.clone();
    let source_checkpoint = checkpoint.clone();
    let source_history = history.clone();
    let source_worker = worker.clone();
    ui.on_open_source(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if ui.get_confirm_close() || ui.get_file_dialog_open() {
            return;
        }
        if !source_checkpoint
            .borrow()
            .is_clean(ui.get_source().as_str())
        {
            ui.set_status("Save a copy of the current draft before opening a source file.".into());
            return;
        }
        let _dialog_guard = DialogGuard::new(&ui);
        let Some(path) = rfd::FileDialog::new()
            .set_title("Open a source draft")
            .add_filter("Writing source", &["fountain", "txt", "md"])
            .pick_file()
        else {
            return;
        };
        if !source_checkpoint
            .borrow()
            .is_clean(ui.get_source().as_str())
        {
            ui.set_status("Draft changed while choosing a source file; it was kept open.".into());
            return;
        }
        let result = recovery::directory()
            .and_then(|root| source_draft::open(&path, ui.get_category(), &root));
        match result {
            Ok((source, category, baseline)) => {
                source_checkpoint.borrow_mut().adopt_saved(&source, None);
                source_history.borrow_mut().reset(&source);
                *source_project.borrow_mut() = None;
                ui.invoke_shelf_reset();
                *source_recovery.borrow_mut() = Ok(baseline);
                ui.set_project_mode(false);
                ui.set_academic_style_label("".into());
                ui.set_document_name(document_label::from_path(&path).into());
                ui.set_category(category);
                ui.set_source(source.clone().into());
                reflect_history(&ui, &source_history.borrow());
                reflect_checkpoint(&ui, &source_checkpoint.borrow());
                ui.set_status(
                    "Source draft · Save a Copy keeps the original file unchanged".into(),
                );
                update_recovery(&ui, &source_recovery.borrow());
                source_worker.submit(source, category);
            }
            Err(error) => ui.set_status(format!("Source not opened: {error}").into()),
        }
    });
    let weak = ui.as_weak();
    let open_project = project.clone();
    let open_recovery = recovery.clone();
    let open_checkpoint = checkpoint.clone();
    let open_history = history.clone();
    let open_worker = worker.clone();
    ui.on_open_project(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if ui.get_confirm_close() || ui.get_file_dialog_open() {
            return;
        }
        if !open_checkpoint.borrow().is_clean(ui.get_source().as_str()) {
            ui.set_status("Save a copy of the current draft before opening a project.".into());
            return;
        }
        let _dialog_guard = DialogGuard::new(&ui);
        let dialog = rfd::FileDialog::new().set_title("Open a Fountage project");
        #[cfg(target_os = "macos")]
        let path = dialog
            .add_filter("Fountage project", &["fountage"])
            .pick_file();
        #[cfg(not(target_os = "macos"))]
        let path = dialog.pick_folder();
        let Some(path) = path else {
            return;
        };
        if !open_checkpoint.borrow().is_clean(ui.get_source().as_str()) {
            ui.set_status("Draft changed while choosing a project; it was kept open.".into());
            return;
        }
        let result = recovery::directory().and_then(|root| project_draft::open(&path, &root));
        match result {
            Ok((loaded, baseline)) => {
                let source = loaded.source().to_owned();
                let category = recovery::category_code(loaded.format());
                open_checkpoint
                    .borrow_mut()
                    .adopt_saved(&source, Some(loaded.metadata()));
                open_history.borrow_mut().reset(&source);
                ui.set_academic_style_label(loaded.academic_style_label().unwrap_or("").into());
                *open_project.borrow_mut() = Some(loaded);
                ui.invoke_shelf_reset();
                *open_recovery.borrow_mut() = Ok(baseline);
                ui.set_project_mode(true);
                ui.set_document_name(document_label::from_path(&path).into());
                ui.set_category(category);
                ui.set_source(source.clone().into());
                reflect_history(&ui, &open_history.borrow());
                reflect_checkpoint(&ui, &open_checkpoint.borrow());
                ui.set_status(
                    "Project draft · Save a Copy preserves metadata and attachments".into(),
                );
                update_recovery(&ui, &open_recovery.borrow());
                open_worker.submit(source, category);
            }
            Err(error) => ui.set_status(format!("Project not opened: {error}").into()),
        }
    });
    let weak = ui.as_weak();
    let saved = checkpoint.clone();
    let recovered_project = project.clone();
    let recovered_recovery = recovery.clone();
    ui.on_recover(move || {
        if let Some(ui) = weak.upgrade() {
            if ui.get_confirm_close() || ui.get_file_dialog_open() {
                return;
            }
            if !saved.borrow().is_clean(ui.get_source().as_str()) {
                ui.set_status(
                    "Save a copy of the current draft before opening a recovery draft.".into(),
                );
                return;
            }
            let _dialog_guard = DialogGuard::new(&ui);
            let mut dialog = rfd::FileDialog::new()
                .set_title("Recover a draft")
                .add_filter("Fountage Lab recovery", &["json"]);
            if let Ok(dir) = recovery::directory() {
                dialog = dialog.set_directory(dir);
            }
            let Some(path) = dialog.pick_file() else {
                return;
            };
            if !saved.borrow().is_clean(ui.get_source().as_str()) {
                ui.set_status("Draft changed while choosing recovery; it was kept open.".into());
                return;
            }
            let result = (|| {
                let snapshot = recovery::read(&path)?;
                let project = recovery::project_from_snapshot(&path, &snapshot)?;
                let baseline = if let Some(project) = &project {
                    recovery::Recovery::for_project(&recovery::directory()?, project)?
                } else {
                    recovery::Recovery::create(&recovery::directory()?)?
                };
                baseline.write(&snapshot.source, snapshot.category)?;
                Ok::<_, String>((snapshot, project, baseline))
            })();
            match result {
                Ok((snapshot, loaded_project, baseline)) => {
                    ui.set_academic_style_label(
                        loaded_project
                            .as_ref()
                            .and_then(|p| p.academic_style_label())
                            .unwrap_or("")
                            .into(),
                    );
                    ui.set_project_mode(loaded_project.is_some());
                    ui.set_document_name(
                        if loaded_project.is_some() {
                            "Recovered project draft"
                        } else {
                            "Recovered draft"
                        }
                        .into(),
                    );
                    saved
                        .borrow_mut()
                        .set_metadata(loaded_project.as_ref().map(|p| p.metadata()));
                    *recovered_project.borrow_mut() = loaded_project;
                    ui.invoke_shelf_reset();
                    *recovered_recovery.borrow_mut() = Ok(baseline);
                    ui.set_category(snapshot.category);
                    ui.set_source(snapshot.source.clone().into());
                    history.borrow_mut().reset(&snapshot.source);
                    reflect_history(&ui, &history.borrow());
                    // A recovered source is still a scratch copy requiring an explicit save.
                    saved.borrow_mut().require_save();
                    reflect_checkpoint(&ui, &saved.borrow());
                    ui.set_status(
                        if ui.get_project_mode() {
                            "Recovered project · Save a project copy to keep this draft"
                        } else {
                            "Recovered scratch copy · Save a copy to keep this draft"
                        }
                        .into(),
                    );
                    update_recovery(&ui, &recovered_recovery.borrow());
                    worker.submit(snapshot.source, snapshot.category);
                }
                Err(error) => ui.set_status(format!("Recovery not opened: {error}").into()),
            }
        }
    });
    let weak = ui.as_weak();
    let saved = checkpoint.clone();
    let save_project = project.clone();
    ui.on_save_copy(move || {
        if let Some(ui) = weak.upgrade() {
            if ui.get_confirm_close() || ui.get_file_dialog_open() {
                return;
            }
            ui.invoke_formatted_finish();
            let source = ui.get_source();
            if save_with_dialog(&ui, source.as_str(), &save_project.borrow()) {
                saved.borrow_mut().mark_saved(source.as_str());
                reflect_checkpoint(&ui, &saved.borrow());
            }
        }
    });
    install_close_requests(&ui, checkpoint.clone());
    let weak = ui.as_weak();
    ui.on_close_choice(move |choice| {
        if let Some(ui) = weak.upgrade() {
            if ui.get_file_dialog_open() {
                return;
            }
            ui.set_confirm_close(false);
            let choice = match choice {
                0 => session::CloseChoice::SaveCopy,
                1 => session::CloseChoice::Discard,
                _ => session::CloseChoice::KeepWriting,
            };
            let source = ui.get_source();
            let mut next_checkpoint = checkpoint.borrow().clone();
            let close = session::may_close(
                &mut next_checkpoint,
                source.as_str(),
                || choice,
                || save_with_dialog(&ui, source.as_str(), &project.borrow()),
            );
            if close {
                *checkpoint.borrow_mut() = next_checkpoint;
                reflect_checkpoint(&ui, &checkpoint.borrow());
                if ui.get_source() != source {
                    ui.set_status("The draft changed while saving; it was kept open.".into());
                    return;
                }
                if let Err(error) = ui.hide() {
                    ui.set_status(format!("Window could not close: {error}").into());
                }
            }
        }
    });
    ui.show()?;
    #[cfg(target_os = "macos")]
    let _quit_guard = macos_quit::install(&ui)?;
    #[cfg(target_os = "macos")]
    let _history_guard = macos_history::install(&ui)?;
    if choose_on_launch {
        ui.set_status("Start a new project or open an existing draft.".into());
        ui.invoke_new_draft();
        ui.set_new_show_open_actions(true);
    }
    install_appearance(&ui, appearance::path());
    ui.run()?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manuscript_heading_uses_parsed_title_without_treating_body_as_metadata() {
        let source = "Title: **Zoë & the Moon**\r\n    Part Two\r\nAuthor: Baz Studios\r\n\r\nINT. ROOM - DAY\r\n\r\nTitle: This is body text.\r\n";
        let (_, navigation, _) = derived(source, 0);
        assert_eq!(navigation.manuscript_title, "Zoë & the Moon Part Two");
        let body_only = "INT. ROOM - DAY\n\nTitle: This is body text.\n";
        assert!(derived(body_only, 0).1.manuscript_title.is_empty());
        assert!(
            derived("Title:\n\nINT. ROOM - DAY\n", 0)
                .1
                .manuscript_title
                .is_empty()
        );
        for category in 1..8 {
            assert!(derived(source, category).1.manuscript_title.is_empty());
        }
    }
    #[test]
    fn preview_reuses_engine_dialogue_and_hidden_source() {
        let source = "INT. ROOM - DAY\n\nMARA\nHello, Zoë 🌙.\n\n/*\nEXT. SECRET - NIGHT\n*/\n";
        let (_, outline, lines) = derived(source, 0);
        assert!(outline.outline.iter().any(|h| h.text.contains("INT. ROOM")));
        assert!(!outline.outline.iter().any(|h| h.text.contains("SECRET")));
        assert!(
            lines
                .iter()
                .any(|l| l.text.contains("Hello, Zoë 🌙") && l.indent == 55.)
        );
        assert!(!lines.iter().any(|l| l.text.contains("SECRET")));
    }
    #[test]
    fn copy_preserves_bytes_and_never_clobbers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("copy.fountain");
        let source = "Zoë 🌙\r\n世界\r\n";
        save_copy(&path, source).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), source.as_bytes());
        assert!(save_copy(&path, "replacement").is_err());
        let invalid = dir.path().join("invalid.fountain");
        assert!(save_copy(&invalid, "bad\0source").is_err());
        assert!(!invalid.exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    }
    #[test]
    fn all_categories_produce_models() {
        for category in 0..8 {
            let (s, _, _) = derived(SAMPLE, category);
            assert!(s.contains("words"));
        }
    }
}

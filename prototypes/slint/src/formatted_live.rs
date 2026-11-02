//! Opt-in live adapter. The production source TextInput remains the default.
use crate::{
    Studio,
    editor_paint::{self, Config},
    editor_session::{Command, Editor, Motion},
};
use slint::{ComponentHandle, Image, SharedPixelBuffer};
use std::{cell::RefCell, rc::Rc};
#[derive(PartialEq)]
struct FrameKey {
    generation: u64,
    selection: crate::editor_session::Selection,
    position: crate::editor_layout::Position,
    width: f32,
    height: f32,
    scroll: f32,
    dark: bool,
}
struct Live {
    editor: Editor,
    width: f32,
    height: f32,
    scroll: f32,
    last_frame: Option<FrameKey>,
}
type State = Rc<RefCell<Option<Live>>>;
fn allowed(ui: &Studio) -> bool {
    ui.get_formatted_active()
        && ui.get_formatted_ready()
        && ui.get_category() == 0
        && !ui.get_confirm_close()
        && !ui.get_file_dialog_open()
}
fn update_ime(ui: &Studio, state: &State) {
    use i_slint_core::fountage_ime::Properties;
    let props = if allowed(ui) && ui.get_formatted_has_focus() {
        let state = state.borrow();
        state.as_ref().and_then(|live| {
            let editor = &live.editor;
            let rect = editor
                .display_document()
                .caret_at(editor.display_position())?
                .rect;
            let selection = editor.selection();
            let (preedit, offset) = editor.ime_preedit().unwrap_or(("", selection.focus));
            Some(Properties {
                text: editor.source().into(),
                cursor: if editor.composing() {
                    offset
                } else {
                    selection.focus
                },
                anchor: (!editor.composing() && selection.anchor != selection.focus)
                    .then_some(selection.anchor),
                preedit: preedit.into(),
                preedit_offset: offset,
                origin: slint::LogicalPosition::new(
                    ui.get_formatted_surface_x() + 32. + rect.x0 as f32,
                    ui.get_formatted_surface_y() + 32. + rect.y0 as f32 - live.scroll,
                ),
                size: slint::LogicalSize::new(1., rect.height().max(1.) as f32),
            })
        })
    } else {
        None
    };
    let active = props.is_some();
    match ui.window().set_fountage_ime(props) {
        Ok(()) => ui.set_formatted_ime_active(active),
        Err(error) => {
            ui.set_formatted_ime_active(false);
            ui.set_formatted_status(format!("Input method unavailable: {error}").into());
        }
    }
}
fn refresh(ui: &Studio, state: &State, width: f32, height: f32, follow: bool) {
    if !ui.get_formatted_active() || ui.get_category() != 0 {
        return;
    }
    let result = (|| -> Result<_, String> {
        let config = Config {
            width,
            height,
            scale: 2.,
            ..Config::default()
        };
        config.validate().map_err(str::to_owned)?;
        let source = ui.get_source();
        let mut state = state.borrow_mut();
        if state.is_none() {
            *state = Some(Live {
                editor: Editor::external_history(&source, width - 64.)
                    .map_err(|e| format!("{e:?}"))?,
                width,
                height,
                scroll: 0.,
                last_frame: None,
            });
        }
        let live = state.as_mut().unwrap();
        if live.editor.source() != source.as_str() {
            live.last_frame = None;
            live.editor
                .adopt(&source, width - 64.)
                .map_err(|e| format!("{e:?}"))?;
            live.scroll = 0.; // Programmatic document adoption starts at the beginning;
            // Undo/Find explicitly supply their source selection afterward.
        }
        if live.width != width {
            live.editor
                .apply(
                    live.editor.generation(),
                    Command::Reflow { width: width - 64. },
                )
                .map_err(|e| format!("{e:?}"))?;
        }
        live.width = width;
        live.height = height;
        if follow {
            let rect = live
                .editor
                .display_document()
                .caret_at(live.editor.display_position())
                .ok_or("Missing caret")?
                .rect;
            if rect.y0 as f32 + 32. < live.scroll + 16. {
                live.scroll = (rect.y0 as f32 + 16.).max(0.);
            }
            if rect.y1 as f32 + 32. > live.scroll + height - 16. {
                live.scroll = rect.y1 as f32 + 48. - height;
            }
        }
        live.scroll = live.scroll.clamp(
            0.,
            (live.editor.display_document().height + 64. - height).max(0.),
        );
        let selection = live.editor.display_selection();
        let committed_selection = live.editor.selection();
        let key = FrameKey {
            generation: live.editor.generation(),
            selection,
            position: live.editor.display_position(),
            width,
            height,
            scroll: live.scroll,
            dark: ui.get_dark() && !ui.get_white_paper(),
        };
        let image = if live.last_frame.as_ref() == Some(&key) {
            None
        } else {
            let painting = editor_paint::paint_live(
                live.editor.display_document(),
                &Config {
                    dark: key.dark,
                    scroll_y: live.scroll,
                    caret: Some(selection.focus),
                    caret_upstream: key.position.upstream,
                    selection: Some([selection.anchor, selection.focus]),
                    marked: live.editor.marked_range(),
                    ..config
                },
            )?;
            let pixels = SharedPixelBuffer::clone_from_slice(
                &painting.pixels,
                painting.width,
                painting.height,
            );
            live.last_frame = Some(key);
            Some(Image::from_rgba8_premultiplied(pixels))
        };
        let status = String::new(); // The UI derives clean/dirty text directly from the checkpoint.
        Ok((
            image,
            committed_selection,
            status,
            live.editor.composing(),
            live.editor.document().shaped_paragraphs,
            live.editor.document().reused_paragraphs,
        ))
    })();
    // Drop the controller borrow before property changes can trigger more callbacks.
    match result {
        Ok((image, selection, status, composing, shaped, reused)) => {
            ui.set_formatted_composing(composing);
            ui.set_formatted_shaped_paragraphs(shaped as i32);
            ui.set_formatted_reused_paragraphs(reused as i32);
            ui.set_formatted_ready(true);
            if let Some(image) = image {
                ui.set_formatted_frame(image);
            }
            ui.set_formatted_anchor_upstream(
                state
                    .borrow()
                    .as_ref()
                    .is_some_and(|live| live.editor.history_selection().anchor_upstream),
            );
            ui.set_formatted_focus_upstream(
                state
                    .borrow()
                    .as_ref()
                    .is_some_and(|live| live.editor.focus_position().upstream),
            );
            ui.set_formatted_has_selection(!composing && selection.anchor != selection.focus);
            ui.invoke_mirror_formatted_selection(selection.anchor as i32, selection.focus as i32);
            ui.set_formatted_status(status.into());
        }
        Err(error) => {
            ui.set_formatted_ready(false);
            ui.set_formatted_has_selection(false);
            ui.set_formatted_frame(Image::default());
            if let Some(live) = state.borrow_mut().as_mut() {
                live.last_frame = None;
            }
            ui.set_formatted_status(format!("Formatted editor unavailable: {error}. Enlarge the writing area or use View → Standard Source Editor.").into());
        }
    }
    update_ime(ui, state);
}
fn redraw(ui: &Studio, state: &State, follow: bool) {
    let dimensions = state.borrow().as_ref().map(|l| (l.width, l.height));
    if let Some((w, h)) = dimensions {
        refresh(ui, state, w, h, follow);
    }
}
fn execute(ui: &Studio, state: &State, command: Command) {
    if !allowed(ui) {
        return;
    }
    // First synchronize programmatic Undo/Open/Find changes that have not yet
    // delivered a Slint property-change callback.
    if state
        .borrow()
        .as_ref()
        .is_some_and(|l| l.editor.source() != ui.get_source().as_str())
    {
        redraw(ui, state, false);
    }
    if let Some(live) = state.borrow().as_ref() {
        let selection = live.editor.history_selection();
        ui.invoke_history_visual_selection(
            live.editor.source().into(),
            selection.anchor as i32,
            selection.focus as i32,
            selection.anchor_upstream,
            selection.focus_upstream,
        );
    }
    let result = {
        let mut state = state.borrow_mut();
        let Some(live) = state.as_mut() else {
            return;
        };
        live.editor
            .apply(live.editor.generation(), command)
            .map(|change| {
                change
                    .source_changed
                    .then(|| live.editor.source().to_owned())
            })
    };
    match result {
        Ok(Some(source)) => {
            ui.set_source(source.clone().into());
            let selection = state.borrow().as_ref().unwrap().editor.history_selection();
            ui.invoke_editor_visual_edited(
                source.into(),
                selection.anchor as i32,
                selection.focus as i32,
                selection.anchor_upstream,
                selection.focus_upstream,
            );
        }
        Ok(None) => {}
        Err(error) => {
            ui.set_formatted_status(format!("Formatted input rejected: {error:?}").into());
            return;
        }
    }
    redraw(ui, state, true);
}
pub fn install(ui: &Studio) {
    let state: State = Rc::new(RefCell::new(None));
    let weak = ui.as_weak();
    let model = state.clone();
    ui.on_formatted_focus_changed(move || {
        if let Some(ui) = weak.upgrade() {
            update_ime(&ui, &model);
        }
    });
    let weak = ui.as_weak();
    let model = state.clone();
    ui.window().on_fountage_composition(move |event| {
        let Some(ui) = weak.upgrade() else {
            return false;
        };
        if !allowed(&ui) || !ui.get_formatted_has_focus() {
            return false;
        }
        // Winit supplies basic preedit/commit events. Advanced replacement and
        // combined partial commits require a separately verified transaction map.
        if event.replacement.is_some()
            || event.cursor.is_some()
            || event.anchor.is_some()
            || (!event.text.is_empty() && !event.preedit.is_empty())
        {
            ui.set_formatted_status(
                "This input-method replacement is not supported yet. Use the standard editor."
                    .into(),
            );
            return true;
        }
        let typed_commit = !ui.get_formatted_composing()
            && event.text.chars().count() == 1
            && event.preedit.is_empty();
        if typed_commit {
            ui.invoke_history_input(event.text.clone());
        } else {
            ui.invoke_history_boundary();
        }
        if event.commit_event || !event.text.is_empty() {
            if event.text.is_empty() {
                execute(&ui, &model, Command::CancelComposition);
            } else {
                execute(
                    &ui,
                    &model,
                    Command::CommitComposition {
                        text: event.text.to_string(),
                    },
                );
            }
        } else {
            let cursor = match event.selection {
                Some(range) if range.start >= 0 && range.end >= 0 => {
                    Some([range.start as usize, range.end as usize])
                }
                Some(_) => {
                    ui.set_formatted_status("Invalid input-method selection rejected".into());
                    return true;
                }
                None => None,
            };
            execute(
                &ui,
                &model,
                Command::Preedit {
                    text: event.preedit.to_string(),
                    cursor,
                    replacement: None,
                },
            );
        }
        if !typed_commit {
            ui.invoke_history_boundary();
        }
        true
    });
    let weak = ui.as_weak();
    let model = state.clone();
    ui.on_formatted_finish(move || {
        if let Some(ui) = weak.upgrade() {
            let text = model
                .borrow()
                .as_ref()
                .and_then(|l| l.editor.ime_preedit().map(|(t, _)| t.to_owned()));
            // Match stock TextInput's preservation of unfinished text on focus loss.
            // Native commit arriving before this callback has already cleared preedit.
            if let Some(text) = text {
                if let Some(live) = model.borrow().as_ref() {
                    let selection = live.editor.history_selection();
                    ui.invoke_history_visual_selection(
                        live.editor.source().into(),
                        selection.anchor as i32,
                        selection.focus as i32,
                        selection.anchor_upstream,
                        selection.focus_upstream,
                    );
                }
                let result = {
                    let mut model = model.borrow_mut();
                    let live = model.as_mut().unwrap();
                    live.editor
                        .apply(
                            live.editor.generation(),
                            Command::CommitComposition { text },
                        )
                        .map(|c| c.source_changed.then(|| live.editor.source().to_owned()))
                };
                if let Ok(Some(source)) = result {
                    ui.invoke_history_boundary();
                    ui.set_source(source.clone().into());
                    let selection = model.borrow().as_ref().unwrap().editor.history_selection();
                    ui.invoke_editor_visual_edited(
                        source.into(),
                        selection.anchor as i32,
                        selection.focus as i32,
                        selection.anchor_upstream,
                        selection.focus_upstream,
                    );
                    ui.invoke_history_boundary();
                }
                ui.set_formatted_composing(false);
                let selection = model.borrow().as_ref().map(|l| l.editor.selection());
                if let Some(selection) = selection {
                    ui.invoke_mirror_formatted_selection(
                        selection.anchor as i32,
                        selection.focus as i32,
                    );
                }
                redraw(&ui, &model, false);
            }
            update_ime(&ui, &model);
        }
    });
    let weak = ui.as_weak();
    let model = state.clone();
    ui.on_formatted_refresh(move |w, h| {
        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &model, w, h, false);
        }
    });
    let weak = ui.as_weak();
    let model = state.clone();
    ui.on_formatted_restore_selection(move |anchor, focus, au, fu| {
        if let Some(ui) = weak.upgrade()
            && anchor >= 0
            && focus >= 0
        {
            execute(
                &ui,
                &model,
                Command::SelectVisual {
                    anchor: anchor as usize,
                    focus: focus as usize,
                    anchor_upstream: au,
                    focus_upstream: fu,
                },
            );
        }
    });
    let weak = ui.as_weak();
    let model = state.clone();
    ui.on_formatted_select(move |a, b| {
        if let Some(ui) = weak.upgrade()
            && a >= 0
            && b >= 0
        {
            execute(
                &ui,
                &model,
                Command::Select {
                    anchor: a as usize,
                    focus: b as usize,
                },
            );
        }
    });
    let weak = ui.as_weak();
    let model = state.clone();
    ui.on_formatted_all(move || {
        if let Some(ui) = weak.upgrade() {
            execute(&ui, &model, Command::SelectAll);
        }
    });
    let weak = ui.as_weak();
    let model = state.clone();
    ui.on_formatted_selection(move || {
        let Some(ui) = weak.upgrade() else {
            return "".into();
        };
        if !allowed(&ui) || ui.get_formatted_composing() {
            return "".into();
        }
        if model
            .borrow()
            .as_ref()
            .is_some_and(|l| l.editor.source() != ui.get_source().as_str())
        {
            redraw(&ui, &model, false);
        }
        let model = model.borrow();
        let Some(live) = model.as_ref() else {
            return "".into();
        };
        let selection = live.editor.selection();
        live.editor.source()
            [selection.anchor.min(selection.focus)..selection.anchor.max(selection.focus)]
            .into()
    });
    let weak = ui.as_weak();
    let model = state.clone();
    ui.on_formatted_insert(move |value| {
        if let Some(ui) = weak.upgrade() {
            ui.invoke_history_boundary();
            execute(
                &ui,
                &model,
                Command::Insert {
                    text: value.to_string(),
                },
            );
            ui.invoke_history_boundary();
        }
    });
    let weak = ui.as_weak();
    let model = state.clone();
    ui.on_formatted_preedit(move |text, a, b| {
        if let Some(ui) = weak.upgrade() {
            ui.invoke_history_boundary();
            execute(
                &ui,
                &model,
                Command::Preedit {
                    text: text.to_string(),
                    cursor: (a >= 0 && b >= 0).then_some([a as usize, b as usize]),
                    replacement: None,
                },
            );
        }
    });
    let weak = ui.as_weak();
    let model = state.clone();
    ui.on_formatted_commit(move |text| {
        if let Some(ui) = weak.upgrade() {
            ui.invoke_history_boundary();
            execute(
                &ui,
                &model,
                Command::CommitComposition {
                    text: text.to_string(),
                },
            );
            ui.invoke_history_boundary();
        }
    });
    let weak = ui.as_weak();
    let model = state.clone();
    ui.on_formatted_cancel(move || {
        if let Some(ui) = weak.upgrade() {
            let changed = {
                let mut model = model.borrow_mut();
                model.as_mut().is_some_and(|l| {
                    let composing = l.editor.composing();
                    let _ = l
                        .editor
                        .apply(l.editor.generation(), Command::CancelComposition);
                    composing
                })
            };
            ui.set_formatted_composing(false);
            if changed {
                redraw(&ui, &model, false);
            }
        }
    });
    let weak = ui.as_weak();
    let model = state.clone();
    ui.on_formatted_point(move |x, y, extend| {
        if let Some(ui) = weak.upgrade() {
            ui.invoke_history_boundary();
            let scroll = model.borrow().as_ref().map(|l| l.scroll).unwrap_or(0.);
            execute(
                &ui,
                &model,
                Command::Point {
                    x: x - 32.,
                    y: y - 32. + scroll,
                    extend,
                },
            );
        }
    });
    let weak = ui.as_weak();
    let model = state.clone();
    ui.on_formatted_scroll(move |delta| {
        if let Some(ui) = weak.upgrade()
            && allowed(&ui)
            && delta.is_finite()
        {
            if let Some(live) = model.borrow_mut().as_mut() {
                live.scroll -= delta;
            }
            redraw(&ui, &model, false);
        }
    });
    let weak = ui.as_weak();
    let native_state = state.clone();
    let native_weak = weak.clone();
    let delete_state = state.clone();
    let delete_weak = weak.clone();
    ui.on_formatted_delete_word(move |forward| {
        let Some(ui) = delete_weak.upgrade() else {
            return;
        };
        if !allowed(&ui) || !ui.get_formatted_has_focus() {
            return;
        }
        ui.invoke_history_boundary();
        execute(&ui, &delete_state, Command::DeleteWord { forward });
    });
    ui.on_formatted_navigation(move |direction, extend| {
        let Some(ui) = native_weak.upgrade() else {
            return;
        };
        if !allowed(&ui) || !ui.get_formatted_has_focus() {
            return;
        }
        let motion = match direction.as_str() {
            "word-left" => Some(Motion::WordLeft),
            "word-right" => Some(Motion::WordRight),
            "line-start" => Some(Motion::LineStart),
            "line-end" => Some(Motion::LineEnd),
            "document-start" => Some(Motion::DocumentStart),
            "document-end" => Some(Motion::DocumentEnd),
            _ => None,
        };
        let command = if let Some(direction) = motion {
            Command::Move { direction, extend }
        } else if direction == "page-up" || direction == "page-down" {
            let page = native_state
                .borrow()
                .as_ref()
                .map(|live| (live.height - 88.).max(24.))
                .unwrap_or(24.);
            Command::Page {
                pixels: if direction == "page-down" {
                    page
                } else {
                    -page
                },
                extend,
            }
        } else {
            return;
        };
        ui.invoke_history_boundary();
        execute(&ui, &native_state, command);
    });
    ui.on_formatted_key(move |event| {
        use slint::platform::Key;
        let Some(ui) = weak.upgrade() else {
            return false;
        };
        if !allowed(&ui) {
            return true;
        }
        let is = |key: Key| event.text == slint::SharedString::from(key);
        let command = event.modifiers.control || event.modifiers.meta;
        let word = if cfg!(target_os = "macos") {
            event.modifiers.alt && !command
        } else {
            command && !event.modifiers.alt
        };
        if word && !event.modifiers.shift && (is(Key::Backspace) || is(Key::Delete)) {
            ui.invoke_history_boundary();
            execute(
                &ui,
                &state,
                Command::DeleteWord {
                    forward: is(Key::Delete),
                },
            );
            return true;
        }
        let boundary = if word && is(Key::LeftArrow) {
            Some(Motion::WordLeft)
        } else if word && is(Key::RightArrow) {
            Some(Motion::WordRight)
        } else if command && (is(Key::Home) || (cfg!(target_os = "macos") && is(Key::UpArrow))) {
            Some(Motion::DocumentStart)
        } else if command && (is(Key::End) || (cfg!(target_os = "macos") && is(Key::DownArrow))) {
            Some(Motion::DocumentEnd)
        } else if command && cfg!(target_os = "macos") && is(Key::LeftArrow) {
            Some(Motion::LineStart)
        } else if command && cfg!(target_os = "macos") && is(Key::RightArrow) {
            Some(Motion::LineEnd)
        } else {
            None
        };
        if let Some(direction) = boundary {
            ui.invoke_history_boundary();
            execute(
                &ui,
                &state,
                Command::Move {
                    direction,
                    extend: event.modifiers.shift,
                },
            );
            return true;
        }
        if !command && !event.modifiers.alt && (is(Key::PageUp) || is(Key::PageDown)) {
            let page = state
                .borrow()
                .as_ref()
                .map(|live| (live.height - 88.).max(24.))
                .unwrap_or(24.);
            ui.invoke_history_boundary();
            execute(
                &ui,
                &state,
                Command::Page {
                    pixels: if is(Key::PageDown) { page } else { -page },
                    extend: event.modifiers.shift,
                },
            );
            return true;
        }
        if event.modifiers.control || event.modifiers.meta {
            match event.text.to_ascii_lowercase().as_str() {
                "/" if !event.modifiers.shift => ui.invoke_note_source(),
                "b" if !event.modifiers.shift => ui.invoke_format_source(0),
                "i" if !event.modifiers.shift => ui.invoke_format_source(1),
                "u" if !event.modifiers.shift => ui.invoke_format_source(2),
                "z" => ui.invoke_history_action(event.modifiers.shift),
                "f" if event.modifiers.shift => ui.invoke_toggle_focus(),
                "f" => ui.invoke_show_find(),
                "a" => execute(&ui, &state, Command::SelectAll),
                "x" => ui.invoke_clipboard_action(0),
                "c" => ui.invoke_clipboard_action(1),
                "v" => ui.invoke_clipboard_action(2),
                _ => return false,
            }
            return true;
        }
        if event.modifiers.alt {
            return false;
        }
        let motion = if is(Key::LeftArrow) {
            Some(Motion::Left)
        } else if is(Key::RightArrow) {
            Some(Motion::Right)
        } else if is(Key::UpArrow) {
            Some(Motion::Up)
        } else if is(Key::DownArrow) {
            Some(Motion::Down)
        } else if is(Key::Home) {
            Some(Motion::LineStart)
        } else if is(Key::End) {
            Some(Motion::LineEnd)
        } else {
            None
        };
        let command = if let Some(direction) = motion {
            ui.invoke_history_boundary();
            Command::Move {
                direction,
                extend: event.modifiers.shift,
            }
        } else if is(Key::Backspace) {
            ui.invoke_history_boundary();
            Command::DeleteBackward
        } else if is(Key::Delete) {
            ui.invoke_history_boundary();
            Command::DeleteForward
        } else if is(Key::Return) {
            ui.invoke_history_boundary();
            Command::Enter
        } else if !event.text.is_empty()
            && event.text.chars().all(|c| {
                (!c.is_control() || matches!(c, '\n' | '\r' | '\t'))
                    && !('\u{f700}'..='\u{f8ff}').contains(&c)
            })
        {
            ui.invoke_history_input(event.text.clone());
            Command::Insert {
                text: event.text.to_string(),
            }
        } else {
            return false;
        };
        execute(&ui, &state, command);
        true
    });
}

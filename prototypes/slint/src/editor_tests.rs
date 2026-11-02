//! Exercise the actual generated manuscript editor with an isolated Slint test backend.
//! These tests do not use the desktop, clipboard, native input methods, or real font layout.
use super::*;
use i_slint_core::input::{InternalKeyEvent, KeyEvent, KeyEventType};
use slint::Model;
use slint::platform::{Key, WindowEvent};
use std::cell::Cell;

fn composition(ui: &Studio, preedit: &str, committed: &str, commit: bool) {
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
}

fn text(ui: &Studio, value: &str) {
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: value.into() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text: value.into() });
}
fn shortcut(ui: &Studio, value: &str, shift: bool) {
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Control.into(),
    });
    if shift {
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Shift.into(),
        });
    }
    text(ui, value);
    if shift {
        ui.window().dispatch_event(WindowEvent::KeyReleased {
            text: Key::Shift.into(),
        });
    }
    ui.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Control.into(),
    });
}

#[test]
fn unicode_selection_replacement_undo_redo_and_close_lock_use_real_editor() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    let edits = Rc::new(Cell::new(0));
    let observed = edits.clone();

    let source = "Title: Café\r\n\r\nINT. ROOM - DAY\r\n\r\nZoë 🌙 e\u{301} 世界\r\n";
    ui.set_source(source.into());
    let checkpoint = Rc::new(RefCell::new(session::Checkpoint::saved(source)));
    let observed_checkpoint = checkpoint.clone();
    let recovery_root = tempfile::tempdir().unwrap();
    let edit_recovery = Rc::new(recovery::Recovery::create(recovery_root.path()).unwrap());
    let observed_recovery = edit_recovery.clone();
    let history = install_editor_history(&ui, move |ui| {
        observed.set(observed.get() + 1);
        reflect_checkpoint(ui, &observed_checkpoint.borrow());
        observed_recovery
            .write(ui.get_source().as_str(), ui.get_category())
            .unwrap();
    });
    ui.show().unwrap();
    assert_eq!(
        ui.get_source().as_str(),
        source,
        "programmatic load retains mixed-width UTF-8 and CRLF"
    );
    let start = source.find("🌙").unwrap();
    let end = start + "🌙 e\u{301} 世界".len();
    ui.invoke_select_source(start as i32, end as i32);
    text(&ui, "✨ مرحبًا");
    let expected = format!("{}✨ مرحبًا{}", &source[..start], &source[end..]);
    assert_eq!(ui.get_source().as_str(), expected);
    assert!(ui.get_dirty());
    assert_eq!(
        edits.get(),
        1,
        "actual input emits the production edited callback"
    );
    shortcut(&ui, "z", false);
    assert_eq!(
        ui.get_source().as_str(),
        source,
        "undo restores the exact original selection contents and CRLF"
    );
    assert!(!ui.get_dirty(), "undo to the saved source clears Edited");
    shortcut(&ui, "z", true);
    assert_eq!(
        ui.get_source().as_str(),
        expected,
        "redo reapplies the replacement"
    );
    assert_eq!(edits.get(), 3);
    assert!(ui.get_dirty(), "redo restores Edited");

    // Close overlay disables the actual input; simulated typing cannot mutate a draft behind it.
    ui.set_confirm_close(true);
    ui.invoke_select_source(0, ui.get_source().len() as i32);
    text(&ui, "must not replace the draft");
    assert_eq!(ui.get_source().as_str(), expected);
    ui.set_confirm_close(false);
    ui.invoke_select_source(0, ui.get_source().len() as i32);
    shortcut(&ui, "a", false);
    text(&ui, "新しい草稿 🌙\n");
    assert_eq!(ui.get_source().as_str(), "新しい草稿 🌙\n");
    // The test backend owns this clipboard; no desktop pasteboard is accessed.
    let clip_source = "Café 🌙 世界\r\n";
    ui.set_source(clip_source.into());
    history.borrow_mut().reset(clip_source);
    let start = clip_source.find("🌙").unwrap();
    let end = start + "🌙 世界".len();
    ui.invoke_select_source(start as i32, end as i32);
    shortcut(&ui, "c", false);
    ui.invoke_select_source(clip_source.len() as i32, clip_source.len() as i32);
    shortcut(&ui, "v", false);
    let pasted = format!("{clip_source}🌙 世界");
    assert_eq!(
        ui.get_source().as_str(),
        pasted,
        "clipboard preserves Unicode and CRLF"
    );
    shortcut(&ui, "z", false);
    assert_eq!(
        ui.get_source().as_str(),
        clip_source,
        "paste is one undo transaction"
    );
    ui.invoke_select_source(start as i32, end as i32);
    shortcut(&ui, "x", false);
    assert_eq!(ui.get_source().as_str(), "Café \r\n");
    shortcut(&ui, "z", false);
    assert_eq!(
        ui.get_source().as_str(),
        clip_source,
        "cut is one undo transaction"
    );
    ui.set_source("".into());
    history.borrow_mut().reset("");
    ui.invoke_select_source(0, 0);
    let before_empty_undo = edits.get();
    shortcut(&ui, "z", false);
    assert_eq!(
        ui.get_source().as_str(),
        "",
        "empty-history shortcut cannot insert text or replay a previous document"
    );
    assert_eq!(edits.get(), before_empty_undo);
    for value in ["C", "a", "f", "é"] {
        text(&ui, value);
    }
    assert_eq!(ui.get_source().as_str(), "Café");
    shortcut(&ui, "z", false);
    assert_eq!(
        ui.get_source().as_str(),
        "",
        "ordinary adjacent typing is one undo run"
    );
    shortcut(&ui, "z", true);
    assert_eq!(ui.get_source().as_str(), "Café");
    // A source selection/navigation boundary separates subsequent typing.
    ui.invoke_select_source(5, 5);
    text(&ui, "!");
    shortcut(&ui, "z", false);
    assert_eq!(ui.get_source().as_str(), "Café");
    // Drive the same internal events used by the pinned native backend. This
    // verifies document/history semantics, not a real OS candidate panel.
    let composition_source = "Café 🌙 世界\r\n";
    ui.set_source(composition_source.into());
    checkpoint.borrow_mut().mark_saved(composition_source);
    history.borrow_mut().reset(composition_source);
    reflect_checkpoint(&ui, &checkpoint.borrow());
    let begin = composition_source.find("世界").unwrap();
    let finish = begin + "世界".len();
    ui.invoke_select_source(begin as i32, finish as i32);
    let before_composition = edits.get();
    for preedit in ["に", "にほん", "日本"] {
        composition(&ui, preedit, "", false);
        assert_eq!(ui.get_source().as_str(), composition_source);
        assert_eq!(edits.get(), before_composition);
        assert!(!ui.get_dirty(), "preedit is not a saved manuscript change");
    }
    composition(&ui, "", "", true);
    assert_eq!(
        ui.get_source().as_str(),
        composition_source,
        "cancelling preserves the selection"
    );
    assert_eq!(edits.get(), before_composition);
    composition(&ui, "にほん", "", false);
    composition(&ui, "", "日本", true);
    let committed = "Café 🌙 日本\r\n";
    assert_eq!(ui.get_source().as_str(), committed);
    assert_eq!(edits.get(), before_composition + 1);
    assert!(ui.get_dirty());
    shortcut(&ui, "z", false);
    assert_eq!(
        ui.get_source().as_str(),
        composition_source,
        "one undo restores the complete replaced selection"
    );
    assert!(!ui.get_dirty());
    shortcut(&ui, "z", true);
    assert_eq!(ui.get_source().as_str(), committed);
    // A one-scalar IME commit must not join adjacent ordinary typing.
    ui.set_source("".into());
    history.borrow_mut().reset("");
    ui.invoke_select_source(0, 0);
    text(&ui, "a");
    composition(&ui, "e", "", false);
    composition(&ui, "", "é", true);
    assert_eq!(ui.get_source().as_str(), "aé");
    shortcut(&ui, "z", false);
    assert_eq!(ui.get_source().as_str(), "a");
    shortcut(&ui, "z", false);
    assert_eq!(ui.get_source().as_str(), "");

    composition(&ui, "e", "", false);
    composition(&ui, "", "e\u{301}", true);
    assert_eq!(
        ui.get_source().as_str(),
        "é",
        "pinned Slint normalizes incoming composition text to NFC; exact committed-byte preservation remains an open gate"
    );
    shortcut(&ui, "z", false);
    assert_eq!(ui.get_source().as_str(), "");
    shortcut(&ui, "z", true);
    assert_eq!(ui.get_source().as_str(), "é");
    for picker in [false, true] {
        ui.set_confirm_close(!picker);
        ui.set_file_dialog_open(picker);
        let before = edits.get();
        composition(&ui, "に", "", false);
        composition(&ui, "", "日", true);
        assert_eq!(
            ui.get_source().as_str(),
            "é",
            "composition cannot edit behind a close review or file picker"
        );
        assert_eq!(edits.get(), before);
        ui.set_confirm_close(false);
        ui.set_file_dialog_open(false);
    }

    // A reconversion commit may name existing text relative to the caret,
    // without first changing the editor's selection. Offsets are UTF-8 bytes.
    ui.set_source(composition_source.into());
    checkpoint.borrow_mut().mark_saved(composition_source);
    history.borrow_mut().reset(composition_source);
    ui.invoke_select_source(finish as i32, finish as i32);
    let before = edits.get();
    let mut key_event = KeyEvent::default();
    key_event.text = "日本語".into();
    ui.window()
        .dispatch_event(WindowEvent::internal(InternalKeyEvent {
            key_event,
            event_type: KeyEventType::CommitComposition,
            replacement_range: Some(-("世界".len() as i32)..0),
            ..Default::default()
        }));
    assert_eq!(ui.get_source().as_str(), "Café 🌙 日本語\r\n");
    assert_eq!(edits.get(), before + 1);
    shortcut(&ui, "z", false);
    assert_eq!(ui.get_source().as_str(), composition_source);
    assert!(!ui.get_dirty());
    shortcut(&ui, "z", true);
    assert_eq!(ui.get_source().as_str(), "Café 🌙 日本語\r\n");

    let navigation_source =
        "🌙 An opening image.\r\n\r\nINT. CAFÉ - DAY\r\n\r\nMARA\r\nHello.\r\n\r\nINT. CAFÉ - DAY";
    ui.set_source(navigation_source.into());
    ui.set_category(0);
    history.borrow_mut().reset(navigation_source);
    let (summary, headings, lines) = derived(navigation_source, 0);
    assert_eq!(headings.outline.len(), 2);
    ui.set_outline_source(navigation_source.into());
    ui.set_outline_category(0);
    apply(&ui, summary, headings, lines, 0.);
    install_outline_navigation(&ui);
    ui.invoke_navigate(1);
    assert_eq!(
        ui.get_source().as_str(),
        navigation_source,
        "navigation alone never edits source"
    );
    text(&ui, "EXT. COURT - NIGHT");
    assert_eq!(
        ui.get_source().as_str(),
        format!(
            "{}EXT. COURT - NIGHT",
            &navigation_source[..navigation_source.rfind("INT. CAFÉ").unwrap()]
        )
    );
    shortcut(&ui, "z", false);
    assert_eq!(ui.get_source().as_str(), navigation_source);
    // An outdated outline must not move the caret to an old source range.
    let changed = format!("new prefix\n{navigation_source}");
    ui.set_source(changed.clone().into());
    history.borrow_mut().reset(&changed);
    ui.invoke_select_source(0, 0);
    ui.invoke_navigate(1);
    assert!(ui.get_status().contains("Navigation is updating"));
    text(&ui, "X");
    assert_eq!(ui.get_source().as_str(), format!("X{changed}"));
    let notes_source = "🌙 Zoë\r\n[[First 世界]]\r\n[[Second\r\n  \r\nline]]";
    ui.set_source(notes_source.into());
    history.borrow_mut().reset(notes_source);
    checkpoint.borrow_mut().mark_saved(notes_source);
    reflect_checkpoint(&ui, &checkpoint.borrow());
    ui.set_outline_source(notes_source.into());
    ui.set_outline_category(0);
    let (summary, navigation, lines) = derived(notes_source, 0);
    assert_eq!(navigation.notes.len(), 2);
    apply(&ui, summary, navigation, lines, 0.);
    ui.set_navigation_mode(1);
    ui.set_note_query("LINE".into());
    ui.invoke_note_filter_changed();
    assert_eq!(
        ui.get_visible_note_rows().row_count(),
        1,
        "filter searches the full multiline body, ignoring case"
    );
    assert_eq!(ui.get_note_filter_status().as_str(), "1 of 2 notes");
    assert!(
        ui.get_visible_note_rows()
            .row_data(0)
            .unwrap()
            .text
            .contains("Second")
    );
    let before_note_navigation = edits.get();
    ui.invoke_navigate(0);
    assert_eq!(edits.get(), before_note_navigation);
    assert!(!ui.get_dirty());
    text(&ui, "[[Updated 🌙]]");
    let updated_note = format!(
        "{}[[Updated 🌙]]",
        &notes_source[..notes_source.find("[[Second").unwrap()]
    );
    assert_eq!(
        ui.get_source().as_str(),
        updated_note,
        "note navigation selects full multiline note after Unicode prefix"
    );
    shortcut(&ui, "z", false);
    assert_eq!(ui.get_source().as_str(), notes_source);
    assert!(!ui.get_dirty());
    shortcut(&ui, "z", true);
    assert_eq!(ui.get_source().as_str(), updated_note);
    let stale_notes = format!("prefix {notes_source}");
    ui.set_source(stale_notes.clone().into());
    history.borrow_mut().reset(&stale_notes);
    ui.invoke_select_source(0, 0);
    ui.invoke_navigate(0);
    assert!(ui.get_status().contains("Navigation is updating"));
    text(&ui, "X");
    assert_eq!(ui.get_source().as_str(), format!("X{stale_notes}"));
    ui.set_note_query("missing".into());
    ui.invoke_note_filter_changed();
    assert_eq!(ui.get_visible_note_rows().row_count(), 0);
    assert_eq!(ui.get_note_filter_status().as_str(), "0 of 2 notes");
    ui.set_note_query("".into());
    ui.invoke_note_filter_changed();
    ui.set_source("".into());
    checkpoint.borrow_mut().require_save();
    reflect_checkpoint(&ui, &checkpoint.borrow());
    assert!(ui.get_dirty(), "an empty recovered draft is still unsaved");
    checkpoint.borrow_mut().mark_saved("");
    reflect_checkpoint(&ui, &checkpoint.borrow());
    assert!(
        !ui.get_dirty(),
        "an explicit successful empty save clears Edited"
    );
    // Closing moves focus away from the disabled editor. Keyboard traversal stays
    // within the three choices and Escape always resumes writing.
    let choices = Rc::new(RefCell::new(Vec::new()));
    let observed_choices = choices.clone();
    let weak = ui.as_weak();
    ui.on_close_choice(move |choice| {
        observed_choices.borrow_mut().push(choice);
        weak.upgrade().unwrap().set_confirm_close(false);
    });
    ui.set_source("draft".into());
    history.borrow_mut().reset("draft");
    ui.invoke_select_source(5, 5);
    ui.set_confirm_close(true);
    text(&ui, &slint::SharedString::from(Key::Return));
    assert_eq!(
        *choices.borrow(),
        [2],
        "safe initial choice is Keep Writing"
    );
    text(&ui, "!");
    assert_eq!(
        ui.get_source().as_str(),
        "draft!",
        "return restores the editor caret"
    );

    ui.set_confirm_close(true);
    text(&ui, &slint::SharedString::from(Key::Tab));
    text(&ui, &slint::SharedString::from(Key::Return));
    assert_eq!(*choices.borrow(), [2, 1], "Tab reaches Discard");
    ui.set_confirm_close(true);
    text(&ui, &slint::SharedString::from(Key::Tab));
    text(&ui, &slint::SharedString::from(Key::Tab));
    text(&ui, &slint::SharedString::from(Key::Return));
    assert_eq!(*choices.borrow(), [2, 1, 0], "second Tab reaches Save Copy");
    ui.set_confirm_close(true);
    for _ in 0..3 {
        text(&ui, &slint::SharedString::from(Key::Tab));
    }
    text(&ui, &slint::SharedString::from(Key::Return));
    assert_eq!(
        *choices.borrow(),
        [2, 1, 0, 2],
        "Tab wraps inside confirmation"
    );
    ui.set_confirm_close(true);
    text(&ui, &slint::SharedString::from(Key::Backtab));
    text(&ui, &slint::SharedString::from(Key::Return));
    assert_eq!(
        *choices.borrow(),
        [2, 1, 0, 2, 0],
        "reverse Tab wraps to Save Copy"
    );
    ui.set_confirm_close(true);
    text(&ui, &slint::SharedString::from(Key::Tab));
    text(&ui, &slint::SharedString::from(Key::Escape));
    assert_eq!(
        *choices.borrow(),
        [2, 1, 0, 2, 0, 2],
        "Escape resumes writing from any choice"
    );
    assert!(!ui.get_confirm_close());
    assert_eq!(
        ui.get_source().as_str(),
        "draft!",
        "confirmation keys never modify source"
    );
    install_find(&ui);
    let source = "🌙 Zoë\r\nMARA\r\nzoË\r\n[.*]";
    ui.set_source(source.into());
    history.borrow_mut().reset(source);
    checkpoint.borrow_mut().mark_saved(source);
    reflect_checkpoint(&ui, &checkpoint.borrow());
    ui.set_find_query("zoë".into());
    ui.invoke_show_find();
    // An empty query/result never edits the manuscript; the first hit is selected
    // through the production callback, then actual TextEdit input replaces it.
    ui.invoke_find_step(false);
    assert_eq!(ui.get_find_status().as_str(), "1 of 2 matches");
    text(&ui, &slint::SharedString::from(Key::Escape));
    assert!(
        !ui.get_find_open(),
        "Escape also dismisses Find from the editor"
    );
    let before_find_again = edits.get();
    shortcut(&ui, "g", false);
    assert!(!ui.get_find_open(), "Find Again keeps the bar closed");
    assert_eq!(ui.get_status().as_str(), "Find · 2 of 2 matches");
    shortcut(&ui, "g", true);
    assert_eq!(ui.get_status().as_str(), "Find · 1 of 2 matches");
    assert_eq!(
        edits.get(),
        before_find_again,
        "navigation never creates document edits/recovery writes"
    );
    assert!(!ui.get_dirty());
    ui.set_find_query("absent".into());
    shortcut(&ui, "g", false);
    assert_eq!(ui.get_status().as_str(), "Find · No matches");
    assert_eq!(ui.get_source().as_str(), source);
    ui.set_find_query("zoë".into());
    shortcut(&ui, "g", false);
    ui.set_find_open(true);
    assert_eq!(ui.get_source().as_str(), source);
    assert!(!ui.get_dirty());
    text(&ui, "Café");
    assert_eq!(ui.get_source().as_str(), "🌙 Café\r\nMARA\r\nzoË\r\n[.*]");
    assert!(ui.get_dirty());
    ui.invoke_find_invalidated();
    assert_eq!(
        ui.get_find_status().as_str(),
        "Press Return to search current draft"
    );
    shortcut(&ui, "z", false);
    assert_eq!(ui.get_source().as_str(), source);
    assert!(!ui.get_dirty());
    ui.invoke_find_step(false);
    assert_eq!(ui.get_find_status().as_str(), "2 of 2 matches");
    text(&ui, "世界");
    assert_eq!(ui.get_source().as_str(), "🌙 Zoë\r\nMARA\r\n世界\r\n[.*]");
    shortcut(&ui, "z", false);
    assert_eq!(ui.get_source().as_str(), source);
    ui.set_find_query("[.*]".into());
    ui.invoke_find_step(false);
    assert_eq!(ui.get_find_status().as_str(), "1 of 1 matches");
    ui.set_confirm_close(true);
    ui.set_find_query("MARA".into());
    ui.invoke_find_step(false);
    text(&ui, "blocked");
    assert_eq!(ui.get_source().as_str(), source);
    ui.set_confirm_close(false);
    ui.set_find_query("absent".into());
    ui.invoke_find_step(false);
    assert_eq!(ui.get_find_status().as_str(), "No matches");
    ui.set_find_query("".into());
    ui.invoke_find_step(false);
    assert_eq!(ui.get_find_status().as_str(), "Enter text to search");
    ui.set_find_open(false);
    shortcut(&ui, "f", false);
    assert!(
        ui.get_find_open(),
        "Find shortcut opens the actual search field"
    );
    text(&ui, "needle");
    assert_eq!(ui.get_find_query().as_str(), "needle");
    text(&ui, &slint::SharedString::from(Key::Escape));
    assert!(!ui.get_find_open());
    assert_eq!(ui.get_source().as_str(), source);
    // Replacement actions use the same production callback/history/persistence path.
    ui.set_find_open(true);
    ui.set_replace_open(true);
    ui.set_find_query("zoë".into());
    ui.set_replacement("$1 世界".into());
    ui.invoke_find_step(false);
    let before_edits = edits.get();
    ui.invoke_replace_match(false);
    let single = "🌙 $1 世界\r\nMARA\r\nzoË\r\n[.*]";
    assert_eq!(ui.get_source().as_str(), single);
    assert_eq!(ui.get_find_status().as_str(), "Replaced 1 match");
    assert_eq!(edits.get(), before_edits + 1);
    assert!(ui.get_dirty());
    assert!(
        !ui.get_find_can_replace(),
        "a replaced match cannot be replayed"
    );
    assert_eq!(recovery::read(edit_recovery.path()).unwrap().source, single);
    shortcut(&ui, "z", false);
    assert_eq!(ui.get_source().as_str(), source);
    assert!(!ui.get_dirty());
    shortcut(&ui, "z", true);
    assert_eq!(ui.get_source().as_str(), single);
    shortcut(&ui, "z", false);
    assert_eq!(ui.get_source().as_str(), source);
    ui.set_replacement("✨".into());
    ui.invoke_replace_match(true);
    let all = "🌙 ✨\r\nMARA\r\n✨\r\n[.*]";
    assert_eq!(ui.get_source().as_str(), all);
    assert_eq!(ui.get_find_status().as_str(), "Replaced 2 matches");
    ui.invoke_find_invalidated();
    assert_eq!(
        ui.get_find_status().as_str(),
        "Replaced 2 matches",
        "queued invalidation cannot erase a completed replacement"
    );
    let copy = recovery_root.path().join("replaced.fountain");
    save_copy(&copy, ui.get_source().as_str()).unwrap();
    assert_eq!(std::fs::read_to_string(&copy).unwrap(), all);
    assert_eq!(recovery::read(edit_recovery.path()).unwrap().source, all);
    shortcut(&ui, "z", false);
    assert_eq!(
        ui.get_source().as_str(),
        source,
        "Replace All is one Undo transaction"
    );
    assert!(!ui.get_dirty());
    assert_eq!(recovery::read(edit_recovery.path()).unwrap().source, source);
    shortcut(&ui, "z", true);
    assert_eq!(ui.get_source().as_str(), all);
    shortcut(&ui, "z", false);
    ui.invoke_find_step(false);
    ui.set_find_query("MARA".into());
    let before_edits = edits.get();
    ui.invoke_replace_match(false);
    assert_eq!(
        ui.get_source().as_str(),
        source,
        "changed query cannot replace an old match"
    );
    assert_eq!(edits.get(), before_edits);
    ui.set_replacement("bad\0text".into());
    ui.invoke_replace_match(true);
    assert_eq!(ui.get_source().as_str(), source);
    assert_eq!(edits.get(), before_edits);
    ui.set_confirm_close(true);
    ui.set_replacement("blocked".into());
    ui.invoke_replace_match(true);
    assert_eq!(
        ui.get_source().as_str(),
        source,
        "modal guard blocks bulk replacement"
    );
    ui.set_confirm_close(false);
    ui.set_replacement("MARA".into());
    ui.invoke_replace_match(true);
    assert_eq!(ui.get_find_status().as_str(), "No text changes needed");
    assert_eq!(
        edits.get(),
        before_edits,
        "identical replacement creates no history/recovery transaction"
    );
    assert!(!ui.get_dirty());
    let word_source = "🌙 MARA MARATHON mara MARA_2\r\n";
    ui.set_source(word_source.into());
    history.borrow_mut().reset(word_source);
    checkpoint.borrow_mut().mark_saved(word_source);
    ui.set_find_match_case(false);
    ui.set_find_mode(0);
    ui.set_find_query("MARA".into());
    ui.set_replacement("NINA".into());
    ui.invoke_find_step(false);
    assert_eq!(ui.get_find_status().as_str(), "1 of 4 matches");
    ui.set_find_mode(2);
    let before = edits.get();
    ui.invoke_replace_match(false);
    assert_eq!(
        ui.get_source().as_str(),
        word_source,
        "changing whole-word mode cannot reuse the old target"
    );
    assert_eq!(edits.get(), before);
    ui.invoke_find_step(false);
    assert_eq!(ui.get_find_status().as_str(), "1 of 2 matches");
    ui.invoke_replace_match(true);
    assert_eq!(ui.get_source().as_str(), "🌙 NINA MARATHON NINA MARA_2\r\n");
    assert_eq!(
        recovery::read(edit_recovery.path()).unwrap().source,
        ui.get_source().as_str()
    );
    shortcut(&ui, "z", false);
    assert_eq!(ui.get_source().as_str(), word_source);
    assert!(!ui.get_dirty());
    shortcut(&ui, "z", true);
    assert_eq!(ui.get_source().as_str(), "🌙 NINA MARATHON NINA MARA_2\r\n");
    ui.set_find_mode(0);

    ui.set_source(word_source.into());
    history.borrow_mut().reset(word_source);
    checkpoint.borrow_mut().mark_saved(word_source);
    reflect_checkpoint(&ui, &checkpoint.borrow());
    ui.set_find_mode(1);
    ui.invoke_find_step(false);
    assert_eq!(ui.get_find_status().as_str(), "1 of 4 matches");
    ui.set_find_wrap(false);
    ui.invoke_find_step(true);
    assert_eq!(
        ui.get_find_status().as_str(),
        "Reached the beginning of the draft"
    );
    assert_eq!(ui.get_source().as_str(), word_source);
    assert!(!ui.get_dirty());
    ui.set_find_wrap(true);
    ui.invoke_find_step(true);
    assert_eq!(ui.get_find_status().as_str(), "4 of 4 matches");
    ui.set_find_mode(0);

    let regex_source = "🌙 [[Café e\u{301}]]\r\n[[世界]]\r\n";
    ui.set_source(regex_source.into());
    history.borrow_mut().reset(regex_source);
    checkpoint.borrow_mut().mark_saved(regex_source);
    ui.set_find_regex(true);
    ui.set_find_query(r"\[\[(.*?)\]\]".into());
    ui.set_replacement("**$1**".into());
    ui.invoke_find_step(false);
    assert_eq!(ui.get_find_status().as_str(), "1 of 2 matches");
    ui.invoke_replace_match(true);
    let regex_result = "🌙 **Café e\u{301}**\r\n**世界**\r\n";
    assert_eq!(ui.get_source().as_str(), regex_result);
    assert_eq!(
        recovery::read(edit_recovery.path()).unwrap().source,
        regex_result
    );
    shortcut(&ui, "z", false);
    assert_eq!(ui.get_source().as_str(), regex_source);
    assert!(!ui.get_dirty());
    shortcut(&ui, "z", true);
    assert_eq!(ui.get_source().as_str(), regex_result);
    let before = edits.get();
    ui.set_replacement("$missing".into());
    ui.invoke_replace_match(true);
    assert_eq!(ui.get_source().as_str(), regex_result);
    assert_eq!(edits.get(), before);
    ui.set_find_query("[".into());
    ui.invoke_find_step(false);
    assert!(!ui.get_find_can_replace());
    assert!(ui.get_find_status().as_str().contains("Invalid"));
    ui.invoke_replace_match(true);
    assert_eq!(ui.get_source().as_str(), regex_result);
    assert_eq!(edits.get(), before);
    ui.set_find_regex(false);

    let advance_source = "catcat cat\r\n";
    ui.set_source(advance_source.into());
    history.borrow_mut().reset(advance_source);
    checkpoint.borrow_mut().mark_saved(advance_source);
    ui.set_find_wrap(false);
    ui.set_find_query("cat".into());
    ui.set_replacement("".into());
    ui.invoke_find_step(false);
    ui.invoke_replace_and_find();
    assert_eq!(ui.get_source().as_str(), "cat cat\r\n");
    assert!(ui.get_find_can_replace());
    ui.invoke_replace_and_find();
    assert_eq!(ui.get_source().as_str(), " cat\r\n");
    assert_eq!(
        recovery::read(edit_recovery.path()).unwrap().source,
        " cat\r\n"
    );
    shortcut(&ui, "z", false);
    assert_eq!(
        ui.get_source().as_str(),
        "cat cat\r\n",
        "each Replace & Find is one source transaction"
    );
    shortcut(&ui, "z", true);
    assert_eq!(ui.get_source().as_str(), " cat\r\n");
    ui.invoke_find_step(false);
    ui.set_replacement("cat".into());
    let before = edits.get();
    ui.invoke_replace_and_find();
    assert_eq!(ui.get_source().as_str(), " cat\r\n");
    assert_eq!(
        edits.get(),
        before,
        "identical replacement advances without source/history/recovery writes"
    );
    assert!(ui.get_find_status().as_str().contains("Reached the end"));
    ui.set_find_wrap(true);

    // Menu entry points use the same history and route clipboard operations to
    // the focused field, rather than accidentally editing the manuscript.
    let source = "Café 🌙 世界";
    ui.set_source(source.into());
    history.borrow_mut().reset(source);
    reflect_history(&ui, &history.borrow());
    checkpoint.borrow_mut().mark_saved(source);
    reflect_checkpoint(&ui, &checkpoint.borrow());
    ui.invoke_select_source(source.len() as i32, source.len() as i32);
    assert!(!ui.get_can_undo());
    assert!(!ui.get_can_redo());
    text(&ui, "!");
    assert!(ui.get_can_undo());
    ui.invoke_perform_history(false);
    assert_eq!(ui.get_source().as_str(), source);
    assert!(!ui.get_can_undo());
    assert!(ui.get_can_redo());
    ui.invoke_perform_history(true);
    let edited_source = format!("{source}!");
    assert_eq!(ui.get_source().as_str(), edited_source);
    assert!(ui.get_can_undo());
    assert!(!ui.get_can_redo());
    ui.invoke_clipboard_action(3);
    ui.invoke_clipboard_action(1);
    ui.invoke_show_find();
    ui.invoke_clipboard_action(3);
    ui.invoke_clipboard_action(2);
    assert_eq!(ui.get_find_query().as_str(), edited_source);
    ui.invoke_perform_history(false);
    assert_eq!(
        ui.get_source().as_str(),
        edited_source,
        "document Undo is disabled while Find field is focused"
    );
    ui.invoke_show_replace();
    ui.invoke_clipboard_action(3);
    ui.invoke_clipboard_action(2);
    assert_eq!(ui.get_replacement().as_str(), edited_source);
    assert_eq!(ui.get_source().as_str(), edited_source);
    ui.set_confirm_close(true);
    ui.invoke_clipboard_action(0);
    ui.invoke_perform_history(false);
    assert_eq!(ui.get_source().as_str(), edited_source);
    ui.set_confirm_close(false);
    ui.invoke_select_source(0, edited_source.len() as i32);
    ui.invoke_clipboard_action(0);
    assert_eq!(ui.get_source().as_str(), "");
    ui.invoke_perform_history(false);
    assert_eq!(
        ui.get_source().as_str(),
        edited_source,
        "menu Cut is one Undo transaction"
    );
    ui.invoke_perform_history(false);
    assert_eq!(
        ui.get_source().as_str(),
        source,
        "typing transaction remains separate from menu Cut"
    );
    assert!(!ui.get_dirty());
    history.borrow_mut().reset("externally changed");
    history.borrow_mut().record("externally changed!");
    reflect_history(&ui, &history.borrow());
    assert!(
        !ui.get_can_undo() && !ui.get_can_redo(),
        "history cannot advertise operations for another source"
    );
    // Find/replacement have independent transactions, not the document's stack.
    ui.set_find_query("".into());
    ui.set_replacement("".into());
    ui.invoke_field_sync();
    ui.invoke_show_find();
    let before_edits = edits.get();
    let recovery_before = std::fs::read(edit_recovery.path()).unwrap();
    for ch in ["Z", "o", "ë"] {
        text(&ui, ch);
    }
    assert_eq!(ui.get_find_query().as_str(), "Zoë");
    assert!(ui.get_query_can_undo());
    shortcut(&ui, "z", false);
    assert_eq!(
        ui.get_find_query().as_str(),
        "",
        "field typing is one Undo run"
    );
    assert!(ui.get_query_can_redo());
    ui.invoke_show_replace();
    for ch in ["世", "界"] {
        text(&ui, ch);
    }
    assert_eq!(ui.get_replacement().as_str(), "世界");
    assert!(ui.get_replacement_can_undo());
    shortcut(&ui, "z", false);
    assert_eq!(ui.get_replacement().as_str(), "");
    shortcut(&ui, "z", true);
    assert_eq!(ui.get_replacement().as_str(), "世界");
    ui.invoke_show_find();
    shortcut(&ui, "z", true);
    assert_eq!(
        ui.get_find_query().as_str(),
        "Zoë",
        "query Redo survives edits in replacement field"
    );
    assert_eq!(ui.get_replacement().as_str(), "世界");
    ui.set_find_query("MARA".into());
    ui.invoke_show_find();
    assert!(
        !ui.get_query_can_undo() && !ui.get_query_can_redo(),
        "programmatic load resets only that field's history"
    );
    ui.invoke_select_field(1, 4, 4);
    text(&ui, "!");
    assert_eq!(ui.get_find_query().as_str(), "MARA!");
    ui.invoke_perform_history(false);
    assert_eq!(ui.get_find_query().as_str(), "MARA");
    ui.set_confirm_close(true);
    ui.invoke_field_history_action(1, true);
    assert_eq!(
        ui.get_find_query().as_str(),
        "MARA",
        "confirmation blocks field history too"
    );
    ui.set_confirm_close(false);
    assert_eq!(ui.get_source().as_str(), source);
    assert!(!ui.get_dirty());
    assert_eq!(
        edits.get(),
        before_edits,
        "field Undo never emits a manuscript edit"
    );
    assert_eq!(
        std::fs::read(edit_recovery.path()).unwrap(),
        recovery_before,
        "field history does not overwrite manuscript recovery"
    );
    let before_filter_edits = edits.get();
    let before_filter_recovery = std::fs::read(edit_recovery.path()).unwrap();
    ui.set_navigation_mode(1);
    ui.set_note_query("".into());
    ui.invoke_field_sync();
    ui.invoke_select_field(3, 0, 0);
    text(&ui, "world");
    assert_eq!(ui.get_note_query().as_str(), "world");
    assert_eq!(ui.get_history_context(), 3);
    ui.invoke_perform_history(false);
    assert_eq!(ui.get_note_query().as_str(), "");
    ui.invoke_perform_history(true);
    assert_eq!(ui.get_note_query().as_str(), "world");
    ui.invoke_select_field(3, 0, 5);
    ui.invoke_clipboard_action(1);
    ui.invoke_select_field(3, 5, 5);
    ui.invoke_clipboard_action(2);
    assert_eq!(ui.get_note_query().as_str(), "worldworld");
    ui.invoke_perform_history(false);
    assert_eq!(ui.get_note_query().as_str(), "world");
    assert_eq!(ui.get_source().as_str(), source);
    assert_eq!(edits.get(), before_filter_edits);
    assert_eq!(
        std::fs::read(edit_recovery.path()).unwrap(),
        before_filter_recovery
    );
    ui.set_navigation_mode(0);
    ui.invoke_focus_source();

    // File pickers and repeated Quit/Close requests share the same source check.
    install_close_requests(&ui, checkpoint.clone());
    assert!(ui.invoke_can_terminate());
    assert!(matches!(
        close_response(&ui, &checkpoint.borrow()),
        slint::CloseRequestResponse::HideWindow
    ));
    {
        let _picker = DialogGuard::new(&ui);
        assert!(ui.get_file_dialog_open());
        assert!(!ui.invoke_can_terminate());
        ui.invoke_request_close();
        assert!(
            !ui.get_confirm_close(),
            "close request cannot create a second confirmation behind picker"
        );
        assert!(matches!(
            close_response(&ui, &checkpoint.borrow()),
            slint::CloseRequestResponse::KeepWindowShown
        ));
        ui.invoke_select_source(0, source.len() as i32);
        text(&ui, "blocked by picker");
        assert_eq!(ui.get_source().as_str(), source);
    }
    assert!(
        !ui.get_file_dialog_open(),
        "picker cancellation/error scope restores interaction"
    );
    let unsaved = format!("{source} unsaved");
    ui.set_source(unsaved.clone().into());
    assert!(
        !ui.invoke_can_terminate(),
        "termination checks exact source, not a stale Edited flag"
    );
    ui.invoke_request_close();
    assert!(ui.get_confirm_close());
    ui.invoke_request_close();
    assert!(
        ui.get_confirm_close(),
        "repeated Quit keeps the existing confirmation"
    );
    assert_eq!(ui.get_source().as_str(), unsaved);
    ui.set_confirm_close(false);
    ui.set_source(source.into());
    let new_project = Rc::new(RefCell::new(None));
    let new_recovery = Rc::new(RefCell::new(Ok(recovery::Recovery::create(
        recovery_root.path(),
    )
    .unwrap())));
    let new_root = recovery_root.path().to_owned();
    install_new_draft(
        &ui,
        DraftSession {
            checkpoint: checkpoint.clone(),
            history: history.clone(),
            project: new_project.clone(),
            recovery: new_recovery.clone(),
        },
        move || Ok(new_root.clone()),
        |_| {},
    );
    let chooser_actions = Rc::new(RefCell::new(Vec::new()));
    let observed_actions = chooser_actions.clone();
    let weak = ui.as_weak();
    ui.on_file_action(move |action| {
        let ui = weak.upgrade().unwrap();
        observed_actions.borrow_mut().push((
            action,
            ui.get_file_dialog_open(),
            ui.get_new_draft_open(),
        ));
    });
    for (choice, action) in [(-2, 0), (-3, 4)] {
        ui.invoke_new_draft();
        ui.invoke_new_category(choice);
        assert_eq!(
            chooser_actions.borrow().last(),
            Some(&(action, false, false)),
            "chooser releases locks before dispatching the existing file action"
        );
        assert_eq!(ui.get_source().as_str(), source);
        assert!(!ui.get_dirty());
    }
    ui.invoke_new_draft();
    assert!(ui.get_new_draft_open());
    assert!(ui.get_file_dialog_open());
    text(&ui, "cannot edit behind chooser");
    assert_eq!(ui.get_source().as_str(), source);
    text(&ui, &slint::SharedString::from(Key::Escape));
    assert!(!ui.get_new_draft_open());
    assert!(!ui.get_file_dialog_open());
    assert_eq!(ui.get_source().as_str(), source);
    ui.set_source(unsaved.clone().into());
    ui.invoke_new_draft();
    assert!(
        !ui.get_new_draft_open(),
        "dirty source cannot be replaced by New"
    );
    ui.set_source(source.into());
    ui.invoke_new_draft();
    text(&ui, &slint::SharedString::from(Key::Tab));
    text(&ui, &slint::SharedString::from(Key::Return));
    assert_eq!(ui.get_source().as_str(), "");
    assert_eq!(ui.get_category(), 1);
    assert_eq!(
        ui.get_outline_rows().row_count(),
        0,
        "New clears the prior outline immediately"
    );
    assert_eq!(
        ui.get_lines().row_count(),
        0,
        "New clears the prior preview immediately"
    );
    assert!(ui.get_project_mode());
    assert!(
        ui.get_dirty(),
        "even empty new project must save chosen type/identity"
    );
    assert!(!ui.get_file_dialog_open());
    assert!(!ui.get_new_draft_open());
    assert_eq!(
        new_project.borrow().as_ref().unwrap().preset(),
        "comic-full-script"
    );
    assert!(!ui.get_can_undo(), "New resets previous document history");
    assert!(
        !ui.invoke_can_terminate(),
        "empty new project is not silently discarded by Quit"
    );
    let appearance_path = recovery_root.path().join("preferences/appearance.json");
    let before_appearance_source = ui.get_source();
    let before_appearance_recovery =
        std::fs::read(new_recovery.borrow().as_ref().unwrap().path()).unwrap();
    install_appearance(&ui, Ok(appearance_path.clone()));
    assert!(
        !appearance_path.exists(),
        "loading defaults does not create settings"
    );
    ui.set_dark(true);
    ui.set_white_paper(false);
    ui.invoke_appearance_changed();
    assert_eq!(
        appearance::load(&appearance_path).unwrap(),
        appearance::Appearance {
            dark: true,
            white_paper: false
        }
    );
    assert_eq!(ui.get_source(), before_appearance_source);
    assert!(
        ui.get_dirty(),
        "appearance cannot mark an unsaved project saved"
    );
    assert_eq!(
        std::fs::read(new_recovery.borrow().as_ref().unwrap().path()).unwrap(),
        before_appearance_recovery
    );
    let reopened_ui = Studio::new().unwrap();
    install_appearance(&reopened_ui, Ok(appearance_path));
    assert!(reopened_ui.get_dark());
    assert!(!reopened_ui.get_white_paper());
    ui.hide().unwrap();
}

#[test]
fn opt_in_formatted_surface_edits_the_real_draft_and_shared_history() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    install_find(&ui);
    let source = "INT. ROOM - DAY\r\n\r\nMARA\r\nZoë e\u{301} waits.\r\n";
    ui.set_source(source.into());
    let checkpoint = Rc::new(RefCell::new(session::Checkpoint::saved(source)));
    let observed_checkpoint = checkpoint.clone();
    let root = tempfile::tempdir().unwrap();
    let recovery = Rc::new(recovery::Recovery::create(root.path()).unwrap());
    let observed_recovery = recovery.clone();
    let _history = install_editor_history(&ui, move |ui| {
        reflect_checkpoint(ui, &observed_checkpoint.borrow());
        observed_recovery
            .write(ui.get_source().as_str(), ui.get_category())
            .unwrap();
    });
    ui.show().unwrap();
    assert!(!ui.get_formatted_active(), "standard input is the default");
    ui.set_formatted_active(true);
    ui.invoke_formatted_refresh(664., 500.);
    assert!(ui.get_formatted_frame().size().width > 0);
    let start = source.find("e\u{301}").unwrap();
    ui.invoke_select_source(start as i32, (start + "e\u{301}".len()) as i32);
    text(&ui, "新しい");
    let expected = source.replace("e\u{301}", "新しい");
    assert_eq!(
        ui.get_source().as_str(),
        expected,
        "formatted input reaches the actual bound draft"
    );
    assert_eq!(
        ui.get_formatted_shaped_paragraphs(),
        1,
        "live editing shapes only the changed paragraph"
    );
    assert!(ui.get_formatted_reused_paragraphs() > 0);
    assert!(ui.get_dirty());
    shortcut(&ui, "z", false);
    assert_eq!(ui.get_source().as_str(), source);
    assert!(
        ui.get_formatted_reused_paragraphs() > 0,
        "production Undo adoption also reuses unchanged shapes"
    );
    assert!(!ui.get_dirty());
    shortcut(&ui, "z", true);
    assert_eq!(ui.get_source().as_str(), expected);
    ui.invoke_show_find();
    ui.set_find_query("waits".into());
    ui.invoke_find_step(false);
    text(&ui, "listens");
    assert_eq!(
        ui.get_source().as_str(),
        expected.replace("waits", "listens")
    );
    ui.invoke_history_action(false);
    assert_eq!(ui.get_source().as_str(), expected);
    ui.set_find_open(false);
    ui.invoke_focus_source();
    ui.invoke_clipboard_action(3); // native Edit > Select All must select the custom controller too
    text(&ui, "All selected\r\n");
    assert_eq!(ui.get_source().as_str(), "All selected\r\n");
    ui.invoke_history_action(false);
    assert_eq!(ui.get_source().as_str(), expected);
    let path = root.path().join("Formatted Copy.fountain");
    save_copy(&path, ui.get_source().as_str()).unwrap();
    assert_eq!(source_draft::read(&path).unwrap(), expected);
    assert_eq!(recovery::read(recovery.path()).unwrap().source, expected);
    ui.set_confirm_close(true);
    text(&ui, "must not change");
    assert_eq!(ui.get_source().as_str(), expected);
    ui.set_confirm_close(false);
    ui.set_formatted_active(false);
    ui.invoke_select_source(0, 0);
    text(&ui, "!");
    assert_eq!(
        ui.get_source().as_str(),
        format!("!{expected}"),
        "standard editor still works after returning"
    );
    ui.hide().unwrap();
}

#[test]
fn formatted_clipboard_uses_exact_controller_selection_and_atomic_source_edits() {
    // This clipboard belongs to the isolated testing backend, never the desktop.
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    let source = "INT. ROOM - DAY\r\n\r\nMARA\r\nZoë e\u{301} 👩‍💻 says שלום.\r\n\r\n!target\r\n";
    ui.set_source(source.into());
    let edits = Rc::new(Cell::new(0));
    let changed = edits.clone();
    let checkpoint = Rc::new(RefCell::new(session::Checkpoint::saved(source)));
    let saved = checkpoint.clone();
    let folder = tempfile::tempdir().unwrap();
    let recovery = Rc::new(recovery::Recovery::create(folder.path()).unwrap());
    let observed_recovery = recovery.clone();
    let _history = install_editor_history(&ui, move |ui| {
        changed.set(changed.get() + 1);
        reflect_checkpoint(ui, &saved.borrow());
        observed_recovery
            .write(ui.get_source().as_str(), 0)
            .unwrap();
    });
    ui.show().unwrap();
    ui.set_formatted_active(true);
    ui.invoke_formatted_refresh(664., 500.);
    ui.invoke_select_source(0, 3);
    ui.invoke_clipboard_action(2); // isolated backend clipboard starts empty
    assert_eq!(
        ui.get_source().as_str(),
        source,
        "empty Paste must not delete a selection"
    );
    assert_eq!(edits.get(), 0);
    let a = source.find("Zoë").unwrap();
    let b = source.find("!target").unwrap();
    let copied = &source[a..b];
    ui.invoke_select_source(b as i32, a as i32); // reversed, includes CRLF and Unicode clusters
    assert_eq!(ui.invoke_formatted_selection().as_str(), copied);
    assert!(ui.get_formatted_has_selection());
    ui.invoke_clipboard_action(1);
    assert_eq!(ui.get_source().as_str(), source);
    assert_eq!(edits.get(), 0);
    assert!(!ui.get_dirty());
    let target = source.find("target").unwrap();
    ui.invoke_select_source(target as i32, (target + 6) as i32);
    ui.invoke_clipboard_action(2);
    let pasted = format!("{}{}{}", &source[..target], copied, &source[target + 6..]);
    assert_eq!(
        ui.get_source().as_str(),
        pasted,
        "paste uses the exact copied source, including markup and endings"
    );
    assert_eq!(edits.get(), 1, "replacement is one production source edit");
    assert!(!ui.get_formatted_has_selection());
    text(&ui, "!");
    let with_caret_edit = format!(
        "{}!{}",
        &pasted[..target + copied.len()],
        &pasted[target + copied.len()..]
    );
    assert_eq!(
        ui.get_source().as_str(),
        with_caret_edit,
        "paste positions the controller caret after the inserted bytes"
    );
    ui.invoke_history_action(false);
    assert_eq!(ui.get_source().as_str(), pasted);
    ui.invoke_history_action(false);
    assert_eq!(
        ui.get_source().as_str(),
        source,
        "one Undo restores the whole paste replacement"
    );
    assert!(!ui.get_dirty());
    ui.invoke_history_action(true);
    assert_eq!(ui.get_source().as_str(), pasted);
    ui.invoke_history_action(false);
    ui.invoke_select_source(b as i32, a as i32);
    ui.invoke_clipboard_action(0);
    let cut = format!("{}{}", &source[..a], &source[b..]);
    assert_eq!(ui.get_source().as_str(), cut);
    ui.invoke_history_action(false);
    assert_eq!(ui.get_source().as_str(), source);
    ui.invoke_history_action(true);
    assert_eq!(ui.get_source().as_str(), cut);
    ui.invoke_clipboard_action(2);
    assert_eq!(
        ui.get_source().as_str(),
        source,
        "Cut then Paste restores exact source at the cut caret"
    );
    ui.invoke_history_action(false);
    assert_eq!(ui.get_source().as_str(), cut);
    ui.invoke_history_action(false);
    assert_eq!(ui.get_source().as_str(), source);
    ui.invoke_select_source(0, 0);
    let count = edits.get();
    ui.invoke_clipboard_action(1);
    ui.invoke_clipboard_action(0);
    assert_eq!(edits.get(), count, "empty Cut/Copy are no-ops");
    assert!(!ui.get_formatted_has_selection());
    ui.invoke_select_source(source.len() as i32, source.len() as i32);
    ui.invoke_clipboard_action(2);
    assert_eq!(
        ui.get_source().as_str(),
        format!("{source}{copied}"),
        "empty Copy must preserve the existing clipboard"
    );
    let expected = ui.get_source().to_string();
    let path = folder.path().join("Clipboard Copy.fountain");
    save_copy(&path, &expected).unwrap();
    assert_eq!(source_draft::read(&path).unwrap(), expected);
    assert_eq!(recovery::read(recovery.path()).unwrap().source, expected);
    assert!(ui.get_formatted_caption().contains("Unsaved edits"));
    checkpoint.borrow_mut().mark_saved(&expected);
    reflect_checkpoint(&ui, &checkpoint.borrow());
    assert!(
        ui.get_formatted_caption()
            .contains("No unsaved text changes"),
        "saving changes the footer without needing a new source/raster"
    );
    ui.set_file_dialog_open(true);
    ui.invoke_clipboard_action(0);
    ui.invoke_clipboard_action(2);
    assert_eq!(ui.get_source().as_str(), expected);
    ui.set_file_dialog_open(false);
    ui.invoke_formatted_refresh(120., 50.);
    assert!(!ui.get_formatted_ready());
    ui.invoke_formatted_insert("must not edit stale geometry".into());
    assert_eq!(ui.get_source().as_str(), expected);
    ui.invoke_formatted_refresh(664., 500.);
    assert!(ui.get_formatted_ready());
    assert!(
        ui.get_formatted_frame().size().width > 0,
        "valid viewport recovers its image after clearing a stale frame"
    );
    ui.hide().unwrap();
}

#[test]
fn formatted_composition_callbacks_preview_cancel_and_commit_use_real_draft_transactions() {
    // Explicit adapter callbacks, not native IME delivery: public FocusScope does
    // not expose backend composition. The native bridge remains an open gate.
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    let source = "INT. ROOM - DAY\r\n\r\nMARA\r\nAn old signal.\r\n";
    ui.set_source(source.into());
    let edits = Rc::new(Cell::new(0));
    let changed = edits.clone();
    let checkpoint = Rc::new(RefCell::new(session::Checkpoint::saved(source)));
    let saved = checkpoint.clone();
    let folder = tempfile::tempdir().unwrap();
    let recovery = Rc::new(recovery::Recovery::create(folder.path()).unwrap());
    recovery.write(source, 0).unwrap();
    let before_recovery = std::fs::read(recovery.path()).unwrap();
    let observed = recovery.clone();
    let _history = install_editor_history(&ui, move |ui| {
        changed.set(changed.get() + 1);
        reflect_checkpoint(ui, &saved.borrow());
        observed.write(ui.get_source().as_str(), 0).unwrap();
    });
    ui.show().unwrap();
    ui.set_formatted_active(true);
    ui.invoke_formatted_refresh(664., 500.);
    let a = source.find("old").unwrap();
    ui.invoke_select_source((a + 3) as i32, a as i32);
    ui.invoke_formatted_preedit("にほん".into(), -1, -1);
    assert!(ui.get_formatted_composing());
    assert!(!ui.get_dirty());
    assert_eq!(edits.get(), 0);
    assert_eq!(ui.get_source().as_str(), source);
    assert_eq!(std::fs::read(recovery.path()).unwrap(), before_recovery);
    assert!(!ui.get_formatted_has_selection());
    assert!(ui.invoke_formatted_selection().is_empty());
    let path = folder.path().join("Uncommitted must not save.fountain");
    save_copy(&path, ui.get_source().as_str()).unwrap();
    assert_eq!(source_draft::read(&path).unwrap(), source);
    ui.invoke_formatted_preedit("日本".into(), 0, 3);
    assert_eq!(edits.get(), 0);
    ui.invoke_formatted_cancel();
    assert!(!ui.get_formatted_composing());
    assert_eq!(ui.get_source().as_str(), source);
    ui.invoke_formatted_preedit("にほん".into(), -1, -1);
    ui.invoke_formatted_commit("日本語".into());
    let expected = source.replace("old", "日本語");
    assert_eq!(ui.get_source().as_str(), expected);
    assert!(!ui.get_formatted_composing());
    assert!(ui.get_dirty());
    assert_eq!(edits.get(), 1);
    assert_eq!(recovery::read(recovery.path()).unwrap().source, expected);
    ui.invoke_history_action(false);
    assert_eq!(ui.get_source().as_str(), source);
    assert!(!ui.get_dirty());
    ui.invoke_history_action(true);
    assert_eq!(ui.get_source().as_str(), expected);
    ui.invoke_select_source(a as i32, (a + "日本語".len()) as i32);
    ui.invoke_formatted_preedit("かな".into(), -1, -1);
    ui.set_formatted_active(false);
    ui.invoke_focus_source();
    text(&ui, "!");
    assert!(!ui.get_formatted_composing());
    assert_eq!(
        ui.get_source().as_str(),
        expected.replace("日本語", "かな!"),
        "leaving the field preserves pending input and positions the standard caret after it"
    );
    ui.hide().unwrap();
}

#[test]
fn native_composition_transport_and_candidate_requests_are_scoped_to_formatted_focus() {
    use i_slint_core::window::{InputMethodRequest, WindowInner};
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    install_find(&ui);
    let source = "INT. ROOM - DAY\r\n\r\nMARA\r\nAn old signal.\r\n";
    ui.set_source(source.into());
    let edits = Rc::new(Cell::new(0));
    let observed = edits.clone();
    let checkpoint = Rc::new(RefCell::new(session::Checkpoint::saved(source)));
    let saved = checkpoint.clone();
    let _history = install_editor_history(&ui, move |ui| {
        observed.set(observed.get() + 1);
        reflect_checkpoint(ui, &saved.borrow());
    });
    install_close_requests(&ui, checkpoint.clone());
    ui.show().unwrap();
    let adapter = WindowInner::from_pub(ui.window()).window_adapter();
    let internal = adapter.internal(i_slint_core::InternalToken).unwrap();
    let backend = (internal as &dyn std::any::Any)
        .downcast_ref::<i_slint_backend_testing::testing_backend::TestingWindow>()
        .unwrap();
    ui.set_formatted_active(true);
    ui.invoke_formatted_refresh(664., 500.);
    ui.invoke_focus_source();
    ui.invoke_formatted_focus_changed();
    assert!(ui.get_formatted_ime_active());
    let props = || {
        backend
            .ime_requests()
            .into_iter()
            .rev()
            .find_map(|r| match r {
                InputMethodRequest::Enable(p) | InputMethodRequest::Update(p) => Some(p),
                _ => None,
            })
            .unwrap()
    };
    let scene_x = props().cursor_rect_origin.x;
    let mara = source.find("MARA").unwrap();
    ui.invoke_select_source(mara as i32, mara as i32);
    assert!(
        props().cursor_rect_origin.x > scene_x + 100.,
        "candidate uses the indented shaped character caret"
    );
    let a = source.find("old").unwrap();
    ui.invoke_select_source((a + 3) as i32, a as i32);
    composition(&ui, "にほん", "", false);
    assert!(
        ui.get_formatted_composing(),
        "actual backend InternalKeyEvent is routed to custom composition"
    );
    assert_eq!(ui.get_source().as_str(), source);
    assert_eq!(edits.get(), 0);
    assert_eq!(props().text.as_str(), source);
    assert_eq!(props().preedit_text.as_str(), "にほん");
    assert_eq!(props().preedit_offset, a);
    composition(&ui, "", "日本語", true);
    let expected = source.replace("old", "日本語");
    assert_eq!(ui.get_source().as_str(), expected);
    assert_eq!(edits.get(), 1);
    ui.invoke_history_action(false);
    assert_eq!(ui.get_source().as_str(), source);
    assert!(!ui.get_dirty());
    ui.invoke_select_source(a as i32, (a + 3) as i32);
    composition(&ui, "", "e\u{301}", true);
    assert_eq!(
        ui.get_source().as_str(),
        source.replace("old", "e\u{301}"),
        "custom transport preserves the native commit's exact normalization form"
    );
    ui.invoke_history_action(false);
    assert_eq!(ui.get_source().as_str(), source);
    // Move to an ordinary query field before application focus-change callbacks
    // run. The toolkit's owner identity check must route to that TextInput.
    ui.invoke_show_find();
    composition(&ui, "かな", "", false);
    composition(&ui, "", "検索", true);
    assert_eq!(ui.get_find_query().as_str(), "検索");
    assert_eq!(ui.get_source().as_str(), source);
    backend.clear_ime_requests();
    ui.invoke_formatted_focus_changed();
    assert!(!ui.get_formatted_ime_active());
    assert!(
        !backend
            .ime_requests()
            .iter()
            .any(|r| matches!(r, InputMethodRequest::Disable)),
        "disabling custom IME must not disable the newly focused standard input"
    );
    ui.set_find_open(false);
    ui.invoke_focus_source();
    ui.invoke_formatted_focus_changed();
    ui.set_file_dialog_open(true);
    composition(&ui, "must not enter draft", "", false);
    composition(&ui, "", "blocked", true);
    assert_eq!(ui.get_source().as_str(), source);
    ui.invoke_formatted_focus_changed();
    assert!(!ui.get_formatted_ime_active());
    ui.set_file_dialog_open(false);
    ui.invoke_focus_source();
    ui.invoke_formatted_focus_changed();
    ui.invoke_select_source(a as i32, (a + 3) as i32);
    composition(&ui, "かな", "", false);
    assert!(
        !ui.invoke_can_terminate(),
        "an apparently clean draft with unfinished composition cannot quit silently"
    );
    ui.invoke_request_close();
    assert_eq!(ui.get_source().as_str(), source.replace("old", "かな"));
    assert!(ui.get_dirty());
    assert!(
        ui.get_confirm_close(),
        "close prompts after preserving pending input"
    );
    ui.hide().unwrap();
}

#[test]
fn focus_mode_preserves_source_selection_and_shared_history_in_both_editors() {
    i_slint_backend_testing::init_no_event_loop();
    for formatted in [false, true] {
        let ui = Studio::new().unwrap();
        formatted_live::install(&ui);
        let source = "INT. ROOM - DAY\r\n\r\nMARA\r\nZoë e\u{301} waits.\r\n";
        ui.set_source(source.into());
        ui.set_navigation_mode(1);
        ui.set_note_query("remember".into());
        let checkpoint = Rc::new(RefCell::new(session::Checkpoint::saved(source)));
        let observed = checkpoint.clone();
        let _history = install_editor_history(&ui, move |ui| {
            reflect_checkpoint(ui, &observed.borrow());
        });
        ui.show().unwrap();
        ui.set_formatted_active(formatted);
        if formatted {
            ui.invoke_formatted_refresh(664., 500.);
        }
        let start = source.find("e\u{301}").unwrap();
        ui.invoke_select_source(start as i32, (start + "e\u{301}".len()) as i32);
        shortcut(&ui, "f", true);
        assert!(ui.get_focus_mode());
        assert_eq!(ui.get_source().as_str(), source);
        assert!(!ui.get_dirty());
        assert!(!ui.get_can_undo());
        ui.invoke_toggle_focus();
        assert!(!ui.get_focus_mode());
        assert_eq!(ui.get_navigation_mode(), 1);
        assert_eq!(ui.get_note_query().as_str(), "remember");
        text(&ui, "新しい");
        let expected = source.replace("e\u{301}", "新しい");
        assert_eq!(
            ui.get_source().as_str(),
            expected,
            "focus retains the original source selection"
        );
        ui.invoke_toggle_focus();
        ui.invoke_history_action(false);
        assert_eq!(ui.get_source().as_str(), source);
        assert!(!ui.get_dirty());
        ui.invoke_history_action(true);
        assert_eq!(ui.get_source().as_str(), expected);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Focus Copy.fountain");
        save_copy(&path, ui.get_source().as_str()).unwrap();
        assert_eq!(source_draft::read(&path).unwrap(), expected);
        for guard in 0..3 {
            ui.set_confirm_close(guard == 0);
            ui.set_file_dialog_open(guard == 1);
            ui.set_new_draft_open(guard == 2);
            ui.invoke_toggle_focus();
            assert!(ui.get_focus_mode(), "modal guards block layout commands");
            assert_eq!(ui.get_source().as_str(), expected);
        }
        ui.set_confirm_close(false);
        ui.set_file_dialog_open(false);
        ui.set_new_draft_open(false);
        ui.invoke_toggle_focus();
        ui.set_preview_open(true);
        ui.invoke_select_field(3, 0, 8);
        ui.invoke_toggle_focus();
        text(&ui, "q");
        assert!(ui.get_focus_mode());
        assert!(ui.get_preview_open());
        assert_eq!(
            ui.get_note_query().as_str(),
            "remember",
            "hidden navigator must not retain keyboard focus in Preview"
        );
        assert_eq!(ui.get_source().as_str(), expected);
        ui.hide().unwrap();
    }
}

#[test]
fn formatted_modified_navigation_keys_select_real_source_and_preserve_undo() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    install_find(&ui);
    let source = format!(
        "Café waits.\r\n\r\n{}",
        "A small sentence wraps across the manuscript. ".repeat(100)
    );
    ui.set_source(source.clone().into());
    let _history = install_editor_history(&ui, |_| {});
    ui.show().unwrap();
    ui.set_formatted_active(true);
    ui.invoke_formatted_refresh(664., 240.);
    ui.invoke_select_source(0, 0);
    if cfg!(target_os = "macos") {
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Alt.into(),
        });
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Shift.into(),
        });
        text(&ui, &slint::SharedString::from(Key::RightArrow));
        ui.window().dispatch_event(WindowEvent::KeyReleased {
            text: Key::Shift.into(),
        });
        ui.window().dispatch_event(WindowEvent::KeyReleased {
            text: Key::Alt.into(),
        });
    } else {
        shortcut(&ui, &slint::SharedString::from(Key::RightArrow), true);
    }
    text(&ui, "Moon");
    assert_eq!(ui.get_source().as_str(), source.replacen("Café", "Moon", 1));
    let directory = tempfile::tempdir().unwrap();
    let copy = directory.path().join("Word Navigation Copy.fountain");
    save_copy(&copy, ui.get_source().as_str()).unwrap();
    assert_eq!(
        source_draft::read(&copy).unwrap(),
        source.replacen("Café", "Moon", 1)
    );
    ui.invoke_history_action(false);
    assert_eq!(ui.get_source().as_str(), source);
    ui.invoke_select_source(0, 0);
    shortcut(&ui, &slint::SharedString::from(Key::End), true);
    text(&ui, "Complete replacement");
    assert_eq!(ui.get_source().as_str(), "Complete replacement");
    ui.invoke_history_action(false);
    assert_eq!(ui.get_source().as_str(), source);
    ui.invoke_select_source(0, 0);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Shift.into(),
    });
    text(&ui, &slint::SharedString::from(Key::PageDown));
    ui.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Shift.into(),
    });
    text(&ui, "Page replacement");
    assert!(ui.get_source().starts_with("Page replacement"));
    assert!(ui.get_source().len() > 100);
    assert!(ui.get_source().len() < source.len());
    ui.invoke_history_action(false);
    assert_eq!(ui.get_source().as_str(), source);
    ui.invoke_select_source(0, 0);
    ui.invoke_show_find();
    ui.invoke_formatted_navigation("document-end".into(), true);
    ui.invoke_focus_source();
    text(&ui, "!");
    assert_eq!(
        ui.get_source().as_str(),
        format!("!{source}"),
        "native navigation cannot change the manuscript selection while Find owns focus"
    );
    ui.invoke_history_action(false);
    ui.hide().unwrap();
}

#[test]
fn chooser_cards_stage_all_eight_categories_before_create_without_editing_source() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    ui.set_source("Existing clean manuscript\r\n".into());
    let choices = Rc::new(RefCell::new(Vec::new()));
    let observed = choices.clone();
    ui.on_new_category(move |choice| observed.borrow_mut().push(choice));
    ui.show().unwrap();
    ui.set_file_dialog_open(true);
    ui.set_new_draft_open(true);
    for (index, label) in [
        "Screenplay",
        "Comic Script",
        "Book",
        "Academic Paper",
        "Stage Play",
        "Audio Script",
        "General Writing",
        "Poetry & Lyrics",
    ]
    .iter()
    .enumerate()
    {
        let card = i_slint_backend_testing::ElementHandle::find_by_accessible_label(&ui, label)
            .find(|element| element.accessible_checkable() == Some(true))
            .unwrap();
        card.invoke_accessible_default_action();
        assert_eq!(ui.get_new_category_choice(), index as i32);
        assert_eq!(card.accessible_checked(), Some(true));
        assert_eq!(ui.get_source().as_str(), "Existing clean manuscript\r\n");
        assert!(
            choices.borrow().is_empty(),
            "selecting a card must not create/replace a draft"
        );
    }
    let create = i_slint_backend_testing::ElementHandle::find_by_accessible_label(&ui, "Create")
        .next()
        .unwrap();
    create.invoke_accessible_default_action();
    assert_eq!(*choices.borrow(), [7]);
    text(&ui, &slint::SharedString::from(Key::Escape));
    assert_eq!(*choices.borrow(), [7, -1]);
    assert_eq!(ui.get_source().as_str(), "Existing clean manuscript\r\n");
    ui.hide().unwrap();
}

#[test]
fn formatted_word_delete_uses_shared_undo_recovery_save_and_focus_guards() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    install_find(&ui);
    let source = "one Café\r\n\r\nMARA\r\nHello. 🌙\r\n";
    ui.set_source(source.into());
    let checkpoint = Rc::new(RefCell::new(session::Checkpoint::saved(source)));
    let observed = checkpoint.clone();
    let directory = tempfile::tempdir().unwrap();
    let recovery = Rc::new(recovery::Recovery::create(directory.path()).unwrap());
    recovery.write(source, 0).unwrap();
    let observed_recovery = recovery.clone();
    let _history = install_editor_history(&ui, move |ui| {
        reflect_checkpoint(ui, &observed.borrow());
        observed_recovery
            .write(ui.get_source().as_str(), ui.get_category())
            .unwrap();
    });
    ui.show().unwrap();
    ui.set_formatted_active(true);
    ui.invoke_formatted_refresh(664., 500.);
    let end = "one Café".len() as i32;
    ui.invoke_select_source(end, end);
    let modifier = if cfg!(target_os = "macos") {
        Key::Alt
    } else {
        Key::Control
    };
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: modifier.into(),
    });
    text(&ui, &slint::SharedString::from(Key::Backspace));
    ui.window().dispatch_event(WindowEvent::KeyReleased {
        text: modifier.into(),
    });
    let expected = source.replacen("Café", "", 1);
    assert_eq!(ui.get_source().as_str(), expected);
    assert!(ui.get_dirty());
    assert_eq!(recovery::read(recovery.path()).unwrap().source, expected);
    ui.invoke_history_action(false);
    assert_eq!(ui.get_source().as_str(), source);
    assert!(!ui.get_dirty());
    ui.invoke_history_action(true);
    assert_eq!(ui.get_source().as_str(), expected);
    let copy = directory.path().join("Word Delete Copy.fountain");
    save_copy(&copy, ui.get_source().as_str()).unwrap();
    assert_eq!(source_draft::read(&copy).unwrap(), expected);
    ui.invoke_history_action(false);
    ui.invoke_select_source(4, 4);
    ui.invoke_formatted_delete_word(true);
    assert_eq!(ui.get_source().as_str(), expected);
    ui.invoke_history_action(false);
    ui.invoke_show_find();
    ui.invoke_formatted_delete_word(false);
    assert_eq!(ui.get_source().as_str(), source);
    ui.set_find_open(false);
    ui.invoke_focus_source();
    ui.set_file_dialog_open(true);
    ui.invoke_formatted_delete_word(false);
    assert_eq!(ui.get_source().as_str(), source);
    ui.hide().unwrap();
}

slint::slint! {
    export component FocusLifecycleProbe inherits Window {
        in-out property <bool> custom-enabled: true;
        out property <bool> custom-focused: custom.has-focus;
        public function focus-custom() { custom.focus(); }
        public function focus-plain() { plain.focus(); }
        custom := FocusScope { enabled: root.custom-enabled; }
        plain := TextInput { text: "plain"; }
    }
}
#[test]
fn disabled_focus_scope_clears_focus_before_another_editor_takes_ownership() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = FocusLifecycleProbe::new().unwrap();
    ui.show().unwrap();
    ui.invoke_focus_custom();
    assert!(ui.get_custom_focused());
    ui.set_custom_enabled(false);
    ui.invoke_focus_plain();
    assert!(
        !ui.get_custom_focused(),
        "disabled FocusScope must still process FocusOut"
    );
    ui.invoke_focus_custom();
    assert!(
        !ui.get_custom_focused(),
        "disabled scopes still reject new focus"
    );
    ui.set_custom_enabled(true);
    ui.invoke_focus_custom();
    assert!(ui.get_custom_focused());
    ui.hide().unwrap();
}

#[test]
fn formatted_reentry_restores_composition_without_pointer_click_or_manual_ime_callback() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    let source = "INT. ROOM - DAY\r\n\r\nMARA\r\nA signal.\r\n";
    ui.set_source(source.into());
    let _history = install_editor_history(&ui, |_| {});
    ui.show().unwrap();
    ui.set_formatted_active(true);
    ui.invoke_formatted_refresh(664., 500.);
    i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
    for _ in 0..3 {
        ui.set_formatted_active(false);
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        assert!(!ui.get_formatted_has_focus());
        ui.set_formatted_active(true);
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        assert!(ui.get_formatted_has_focus());
        assert!(
            ui.get_formatted_ime_active(),
            "IME ownership must return on focus-in without a click"
        );
        assert!(!ui.get_formatted_status().contains("no focused item"));
        composition(&ui, "かな", "", false);
        assert!(ui.get_formatted_composing());
        assert_eq!(ui.get_source().as_str(), source);
        composition(&ui, "", "", false);
        assert!(!ui.get_formatted_composing());
        assert_eq!(ui.get_source().as_str(), source);
        ui.invoke_select_source(0, 0);
        composition(&ui, "かな", "", false);
        composition(&ui, "", "かな", true);
        assert_eq!(ui.get_source().as_str(), format!("かな{source}"));
        ui.invoke_history_action(false);
        assert_eq!(ui.get_source().as_str(), source);
    }
    ui.hide().unwrap();
}

#[test]
fn starter_checkbox_creates_real_project_and_undo_returns_to_starting_manuscript() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let checkpoint = Rc::new(RefCell::new(session::Checkpoint::saved("")));
    let observed = checkpoint.clone();
    let history = install_editor_history(&ui, move |ui| reflect_checkpoint(ui, &observed.borrow()));
    let project = Rc::new(RefCell::new(None));
    let recovery = Rc::new(RefCell::new(Ok(recovery::Recovery::create(
        directory.path(),
    )
    .unwrap())));
    let root = directory.path().to_owned();
    install_new_draft(
        &ui,
        DraftSession {
            checkpoint: checkpoint.clone(),
            history,
            project: project.clone(),
            recovery: recovery.clone(),
        },
        move || Ok(root.clone()),
        |_| {},
    );
    ui.show().unwrap();
    ui.invoke_new_draft();
    i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
    assert!(!ui.get_new_include_starter());
    let checkbox = i_slint_backend_testing::ElementHandle::find_by_accessible_label(
        &ui,
        "Include a starting outline",
    )
    .next()
    .unwrap();
    checkbox.invoke_accessible_default_action();
    assert!(ui.get_new_include_starter());
    let comic =
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(&ui, "Comic Script")
            .find(|e| e.accessible_checkable() == Some(true))
            .unwrap();
    comic.invoke_accessible_default_action();
    assert_eq!(ui.get_source().as_str(), "");
    let create = i_slint_backend_testing::ElementHandle::find_by_accessible_label(&ui, "Create")
        .next()
        .unwrap();
    create.invoke_accessible_default_action();
    i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
    let starter = ui.get_source().to_string();
    assert!(starter.contains("PAGE 1 (2 panels)"));
    assert_eq!(ui.get_category(), 1);
    assert!(ui.get_dirty());
    assert!(
        !ui.get_can_undo(),
        "starting manuscript is the new history baseline"
    );
    assert!(ui.get_outline_rows().row_count() > 0);
    assert_eq!(project.borrow().as_ref().unwrap().source(), starter);
    assert_eq!(
        recovery::read(recovery.borrow().as_ref().unwrap().path())
            .unwrap()
            .source,
        starter
    );
    ui.invoke_select_source(0, 0);
    text(&ui, "Zoë 🌙");
    assert!(
        ui.get_source().starts_with("Zoë 🌙"),
        "actual source: {:?}",
        ui.get_source()
    );
    ui.invoke_perform_history(false);
    assert_eq!(ui.get_source().as_str(), starter);
    ui.invoke_perform_history(true);
    let edited = ui.get_source().to_string();
    let path = directory.path().join("Edited.fountage");
    project_draft::save_copy(project.borrow().as_ref().unwrap(), &edited, &path).unwrap();
    assert_eq!(
        fountage_core::project::Project::read(&path)
            .unwrap()
            .source(),
        edited
    );
    checkpoint.borrow_mut().mark_saved(&edited);
    ui.invoke_new_draft();
    i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
    assert!(
        !ui.get_new_include_starter(),
        "new chooser defaults back to a blank manuscript"
    );
    ui.invoke_new_category(-1);
    assert_eq!(
        ui.get_source().as_str(),
        edited,
        "Cancel preserves the manuscript"
    );
    ui.hide().unwrap();
}

#[test]
fn academic_creation_routes_style_into_project_and_cancel_leaves_existing_metadata_alone() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let checkpoint = Rc::new(RefCell::new(session::Checkpoint::saved("")));
    let observed = checkpoint.clone();
    let history = install_editor_history(&ui, move |ui| reflect_checkpoint(ui, &observed.borrow()));
    let project = Rc::new(RefCell::new(None));
    let recovery = Rc::new(RefCell::new(Ok(recovery::Recovery::create(
        directory.path(),
    )
    .unwrap())));
    let root = directory.path().to_owned();
    install_new_draft(
        &ui,
        DraftSession {
            checkpoint: checkpoint.clone(),
            history,
            project: project.clone(),
            recovery: recovery.clone(),
        },
        move || Ok(root.clone()),
        |_| {},
    );
    ui.show().unwrap();
    ui.invoke_new_draft();
    i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
    ui.set_new_category_choice(3);
    ui.set_new_academic_style(2);
    ui.set_new_include_starter(true);
    assert_eq!(ui.get_source().as_str(), "");
    ui.invoke_new_category(3);
    i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
    assert_eq!(
        ui.get_academic_style_label().as_str(),
        "APA 7 · Professional"
    );
    assert!(ui.get_source().contains("# Introduction"));
    assert!(ui.get_dirty());
    let metadata = project.borrow().as_ref().unwrap().metadata().clone();
    let snapshot = recovery::read(recovery.borrow().as_ref().unwrap().path()).unwrap();
    let recovered =
        recovery::project_from_snapshot(recovery.borrow().as_ref().unwrap().path(), &snapshot)
            .unwrap()
            .unwrap();
    assert_eq!(recovered.metadata(), &metadata);
    let source = ui.get_source().to_string();
    checkpoint.borrow_mut().mark_saved(&source);
    ui.invoke_new_draft();
    i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
    assert_eq!(ui.get_new_academic_style(), 0);
    ui.set_new_category_choice(3);
    ui.set_new_academic_style(3);
    ui.invoke_new_category(-1);
    assert_eq!(ui.get_source().as_str(), source);
    assert_eq!(project.borrow().as_ref().unwrap().metadata(), &metadata);
    assert_eq!(
        ui.get_academic_style_label().as_str(),
        "APA 7 · Professional"
    );
    ui.hide().unwrap();
}

#[test]
fn locations_navigate_exact_repeated_scenes_with_unicode_undo_save_and_stale_guards() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    install_outline_navigation(&ui);
    let source = "🌙 An opening image.\r\n\r\nINT. CAFÉ - DAY #1#\r\n\r\nMARA\r\nHello.\r\n\r\nEXT. CAFÉ – NIGHT\r\n\r\n/*\r\nINT. HIDDEN - DAY\r\n*/\r\n\r\nINT./EXT. CAR - CONTINUOUS\r\n";
    let checkpoint = Rc::new(RefCell::new(session::Checkpoint::saved(source)));
    let observed = checkpoint.clone();
    let history = install_editor_history(&ui, move |ui| reflect_checkpoint(ui, &observed.borrow()));
    let directory = tempfile::tempdir().unwrap();
    ui.show().unwrap();
    for formatted in [false, true] {
        ui.set_source(source.into());
        ui.set_category(0);
        ui.set_outline_source(source.into());
        ui.set_outline_category(0);
        history.borrow_mut().reset(source);
        reflect_checkpoint(&ui, &checkpoint.borrow());
        let (summary, navigation, lines) = derived(source, 0);
        assert_eq!(navigation.location_count, 2);
        assert_eq!(navigation.locations.len(), 3);
        assert_eq!(navigation.locations[1].text, "CAFÉ · 2/2");
        assert_eq!(
            &source[navigation.locations[1].start..navigation.locations[1].end],
            "EXT. CAFÉ – NIGHT"
        );
        assert!(
            navigation
                .locations
                .iter()
                .all(|r| !r.text.contains("HIDDEN"))
        );
        apply(&ui, summary, navigation, lines, 0.);
        ui.set_navigation_mode(2);
        ui.set_formatted_active(formatted);
        if formatted {
            ui.invoke_formatted_refresh(664., 500.);
        }
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        ui.invoke_navigate(1);
        assert_eq!(ui.get_source().as_str(), source);
        assert!(!ui.get_dirty());
        text(&ui, "INT. MOON - DAWN");
        let changed = source.replacen("EXT. CAFÉ – NIGHT", "INT. MOON - DAWN", 1);
        assert_eq!(ui.get_source().as_str(), changed);
        ui.invoke_perform_history(false);
        assert_eq!(ui.get_source().as_str(), source);
        ui.invoke_perform_history(true);
        assert_eq!(ui.get_source().as_str(), changed);
        let path = directory
            .path()
            .join(format!("locations-{formatted}.fountain"));
        save_copy(&path, &changed).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), changed.as_bytes());
        let (reopened, category, baseline) =
            source_draft::open(&path, 0, directory.path()).unwrap();
        assert_eq!(reopened, changed);
        assert_eq!(category, 0);
        assert_eq!(recovery::read(baseline.path()).unwrap().source, changed);
        ui.invoke_select_source(0, 0);
        ui.invoke_navigate(1);
        assert!(ui.get_status().contains("Navigation is updating"));
        text(&ui, "X");
        assert_eq!(
            ui.get_source().as_str(),
            format!("X{changed}"),
            "stale locations cannot move selection"
        );
        ui.invoke_perform_history(false);
        ui.invoke_select_source(0, 0);
        for guard in [0, 1] {
            ui.set_confirm_close(guard == 0);
            ui.set_file_dialog_open(guard == 1);
            ui.invoke_navigate(0);
            assert_eq!(ui.get_source().as_str(), changed);
            ui.set_confirm_close(false);
            ui.set_file_dialog_open(false);
        }
    }
    ui.set_category(1);
    i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
    assert_eq!(
        ui.get_navigation_mode(),
        0,
        "locations mode exits outside screenplay"
    );
    ui.set_navigation_mode(2);
    ui.invoke_select_source(0, 0);
    ui.invoke_navigate(0);
    text(&ui, "Q");
    assert!(ui.get_source().starts_with('Q'));
    let (_, other, _) = derived("PAGE 1\n\nPanel 1\nA room.\n", 1);
    assert!(other.locations.is_empty());
    ui.hide().unwrap();
}

#[test]
fn navigator_keyboard_browses_without_focus_theft_and_return_activates_exact_destination() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    install_outline_navigation(&ui);
    let source = "🌙\r\n\r\nINT. CAFÉ - DAY\r\n\r\n[[First note]]\r\n\r\nINT. CAFÉ - NIGHT\r\n\r\n[[Second note]]\r\n\r\nEXT. CAR - DAY\r\n\r\n[[Third 世界 note]]\r\n";
    let checkpoint = Rc::new(RefCell::new(session::Checkpoint::saved(source)));
    let observed = checkpoint.clone();
    let history = install_editor_history(&ui, move |ui| reflect_checkpoint(ui, &observed.borrow()));
    let directory = tempfile::tempdir().unwrap();
    ui.show().unwrap();
    for formatted in [false, true] {
        for mode in 0..3 {
            ui.set_source(source.into());
            history.borrow_mut().reset(source);
            reflect_checkpoint(&ui, &checkpoint.borrow());
            ui.set_outline_source(source.into());
            ui.set_outline_category(0);
            let (summary, navigation, lines) = derived(source, 0);
            apply(&ui, summary, navigation, lines, 0.);
            ui.set_navigation_mode(mode);
            ui.set_formatted_active(formatted);
            if formatted {
                ui.invoke_formatted_refresh(664., 500.);
            }
            i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
            ui.invoke_focus_navigation();
            assert!(ui.get_navigator_has_focus());
            for (key, expected) in [
                (Key::Home, 0),
                (Key::DownArrow, 1),
                (Key::DownArrow, 2),
                (Key::UpArrow, 1),
                (Key::End, 2),
            ] {
                text(&ui, &slint::SharedString::from(key));
                assert_eq!(
                    ui.get_outline_selected(),
                    expected,
                    "mode {mode}, formatted {formatted}"
                );
                assert!(
                    ui.get_navigator_has_focus(),
                    "browsing must keep actual keyboard focus in list"
                );
                assert_eq!(ui.get_source().as_str(), source);
                assert!(!ui.get_dirty());
                assert!(!ui.get_can_undo());
            }
            text(&ui, "ordinary letters");
            assert_eq!(
                ui.get_source().as_str(),
                source,
                "typing in a navigator cannot edit the draft"
            );
            text(&ui, &slint::SharedString::from(Key::Return));
            assert!(!ui.get_navigator_has_focus());
            text(&ui, "REPLACED");
            let destination = if mode == 1 {
                "[[Third 世界 note]]"
            } else {
                "EXT. CAR - DAY"
            };
            assert_eq!(
                ui.get_source().as_str(),
                source.replacen(destination, "REPLACED", 1)
            );
            let changed = ui.get_source().to_string();
            let path = directory
                .path()
                .join(format!("keyboard-{mode}-{formatted}.fountain"));
            save_copy(&path, &changed).unwrap();
            let (reopened, category, baseline) =
                source_draft::open(&path, 0, directory.path()).unwrap();
            assert_eq!(reopened.as_bytes(), changed.as_bytes());
            assert_eq!(category, 0);
            assert_eq!(recovery::read(baseline.path()).unwrap().source, changed);
            ui.invoke_perform_history(false);
            assert_eq!(ui.get_source().as_str(), source);
            ui.invoke_focus_navigation();
            text(&ui, &slint::SharedString::from(Key::Escape));
            assert!(!ui.get_navigator_has_focus());
            assert_eq!(ui.get_source().as_str(), source);
        }
    }
    ui.set_confirm_close(true);
    ui.invoke_focus_navigation();
    assert!(!ui.get_navigator_has_focus());
    ui.set_confirm_close(false);
    ui.set_file_dialog_open(true);
    ui.invoke_focus_navigation();
    assert!(!ui.get_navigator_has_focus());
    ui.set_file_dialog_open(false);
    ui.set_focus_mode(true);
    ui.invoke_focus_navigation();
    assert!(
        !ui.get_navigator_has_focus(),
        "hidden list cannot take focus"
    );
    ui.hide().unwrap();
}

#[test]
fn navigator_tabs_remain_exclusive_on_repeated_clicks_and_programmatic_switches() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    ui.show().unwrap();
    let names = ["Outline", "Notes", "Locations"];
    for choice in [0, 0, 1, 2, 1, 1, 0, 2, 2] {
        let tab =
            i_slint_backend_testing::ElementHandle::find_by_accessible_label(&ui, names[choice])
                .find(|e| e.accessible_checkable() == Some(true))
                .unwrap();
        tab.invoke_accessible_default_action();
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        assert_eq!(ui.get_navigation_mode(), choice as i32);
        for (index, name) in names.iter().enumerate() {
            let tab = i_slint_backend_testing::ElementHandle::find_by_accessible_label(&ui, name)
                .find(|e| e.accessible_checkable() == Some(true))
                .unwrap();
            assert_eq!(
                tab.accessible_checked(),
                Some(index == choice),
                "exclusive checked state after selecting {choice}"
            );
        }
    }
    ui.set_navigation_mode(1);
    i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
    for (index, name) in names.iter().enumerate() {
        let tab = i_slint_backend_testing::ElementHandle::find_by_accessible_label(&ui, name)
            .find(|e| e.accessible_checkable() == Some(true))
            .unwrap();
        assert_eq!(tab.accessible_checked(), Some(index == 1));
    }
    ui.hide().unwrap();
}

#[test]
fn undo_restores_whole_directed_note_selection_and_redo_restores_actual_caret() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    let source = "🌙\r\n\r\nINT. CAFÉ - DAY\r\n\r\n[[Third 世界 note]]\r\n";
    let start = source.find("[[").unwrap();
    let end = source.find("]]").unwrap() + 2;
    let replacement = "[[Revised 世界 note]]";
    let expected = source.replacen("[[Third 世界 note]]", replacement, 1);
    let checkpoint = Rc::new(RefCell::new(session::Checkpoint::saved(source)));
    let observed = checkpoint.clone();
    let history = install_editor_history(&ui, move |ui| reflect_checkpoint(ui, &observed.borrow()));
    let directory = tempfile::tempdir().unwrap();
    ui.show().unwrap();
    for formatted in [false, true] {
        for reversed in [false, true] {
            ui.set_source(source.into());
            history.borrow_mut().reset(source);
            ui.set_formatted_active(formatted);
            if formatted {
                ui.invoke_formatted_refresh(664., 500.);
            }
            i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
            let (anchor, focus) = if reversed { (end, start) } else { (start, end) };
            ui.invoke_select_source(anchor as i32, focus as i32);
            text(&ui, replacement);
            assert_eq!(ui.get_source().as_str(), expected);
            ui.invoke_perform_history(false);
            assert_eq!(ui.get_source().as_str(), source);
            assert_eq!(
                (ui.get_source_anchor(), ui.get_source_focus()),
                (anchor as i32, focus as i32)
            );
            ui.invoke_perform_history(true);
            assert_eq!(ui.get_source().as_str(), expected);
            let caret = (start + replacement.len()) as i32;
            assert_eq!(
                (ui.get_source_anchor(), ui.get_source_focus()),
                (caret, caret)
            );
            let path = directory
                .path()
                .join(format!("selection-{formatted}-{reversed}.fountain"));
            save_copy(&path, ui.get_source().as_str()).unwrap();
            let (reopened, _, _) = source_draft::open(&path, 0, directory.path()).unwrap();
            assert_eq!(reopened, expected);
            ui.invoke_perform_history(false);
            text(&ui, "X");
            assert_eq!(
                ui.get_source().as_str(),
                source.replacen("[[Third 世界 note]]", "X", 1),
                "next edit replaces the whole restored selection"
            );
        }
    }
    ui.hide().unwrap();
}

#[test]
fn scene_numbering_dialog_is_atomic_scoped_undoable_and_preserves_project_metadata() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    install_scene_numbering(&ui);
    let source = "🌙\r\n\r\nINT. FIRST - DAY #A#\r\n\r\nINT. CAFÉ - DAY [[keep]]\r\n\r\nZoë waits.\r\n\r\nEXT. ROOF - NIGHT #old#\r\n\r\n/*\r\nINT. HIDDEN - DAY\r\n*/\r\n\r\nINT. LAST - DAY #Z#\r\n";
    let checkpoint = Rc::new(RefCell::new(session::Checkpoint::saved(source)));
    let observed = checkpoint.clone();
    let directory = tempfile::tempdir().unwrap();
    let recovery = Rc::new(recovery::Recovery::create(directory.path()).unwrap());
    let recovered = recovery.clone();
    let history = install_editor_history(&ui, move |ui| {
        reflect_checkpoint(ui, &observed.borrow());
        recovered.write(ui.get_source().as_str(), 0).unwrap();
    });
    ui.show().unwrap();
    for formatted in [false, true] {
        ui.set_source(source.into());
        history.borrow_mut().reset(source);
        reflect_checkpoint(&ui, &checkpoint.borrow());
        ui.set_formatted_active(formatted);
        if formatted {
            ui.invoke_formatted_refresh(664., 500.);
        }
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        let start = source.find("CAFÉ").unwrap();
        let end = source.find("#old#").unwrap() + 5;
        ui.invoke_select_source(end as i32, start as i32);
        ui.invoke_show_numbering();
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        assert!(ui.get_numbering_open() && ui.get_file_dialog_open());
        assert!(ui.get_numbering_selection_available());
        text(&ui, &slint::SharedString::from(Key::Escape));
        assert!(!ui.get_numbering_open());
        assert_eq!(
            (ui.get_source_anchor(), ui.get_source_focus()),
            (end as i32, start as i32)
        );
        ui.invoke_show_numbering();
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        ui.set_numbering_start("0".into());
        ui.invoke_numbering_action(0);
        assert!(ui.get_numbering_open());
        assert!(!ui.get_numbering_error().is_empty());
        assert_eq!(ui.get_source().as_str(), source);
        ui.set_numbering_start("42".into());
        ui.set_numbering_selected_only(true);
        ui.invoke_numbering_action(0);
        let expected = source
            .replace("DAY [[keep]]", "DAY #42# [[keep]]")
            .replace("#old#", "#43#");
        assert_eq!(ui.get_source().as_str(), expected);
        assert!(ui.get_dirty());
        assert_eq!(recovery::read(recovery.path()).unwrap().source, expected);
        ui.invoke_show_numbering();
        ui.set_numbering_start("42".into());
        ui.set_numbering_selected_only(true);
        ui.invoke_numbering_action(0);
        ui.invoke_perform_history(false);
        assert_eq!(
            ui.get_source().as_str(),
            source,
            "repeat numbering was a no-op, first numbering is one undo"
        );
        assert_eq!(
            (ui.get_source_anchor(), ui.get_source_focus()),
            (end as i32, start as i32)
        );
        ui.invoke_perform_history(true);
        assert_eq!(ui.get_source().as_str(), expected);
        let mut project =
            fountage_core::project::Project::new(fountage_core::writing::Format::Screenplay)
                .unwrap();
        project.replace_source(source).unwrap();
        let mut entries = project.entries().clone();
        let mut metadata = project.metadata().clone();
        metadata["foreign"] = serde_json::json!({"opaque":[true,3,null]});
        entries.insert(
            "info.json".into(),
            fountage_core::project::Entry::File(serde_json::to_vec(&metadata).unwrap()),
        );
        entries.insert(
            "attachment.bin".into(),
            fountage_core::project::Entry::File(vec![255, 0, 1]),
        );
        let project = fountage_core::project::Project::from_entries(entries).unwrap();
        let path = directory
            .path()
            .join(format!("Numbered-{formatted}.fountage"));
        project_draft::save_copy(&project, &expected, &path).unwrap();
        let reopened = fountage_core::project::Project::read(&path).unwrap();
        assert_eq!(reopened.source(), expected);
        assert_eq!(reopened.metadata(), &metadata);
        assert_eq!(
            reopened
                .entries()
                .get(std::ffi::OsStr::new("attachment.bin")),
            project
                .entries()
                .get(std::ffi::OsStr::new("attachment.bin"))
        );
        ui.invoke_show_numbering();
        ui.set_numbering_selected_only(true);
        ui.invoke_numbering_action(1);
        assert_eq!(ui.get_source().as_str(), source.replace(" #old#", ""));
        ui.invoke_perform_history(false);
        assert_eq!(ui.get_source().as_str(), expected);
        ui.invoke_perform_history(false);
        assert_eq!(ui.get_source().as_str(), source);
        assert!(!ui.get_dirty());
        ui.invoke_show_numbering();
        ui.set_source("external replacement".into());
        ui.invoke_numbering_action(0);
        assert!(ui.get_numbering_open());
        assert!(ui.get_numbering_error().contains("draft changed"));
        assert_eq!(ui.get_source().as_str(), "external replacement");
        ui.invoke_numbering_action(-1);
    }
    ui.set_category(1);
    ui.invoke_show_numbering();
    assert!(!ui.get_numbering_open());
    ui.hide().unwrap();
}

#[test]
fn title_page_form_preserves_custom_fields_body_undo_selection_and_project_copy() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    install_title_page(&ui);
    let source = "Title: Original\r\nAuthors: Zoë\r\n    世界\r\nCustom Field:  Opaque 🌙  \r\n\r\nINT. CAFÉ - DAY\r\n\r\nMARA\r\nHello. [[private]]\r\n";
    let body = &source[source.find("\r\n\r\nINT.").unwrap()..];
    let checkpoint = Rc::new(RefCell::new(session::Checkpoint::saved(source)));
    let observed = checkpoint.clone();
    let directory = tempfile::tempdir().unwrap();
    let recovery = Rc::new(recovery::Recovery::create(directory.path()).unwrap());
    let baseline = recovery.clone();
    let history = install_editor_history(&ui, move |ui| {
        reflect_checkpoint(ui, &observed.borrow());
        baseline.write(ui.get_source().as_str(), 0).unwrap();
    });
    ui.show().unwrap();
    for formatted in [false, true] {
        ui.set_source(source.into());
        history.borrow_mut().reset(source);
        reflect_history(&ui, &history.borrow());
        reflect_checkpoint(&ui, &checkpoint.borrow());
        ui.set_formatted_active(formatted);
        if formatted {
            ui.invoke_formatted_refresh(664., 500.);
        }
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        let start = source.find("Hello").unwrap();
        ui.invoke_select_source(start as i32, (start + 5) as i32);
        ui.invoke_show_title_page();
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        assert!(ui.get_title_page_open() && ui.get_file_dialog_open() && ui.get_title_page_ready());
        assert_eq!(ui.get_title_page_author().as_str(), "Zoë\n世界");
        ui.set_title_page_title("Cancel me".into());
        text(&ui, &slint::SharedString::from(Key::Escape));
        assert_eq!(ui.get_source().as_str(), source);
        assert!(!ui.get_title_page_open());
        assert_eq!(
            (ui.get_source_anchor(), ui.get_source_focus()),
            (start as i32, (start + 5) as i32)
        );
        ui.invoke_show_title_page();
        ui.invoke_title_page_action(true);
        assert!(!ui.get_can_undo(), "unchanged form creates no edit");
        ui.invoke_show_title_page();
        ui.set_title_page_title("A\n\nB".into());
        ui.invoke_title_page_action(true);
        assert!(ui.get_title_page_open() && !ui.get_title_page_error().is_empty());
        assert_eq!(ui.get_source().as_str(), source);
        ui.set_title_page_title("The Moon 🌙".into());
        ui.set_title_page_contact("Zoë\nwriter@example.invalid".into());
        ui.invoke_title_page_action(true);
        let updated = ui.get_source().to_string();
        assert!(updated.ends_with(body));
        assert!(updated.contains("Custom Field:  Opaque 🌙  \r\n"));
        assert!(updated.contains("Contact:\r\n    Zoë\r\n    writer@example.invalid\r\n"));
        assert_eq!(recovery::read(recovery.path()).unwrap().source, updated);
        ui.invoke_perform_history(false);
        assert_eq!(ui.get_source().as_str(), source);
        assert_eq!(
            (ui.get_source_anchor(), ui.get_source_focus()),
            (start as i32, (start + 5) as i32)
        );
        ui.invoke_perform_history(true);
        assert_eq!(ui.get_source().as_str(), updated);
        assert_eq!((ui.get_source_anchor(), ui.get_source_focus()), (0, 0));
        let project =
            fountage_core::project::Project::new(fountage_core::writing::Format::Screenplay)
                .unwrap();
        let mut entries = project.entries().clone();
        let mut metadata = project.metadata().clone();
        metadata["foreign"] = serde_json::json!({"future":42});
        entries.insert(
            "info.json".into(),
            fountage_core::project::Entry::File(serde_json::to_vec(&metadata).unwrap()),
        );
        entries.insert(
            "attachment.bin".into(),
            fountage_core::project::Entry::File(vec![0, 255]),
        );
        let project = fountage_core::project::Project::from_entries(entries).unwrap();
        let path = directory.path().join(format!("Title-{formatted}.fountage"));
        project_draft::save_copy(&project, &updated, &path).unwrap();
        let reopened = fountage_core::project::Project::read(&path).unwrap();
        assert_eq!(reopened.source(), updated);
        assert_eq!(reopened.metadata(), &metadata);
        assert_eq!(
            reopened
                .entries()
                .get(std::ffi::OsStr::new("attachment.bin")),
            project
                .entries()
                .get(std::ffi::OsStr::new("attachment.bin"))
        );
        ui.invoke_show_title_page();
        ui.set_source("external change".into());
        ui.invoke_title_page_action(true);
        assert!(ui.get_title_page_error().contains("manuscript changed"));
        assert_eq!(ui.get_source().as_str(), "external change");
        ui.invoke_title_page_action(false);
    }
    ui.set_formatted_active(false);
    ui.set_source("Title: Name\n/* hidden */\nAuthor: A\n\nINT. ROOM - DAY".into());
    ui.invoke_show_title_page();
    assert!(!ui.get_title_page_ready());
    assert!(!ui.get_title_page_error().is_empty());
    ui.invoke_title_page_action(false);
    ui.set_category(1);
    ui.invoke_show_title_page();
    assert!(!ui.get_title_page_open());
    ui.hide().unwrap();
}

#[test]
fn title_field_history_and_clipboard_are_isolated_from_manuscript_and_cancel() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    install_title_page(&ui);
    let source = "Title: Original\n\nINT. ROOM - DAY\n";
    ui.set_source(source.into());
    let edits = Rc::new(Cell::new(0));
    let observed = edits.clone();
    install_editor_history(&ui, move |_| observed.set(observed.get() + 1));
    ui.show().unwrap();
    ui.invoke_show_title_page();
    i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
    ui.invoke_select_title_field(5, 0, 0);
    text(&ui, "Zoë 世界");
    assert_eq!(ui.get_title_page_contact().as_str(), "Zoë 世界");
    ui.invoke_perform_history(false);
    assert_eq!(ui.get_title_page_contact().as_str(), "");
    ui.invoke_perform_history(true);
    assert_eq!(ui.get_title_page_contact().as_str(), "Zoë 世界");
    ui.invoke_clipboard_action(3);
    ui.invoke_clipboard_action(1);
    ui.invoke_select_title_field(0, 0, 8);
    ui.invoke_clipboard_action(2);
    assert_eq!(ui.get_title_page_title().as_str(), "Zoë 世界");
    ui.invoke_perform_history(false);
    assert_eq!(ui.get_title_page_title().as_str(), "Original");
    assert_eq!(ui.get_title_page_contact().as_str(), "Zoë 世界");
    assert_eq!(ui.get_source().as_str(), source);
    assert_eq!(edits.get(), 0);
    ui.invoke_title_page_action(false);
    assert_eq!(ui.get_source().as_str(), source);
    assert_eq!(edits.get(), 0);
    ui.invoke_show_title_page();
    assert_eq!(ui.get_title_page_contact().as_str(), "");
    ui.invoke_select_title_field(5, 0, 0);
    composition(&ui, "かな", "", false);
    assert!(ui.get_title_page_composing());
    ui.invoke_request_title_apply();
    assert!(ui.get_title_page_open());
    assert!(ui.get_title_page_error().contains("composing"));
    assert_eq!(ui.get_source().as_str(), source);
    ui.invoke_clipboard_action(2);
    assert!(
        ui.get_title_page_composing(),
        "pending text is not discarded by field paste"
    );
    composition(&ui, "", "かな", true);
    assert!(!ui.get_title_page_composing());
    assert_eq!(ui.get_title_page_contact().as_str(), "かな");
    ui.invoke_title_page_action(false);
    ui.hide().unwrap();
}

#[test]
fn live_formatted_undo_restores_wrap_affinity_and_anchor_in_shared_history() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    install_formatting(&ui);
    let source = "INT. ROOM - DAY\r\n\r\nMARA\r\nOne two three four five six seven eight nine ten eleven twelve thirteen.\r\n";
    ui.set_source(source.into());
    let checkpoint = Rc::new(RefCell::new(session::Checkpoint::saved(source)));
    let observed = checkpoint.clone();
    let history = install_editor_history(&ui, move |ui| reflect_checkpoint(ui, &observed.borrow()));
    ui.show().unwrap();
    ui.set_formatted_active(true);
    ui.invoke_formatted_refresh(364., 500.);
    i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
    for mode in 0..3 {
        ui.set_source(source.into());
        history.borrow_mut().reset(source);
        ui.invoke_formatted_refresh(364., 500.);
        let start = source.find("One").unwrap() as i32;
        ui.invoke_select_source(start, start);
        ui.invoke_formatted_navigation("line-end".into(), mode == 1);
        let edge = ui.get_source_focus();
        assert!(ui.get_formatted_focus_upstream());
        if mode == 2 {
            ui.invoke_formatted_navigation("line-start".into(), true);
        }
        let selection = (ui.get_source_anchor(), ui.get_source_focus());
        let upstream = ui.get_formatted_focus_upstream();
        let anchor_upstream = ui.get_formatted_anchor_upstream();
        ui.invoke_format_source(2);
        assert_ne!(ui.get_source().as_str(), source);
        ui.invoke_perform_history(false);
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        assert_eq!(ui.get_source().as_str(), source);
        assert_eq!((ui.get_source_anchor(), ui.get_source_focus()), selection);
        assert_eq!(ui.get_formatted_focus_upstream(), upstream);
        assert_eq!(ui.get_formatted_anchor_upstream(), anchor_upstream);
        text(&ui, "X");
        assert_ne!(ui.get_source().as_str(), source);
        ui.invoke_perform_history(false);
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        assert_eq!(ui.get_source().as_str(), source);
        assert_eq!((ui.get_source_anchor(), ui.get_source_focus()), selection);
        assert_eq!(
            ui.get_formatted_focus_upstream(),
            upstream,
            "plain fallback mirroring must not overwrite formatted affinity"
        );
        ui.invoke_perform_history(true);
        assert!(!ui.get_formatted_focus_upstream());
        ui.invoke_perform_history(false);
        if mode == 2 {
            text(&ui, &slint::SharedString::from(Key::RightArrow));
            assert_eq!(ui.get_source_focus(), edge);
            assert!(
                ui.get_formatted_focus_upstream(),
                "collapsing to restored anchor keeps its wrap side"
            );
        }
    }
    ui.hide().unwrap();
}

#[test]
fn chooser_arrow_grid_navigation_stages_without_replacing_a_manuscript() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    let original = "Existing manuscript 🌙\r\n";
    ui.set_source(original.into());
    let creations = Rc::new(RefCell::new(Vec::new()));
    let observed = creations.clone();
    ui.on_new_category(move |index| observed.borrow_mut().push(index));
    ui.show().unwrap();
    for (width, columns) in [(1280, 4), (960, 2)] {
        ui.window().set_size(slint::PhysicalSize::new(width, 860));
        ui.set_new_draft_open(false);
        ui.set_new_draft_open(true);
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        for (key, expected) in [
            (Key::RightArrow, 1),
            (Key::UpArrow, 1),
            (Key::DownArrow, 1 + columns),
            (Key::End, 7),
            (Key::RightArrow, 7),
            (Key::DownArrow, 7),
            (Key::UpArrow, 7 - columns),
            (Key::Home, 0),
            (Key::LeftArrow, 0),
        ] {
            text(&ui, &slint::SharedString::from(key));
            assert_eq!(ui.get_new_category_choice(), expected);
            assert_eq!(ui.get_source().as_str(), original);
            assert!(creations.borrow().is_empty());
        }
        text(&ui, &slint::SharedString::from(Key::RightArrow));
        text(&ui, &slint::SharedString::from(Key::Return));
        assert_eq!(*creations.borrow(), [1], "Return creates the focused card");
        creations.borrow_mut().clear();
    }
    ui.hide().unwrap();
}

#[test]
fn fountain_formatting_shortcuts_preserve_undo_recovery_and_project_bytes() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    install_formatting(&ui);
    let source = "Title: Keep\r\n\r\nINT. CAFÉ - DAY\r\n\r\nMARA\r\n  Zoë 🌙\r\nSecond line.  \r\n\r\n[[private]]\r\n";
    let start = source.find("  Zoë").unwrap();
    let end = source.find("\r\n\r\n[[").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let recovery = Rc::new(recovery::Recovery::create(directory.path()).unwrap());
    let baseline = recovery.clone();
    let history = install_editor_history(&ui, move |ui| {
        baseline.write(ui.get_source().as_str(), 0).unwrap();
    });
    ui.show().unwrap();
    for formatted in [false, true] {
        for (key, marker) in [("b", "**"), ("i", "*"), ("u", "_")] {
            ui.set_source(source.into());
            history.borrow_mut().reset(source);
            reflect_history(&ui, &history.borrow());
            ui.set_formatted_active(formatted);
            if formatted {
                ui.invoke_formatted_refresh(664., 500.);
            }
            ui.invoke_select_source(end as i32, start as i32);
            i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
            assert!(ui.get_can_format_source());
            shortcut(&ui, key, false);
            let expected = source.replace(
                "  Zoë 🌙\r\nSecond line.  ",
                &format!("  {marker}Zoë 🌙{marker}\r\n{marker}Second line.{marker}  "),
            );
            assert_eq!(ui.get_source().as_str(), expected);
            assert!(ui.get_source_anchor() > ui.get_source_focus());
            assert_eq!(recovery::read(recovery.path()).unwrap().source, expected);
            ui.invoke_perform_history(false);
            assert_eq!(ui.get_source().as_str(), source);
            assert_eq!(
                (ui.get_source_anchor(), ui.get_source_focus()),
                (end as i32, start as i32)
            );
            ui.invoke_perform_history(true);
            assert_eq!(ui.get_source().as_str(), expected);
            shortcut(&ui, key, false);
            assert_eq!(ui.get_source().as_str(), source);
            ui.invoke_perform_history(false);
            assert_eq!(ui.get_source().as_str(), expected);
            let project = fountage_core::project::Project::new(Format::Screenplay).unwrap();
            let mut metadata = project.metadata().clone();
            metadata["foreign"] = serde_json::json!({"future": [true, 42, "🌙"]});
            let mut entries = project.entries().clone();
            entries.insert(
                "info.json".into(),
                fountage_core::project::Entry::File(serde_json::to_vec(&metadata).unwrap()),
            );
            entries.insert(
                "attachment.bin".into(),
                fountage_core::project::Entry::File(vec![255, 0, 1]),
            );
            let project = fountage_core::project::Project::from_entries(entries).unwrap();
            let path = directory
                .path()
                .join(format!("Formatted-{formatted}-{key}.fountage"));
            project_draft::save_copy(&project, &expected, &path).unwrap();
            let reopened = fountage_core::project::Project::read(&path).unwrap();
            assert_eq!(reopened.source(), expected);
            assert_eq!(reopened.metadata(), &metadata);
            assert_eq!(
                reopened
                    .entries()
                    .get(std::ffi::OsStr::new("attachment.bin")),
                project
                    .entries()
                    .get(std::ffi::OsStr::new("attachment.bin"))
            );
        }
    }
    ui.hide().unwrap();
}

#[test]
fn formatting_rejects_composition_other_fields_and_modals_and_inserts_caret_pairs() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    install_formatting(&ui);
    install_find(&ui);
    let source = "Zoë 🌙\r\n";
    ui.set_source(source.into());
    let history = install_editor_history(&ui, |_| {});
    ui.show().unwrap();
    for formatted in [false, true] {
        ui.set_source(source.into());
        history.borrow_mut().reset(source);
        reflect_history(&ui, &history.borrow());
        ui.set_category(0);
        ui.set_formatted_active(formatted);
        if formatted {
            ui.invoke_formatted_refresh(664., 500.);
        }
        ui.invoke_select_source(0, 0);
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        if formatted {
            ui.invoke_formatted_preedit("かな".into(), -1, -1);
        } else {
            composition(&ui, "かな", "", false);
        }
        assert!(!ui.get_can_format_source());
        ui.invoke_format_source(0);
        assert_eq!(ui.get_source().as_str(), source);
        if formatted {
            ui.invoke_formatted_cancel();
        } else {
            composition(&ui, "", "", false);
        }
        ui.invoke_format_source(0);
        assert_eq!(ui.get_source().as_str(), format!("****{source}"));
        assert_eq!((ui.get_source_anchor(), ui.get_source_focus()), (2, 2));
        ui.invoke_perform_history(false);
        assert_eq!(ui.get_source().as_str(), source);
        assert!(!ui.get_can_undo());
        for modal in [true, false] {
            ui.set_file_dialog_open(modal);
            ui.set_preview_open(!modal);
            ui.invoke_format_source(1);
            assert_eq!(ui.get_source().as_str(), source);
        }
        ui.set_preview_open(false);
        ui.invoke_show_find();
        ui.invoke_format_source(2);
        assert_eq!(ui.get_source().as_str(), source);
        ui.set_find_open(false);
        ui.invoke_focus_source();
        ui.set_category(1);
        ui.invoke_format_source(0);
        assert_eq!(ui.get_source().as_str(), source);
    }
    ui.hide().unwrap();
}

#[test]
fn editor_mode_handoff_keeps_the_selected_source_for_formatting_and_undo() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    install_formatting(&ui);
    let source = "INT. CAFÉ - DAY\r\n\r\nMARA\r\nHello Zoë 🌙.\r\n";
    ui.set_source(source.into());
    let history = install_editor_history(&ui, |_| {});
    ui.show().unwrap();
    let start = source.find("Zoë").unwrap() as i32;
    let end = start + "Zoë 🌙".len() as i32;
    for (anchor, focus) in [(start, end), (end, start), (start, start)] {
        ui.set_formatted_active(false);
        ui.set_source(source.into());
        history.borrow_mut().reset(source);
        ui.invoke_select_source(anchor, focus);
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        assert_eq!(
            (ui.get_source_anchor(), ui.get_source_focus()),
            (anchor, focus)
        );
        ui.invoke_toggle_formatted_editor();
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        assert_eq!(
            (ui.get_source_anchor(), ui.get_source_focus()),
            (anchor, focus)
        );
        ui.invoke_format_source(0);
        let expected = if anchor == focus {
            source.replacen("Zoë", "****Zoë", 1)
        } else {
            source.replace("Zoë 🌙", "**Zoë 🌙**")
        };
        assert_eq!(ui.get_source().as_str(), expected);
        ui.invoke_perform_history(false);
        assert_eq!(ui.get_source().as_str(), source);
        assert_eq!(
            (ui.get_source_anchor(), ui.get_source_focus()),
            (anchor, focus)
        );
        ui.invoke_toggle_formatted_editor();
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        assert_eq!(
            (ui.get_source_anchor(), ui.get_source_focus()),
            (anchor, focus)
        );
    }
    ui.hide().unwrap();
}

#[test]
fn note_shortcut_updates_navigator_and_preserves_all_category_projects_and_undo() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    install_formatting(&ui);
    install_outline_navigation(&ui);
    let source = "Title: Notes QA\r\n\r\nINT. CAFÉ - DAY\r\n\r\nMARA\r\nCheck Zoë 🌙.\r\nSecond line.\r\n\r\n[[keep]]\r\n";
    let expected = source.replace(
        "Check Zoë 🌙.\r\nSecond line.",
        "[[Check Zoë 🌙.]]\r\n[[Second line.]]",
    );
    let a = source.find("Check").unwrap() as i32;
    let b = source.find("Second line.").unwrap() as i32 + "Second line.".len() as i32;
    let directory = tempfile::tempdir().unwrap();
    let recovery = Rc::new(recovery::Recovery::create(directory.path()).unwrap());
    let observed = recovery.clone();
    let history = install_editor_history(&ui, move |ui| {
        observed
            .write(ui.get_source().as_str(), ui.get_category())
            .unwrap();
        let (summary, headings, lines) = derived(ui.get_source().as_str(), ui.get_category());
        ui.set_outline_source(ui.get_source());
        ui.set_outline_category(ui.get_category());
        apply(ui, summary, headings, lines, 0.);
    });
    ui.show().unwrap();
    for category in 0..8 {
        for formatted in [false, true].into_iter().filter(|v| !v || category == 0) {
            ui.set_formatted_active(false);
            ui.set_category(category);
            ui.set_source(source.into());
            history.borrow_mut().reset(source);
            reflect_history(&ui, &history.borrow());
            ui.set_formatted_active(formatted);
            if formatted {
                ui.invoke_formatted_refresh(664., 500.);
            }
            ui.invoke_select_source(b, a);
            shortcut(&ui, "/", false);
            assert_eq!(ui.get_source().as_str(), expected);
            assert_eq!(ui.get_note_rows().row_count(), 3);
            assert!(ui.get_source_anchor() > ui.get_source_focus());
            assert_eq!(recovery::read(recovery.path()).unwrap().source, expected);
            shortcut(&ui, "/", false);
            assert_eq!(ui.get_source().as_str(), source);
            assert_eq!(ui.get_note_rows().row_count(), 1);
            ui.invoke_perform_history(false);
            assert_eq!(ui.get_source().as_str(), expected);
            ui.invoke_perform_history(false);
            assert_eq!(ui.get_source().as_str(), source);
            assert_eq!((ui.get_source_anchor(), ui.get_source_focus()), (b, a));
            ui.invoke_perform_history(true);
            let project =
                fountage_core::project::Project::new(Format::from_code(category as u32).unwrap())
                    .unwrap();
            let mut metadata = project.metadata().clone();
            metadata["foreign"] = serde_json::json!({"opaque": [true, "🌙", null]});
            let mut entries = project.entries().clone();
            entries.insert(
                "info.json".into(),
                fountage_core::project::Entry::File(serde_json::to_vec(&metadata).unwrap()),
            );
            entries.insert(
                "attachment.bin".into(),
                fountage_core::project::Entry::File(vec![255, 0, 1]),
            );
            let project = fountage_core::project::Project::from_entries(entries).unwrap();
            let path = directory
                .path()
                .join(format!("Notes-{category}-{formatted}.fountage"));
            project_draft::save_copy(&project, ui.get_source().as_str(), &path).unwrap();
            let reopened = fountage_core::project::Project::read(&path).unwrap();
            assert_eq!(reopened.source(), expected);
            assert_eq!(reopened.metadata(), &metadata);
            assert_eq!(
                reopened
                    .entries()
                    .get(std::ffi::OsStr::new("attachment.bin")),
                project
                    .entries()
                    .get(std::ffi::OsStr::new("attachment.bin"))
            );
        }
    }
    ui.hide().unwrap();
}

#[test]
fn note_caret_insertion_typing_and_composition_modal_field_guards() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    install_formatting(&ui);
    install_find(&ui);
    let history = install_editor_history(&ui, |_| {});
    ui.show().unwrap();
    for formatted in [false, true] {
        let source = "Remember.\r\n";
        ui.set_source(source.into());
        history.borrow_mut().reset(source);
        reflect_history(&ui, &history.borrow());
        ui.set_formatted_active(formatted);
        if formatted {
            ui.invoke_formatted_refresh(664., 500.);
        }
        ui.invoke_select_source(0, 0);
        if formatted {
            ui.invoke_formatted_preedit("かな".into(), -1, -1);
        } else {
            composition(&ui, "かな", "", false);
        }
        ui.invoke_note_source();
        assert_eq!(ui.get_source().as_str(), source);
        if formatted {
            ui.invoke_formatted_cancel();
        } else {
            composition(&ui, "", "", false);
        }
        ui.invoke_note_source();
        assert_eq!(ui.get_source().as_str(), format!("[[]]{source}"));
        assert_eq!((ui.get_source_anchor(), ui.get_source_focus()), (2, 2));
        text(&ui, "Zoë 🌙");
        assert_eq!(ui.get_source().as_str(), format!("[[Zoë 🌙]]{source}"));
        ui.invoke_perform_history(false);
        ui.invoke_perform_history(false);
        assert_eq!(ui.get_source().as_str(), source);
        assert!(!ui.get_can_undo());
        ui.set_file_dialog_open(true);
        ui.invoke_note_source();
        ui.set_file_dialog_open(false);
        ui.set_preview_open(true);
        ui.invoke_note_source();
        ui.set_preview_open(false);
        ui.invoke_show_find();
        ui.invoke_note_source();
        assert_eq!(ui.get_source().as_str(), source);
        ui.set_find_open(false);
        ui.invoke_focus_source();
        let unsafe_source = "[[a larger note]]";
        ui.set_source(unsafe_source.into());
        history.borrow_mut().reset(unsafe_source);
        reflect_history(&ui, &history.borrow());
        ui.invoke_select_source(4, 10);
        ui.invoke_note_source();
        assert_eq!(ui.get_source().as_str(), unsafe_source);
        assert!(ui.get_status().contains("whole note"));
        if formatted {
            assert!(
                ui.get_formatted_caption().contains("whole note"),
                "the rejection must be visible in formatted mode"
            );
        }
        assert!(!ui.get_can_undo());
    }
    ui.hide().unwrap();
}

#[test]
fn preview_retains_styles_literal_text_source_selection_undo_and_project_data() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    install_formatting(&ui);
    let source = "Title: Preview QA\r\n\r\nINT. CAFÉ - DAY\r\n\r\nMARA\r\nA **bold**, *quiet*, _underlined_ **_promise_**. [[private]]\r\nLiteral \\*stars\\*, <u>raw</u>, &amp;, [x](url), 🌙 e\u{301}.\r\n";
    ui.set_source(source.into());
    let history = install_editor_history(&ui, |ui| {
        let (summary, headings, lines) = derived(ui.get_source().as_str(), 0);
        apply(ui, summary, headings, lines, 0.);
    });
    let (summary, headings, lines) = derived(source, 0);
    apply(&ui, summary, headings, lines, 0.);
    ui.show().unwrap();
    let start = source.find("promise").unwrap() as i32;
    ui.invoke_select_source(start + 7, start);
    ui.set_preview_open(true);
    i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
    assert!(ui.get_preview_error().is_empty());
    let lines: Vec<_> = ui.get_lines().iter().collect();
    let text = lines
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!text.contains("private"));
    assert!(text.contains("A bold, quiet, underlined promise."));
    assert!(text.contains("Literal *stars*, <u>raw</u>, &amp;, [x](url), 🌙 e\u{301}."));
    for line in &lines {
        assert_eq!(
            i_slint_core::styled_text::get_raw_text(&line.styled),
            line.text.as_str()
        );
    }
    let phrase = lines.iter().find(|l| l.text.starts_with("A bold")).unwrap();
    let styles = format!("{:?}", phrase.styled);
    assert!(
        styles.contains("Strong") && styles.contains("Emphasis") && styles.contains("Underline")
    );
    assert!(
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(&ui, phrase.text.as_str())
            .next()
            .is_some()
    );
    assert_eq!(ui.get_source().as_str(), source);
    assert!(!ui.get_can_undo());
    ui.set_preview_open(false);
    ui.invoke_select_source(start + 7, start);
    ui.invoke_format_source(2);
    assert!(ui.get_can_undo());
    ui.invoke_perform_history(false);
    assert_eq!(ui.get_source().as_str(), source);
    assert_eq!(
        (ui.get_source_anchor(), ui.get_source_focus()),
        (start + 7, start)
    );
    let directory = tempfile::tempdir().unwrap();
    let project = fountage_core::project::Project::new(Format::Screenplay).unwrap();
    let mut entries = project.entries().clone();
    let mut metadata = project.metadata().clone();
    metadata["foreign"] = serde_json::json!({"opaque":true});
    entries.insert(
        "info.json".into(),
        fountage_core::project::Entry::File(serde_json::to_vec(&metadata).unwrap()),
    );
    entries.insert(
        "attachment.bin".into(),
        fountage_core::project::Entry::File(vec![255, 0, 1]),
    );
    let project = fountage_core::project::Project::from_entries(entries).unwrap();
    let path = directory.path().join("Preview.fountage");
    project_draft::save_copy(&project, ui.get_source().as_str(), &path).unwrap();
    let reopened = fountage_core::project::Project::read(&path).unwrap();
    assert_eq!(reopened.source(), source);
    assert_eq!(reopened.metadata(), &metadata);
    assert_eq!(
        reopened
            .entries()
            .get(std::ffi::OsStr::new("attachment.bin")),
        project
            .entries()
            .get(std::ffi::OsStr::new("attachment.bin"))
    );
    // Other categories keep their existing literal preview policy.
    for category in 1..8 {
        let (_, _, lines) = derived(source, category);
        for line in lines {
            assert_eq!(
                i_slint_core::styled_text::get_raw_text(
                    &preview_text::styled(&line.text, &line.runs, line.strong).unwrap()
                ),
                line.text
            );
        }
    }
    history.borrow_mut().reset(source);
    ui.hide().unwrap();
}

#[test]
fn invalid_preview_style_ranges_keep_plain_text_visible_without_touching_source() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    let source = "Original source stays intact.\r\n";
    ui.set_source(source.into());
    let (summary, headings, _) = derived(source, 0);
    apply(
        &ui,
        summary,
        headings,
        vec![PlainLine {
            text: "🌙 <u>literal</u>".into(),
            indent: 0.,
            strong: false,
            runs: vec![output::Run {
                start: 1,
                length: 1,
                style: output::Style {
                    bold: true,
                    ..Default::default()
                },
            }],
        }],
        0.,
    );
    ui.show().unwrap();
    ui.set_preview_open(true);
    i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
    assert!(ui.get_preview_error().contains("Showing plain text"));
    assert_eq!(
        i_slint_core::styled_text::get_raw_text(&ui.get_lines().row_data(0).unwrap().styled),
        "🌙 <u>literal</u>"
    );
    assert!(
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(
            &ui,
            ui.get_preview_error().as_str()
        )
        .next()
        .is_some()
    );
    assert_eq!(ui.get_source().as_str(), source);
    assert!(!ui.get_can_undo());
    ui.hide().unwrap();
}

#[test]
fn preview_toolbar_roundtrip_restores_directed_selection_and_typing_undo() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    let source = "INT. CAFÉ - DAY\r\n\r\nMARA\r\nHello Zoë 🌙 e\u{301}.\r\n";
    ui.set_source(source.into());
    let history = install_editor_history(&ui, |_| {});
    ui.show().unwrap();
    let start = source.find("Zoë").unwrap() as i32;
    let end = start + "Zoë 🌙".len() as i32;
    for formatted in [false, true] {
        if ui.get_formatted_active() != formatted {
            ui.invoke_toggle_formatted_editor();
        }
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        for (anchor, focus) in [(start, end), (end, start), (start, start)] {
            ui.set_source(source.into());
            history.borrow_mut().reset(source);
            ui.invoke_select_source(anchor, focus);
            i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
            assert_eq!(
                (ui.get_source_anchor(), ui.get_source_focus()),
                (anchor, focus),
                "before preview formatted={formatted}"
            );
            for label in ["Preview", "Edit manuscript"] {
                i_slint_backend_testing::ElementHandle::find_by_accessible_label(&ui, label)
                    .next()
                    .unwrap()
                    .invoke_accessible_default_action();
                i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
            }
            assert!(!ui.get_preview_open());
            assert_eq!(ui.get_source().as_str(), source);
            assert_eq!(
                (ui.get_source_anchor(), ui.get_source_focus()),
                (anchor, focus),
                "formatted={formatted}"
            );
            assert!(
                ui.get_can_edit_source(),
                "return must focus the actual manuscript"
            );
            text(&ui, "X");
            let mut expected = source.to_owned();
            expected.replace_range(anchor.min(focus) as usize..anchor.max(focus) as usize, "X");
            assert_eq!(ui.get_source().as_str(), expected);
            ui.invoke_perform_history(false);
            assert_eq!(ui.get_source().as_str(), source);
            assert_eq!(
                (ui.get_source_anchor(), ui.get_source_focus()),
                (anchor, focus)
            );
        }
    }
    ui.hide().unwrap();
}

#[test]
fn preview_transition_blocks_composition_modals_and_rejects_stale_selection() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    let source = "INT. ROOM - DAY\r\n\r\nZoë waits.\r\n";
    ui.set_source(source.into());
    let _history = install_editor_history(&ui, |_| {});
    ui.show().unwrap();
    for formatted in [false, true] {
        if ui.get_formatted_active() != formatted {
            ui.invoke_toggle_formatted_editor();
        }
        ui.invoke_select_source(0, 0);
        if formatted {
            ui.invoke_formatted_preedit("かな".into(), -1, -1);
        } else {
            composition(&ui, "かな", "", false);
        }
        assert!(!ui.get_can_toggle_preview());
        ui.invoke_toggle_preview();
        assert!(!ui.get_preview_open());
        assert_eq!(ui.get_source().as_str(), source);
        if formatted {
            ui.invoke_formatted_cancel();
        } else {
            composition(&ui, "", "", false);
        }
        assert!(ui.get_can_toggle_preview());
        ui.set_confirm_close(true);
        ui.invoke_toggle_preview();
        assert!(!ui.get_preview_open());
        ui.set_confirm_close(false);
        ui.set_file_dialog_open(true);
        ui.invoke_toggle_preview();
        assert!(!ui.get_preview_open());
        ui.set_file_dialog_open(false);
        ui.invoke_select_source(21, 25);
        ui.invoke_toggle_preview();
        assert!(ui.get_preview_open());
        ui.set_source("New draft".into());
        ui.invoke_toggle_preview();
        assert!(!ui.get_preview_open());
        assert!(ui.get_source_anchor() <= 9 && ui.get_source_focus() <= 9);
        assert_eq!(ui.get_source().as_str(), "New draft");
        ui.set_source(source.into());
    }
    ui.hide().unwrap();
}

#[test]
fn chooser_close_dismisses_staging_but_never_bypasses_draft_protection() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    let source = "Title: Original\r\n\r\nZoë 🌙 waits.\r\n";
    ui.set_source(source.into());
    let checkpoint = Rc::new(RefCell::new(session::Checkpoint::saved(source)));
    install_close_requests(&ui, checkpoint.clone());
    let _history = install_editor_history(&ui, |_| {});
    ui.show().unwrap();
    ui.set_file_dialog_open(true);
    ui.set_new_draft_open(true);
    ui.set_new_category_choice(7);
    ui.set_new_include_starter(true);
    assert!(
        ui.invoke_can_terminate(),
        "clean chooser must allow native Quit"
    );
    assert!(matches!(
        close_response(&ui, &checkpoint.borrow()),
        slint::CloseRequestResponse::HideWindow
    ));
    assert!(!ui.get_new_draft_open());
    assert!(!ui.get_file_dialog_open());
    assert_eq!(ui.get_source().as_str(), source);
    assert!(
        !ui.get_can_undo(),
        "staged chooser options must not edit the manuscript"
    );
    // Defensive case: source changes while chooser is showing. Closing must
    // dismiss the chooser and then review the dirty source, never bypass it.
    ui.set_file_dialog_open(true);
    ui.set_new_draft_open(true);
    let changed = format!("{source}Unsaved 世界\r\n");
    ui.set_source(changed.clone().into());
    assert!(!ui.invoke_can_terminate());
    ui.invoke_request_close();
    assert!(!ui.get_new_draft_open());
    assert!(!ui.get_file_dialog_open());
    assert!(ui.get_confirm_close());
    ui.invoke_request_close();
    assert!(ui.get_confirm_close());
    assert_eq!(ui.get_source().as_str(), changed);
    assert_eq!(*checkpoint.borrow(), session::Checkpoint::saved(source));
    // Actual file panels and other forms remain protected by the modal guard.
    ui.set_confirm_close(false);
    for form in [0, 1, 2] {
        ui.set_file_dialog_open(true);
        ui.set_title_page_open(form == 1);
        ui.set_numbering_open(form == 2);
        assert!(!ui.invoke_can_terminate());
        ui.invoke_request_close();
        assert!(!ui.get_confirm_close());
        assert!(ui.get_file_dialog_open());
        assert_eq!(ui.get_source().as_str(), changed);
        ui.set_title_page_open(false);
        ui.set_numbering_open(false);
        ui.set_file_dialog_open(false);
    }
    ui.hide().unwrap();
}

#[test]
fn task_navigator_toggle_is_undoable_and_preserves_all_category_projects() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    install_outline_navigation(&ui);
    let source = "Title: Tasks\r\n\r\nINT. ROOM - DAY\r\n\r\n  [ ] Review Zoë 🌙 [[private]]\r\n[X] Finished\r\n[[\r\n[ ] Hidden\r\n]]\r\n/*\r\n[ ] Omitted\r\n*/\r\n";
    ui.set_source(source.into());
    let refresh = |ui: &Studio| {
        let source = ui.get_source();
        let category = ui.get_category();
        ui.set_outline_source(source.clone());
        ui.set_outline_category(category);
        let (summary, navigation, lines) = derived(&source, category);
        apply(ui, summary, navigation, lines, 0.);
    };
    let recovery_root = tempfile::tempdir().unwrap();
    let recovery = Rc::new(recovery::Recovery::create(recovery_root.path()).unwrap());
    let edit_recovery = recovery.clone();
    let history = install_editor_history(&ui, move |ui| {
        edit_recovery
            .write(ui.get_source().as_str(), ui.get_category())
            .unwrap();
        refresh(ui);
    });
    ui.show().unwrap();
    for category in 0..8 {
        for formatted in [false, true].into_iter().filter(|f| !*f || category == 0) {
            ui.set_category(category);
            if ui.get_formatted_active() != formatted {
                ui.invoke_toggle_formatted_editor();
            }
            ui.set_source(source.into());
            history.borrow_mut().reset(source);
            refresh(&ui);
            ui.set_navigation_mode(3);
            if formatted {
                ui.invoke_formatted_refresh(664., 500.);
            }
            i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
            assert_eq!(ui.get_task_rows().row_count(), 2);
            assert_eq!(ui.get_task_status().as_str(), "1 of 2 completed");
            assert!(!ui.get_can_toggle_task());
            ui.invoke_browse_navigation(0);
            ui.set_outline_selected(0);
            assert!(ui.get_can_toggle_task());
            ui.invoke_toggle_task();
            let toggled = source.replacen("[ ]", "[x]", 1);
            assert_eq!(ui.get_source().as_str(), toggled);
            let snapshot = recovery::read(recovery.path()).unwrap();
            assert_eq!(snapshot.source, toggled);
            assert_eq!(snapshot.category, category);
            assert_eq!(ui.get_task_status().as_str(), "2 of 2 completed");
            ui.invoke_perform_history(false);
            assert_eq!(ui.get_source().as_str(), source);
            assert_eq!(ui.get_task_status().as_str(), "1 of 2 completed");
            ui.invoke_perform_history(true);
            assert_eq!(ui.get_source().as_str(), toggled);
            // Space on the focused navigator reopens the selected task.
            ui.invoke_focus_navigation();
            i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
            text(&ui, &slint::SharedString::from(Key::Home));
            assert!(
                ui.get_navigator_has_focus(),
                "category {category} formatted {formatted}"
            );
            assert!(
                ui.get_can_toggle_task(),
                "mode {} selected {} category {}/{} source_equal {} preview {} can_preview {}",
                ui.get_navigation_mode(),
                ui.get_outline_selected(),
                ui.get_category(),
                ui.get_outline_category(),
                ui.get_source() == ui.get_outline_source(),
                ui.get_preview_open(),
                ui.get_can_toggle_preview()
            );
            text(&ui, " ");
            assert_eq!(ui.get_source().as_str(), source);
            let directory = tempfile::tempdir().unwrap();
            let project =
                fountage_core::project::Project::new(Format::from_code(category as u32).unwrap())
                    .unwrap();
            let mut entries = project.entries().clone();
            let mut metadata = project.metadata().clone();
            metadata["foreign"] = serde_json::json!({"keep":"opaque"});
            entries.insert(
                "info.json".into(),
                fountage_core::project::Entry::File(serde_json::to_vec(&metadata).unwrap()),
            );
            entries.insert(
                "asset.bin".into(),
                fountage_core::project::Entry::File(vec![0, 255, 1]),
            );
            let project = fountage_core::project::Project::from_entries(entries).unwrap();
            let path = directory.path().join("Tasks.fountage");
            project_draft::save_copy(&project, ui.get_source().as_str(), &path).unwrap();
            let reopened = fountage_core::project::Project::read(&path).unwrap();
            assert_eq!(reopened.source(), source);
            let mut expected_project = project.clone();
            expected_project.replace_source(source).unwrap();
            assert_eq!(reopened.entries(), expected_project.entries());
            // Reject stale analysis, modal and Preview requests without edits.
            ui.set_outline_selected(0);
            ui.set_source("Different draft".into());
            ui.invoke_toggle_task();
            assert_eq!(ui.get_source().as_str(), "Different draft");
            ui.set_source(source.into());
            refresh(&ui);
            ui.set_outline_selected(0);
            ui.set_file_dialog_open(true);
            ui.invoke_toggle_task();
            assert_eq!(ui.get_source().as_str(), source);
            ui.set_file_dialog_open(false);
            ui.invoke_toggle_preview();
            ui.invoke_toggle_task();
            assert_eq!(ui.get_source().as_str(), source);
            ui.invoke_toggle_preview();
        }
    }
    ui.hide().unwrap();
}

#[test]
fn inserting_tasks_preserves_source_history_recovery_and_project_data() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    install_formatting(&ui);
    let directory = tempfile::tempdir().unwrap();
    let recovery = Rc::new(recovery::Recovery::create(directory.path()).unwrap());
    let edit_recovery = recovery.clone();
    let history = install_editor_history(&ui, move |ui| {
        edit_recovery
            .write(ui.get_source().as_str(), ui.get_category())
            .unwrap();
        let source = ui.get_source();
        let (summary, navigation, lines) = derived(&source, ui.get_category());
        apply(ui, summary, navigation, lines, 0.);
    });
    ui.show().unwrap();
    let source = "Title: Tasks\r\n\r\nBefore\r\n  Review Zoë 🌙 [[private]]\r\n\r\n\tSend draft\r\n[x] Done\r\nAfter";
    let start = source.find("  Review").unwrap() as i32;
    let end = source.find("After").unwrap() as i32;
    let expected = source
        .replace("  Review", "  [ ] Review")
        .replace("\tSend", "\t[ ] Send");
    for category in 0..8 {
        for formatted in [false, true].into_iter().filter(|f| !*f || category == 0) {
            ui.set_category(category);
            if ui.get_formatted_active() != formatted {
                ui.invoke_toggle_formatted_editor();
            }
            ui.set_source(source.into());
            history.borrow_mut().reset(source);
            if formatted {
                ui.invoke_formatted_refresh(664., 500.);
            }
            i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
            ui.invoke_select_source(end, start);
            ui.invoke_insert_tasks();
            assert_eq!(ui.get_source().as_str(), expected);
            assert!(ui.get_source_anchor() > ui.get_source_focus());
            assert_eq!(ui.get_task_rows().row_count(), 3);
            assert_eq!(recovery::read(recovery.path()).unwrap().source, expected);
            ui.invoke_insert_tasks(); // Existing checkbox states must remain unchanged.
            ui.invoke_perform_history(false);
            assert_eq!(ui.get_source().as_str(), source);
            assert_eq!(
                (ui.get_source_anchor(), ui.get_source_focus()),
                (end, start)
            );
            ui.invoke_perform_history(true);
            assert_eq!(ui.get_source().as_str(), expected);
            let mut project =
                fountage_core::project::Project::new(Format::from_code(category as u32).unwrap())
                    .unwrap();
            let mut entries = project.entries().clone();
            let mut metadata = project.metadata().clone();
            metadata["foreign"] = serde_json::json!({"keep":true});
            entries.insert(
                "info.json".into(),
                fountage_core::project::Entry::File(serde_json::to_vec(&metadata).unwrap()),
            );
            entries.insert(
                "private.bin".into(),
                fountage_core::project::Entry::File(vec![0, 255]),
            );
            project = fountage_core::project::Project::from_entries(entries).unwrap();
            let path = directory
                .path()
                .join(format!("insert-{category}-{formatted}.fountage"));
            project_draft::save_copy(&project, ui.get_source().as_str(), &path).unwrap();
            project.replace_source(&expected).unwrap();
            assert_eq!(
                fountage_core::project::Project::read(&path)
                    .unwrap()
                    .entries(),
                project.entries()
            );
            ui.set_source("".into());
            history.borrow_mut().reset("");
            ui.invoke_select_source(0, 0);
            ui.invoke_insert_tasks();
            assert_eq!(ui.get_source().as_str(), "[ ] ");
            assert_eq!((ui.get_source_anchor(), ui.get_source_focus()), (4, 4));
            text(&ui, "Review 世界");
            assert_eq!(ui.get_source().as_str(), "[ ] Review 世界");
            ui.invoke_perform_history(false);
            assert_eq!(ui.get_source().as_str(), "[ ] ");
            ui.invoke_perform_history(false);
            assert_eq!(ui.get_source().as_str(), "");
            // Hidden source must fail atomically and report the actual reason.
            ui.set_source("/*\nHidden\n*/".into());
            history.borrow_mut().reset("/*\nHidden\n*/");
            ui.invoke_select_source(3, 9);
            ui.invoke_insert_tasks();
            assert_eq!(ui.get_source().as_str(), "/*\nHidden\n*/");
            assert!(ui.get_status().contains("No text was changed"));
            if formatted {
                assert!(ui.get_formatted_status().contains("No text was changed"));
            }
        }
    }
    ui.hide().unwrap();
}

#[test]
fn task_return_continues_exits_and_undoes_in_both_editors_and_all_categories() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    install_formatting(&ui);
    let directory = tempfile::tempdir().unwrap();
    let recovery = Rc::new(recovery::Recovery::create(directory.path()).unwrap());
    let edit_recovery = recovery.clone();
    let history = install_editor_history(&ui, move |ui| {
        edit_recovery
            .write(ui.get_source().as_str(), ui.get_category())
            .unwrap();
        let (summary, navigation, lines) = derived(ui.get_source().as_str(), ui.get_category());
        apply(ui, summary, navigation, lines, 0.);
    });
    ui.show().unwrap();
    let source = "# Work\r\n\r\n\t[X] Review Zoë 🌙";
    for category in 0..8 {
        for formatted in [false, true].into_iter().filter(|f| !*f || category == 0) {
            ui.set_category(category);
            if ui.get_formatted_active() != formatted {
                ui.invoke_toggle_formatted_editor();
            }
            ui.set_source(source.into());
            history.borrow_mut().reset(source);
            if formatted {
                ui.invoke_formatted_refresh(664., 500.);
            }
            i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
            ui.invoke_select_source(source.len() as i32, source.len() as i32);
            text(&ui, &slint::SharedString::from(Key::Return));
            let continued = format!("{source}\r\n\t[ ] ");
            assert_eq!(
                ui.get_source().as_str(),
                continued,
                "category {category}, formatted {formatted}"
            );
            assert_eq!(ui.get_source_focus() as usize, continued.len());
            assert_eq!(ui.get_task_rows().row_count(), 2);
            assert_eq!(recovery::read(recovery.path()).unwrap().source, continued);
            ui.invoke_perform_history(false);
            assert_eq!(ui.get_source().as_str(), source);
            ui.invoke_perform_history(true);
            assert_eq!(ui.get_source().as_str(), continued);
            text(&ui, &slint::SharedString::from(Key::Return));
            let exited = format!("{source}\r\n\r\n");
            assert_eq!(ui.get_source().as_str(), exited);
            assert_eq!(ui.get_task_rows().row_count(), 1);
            ui.invoke_perform_history(false);
            assert_eq!(ui.get_source().as_str(), continued);
            ui.invoke_perform_history(true);
            assert_eq!(ui.get_source().as_str(), exited);
            let mut project =
                fountage_core::project::Project::new(Format::from_code(category as u32).unwrap())
                    .unwrap();
            let mut entries = project.entries().clone();
            let mut metadata = project.metadata().clone();
            metadata["foreign"] = serde_json::json!({"keep":true});
            entries.insert(
                "info.json".into(),
                fountage_core::project::Entry::File(serde_json::to_vec(&metadata).unwrap()),
            );
            entries.insert(
                "asset.bin".into(),
                fountage_core::project::Entry::File(vec![0, 255]),
            );
            project = fountage_core::project::Project::from_entries(entries).unwrap();
            let path = directory
                .path()
                .join(format!("return-{category}-{formatted}.fountage"));
            project_draft::save_copy(&project, ui.get_source().as_str(), &path).unwrap();
            project.replace_source(&exited).unwrap();
            assert_eq!(
                fountage_core::project::Project::read(&path)
                    .unwrap()
                    .entries(),
                project.entries()
            );
            // A selection, modal, Preview or pending marked text is not a smart Return.
            ui.invoke_select_source(0, 4);
            assert!(!ui.invoke_task_newline());
            ui.invoke_select_source(source.len() as i32, source.len() as i32);
            ui.set_file_dialog_open(true);
            assert!(!ui.invoke_task_newline());
            ui.set_file_dialog_open(false);
            ui.invoke_focus_source();
            if formatted {
                ui.invoke_formatted_preedit("かな".into(), -1, -1);
            } else {
                composition(&ui, "かな", "", false);
            }
            assert!(!ui.invoke_task_newline());
            if formatted {
                ui.invoke_formatted_cancel();
            } else {
                composition(&ui, "", "", false);
            }
            assert_eq!(ui.get_source().as_str(), exited);
        }
    }
    ui.hide().unwrap();
}

#[test]
fn task_removal_reviews_cancels_and_undoes_without_losing_project_data() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    install_outline_navigation(&ui);
    let directory = tempfile::tempdir().unwrap();
    let refresh = |ui: &Studio| {
        ui.set_outline_source(ui.get_source());
        ui.set_outline_category(ui.get_category());
        let (summary, navigation, lines) = derived(ui.get_source().as_str(), ui.get_category());
        apply(ui, summary, navigation, lines, 0.);
    };
    let history = install_editor_history(&ui, refresh);
    ui.show().unwrap();
    let source =
        "Title: Remove QA\r\n\r\nBefore 🌙\r\n\t[X] Review Zoë [[private]]\r\n[ ] Later\r\nAfter";
    install_close_requests(
        &ui,
        Rc::new(RefCell::new(session::Checkpoint::saved(source))),
    );
    let start = source.find("\t[X]").unwrap();
    let end = source.find("\r\n[ ] Later").unwrap();
    for category in 0..8 {
        for formatted in [false, true].into_iter().filter(|f| !*f || category == 0) {
            ui.set_category(category);
            if ui.get_formatted_active() != formatted {
                ui.invoke_toggle_formatted_editor();
            }
            ui.set_source(source.into());
            history.borrow_mut().reset(source);
            reflect_history(&ui, &history.borrow());
            refresh(&ui);
            ui.set_navigation_mode(3);
            if formatted {
                ui.invoke_formatted_refresh(664., 500.);
            }
            i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
            ui.invoke_select_source(start as i32, end as i32);
            ui.set_outline_selected(0);
            ui.invoke_request_task_removal();
            assert!(ui.get_task_removal_open() && ui.get_file_dialog_open());
            assert_eq!(ui.get_source().as_str(), source);
            assert_eq!(ui.get_task_removal_label().as_str(), "Review Zoë");
            assert!(!ui.invoke_can_terminate());
            i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
            text(&ui, &slint::SharedString::from(Key::Return)); // Cancel initially owns Return.
            assert!(!ui.get_task_removal_open() && !ui.get_file_dialog_open());
            assert_eq!(
                (ui.get_source_anchor(), ui.get_source_focus()),
                (start as i32, end as i32)
            );
            assert_eq!(ui.get_source().as_str(), source);
            assert!(!ui.get_can_undo());
            ui.invoke_request_task_removal();
            ui.invoke_task_removal_action(true);
            let expected = source.replace("\t[X] Review Zoë [[private]]\r\n", "");
            assert_eq!(ui.get_source().as_str(), expected);
            assert_eq!(ui.get_task_rows().row_count(), 1);
            ui.invoke_perform_history(false);
            assert_eq!(ui.get_source().as_str(), source);
            assert_eq!(
                (ui.get_source_anchor(), ui.get_source_focus()),
                (start as i32, end as i32)
            );
            ui.invoke_perform_history(true);
            assert_eq!(ui.get_source().as_str(), expected);
            let mut project =
                fountage_core::project::Project::new(Format::from_code(category as u32).unwrap())
                    .unwrap();
            let mut entries = project.entries().clone();
            let mut metadata = project.metadata().clone();
            metadata["foreign"] = serde_json::json!({"keep":true});
            entries.insert(
                "info.json".into(),
                fountage_core::project::Entry::File(serde_json::to_vec(&metadata).unwrap()),
            );
            entries.insert(
                "asset.bin".into(),
                fountage_core::project::Entry::File(vec![0, 255]),
            );
            project = fountage_core::project::Project::from_entries(entries).unwrap();
            let path = directory
                .path()
                .join(format!("remove-{category}-{formatted}.fountage"));
            project_draft::save_copy(&project, ui.get_source().as_str(), &path).unwrap();
            project.replace_source(&expected).unwrap();
            assert_eq!(
                fountage_core::project::Project::read(&path)
                    .unwrap()
                    .entries(),
                project.entries()
            );
            // A draft changed while review is visible cannot be removed using old offsets.
            ui.set_source(source.into());
            refresh(&ui);
            ui.set_outline_selected(0);
            ui.invoke_request_task_removal();
            assert!(ui.get_task_removal_open());
            ui.set_source("New manuscript".into());
            ui.invoke_task_removal_action(true);
            assert_eq!(ui.get_source().as_str(), "New manuscript");
            assert!(ui.get_status().contains("No task was removed"));
            assert!(!ui.get_task_removal_open() && !ui.get_file_dialog_open());
        }
    }
    ui.hide().unwrap();
}

#[test]
fn metadata_only_shelf_edits_protect_close_and_save_checkpoint_in_generated_window() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let mut project = fountage_core::project::Project::new(Format::Screenplay).unwrap();
    let source = "Title: Metadata Close QA\r\n\r\nZoë 🌙 waits.\r\n";
    project.replace_source(source).unwrap();
    let item = fountage_core::shelf::Item::new(
        fountage_core::shelf::Kind::Checklist,
        "Plan".into(),
        "[ ] Review draft\r\n".into(),
    );
    let expected = project.metadata().clone();
    project
        .edit_shelf(&expected, &fountage_core::shelf::Operation::Add(item))
        .unwrap();
    let saved = Rc::new(RefCell::new(session::Checkpoint::saved(source)));
    saved
        .borrow_mut()
        .adopt_saved(source, Some(project.metadata()));
    ui.set_project_mode(true);
    ui.set_source(source.into());
    let _source_history = install_editor_history(&ui, |_| {});
    install_close_requests(&ui, saved.clone());
    ui.show().unwrap();
    assert!(ui.invoke_can_terminate());
    let mut metadata_history = project_metadata::History::default();
    let item = fountage_core::shelf::items(project.metadata())
        .unwrap()
        .remove(0);
    let expected = project.metadata().clone();
    let recovery = metadata_history
        .edit(
            &mut project,
            source,
            &mut saved.borrow_mut(),
            &expected,
            &fountage_core::shelf::Operation::ToggleTask {
                expected: item,
                start: 0,
            },
            directory.path(),
        )
        .unwrap()
        .unwrap();
    reflect_checkpoint(&ui, &saved.borrow());
    assert!(ui.get_dirty());
    assert!(!ui.invoke_can_terminate());
    assert_eq!(ui.get_source().as_str(), source);
    assert!(
        !ui.get_can_undo(),
        "metadata history must not create a source Undo transaction"
    );
    ui.invoke_request_close();
    assert!(ui.get_confirm_close());
    assert_eq!(ui.get_source().as_str(), source);
    ui.set_confirm_close(false);
    let copy = directory.path().join("Saved Metadata.fountage");
    project_draft::save_copy(&project, source, &copy).unwrap();
    saved.borrow_mut().mark_saved(source);
    reflect_checkpoint(&ui, &saved.borrow());
    assert!(!ui.get_dirty());
    assert!(ui.invoke_can_terminate());
    metadata_history
        .undo(
            &mut project,
            source,
            &mut saved.borrow_mut(),
            directory.path(),
        )
        .unwrap();
    reflect_checkpoint(&ui, &saved.borrow());
    assert!(ui.get_dirty());
    assert!(!ui.invoke_can_terminate());
    metadata_history
        .redo(
            &mut project,
            source,
            &mut saved.borrow_mut(),
            directory.path(),
        )
        .unwrap();
    reflect_checkpoint(&ui, &saved.borrow());
    assert!(!ui.get_dirty());
    assert!(ui.invoke_can_terminate());
    let path = recovery.path().to_owned();
    drop(recovery);
    let snapshot = recovery::read(&path).unwrap();
    let recovered = recovery::project_from_snapshot(&path, &snapshot)
        .unwrap()
        .unwrap();
    assert_eq!(recovered.entries(), project.entries());
    assert_eq!(
        fountage_core::project::Project::read(&copy)
            .unwrap()
            .entries(),
        project.entries()
    );
    ui.hide().unwrap();
}

#[test]
fn layout_chooser_stages_resets_and_creates_alternate_with_retry_and_undo() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let checkpoint = Rc::new(RefCell::new(session::Checkpoint::saved("")));
    let observed = checkpoint.clone();
    let history = install_editor_history(&ui, move |ui| reflect_checkpoint(ui, &observed.borrow()));
    let project = Rc::new(RefCell::new(None));
    let recovery = Rc::new(RefCell::new(Ok(recovery::Recovery::create(
        directory.path(),
    )
    .unwrap())));
    let blocked = directory.path().join("blocked");
    std::fs::write(&blocked, "original file").unwrap();
    let fail = Rc::new(std::cell::Cell::new(true));
    let should_fail = fail.clone();
    let root = directory.path().to_owned();
    install_new_draft(
        &ui,
        DraftSession {
            checkpoint: checkpoint.clone(),
            history: history.clone(),
            project: project.clone(),
            recovery: recovery.clone(),
        },
        move || {
            Ok(if should_fail.get() {
                blocked.clone()
            } else {
                root.clone()
            })
        },
        |_| {},
    );
    ui.show().unwrap();
    ui.invoke_new_draft();
    i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
    for label in [
        "Comic Script",
        "Book",
        "Audio Script",
        "General Writing",
        "Poetry & Lyrics",
    ] {
        let card = i_slint_backend_testing::ElementHandle::find_by_accessible_label(&ui, label)
            .find(|e| e.accessible_checkable() == Some(true))
            .unwrap();
        card.invoke_accessible_default_action();
        i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
        assert_eq!(
            ui.get_new_layout_choice(),
            0,
            "changing category resets its layout"
        );
        ui.set_new_layout_choice(1);
        assert_eq!(ui.get_source().as_str(), "");
        assert!(project.borrow().is_none());
    }
    ui.set_new_include_starter(true);
    let before_recovery = recovery.borrow().as_ref().unwrap().path().to_owned();
    ui.invoke_new_category(7);
    assert!(
        ui.get_new_draft_open(),
        "failed recovery keeps options open for retry"
    );
    assert!(ui.get_file_dialog_open());
    assert_eq!(ui.get_new_layout_choice(), 1);
    assert!(project.borrow().is_none());
    assert_eq!(recovery.borrow().as_ref().unwrap().path(), before_recovery);
    assert_eq!(ui.get_source().as_str(), "");
    fail.set(false);
    ui.invoke_new_category(7);
    i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
    assert!(!ui.get_new_draft_open());
    let starter = ui.get_source().to_string();
    assert!(starter.contains("CHORUS"));
    assert_eq!(project.borrow().as_ref().unwrap().preset(), "poetry-lyrics");
    assert!(ui.get_dirty());
    assert!(
        !ui.get_can_undo(),
        "creation establishes a new source baseline"
    );
    ui.invoke_select_source(0, 0);
    text(&ui, "Zoë 🌙");
    let edited = ui.get_source().to_string();
    ui.invoke_perform_history(false);
    assert_eq!(ui.get_source().as_str(), starter);
    ui.invoke_perform_history(true);
    assert_eq!(ui.get_source().as_str(), edited);
    assert_eq!(project.borrow().as_ref().unwrap().preset(), "poetry-lyrics");
    let copy = directory.path().join("Lyrics.fountage");
    project_draft::save_copy(project.borrow().as_ref().unwrap(), &edited, &copy).unwrap();
    let reopened = fountage_core::project::Project::read(&copy).unwrap();
    assert_eq!(reopened.preset(), "poetry-lyrics");
    assert_eq!(reopened.source(), edited);
    checkpoint.borrow_mut().mark_saved(&edited);
    checkpoint
        .borrow_mut()
        .set_metadata(Some(reopened.metadata()));
    ui.invoke_new_draft();
    i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
    assert_eq!(ui.get_new_layout_choice(), 0);
    ui.set_new_category_choice(5);
    i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
    ui.set_new_layout_choice(1);
    ui.invoke_new_category(-1);
    assert_eq!(ui.get_source().as_str(), edited);
    assert_eq!(project.borrow().as_ref().unwrap().preset(), "poetry-lyrics");
    ui.hide().unwrap();
}

#[test]
fn save_request_preserves_source_selection_history_and_blocks_modal_or_preedit() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = Studio::new().unwrap();
    formatted_live::install(&ui);
    let source = "Zoë 🌙\r\nOriginal 世界\r\n";
    ui.set_source(source.into());
    let checkpoint = Rc::new(RefCell::new(session::Checkpoint::saved(source)));
    let observed = checkpoint.clone();
    install_editor_history(&ui, move |ui| reflect_checkpoint(ui, &observed.borrow()));
    let requests = Rc::new(RefCell::new(Vec::new()));
    let captured = requests.clone();
    let weak = ui.as_weak();
    ui.on_file_action(move |action| {
        dispatch_file_action(&weak.upgrade().unwrap(), action);
    });
    let weak = ui.as_weak();
    ui.on_save_copy(move || {
        let ui = weak.upgrade().unwrap();
        captured.borrow_mut().push((
            2,
            ui.get_source().to_string(),
            ui.get_source_anchor(),
            ui.get_source_focus(),
        ));
    });
    ui.show().unwrap();
    ui.invoke_select_source(0, 0);
    text(&ui, "First ");
    let first = ui.get_source().to_string();
    let anchor = ui.get_source_anchor();
    let focus = ui.get_source_focus();
    assert!(ui.get_can_save_copy());
    ui.invoke_request_save_copy();
    assert_eq!(
        requests.borrow().as_slice(),
        &[(2, first.clone(), anchor, focus)]
    );
    assert_eq!(ui.get_source().as_str(), first);
    text(&ui, "Second ");
    let second = ui.get_source().to_string();
    ui.invoke_perform_history(false);
    assert_eq!(
        ui.get_source().as_str(),
        first,
        "Save separates adjacent typing history"
    );
    ui.invoke_perform_history(true);
    assert_eq!(ui.get_source().as_str(), second);
    for modal in 0..5 {
        ui.set_file_dialog_open(modal != 0);
        ui.set_confirm_close(modal == 0);
        ui.set_new_draft_open(modal == 1);
        ui.set_shelf_open(modal == 2);
        ui.set_title_page_open(modal == 3);
        assert!(!ui.get_can_save_copy());
        ui.invoke_request_save_copy();
        dispatch_file_action(&ui, 2); // A previously queued Save rechecks the same guard.
        assert_eq!(requests.borrow().len(), 1);
        assert_eq!(ui.get_source().as_str(), second);
        ui.set_new_draft_open(false);
        ui.set_shelf_open(false);
        ui.set_title_page_open(false);
    }
    ui.set_file_dialog_open(false);
    ui.set_confirm_close(false);
    ui.invoke_focus_source();
    composition(&ui, "にほん", "", false);
    assert!(!ui.get_can_save_copy());
    ui.invoke_request_save_copy();
    dispatch_file_action(&ui, 2);
    assert_eq!(requests.borrow().len(), 1);
    assert_eq!(ui.get_source().as_str(), second);
    composition(&ui, "", "日本", true);
    assert!(ui.get_can_save_copy());
    ui.invoke_request_save_copy();
    assert_eq!(requests.borrow().len(), 2);
    assert!(requests.borrow()[1].1.contains("日本"));
    ui.set_formatted_composing(true);
    assert!(!ui.get_can_save_copy());
    ui.invoke_request_save_copy();
    assert_eq!(requests.borrow().len(), 2);
    ui.set_formatted_composing(false);
    ui.hide().unwrap();
}

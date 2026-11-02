//! Preserve original modifiers for history, clipboard and formatted-editor navigation.
//! The pinned backend can deliver that unhandled key as ordinary input without
//! its Command modifier. Use AppKit's original event, not inferred typed text.
use crate::Studio;
use block2::RcBlock;
use objc2::{MainThreadMarker, rc::Retained, runtime::AnyObject};
use objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags};
use slint::ComponentHandle;
use std::{panic::AssertUnwindSafe, ptr::NonNull};

pub struct HistoryGuard(Retained<AnyObject>);
impl Drop for HistoryGuard {
    fn drop(&mut self) {
        // SAFETY: Token came from addLocalMonitor; guard remains on UI thread.
        unsafe { NSEvent::removeMonitor(&self.0) };
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Action {
    SaveCopy,
    History(bool),
    ShelfFocus(bool),
    TitleApply,
    Format(i32),
    Note,
    Clipboard(i32),
    Navigation(&'static str, bool),
    DeleteWord(bool),
}
fn key_action(text: &str, flags: NSEventModifierFlags) -> Option<Action> {
    let shift = flags.contains(NSEventModifierFlags::Shift);
    let command = flags.contains(NSEventModifierFlags::Command)
        && !flags.intersects(NSEventModifierFlags::Control | NSEventModifierFlags::Option);
    let option = flags.contains(NSEventModifierFlags::Option)
        && !flags.intersects(NSEventModifierFlags::Control | NSEventModifierFlags::Command);
    let plain = !flags.intersects(
        NSEventModifierFlags::Control
            | NSEventModifierFlags::Option
            | NSEventModifierFlags::Command,
    );
    if plain && (text == "\t" || text == "\u{19}") {
        return Some(Action::ShelfFocus(shift || text == "\u{19}"));
    }
    if option && !shift {
        match text {
            "\u{8}" | "\u{7f}" => return Some(Action::DeleteWord(false)),
            "\u{f728}" => return Some(Action::DeleteWord(true)),
            _ => {}
        }
    }
    let navigation = match text {
        "\u{f702}" if command => Some("line-start"),
        "\u{f703}" if command => Some("line-end"),
        "\u{f700}" if command => Some("document-start"),
        "\u{f701}" if command => Some("document-end"),
        "\u{f729}" if command => Some("document-start"),
        "\u{f72b}" if command => Some("document-end"),
        "\u{f702}" if option => Some("word-left"),
        "\u{f703}" if option => Some("word-right"),
        "\u{f72c}" if plain => Some("page-up"),
        "\u{f72d}" if plain => Some("page-down"),
        _ => None,
    };
    if let Some(direction) = navigation {
        return Some(Action::Navigation(direction, shift));
    }
    if !flags.contains(NSEventModifierFlags::Command)
        || flags.intersects(NSEventModifierFlags::Control | NSEventModifierFlags::Option)
    {
        return None;
    }
    // Both the normal Save chord and the previous Save Copy chord open the
    // existing exclusive-copy workflow. Never infer this from typed "s".
    if text.eq_ignore_ascii_case("s") {
        return Some(Action::SaveCopy);
    }
    if matches!(text, "\r" | "\n") && !shift {
        return Some(Action::TitleApply);
    }
    if text.eq_ignore_ascii_case("z") {
        return Some(Action::History(flags.contains(NSEventModifierFlags::Shift)));
    }
    if flags.contains(NSEventModifierFlags::Shift) {
        return None;
    }
    match text.to_ascii_lowercase().as_str() {
        "/" => Some(Action::Note),
        "b" => Some(Action::Format(0)),
        "i" => Some(Action::Format(1)),
        "u" => Some(Action::Format(2)),
        "x" => Some(Action::Clipboard(0)),
        "c" => Some(Action::Clipboard(1)),
        "v" => Some(Action::Clipboard(2)),
        "a" => Some(Action::Clipboard(3)),
        _ => None,
    }
}

fn is_backend_view(view: &AnyObject) -> bool {
    // AccessKit subclasses WinitView at runtime. Accept its subclasses too,
    // while excluding AppKit text fields and native Open/Save panel contents.
    let mut class = Some(view.class());
    while let Some(current) = class {
        if current.name() == c"WinitView" {
            return true;
        }
        class = current.superclass();
    }
    false
}

pub fn install(ui: &Studio) -> Result<HistoryGuard, String> {
    let weak = ui.as_weak();
    let handler = RcBlock::new(move |pointer: NonNull<NSEvent>| -> *mut NSEvent {
        std::panic::catch_unwind(AssertUnwindSafe(|| {
            // SAFETY: AppKit supplies a live event for this synchronous call.
            let event = unsafe { pointer.as_ref() };
            let Some(action) = event
                .charactersIgnoringModifiers()
                .and_then(|text| key_action(&text.to_string(), event.modifierFlags()))
            else {
                return pointer.as_ptr();
            };
            let Some(mtm) = MainThreadMarker::new() else {
                return pointer.as_ptr();
            };
            let Some(ui) = weak.upgrade() else {
                return pointer.as_ptr();
            };
            if matches!(action, Action::SaveCopy) {
                // Native Open/Save panels keep their own responder/shortcuts.
                // Consume disabled Save chords in our content (including forms)
                // so a lost Command modifier can never insert s into a field.
                if event
                    .window(mtm)
                    .and_then(|window| window.contentView())
                    .is_none_or(|view| !is_backend_view(&view))
                {
                    return pointer.as_ptr();
                }
                if ui.get_can_save_copy()
                    && let Err(error) = weak.upgrade_in_event_loop(|ui| {
                        ui.invoke_request_save_copy();
                    })
                {
                    eprintln!("Save shortcut could not be queued: {error}");
                }
                return std::ptr::null_mut();
            }
            if matches!(action, Action::ShelfFocus(_)) && !ui.get_shelf_open() {
                return pointer.as_ptr();
            }
            // Format/Note callbacks recheck editor ownership when dispatched.
            // Consume these recognized chords in the backend view even when
            // disabled, so lost Command modifiers cannot type into search fields.
            if matches!(action, Action::TitleApply)
                && !ui.get_title_page_input_focused()
                && !ui.get_shelf_draft_open()
                && !ui.get_shelf_quick_focused()
            {
                return pointer.as_ptr();
            }
            // Route only the opt-in surface or a focused title field; native
            // picker responders and the ordinary source path remain untouched.
            if matches!(action, Action::Clipboard(_))
                && !ui.get_formatted_active()
                && !ui.get_title_page_input_focused()
                && !ui.get_shelf_draft_open()
                && !ui.get_shelf_query_focused()
                && !ui.get_shelf_quick_focused()
            {
                return pointer.as_ptr();
            }
            if matches!(action, Action::Navigation(..) | Action::DeleteWord(_))
                && (ui.get_file_dialog_open()
                    || !ui.get_formatted_active()
                    || !ui.get_formatted_has_focus()
                    || ui.get_formatted_composing())
            {
                return pointer.as_ptr();
            }
            // Do not steal native picker field Undo or keys from other windows.
            // The class check is scoped to the pinned backend's content view.
            let shelf_history = ui.get_shelf_open()
                && (matches!(
                    action,
                    Action::History(_) | Action::ShelfFocus(_) | Action::Format(_) | Action::Note
                ) || ((ui.get_shelf_draft_open() || ui.get_shelf_quick_focused())
                    && matches!(action, Action::TitleApply))
                    || ((ui.get_shelf_draft_open()
                        || ui.get_shelf_query_focused()
                        || ui.get_shelf_quick_focused())
                        && matches!(action, Action::Clipboard(_))));
            if (ui.get_file_dialog_open() && !ui.get_title_page_input_focused() && !shelf_history)
                || ui.get_confirm_close()
                || event
                    .window(mtm)
                    .and_then(|window| window.contentView())
                    .is_none_or(|view| !is_backend_view(&view))
            {
                return pointer.as_ptr();
            }
            // Admit all intercepted Shelf commands without UI writes here.
            // Its controller drains in order before close/save/focus boundaries.
            if ui.get_shelf_open() {
                let admitted = match action {
                    Action::History(redo) => Some(ui.invoke_shelf_queue_history(redo)),
                    Action::ShelfFocus(backward) => Some(ui.invoke_shelf_queue_focus(backward)),
                    Action::TitleApply => Some(ui.invoke_shelf_queue_apply()),
                    Action::Clipboard(kind) => Some(ui.invoke_shelf_queue_clipboard(kind)),
                    // Formatting belongs to the manuscript, not a plain Shelf
                    // form. Consume disabled chords rather than losing Command
                    // and accidentally inserting their letters into a field.
                    Action::Format(_) | Action::Note => Some(false),
                    _ => None,
                };
                if let Some(admitted) = admitted {
                    if admitted
                        && let Err(error) = weak.upgrade_in_event_loop(|ui| {
                            ui.invoke_shelf_flush_history();
                        })
                    {
                        eprintln!("Shelf commands could not be dispatched yet: {error}");
                    }
                    return std::ptr::null_mut();
                }
            }
            // Defer model/menu mutations until outside native sendEvent.
            // Disabled history is still consumed, but perform-history is a no-op.
            let shelf_owner = (ui.get_shelf_open(), ui.get_shelf_session());
            let shelf_field_owner = ui.get_shelf_draft_field_index();
            let title_owner = ui
                .get_title_page_input_focused()
                .then(|| ui.get_title_field_index());
            if let Err(error) = weak.upgrade_in_event_loop(move |ui| {
                // Never retarget a queued title-field operation to a different
                // field or to the manuscript after the form closes.
                if shelf_owner != (ui.get_shelf_open(), ui.get_shelf_session())
                    || shelf_field_owner != ui.get_shelf_draft_field_index()
                    || title_owner
                        != ui
                            .get_title_page_input_focused()
                            .then(|| ui.get_title_field_index())
                {
                    return;
                }
                match action {
                    Action::SaveCopy => ui.invoke_request_save_copy(),
                    Action::ShelfFocus(backward) => ui.invoke_shelf_focus_move(backward),
                    Action::TitleApply => {
                        if ui.get_shelf_draft_open() {
                            ui.invoke_shelf_draft_action(true);
                        } else {
                            ui.invoke_request_title_apply();
                        }
                    }
                    Action::Format(style) => ui.invoke_format_source(style),
                    Action::Note => ui.invoke_note_source(),
                    Action::History(redo) => ui.invoke_perform_history(redo),
                    Action::Clipboard(kind) => ui.invoke_clipboard_action(kind),
                    Action::Navigation(direction, extend) => {
                        ui.invoke_formatted_navigation(direction.into(), extend)
                    }
                    Action::DeleteWord(forward) => ui.invoke_formatted_delete_word(forward),
                }
            }) {
                eprintln!("Editor shortcut could not be queued: {error}");
            }
            std::ptr::null_mut()
        }))
        .unwrap_or(pointer.as_ptr()) // Never unwind into AppKit.
    });
    // SAFETY: Handler returns either the original live event or null. AppKit
    // copies the block; the returned token is removed before UI thread exits.
    unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &handler) }
        .map(HistoryGuard)
        .ok_or_else(|| "Could not install native history shortcut protection.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn save_accepts_command_and_previous_copy_chord_without_stealing_other_modifiers() {
        let command = NSEventModifierFlags::Command;
        for key in ["s", "S"] {
            for extra in [
                NSEventModifierFlags::empty(),
                NSEventModifierFlags::Shift,
                NSEventModifierFlags::CapsLock,
            ] {
                assert_eq!(key_action(key, command | extra), Some(Action::SaveCopy));
            }
            for flags in [
                NSEventModifierFlags::empty(),
                NSEventModifierFlags::Shift,
                NSEventModifierFlags::Control,
                NSEventModifierFlags::Option,
                command | NSEventModifierFlags::Control,
                command | NSEventModifierFlags::Option,
            ] {
                assert_eq!(key_action(key, flags), None);
            }
        }
    }
    #[test]
    fn native_shelf_tab_preserves_shift_and_ignores_other_modifiers() {
        assert_eq!(
            key_action("\t", NSEventModifierFlags::empty()),
            Some(Action::ShelfFocus(false))
        );
        assert_eq!(
            key_action("\t", NSEventModifierFlags::Shift),
            Some(Action::ShelfFocus(true))
        );
        assert_eq!(
            key_action("\u{19}", NSEventModifierFlags::empty()),
            Some(Action::ShelfFocus(true))
        );
        assert_eq!(key_action("\t", NSEventModifierFlags::Command), None);
        assert_eq!(key_action("\t", NSEventModifierFlags::Control), None);
        assert_eq!(key_action("\t", NSEventModifierFlags::Option), None);
    }
    #[test]
    fn native_word_delete_requires_option_and_does_not_capture_dead_keys_or_command_delete() {
        let option = NSEventModifierFlags::Option;
        for key in ["\u{8}", "\u{7f}"] {
            assert_eq!(key_action(key, option), Some(Action::DeleteWord(false)));
            assert_eq!(key_action(key, NSEventModifierFlags::empty()), None);
            assert_eq!(
                key_action(key, option | NSEventModifierFlags::Control),
                None
            );
            assert_eq!(key_action(key, NSEventModifierFlags::Command), None);
        }
        assert_eq!(
            key_action("\u{f728}", option),
            Some(Action::DeleteWord(true))
        );
        assert_eq!(key_action("é", option), None);
    }
    #[test]
    fn native_navigation_preserves_option_command_and_shift_without_catching_control_chords() {
        let cmd = NSEventModifierFlags::Command;
        let alt = NSEventModifierFlags::Option;
        let shift = NSEventModifierFlags::Shift;
        for (text, flags, direction) in [
            ("\u{f702}", cmd, "line-start"),
            ("\u{f703}", cmd, "line-end"),
            ("\u{f700}", cmd, "document-start"),
            ("\u{f701}", cmd, "document-end"),
            ("\u{f702}", alt, "word-left"),
            ("\u{f703}", alt, "word-right"),
            ("\u{f72c}", NSEventModifierFlags::empty(), "page-up"),
            ("\u{f72d}", NSEventModifierFlags::empty(), "page-down"),
        ] {
            assert_eq!(
                key_action(text, flags),
                Some(Action::Navigation(direction, false))
            );
            assert_eq!(
                key_action(text, flags | shift),
                Some(Action::Navigation(direction, true))
            );
            assert_eq!(
                key_action(text, flags | NSEventModifierFlags::Control),
                None
            );
        }
        assert_eq!(key_action("\u{f702}", NSEventModifierFlags::empty()), None);
        assert_eq!(key_action("é", alt), None);
    }
    #[test]
    fn formatting_shortcuts_require_unmodified_command_chords() {
        assert_eq!(
            key_action("/", NSEventModifierFlags::Command),
            Some(Action::Note)
        );
        for flags in [
            NSEventModifierFlags::empty(),
            NSEventModifierFlags::Control,
            NSEventModifierFlags::Command | NSEventModifierFlags::Shift,
            NSEventModifierFlags::Command | NSEventModifierFlags::Option,
        ] {
            assert_eq!(key_action("/", flags), None);
        }
        for (key, code) in [("b", 0), ("I", 1), ("u", 2)] {
            assert_eq!(
                key_action(key, NSEventModifierFlags::Command),
                Some(Action::Format(code))
            );
            for flags in [
                NSEventModifierFlags::empty(),
                NSEventModifierFlags::Control,
                NSEventModifierFlags::Command | NSEventModifierFlags::Shift,
                NSEventModifierFlags::Command | NSEventModifierFlags::Option,
            ] {
                assert_eq!(key_action(key, flags), None);
            }
        }
    }
    #[test]
    fn only_command_history_keys_are_consumed() {
        let command = NSEventModifierFlags::Command;
        assert_eq!(key_action("z", command), Some(Action::History(false)));
        assert_eq!(
            key_action("Z", command | NSEventModifierFlags::CapsLock),
            Some(Action::History(false))
        );
        assert_eq!(
            key_action("Z", command | NSEventModifierFlags::Shift),
            Some(Action::History(true))
        );
        for flags in [
            NSEventModifierFlags::empty(),
            NSEventModifierFlags::Shift,
            NSEventModifierFlags::Control,
            command | NSEventModifierFlags::Option,
            command | NSEventModifierFlags::Control,
        ] {
            assert_eq!(key_action("z", flags), None);
        }
        for text in ["", "é", "zz", "日本語"] {
            assert_eq!(key_action(text, command), None);
        }
    }
    #[test]
    fn clipboard_shortcuts_require_command_and_preserve_native_variants() {
        let cmd = NSEventModifierFlags::Command;
        for (text, kind) in [("x", 0), ("C", 1), ("v", 2), ("a", 3)] {
            assert_eq!(key_action(text, cmd), Some(Action::Clipboard(kind)));
            for extra in [
                NSEventModifierFlags::Shift,
                NSEventModifierFlags::Option,
                NSEventModifierFlags::Control,
            ] {
                assert_eq!(key_action(text, cmd | extra), None);
            }
            assert_eq!(key_action(text, NSEventModifierFlags::empty()), None);
        }
    }
}

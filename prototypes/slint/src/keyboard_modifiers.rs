//! Reconcile native aggregate modifier changes for the pinned Winit backend.
//! Slint 1.18.1 handles individual modifier key events but not ModifiersChanged.
//! The aggregate is authoritative after focus changes and synthesized key chords.
use slint::platform::{Key, WindowEvent};
use slint::winit_030::{EventResult, WinitWindowAccessor, winit};
use winit::keyboard::ModifiersState;

fn reconcile(window: &slint::Window, modifiers: ModifiersState) {
    // Match Slint's macOS Command -> Control and Control -> Meta mapping.
    // Collapse left/right state only at an aggregate update. Release both sides
    // when inactive so a right-hand key released outside the window cannot stick.
    for (key, pressed) in [
        (Key::Shift, modifiers.shift_key()),
        (Key::ShiftR, false),
        (Key::Control, modifiers.super_key()),
        (Key::ControlR, false),
        (Key::Meta, modifiers.control_key()),
        (Key::MetaR, false),
        (Key::Alt, modifiers.alt_key()),
        (Key::AltGr, false),
    ] {
        window.dispatch_event(if pressed {
            WindowEvent::KeyPressed { text: key.into() }
        } else {
            WindowEvent::KeyReleased { text: key.into() }
        });
    }
}

pub fn install(ui: &crate::Studio) {
    use slint::ComponentHandle;
    ui.window().on_winit_window_event(|window, event| {
        if let winit::event::WindowEvent::ModifiersChanged(modifiers) = event {
            reconcile(window, modifiers.state());
        } else if matches!(event, winit::event::WindowEvent::Focused(false)) {
            reconcile(window, ModifiersState::empty());
        }
        EventResult::Propagate
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Studio;
    use slint::ComponentHandle;

    fn key(ui: &Studio, key: Key) {
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: key.into() });
        ui.window()
            .dispatch_event(WindowEvent::KeyReleased { text: key.into() });
    }
    #[test]
    fn aggregate_shift_extends_a_reverse_unicode_selection_and_release_stops_extending() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = Studio::new().unwrap();
        ui.set_source("Zoë 🌙 abc".into());
        ui.show().unwrap();
        ui.invoke_select_source(13, 13);
        // No individual Shift press: this is the missing aggregate-only path.
        reconcile(ui.window(), ModifiersState::SHIFT);
        for _ in 0..3 {
            key(&ui, Key::LeftArrow);
        }
        assert_eq!((ui.get_source_anchor(), ui.get_source_focus()), (13, 10));
        reconcile(ui.window(), ModifiersState::empty());
        key(&ui, Key::LeftArrow);
        assert_eq!(ui.get_source_anchor(), ui.get_source_focus());
        assert_eq!(ui.get_source(), "Zoë 🌙 abc");
        ui.hide().unwrap();
    }
}

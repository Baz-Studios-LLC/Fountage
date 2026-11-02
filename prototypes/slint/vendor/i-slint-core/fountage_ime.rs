// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
// Fountage-specific, feature-gated extension to pinned Slint 1.18.1.
//! Custom-editor input-method transport. This is not upstream Slint API.
use crate::{
    SharedString,
    api::{LogicalPosition, LogicalSize, Window},
    input::{InternalKeyEvent, KeyEventType},
    item_tree::ItemWeak,
    window::{InputMethodProperties, InputMethodRequest, WindowInner},
};
use alloc::{rc::Rc, string::String};
use core::{
    cell::{Cell, RefCell},
    ops::Range,
};

/// An unnormalized native composition event. All offsets are UTF8 bytes.
#[derive(Clone, Debug)]
pub struct CompositionEvent {
    /// True for a native commit event, false for a preedit update.
    pub commit_event: bool,
    /// Text that the native input method committed.
    pub text: SharedString,
    /// Text not yet committed to the draft.
    pub preedit: SharedString,
    /// Selection within preedit in UTF8 bytes.
    pub selection: Option<Range<i32>>,
    /// Replacement relative to the currently reported committed cursor.
    pub replacement: Option<Range<i32>>,
    /// Optional absolute cursor position in resulting committed source.
    pub cursor: Option<i32>,
    /// Optional absolute anchor position in resulting committed source.
    pub anchor: Option<i32>,
}
/// Surrounding text and candidate geometry for a custom editor.
#[derive(Clone, Default, Debug)]
pub struct Properties {
    /// Committed source, excluding preedit.
    pub text: SharedString,
    /// Cursor UTF8 byte in committed source.
    pub cursor: usize,
    /// Anchor UTF8 byte, if selected.
    pub anchor: Option<usize>,
    /// Current preedit, not part of committed source.
    pub preedit: SharedString,
    /// Insertion point for preedit within committed source.
    pub preedit_offset: usize,
    /// Candidate cursor origin in logical window coordinates.
    pub origin: LogicalPosition,
    /// Candidate cursor size in logical window coordinates.
    pub size: LogicalSize,
}
#[derive(Default)]
pub(crate) struct State {
    handler: RefCell<Option<Rc<dyn Fn(CompositionEvent) -> bool>>>,
    owner: RefCell<Option<ItemWeak>>,
    enabled: Cell<bool>,
}
impl State {
    pub(crate) fn dispatch(&self, window: &WindowInner, event: &InternalKeyEvent) -> bool {
        if !self.enabled.get()
            || !matches!(
                event.event_type,
                KeyEventType::UpdateComposition | KeyEventType::CommitComposition
            )
        {
            return false;
        }
        // A focus change may precede the application callback that disables IME.
        // Never steal composition from a different widget, especially TextInput.
        if self.owner.borrow().as_ref() != Some(&*window.focus_item.borrow()) {
            return false;
        }
        let handler = self.handler.borrow().clone();
        handler.is_some_and(|handler| {
            handler(CompositionEvent {
                commit_event: event.event_type == KeyEventType::CommitComposition,
                text: event.key_event.text.clone(),
                preedit: event.preedit_text.clone(),
                selection: event.preedit_selection.clone(),
                replacement: event.replacement_range.clone(),
                cursor: event.cursor_position,
                anchor: event.anchor_position,
            })
        })
    }
}
impl Window {
    /// Install the feature-gated Fountage composition handler. Disabled by default.
    pub fn on_fountage_composition(&self, handler: impl Fn(CompositionEvent) -> bool + 'static) {
        *WindowInner::from_pub(self)
            .fountage_ime
            .handler
            .borrow_mut() = Some(Rc::new(handler));
    }
    /// Enable/update custom-editor IME or disable it without disturbing TextInput.
    /// Returns an error without changing native state for invalid byte/geometry values.
    pub fn set_fountage_ime(&self, properties: Option<Properties>) -> Result<(), String> {
        let window = WindowInner::from_pub(self);
        let state = &window.fountage_ime;
        let Some(properties) = properties else {
            if state.enabled.replace(false) {
                *state.owner.borrow_mut() = None;
                let native_text = window
                    .focus_item
                    .borrow()
                    .upgrade()
                    .is_some_and(|item| item.downcast::<crate::items::TextInput>().is_some());
                if !native_text {
                    window.set_text_input_focused(false); // Also sends backend Disable.
                }
            }
            return Ok(());
        };
        if !properties.text.is_char_boundary(properties.cursor)
            || properties
                .anchor
                .is_some_and(|p| !properties.text.is_char_boundary(p))
            || !properties.text.is_char_boundary(properties.preedit_offset)
            || !properties.origin.x.is_finite()
            || !properties.origin.y.is_finite()
            || !properties.size.width.is_finite()
            || !properties.size.height.is_finite()
            || properties.size.width <= 0.
            || properties.size.height <= 0.
        {
            return Err("Invalid custom-editor input-method properties".into());
        }
        let focus = window.focus_item.borrow().clone();
        let Some(item) = focus.upgrade() else {
            return Err("Custom editor has no focused item".into());
        };
        if item.downcast::<crate::items::TextInput>().is_some() {
            return Err("Custom IME cannot own a standard TextInput".into());
        }
        let mut props = InputMethodProperties::default();
        props.text = properties.text;
        props.cursor_position = properties.cursor;
        props.anchor_position = properties.anchor;
        props.preedit_text = properties.preedit;
        props.preedit_offset = properties.preedit_offset;
        props.cursor_rect_origin = properties.origin;
        props.cursor_rect_size = properties.size;
        props.anchor_point = properties.origin;
        let enabled = state.enabled.replace(true);
        *state.owner.borrow_mut() = Some(focus);
        *window.last_ime_text.borrow_mut() = props.text.clone();
        window.set_text_input_focused(true);
        if let Some(adapter) = window.window_adapter().internal(crate::InternalToken) {
            adapter.input_method_request(if enabled {
                InputMethodRequest::Update(props)
            } else {
                InputMethodRequest::Enable(props)
            });
        }
        Ok(())
    }
}

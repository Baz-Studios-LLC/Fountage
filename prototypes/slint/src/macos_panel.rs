//! Native responder actions for hosted AppKit file dialogs. Slint rebuilds its
//! native menu during modal entry; observe those rebuilds without changing UI models.
use block2::RcBlock;
use objc2::{
    MainThreadMarker,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject, Sel},
    sel,
};
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObjectProtocol, NSString};
use std::{cell::RefCell, collections::BTreeMap, ptr::NonNull};

struct ItemState {
    item: Retained<NSMenuItem>,
    target: Option<Retained<AnyObject>>,
    action: Option<Sel>,
    enabled: bool,
    native_action: Sel,
    key: Retained<NSString>,
    modifiers: NSEventModifierFlags,
}
#[derive(Default)]
struct State {
    items: BTreeMap<usize, ItemState>,
    menus: BTreeMap<usize, (Retained<NSMenu>, bool)>,
}
thread_local! { static ACTIVE: RefCell<Option<State>> = const { RefCell::new(None) }; }

fn patch() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Some(main) = NSApplication::sharedApplication(mtm).mainMenu() else {
        return;
    };
    ACTIVE.with(|active| {
        // Our own menu mutations also post notifications. Ignore synchronous reentry.
        let Ok(mut slot) = active.try_borrow_mut() else {
            return;
        };
        let Some(state) = slot.as_mut() else {
            return;
        };
        for parent in main.itemArray() {
            if parent.title().to_string() != "Edit" {
                continue;
            }
            let Some(menu) = parent.submenu() else {
                continue;
            };
            state
                .menus
                .entry(Retained::as_ptr(&menu) as usize)
                .or_insert_with(|| (menu.clone(), menu.autoenablesItems()));
            menu.setAutoenablesItems(true);
            for item in menu.itemArray() {
                let (selector, key, shift) = match item.title().to_string().as_str() {
                    "Undo" => (sel!(undo:), "z", false),
                    "Redo" => (sel!(redo:), "z", true),
                    "Cut" => (sel!(cut:), "x", false),
                    "Copy" => (sel!(copy:), "c", false),
                    "Paste" => (sel!(paste:), "v", false),
                    "Select All" => (sel!(selectAll:), "a", false),
                    _ => continue,
                };
                let modifiers = if shift {
                    NSEventModifierFlags::Command | NSEventModifierFlags::Shift
                } else {
                    NSEventModifierFlags::Command
                };
                if item.action() == Some(selector)
                    && item.target().is_none()
                    && item.keyEquivalent().to_string() == key
                    && item.keyEquivalentModifierMask() == modifiers
                {
                    continue;
                }
                state
                    .items
                    .entry(Retained::as_ptr(&item) as usize)
                    .or_insert_with(|| ItemState {
                        item: item.clone(),
                        target: item.target(),
                        action: item.action(),
                        enabled: item.isEnabled(),
                        native_action: selector,
                        key: item.keyEquivalent(),
                        modifiers: item.keyEquivalentModifierMask(),
                    });
                // SAFETY: Standard one-object AppKit action signatures. Nil target
                // routes to native first responder, including remote panel fields.
                unsafe {
                    item.setTarget(None);
                    item.setAction(Some(selector));
                }
                item.setKeyEquivalent(&NSString::from_str(key));
                item.setKeyEquivalentModifierMask(modifiers);
            }
            menu.update();
        }
    });
}

pub struct PanelMenu {
    tokens: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
    active: bool,
}
impl PanelMenu {
    pub fn install() -> Self {
        let active = ACTIVE.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_some() {
                false
            } else {
                *slot = Some(State::default());
                true
            }
        });
        let mut guard = Self {
            tokens: Vec::new(),
            active,
        };
        if !active {
            return guard;
        }
        let center = NSNotificationCenter::defaultCenter();
        for name in [
            "NSMenuDidAddItemNotification",
            "NSMenuDidChangeItemNotification",
            "NSMenuDidRemoveItemNotification",
        ] {
            let block = RcBlock::new(|_: NonNull<NSNotification>| {
                // Never unwind into Objective-C. Native menus are main-thread-only;
                // the capture-free block safely ignores any off-thread notification.
                let _ = std::panic::catch_unwind(patch);
            });
            // SAFETY: No object filter; synchronous delivery, capture-free sendable
            // block. Tokens are removed before restoring any menu state.
            guard.tokens.push(unsafe {
                center.addObserverForName_object_queue_usingBlock(
                    Some(&NSString::from_str(name)),
                    None,
                    None,
                    &block,
                )
            });
        }
        patch();
        guard
    }
    pub fn restore(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        let center = NSNotificationCenter::defaultCenter();
        for token in self.tokens.drain(..) {
            // SAFETY: Every token was registered with this notification center.
            unsafe {
                center.removeObserver((*token).as_ref());
            }
        }
        let state = ACTIVE.with(|slot| slot.borrow_mut().take());
        if let Some(state) = state {
            for saved in state.items.into_values() {
                if saved.item.action() == Some(saved.native_action) && saved.item.target().is_none()
                {
                    // SAFETY: Restore retained original target/action pair while
                    // it is alive, before releasing this saved state.
                    unsafe {
                        saved.item.setTarget(saved.target.as_deref());
                        saved.item.setAction(saved.action);
                    }
                    saved.item.setKeyEquivalent(&saved.key);
                    saved.item.setKeyEquivalentModifierMask(saved.modifiers);
                    saved.item.setEnabled(saved.enabled);
                }
            }
            for (menu, auto) in state.menus.into_values() {
                menu.setAutoenablesItems(auto);
            }
        }
    }
}
impl Drop for PanelMenu {
    fn drop(&mut self) {
        self.restore();
    }
}

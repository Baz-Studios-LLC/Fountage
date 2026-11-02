//! Bridge AppKit Quit to the same single-window close policy as the title-bar button.
//! Scoped to the pinned winit delegate instance; no replacement delegate or backend patch.
use crate::Studio;
use objc2::{
    class, msg_send,
    runtime::{AnyObject, ClassBuilder, Sel},
    sel,
};
use slint::ComponentHandle;
use std::cell::RefCell;

thread_local! {
    static TARGET: RefCell<Option<slint::Weak<Studio>>> = const { RefCell::new(None) };
}
pub struct QuitGuard;
impl Drop for QuitGuard {
    fn drop(&mut self) {
        TARGET.with(|target| *target.borrow_mut() = None);
    }
}
// NSApplicationTerminateReply is NSUInteger. Clean drafts use native termination.
// Dirty requests are cancelled then reviewed in the ordinary Slint loop; accepting
// the close ends this single-window process normally. No nested AppKit modal loop.
extern "C" fn should_terminate(_: &AnyObject, _: Sel, _: &AnyObject) -> usize {
    std::panic::catch_unwind(|| {
        let weak = TARGET.with(|target| target.borrow().clone());
        let Some(weak) = weak else {
            return 0;
        };
        if weak.upgrade().is_some_and(|ui| ui.invoke_can_terminate()) {
            return 1; // NSTerminateNow
        }
        if let Err(error) = weak.upgrade_in_event_loop(|ui| ui.invoke_request_close()) {
            eprintln!("Quit review could not be queued; application kept open: {error}");
        }
        0 // NSTerminateCancel
    })
    .unwrap_or(0) // Keep open on failure; never unwind into Objective-C.
}

pub fn install(ui: &Studio) -> Result<QuitGuard, String> {
    // SAFETY: Called on the UI/main thread after Slint has created its native
    // window. NSApplication owns its delegate for the application lifetime.
    let delegate = unsafe {
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let app = app.as_ref().ok_or("NSApplication is unavailable")?;
        let delegate: *mut AnyObject = msg_send![app, delegate];
        delegate
            .as_ref()
            .ok_or("Application delegate is unavailable")?
    };
    let superclass = delegate.class();
    if superclass.name() != c"WinitApplicationDelegate" {
        return Err("Quit protection requires the pinned winit application delegate.".into());
    }
    if superclass
        .instance_method(sel!(applicationShouldTerminate:))
        .is_some()
    {
        return Err(
            "The backend already has a termination policy; refusing to override it.".into(),
        );
    }
    let mut builder = ClassBuilder::new(c"FountageSlintQuitDelegate", superclass)
        .ok_or("Quit delegate subclass already exists")?;
    // SAFETY: Exactly implements AppKit's optional applicationShouldTerminate:
    // signature. No ivars or existing methods are added/replaced.
    unsafe {
        builder.add_method(
            sel!(applicationShouldTerminate:),
            should_terminate as extern "C" fn(_, _, _) -> usize,
        );
    }
    let subclass = builder.register();
    if superclass.instance_size() != subclass.instance_size() {
        return Err("Quit delegate layout differs from the backend delegate.".into());
    }
    // SAFETY: This main-thread-only instance has not been exposed to concurrent
    // class mutation. The new class directly subclasses its exact current class,
    // adds no ivars, and inherits every existing implementation unchanged.
    let previous = unsafe { AnyObject::set_class(delegate, subclass) };
    assert_eq!(
        previous, superclass,
        "application delegate class changed during installation"
    );
    TARGET.with(|target| *target.borrow_mut() = Some(ui.as_weak()));
    Ok(QuitGuard)
}

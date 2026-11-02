//! Unified macOS header using the same native window attributes as TuplePad.
//! Keep decorations enabled: AppKit owns close/minimize/full-screen and the
//! existing Slint close request still protects unsaved drafts.
use crate::Studio;
#[cfg(target_os = "macos")]
use slint::ComponentHandle;

pub fn select_backend() -> Result<(), slint::PlatformError> {
    #[cfg(target_os = "macos")]
    {
        use slint::winit_030::winit::platform::macos::WindowAttributesExtMacOS;
        slint::BackendSelector::new()
            .backend_name("winit".into())
            .with_winit_window_attributes_hook(|attributes| {
                attributes
                    .with_titlebar_transparent(true)
                    .with_fullsize_content_view(true)
                    .with_title_hidden(true)
            })
            .select()?;
    }
    Ok(())
}

pub fn install(ui: &Studio) {
    #[cfg(target_os = "macos")]
    {
        use slint::winit_030::WinitWindowAccessor;
        ui.set_integrated_titlebar(true);
        let weak = ui.as_weak();
        ui.on_drag_window(move || {
            if let Some(ui) = weak.upgrade() {
                ui.window().with_winit_window(|window| {
                    let _ = window.drag_window();
                });
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = ui;
}

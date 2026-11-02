pub mod document;
mod emphasis;
pub mod formatting;
mod fountain;
pub mod numbering;
pub mod output;
pub mod project;
mod project_copy;
pub mod script_notes;
pub mod shelf;
pub mod storage;
pub mod title_page;
pub mod todo;
pub mod writing;
use std::ffi::{CStr, CString, c_char};

/// Builds a UTF-8 JSON outline. The returned allocation belongs to Rust.
///
/// # Safety
/// `source` must be null or point to a valid NUL-terminated UTF-8 string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fountage_outline(source: *const c_char) -> *mut c_char {
    if source.is_null() {
        return std::ptr::null_mut();
    }
    std::panic::catch_unwind(|| {
        // SAFETY: the caller guarantees the lifetime and termination of source.
        let source = unsafe { CStr::from_ptr(source) }.to_str().ok()?;
        let json = serde_json::to_string(&document::outline(source)).ok()?;
        Some(CString::new(json).ok()?.into_raw())
    })
    .ok()
    .flatten()
    .unwrap_or(std::ptr::null_mut())
}

/// Builds format-specific editing data (0: screenplay, 1: comic, 2: novel).
///
/// # Safety
/// `source` must be null or a valid NUL-terminated UTF-8 string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fountage_analyze(source: *const c_char, format: u32) -> *mut c_char {
    if source.is_null() {
        return std::ptr::null_mut();
    }
    std::panic::catch_unwind(|| {
        let format = writing::Format::from_code(format)?;
        let source = unsafe { CStr::from_ptr(source) }.to_str().ok()?;
        let json = serde_json::to_string(&writing::analyze(source, format)).ok()?;
        Some(CString::new(json).ok()?.into_raw())
    })
    .ok()
    .flatten()
    .unwrap_or(std::ptr::null_mut())
}

/// Produces semantic screenplay output shared by preview and export.
///
/// # Safety
/// `source` must be null or a valid NUL-terminated UTF-8 string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fountage_screenplay_output(source: *const c_char) -> *mut c_char {
    if source.is_null() {
        return std::ptr::null_mut();
    }
    std::panic::catch_unwind(|| {
        // SAFETY: the caller guarantees a valid, terminated input allocation.
        let source = unsafe { CStr::from_ptr(source) }.to_str().ok()?;
        let json = serde_json::to_string(&output::screenplay(source)).ok()?;
        Some(CString::new(json).ok()?.into_raw())
    })
    .ok()
    .flatten()
    .unwrap_or(std::ptr::null_mut())
}

/// Produces editing indexes and semantic output from one shared screenplay parse.
///
/// # Safety
/// `source` must be null or a valid NUL-terminated UTF-8 string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fountage_screenplay_snapshot(source: *const c_char) -> *mut c_char {
    if source.is_null() {
        return std::ptr::null_mut();
    }
    std::panic::catch_unwind(|| {
        // SAFETY: the caller guarantees a valid, terminated input allocation.
        let source = unsafe { CStr::from_ptr(source) }.to_str().ok()?;
        let json = serde_json::to_string(&output::snapshot(source)).ok()?;
        Some(CString::new(json).ok()?.into_raw())
    })
    .ok()
    .flatten()
    .unwrap_or(std::ptr::null_mut())
}

/// Computes one atomic numbering edit; caller frees the JSON result.
///
/// # Safety
/// `source` must be null or a valid NUL-terminated UTF-8 string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fountage_scene_numbers(
    source: *const c_char,
    start: u32,
    remove: u32,
) -> *mut c_char {
    if source.is_null() {
        return std::ptr::null_mut();
    }
    std::panic::catch_unwind(|| {
        let source = unsafe { CStr::from_ptr(source) }.to_str().ok()?;
        let json = serde_json::to_string(&numbering::response(source, start, remove != 0)).ok()?;
        Some(CString::new(json).ok()?.into_raw())
    })
    .ok()
    .flatten()
    .unwrap_or(std::ptr::null_mut())
}

/// Numbers headings intersecting a nonempty UTF-16 range, preserving full-document parse context.
/// # Safety
/// `source` must be null or a valid NUL-terminated UTF-8 string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fountage_scene_numbers_range(
    source: *const c_char,
    start: u32,
    remove: u32,
    location: usize,
    length: usize,
) -> *mut c_char {
    if source.is_null() {
        return std::ptr::null_mut();
    }
    std::panic::catch_unwind(|| {
        let source = unsafe { CStr::from_ptr(source) }.to_str().ok()?;
        let result = location
            .checked_add(length)
            .ok_or("The selection is invalid. No text was changed.")
            .and_then(|end| numbering::edit_range(source, start, remove != 0, Some(location..end)));
        let response = match result {
            Ok(change) => numbering::Response {
                change,
                error: None,
            },
            Err(error) => numbering::Response {
                change: None,
                error: Some(error),
            },
        };
        Some(
            CString::new(serde_json::to_string(&response).ok()?)
                .ok()?
                .into_raw(),
        )
    })
    .ok()
    .flatten()
    .unwrap_or(std::ptr::null_mut())
}

/// Releases an outline result exactly once.
///
/// # Safety
/// `value` must be null or an unfreed allocation returned by a Fountage JSON function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fountage_string_free(value: *mut c_char) {
    if !value.is_null() {
        drop(unsafe { CString::from_raw(value) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn screenplay_snapshot_bridge_matches_existing_outputs_and_rejects_invalid_input() {
        unsafe {
            assert!(fountage_screenplay_snapshot(std::ptr::null()).is_null());
            let invalid = [0xffu8, 0];
            assert!(fountage_screenplay_snapshot(invalid.as_ptr().cast()).is_null());
            for source in [
                "",
                "Title: Zoë\r\n\r\n# Act One\r\n\r\nINT. CAFÉ - DAY #2# [[private]]\r\n\r\n@Zoë\r\n**A signal** 🌙.\r\n\r\n/*hidden*/",
                "INT. ROOM - DAY\n\nMARA\nHello.\n\nVOICE ^\nWorld.\n\n===\n\n= Synopsis",
            ] {
                let input = CString::new(source).unwrap();
                let pointer = fountage_screenplay_snapshot(input.as_ptr());
                assert!(!pointer.is_null());
                let snapshot: serde_json::Value =
                    serde_json::from_slice(CStr::from_ptr(pointer).to_bytes()).unwrap();
                fountage_string_free(pointer);
                assert_eq!(
                    snapshot["analysis"],
                    serde_json::to_value(writing::analyze(source, writing::Format::Screenplay))
                        .unwrap()
                );
                assert_eq!(
                    snapshot["output"],
                    serde_json::to_value(output::screenplay(source)).unwrap()
                );
            }
        }
    }
    #[test]
    fn bridge_round_trip_and_null() {
        unsafe {
            assert!(fountage_outline(std::ptr::null()).is_null());
            fountage_string_free(std::ptr::null_mut());
            let input = CString::new("EXT. CAFÉ - DAY\n\n@Zoë\nHello.").unwrap();
            let output = fountage_outline(input.as_ptr());
            assert!(!output.is_null());
            let json: serde_json::Value =
                serde_json::from_slice(CStr::from_ptr(output).to_bytes()).unwrap();
            assert_eq!(json["characters"]["Zoë"], 1);
            assert_eq!(json["scenes"][0]["heading"], "EXT. CAFÉ - DAY");
            fountage_string_free(output);
        }
    }
}

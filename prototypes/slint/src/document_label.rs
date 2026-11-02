//! Presentation labels only: never change the document source or project metadata.
use std::path::Path;

pub fn from_path(path: &Path) -> String {
    let name = if matches!(
        path.extension().and_then(|v| v.to_str()),
        Some("fountage" | "fountain" | "txt" | "md")
    ) {
        path.file_stem()
    } else {
        path.file_name()
    };
    let name: String = name
        .map(|name| {
            name.to_string_lossy()
                .chars()
                .map(|ch| if ch.is_control() { ' ' } else { ch })
                .collect()
        })
        .unwrap_or_default();
    if name.trim().is_empty() {
        "Scratch draft".into()
    } else {
        name.trim().into()
    }
}
pub fn copy_filename(label: &str, project: bool, screenplay: bool) -> String {
    let extension = if project {
        "fountage"
    } else if screenplay {
        "fountain"
    } else {
        "txt"
    };
    format!("{label} Copy.{extension}")
}

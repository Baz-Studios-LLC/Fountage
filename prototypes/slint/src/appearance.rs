//! Session-independent appearance preferences, separate from manuscripts.
use serde_json::value::RawValue;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
type Fields = BTreeMap<String, Box<RawValue>>;

#[derive(Debug, PartialEq, Eq)]
pub struct Appearance {
    pub dark: bool,
    pub white_paper: bool,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            dark: false,
            white_paper: true,
        }
    }
}
pub fn path() -> Result<PathBuf, String> {
    Ok(dirs::config_dir()
        .ok_or("Application settings directory is unavailable.")?
        .join("Fountage Slint Lab")
        .join("appearance.json"))
}
fn fields(path: &Path) -> Result<Fields, String> {
    match std::fs::metadata(path) {
        Ok(info) if info.len() > 1024 * 1024 => {
            return Err(
                "Appearance settings exceed the supported size; file left unchanged.".into(),
            );
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(BTreeMap::from([(
                "version".into(),
                RawValue::from_string("1".into()).unwrap(),
            )]));
        }
        Err(error) => return Err(error.to_string()),
        _ => (),
    }
    let fields: Fields = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let version = fields
        .get("version")
        .ok_or("Appearance settings have no schema version.")?;
    if serde_json::from_str::<u32>(version.get()).ok() != Some(1) {
        return Err("Unsupported appearance settings version; file left unchanged.".into());
    }
    // Validate both known values before any write, retaining opaque extra fields.
    preferences(&fields)?;
    Ok(fields)
}
fn preferences(fields: &Fields) -> Result<Appearance, String> {
    let flag = |key: &str, fallback| -> Result<bool, String> {
        fields.get(key).map_or(Ok(fallback), |value| {
            serde_json::from_str(value.get())
                .map_err(|_| format!("Invalid appearance value: {key}. File left unchanged."))
        })
    };
    Ok(Appearance {
        dark: flag("dark", false)?,
        white_paper: flag("whitePaper", true)?,
    })
}
pub fn load(path: &Path) -> Result<Appearance, String> {
    preferences(&fields(path)?)
}
pub fn save(path: &Path, value: &Appearance) -> Result<(), String> {
    if std::fs::symlink_metadata(path).is_ok_and(|info| info.file_type().is_symlink()) {
        return Err("Appearance settings are a symbolic link; file left unchanged.".into());
    }
    // Re-read on each explicit change so externally added unknown fields survive.
    let mut fields = fields(path)?;
    fields.insert(
        "dark".into(),
        serde_json::value::to_raw_value(&value.dark).map_err(|e| e.to_string())?,
    );
    fields.insert(
        "whitePaper".into(),
        serde_json::value::to_raw_value(&value.white_paper).map_err(|e| e.to_string())?,
    );
    let json = serde_json::to_string_pretty(&fields).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(path.parent().ok_or("Invalid settings path.")?)
        .map_err(|e| e.to_string())?;
    fountage_core::storage::save(path, &json)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_reopen_and_preserve_external_opaque_fields_without_touching_other_files() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("settings/appearance.json");
        assert_eq!(load(&path).unwrap(), Appearance::default());
        assert!(!path.exists());
        let chosen = Appearance {
            dark: true,
            white_paper: false,
        };
        save(&path, &chosen).unwrap();
        assert_eq!(load(&path).unwrap(), chosen);
        let json = r#"{"version":1,"dark":true,"whitePaper":false,"future":{"number":123456789012345678901234567890,"label":"世界 🌙"}}"#;
        std::fs::write(&path, json).unwrap();
        let opaque: Fields = serde_json::from_str(json).unwrap();
        let manuscript = root.path().join("draft.fountain");
        std::fs::write(&manuscript, "Café\r\n世界").unwrap();
        save(&path, &Appearance::default()).unwrap();
        assert_eq!(
            fields(&path).unwrap()["future"].get(),
            opaque["future"].get()
        );
        assert_eq!(load(&path).unwrap(), Appearance::default());
        assert_eq!(std::fs::read_to_string(manuscript).unwrap(), "Café\r\n世界");
    }
    #[test]
    fn invalid_future_and_failed_settings_never_overwrite_the_original() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("appearance.json");
        for json in [
            "{bad",
            r#"{"version":2,"dark":true}"#,
            r#"{"version":1,"dark":"wrong"}"#,
        ] {
            std::fs::write(&path, json).unwrap();
            assert!(load(&path).is_err());
            assert!(save(&path, &Appearance::default()).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), json);
        }
        let blocked = path.join("appearance.json");
        assert!(save(&blocked, &Appearance::default()).is_err());
    }
}

//! Source-preserving snapshots of existing directory-based .fountage packages.
//! Exclusive new-copy publication is separate from replacement saves, migrations and conflict handling.
use crate::writing::Format;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

pub const NAMESPACE: &str = "com.bazstudios.fountage";
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    File(Vec<u8>),
    Directory(BTreeMap<OsString, Entry>),
    SymbolicLink(PathBuf),
}
#[derive(Debug, Clone)]
pub struct Project {
    entries: BTreeMap<OsString, Entry>,
    metadata: Value,
    source: String,
    source_filename: &'static str,
    format: Format,
    preset: String,
    project_id: String,
    schema: u64,
}
/// A requested academic standard, not an assertion of rendered compliance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AcademicStyle {
    Apa7Student,
    Apa7Professional,
    Mla9,
}
impl AcademicStyle {
    pub fn from_choice(choice: i32) -> Result<Option<Self>, String> {
        match choice {
            0 => Ok(None),
            1 => Ok(Some(Self::Apa7Student)),
            2 => Ok(Some(Self::Apa7Professional)),
            3 => Ok(Some(Self::Mla9)),
            _ => Err("Invalid academic style selection.".into()),
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Apa7Student => "APA 7 · Student",
            Self::Apa7Professional => "APA 7 · Professional",
            Self::Mla9 => "MLA 9",
        }
    }
    fn fields(self) -> Value {
        match self {
            Self::Apa7Student => {
                serde_json::json!({"family":"apa","edition":7,"variant":"student"})
            }
            Self::Apa7Professional => {
                serde_json::json!({"family":"apa","edition":7,"variant":"professional"})
            }
            Self::Mla9 => serde_json::json!({"family":"mla","edition":9,"variant":""}),
        }
    }
}
fn presets(name: &str) -> Option<(Format, &'static [&'static str])> {
    let format = match name {
        "screenplay" => Format::Screenplay,
        "comic" => Format::Comic,
        "novel" => Format::Novel,
        "academic" => Format::Academic,
        "stageplay" => Format::Stageplay,
        "audio" => Format::Audio,
        "general" => Format::General,
        "poetry" => Format::Poetry,
        _ => return None,
    };
    Some((format, format.layout_presets()))
}
fn valid_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}
fn regular<'a>(entries: &'a BTreeMap<OsString, Entry>, name: &str) -> Result<&'a [u8], String> {
    match entries.get(OsStr::new(name)) {
        Some(Entry::File(data)) => Ok(data),
        _ => Err(format!("Missing or non-regular project entry: {name}")),
    }
}
fn decode_source(bytes: &[u8]) -> Result<&str, String> {
    let source = std::str::from_utf8(bytes).map_err(|_| "Project manuscript is not UTF-8.")?;
    if source.contains('\0') {
        return Err("Project manuscript contains NUL.".into());
    }
    Ok(source)
}
impl Project {
    /// A new schema-2 package with its own identity and empty manuscript.
    /// Existing projects never pass through this constructor or lose metadata.
    pub fn new(format: Format) -> Result<Self, String> {
        Self::new_with_academic_style(format, None)
    }
    /// Creates a fresh package. Existing metadata never passes through this constructor.
    pub fn new_with_academic_style(
        format: Format,
        style: Option<AcademicStyle>,
    ) -> Result<Self, String> {
        Self::new_with_layout(format, format.layout_presets()[0], style)
    }
    /// Creates a new package with a validated category-specific preset.
    /// Never rewrites an existing project's metadata.
    pub fn new_with_layout(
        format: Format,
        preset: &str,
        style: Option<AcademicStyle>,
    ) -> Result<Self, String> {
        if !format.layout_presets().contains(&preset) {
            return Err("Unsupported layout preset for writing category.".into());
        }
        if style.is_some() && format != Format::Academic {
            return Err("Academic style requires an academic document.".into());
        }
        let name = match format {
            Format::Screenplay => "screenplay",
            Format::Comic => "comic",
            Format::Novel => "novel",
            Format::Academic => "academic",
            Format::Stageplay => "stageplay",
            Format::Audio => "audio",
            Format::General => "general",
            Format::Poetry => "poetry",
        };
        let screenplay = format == Format::Screenplay;
        let source = if screenplay {
            "text.fountain"
        } else {
            "text.txt"
        };
        let timestamp = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .map_err(|e| e.to_string())?;
        let mut metadata = serde_json::json!({
            "version": 2, "type": if screenplay { "com.fountain.screenplay" } else { "public.plain-text" },
            "creatorIdentifier": NAMESPACE,
            NAMESPACE: { "version": 2, "documentFormat": name, "layoutPreset": preset,
                "sourceFile": source, "projectID": uuid::Uuid::new_v4().to_string(),
                "createdAt": timestamp, "modifiedAt": timestamp,
                "authors": [], "settings": {}, "characters": {}, "notes": [],
                "revisions": [], "snippets": [], "goals": {} }
        });
        if let Some(style) = style {
            metadata[NAMESPACE]["academicStyle"] = style.fields();
        }
        Self::from_entries(BTreeMap::from([
            (
                "info.json".into(),
                Entry::File(serde_json::to_vec_pretty(&metadata).map_err(|e| e.to_string())?),
            ),
            (source.into(), Entry::File(Vec::new())),
            ("assets".into(), Entry::Directory(BTreeMap::new())),
        ]))
    }
    pub fn from_entries(entries: BTreeMap<OsString, Entry>) -> Result<Self, String> {
        let metadata: Value =
            serde_json::from_slice(regular(&entries, "info.json")?).map_err(|e| e.to_string())?;
        let own = metadata
            .get(NAMESPACE)
            .and_then(Value::as_object)
            .ok_or("Missing project metadata namespace.")?;
        let schema = own
            .get("version")
            .and_then(Value::as_u64)
            .filter(|v| [1, 2].contains(v))
            .ok_or("Unsupported project schema.")?;
        let (format, preset) = if schema == 1 {
            (Format::Screenplay, "screenplay-standard".to_owned())
        } else {
            let name = own
                .get("documentFormat")
                .and_then(Value::as_str)
                .ok_or("Missing writing category.")?;
            let (format, choices) = presets(name).ok_or("Unsupported writing category.")?;
            let preset = own
                .get("layoutPreset")
                .and_then(Value::as_str)
                .filter(|p| choices.contains(p))
                .ok_or("Unsupported layout preset for writing category.")?;
            (format, preset.to_owned())
        };
        let screenplay = format == Format::Screenplay;
        let source_filename = if screenplay {
            "text.fountain"
        } else {
            "text.txt"
        };
        if schema == 2 && own.get("sourceFile").and_then(Value::as_str) != Some(source_filename) {
            return Err("Invalid manuscript path.".into());
        }
        if metadata.get("version").and_then(Value::as_u64) != Some(2)
            || metadata.get("type").and_then(Value::as_str)
                != Some(if screenplay {
                    "com.fountain.screenplay"
                } else {
                    "public.plain-text"
                })
        {
            return Err("Invalid project container metadata.".into());
        }
        let project_id = own
            .get("projectID")
            .and_then(Value::as_str)
            .filter(|id| valid_id(id))
            .ok_or("Invalid project identity.")?
            .to_owned();
        let source = decode_source(regular(&entries, source_filename)?)?.to_owned();
        Ok(Self {
            entries,
            metadata,
            source,
            source_filename,
            format,
            preset,
            project_id,
            schema,
        })
    }
    pub fn read(path: &Path) -> Result<Self, String> {
        // Do not follow an attachment link into external directories (including link cycles).
        match read_entry(path)? {
            Entry::Directory(entries) => Self::from_entries(entries),
            _ => Err("Fountage project must be a directory package.".into()),
        }
    }
    pub fn format(&self) -> Format {
        self.format
    }
    /// Known labels only; unrecognized imported style fields remain opaque and unchanged.
    pub fn academic_style_label(&self) -> Option<&'static str> {
        if self.format != Format::Academic {
            return None;
        }
        let value = self.metadata.get(NAMESPACE)?.get("academicStyle")?;
        for style in [
            AcademicStyle::Apa7Student,
            AcademicStyle::Apa7Professional,
            AcademicStyle::Mla9,
        ] {
            let expected = style.fields();
            if ["family", "edition", "variant"]
                .iter()
                .all(|key| value.get(key) == expected.get(key))
            {
                return Some(style.label());
            }
        }
        Some("Saved academic style")
    }
    pub fn source_filename(&self) -> &'static str {
        self.source_filename
    }
    pub fn preset(&self) -> &str {
        &self.preset
    }
    pub fn project_id(&self) -> &str {
        &self.project_id
    }
    pub fn schema(&self) -> u64 {
        self.schema
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    /// Raw tree retains original metadata bytes, foreign entries, filenames and link targets.
    pub fn entries(&self) -> &BTreeMap<OsString, Entry> {
        &self.entries
    }
    /// Parsed known-field view only; original metadata is never reserialized during source editing.
    pub fn metadata(&self) -> &Value {
        &self.metadata
    }
    /// Publish a complete new directory package; refuses any existing destination.
    /// Original metadata bytes and identity stay unchanged. This is not an in-place save/migration.
    pub fn save_copy(&self, destination: &Path) -> Result<(), String> {
        crate::project_copy::save(self.entries(), destination)
    }
    /// Apply a Shelf-only metadata transaction against an exact current snapshot.
    /// Source and every other package entry retain their original bytes. No disk
    /// writes occur until the caller explicitly publishes a new project copy.
    pub fn edit_shelf(
        &mut self,
        expected: &Value,
        operation: &crate::shelf::Operation,
    ) -> Result<bool, String> {
        if &self.metadata != expected {
            return Err("The project metadata changed. No Shelf edit was applied.".into());
        }
        let Some(metadata) =
            crate::shelf::apply(&self.metadata, operation).map_err(str::to_owned)?
        else {
            return Ok(false);
        };
        let mut entries = self.entries.clone();
        entries.insert(
            "info.json".into(),
            Entry::File(serde_json::to_vec_pretty(&metadata).map_err(|e| e.to_string())?),
        );
        let candidate = Self::from_entries(entries)?;
        *self = candidate;
        Ok(true)
    }
    pub fn replace_source(&mut self, source: &str) -> Result<(), String> {
        decode_source(source.as_bytes())?;
        self.entries.insert(
            self.source_filename.into(),
            Entry::File(source.as_bytes().to_owned()),
        );
        self.source = source.to_owned();
        Ok(())
    }
}
fn read_entry(path: &Path) -> Result<Entry, String> {
    let info = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if info.file_type().is_symlink() {
        return std::fs::read_link(path)
            .map(Entry::SymbolicLink)
            .map_err(|e| e.to_string());
    }
    if info.is_file() {
        return std::fs::read(path)
            .map(Entry::File)
            .map_err(|e| e.to_string());
    }
    if info.is_dir() {
        let mut entries = BTreeMap::new();
        for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            entries.insert(entry.file_name(), read_entry(&entry.path())?);
        }
        return Ok(Entry::Directory(entries));
    }
    Err("Unsupported project attachment type; nothing was changed.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_layouts_reject_cross_category_presets_and_styles() {
        for code in 0..8 {
            let format = Format::from_code(code).unwrap();
            for other in 0..8 {
                let other = Format::from_code(other).unwrap();
                for preset in other.layout_presets() {
                    let result = Project::new_with_layout(format, preset, None);
                    assert_eq!(result.is_ok(), format == other);
                    assert_eq!(format.starter_for_preset(preset).is_ok(), format == other);
                }
            }
            for preset in format.layout_presets() {
                assert_eq!(
                    Project::new_with_layout(format, preset, Some(AcademicStyle::Apa7Student))
                        .is_ok(),
                    format == Format::Academic
                );
            }
        }
    }
    #[test]
    fn requested_academic_editions_are_explicit_and_source_edits_keep_foreign_style_fields() {
        let root = tempfile::tempdir().unwrap();
        assert!(AcademicStyle::from_choice(-1).is_err());
        assert!(AcademicStyle::from_choice(4).is_err());
        assert!(
            Project::new_with_academic_style(Format::Comic, Some(AcademicStyle::Mla9)).is_err()
        );
        for choice in 0..4 {
            let style = AcademicStyle::from_choice(choice).unwrap();
            let project = Project::new_with_academic_style(Format::Academic, style).unwrap();
            let own = &project.metadata()[NAMESPACE];
            assert_eq!(
                project.academic_style_label(),
                style.map(AcademicStyle::label)
            );
            assert_eq!(
                own.get("academicStyle"),
                style.map(AcademicStyle::fields).as_ref()
            );
            let mut entries = project.entries().clone();
            let mut metadata = project.metadata().clone();
            if choice > 0 {
                metadata[NAMESPACE]["academicStyle"]["future"] =
                    serde_json::json!({"preserve": [true, null, 3]});
            }
            metadata[NAMESPACE]["foreign"] = serde_json::json!({"opaque": "keep"});
            let original = serde_json::to_vec_pretty(&metadata).unwrap();
            entries.insert("info.json".into(), Entry::File(original.clone()));
            entries.insert("opaque.dat".into(), Entry::File(vec![255, 0, 9]));
            let mut edited = Project::from_entries(entries).unwrap();
            edited
                .replace_source("# Research\r\nZoë 🌙 世界\n")
                .unwrap();
            let destination = root.path().join(format!("academic-{choice}.fountage"));
            edited.save_copy(&destination).unwrap();
            let reopened = Project::read(&destination).unwrap();
            assert_eq!(reopened.entries(), edited.entries());
            assert_eq!(
                reopened.entries().get(std::ffi::OsStr::new("info.json")),
                Some(&Entry::File(original))
            );
            assert_eq!(
                reopened.academic_style_label(),
                style.map(AcademicStyle::label)
            );
        }
        let mut project = Project::new(Format::Academic).unwrap();
        project.metadata[NAMESPACE]["academicStyle"] =
            serde_json::json!({"family":"apa", "edition":42,"variant":"future"});
        assert_eq!(project.academic_style_label(), Some("Saved academic style"));
    }
    #[test]
    fn new_projects_have_unique_identity_and_reopen_with_every_category() {
        let root = tempfile::tempdir().unwrap();
        let mut ids = std::collections::BTreeSet::new();
        for code in 0..8 {
            let format = Format::from_code(code).unwrap();
            let project = Project::new(format).unwrap();
            assert!(ids.insert(project.project_id().to_owned()));
            assert_eq!(project.source(), "");
            assert_eq!(project.schema(), 2);
            assert_eq!(project.metadata()["creatorIdentifier"], NAMESPACE);
            let own = &project.metadata()[NAMESPACE];
            assert_eq!(own["createdAt"], own["modifiedAt"]);
            assert!(!own["createdAt"].as_str().unwrap().is_empty());
            let destination = root.path().join(format!("new-{code}.fountage"));
            project.save_copy(&destination).unwrap();
            let reopened = Project::read(&destination).unwrap();
            assert_eq!(reopened.format(), format);
            assert_eq!(reopened.preset(), project.preset());
            assert_eq!(reopened.project_id(), project.project_id());
            assert_eq!(reopened.entries(), project.entries());
        }
    }
    fn tree(format: &str, preset: &str) -> BTreeMap<OsString, Entry> {
        let screenplay = format == "screenplay";
        let info = serde_json::json!({"version":2,"type":if screenplay {"com.fountain.screenplay"} else {"public.plain-text"},"foreign":{"opaque":[true,3.5,null]},NAMESPACE:{"version":2,"documentFormat":format,"layoutPreset":preset,"sourceFile":if screenplay {"text.fountain"} else {"text.txt"},"projectID":"00000000-1111-2222-3333-444444444444","future":{"keep":true},"academicStyle":{"family":"apa","edition":7,"variant":"student"}}});
        BTreeMap::from([
            (
                "info.json".into(),
                Entry::File(serde_json::to_vec_pretty(&info).unwrap()),
            ),
            (
                (if screenplay {
                    "text.fountain"
                } else {
                    "text.txt"
                })
                .into(),
                Entry::File("Zoë 🌙\r\n世界\n".as_bytes().to_owned()),
            ),
            (
                "assets".into(),
                Entry::Directory(BTreeMap::from([(
                    "binary".into(),
                    Entry::File(vec![0, 255, 1]),
                )])),
            ),
            ("foreign".into(), Entry::Directory(BTreeMap::new())),
        ])
    }
    #[test]
    fn every_existing_category_and_preset_retains_all_other_entry_bytes() {
        for name in [
            "screenplay",
            "comic",
            "novel",
            "academic",
            "stageplay",
            "audio",
            "general",
            "poetry",
        ] {
            for preset in presets(name).unwrap().1 {
                let original = tree(name, preset);
                let mut p = Project::from_entries(original.clone()).unwrap();
                assert_eq!(p.preset, *preset);
                assert_eq!(p.schema, 2);
                assert_eq!(p.source(), "Zoë 🌙\r\n世界\n");
                assert_eq!(p.metadata()[NAMESPACE]["academicStyle"]["edition"], 7);
                p.replace_source("New e\u{301} 🌙\r\n").unwrap();
                for (key, entry) in &original {
                    if key != OsStr::new(p.source_filename) {
                        assert_eq!(p.entries().get(key), Some(entry));
                    }
                }
                let reopened = Project::from_entries(p.entries().clone()).unwrap();
                assert_eq!(reopened.source(), p.source());
                assert_eq!(reopened.project_id, p.project_id);
            }
        }
    }
    #[test]
    fn new_copy_publication_reopens_every_category_and_preset_with_opaque_entries() {
        let root = tempfile::tempdir().unwrap();
        for name in [
            "screenplay",
            "comic",
            "novel",
            "academic",
            "stageplay",
            "audio",
            "general",
            "poetry",
        ] {
            for preset in presets(name).unwrap().1 {
                let entries = tree(name, preset);
                #[cfg(unix)]
                let entries = {
                    let mut with_link = entries;
                    with_link.insert(
                        "attachment-link".into(),
                        Entry::SymbolicLink("assets/binary".into()),
                    );
                    with_link
                };
                let mut project = Project::from_entries(entries).unwrap();
                project.replace_source("New e\u{301} 🌙\r\n").unwrap();
                let destination = root.path().join(format!("{preset}.fountage"));
                project.save_copy(&destination).unwrap();
                let reopened = Project::read(&destination).unwrap();
                assert_eq!(reopened.entries(), project.entries());
                assert_eq!(reopened.source(), project.source());
                assert_eq!(reopened.project_id(), project.project_id());
                assert!(project.save_copy(&destination).is_err());
                assert_eq!(
                    Project::read(&destination).unwrap().entries(),
                    reopened.entries()
                );
            }
        }
    }
    #[test]
    fn legacy_source_edits_keep_schema_and_metadata_opaque_until_explicit_migration() {
        let mut entries = tree("screenplay", "screenplay-standard");
        let info=br#"{ "version":2,"type":"com.fountain.screenplay","foreign":123456789012345678901234567890,"com.bazstudios.fountage":{"version":1,"projectID":"00000000-1111-2222-3333-444444444444"}}"#.to_vec();
        entries.insert("info.json".into(), Entry::File(info.clone()));
        let mut p = Project::from_entries(entries).unwrap();
        assert_eq!(p.schema, 1);
        p.replace_source("Legacy draft\r\n").unwrap();
        assert_eq!(p.entries()[OsStr::new("info.json")], Entry::File(info));
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("Legacy.fountage");
        p.save_copy(&destination).unwrap();
        let reopened = Project::read(&destination).unwrap();
        assert_eq!(reopened.schema(), 1);
        assert_eq!(reopened.entries(), p.entries());
        let before = p.entries().clone();
        assert!(p.replace_source("bad\0text").is_err());
        assert_eq!(p.entries(), &before);
    }
    #[test]
    fn malformed_and_unsupported_projects_fail_without_mutating_input() {
        let valid = tree("screenplay", "screenplay-standard");
        let Entry::File(bytes) = &valid[OsStr::new("info.json")] else {
            panic!()
        };
        let info: Value = serde_json::from_slice(bytes).unwrap();
        for (field, value) in [
            ("version", serde_json::json!(3)),
            ("version", serde_json::json!(true)),
            ("sourceFile", serde_json::json!("../text.fountain")),
            ("projectID", serde_json::json!("invalid")),
            ("layoutPreset", serde_json::json!("poetry-standard")),
        ] {
            let mut entries = valid.clone();
            let mut info = info.clone();
            info[NAMESPACE][field] = value;
            entries.insert(
                "info.json".into(),
                Entry::File(serde_json::to_vec(&info).unwrap()),
            );
            assert!(Project::from_entries(entries).is_err());
        }
        for (field, value) in [
            ("version", serde_json::json!(true)),
            ("type", serde_json::json!("public.plain-text")),
        ] {
            let mut entries = valid.clone();
            let mut changed = info.clone();
            changed[field] = value;
            entries.insert(
                "info.json".into(),
                Entry::File(serde_json::to_vec(&changed).unwrap()),
            );
            assert!(Project::from_entries(entries).is_err());
        }
        let mut missing = valid.clone();
        missing.remove(OsStr::new("text.fountain"));
        assert!(Project::from_entries(missing).is_err());
        let mut malformed = valid.clone();
        malformed.insert("info.json".into(), Entry::File(b"{".to_vec()));
        assert!(Project::from_entries(malformed).is_err());
        for bytes in [vec![255], b"bad\0text".to_vec()] {
            let mut entries = valid.clone();
            entries.insert("text.fountain".into(), Entry::File(bytes));
            assert!(Project::from_entries(entries).is_err());
        }
        let mut entries = valid.clone();
        entries.insert(
            "info.json".into(),
            Entry::SymbolicLink("external.json".into()),
        );
        assert!(Project::from_entries(entries).is_err());
        assert_eq!(
            Project::from_entries(valid.clone()).unwrap().entries(),
            &valid
        );
    }
    #[test]
    fn directory_read_is_nonmutating_and_source_changes_are_only_in_memory() {
        let root = tempfile::tempdir().unwrap();
        let entries = tree("screenplay", "screenplay-standard");
        for name in ["info.json", "text.fountain"] {
            let Entry::File(bytes) = &entries[OsStr::new(name)] else {
                panic!()
            };
            std::fs::write(root.path().join(name), bytes).unwrap();
        }
        std::fs::create_dir(root.path().join("assets")).unwrap();
        std::fs::write(root.path().join("assets/opaque"), [0, 255, 2]).unwrap();
        let mut p = Project::read(root.path()).unwrap();
        p.replace_source("Edited snapshot").unwrap();
        assert_eq!(
            std::fs::read(root.path().join("text.fountain")).unwrap(),
            "Zoë 🌙\r\n世界\n".as_bytes()
        );
        assert_eq!(
            std::fs::read(root.path().join("assets/opaque")).unwrap(),
            [0, 255, 2]
        );
    }
    #[cfg(unix)]
    #[test]
    fn links_are_not_followed_and_non_utf8_attachment_names_survive() {
        use std::os::unix::{ffi::OsStringExt, fs::symlink};
        let root = tempfile::tempdir().unwrap();
        let entries = tree("screenplay", "screenplay-standard");
        for name in ["info.json", "text.fountain"] {
            let Entry::File(bytes) = &entries[OsStr::new(name)] else {
                panic!()
            };
            std::fs::write(root.path().join(name), bytes).unwrap();
        }
        symlink(".", root.path().join("cycle")).unwrap();
        let name = OsString::from("opaque-ñ");
        std::fs::write(root.path().join(&name), [3]).unwrap();
        let p = Project::read(root.path()).unwrap();
        assert_eq!(
            p.entries()[OsStr::new("cycle")],
            Entry::SymbolicLink(".".into())
        );
        assert_eq!(p.entries()[&name], Entry::File(vec![3]));
        // macOS filesystems reject arbitrary non-UTF8 names; the in-memory tree
        // must still retain them for snapshots loaded on filesystems that allow them.
        let invalid_name = OsString::from_vec(vec![255, 254]);
        let mut entries = p.entries().clone();
        entries.insert(invalid_name.clone(), Entry::File(vec![4]));
        let mut snapshot = Project::from_entries(entries).unwrap();
        snapshot.replace_source("Edited").unwrap();
        assert_eq!(snapshot.entries()[&invalid_name], Entry::File(vec![4]));
    }
}

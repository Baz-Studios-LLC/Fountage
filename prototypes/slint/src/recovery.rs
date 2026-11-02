//! Independent, persistent scratch sessions. Recovery never writes to an original manuscript.
use fountage_core::project::{Entry, Project};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

fn is_false(value: &bool) -> bool {
    !*value
}
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    pub schema: u8,
    pub source: String,
    pub category: i32,
    #[serde(default, skip_serializing_if = "is_false")]
    pub project: bool,
    // Schema 3 overlays the complete raw info.json while sharing the same
    // immutable sibling baseline. Older readers reject schema 3 explicitly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_info: Option<String>,
}
impl Snapshot {
    fn validate(&self) -> Result<(), String> {
        if !((self.schema == 1 && !self.project && self.project_info.is_none())
            || (self.schema == 2 && self.project && self.project_info.is_none())
            || (self.schema == 3 && self.project && self.project_info.is_some()))
        {
            return Err("Unsupported recovery schema; the file was left unchanged.".into());
        }
        if let Some(info) = &self.project_info {
            let value: serde_json::Value = serde_json::from_str(info)
                .map_err(|e| format!("Invalid recovery metadata: {e}"))?;
            if !value.is_object() {
                return Err("Recovery metadata must be an object.".into());
            }
        }
        if self.project && self.source.contains('\0') {
            return Err("Project recovery manuscript contains NUL.".into());
        }
        if !(0..8).contains(&self.category) {
            return Err("Invalid recovery writing category.".into());
        }
        Ok(())
    }
}
#[derive(Debug, PartialEq, Eq)]
struct FileStamp {
    kind: u8,
    size: u64,
    modified: std::time::SystemTime,
    link: Option<PathBuf>,
    #[cfg(unix)]
    inode: (u64, u64),
}
fn baseline_stamps(root: &Path) -> Result<BTreeMap<PathBuf, FileStamp>, String> {
    fn visit(
        root: &Path,
        path: &Path,
        stamps: &mut BTreeMap<PathBuf, FileStamp>,
    ) -> Result<(), String> {
        let metadata = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        let kind = if metadata.is_symlink() {
            2
        } else if metadata.is_dir() {
            1
        } else if metadata.is_file() {
            0
        } else {
            return Err("Unsupported recovery baseline entry.".into());
        };
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        stamps.insert(
            path.strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_owned(),
            FileStamp {
                kind,
                size: metadata.len(),
                modified: metadata.modified().map_err(|e| e.to_string())?,
                link: if kind == 2 {
                    Some(std::fs::read_link(path).map_err(|e| e.to_string())?)
                } else {
                    None
                },
                #[cfg(unix)]
                inode: (metadata.dev(), metadata.ino()),
            },
        );
        if kind == 1 {
            for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
                visit(root, &entry.map_err(|e| e.to_string())?.path(), stamps)?;
            }
        }
        Ok(())
    }
    let mut stamps = BTreeMap::new();
    visit(root, root, &mut stamps)?;
    Ok(stamps)
}
struct Baseline {
    project: Project,
    stamps: BTreeMap<PathBuf, FileStamp>,
}
#[derive(Clone)]
pub struct Recovery {
    path: PathBuf,
    project_category: Option<i32>,
    root: PathBuf,
    baseline: Option<Arc<Baseline>>,
    project_info: Option<String>,
}
impl Recovery {
    pub fn create(root: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
        // Unique private directory per process/session; preserved after exit, never auto-pruned.
        let dir = tempfile::Builder::new()
            .prefix("session-")
            .tempdir_in(root)
            .map_err(|e| e.to_string())?
            .keep();
        Ok(Self {
            path: dir.join("scratch.json"),
            project_category: None,
            root: root.to_owned(),
            baseline: None,
            project_info: None,
        })
    }
    pub fn for_project(
        root: &Path,
        project: &fountage_core::project::Project,
    ) -> Result<Self, String> {
        let mut recovery = Self::create(root)?;
        project.save_copy(
            &recovery
                .path
                .parent()
                .unwrap()
                .join("project-baseline.fountage"),
        )?;
        recovery.project_category = Some(category_code(project.format()));
        recovery.baseline = Some(Arc::new(Baseline {
            project: project.clone(),
            stamps: baseline_stamps(
                &recovery
                    .path
                    .parent()
                    .unwrap()
                    .join("project-baseline.fountage"),
            )?,
        }));
        recovery.write(project.source(), category_code(project.format()))?;
        Ok(recovery)
    }
    /// Publish a new immutable metadata revision without copying attachments.
    /// Every revision is independently recoverable with its sibling baseline.
    /// Changed/missing baseline entries or changed attachments force a fresh copy
    /// from the trusted candidate; a failed publication adopts nothing.
    pub fn for_metadata(
        root: &Path,
        project: &Project,
        previous: Option<&Self>,
    ) -> Result<Self, String> {
        let reusable = previous.and_then(|previous| {
            let baseline = previous.baseline.as_ref()?;
            let parent = previous.path.parent()?;
            let base_path = parent.join("project-baseline.fountage");
            let original = &baseline.project;
            let source = std::ffi::OsStr::new(original.source_filename());
            let info = std::ffi::OsStr::new("info.json");
            let same_entries = original.entries().len() == project.entries().len()
                && original.entries().iter().all(|(key, entry)| {
                    key == source || key == info || project.entries().get(key) == Some(entry)
                });
            (previous.root == root
                && original.project_id() == project.project_id()
                && original.format() == project.format()
                && original.schema() == project.schema()
                && same_entries
                && baseline_stamps(&base_path).ok().as_ref() == Some(&baseline.stamps))
            .then_some((parent, baseline))
        });
        let Some((parent, baseline)) = reusable else {
            return Self::for_project(root, project);
        };
        let info = match project.entries().get(std::ffi::OsStr::new("info.json")) {
            Some(Entry::File(bytes)) => std::str::from_utf8(bytes)
                .map_err(|e| e.to_string())?
                .to_owned(),
            _ => return Err("Project metadata is unavailable.".into()),
        };
        let snapshot = Snapshot {
            schema: 3,
            source: project.source().to_owned(),
            category: category_code(project.format()),
            project: true,
            project_info: Some(info.clone()),
        };
        snapshot.validate()?;
        let mut file = tempfile::Builder::new()
            .prefix(".metadata-")
            .tempfile_in(parent)
            .map_err(|e| e.to_string())?;
        let filename = file.path().file_name().unwrap().to_string_lossy();
        let path = parent.join(format!("{}.json", filename.trim_start_matches('.')));
        serde_json::to_writer(file.as_file_mut(), &snapshot).map_err(|e| e.to_string())?;
        file.as_file_mut().flush().map_err(|e| e.to_string())?;
        file.as_file().sync_all().map_err(|e| e.to_string())?;
        file.persist_noclobber(&path).map_err(|e| e.to_string())?;
        Ok(Self {
            path,
            project_category: Some(snapshot.category),
            root: root.to_owned(),
            baseline: Some(baseline.clone()),
            project_info: Some(info),
        })
    }
    pub fn write(&self, source: &str, category: i32) -> Result<(), String> {
        if let Some(expected) = self.project_category
            && (category != expected || source.contains('\0'))
        {
            return Err("Project recovery requires the original category and valid source.".into());
        }
        if self.project_info.is_some()
            && self.baseline.as_ref().is_some_and(|baseline| {
                baseline_stamps(
                    &self
                        .path
                        .parent()
                        .unwrap()
                        .join("project-baseline.fountage"),
                )
                .ok()
                .as_ref()
                    != Some(&baseline.stamps)
            })
        {
            return Err(
                "Recovery baseline changed. Keep your draft open and save a new project copy."
                    .into(),
            );
        }
        let snapshot = Snapshot {
            schema: if self.project_info.is_some() {
                3
            } else if self.project_category.is_some() {
                2
            } else {
                1
            },
            source: source.to_owned(),
            category,
            project: self.project_category.is_some(),
            project_info: self.project_info.clone(),
        };
        snapshot.validate()?;
        let json = serde_json::to_string(&snapshot).map_err(|e| e.to_string())?;
        fountage_core::storage::save(&self.path, &json)
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}
pub fn read(path: &Path) -> Result<Snapshot, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let snapshot: Snapshot = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    snapshot.validate()?;
    Ok(snapshot)
}
pub fn category_code(format: fountage_core::writing::Format) -> i32 {
    use fountage_core::writing::Format;
    match format {
        Format::Screenplay => 0,
        Format::Comic => 1,
        Format::Novel => 2,
        Format::Academic => 3,
        Format::Stageplay => 4,
        Format::Audio => 5,
        Format::General => 6,
        Format::Poetry => 7,
    }
}
pub fn project_from_snapshot(
    path: &Path,
    snapshot: &Snapshot,
) -> Result<Option<fountage_core::project::Project>, String> {
    snapshot.validate()?;
    if !snapshot.project {
        return Ok(None);
    }
    let baseline = path
        .parent()
        .ok_or("Recovery location is invalid.")?
        .join("project-baseline.fountage");
    let mut project = fountage_core::project::Project::read(&baseline).map_err(|error| {
        format!("Project recovery could not read its baseline. Keep the snapshot JSON and project-baseline.fountage folder together. {error}")
    })?;
    if category_code(project.format()) != snapshot.category {
        return Err("Recovery category does not match its project baseline.".into());
    }
    if let Some(info) = &snapshot.project_info {
        let mut entries = project.entries().clone();
        entries.insert("info.json".into(), Entry::File(info.as_bytes().to_vec()));
        let candidate = Project::from_entries(entries)?;
        if candidate.project_id() != project.project_id()
            || candidate.format() != project.format()
            || candidate.schema() != project.schema()
        {
            return Err(
                "Recovery metadata does not match its project baseline identity or category."
                    .into(),
            );
        }
        project = candidate;
    }
    project.replace_source(&snapshot.source)?;
    Ok(Some(project))
}
pub fn directory() -> Result<PathBuf, String> {
    dirs::data_local_dir()
        .map(|p| p.join("Fountage Slint Lab").join("Recovery"))
        .ok_or_else(|| "Local application data directory is unavailable.".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persisted_category_codes_match_the_shared_engine_and_project_flag_requires_schema_two() {
        for code in 0..8 {
            assert_eq!(
                category_code(fountage_core::writing::Format::from_code(code).unwrap()),
                code as i32
            );
        }
        let invalid = Snapshot {
            schema: 1,
            source: "draft".into(),
            category: 0,
            project: true,
            project_info: None,
        };
        assert!(invalid.validate().is_err());
    }
    #[test]
    fn independent_sessions_survive_drop_and_reopen_exact_unicode_line_endings() {
        let root = tempfile::tempdir().unwrap();
        let first = Recovery::create(root.path()).unwrap();
        let second = Recovery::create(root.path()).unwrap();
        let source = "Zoë 🌙\r\n世界\n\0e\u{301}";
        first.write(source, 1).unwrap();
        second.write("separate", 7).unwrap();
        assert_ne!(first.path(), second.path());
        let path = first.path().to_owned();
        drop(first);
        assert_eq!(
            read(&path).unwrap(),
            Snapshot {
                schema: 1,
                source: source.into(),
                category: 1,
                project: false,
                project_info: None,
            }
        );
        assert_eq!(read(second.path()).unwrap().source, "separate");
    }
    #[test]
    fn invalid_update_and_bad_recovery_preserve_the_last_good_snapshot() {
        let root = tempfile::tempdir().unwrap();
        let session = Recovery::create(root.path()).unwrap();
        session.write("original", 0).unwrap();
        let bytes = std::fs::read(session.path()).unwrap();
        assert!(session.write("bad", 8).is_err());
        assert_eq!(std::fs::read(session.path()).unwrap(), bytes);
        let bad = root.path().join("bad.json");
        for text in [
            "{",
            r#"{"schema":2,"source":"future","category":0}"#,
            r#"{"schema":1,"source":"bad","category":-1}"#,
        ] {
            std::fs::write(&bad, text).unwrap();
            assert!(read(&bad).is_err());
            assert_eq!(std::fs::read_to_string(&bad).unwrap(), text);
        }
    }
    #[test]
    fn every_category_updates_atomically_and_failure_is_reported() {
        let root = tempfile::tempdir().unwrap();
        let session = Recovery::create(root.path()).unwrap();
        for category in 0..8 {
            session.write("new draft", category).unwrap();
            assert_eq!(read(session.path()).unwrap().category, category);
        }
        assert_eq!(
            std::fs::read_dir(session.path().parent().unwrap())
                .unwrap()
                .count(),
            1
        );
        let invalid = Recovery {
            path: root.path().join("missing").join("scratch.json"),
            project_category: None,
            root: root.path().to_owned(),
            baseline: None,
            project_info: None,
        };
        assert!(invalid.write("unsaved", 0).is_err());
        assert_eq!(read(session.path()).unwrap().source, "new draft");
    }
    fn metadata_fixture(code: u32) -> Project {
        use fountage_core::{project::NAMESPACE, writing::Format};
        let mut project = Project::new(Format::from_code(code).unwrap()).unwrap();
        project
            .replace_source("Original Zoë 🌙\r\n世界\r\n")
            .unwrap();
        let mut metadata = project.metadata().clone();
        metadata["foreign"] = serde_json::json!({"future":[18446744073709551615u64,null,true]});
        metadata[NAMESPACE]["shelf"] =
            serde_json::json!({"version":1,"future":"keep","items":{},"order":["reserved"]});
        let mut entries = project.entries().clone();
        entries.insert(
            "info.json".into(),
            Entry::File(serde_json::to_vec_pretty(&metadata).unwrap()),
        );
        entries.insert("asset.bin".into(), Entry::File(vec![0, 255, 1]));
        entries.insert("link".into(), Entry::SymbolicLink("asset.bin".into()));
        Project::from_entries(entries).unwrap()
    }
    fn quick(project: &Project, text: &str) -> Project {
        use fountage_core::shelf::{self, Operation};
        let mut edited = project.clone();
        edited
            .edit_shelf(
                project.metadata(),
                &Operation::SetQuickText {
                    expected: shelf::quick_text(project.metadata()).unwrap().to_owned(),
                    text: text.into(),
                },
            )
            .unwrap();
        edited
    }
    #[test]
    fn metadata_revisions_share_immutable_baseline_and_recover_all_formats_after_handles_drop() {
        let root = tempfile::tempdir().unwrap();
        for code in 0..8 {
            let original = metadata_fixture(code);
            let base = Recovery::for_project(root.path(), &original).unwrap();
            let baseline = base
                .path()
                .parent()
                .unwrap()
                .join("project-baseline.fountage");
            let stamps = baseline_stamps(&baseline).unwrap();
            let old_snapshot = std::fs::read(base.path()).unwrap();
            let candidate = quick(&original, "Draft Zoë 🌙\r\n世界 e\u{301}\r\n");
            let revision = Recovery::for_metadata(root.path(), &candidate, Some(&base)).unwrap();
            assert_eq!(revision.path().parent(), base.path().parent());
            assert_ne!(revision.path(), base.path());
            assert_eq!(read(revision.path()).unwrap().schema, 3);
            assert_eq!(baseline_stamps(&baseline).unwrap(), stamps);
            assert_eq!(std::fs::read(base.path()).unwrap(), old_snapshot);
            let immutable_revision = std::fs::read(revision.path()).unwrap();
            let next = quick(&candidate, "Next unfinished note");
            let next_revision =
                Recovery::for_metadata(root.path(), &next, Some(&revision)).unwrap();
            assert_eq!(std::fs::read(revision.path()).unwrap(), immutable_revision);
            assert_eq!(baseline_stamps(&baseline).unwrap(), stamps);
            next_revision
                .write("Live manuscript 日本 🌙\r\n", code as i32)
                .unwrap();
            let path = next_revision.path().to_owned();
            drop(base);
            drop(revision);
            drop(next_revision);
            let snapshot = read(&path).unwrap();
            let restored = project_from_snapshot(&path, &snapshot).unwrap().unwrap();
            let mut expected = next.clone();
            expected
                .replace_source("Live manuscript 日本 🌙\r\n")
                .unwrap();
            assert_eq!(restored.entries(), expected.entries());
            assert_eq!(
                Project::read(&baseline).unwrap().entries(),
                original.entries()
            );
            let copy = root.path().join(format!("Restored {code}.fountage"));
            restored.save_copy(&copy).unwrap();
            assert_eq!(Project::read(&copy).unwrap().entries(), expected.entries());
            let candidate_snapshot =
                read(path.parent().unwrap().join("scratch.json").as_path()).unwrap();
            assert_eq!(
                candidate_snapshot.schema, 2,
                "older snapshot remains readable"
            );
        }
    }
    #[test]
    fn changed_baseline_assets_identity_or_root_never_reuse_an_unsafe_baseline() {
        let root = tempfile::tempdir().unwrap();
        let original = metadata_fixture(0);
        let base = Recovery::for_project(root.path(), &original).unwrap();
        let candidate = quick(&original, "Keep this draft");
        let asset = base
            .path()
            .parent()
            .unwrap()
            .join("project-baseline.fountage/asset.bin");
        std::fs::write(&asset, [8, 9, 0]).unwrap();
        std::fs::File::open(&asset)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(
                std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(100),
            ))
            .unwrap();
        let repaired = Recovery::for_metadata(root.path(), &candidate, Some(&base)).unwrap();
        assert_ne!(repaired.path().parent(), base.path().parent());
        let restored = project_from_snapshot(repaired.path(), &read(repaired.path()).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(restored.entries(), candidate.entries());
        let mut changed_entries = candidate.entries().clone();
        changed_entries.insert("asset.bin".into(), Entry::File(vec![4, 5, 6]));
        let changed = Project::from_entries(changed_entries).unwrap();
        let with_asset = Recovery::for_metadata(root.path(), &changed, Some(&repaired)).unwrap();
        assert_ne!(with_asset.path().parent(), repaired.path().parent());
        assert_eq!(
            project_from_snapshot(with_asset.path(), &read(with_asset.path()).unwrap())
                .unwrap()
                .unwrap()
                .entries(),
            changed.entries()
        );
        let different = metadata_fixture(0);
        let new_identity =
            Recovery::for_metadata(root.path(), &different, Some(&with_asset)).unwrap();
        assert_ne!(new_identity.path().parent(), with_asset.path().parent());
        let blocked = root.path().join("blocked");
        std::fs::write(&blocked, "keep").unwrap();
        let before = std::fs::read(with_asset.path()).unwrap();
        assert!(Recovery::for_metadata(&blocked, &changed, Some(&with_asset)).is_err());
        assert_eq!(std::fs::read(with_asset.path()).unwrap(), before);
        let missing = with_asset
            .path()
            .parent()
            .unwrap()
            .join("project-baseline.fountage/asset.bin");
        std::fs::remove_file(missing).unwrap();
        let repaired = Recovery::for_metadata(root.path(), &changed, Some(&with_asset)).unwrap();
        assert_eq!(
            project_from_snapshot(repaired.path(), &read(repaired.path()).unwrap())
                .unwrap()
                .unwrap()
                .entries(),
            changed.entries()
        );
    }
    #[test]
    fn metadata_snapshot_schema_identity_and_failed_source_updates_reject_without_rewriting() {
        let root = tempfile::tempdir().unwrap();
        let original = metadata_fixture(0);
        let base = Recovery::for_project(root.path(), &original).unwrap();
        let edited = quick(&original, "Idea");
        let revision = Recovery::for_metadata(root.path(), &edited, Some(&base)).unwrap();
        let before = std::fs::read(revision.path()).unwrap();
        assert!(revision.write("wrong category", 1).is_err());
        assert!(revision.write("bad\0", 0).is_err());
        assert_eq!(std::fs::read(revision.path()).unwrap(), before);
        let mut invalid_location = revision.clone();
        invalid_location.path = root.path().join("missing/scratch.json");
        assert!(invalid_location.write("not adopted", 0).is_err());
        assert_eq!(std::fs::read(revision.path()).unwrap(), before);
        let mut snapshot = read(revision.path()).unwrap();
        for (schema, project, info) in [
            (3, true, None),
            (2, true, Some("{}")),
            (1, false, Some("{}")),
            (3, false, Some("{}")),
            (4, true, Some("{}")),
            (3, true, Some("not JSON")),
            (3, true, Some("[]")),
        ] {
            snapshot.schema = schema;
            snapshot.project = project;
            snapshot.project_info = info.map(str::to_owned);
            assert!(snapshot.validate().is_err());
        }
        let mut snapshot = read(revision.path()).unwrap();
        let different = metadata_fixture(0);
        snapshot.project_info =
            Some(String::from_utf8(crate::project_metadata::info(&different).unwrap()).unwrap());
        assert!(project_from_snapshot(revision.path(), &snapshot).is_err());
        let mut metadata = edited.metadata().clone();
        metadata[fountage_core::project::NAMESPACE]["sourceFile"] = "../escape".into();
        snapshot.project_info = Some(serde_json::to_string(&metadata).unwrap());
        assert!(project_from_snapshot(revision.path(), &snapshot).is_err());
        let mut metadata = edited.metadata().clone();
        metadata[fountage_core::project::NAMESPACE]["version"] = 1.into();
        snapshot.project_info = Some(serde_json::to_string(&metadata).unwrap());
        assert!(project_from_snapshot(revision.path(), &snapshot).is_err());
        assert_eq!(std::fs::read(revision.path()).unwrap(), before);
    }
}

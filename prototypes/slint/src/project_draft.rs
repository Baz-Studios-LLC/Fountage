//! Shared guards used by CLI/UI project drafts: no source-only fallback on package failures.
use crate::recovery::Recovery;
use fountage_core::project::Project;
use std::path::Path;
pub fn create(
    category: i32,
    include_starter: bool,
    academic_style: i32,
    layout_choice: i32,
    recovery_root: &Path,
) -> Result<(Project, Recovery), String> {
    let format = u32::try_from(category)
        .ok()
        .and_then(fountage_core::writing::Format::from_code)
        .ok_or("Invalid writing category.")?;
    let style = if format == fountage_core::writing::Format::Academic {
        fountage_core::project::AcademicStyle::from_choice(academic_style)?
    } else {
        None
    };
    let preset = usize::try_from(layout_choice)
        .ok()
        .and_then(|index| format.layout_presets().get(index))
        .ok_or("Invalid layout selection for writing category.")?;
    let mut project = Project::new_with_layout(format, preset, style)?;
    if include_starter {
        project.replace_source(format.starter_for_preset(preset)?)?;
    }
    let recovery = Recovery::for_project(recovery_root, &project)?;
    Ok((project, recovery))
}
pub fn open(path: &Path, recovery_root: &Path) -> Result<(Project, Recovery), String> {
    let project = Project::read(path)?;
    let recovery = Recovery::for_project(recovery_root, &project)?;
    Ok((project, recovery))
}
pub fn save_copy(project: &Project, source: &str, destination: &Path) -> Result<(), String> {
    let mut copy = project.clone();
    copy.replace_source(source)?;
    copy.save_copy(destination)
}
#[cfg(test)]
mod tests {
    use super::*;
    use fountage_core::project::{Entry, NAMESPACE};
    use std::{collections::BTreeMap, ffi::OsString};
    #[test]
    fn new_project_recovery_preserves_type_and_identity_and_failure_cannot_adopt() {
        let root = tempfile::tempdir().unwrap();
        for category in 0..8 {
            let (project, baseline) = create(category, false, 0, 0, root.path()).unwrap();
            let snapshot = crate::recovery::read(baseline.path()).unwrap();
            assert_eq!(snapshot.source, "");
            assert_eq!(snapshot.category, category);
            let recovered = crate::recovery::project_from_snapshot(baseline.path(), &snapshot)
                .unwrap()
                .unwrap();
            assert_eq!(recovered.format(), project.format());
            assert_eq!(recovered.project_id(), project.project_id());
        }
        assert!(create(-1, false, 0, 0, root.path()).is_err());
        assert!(create(8, false, 0, 0, root.path()).is_err());
        let blocked = root.path().join("file");
        std::fs::write(&blocked, "keep").unwrap();
        assert!(create(1, true, 0, 0, &blocked).is_err());
        assert_eq!(std::fs::read_to_string(blocked).unwrap(), "keep");
    }
    #[test]
    fn starter_projects_recover_and_reopen_all_categories_without_overwrite() {
        let root = tempfile::tempdir().unwrap();
        for (category, marker) in [
            (0, "INT. ROOM - DAY"),
            (1, "PAGE 1 (2 panels)"),
            (2, "Chapter One"),
            (3, "# Introduction"),
            (4, "ACT I"),
            (5, "SFX:"),
            (6, "# An idea"),
            (7, "# First Poem"),
        ] {
            let (project, recovery) = create(category, true, 0, 0, root.path()).unwrap();
            assert!(project.source().contains(marker));
            let snapshot = crate::recovery::read(recovery.path()).unwrap();
            assert_eq!(snapshot.source, project.source());
            let restored = crate::recovery::project_from_snapshot(recovery.path(), &snapshot)
                .unwrap()
                .unwrap();
            assert_eq!(restored.entries(), project.entries());
            let destination = root.path().join(format!("Starter-{category}.fountage"));
            save_copy(&project, project.source(), &destination).unwrap();
            let reopened = Project::read(&destination).unwrap();
            assert_eq!(reopened.entries(), project.entries());
            assert!(save_copy(&project, "must not overwrite", &destination).is_err());
            assert_eq!(
                Project::read(&destination).unwrap().entries(),
                project.entries()
            );
        }
    }
    #[test]
    fn academic_choices_recover_and_copy_without_changing_manuscript_or_edition() {
        let root = tempfile::tempdir().unwrap();
        for choice in 0..4 {
            for starter in [false, true] {
                let (project, baseline) = create(3, starter, choice, 0, root.path()).unwrap();
                let snapshot = crate::recovery::read(baseline.path()).unwrap();
                let recovered = crate::recovery::project_from_snapshot(baseline.path(), &snapshot)
                    .unwrap()
                    .unwrap();
                assert_eq!(recovered.entries(), project.entries());
                let edited = format!("{}\r\nZoë 世界", project.source());
                let destination = root
                    .path()
                    .join(format!("Academic-{choice}-{starter}.fountage"));
                save_copy(&recovered, &edited, &destination).unwrap();
                let reopened = Project::read(&destination).unwrap();
                assert_eq!(reopened.metadata(), project.metadata());
                assert_eq!(reopened.source(), edited);
                assert_eq!(
                    reopened.academic_style_label(),
                    project.academic_style_label()
                );
            }
        }
        assert!(create(3, true, 99, 0, root.path()).is_err());
        let (comic, _) = create(1, false, 2, 0, root.path()).unwrap();
        assert!(
            comic.metadata()[NAMESPACE].get("academicStyle").is_none(),
            "hidden academic choice never leaks into another category"
        );
    }
    #[test]
    fn every_layout_and_starter_survives_complete_recovery_save_and_reopen() {
        use fountage_core::writing::Format;
        let root = tempfile::tempdir().unwrap();
        for category in 0..8 {
            let format = Format::from_code(category as u32).unwrap();
            for (choice, preset) in format.layout_presets().iter().enumerate() {
                for starter in [false, true] {
                    let (project, baseline) =
                        create(category, starter, 0, choice as i32, root.path()).unwrap();
                    assert_eq!(project.preset(), *preset);
                    assert_eq!(
                        project.source(),
                        if starter {
                            format.starter_for_preset(preset).unwrap()
                        } else {
                            ""
                        }
                    );
                    let snapshot = crate::recovery::read(baseline.path()).unwrap();
                    let restored =
                        crate::recovery::project_from_snapshot(baseline.path(), &snapshot)
                            .unwrap()
                            .unwrap();
                    assert_eq!(restored.entries(), project.entries());
                    let source = format!("{}\r\nZoë 🌙 世界 e\u{301}\r\n", project.source());
                    let path = root.path().join(format!("{preset}-{starter}.fountage"));
                    save_copy(&restored, &source, &path).unwrap();
                    let reopened = Project::read(&path).unwrap();
                    assert_eq!(reopened.preset(), *preset);
                    assert_eq!(reopened.metadata(), project.metadata());
                    assert_eq!(reopened.source(), source);
                    assert!(save_copy(&restored, "must not overwrite", &path).is_err());
                    assert_eq!(Project::read(&path).unwrap().source(), source);
                }
            }
            for invalid in [-1, format.layout_presets().len() as i32, i32::MAX] {
                assert!(create(category, true, 0, invalid, root.path()).is_err());
            }
        }
        for (category, marker) in [
            (5, "SEGMENT 1 - INTRODUCTION"),
            (6, "## Key points"),
            (7, "CHORUS"),
        ] {
            let (project, _) = create(category, true, 0, 1, root.path()).unwrap();
            assert!(project.source().contains(marker));
        }
    }
    fn fixture() -> Project {
        let info = serde_json::json!({"version":2,"type":"public.plain-text","foreign":{"future":true},NAMESPACE:{"version":2,"documentFormat":"comic","layoutPreset":"comic-full-script","sourceFile":"text.txt","projectID":"00000000-1111-2222-3333-444444444444","shelf":{"opaque":[1,2,3]}}});
        Project::from_entries(BTreeMap::<OsString, Entry>::from([
            (
                "info.json".into(),
                Entry::File(serde_json::to_vec_pretty(&info).unwrap()),
            ),
            ("text.txt".into(), Entry::File(b"PAGE 1\r\n".to_vec())),
            (
                "assets".into(),
                Entry::Directory(BTreeMap::from([(
                    "opaque".into(),
                    Entry::File(vec![0, 255]),
                )])),
            ),
        ]))
        .unwrap()
    }
    #[test]
    fn open_edit_recover_and_copy_preserve_full_project_and_leave_original_intact() {
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("Original.fountage");
        let fixture = fixture();
        fixture.save_copy(&original).unwrap();
        let (project, recovery) = open(&original, &root.path().join("Recovery")).unwrap();
        let source = "PAGE 1\r\n\r\nPanel 1\r\nZoë 🌙\r\n";
        let replacement = crate::find::replace_all(source, "Zoë", true, "$1 世界")
            .unwrap()
            .unwrap();
        assert_eq!(replacement.count, 1);
        let source = replacement.source.as_str();
        recovery.write(source, 1).unwrap();
        let snapshot = crate::recovery::read(recovery.path()).unwrap();
        assert_eq!(snapshot.schema, 2);
        let restored = crate::recovery::project_from_snapshot(recovery.path(), &snapshot)
            .unwrap()
            .unwrap();
        let copied = root.path().join("Copied.fountage");
        save_copy(&restored, restored.source(), &copied).unwrap();
        let mut expected = fixture.clone();
        expected.replace_source(source).unwrap();
        assert_eq!(
            Project::read(&copied).unwrap().entries(),
            expected.entries()
        );
        assert_eq!(
            Project::read(&original).unwrap().entries(),
            fixture.entries()
        );
        assert_eq!(project.source(), fixture.source());
        assert!(save_copy(&project, source, &original).is_err());
        assert_eq!(
            Project::read(&original).unwrap().entries(),
            fixture.entries()
        );
    }
    #[test]
    fn failed_baseline_category_drift_or_missing_baseline_never_becomes_a_plain_draft() {
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("Original.fountage");
        fixture().save_copy(&original).unwrap();
        let blocked = root.path().join("blocked");
        std::fs::write(&blocked, b"file").unwrap();
        assert!(open(&original, &blocked).is_err());
        let (_, recovery) = open(&original, &root.path().join("Recovery")).unwrap();
        let before = std::fs::read(recovery.path()).unwrap();
        assert!(recovery.write("new", 0).is_err());
        assert!(recovery.write("bad\0", 1).is_err());
        assert_eq!(std::fs::read(recovery.path()).unwrap(), before);
        let mut snapshot = crate::recovery::read(recovery.path()).unwrap();
        snapshot.category = 0;
        assert!(crate::recovery::project_from_snapshot(recovery.path(), &snapshot).is_err());
        snapshot.category = 1;
        assert!(
            crate::recovery::project_from_snapshot(&root.path().join("missing.json"), &snapshot)
                .is_err()
        );
        assert_eq!(
            Project::read(&original).unwrap().entries(),
            fixture().entries()
        );
    }
}

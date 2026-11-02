//! Stage a new package beside the destination, then rename with an OS exclusive operation.
//! Never fall back to exists()+rename(), which could overwrite a racing destination.
use crate::project::Entry;
use std::{
    collections::BTreeMap,
    ffi::OsString,
    io::Write,
    path::{Component, Path},
};

fn validate(entries: &BTreeMap<OsString, Entry>) -> Result<(), String> {
    for (name, entry) in entries {
        let mut components = Path::new(name).components();
        if !matches!(components.next(),Some(Component::Normal(n)) if n == name)
            || components.next().is_some()
            || name.as_encoded_bytes().contains(&0)
        {
            return Err("Invalid package entry name; no project copy was published.".into());
        }
        if let Entry::Directory(children) = entry {
            validate(children)?;
        }
        #[cfg(not(unix))]
        if matches!(entry, Entry::SymbolicLink(_)) {
            return Err("This platform cannot yet preserve project attachment links; no copy was published.".into());
        }
    }
    Ok(())
}
fn write_entries(entries: &BTreeMap<OsString, Entry>, directory: &Path) -> Result<(), String> {
    for (name, entry) in entries {
        let path = directory.join(name);
        match entry {
            Entry::File(bytes) => {
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .map_err(|e| e.to_string())?;
                file.write_all(bytes).map_err(|e| e.to_string())?;
                file.sync_all().map_err(|e| e.to_string())?;
            }
            Entry::Directory(children) => {
                std::fs::create_dir(&path).map_err(|e| e.to_string())?;
                write_entries(children, &path)?;
            }
            Entry::SymbolicLink(target) => {
                #[cfg(unix)]
                std::os::unix::fs::symlink(target, &path).map_err(|e| e.to_string())?;
                #[cfg(not(unix))]
                {
                    let _ = target;
                    return Err(
                        "Attachment links are not supported for copy on this platform.".into(),
                    );
                }
            }
        }
    }
    Ok(())
}
pub(crate) fn save(entries: &BTreeMap<OsString, Entry>, destination: &Path) -> Result<(), String> {
    if destination.extension() != Some(std::ffi::OsStr::new("fountage"))
        || destination.file_name().is_none()
    {
        return Err("Choose a new .fountage destination.".into());
    }
    validate(entries)?;
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let staging = tempfile::Builder::new()
        .prefix(".fountage-copy-")
        .tempdir_in(parent)
        .map_err(|e| e.to_string())?;
    write_entries(entries, staging.path())?;
    // No precheck is relied on. Failure (including unsupported filesystems) cleans only our staging tree.
    renamore::rename_exclusive(staging.path(), destination).map_err(|e| e.to_string())?;
    let _ = staging.keep(); // The former staging name no longer exists; don't attempt cleanup after publication.
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn occupied_empty_directory_file_and_dangling_link_are_never_replaced() {
        let root = tempfile::tempdir().unwrap();
        let tree = BTreeMap::from([("marker".into(), Entry::File(b"new".to_vec()))]);
        let destination = root.path().join("Occupied.fountage");
        std::fs::create_dir(&destination).unwrap();
        assert!(save(&tree, &destination).is_err());
        assert_eq!(std::fs::read_dir(&destination).unwrap().count(), 0);
        let file = root.path().join("File.fountage");
        std::fs::write(&file, b"old").unwrap();
        assert!(save(&tree, &file).is_err());
        assert_eq!(std::fs::read(&file).unwrap(), b"old");
        #[cfg(unix)]
        {
            let link = root.path().join("Link.fountage");
            std::os::unix::fs::symlink("missing", &link).unwrap();
            assert!(save(&tree, &link).is_err());
            assert_eq!(std::fs::read_link(&link).unwrap(), Path::new("missing"));
        }
        assert!(!std::fs::read_dir(root.path()).unwrap().any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".fountage-copy-")
        }));
    }
    #[test]
    fn invalid_tree_cannot_escape_staging_or_publish_a_partial_project() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("Copy.fountage");
        for name in ["../outside", ".", "folder/.", "/absolute", "bad\0name"] {
            let tree = BTreeMap::from([(name.into(), Entry::File(vec![1]))]);
            assert!(save(&tree, &destination).is_err());
            assert!(!destination.exists());
        }
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }
    #[test]
    fn staging_write_failure_leaves_no_destination_or_staging_residue() {
        let root = tempfile::tempdir().unwrap();
        let tree = BTreeMap::from([
            ("a-written-first".into(), Entry::File(vec![1])),
            (OsString::from("z".repeat(4096)), Entry::File(vec![2])),
        ]);
        assert!(save(&tree, &root.path().join("Failed.fountage")).is_err());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }
    #[test]
    fn concurrent_publications_have_exactly_one_winner_with_complete_content() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("Race.fountage");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let jobs = (0..2)
            .map(|i| {
                let path = destination.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let tree = BTreeMap::from([
                        ("first".into(), Entry::File(vec![i])),
                        ("last".into(), Entry::File(vec![i; 4096])),
                    ]);
                    barrier.wait();
                    save(&tree, &path).is_ok()
                })
            })
            .collect::<Vec<_>>();
        let results = jobs
            .into_iter()
            .map(|job| job.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|success| **success).count(), 1);
        let first = std::fs::read(destination.join("first")).unwrap();
        assert_eq!(
            std::fs::read(destination.join("last")).unwrap(),
            vec![first[0]; 4096]
        );
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    }
}

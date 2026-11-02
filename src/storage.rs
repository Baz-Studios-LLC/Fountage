use std::{io::Write, path::Path};

pub fn save(path: &Path, source: &str) -> Result<(), String> {
    let parent = path.parent().ok_or("Invalid save location")?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    if let Ok(metadata) = std::fs::metadata(path) {
        file.as_file()
            .set_permissions(metadata.permissions())
            .map_err(|e| e.to_string())?;
    }
    file.write_all(source.as_bytes())
        .map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_unicode_and_replaces_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("story.fountain");
        save(&path, "old").unwrap();
        let source = "Title: Café\n\n@Zoë\nHello, 世界.\n";
        save(&path, source).unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), source);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn failed_save_is_reported() {
        assert!(
            save(
                Path::new("/no-such-fountage-directory/story.fountain"),
                "test"
            )
            .is_err()
        );
    }
}

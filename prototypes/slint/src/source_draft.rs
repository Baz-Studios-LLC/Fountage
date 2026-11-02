//! Prepare a source-only draft completely before replacing the current session.
use crate::recovery;
use std::path::Path;

pub fn read(path: &Path) -> Result<String, String> {
    let extension = path.extension().and_then(|v| v.to_str()).unwrap_or("");
    if !["fountain", "txt", "md"]
        .iter()
        .any(|v| extension.eq_ignore_ascii_case(v))
    {
        return Err("Choose a UTF-8 .fountain, .txt or .md source file.".into());
    }
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("Source could not be read as UTF-8: {e}"))?;
    if source.contains('\0') {
        return Err("Source contains a NUL character and was not opened.".into());
    }
    Ok(source)
}

pub fn open(
    path: &Path,
    category: i32,
    recovery_root: &Path,
) -> Result<(String, i32, recovery::Recovery), String> {
    let source = read(path)?;
    let category = if path
        .extension()
        .is_some_and(|v| v.eq_ignore_ascii_case("fountain"))
    {
        0
    } else {
        category
    };
    let baseline = recovery::Recovery::create(recovery_root)?;
    baseline.write(&source, category)?;
    Ok((source, category, baseline))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn open_preserves_source_bytes_category_and_empty_drafts_and_requires_recovery() {
        let root = tempfile::tempdir().unwrap();
        let recovery = root.path().join("recovery");
        for (name, source, chosen, expected) in [
            ("Café.FOUNTAIN", "Café 🌙 e\u{301}\r\n世界\r\n", 1, 0),
            ("Comic.txt", "PAGE 1\r\nPanel 1\r\n", 1, 1),
            ("Empty.md", "", 2, 2),
        ] {
            let path = root.path().join(name);
            std::fs::write(&path, source).unwrap();
            let (loaded, category, baseline) = open(&path, chosen, &recovery).unwrap();
            assert_eq!(loaded, source);
            assert_eq!(category, expected);
            let snapshot = recovery::read(baseline.path()).unwrap();
            assert_eq!(snapshot.source, source);
            assert_eq!(snapshot.category, expected);
            assert!(!snapshot.project);
            assert_eq!(std::fs::read(&path).unwrap(), source.as_bytes());
        }
        let blocked = root.path().join("blocked");
        std::fs::write(&blocked, "file, not directory").unwrap();
        assert!(open(&root.path().join("Comic.txt"), 1, &blocked).is_err());
        for (name, bytes) in [
            ("Bad.txt", vec![255]),
            ("Nul.txt", b"a\0b".to_vec()),
            ("Unknown.pdf", b"text".to_vec()),
        ] {
            let path = root.path().join(name);
            std::fs::write(&path, &bytes).unwrap();
            assert!(open(&path, 0, &recovery).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
    }
}

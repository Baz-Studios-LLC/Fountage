//! Source save checkpoints distinguish a saved empty file from an unsaved recovered draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    source: Option<String>,
    metadata: Option<serde_json::Value>,
    saved_metadata: Option<serde_json::Value>,
}
impl Checkpoint {
    pub fn saved(source: &str) -> Self {
        Self {
            source: Some(source.to_owned()),
            metadata: None,
            saved_metadata: None,
        }
    }
    pub fn is_clean(&self, source: &str) -> bool {
        self.source.as_deref() == Some(source) && self.metadata == self.saved_metadata
    }
    pub fn mark_saved(&mut self, source: &str) {
        self.source = Some(source.to_owned());
        self.saved_metadata = self.metadata.clone();
    }
    /// Adopt a newly opened document's saved source and metadata together.
    pub fn adopt_saved(&mut self, source: &str, metadata: Option<&serde_json::Value>) {
        self.set_metadata(metadata);
        self.mark_saved(source);
    }
    /// Track the current project metadata independently of the saved checkpoint.
    pub fn tracks_metadata(&self, metadata: Option<&serde_json::Value>) -> bool {
        self.metadata.as_ref() == metadata
    }
    pub fn set_metadata(&mut self, metadata: Option<&serde_json::Value>) {
        self.metadata = metadata.cloned();
    }
    pub fn require_save(&mut self) {
        self.source = None;
    }
}
#[derive(Debug, Clone, Copy)]
pub enum CloseChoice {
    SaveCopy,
    Discard,
    KeepWriting,
}
pub fn may_close(
    checkpoint: &mut Checkpoint,
    source: &str,
    choose: impl FnOnce() -> CloseChoice,
    save: impl FnOnce() -> bool,
) -> bool {
    if checkpoint.is_clean(source) {
        return true;
    }
    match choose() {
        CloseChoice::KeepWriting => false,
        CloseChoice::Discard => true,
        CloseChoice::SaveCopy => {
            if save() {
                checkpoint.mark_saved(source);
                true
            } else {
                false
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clean_source_and_undo_to_checkpoint_do_not_prompt() {
        let mut checkpoint = Checkpoint::saved("Zoë 🌙\r\n");
        assert!(may_close(
            &mut checkpoint,
            "Zoë 🌙\r\n",
            || panic!("must not prompt"),
            || panic!("must not save")
        ));
        assert!(!checkpoint.is_clean("Zoë 🌙\n"));
        assert!(checkpoint.is_clean("Zoë 🌙\r\n"));
    }
    #[test]
    fn cancel_and_failed_or_canceled_save_preserve_checkpoint_and_source() {
        let source = "Zoë 🌙\r\n世界\r\n";
        let mut checkpoint = Checkpoint::saved("original");
        assert!(!may_close(
            &mut checkpoint,
            source,
            || CloseChoice::KeepWriting,
            || panic!("must not save")
        ));
        assert_eq!(checkpoint, Checkpoint::saved("original"));
        assert!(!may_close(
            &mut checkpoint,
            source,
            || CloseChoice::SaveCopy,
            || false
        ));
        assert_eq!(checkpoint, Checkpoint::saved("original"));
        assert_eq!(source, "Zoë 🌙\r\n世界\r\n");
    }
    #[test]
    fn explicit_discard_and_successful_save_are_the_only_dirty_close_paths() {
        let mut checkpoint = Checkpoint::saved("original");
        assert!(may_close(
            &mut checkpoint,
            "changed",
            || CloseChoice::Discard,
            || panic!("must not save")
        ));
        assert_eq!(checkpoint, Checkpoint::saved("original"));
        assert!(may_close(
            &mut checkpoint,
            "Zoë 🌙\r\n",
            || CloseChoice::SaveCopy,
            || true
        ));
        assert_eq!(checkpoint, Checkpoint::saved("Zoë 🌙\r\n"));
        assert!(!may_close(
            &mut checkpoint,
            "Zoë 🌙\n",
            || CloseChoice::KeepWriting,
            || false
        ));
    }
    #[test]
    fn recovered_empty_source_requires_a_save_and_cannot_replace_a_current_draft() {
        let mut checkpoint = Checkpoint::saved("");
        assert!(checkpoint.is_clean(""));
        checkpoint.require_save();
        assert!(!checkpoint.is_clean(""));
        assert!(!may_close(
            &mut checkpoint,
            "",
            || CloseChoice::KeepWriting,
            || false
        ));
        assert!(!may_close(
            &mut checkpoint,
            "",
            || CloseChoice::SaveCopy,
            || false
        ));
        assert!(!checkpoint.is_clean(""));
        assert!(may_close(
            &mut checkpoint,
            "",
            || CloseChoice::SaveCopy,
            || true
        ));
        assert!(checkpoint.is_clean(""));
    }
}

#[cfg(test)]
mod metadata_tests {
    use super::*;
    #[test]
    fn metadata_only_dirty_state_tracks_saved_baseline_without_resetting_source() {
        let source = "Zoë 🌙\r\n";
        let before = serde_json::json!({"shelf":{"state":false}});
        let after = serde_json::json!({"shelf":{"state":true}});
        let mut checkpoint = Checkpoint::saved(source);
        checkpoint.adopt_saved(source, Some(&before));
        checkpoint.set_metadata(Some(&after));
        assert!(!checkpoint.is_clean(source));
        assert!(!may_close(
            &mut checkpoint,
            source,
            || CloseChoice::KeepWriting,
            || panic!("must not save")
        ));
        let snapshot = checkpoint.clone();
        assert!(!may_close(
            &mut checkpoint,
            source,
            || CloseChoice::SaveCopy,
            || false
        ));
        assert_eq!(checkpoint, snapshot);
        checkpoint.set_metadata(Some(&before));
        assert!(checkpoint.is_clean(source));
        checkpoint.set_metadata(Some(&after));
        checkpoint.mark_saved(source);
        assert!(checkpoint.is_clean(source));
        checkpoint.set_metadata(Some(&before));
        assert!(
            !checkpoint.is_clean(source),
            "Undo after Save must be dirty"
        );
        checkpoint.set_metadata(Some(&after));
        assert!(
            checkpoint.is_clean(source),
            "Redo to saved metadata must be clean"
        );
        assert!(!checkpoint.is_clean("new text"));
        checkpoint.set_metadata(Some(&before));
        checkpoint.mark_saved("new text");
        assert!(!checkpoint.is_clean(source));
        checkpoint.set_metadata(Some(&after));
        assert!(!checkpoint.is_clean("new text"));
        checkpoint.adopt_saved("source file", None);
        assert!(checkpoint.is_clean("source file"));
        assert!(checkpoint.tracks_metadata(None));
        checkpoint.require_save();
        assert!(!checkpoint.is_clean("source file"));
        checkpoint.mark_saved("source file");
        assert!(checkpoint.is_clean("source file"));
    }
}

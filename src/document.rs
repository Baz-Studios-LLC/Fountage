use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Scene {
    pub line: usize,
    pub heading: String,
}
#[derive(Debug, Default, serde::Serialize)]
pub struct Outline {
    pub scenes: Vec<Scene>,
    pub characters: BTreeMap<String, usize>,
    pub words: usize,
}

// Derived information only: the original Fountain source remains authoritative.
pub fn outline(source: &str) -> Outline {
    let analysis = crate::fountain::analyze(source);
    Outline {
        words: analysis.words,
        characters: analysis.characters,
        scenes: analysis
            .headings
            .into_iter()
            .filter(|heading| heading.kind == crate::writing::Kind::Scene)
            .map(|heading| Scene {
                line: heading.line,
                heading: heading.heading,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scenes_characters_and_extensions() {
        let o = outline(
            "Title: Test\n\nINT. ROOM - DAY\n\nALICE (V.O.)\nHello.\n\n@Bob ^\nHi.\n\n.Alley at midnight\n\nCUT TO:\n\nEXT. ROAD - NIGHT",
        );
        assert_eq!(o.scenes.len(), 3);
        assert_eq!(o.scenes[0].line, 2);
        assert_eq!(
            o.characters.keys().cloned().collect::<Vec<_>>(),
            ["ALICE", "Bob"]
        );
    }
    #[test]
    fn established_scene_prefix_and_lowercase_extension() {
        let o =
            outline("est. HARBOR - DAY\n\nALICE (on the radio)\nHello.\n\n. Not a forced heading");
        assert_eq!(o.scenes.len(), 1);
        assert_eq!(o.characters.get("ALICE"), Some(&1));
    }
    #[test]
    fn ignores_boneyard_and_non_heading_paragraphs() {
        let o = outline(
            "/*\nINT. HIDDEN - NIGHT\n\nGHOST\nBoo.\n*/\n\nAction continues\nINT. NOT A HEADING\n\n...not a scene",
        );
        assert!(o.scenes.is_empty());
        assert!(o.characters.is_empty());
    }
}

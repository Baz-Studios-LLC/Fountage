//! Feed semantic output styles to Slint without reinterpreting rendered text.
use fountage_core::output::Run;
use i_slint_core::styled_text::StyledTextSpan;

pub fn styled(text: &str, runs: &[Run], strong: bool) -> Result<slint::StyledText, &'static str> {
    let ranges = crate::navigation::utf16_ranges(text, runs.iter().map(|r| (r.start, r.length)));
    let mut spans = Vec::with_capacity(runs.len() + usize::from(strong));
    if strong {
        spans.push(StyledTextSpan {
            range: 0..text.len(),
            bold: true,
            italic: false,
            underline: false,
        });
    }
    for (run, range) in runs.iter().zip(ranges) {
        let range = range.ok_or(
            "Preview formatting is unavailable. Showing plain text; your manuscript is unchanged.",
        )?;
        spans.push(StyledTextSpan {
            range,
            bold: run.style.bold,
            italic: run.style.italic,
            underline: run.style.underline,
        });
    }
    slint::StyledText::from_spans(text, &spans).ok_or(
        "Preview formatting is unavailable. Showing plain text; your manuscript is unchanged.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn semantic_styles_keep_literal_text_unicode_and_all_range_coordinates() {
        let source = "INT. ROOM - DAY\r\n\r\n**bold** *italic* _underlined_ ***both*** \\*literal\\* <u>raw</u> &amp; [x](url) {value} 🌙 e\u{301}. [[private]]\r\n";
        let output = fountage_core::output::snapshot(source).output;
        let line = output
            .body
            .iter()
            .find(|l| l.text.contains("bold"))
            .unwrap();
        let value = styled(&line.text, &line.runs, false).unwrap();
        assert_eq!(i_slint_core::styled_text::get_raw_text(&value), line.text);
        let debug = format!("{value:?}");
        assert!(
            debug.contains("Strong") && debug.contains("Emphasis") && debug.contains("Underline")
        );
        assert!(
            line.text
                .contains("*literal* <u>raw</u> &amp; [x](url) {value} 🌙 e\u{301}.")
        );
        assert!(!line.text.contains("private"));
        for literal in [
            "# heading",
            "  indented  ",
            "<font color=\"red\">raw</font>",
            "\u{fffc}\u{e000}",
            "<https://example.invalid> *** _ ~ `",
        ] {
            let value = styled(literal, &[], false).unwrap();
            assert_eq!(i_slint_core::styled_text::get_raw_text(&value), literal);
        }
    }
    #[test]
    fn invalid_surrogates_and_ranges_fail_and_untouched_text_is_available_for_fallback() {
        let invalid = Run {
            start: 1,
            length: 1,
            style: fountage_core::output::Style {
                bold: true,
                ..Default::default()
            },
        };
        assert!(styled("🌙", &[invalid], false).is_err());
        for range in [1..4, 0..5] {
            assert!(
                slint::StyledText::from_spans(
                    "🌙",
                    &[StyledTextSpan {
                        range,
                        bold: true,
                        italic: false,
                        underline: false
                    }]
                )
                .is_none()
            );
        }
    }
}

// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
//! Preserve source hard breaks in the accessible TextRun, without shaping CRLF twice.
extern crate alloc;
use accesskit::Node;
use alloc::string::ToString;

pub(super) fn append(node: &mut Node, separator: &str) {
    debug_assert!(matches!(separator, "\n" | "\r\n"));
    let mut value = node.value().map(ToString::to_string).unwrap_or_default();
    let mut lengths = node.character_lengths().to_vec();
    let mut widths = node
        .character_widths()
        .map(<[_]>::to_vec)
        .unwrap_or_default();
    let mut positions = node
        .character_positions()
        .map(<[_]>::to_vec)
        .unwrap_or_default();
    let last_x = positions.last().copied().unwrap_or(0.0) + widths.last().copied().unwrap_or(0.0);
    value.push_str(separator);
    lengths.push(separator.len() as u8);
    positions.push(last_x);
    widths.push(0.0);
    node.set_value(value);
    node.set_character_lengths(lengths);
    node.set_character_widths(widths);
    node.set_character_positions(positions);
}

#[cfg(test)]
mod tests {
    use super::*;
    use accesskit::Role;
    #[test]
    fn source_breaks_keep_accessible_utf16_offsets_and_character_metrics_aligned() {
        for separators in [
            ["\n", "\n", "\n"],
            ["\r\n", "\r\n", "\r\n"],
            ["\r\n", "\n", "\r\n"],
        ] {
            let paragraphs = ["Café 🌙", "", "e\u{301} 世界"];
            let mut emitted = alloc::string::String::new();
            let mut expected = alloc::string::String::new();
            for (text, separator) in paragraphs.into_iter().zip(separators) {
                let mut node = Node::new(Role::TextRun);
                node.set_value(text);
                node.set_character_lengths(
                    text.chars()
                        .map(|c| c.len_utf8() as u8)
                        .collect::<alloc::vec::Vec<_>>(),
                );
                node.set_character_positions(alloc::vec![0.; text.chars().count()]);
                node.set_character_widths(alloc::vec![1.; text.chars().count()]);
                append(&mut node, separator);
                emitted.push_str(node.value().unwrap());
                expected.push_str(text);
                expected.push_str(separator);
                assert_eq!(
                    node.character_lengths()
                        .iter()
                        .map(|n| *n as usize)
                        .sum::<usize>(),
                    node.value().unwrap().len()
                );
                assert_eq!(
                    node.character_lengths().last().copied(),
                    Some(separator.len() as u8)
                );
                assert_eq!(
                    node.character_lengths().len(),
                    node.character_positions().unwrap().len()
                );
                assert_eq!(node.character_widths().unwrap().last(), Some(&0.));
            }
            assert_eq!(emitted, expected);
            assert_eq!(
                emitted.encode_utf16().count(),
                expected.encode_utf16().count()
            );
        }
    }
}

//! Source search. Offsets always refer to the exact current UTF-8 source.
use regex::{Regex, RegexBuilder};
use std::borrow::Cow;
use std::ops::Range;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub match_case: bool,
    pub whole_word: bool,
    pub regex: bool,
    pub starts_with: bool,
    pub wrap: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            match_case: false,
            whole_word: false,
            regex: false,
            starts_with: false,
            wrap: true,
        }
    }
}
impl Options {
    fn same_pattern(self, other: Self) -> bool {
        self.match_case == other.match_case
            && self.whole_word == other.whole_word
            && self.regex == other.regex
            && self.starts_with == other.starts_with
    }
}
impl From<bool> for Options {
    fn from(match_case: bool) -> Self {
        Self {
            match_case,
            ..Default::default()
        }
    }
}
fn pattern(query: &str, options: Options) -> Result<Regex, String> {
    let literal = if options.regex {
        query.to_owned()
    } else {
        regex::escape(query)
    };
    // Unicode word boundaries include letters, marks, digits and underscores.
    // This is boundary matching, not a linguistic tokenizer (e.g. for CJK).
    let expression = if options.whole_word {
        format!(r"\b(?:{literal})\b")
    } else if options.starts_with {
        format!(r"\b(?:{literal})")
    } else {
        literal
    };
    RegexBuilder::new(&expression)
        .case_insensitive(!options.match_case)
        .size_limit(2 * 1024 * 1024)
        .build()
        .map_err(|_| "Invalid or unsupported search pattern (or pattern too large). Check the expression; look-around and backreferences are not supported.".to_owned())
}

#[derive(Default)]
pub struct Search {
    source: String,
    query: String,
    options: Options,
    pattern: Option<Regex>,
    current: Option<usize>,
}
#[derive(Debug, PartialEq)]
pub struct Match {
    pub range: Range<usize>,
    pub ordinal: usize,
    pub count: usize,
}
impl Search {
    pub fn has_input(&self, source: &str, query: &str, options: impl Into<Options>) -> bool {
        self.source == source && self.query == query && self.options == options.into()
    }

    pub fn replaced(&mut self, source: &str, query: &str, options: impl Into<Options>) {
        self.source = source.into();
        self.query = query.into();
        self.options = options.into();
        self.pattern = None;
        self.current = None;
    }

    pub fn replace_current(
        &self,
        source: &str,
        query: &str,
        options: impl Into<Options>,
        replacement: &str,
    ) -> Result<Option<Replacement>, String> {
        if !self.has_input(source, query, options) {
            return Err("The draft or search changed. Find a match again before replacing.".into());
        }
        let (Some(pattern), Some(index)) = (&self.pattern, self.current) else {
            return Err("Find a match before replacing.".into());
        };
        plan_matches(
            source,
            pattern,
            replacement,
            self.options.regex,
            Some(index),
        )
    }

    pub fn step(
        &mut self,
        source: &str,
        query: &str,
        options: impl Into<Options>,
        backwards: bool,
    ) -> Result<Option<Match>, String> {
        let options = options.into();
        if self.query != query || !self.options.same_pattern(options) {
            self.pattern = None;
            self.current = None;
            self.query = query.into();
        }
        self.options = options;
        if self.source != source {
            self.source = source.into();
            self.current = None;
        }
        if query.is_empty() {
            self.current = None;
            return Ok(None);
        }
        if self.pattern.is_none() {
            self.pattern = Some(pattern(query, options)?);
        }
        let pattern = self.pattern.as_ref().unwrap();
        // Count without retaining a range per hit in manuscripts with many matches.
        let count = pattern.find_iter(source).count();
        if count == 0 {
            self.current = None;
            return Ok(None);
        }
        let index = match self.current {
            None => {
                if backwards {
                    count - 1
                } else {
                    0
                }
            }
            Some(0) if backwards && !options.wrap => {
                return Err("Reached the beginning of the draft".into());
            }
            Some(i) if !backwards && i + 1 == count && !options.wrap => {
                return Err("Reached the end of the draft".into());
            }
            Some(0) if backwards => count - 1,
            Some(i) if backwards => i - 1,
            Some(i) => (i + 1) % count,
        };
        let found = pattern.find_iter(source).nth(index).unwrap();
        self.current = Some(index);
        Ok(Some(Match {
            range: found.range(),
            ordinal: index + 1,
            count,
        }))
    }

    /// Continue after a completed replacement, skipping the inserted span.
    /// A zero-width match must also skip its relocated position after insertion.
    pub fn continue_after(
        &mut self,
        source: &str,
        query: &str,
        options: Options,
        offset: usize,
        skip_equal: bool,
    ) -> Result<Option<Match>, String> {
        if !source.is_char_boundary(offset) {
            return Err("Invalid search continuation offset.".into());
        }
        let compiled = pattern(query, options)?;
        let mut count = 0;
        let mut first = None;
        let mut next = None;
        for found in compiled.find_iter(source) {
            let range = found.range();
            if first.is_none() {
                first = Some(range.clone());
            }
            if next.is_none() && (range.start > offset || (!skip_equal && range.start == offset)) {
                next = Some((count, range));
            }
            count += 1;
        }
        self.replaced(source, query, options);
        self.pattern = Some(compiled);
        if count == 0 {
            return Ok(None);
        }
        let selected = next.or_else(|| options.wrap.then(|| (0, first.unwrap())));
        let Some((index, range)) = selected else {
            self.current = Some(count - 1);
            return Err("Reached the end of the draft".into());
        };
        self.current = Some(index);
        Ok(Some(Match {
            range,
            ordinal: index + 1,
            count,
        }))
    }
}
#[derive(Debug, PartialEq)]
pub struct Replacement {
    pub source: String,
    pub selection: Range<usize>,
    pub count: usize,
    pub first_match_empty: bool,
}

pub fn replace_all(
    source: &str,
    query: &str,
    options: impl Into<Options>,
    replacement: &str,
) -> Result<Option<Replacement>, String> {
    if query.is_empty() {
        return Err("Enter text to search before replacing.".into());
    }
    let options = options.into();
    let pattern = pattern(query, options)?;
    plan_matches(source, &pattern, replacement, options.regex, None)
}

enum Part<'a> {
    Literal(&'a str),
    Group(usize),
}
fn template<'a>(value: &'a str, pattern: &Regex) -> Result<Vec<Part<'a>>, String> {
    let mut parts = Vec::new();
    let mut remaining = value;
    while let Some(dollar) = remaining.find('$') {
        parts.push(Part::Literal(&remaining[..dollar]));
        remaining = &remaining[dollar + 1..];
        if let Some(rest) = remaining.strip_prefix('$') {
            parts.push(Part::Literal("$"));
            remaining = rest;
            continue;
        }
        let (name, rest) = if let Some(braced) = remaining.strip_prefix('{') {
            let end = braced.find('}').ok_or(
                "Unclosed replacement capture. Use ${name}, $1 or $$ for a literal dollar.",
            )?;
            (&braced[..end], &braced[end + 1..])
        } else {
            let length = remaining
                .bytes()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == b'_')
                .count();
            if length == 0 {
                parts.push(Part::Literal("$"));
                continue;
            }
            (&remaining[..length], &remaining[length..])
        };
        let group = name
            .parse::<usize>()
            .ok()
            .filter(|n| *n < pattern.captures_len())
            .or_else(|| {
                pattern
                    .capture_names()
                    .position(|candidate| candidate == Some(name))
            })
            .ok_or_else(|| {
                format!("Unknown replacement capture: ${{{name}}}. Use $$ for a literal dollar.")
            })?;
        parts.push(Part::Group(group));
        remaining = rest;
    }
    parts.push(Part::Literal(remaining));
    Ok(parts)
}
fn plan_matches(
    source: &str,
    pattern: &Regex,
    replacement: &str,
    regex: bool,
    only: Option<usize>,
) -> Result<Option<Replacement>, String> {
    if replacement.contains('\0') {
        return Err("Replacement cannot contain NUL characters.".into());
    }
    let skip = only.unwrap_or(0);
    let take = if only.is_some() { 1 } else { usize::MAX };
    if !regex {
        return replacement_plan(
            source,
            pattern
                .find_iter(source)
                .skip(skip)
                .take(take)
                .map(|found| Ok((found.range(), Cow::Borrowed(replacement)))),
        );
    }
    let parts = template(replacement, pattern)?;
    let replacements = pattern
        .captures_iter(source)
        .skip(skip)
        .take(take)
        .map(|captures| {
            let range = captures.get(0).unwrap().range();
            let mut expanded = String::new();
            for part in &parts {
                let piece = match part {
                    Part::Literal(value) => value,
                    Part::Group(index) => captures.get(*index).map_or("", |m| m.as_str()),
                };
                expanded
                    .len()
                    .checked_add(piece.len())
                    .filter(|n| *n <= i32::MAX as usize)
                    .ok_or("Replacement exceeds the editor's supported size.")?;
                expanded.push_str(piece);
            }
            Ok((range, Cow::Owned(expanded)))
        });
    replacement_plan(source, replacements)
}
fn replacement_plan<'a>(
    source: &str,
    replacements: impl Iterator<Item = Result<(Range<usize>, Cow<'a, str>), String>>,
) -> Result<Option<Replacement>, String> {
    let mut result = String::new();
    let mut end = 0;
    let mut count = 0;
    let mut selection = 0..0;
    let mut first_match_empty = false;
    for replacement in replacements {
        let (range, replacement) = replacement?;
        let prefix = &source[end..range.start];
        let new_len = result
            .len()
            .checked_add(prefix.len())
            .and_then(|n| n.checked_add(replacement.len()))
            .filter(|n| *n <= i32::MAX as usize)
            .ok_or("Replacement exceeds the editor's supported size.")?;
        result.push_str(prefix);
        if count == 0 {
            selection = result.len()..new_len;
            first_match_empty = range.is_empty();
        }
        result.push_str(&replacement);
        end = range.end;
        count += 1;
    }
    if count == 0 {
        return Ok(None);
    }
    result
        .len()
        .checked_add(source.len() - end)
        .filter(|n| *n <= i32::MAX as usize)
        .ok_or("Replacement exceeds the editor's supported size.")?;
    result.push_str(&source[end..]);
    Ok(Some(Replacement {
        source: result,
        selection,
        count,
        first_match_empty,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacement_continuation_skips_inserted_matches_and_handles_deletions_and_empty_matches() {
        let options = Options {
            wrap: false,
            ..Default::default()
        };
        let mut search = Search::default();
        let source = "🌙 cat cat cat\r\n";
        search.step(source, "cat", options, false).unwrap();
        let plan = search
            .replace_current(source, "cat", options, "catcat")
            .unwrap()
            .unwrap();
        let next = search
            .continue_after(
                &plan.source,
                "cat",
                options,
                plan.selection.end,
                plan.first_match_empty,
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            next.ordinal, 3,
            "skip both new matches inside inserted text"
        );
        assert_eq!(&plan.source[next.range], "cat");
        assert_eq!(
            search
                .replace_current(&plan.source, "cat", options, "dog")
                .unwrap()
                .unwrap()
                .source,
            "🌙 catcat dog cat\r\n"
        );
        search.step("catcat", "cat", options, false).unwrap();
        let plan = search
            .replace_current("catcat", "cat", options, "")
            .unwrap()
            .unwrap();
        assert_eq!(
            search
                .continue_after(
                    &plan.source,
                    "cat",
                    options,
                    plan.selection.end,
                    plan.first_match_empty
                )
                .unwrap()
                .unwrap()
                .range,
            0..3,
            "deletion includes the next match relocated to the same offset"
        );
        let regex = Options {
            regex: true,
            ..options
        };
        let source = "é\n世界";
        search.step(source, "(?m)^", regex, false).unwrap();
        let plan = search
            .replace_current(source, "(?m)^", regex, "")
            .unwrap()
            .unwrap();
        assert!(plan.first_match_empty);
        assert_eq!(
            search
                .continue_after(&plan.source, "(?m)^", regex, 0, true)
                .unwrap()
                .unwrap()
                .range,
            3..3
        );
        let plan = search
            .replace_current(source, "(?m)^", regex, ">")
            .unwrap()
            .unwrap();
        assert_eq!(
            search
                .continue_after(&plan.source, "(?m)^", regex, plan.selection.end, true)
                .unwrap_err(),
            "Reached the end of the draft"
        );
        assert!(
            search
                .continue_after(source, "cat", options, 1, false)
                .is_err(),
            "reject offset within UTF-8 scalar"
        );
    }
    #[test]
    fn prefix_mode_and_optional_wrap_preserve_navigation_at_boundaries() {
        let source = "ant giant antagonize ant2 _ant ant_";
        let prefix = Options {
            starts_with: true,
            ..Default::default()
        };
        let mut search = Search::default();
        assert_eq!(
            search
                .step(source, "ant", false, false)
                .unwrap()
                .unwrap()
                .count,
            6
        );
        assert_eq!(
            search
                .step(source, "ant", prefix, false)
                .unwrap()
                .unwrap()
                .count,
            4
        );
        assert_eq!(
            replace_all(source, "ant", prefix, "x")
                .unwrap()
                .unwrap()
                .source,
            "x giant xagonize x2 _ant x_"
        );
        let bounded = Options {
            wrap: false,
            ..prefix
        };
        assert_eq!(
            search
                .step(source, "ant", bounded, false)
                .unwrap()
                .unwrap()
                .ordinal,
            2,
            "wrap toggle preserves current match"
        );
        assert_eq!(
            search
                .step(source, "ant", bounded, false)
                .unwrap()
                .unwrap()
                .ordinal,
            3
        );
        assert_eq!(
            search
                .step(source, "ant", bounded, false)
                .unwrap()
                .unwrap()
                .ordinal,
            4
        );
        assert_eq!(
            search.step(source, "ant", bounded, false).unwrap_err(),
            "Reached the end of the draft"
        );
        assert_eq!(
            search
                .step(source, "ant", prefix, false)
                .unwrap()
                .unwrap()
                .ordinal,
            1
        );
        assert_eq!(
            search.step(source, "ant", bounded, true).unwrap_err(),
            "Reached the beginning of the draft"
        );
        assert_eq!(
            search
                .step(source, "ant", prefix, true)
                .unwrap()
                .unwrap()
                .ordinal,
            4
        );
        assert!(
            search.replace_current(source, "ant", false, "x").is_err(),
            "mode changes invalidate targets"
        );
    }
    #[test]
    fn regex_captures_replace_notes_with_exact_unicode_and_line_endings() {
        let options = Options {
            regex: true,
            match_case: true,
            ..Default::default()
        };
        let source = "🌙 [[Café e\u{301}]]\r\n[[世界]]\r\n";
        let query = r"\[\[(?P<note>.*?)\]\]";
        let mut search = Search::default();
        let first = search.step(source, query, options, false).unwrap().unwrap();
        assert_eq!(first.count, 2);
        search.step(source, query, options, false).unwrap();
        let single = search
            .replace_current(source, query, options, "**${note}** $$")
            .unwrap()
            .unwrap();
        assert_eq!(single.source, "🌙 [[Café e\u{301}]]\r\n**世界** $\r\n");
        let all = replace_all(source, query, options, "**$1**")
            .unwrap()
            .unwrap();
        assert_eq!(all.source, "🌙 **Café e\u{301}**\r\n**世界**\r\n");
        assert_eq!(&all.source[all.selection], "**Café e\u{301}**");
        assert!(search.replace_current(source, query, true, "$1").is_err());
        assert!(replace_all(source, query, options, "$missing").is_err());
        assert!(replace_all(source, query, options, "${note").is_err());
        assert!(replace_all(source, query, options, "$999999999999999999999").is_err());
        assert!(replace_all(source, "[", options, "x").is_err());
        assert!(replace_all(source, "(?=x)", options, "x").is_err());
        assert_eq!(
            replace_all("b ab", "(a)?b", options, "<$1>")
                .unwrap()
                .unwrap()
                .source,
            "<> <a>"
        );
        assert_eq!(
            replace_all("世界", "(世界)", options, "$0|${1}|$$|\\")
                .unwrap()
                .unwrap()
                .source,
            "世界|世界|$|\\"
        );
    }
    #[test]
    fn zero_width_regex_terminates_at_unicode_boundaries_and_preserves_captures_in_word_mode() {
        let options = Options {
            regex: true,
            match_case: true,
            ..Default::default()
        };
        let source = "é\n世界";
        let all = replace_all(source, "(?m)^", options, ">").unwrap().unwrap();
        assert_eq!(all.count, 2);
        assert_eq!(all.source, ">é\n>世界");
        let mut search = Search::default();
        let first = search
            .step(source, "(?m)^", options, false)
            .unwrap()
            .unwrap();
        assert_eq!(first.range, 0..0);
        assert_eq!(
            search
                .step(source, "(?m)^", options, true)
                .unwrap()
                .unwrap()
                .range,
            3..3
        );
        let words = Options {
            whole_word: true,
            ..options
        };
        assert_eq!(
            replace_all("MARA MARATHON", "(MARA)", words, "$1!")
                .unwrap()
                .unwrap()
                .source,
            "MARA! MARATHON"
        );
    }
    #[test]
    fn whole_word_find_replace_and_option_changes_share_unicode_boundaries() {
        let words = Options {
            match_case: false,
            whole_word: true,
            regex: false,
            ..Default::default()
        };
        let source = "🌙 ant antagonize ANT ant2 _ant ant_ ant-ant (ant)\r\n";
        let mut search = Search::default();
        let first = search.step(source, "ant", words, false).unwrap().unwrap();
        assert_eq!(&source[first.range], "ant");
        assert_eq!(first.count, 5);
        let last = search.step(source, "ant", words, true).unwrap().unwrap();
        assert_eq!(last.ordinal, 5);
        let changed = replace_all(source, "ant", words, "$1").unwrap().unwrap();
        assert_eq!(changed.count, 5);
        assert_eq!(
            changed.source,
            "🌙 $1 antagonize $1 ant2 _ant ant_ $1-$1 ($1)\r\n"
        );
        assert!(
            search.replace_current(source, "ant", false, "x").is_err(),
            "changing whole-word mode invalidates the prior replacement target"
        );
        let substring = search.step(source, "ant", false, false).unwrap().unwrap();
        assert_eq!(substring.ordinal, 1);
        assert!(substring.count > 5);
        let sensitive = Options {
            match_case: true,
            ..words
        };
        assert_eq!(
            search
                .step(source, "ant", sensitive, false)
                .unwrap()
                .unwrap()
                .count,
            4
        );
        for (source, query) in [
            ("café cafétéria", "café"),
            ("e\u{301} e\u{301}lan", "e\u{301}"),
            ("世界 世界観", "世界"),
        ] {
            let matched = search.step(source, query, words, false).unwrap().unwrap();
            assert_eq!(matched.count, 1);
            assert_eq!(&source[matched.range], query);
        }
    }
    #[test]
    fn replacements_preserve_unicode_crlf_and_treat_dollars_literally() {
        let source = "🌙 Zoë\r\nzoË / Zoë";
        let mut search = Search::default();
        search.step(source, "zoë", false, false).unwrap();
        search.step(source, "zoë", false, false).unwrap();
        let single = search
            .replace_current(source, "zoë", false, "$1\\世界")
            .unwrap()
            .unwrap();
        assert_eq!(single.source, "🌙 Zoë\r\n$1\\世界 / Zoë");
        assert_eq!(&single.source[single.selection], "$1\\世界");
        assert_eq!(single.count, 1);
        let all = replace_all(source, "zoë", false, "✨").unwrap().unwrap();
        assert_eq!(all.source, "🌙 ✨\r\n✨ / ✨");
        assert_eq!(all.count, 3);
        assert_eq!(&all.source[all.selection], "✨");
        assert_eq!(
            replace_all(source, "Zoë", true, "")
                .unwrap()
                .unwrap()
                .source,
            "🌙 \r\nzoË / "
        );
        assert_eq!(
            replace_all("aaaa", "aa", true, "aaa")
                .unwrap()
                .unwrap()
                .source,
            "aaaaaa"
        );
    }
    #[test]
    fn stale_and_invalid_replacement_plans_cannot_mutate_source() {
        let source = "Zoë\r\nZoë";
        let mut search = Search::default();
        assert!(search.replace_current(source, "Zoë", true, "x").is_err());
        search.step(source, "Zoë", true, false).unwrap();
        assert!(search.replace_current("changed", "Zoë", true, "x").is_err());
        assert!(search.replace_current(source, "other", true, "x").is_err());
        assert!(search.replace_current(source, "Zoë", false, "x").is_err());
        assert!(
            search
                .replace_current(source, "Zoë", true, "bad\0text")
                .is_err()
        );
        assert!(replace_all(source, "", true, "x").is_err());
        assert!(replace_all(source, "Zoë", true, "\0").is_err());
        assert!(replace_all(source, "missing", true, "x").unwrap().is_none());
        let same = replace_all(source, "Zoë", true, "Zoë").unwrap().unwrap();
        assert_eq!(same.source, source);
        search.replaced(source, "Zoë", true);
        assert!(search.replace_current(source, "Zoë", true, "x").is_err());
    }
    #[test]
    fn unicode_case_literal_and_wrap_preserve_byte_positions() {
        let s = "🌙 Zoë\r\nzoË [.*] Zoë";
        let mut search = Search::default();
        let first = search.step(s, "zoë", false, false).unwrap().unwrap();
        assert_eq!(&s[first.range.clone()], "Zoë");
        assert_eq!((first.ordinal, first.count), (1, 3));
        let second = search.step(s, "zoë", false, false).unwrap().unwrap();
        assert_eq!(&s[second.range], "zoË");
        assert_eq!(search.step(s, "zoë", false, true).unwrap().unwrap(), first);
        assert_eq!(
            search.step(s, "zoë", false, true).unwrap().unwrap().ordinal,
            3
        );
        assert_eq!(
            search.step(s, "Zoë", true, false).unwrap().unwrap().count,
            2
        );
        let literal = search.step(s, "[.*]", true, false).unwrap().unwrap();
        assert_eq!(&s[literal.range], "[.*]");
    }
    #[test]
    fn edits_query_changes_missing_empty_and_reverse_first_reset_navigation() {
        let mut search = Search::default();
        assert_eq!(
            search.step("aaa", "a", true, true).unwrap().unwrap().range,
            2..3
        );
        assert_eq!(
            search.step("aaa", "a", true, false).unwrap().unwrap().range,
            0..1
        );
        assert_eq!(
            search
                .step("prefix aaa", "a", true, false)
                .unwrap()
                .unwrap()
                .range,
            7..8
        );
        assert!(
            search
                .step("prefix aaa", "missing", true, false)
                .unwrap()
                .is_none()
        );
        assert!(
            search
                .step("prefix aaa", "", true, false)
                .unwrap()
                .is_none()
        );
        assert!(search.step("", "a", true, false).unwrap().is_none());
        assert_eq!(
            search
                .step("aaaa", "aa", true, false)
                .unwrap()
                .unwrap()
                .count,
            2
        );
    }
}

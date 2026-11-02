//! Experimental source-preserving formatted layout; not the production input widget.
//! The same shaped geometry serves rendering, hit testing and caret placement.
use fountage_core::writing::{self, Format, Kind};
use parley::{
    Affinity, Alignment, AlignmentOptions, BoundingBox, FontContext, FontFamily, FontStyle,
    FontWeight, Layout, LayoutContext, StyleProperty, editing::Cursor,
};
use std::{
    borrow::Cow,
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    ops::Range,
    rc::Rc,
};
use unicode_segmentation::UnicodeSegmentation;

pub struct Context {
    fonts: FontContext,
    layouts: LayoutContext<()>,
}
impl Default for Context {
    fn default() -> Self {
        Self {
            fonts: FontContext::new(),
            layouts: LayoutContext::new(),
        }
    }
}
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct LocalStyle {
    range: Range<usize>,
    bold: bool,
    italic: bool,
    underline: bool,
}
pub struct Paragraph {
    pub source: Range<usize>, // includes original LF/CRLF, never synthetic padding
    pub content: Range<usize>,
    pub kind: Kind,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub layout: Rc<Layout<()>>,
    styles: Vec<LocalStyle>,
    signature: u64,
}
pub struct Document {
    source: String,
    graphemes: Vec<usize>,
    pub paragraphs: Vec<Paragraph>,
    pub height: f32,
    pub width: f32,
    pub shaped_paragraphs: usize,
    pub reused_paragraphs: usize,
    pub rewrapped_paragraphs: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Position {
    pub byte: usize,
    pub upstream: bool,
}
pub struct Caret {
    pub byte: usize,
    pub rect: BoundingBox,
}

impl Context {
    pub fn shape(&mut self, source: &str, width: f32) -> Result<Document, &'static str> {
        self.reshape(source, width, None)
    }
    pub fn reshape(
        &mut self,
        source: &str,
        width: f32,
        previous: Option<&Document>,
    ) -> Result<Document, &'static str> {
        if !width.is_finite() || !(120. ..=10_000.).contains(&width) {
            return Err("Layout width must be finite and between 120 and 10000 logical pixels");
        }
        let analysis = writing::analyze(source, Format::Screenplay);
        let mut ranges = Vec::new();
        let mut start = 0;
        for line in source.split_inclusive('\n') {
            let end = start + line.len();
            let content_end = if line.ends_with("\r\n") {
                end - 2
            } else if line.ends_with('\n') {
                end - 1
            } else {
                end
            };
            ranges.push((start..end, start..content_end));
            start = end;
        }
        if source.is_empty() || source.ends_with('\n') {
            ranges.push((source.len()..source.len(), source.len()..source.len()));
        }
        let mut kinds = vec![Kind::Body; ranges.len()];
        for element in &analysis.elements {
            if let Some(kind) = kinds.get_mut(element.line) {
                *kind = element.kind;
            }
        }
        let emphasis = crate::navigation::utf16_ranges(
            source,
            analysis.emphasis.iter().map(|e| (e.start, e.length)),
        );
        // Emphasis is line-local. Sort once, then consume each span in its
        // paragraph; avoid rescanning every style for every source line.
        let mut styled: Vec<_> = analysis
            .emphasis
            .iter()
            .zip(emphasis)
            .filter_map(|(style, range)| Some((style, range?)))
            .collect();
        styled.sort_by_key(|(_, range)| (range.start, range.end));
        let mut reuse: HashMap<u64, Vec<&Paragraph>> = HashMap::new();
        if let Some(previous) = previous {
            for p in &previous.paragraphs {
                reuse.entry(p.signature).or_default().push(p);
            }
        }
        let mut shaped_paragraphs = 0;
        let mut reused_paragraphs = 0;
        let mut rewrapped_paragraphs = 0;
        let mut next_style = 0;
        let mut paragraphs = Vec::with_capacity(ranges.len());
        let mut y = 0.;
        for (index, (full, content)) in ranges.into_iter().enumerate() {
            let kind = kinds[index];
            // Initial editor geometry follows the existing native reference.
            // These proportions are not a certified print/pagination template.
            let (left, right, alignment) = match kind {
                Kind::Character => (0.36, 0., Alignment::Start),
                Kind::Dialogue => (0.20, 0.18, Alignment::Start),
                Kind::Parenthetical => (0.29, 0.23, Alignment::Start),
                Kind::Transition => (0., 0., Alignment::End),
                Kind::Centered | Kind::PageBreak => (0., 0., Alignment::Center),
                _ => (0., 0., Alignment::Start),
            };
            let x = width * left;
            let available = width * (1. - left - right);
            let text = &source[content.clone()];
            let mut styles = Vec::new();
            while next_style < styled.len() && styled[next_style].1.start < content.end {
                let (span, range) = &styled[next_style];
                next_style += 1;
                let a = range.start.max(content.start);
                let b = range.end.min(content.end);
                if a >= b {
                    continue;
                }
                styles.push(LocalStyle {
                    range: a - content.start..b - content.start,
                    bold: span.bold,
                    italic: span.italic,
                    underline: span.underline,
                });
            }
            let mut hasher = DefaultHasher::new();
            text.hash(&mut hasher);
            (kind as u8).hash(&mut hasher);
            styles.hash(&mut hasher);
            let signature = hasher.finish();
            // A hash narrows candidates only. Exact equality guards collisions,
            // moved source offsets, contextual role changes and style changes.
            let cached = previous.and_then(|old| {
                reuse.get(&signature).and_then(|candidates| {
                    candidates.iter().find(|p| {
                        p.kind == kind
                            && p.styles == styles
                            && old.source[p.content.clone()] == *text
                    })
                })
            });
            let layout = if let Some(cached) = cached {
                if cached.width.to_bits() == available.to_bits() {
                    reused_paragraphs += 1;
                    Rc::clone(&cached.layout)
                } else {
                    // Rebreak a private copy of immutable shaped glyphs. Never
                    // mutate a layout still referenced by the previous document.
                    rewrapped_paragraphs += 1;
                    let mut layout = cached.layout.as_ref().clone();
                    layout.break_all_lines(Some(available));
                    layout.align(alignment, AlignmentOptions::default());
                    Rc::new(layout)
                }
            } else {
                shaped_paragraphs += 1;
                let mut builder = self.layouts.ranged_builder(&mut self.fonts, text, 1., true);
                builder.push_default(StyleProperty::FontFamily(FontFamily::Source(
                    Cow::Borrowed("Courier, monospace"),
                )));
                builder.push_default(StyleProperty::FontSize(16.));
                if matches!(kind, Kind::Scene | Kind::Section) {
                    builder.push_default(StyleProperty::FontWeight(FontWeight::new(700.)));
                }
                if matches!(kind, Kind::Synopsis | Kind::Lyric) {
                    builder.push_default(StyleProperty::FontStyle(FontStyle::Italic));
                }
                for span in &styles {
                    if span.bold {
                        builder.push(
                            StyleProperty::FontWeight(FontWeight::new(700.)),
                            span.range.clone(),
                        );
                    }
                    if span.italic {
                        builder.push(
                            StyleProperty::FontStyle(FontStyle::Italic),
                            span.range.clone(),
                        );
                    }
                    if span.underline {
                        builder.push(StyleProperty::Underline(true), span.range.clone());
                    }
                }
                let mut layout = builder.build(text);
                layout.break_all_lines(Some(available));
                layout.align(alignment, AlignmentOptions::default());
                Rc::new(layout)
            };
            let height = layout.height().max(24.);
            paragraphs.push(Paragraph {
                source: full,
                content,
                kind,
                x,
                y,
                width: available,
                height,
                layout,
                styles,
                signature,
            });
            y += height + 5.;
        }
        Ok(Document {
            source: source.into(),
            graphemes: source
                .grapheme_indices(true)
                .map(|(byte, _)| byte)
                .chain(std::iter::once(source.len()))
                .collect(),
            paragraphs,
            height: y,
            width,
            shaped_paragraphs,
            reused_paragraphs,
            rewrapped_paragraphs,
        })
    }
}
impl Document {
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn boundaries(&self) -> &[usize] {
        &self.graphemes
    }
    pub fn hit_test(&self, x: f32, y: f32) -> Option<usize> {
        self.hit_position(x, y).map(|p| p.byte)
    }
    pub fn hit_position(&self, x: f32, y: f32) -> Option<Position> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        let index = self
            .paragraphs
            .partition_point(|p| p.y + p.height + 5. <= y)
            .min(self.paragraphs.len() - 1);
        let p = &self.paragraphs[index];
        let cursor = Cursor::from_point(&p.layout, x - p.x, (y - p.y).max(0.).min(p.height));
        let byte = p.content.start + cursor.index().min(p.content.len());
        let normalized = self.graphemes[self
            .graphemes
            .partition_point(|&b| b <= byte)
            .saturating_sub(1)];
        Some(Position {
            byte: normalized,
            upstream: normalized == byte && cursor.affinity() == Affinity::Upstream,
        })
    }
    pub fn caret(&self, byte: usize) -> Option<Caret> {
        self.caret_at(Position {
            byte,
            upstream: false,
        })
    }
    pub fn caret_at(&self, position: Position) -> Option<Caret> {
        let byte = position.byte;
        if !self.source.is_char_boundary(byte) {
            return None;
        }
        let index = self
            .paragraphs
            .partition_point(|p| p.source.end <= byte && p.source.end != self.source.len())
            .min(self.paragraphs.len() - 1);
        // At EOF after a newline the final empty paragraph owns the caret.
        let index = if byte == self.source.len() && self.source.ends_with('\n') {
            self.paragraphs.len() - 1
        } else {
            index
        };
        let p = &self.paragraphs[index];
        if byte > p.content.end {
            return None;
        } // CRLF is one indivisible line ending.
        let byte = self.graphemes[self
            .graphemes
            .partition_point(|&b| b <= byte)
            .saturating_sub(1)];
        let cursor = Cursor::from_byte_index(
            &p.layout,
            byte - p.content.start,
            if position.upstream {
                Affinity::Upstream
            } else {
                Affinity::Downstream
            },
        );
        let mut rect = cursor.geometry(&p.layout, 1.);
        rect.x0 += p.x as f64;
        rect.x1 += p.x as f64;
        rect.y0 += p.y as f64;
        rect.y1 += p.y as f64;
        if rect.height() <= 0. {
            rect.y1 = rect.y0 + 24.;
        }
        Some(Caret {
            byte: p.content.start + cursor.index(),
            rect,
        })
    }
    fn cursor_at(&self, position: Position) -> Option<(usize, Cursor)> {
        if self.caret_at(position)?.byte != position.byte {
            return None;
        }
        let index = self
            .paragraphs
            .partition_point(|p| p.content.start <= position.byte)
            .saturating_sub(1);
        let p = &self.paragraphs[index];
        Some((
            index,
            Cursor::from_byte_index(
                &p.layout,
                position.byte - p.content.start,
                if position.upstream {
                    Affinity::Upstream
                } else {
                    Affinity::Downstream
                },
            ),
        ))
    }
    pub fn horizontal(&self, position: Position, right: bool) -> Option<Position> {
        self.horizontal_by(position, right, false)
    }
    pub fn word(&self, position: Position, right: bool) -> Option<Position> {
        self.horizontal_by(position, right, true)
    }
    /// Deletion follows contiguous logical source, independently of bidi visual movement.
    pub fn deletion_word_boundary(&self, position: Position, forward: bool) -> Option<usize> {
        let (index, cursor) = self.cursor_at(position)?;
        let paragraph = &self.paragraphs[index];
        let next = if forward {
            cursor.next_logical_word(&paragraph.layout)
        } else {
            cursor.previous_logical_word(&paragraph.layout)
        };
        let byte = paragraph.content.start + next.index();
        if byte == position.byte {
            return Some(if forward {
                self.paragraphs
                    .get(index + 1)
                    .map_or(position.byte, |next| next.content.start)
            } else {
                index
                    .checked_sub(1)
                    .and_then(|index| self.paragraphs.get(index))
                    .map_or(position.byte, |previous| previous.content.end)
            });
        }
        let boundary = if forward {
            self.graphemes.partition_point(|&boundary| boundary < byte)
        } else {
            self.graphemes
                .partition_point(|&boundary| boundary <= byte)
                .saturating_sub(1)
        };
        self.graphemes.get(boundary).copied().filter(|&target| {
            if forward {
                target >= position.byte
            } else {
                target <= position.byte
            }
        })
    }
    fn horizontal_by(&self, position: Position, right: bool, word: bool) -> Option<Position> {
        let (index, mut cursor) = self.cursor_at(position)?;
        let p = &self.paragraphs[index];
        // Font fallback can produce clusters inside one Unicode grapheme.
        // Skip those without losing the shaped cursor's visual direction/affinity.
        for _ in 0..=p.content.len() + 1 {
            let next = if word && right {
                cursor.next_visual_word(&p.layout)
            } else if word {
                cursor.previous_visual_word(&p.layout)
            } else if right {
                cursor.next_visual(&p.layout)
            } else {
                cursor.previous_visual(&p.layout)
            };
            if next == cursor {
                break;
            }
            cursor = next;
            let target = Position {
                byte: p.content.start + cursor.index(),
                upstream: cursor.affinity() == Affinity::Upstream,
            };
            if self.graphemes.binary_search(&target.byte).is_ok() {
                return Some(target);
            }
        }
        let next = if right {
            index.checked_add(1)
        } else {
            index.checked_sub(1)
        };
        if let Some(next) = next.and_then(|i| self.paragraphs.get(i)) {
            Some(Position {
                byte: if right {
                    next.content.start
                } else {
                    next.content.end
                },
                upstream: !right,
            })
        } else {
            Some(position)
        }
    }
    pub fn vertical(&self, position: Position, down: bool, x: f32) -> Option<Position> {
        if !x.is_finite() {
            return None;
        }
        let (index, cursor) = self.cursor_at(position)?;
        let p = &self.paragraphs[index];
        let geometry = cursor.geometry(&p.layout, 1.);
        let line_index = p
            .layout
            .lines()
            .position(|l| geometry.y0 < l.metrics().block_max_coord as f64)
            .unwrap_or(p.layout.len().saturating_sub(1));
        let target = if down {
            line_index.checked_add(1)
        } else {
            line_index.checked_sub(1)
        };
        let (paragraph, line) = if let Some(line) = target.and_then(|i| p.layout.get(i)) {
            (p, Some(line))
        } else {
            let next_index = if down {
                index.checked_add(1)
            } else {
                index.checked_sub(1)
            };
            let Some(next) = next_index.and_then(|i| self.paragraphs.get(i)) else {
                return Some(position);
            };
            (
                next,
                next.layout.get(if down {
                    0
                } else {
                    next.layout.len().saturating_sub(1)
                }),
            )
        };
        let y = line
            .map(|l| (l.metrics().block_min_coord + l.metrics().block_max_coord) * 0.5)
            .unwrap_or(8.);
        self.hit_position(x, paragraph.y + y)
    }
    pub fn line_edge(&self, position: Position, end: bool) -> Option<Position> {
        let (index, cursor) = self.cursor_at(position)?;
        let p = &self.paragraphs[index];
        let selection = parley::editing::Selection::new(cursor, cursor);
        let selection = if end {
            selection.line_end(&p.layout, false)
        } else {
            selection.line_start(&p.layout, false)
        };
        let cursor = selection.focus();
        let position = Position {
            byte: p.content.start + cursor.index(),
            upstream: cursor.affinity() == Affinity::Upstream,
        };
        let caret = self.caret_at(position)?;
        Some(Position {
            byte: caret.byte,
            ..position
        })
    }
    pub fn visible_paragraphs(&self, top: f64, bottom: f64) -> Option<&[Paragraph]> {
        if !top.is_finite() || !bottom.is_finite() || bottom < top {
            return None;
        }
        let start = self
            .paragraphs
            .partition_point(|p| ((p.y + p.height) as f64) < top);
        let end = start + self.paragraphs[start..].partition_point(|p| (p.y as f64) <= bottom);
        Some(&self.paragraphs[start..end])
    }
    #[cfg(test)] // Unclipped reference geometry for viewport equivalence tests.
    pub fn selection_rects(&self, anchor: usize, focus: usize) -> Option<Vec<BoundingBox>> {
        self.selection_geometry(anchor, focus, &self.paragraphs)
    }
    pub fn viewport_selection_rects(
        &self,
        anchor: usize,
        focus: usize,
        top: f64,
        bottom: f64,
    ) -> Option<Vec<BoundingBox>> {
        self.selection_geometry(anchor, focus, self.visible_paragraphs(top, bottom)?)
    }
    fn selection_geometry(
        &self,
        anchor: usize,
        focus: usize,
        paragraphs: &[Paragraph],
    ) -> Option<Vec<BoundingBox>> {
        if self.caret(anchor)?.byte != anchor || self.caret(focus)?.byte != focus {
            return None;
        }
        let range = anchor.min(focus)..anchor.max(focus);
        let mut rects = Vec::new();
        if range.is_empty() {
            return Some(rects);
        }
        for p in paragraphs {
            if p.source.end <= range.start || p.source.start >= range.end {
                continue;
            }
            let a = range.start.max(p.content.start).min(p.content.end) - p.content.start;
            let b = range.end.min(p.content.end).max(p.content.start) - p.content.start;
            if a < b {
                let selection = parley::editing::Selection::new(
                    Cursor::from_byte_index(&p.layout, a, Affinity::Downstream),
                    Cursor::from_byte_index(&p.layout, b, Affinity::Upstream),
                );
                for (mut rect, _) in selection.geometry(&p.layout) {
                    rect.x0 += p.x as f64;
                    rect.x1 += p.x as f64;
                    rect.y0 += p.y as f64;
                    rect.y1 += p.y as f64;
                    rects.push(rect);
                }
            }
            if range.start <= p.content.end
                && range.end >= p.source.end
                && p.source.end > p.content.end
            {
                let mut rect = self.caret(p.content.end)?.rect;
                rect.x1 = rect.x0 + 8.; // explicit selected line-ending indicator
                rects.push(rect);
            }
        }
        Some(rects)
    }
    pub fn replace(&self, anchor: usize, focus: usize, text: &str) -> Option<String> {
        let a = self.caret(anchor)?;
        let b = self.caret(focus)?;
        // Reject requests inside a shaped cluster instead of silently moving edits.
        if a.byte != anchor || b.byte != focus {
            return None;
        }
        let mut source = self.source.clone();
        source.replace_range(anchor.min(focus)..anchor.max(focus), text);
        Some(source)
    }
}

pub fn audit(source: &str, width: f32) -> Result<serde_json::Value, &'static str> {
    let document = Context::default().shape(source, width)?;
    let mut hit_tests = 0;
    for p in &document.paragraphs {
        let caret = document
            .caret(p.content.start)
            .ok_or("Missing paragraph caret")?;
        let byte = document
            .hit_test(
                caret.rect.x0 as f32,
                ((caret.rect.y0 + caret.rect.y1) * 0.5) as f32,
            )
            .ok_or("Invalid hit test")?;
        if !source.is_char_boundary(byte) {
            return Err("Hit test split UTF-8");
        }
        hit_tests += 1;
    }
    if document.replace(0, 0, "").as_deref() != Some(source) {
        return Err("Layout changed source");
    }
    Ok(
        serde_json::json!({"experimental":true,"format":"screenplay","units":"logicalPixels","displayScale":1.,"syntaxMarkersVisible":true,"printTemplateCertified":false,"editableWidgetInstalled":false,"sourceBytes":source.len(),"paragraphs":document.paragraphs.len(),"width":width,"height":document.height,"indentedParagraphs":document.paragraphs.iter().filter(|p|p.x>0.).count(),"shapedParagraphs":document.shaped_paragraphs,"reusedParagraphs":document.reused_paragraphs,"rewrappedParagraphs":document.rewrapped_paragraphs,"shapedLines":document.paragraphs.iter().map(|p|p.layout.len()).sum::<usize>(),"dialogueParagraphs":document.paragraphs.iter().filter(|p|p.kind==Kind::Dialogue).count(),"maximumTextWidth":document.paragraphs.iter().map(|p|p.width).fold(0.,f32::max),"hitTests":hit_tests,"sourcePreserved":true}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_paragraphs_preserve_crlf_empty_eof_and_actual_styled_layout() {
        let source = "Title: Layout QA\r\n\r\nINT. ROOM - DAY\r\n\r\nMARA\r\nHello **bold** _under_ *soft*.\r\n(quietly)\r\nKeep writing.\r\n";
        let doc = Context::default().shape(source, 600.).unwrap();
        assert_eq!(
            doc.paragraphs
                .iter()
                .map(|p| &source[p.source.clone()])
                .collect::<String>(),
            source
        );
        assert_eq!(
            doc.paragraphs.last().unwrap().content,
            source.len()..source.len()
        );
        let character = doc
            .paragraphs
            .iter()
            .find(|p| p.kind == Kind::Character)
            .unwrap();
        let dialogue = doc
            .paragraphs
            .iter()
            .find(|p| p.kind == Kind::Dialogue)
            .unwrap();
        assert!((character.x - 216.).abs() < 0.001);
        assert!((dialogue.x - 120.).abs() < 0.001);
        assert!((dialogue.width - 372.).abs() < 0.001);
        assert!(
            dialogue
                .layout
                .styles()
                .iter()
                .any(|style| style.underline.is_some())
        );
        assert!(!dialogue.layout.is_empty());
        assert!(doc.height.is_finite() && doc.height > 0.);
        for source in ["", "\r\n", "\n\n", "\r", "A\r\r\n"] {
            let doc = Context::default().shape(source, 600.).unwrap();
            assert!(doc.caret(0).is_some());
            assert!(doc.caret(source.len()).is_some());
            assert_eq!(
                doc.paragraphs
                    .iter()
                    .map(|p| &source[p.source.clone()])
                    .collect::<String>(),
                source
            );
        }
    }
    #[test]
    fn shaped_caret_hit_testing_handles_indent_wrap_unicode_and_bidi() {
        let source = "INT. ROOM - DAY\r\n\r\nMARA\r\nZoë café e\u{301} 日本語 🌙 👩‍💻 keeps writing a long sentence that must wrap in the dialogue column.\r\n\r\n!שלום العربية\r\n";
        let doc = Context::default().shape(source, 300.).unwrap();
        let dialogue = doc
            .paragraphs
            .iter()
            .find(|p| p.kind == Kind::Dialogue)
            .unwrap();
        assert!(dialogue.layout.len() > 1);
        for p in &doc.paragraphs {
            for x in [p.x - 50., p.x, p.x + p.width / 2., p.x + p.width + 50.] {
                let byte = doc.hit_test(x, p.y + 8.).unwrap();
                assert!(p.content.contains(&byte) || byte == p.content.end);
                assert!(source.is_char_boundary(byte));
            }
        }
        let mara = source.find("MARA").unwrap();
        let caret = doc.caret(mara).unwrap();
        assert!((caret.rect.x0 - 108.).abs() < 1.);
        assert_eq!(
            doc.hit_test(caret.rect.x0 as f32 + 0.1, (caret.rect.y0 + 8.) as f32),
            Some(mara)
        );
        assert!(doc.caret(source.find('ë').unwrap() + 1).is_none());
        let e = source.find("e\u{301}").unwrap();
        assert_eq!(doc.caret(e + 1).unwrap().byte, e); // grapheme guard also covers font fallback cluster splits
        assert!(doc.replace(e + 1, e + 1, "x").is_none());
        let emoji = source.find("👩‍💻").unwrap();
        assert_eq!(doc.caret(emoji + "👩".len()).unwrap().byte, emoji);
        assert!(
            doc.replace(emoji + "👩".len(), emoji + "👩".len(), "x")
                .is_none()
        );
        let cr = source.find("\r\n").unwrap();
        assert!(doc.caret(cr + 1).is_none());
        assert!(doc.hit_test(f32::NAN, 0.).is_none());
        assert!(doc.caret(source.len() + 1).is_none());
    }
    #[test]
    fn cross_paragraph_replacement_round_trips_through_shared_history_and_disk() {
        let source =
            "INT. ROOM - DAY\r\n\r\nMARA\r\nZoë 🌙 says hello.\r\n\r\nEXT. ROOF - NIGHT\r\n";
        let mut context = Context::default();
        let doc = context.shape(source, 600.).unwrap();
        let a = source.find("MARA").unwrap();
        let b = source.find("EXT.").unwrap();
        let replacement = "NINA\r\nA new café opens.\r\n\r\n";
        let edited = doc.replace(b, a, replacement).unwrap();
        assert_eq!(
            edited,
            "INT. ROOM - DAY\r\n\r\nNINA\r\nA new café opens.\r\n\r\nEXT. ROOF - NIGHT\r\n"
        );
        let shaped = context.shape(&edited, 400.).unwrap();
        assert!(
            shaped
                .paragraphs
                .iter()
                .any(|p| p.kind == Kind::Character && &edited[p.content.clone()] == "NINA")
        );
        let mut history = crate::history::History::default();
        history.reset(source);
        history.record(&edited);
        let (undone, _, _) = history.step(&edited, false).unwrap();
        assert_eq!(undone, source);
        let (redone, _, _) = history.step(&undone, true).unwrap();
        assert_eq!(redone, edited);
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("Layout Copy.fountain");
        crate::save_copy(&path, &redone).unwrap();
        assert_eq!(crate::source_draft::read(&path).unwrap(), edited);
        assert_eq!(doc.source, source); // existing layout/source snapshot is immutable
    }
    #[test]
    fn width_validation_and_cli_audit_are_read_only() {
        let mut context = Context::default();
        for width in [0., -1., 119., 10_001., f32::INFINITY, f32::NAN] {
            assert!(context.shape("MARA\nHello.", width).is_err());
        }
        let result = audit("INT. ROOM - DAY\n\nMARA\nHello.\n", 600.).unwrap();
        assert_eq!(result["sourcePreserved"], true);
        assert_eq!(result["editableWidgetInstalled"], false);
        assert_eq!(result["dialogueParagraphs"], 1);
    }
}

#[cfg(test)]
mod reuse_tests {
    use super::*;
    fn same_as_cold(document: &Document) {
        let cold = Context::default()
            .shape(document.source(), document.width)
            .unwrap();
        assert_eq!(document.height, cold.height);
        for (a, b) in document.paragraphs.iter().zip(&cold.paragraphs) {
            assert_eq!(a.source, b.source);
            assert_eq!(a.content, b.content);
            assert_eq!(a.kind, b.kind);
            assert_eq!((a.x, a.y, a.width, a.height), (b.x, b.y, b.width, b.height));
        }
        let config = crate::editor_paint::Config {
            width: document.width + 64.,
            height: 800.,
            selection: Some([0, document.source().len()]),
            caret: Some(document.source().len()),
            ..Default::default()
        };
        let a = crate::editor_paint::paint_live(document, &config).unwrap();
        let b = crate::editor_paint::paint_live(&cold, &config).unwrap();
        assert_eq!(
            a.pixels, b.pixels,
            "reused shapes must paint exactly like a fresh layout including selection and caret"
        );
    }
    #[test]
    fn reuse_retargets_source_ranges_without_mutating_previous_layouts() {
        let source =
            "INT. ROOM - DAY\r\n\r\nMARA\r\nZoë e\u{301} waits for a **new** _signal_.\r\n";
        let mut context = Context::default();
        let original = context.shape(source, 600.).unwrap();
        let unchanged = context.reshape(source, 600., Some(&original)).unwrap();
        assert_eq!(unchanged.shaped_paragraphs, 0);
        assert_eq!(unchanged.reused_paragraphs, original.paragraphs.len());
        for p in &unchanged.paragraphs {
            assert!(
                original
                    .paragraphs
                    .iter()
                    .any(|old| Rc::ptr_eq(&old.layout, &p.layout)),
                "identical blank paragraphs can share any matching prior shape"
            );
        }
        let edited = source.replace("waits", "waits patiently");
        let changed = context.reshape(&edited, 600., Some(&original)).unwrap();
        assert_eq!(changed.shaped_paragraphs, 1);
        assert_eq!(
            changed.reused_paragraphs + changed.shaped_paragraphs,
            changed.paragraphs.len()
        );
        same_as_cold(&changed);
        let prefix = "!日本語 opening.\r\n\r\n";
        let shifted = context
            .reshape(&format!("{prefix}{source}"), 600., Some(&original))
            .unwrap();
        let scene = shifted
            .paragraphs
            .iter()
            .find(|p| p.kind == Kind::Scene)
            .unwrap();
        assert_eq!(scene.content.start, prefix.len());
        assert!(Rc::ptr_eq(&scene.layout, &original.paragraphs[0].layout));
        same_as_cold(&shifted);
        assert_eq!(original.source(), source);
        assert_eq!(original.paragraphs[0].content.start, 0);
    }
    #[test]
    fn rewrap_matches_fresh_layout_through_width_roundtrips_with_bidi_styles_and_alignment() {
        let source = "# Act One\r\n\r\nINT. ROOM - DAY\r\n\r\nMARA\r\nA **long** _signal_ arrives: שלום العربية 日本語 e\u{301} 👩‍💻 and more words to wrap.\r\n(quietly)\r\nKeep writing.\r\n\r\n>END<\r\n\r\nFADE OUT:\r\n";
        let mut context = Context::default();
        let original = context.shape(source, 600.).unwrap();
        let mut current = context.reshape(source, 600., Some(&original)).unwrap();
        for width in [300., 420., 600., 220., 600.] {
            current = context.reshape(source, width, Some(&current)).unwrap();
            assert_eq!(current.shaped_paragraphs, 0);
            assert_eq!(current.rewrapped_paragraphs, current.paragraphs.len());
            same_as_cold(&current);
        }
        assert_eq!(original.width, 600.);
        assert_eq!(original.source(), source);
        same_as_cold(&original);
    }
    #[test]
    fn context_roles_width_styles_and_hash_collisions_cannot_reuse_wrong_shapes() {
        let mut context = Context::default();
        let mut original = context.shape("MARA\nHello *stars*.\n", 600.).unwrap();
        let changed = context
            .reshape("!MARA\nHello *stars*.\n", 600., Some(&original))
            .unwrap();
        let old = original
            .paragraphs
            .iter()
            .find(|p| &original.source()[p.content.clone()] == "Hello *stars*.")
            .unwrap();
        let new = changed
            .paragraphs
            .iter()
            .find(|p| &changed.source()[p.content.clone()] == "Hello *stars*.")
            .unwrap();
        assert_ne!(
            old.kind, new.kind,
            "editing an earlier cue changes later paragraph roles"
        );
        assert!(!Rc::ptr_eq(&old.layout, &new.layout));
        same_as_cold(&changed);
        let resized = context
            .reshape(original.source(), 420., Some(&original))
            .unwrap();
        assert_eq!(resized.reused_paragraphs, 0);
        assert_eq!(resized.shaped_paragraphs, 0);
        assert_eq!(resized.rewrapped_paragraphs, resized.paragraphs.len());
        same_as_cold(&resized);
        let styled = context
            .reshape("MARA\nHello **stars**.\n", 600., Some(&original))
            .unwrap();
        assert_eq!(styled.shaped_paragraphs, 1);
        same_as_cold(&styled);
        let fresh = context.shape("NINA\nHello *stars*.\n", 600.).unwrap();
        original.paragraphs[0].signature = fresh.paragraphs[0].signature; // intentional hash collision
        let collided = context
            .reshape(fresh.source(), 600., Some(&original))
            .unwrap();
        assert!(!Rc::ptr_eq(
            &original.paragraphs[0].layout,
            &collided.paragraphs[0].layout
        ));
        assert_eq!(collided.shaped_paragraphs, 1);
        same_as_cold(&collided);
    }
}

#[cfg(test)]
mod viewport_tests {
    use super::*;
    #[test]
    fn selected_long_document_builds_only_visible_selection_geometry() {
        let source = "INT. ROOM - DAY\r\n\r\nMARA\r\nA _new_ signal.\r\n\r\n".repeat(200);
        let document = Context::default().shape(&source, 600.).unwrap();
        let top = document.height * 0.5;
        let bottom = top + 600.;
        let visible = document
            .visible_paragraphs(top as f64, bottom as f64)
            .unwrap();
        assert!(visible.len() < 25);
        assert!(
            visible
                .iter()
                .all(|p| p.y + p.height >= top && p.y <= bottom)
        );
        let all = document.selection_rects(0, source.len()).unwrap();
        let clipped = document
            .viewport_selection_rects(source.len(), 0, top as f64, bottom as f64)
            .unwrap();
        assert!(clipped.len() < 100 && all.len() > 1000);
        for rect in &clipped {
            assert!(
                all.contains(rect),
                "viewport retains original geometry without remapping source"
            );
        }
        // Every rectangle that can affect the viewport remains present.
        for rect in all
            .iter()
            .filter(|r| r.y1 >= top as f64 && r.y0 <= bottom as f64)
        {
            assert!(clipped.contains(rect));
        }
        assert!(
            document
                .visible_paragraphs(f64::NAN, bottom as f64)
                .is_none()
        );
        assert!(document.visible_paragraphs(2., 1.).is_none());
        assert!(
            document
                .visible_paragraphs(document.height as f64 + 100., document.height as f64 + 200.)
                .unwrap()
                .is_empty()
        );
    }
}

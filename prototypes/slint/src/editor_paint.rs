//! Experimental viewport painting from the same Parley layouts used for carets.
//! Bitmap strikes share shaped geometry; unsupported glyph formats remain explicit.
use crate::editor_layout::Document;
use parley::PositionedLayoutItem;
use serde::Deserialize;
use skrifa::{
    FontRef, GlyphId, MetadataProvider,
    instance::{LocationRef, NormalizedCoord, Size},
    outline::{DrawSettings, pen::SvgPen},
};
use std::fmt::Write;

const PAD: f64 = 32.;
#[derive(Debug, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub width: f32,
    pub height: f32,
    pub scale: f32,
    pub scroll_y: f32,
    pub dark: bool,
    pub caret: Option<usize>,
    pub caret_upstream: bool,
    pub selection: Option<[usize; 2]>,
    pub marked: Option<[usize; 2]>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            width: 664.,
            height: 800.,
            scale: 2.,
            scroll_y: 0.,
            dark: false,
            caret: None,
            caret_upstream: false,
            selection: None,
            marked: None,
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !self.width.is_finite()
            || !(184. ..=4096.).contains(&self.width)
            || !self.height.is_finite()
            || !(100. ..=4096.).contains(&self.height)
            || !self.scale.is_finite()
            || !(0.5..=3.).contains(&self.scale)
            || !self.scroll_y.is_finite()
            || self.scroll_y < 0.
            || (self.width * self.scale).ceil() as u64 * (self.height * self.scale).ceil() as u64
                > 16_777_216
        {
            return Err("Invalid or oversized experimental viewport");
        }
        Ok(())
    }
}
pub struct Painting {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub svg: String,
    pub png: Vec<u8>,
    pub metrics: serde_json::Value,
}

pub fn paint(document: &Document, config: &Config) -> Result<Painting, String> {
    paint_inner(document, config, true)
}
pub fn paint_live(document: &Document, config: &Config) -> Result<Painting, String> {
    paint_inner(document, config, false)
}
fn paint_inner(document: &Document, config: &Config, encode_png: bool) -> Result<Painting, String> {
    config.validate()?;
    if (document.width - (config.width - 64.)).abs() > 0.001 {
        return Err("Viewport width differs from shaped layout".into());
    }
    let paper = if config.dark { "#1e1e1e" } else { "#fffdf7" };
    let ink = if config.dark { "#eee9e1" } else { "#242922" };
    let dy = PAD - config.scroll_y as f64;
    let top = -dy;
    let bottom = config.height as f64 - dy;
    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}"><rect width="100%" height="100%" fill="{paper}"/>"##,
        config.width, config.height, config.width, config.height
    );
    let mut selections = 0;
    if let Some([anchor, focus]) = config.selection {
        for rect in document
            .viewport_selection_rects(anchor, focus, top, bottom)
            .ok_or("Invalid selection endpoints")?
        {
            write!(svg, r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#0a66c2" fill-opacity="0.28"/>"##,rect.x0+PAD,rect.y0+dy,rect.width(),rect.height()).unwrap();
            selections += 1;
        }
    }
    let mut marked_rects = 0;
    if let Some([a, b]) = config.marked {
        for rect in document
            .viewport_selection_rects(a, b, top, bottom)
            .ok_or("Invalid marked range")?
        {
            write!(
                svg,
                r##"<rect x="{}" y="{}" width="{}" height="1" fill="#0a84ff"/>"##,
                rect.x0 + PAD,
                rect.y1 + dy - 1.,
                rect.width()
            )
            .unwrap();
            marked_rects += 1;
        }
    }
    let mut bitmap_cache = std::collections::HashMap::new();
    let mut bitmap_glyphs = 0;
    let mut glyphs = 0;
    let mut unsupported = 0;
    let mut missing = 0;
    let mut visible_paragraphs = 0;
    let mut underlines = 0;
    for p in document
        .visible_paragraphs(top, bottom)
        .ok_or("Invalid viewport bounds")?
    {
        if p.y as f64 + dy > config.height as f64 || (p.y + p.height) as f64 + dy < 0. {
            continue;
        }
        visible_paragraphs += 1;
        for line in p.layout.lines() {
            if line.metrics().block_max_coord as f64 + p.y as f64 + dy < 0.
                || line.metrics().block_min_coord as f64 + p.y as f64 + dy > config.height as f64
            {
                continue;
            }
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(run) = item else {
                    continue;
                };
                let font_data = run.run().font();
                let font = FontRef::from_index(font_data.data.as_ref(), font_data.index)
                    .map_err(|e| format!("Font data: {e}"))?;
                let coords: Vec<_> = run
                    .run()
                    .normalized_coords()
                    .iter()
                    .map(|&coord| NormalizedCoord::from_bits(coord))
                    .collect();
                let outlines = font.outline_glyphs();
                for glyph in run.positioned_glyphs() {
                    glyphs += 1;
                    if glyph.id == 0 {
                        missing += 1;
                    }
                    let x = PAD + p.x as f64 + glyph.x as f64;
                    let y = dy + p.y as f64 + glyph.y as f64;
                    let id = bitmap_cache.len();
                    let key = (
                        font_data.data.as_ref().as_ptr() as usize,
                        font_data.index,
                        glyph.id,
                        run.run().font_size().to_bits(),
                    );
                    let bitmap = bitmap_cache.entry(key).or_insert_with(|| {
                        let asset = crate::bitmap_glyph::asset(&font, GlyphId::new(glyph.id), run.run().font_size(), config.scale, config.dark)?;
                        write!(svg, r##"<defs><image id="bitmap{id}" width="{}" height="{}" href="data:image/png;base64,{}"/></defs>"##, asset.width, asset.height, asset.png).unwrap();
                        Some((id, asset))
                    });
                    if let Some((id, asset)) = bitmap {
                        write!(
                            svg,
                            r##"<use href="#bitmap{id}" x="{}" y="{}"/>"##,
                            x + asset.x,
                            y + asset.y
                        )
                        .unwrap();
                        bitmap_glyphs += 1;
                        continue;
                    }
                    let mut path = SvgPen::new();
                    let drawn = outlines.get(GlyphId::new(glyph.id)).is_some_and(|outline| {
                        outline
                            .draw(
                                DrawSettings::unhinted(
                                    Size::new(run.run().font_size()),
                                    LocationRef::new(&coords),
                                ),
                                &mut path,
                            )
                            .is_ok()
                    });
                    let needs_image = font.color_glyphs().get(GlyphId::new(glyph.id)).is_some()
                        || font
                            .bitmap_strikes()
                            .glyph_for_size(
                                Size::new(run.run().font_size() * config.scale),
                                GlyphId::new(glyph.id),
                            )
                            .is_some();
                    if drawn && (!path.is_empty() || !needs_image) {
                        if !path.is_empty() {
                            write!(svg, r##"<path transform="translate({x} {y}) scale(1 -1)" fill="{ink}" d="{}"/>"##, &*path).unwrap();
                        }
                    } else {
                        unsupported += 1;
                        write!(svg, r##"<rect x="{x}" y="{}" width="{}" height="{}" fill="none" stroke="#d45b76"/>"##, y-run.run().font_size() as f64, glyph.advance.max(6.), run.run().font_size()).unwrap();
                    }
                }
                if let Some(underline) = &run.style().underline {
                    let metrics = run.run().metrics();
                    let x = PAD + p.x as f64 + run.offset() as f64;
                    let y = dy + p.y as f64 + run.baseline() as f64
                        - underline.offset.unwrap_or(metrics.underline_offset) as f64;
                    write!(
                        svg,
                        r##"<rect x="{x}" y="{y}" width="{}" height="{}" fill="{ink}"/>"##,
                        run.advance(),
                        underline.size.unwrap_or(metrics.underline_size).max(0.5)
                    )
                    .unwrap();
                    underlines += 1;
                }
            }
        }
    }
    if let Some(byte) = config.caret {
        let caret = document
            .caret_at(crate::editor_layout::Position {
                byte,
                upstream: config.caret_upstream,
            })
            .ok_or("Invalid caret")?;
        if caret.byte != byte {
            return Err("Caret is inside a grapheme or shaped cluster".into());
        }
        write!(
            svg,
            r##"<rect x="{}" y="{}" width="1" height="{}" fill="#0a84ff"/>"##,
            caret.rect.x0 + PAD,
            caret.rect.y0 + dy,
            caret.rect.height()
        )
        .unwrap();
    }
    svg.push_str("</svg>");
    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default())
        .map_err(|e| format!("Viewport SVG: {e}"))?;
    let width = (config.width * config.scale).ceil() as u32;
    let height = (config.height * config.scale).ceil() as u32;
    let mut image =
        resvg::tiny_skia::Pixmap::new(width, height).ok_or("Viewport allocation failed")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(config.scale, config.scale),
        &mut image.as_mut(),
    );
    let png = if encode_png {
        image
            .encode_png()
            .map_err(|e| format!("Viewport PNG: {e}"))?
    } else {
        Vec::new()
    };
    Ok(Painting {
        pixels: image.data().to_vec(),
        width,
        height,
        svg,
        png,
        metrics: serde_json::json!({"experimental":true,"editableWidgetInstalled":false,"visibleParagraphs":visible_paragraphs,"totalParagraphs":document.paragraphs.len(),"glyphs":glyphs,"bitmapGlyphs":bitmap_glyphs,"bitmapImages":bitmap_cache.values().filter(|v| v.is_some()).count(),"unsupportedGlyphs":unsupported,"missingGlyphs":missing,"underlineRuns":underlines,"selectionRects":selections,"markedRects":marked_rects,"caret":config.caret,"caretUpstream":config.caret_upstream,"width":config.width,"height":config.height,"scale":config.scale,"scrollY":config.scroll_y,"dark":config.dark,"rasterWidth":width,"rasterHeight":height}),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn viewport_uses_shaped_geometry_and_rejects_invalid_endpoints_and_sizes() {
        let source = "INT. ROOM - DAY\r\n\r\nMARA\r\nAn _underlined_ word.\r\n";
        let document = crate::editor_layout::Context::default()
            .shape(source, 600.)
            .unwrap();
        let config = Config {
            caret: Some(0),
            selection: Some([0, 3]),
            ..Config::default()
        };
        let result = paint(&document, &config).unwrap();
        assert_eq!(&result.png[..8], b"\x89PNG\r\n\x1a\n");
        let live = paint_live(&document, &config).unwrap();
        assert!(
            live.png.is_empty(),
            "live frames must avoid PNG compression"
        );
        assert_eq!(
            live.pixels, result.pixels,
            "live and artifact painting share exact glyph geometry"
        );
        assert!(result.metrics["glyphs"].as_u64().unwrap() > 20);
        assert!(result.metrics["underlineRuns"].as_u64().unwrap() > 0);
        assert!(result.metrics["selectionRects"].as_u64().unwrap() > 0);
        assert_eq!(result.metrics["editableWidgetInstalled"], false);
        assert!(
            paint(
                &document,
                &Config {
                    width: 400.,
                    ..Config::default()
                }
            )
            .is_err()
        );
        assert!(
            paint(
                &document,
                &Config {
                    caret: Some(16),
                    ..Config::default()
                }
            )
            .is_err()
        ); // CRLF interior
        assert!(
            Config {
                scale: f32::NAN,
                ..Config::default()
            }
            .validate()
            .is_err()
        );
    }
}

#[cfg(all(test, target_os = "macos"))]
mod emoji_regression {
    use super::*;
    #[test]
    fn shaped_emoji_has_visible_color_pixels_in_its_own_caret_cell() {
        let source = "A 🌙 B";
        let document = crate::editor_layout::Context::default()
            .shape(source, 600.)
            .unwrap();
        for dark in [false, true] {
            let config = Config {
                dark,
                ..Config::default()
            };
            let result = paint(&document, &config).unwrap();
            assert_eq!(result.metrics["bitmapGlyphs"], 1);
            assert_eq!(result.metrics["unsupportedGlyphs"], 0);
            assert_eq!(result.metrics["missingGlyphs"], 0);
            let a = document.caret(2).unwrap().rect;
            let b = document.caret(6).unwrap().rect;
            let mut colored = 0;
            for y in ((a.y0 + PAD) * 2.) as u32..((a.y1 + PAD) * 2.) as u32 {
                for x in ((a.x0 + PAD) * 2.) as u32..((b.x0 + PAD) * 2.) as u32 {
                    let pixel = &result.pixels[((y * result.width + x) * 4) as usize..][..4];
                    if pixel[0].max(pixel[1]).max(pixel[2]) - pixel[0].min(pixel[1]).min(pixel[2])
                        > 40
                    {
                        colored += 1;
                    }
                }
            }
            assert!(colored > 8, "emoji cell is empty: {colored} colored pixels");
        }
    }
    #[test]
    fn repeated_and_joined_emoji_use_shaped_glyphs_and_reuse_embedded_images() {
        for source in ["🌙 🌙", "👩🏽‍🚀 🏳️‍🌈 🇺🇸"] {
            let document = crate::editor_layout::Context::default()
                .shape(source, 600.)
                .unwrap();
            let result = paint(&document, &Config::default()).unwrap();
            assert_eq!(result.metrics["unsupportedGlyphs"], 0);
            assert_eq!(result.metrics["missingGlyphs"], 0);
            let images = result.metrics["bitmapImages"].as_u64().unwrap();
            let glyphs = result.metrics["bitmapGlyphs"].as_u64().unwrap();
            assert!(glyphs > 0);
            if source == "🌙 🌙" {
                assert_eq!((images, glyphs), (1, 2));
            }
            assert_eq!(result.svg.matches("<image ").count(), images as usize);
            assert_eq!(result.svg.matches("<use ").count(), glyphs as usize);
            let live = paint_live(&document, &Config::default()).unwrap();
            assert_eq!(live.pixels, result.pixels);
        }
    }
}

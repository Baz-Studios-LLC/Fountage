//! Bitmap font glyphs positioned from the same font and glyph ID as Parley.
//! Coordinates remain logical pixels; a higher-DPI strike changes image detail only.
use base64::{Engine, engine::general_purpose::STANDARD};
use skrifa::{
    FontRef, GlyphId, MetadataProvider,
    bitmap::{BitmapData, BitmapGlyph, Origin},
    instance::{LocationRef, Size},
};

pub struct Asset {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub png: String,
}
pub fn asset(font: &FontRef<'_>, id: GlyphId, size: f32, scale: f32, dark: bool) -> Option<Asset> {
    let glyph = font
        .bitmap_strikes()
        .glyph_for_size(Size::new(size * scale), id)?;
    let units = font
        .metrics(Size::unscaled(), LocationRef::default())
        .units_per_em;
    from_glyph(&glyph, units, size, dark)
}
fn from_glyph(glyph: &BitmapGlyph<'_>, units: u16, size: f32, dark: bool) -> Option<Asset> {
    // Bound allocation before decoding any font-provided image.
    let count = glyph.width.checked_mul(glyph.height)?;
    if units == 0
        || count == 0
        || count > 1_048_576
        || glyph.width > 4096
        || glyph.height > 4096
        || !size.is_finite()
        || size <= 0.
        || !glyph.ppem_x.is_finite()
        || glyph.ppem_x <= 0.
        || !glyph.ppem_y.is_finite()
        || glyph.ppem_y <= 0.
        || ![
            glyph.bearing_x,
            glyph.bearing_y,
            glyph.inner_bearing_x,
            glyph.inner_bearing_y,
        ]
        .iter()
        .all(|v| v.is_finite())
    {
        return None;
    }
    let png = match &glyph.data {
        BitmapData::Png(data) => {
            // Validate both encoded dimensions and payload; malformed PNGs must
            // take the explicit unsupported-glyph path, not disappear in SVG.
            if data.len() < 24
                || data[..8] != *b"\x89PNG\r\n\x1a\n"
                || u32::from_be_bytes(data[16..20].try_into().ok()?) != glyph.width
                || u32::from_be_bytes(data[20..24].try_into().ok()?) != glyph.height
            {
                return None;
            }
            let decoded = resvg::tiny_skia::Pixmap::decode_png(data).ok()?;
            if decoded.width() != glyph.width || decoded.height() != glyph.height {
                return None;
            }
            data.to_vec()
        }
        BitmapData::Bgra(data) => {
            if data.len() != count as usize * 4 {
                return None;
            }
            let mut image = resvg::tiny_skia::Pixmap::new(glyph.width, glyph.height)?;
            for (output, pixel) in image
                .data_mut()
                .chunks_exact_mut(4)
                .zip(data.chunks_exact(4))
            {
                if pixel[..3].iter().any(|&c| c > pixel[3]) {
                    return None;
                }
                output.copy_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
            }
            image.encode_png().ok()?
        }
        BitmapData::Mask(mask) => {
            let alpha = mask.decode(glyph.width, glyph.height).ok()?;
            let mut image = resvg::tiny_skia::Pixmap::new(glyph.width, glyph.height)?;
            let ink: [u8; 3] = if dark {
                [0xee, 0xe9, 0xe1]
            } else {
                [0x24, 0x29, 0x22]
            };
            for (output, alpha) in image.data_mut().chunks_exact_mut(4).zip(alpha) {
                output.copy_from_slice(&[
                    ((ink[0] as u16 * alpha as u16 + 127) / 255) as u8,
                    ((ink[1] as u16 * alpha as u16 + 127) / 255) as u8,
                    ((ink[2] as u16 * alpha as u16 + 127) / 255) as u8,
                    alpha,
                ]);
            }
            image.encode_png().ok()?
        }
    };
    let font_scale = size as f64 / units as f64;
    let sx = size as f64 / glyph.ppem_x as f64;
    let sy = size as f64 / glyph.ppem_y as f64;
    let width = glyph.width as f64 * sx;
    let height = glyph.height as f64 * sy;
    let x = glyph.bearing_x as f64 * font_scale + glyph.inner_bearing_x as f64 * sx;
    let y = -(glyph.bearing_y as f64 * font_scale + glyph.inner_bearing_y as f64 * sy)
        - if glyph.placement_origin == Origin::BottomLeft {
            height
        } else {
            0.
        };
    Some(Asset {
        x,
        y,
        width,
        height,
        png: STANDARD.encode(png),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use skrifa::bitmap::MaskData;
    fn glyph(data: BitmapData<'_>, origin: Origin) -> BitmapGlyph<'_> {
        BitmapGlyph {
            data,
            bearing_x: 100.,
            bearing_y: -50.,
            inner_bearing_x: 2.,
            inner_bearing_y: 3.,
            ppem_x: 10.,
            ppem_y: 20.,
            width: 2,
            height: 2,
            advance: None,
            placement_origin: origin,
        }
    }
    #[test]
    fn png_bgra_and_masks_preserve_placement_alpha_and_ink() {
        let bgra = [0, 0, 255, 255, 0, 255, 0, 255, 255, 0, 0, 255, 0, 0, 0, 0];
        let bitmap = glyph(BitmapData::Bgra(&bgra), Origin::BottomLeft);
        let asset = from_glyph(&bitmap, 1000, 20., false).unwrap();
        assert_eq!(
            (asset.x, asset.y, asset.width, asset.height),
            (6., -4., 4., 2.)
        );
        let png = STANDARD.decode(&asset.png).unwrap();
        let decoded = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
        assert_eq!(
            decoded.data(),
            [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 0, 0, 0, 0]
        );
        let top = from_glyph(
            &glyph(BitmapData::Png(&png), Origin::TopLeft),
            1000,
            20.,
            false,
        )
        .unwrap();
        assert_eq!((top.x, top.y), (6., -2.));
        assert_eq!(top.png, asset.png);
        for dark in [false, true] {
            let mask = glyph(
                BitmapData::Mask(MaskData {
                    bpp: 8,
                    is_packed: false,
                    data: &[255, 0, 0, 255],
                }),
                Origin::TopLeft,
            );
            let asset = from_glyph(&mask, 1000, 20., dark).unwrap();
            let png = STANDARD.decode(asset.png).unwrap();
            let image = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
            let ink = if dark {
                [0xee, 0xe9, 0xe1, 255]
            } else {
                [0x24, 0x29, 0x22, 255]
            };
            assert_eq!(&image.data()[..4], ink);
            assert_eq!(&image.data()[4..8], [0, 0, 0, 0]);
        }
    }
    #[test]
    fn invalid_payload_dimensions_and_metrics_fail_before_raster_allocation() {
        let mut invalid = glyph(BitmapData::Png(b"not a png"), Origin::TopLeft);
        assert!(from_glyph(&invalid, 1000, 20., false).is_none());
        invalid.data = BitmapData::Bgra(&[255, 0, 0, 0]);
        assert!(from_glyph(&invalid, 1000, 20., false).is_none());
        invalid.width = 4096;
        invalid.height = 4096;
        assert!(from_glyph(&invalid, 1000, 20., false).is_none());
        invalid.width = 2;
        invalid.height = 2;
        invalid.data = BitmapData::Bgra(&[0; 16]);
        invalid.ppem_y = f32::NAN;
        assert!(from_glyph(&invalid, 1000, 20., false).is_none());
    }
}

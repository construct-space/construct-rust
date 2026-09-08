use std::sync::Arc;

use skrifa::raw::FileRef;
use vello::kurbo::Affine;
use vello::peniko::{Blob, Color, Fill, FontData};
use vello::{Glyph, Scene};

pub fn load_font() -> Option<FontData> {
    let paths = [
        "/System/Library/Fonts/SFNS.ttf",
        "/System/Library/Fonts/SFNSDisplay.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "C:\\Windows\\Fonts\\arial.ttf",
    ];
    for path in paths {
        if let Ok(data) = std::fs::read(path) {
            return Some(FontData::new(Blob::new(Arc::new(data)), 0));
        }
    }
    None
}

pub fn load_mono_font() -> Option<FontData> {
    let paths = [
        "/System/Library/Fonts/SFNSMono.ttf",
        "/System/Library/Fonts/Menlo.ttc",
        "/System/Library/Fonts/Monaco.dfont",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
        "C:\\Windows\\Fonts\\consola.ttf",
    ];
    for path in paths {
        if let Ok(data) = std::fs::read(path) {
            let blob = Arc::new(data);
            match FileRef::new(blob.as_ref()) {
                Ok(FileRef::Collection(_)) => {
                    return Some(FontData::new(Blob::new(blob), 0));
                }
                Ok(FileRef::Font(_)) => {
                    return Some(FontData::new(Blob::new(blob), 0));
                }
                Err(_) => continue,
            }
        }
    }
    None
}

pub fn layout_glyphs(font: &FontData, text: &str, font_size: f64) -> Vec<Glyph> {
    use skrifa::{
        FontRef, MetadataProvider,
        instance::{LocationRef, Size},
    };
    let Ok(font_ref) = FontRef::from_index(font.data.as_ref(), font.index) else {
        return vec![];
    };
    let charmap = font_ref.charmap();
    let loc = LocationRef::default();
    let gm = font_ref.glyph_metrics(Size::new(font_size as f32), loc);
    let mut glyphs = Vec::new();
    let mut x = 0.0f32;
    for ch in text.chars() {
        if let Some(gid) = charmap.map(ch) {
            glyphs.push(Glyph {
                id: gid.to_u32(),
                x,
                y: 0.0,
            });
            x += gm.advance_width(gid).unwrap_or(font_size as f32 * 0.5);
        } else {
            x += font_size as f32 * 0.3;
        }
    }
    glyphs
}

pub fn draw_text(
    scene: &mut Scene,
    font: &FontData,
    text: &str,
    x: f64,
    y: f64,
    size: f64,
    color: Color,
    xf: Affine,
) {
    let glyphs = layout_glyphs(font, text, size);
    if glyphs.is_empty() {
        return;
    }
    let text_xf = xf * Affine::translate((x, y));
    scene
        .draw_glyphs(font)
        .font_size(size as f32)
        .transform(text_xf)
        .brush(&color)
        .draw(Fill::NonZero, glyphs.into_iter());
}

pub fn measure_char_width(font: &FontData, size: f64) -> f64 {
    let glyphs = layout_glyphs(font, "M", size);
    if glyphs.last().is_some() {
        let glyphs2 = layout_glyphs(font, "MM", size);
        if glyphs2.len() == 2 {
            return (glyphs2[1].x - glyphs2[0].x) as f64;
        }
    }
    size * 0.6
}

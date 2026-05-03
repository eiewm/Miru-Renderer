use ab_glyph::{point, Font, FontArc, GlyphId, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};
use std::sync::OnceLock;
const FONT_UBUNTU_REGULAR: &[u8] = include_bytes!("../../assets/Ubuntu-Regular.ttf");
static LOADED_UBUNTU_REGULAR: OnceLock<Option<FontArc>> = OnceLock::new();
pub fn font_regular() -> Option<&'static FontArc> {
    font_ubuntu_regular()
}
pub fn font_bold() -> Option<&'static FontArc> {
    font_ubuntu_regular()
}
pub fn font_ubuntu_regular() -> Option<&'static FontArc> {
    LOADED_UBUNTU_REGULAR
        .get_or_init(|| FontArc::try_from_slice(FONT_UBUNTU_REGULAR).ok())
        .as_ref()
}
pub fn font_badge_value() -> Option<&'static FontArc> {
    font_ubuntu_regular()
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Normal,
    Bold,
}
pub struct RenderedText {
    pub image: RgbaImage,
    pub width: u32,
    pub height: u32,
}
impl RenderedText {
    pub fn into_raw(self) -> Vec<u8> {
        self.image.into_raw()
    }
}
fn blend_text_pixel(image: &mut RgbaImage, x: i32, y: i32, color: Rgba<u8>, coverage: f32) {
    if coverage <= 0.0 || x < 0 || y < 0 {
        return;
    }
    let x = x as u32;
    let y = y as u32;
    if x >= image.width() || y >= image.height() {
        return;
    }
    let mut dst = *image.get_pixel(x, y);
    let src_alpha = (color[3] as f32 / 255.0) * coverage.clamp(0.0, 1.0);
    if src_alpha <= 0.0 {
        return;
    }
    // Glyph coverage is straight alpha from ab_glyph, so RGB must be normalized by the output alpha.
    let dst_alpha = dst[3] as f32 / 255.0;
    let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);
    if out_alpha <= 0.0 {
        return;
    }
    let blend_channel = |src: u8, dst: u8| {
        ((src as f32 * src_alpha + dst as f32 * dst_alpha * (1.0 - src_alpha)) / out_alpha)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    dst[0] = blend_channel(color[0], dst[0]);
    dst[1] = blend_channel(color[1], dst[1]);
    dst[2] = blend_channel(color[2], dst[2]);
    dst[3] = (out_alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    image.put_pixel(x, y, dst);
}
pub(crate) fn draw_text_rgba(
    image: &mut RgbaImage,
    color: Rgba<u8>,
    x: i32,
    y: i32,
    scale: PxScale,
    font: &FontArc,
    text: &str,
) {
    let scaled = font.as_scaled(scale);
    let line_height = (scaled.height() + scaled.line_gap()).max(scale.y);
    let mut caret = point(x as f32, y as f32 + scaled.ascent());
    let start_x = caret.x;
    let mut previous = None::<GlyphId>;
    for ch in text.chars() {
        if ch == '\n' {
            caret.x = start_x;
            caret.y += line_height;
            previous = None;
            continue;
        }
        let glyph_id = scaled.glyph_id(ch);
        if let Some(previous_id) = previous {
            caret.x += scaled.kern(previous_id, glyph_id);
        }
        let glyph = glyph_id.with_scale_and_position(scale, caret);
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            let origin_x = bounds.min.x.floor() as i32;
            let origin_y = bounds.min.y.floor() as i32;
            outlined.draw(|gx, gy, coverage| {
                blend_text_pixel(
                    image,
                    origin_x + gx as i32,
                    origin_y + gy as i32,
                    color,
                    coverage,
                );
            });
        }
        caret.x += scaled.h_advance(glyph_id);
        previous = Some(glyph_id);
    }
}
pub fn render_text(
    text: &str,
    font_size: f32,
    color: [u8; 4],
    weight: FontWeight,
) -> Option<RenderedText> {
    let font = match weight {
        FontWeight::Bold => font_bold()?,
        FontWeight::Normal => font_regular()?,
    };
    let scale = PxScale::from(font_size);
    let scaled = font.as_scaled(scale);
    let mut text_width = 0.0f32;
    for ch in text.chars() {
        let glyph_id = scaled.glyph_id(ch);
        text_width += scaled.h_advance(glyph_id);
    }
    let shadow_pad = 4u32;
    let img_w = (text_width.ceil() as u32) + shadow_pad * 2;
    let img_h = (font_size * 1.5).ceil() as u32;
    let mut img = RgbaImage::new(img_w, img_h);
    let text_x = shadow_pad as i32;
    let text_y = (font_size * 0.1) as i32;
    let shadow_color = Rgba([0, 0, 0, 160]);
    for (dx, dy) in [(1, 1), (1, 2), (2, 1), (2, 2)] {
        draw_text_rgba(
            &mut img,
            shadow_color,
            text_x + dx,
            text_y + dy,
            scale,
            font,
            text,
        );
    }
    let main_color = Rgba(color);
    draw_text_rgba(&mut img, main_color, text_x, text_y, scale, font, text);
    Some(RenderedText {
        width: img_w,
        height: img_h,
        image: img,
    })
}
pub fn render_text_with_font(
    text: &str,
    font_size: f32,
    color: [u8; 4],
    font: &FontArc,
) -> Option<RenderedText> {
    let scale = PxScale::from(font_size);
    let scaled = font.as_scaled(scale);
    let mut text_width = 0.0f32;
    for ch in text.chars() {
        let glyph_id = scaled.glyph_id(ch);
        text_width += scaled.h_advance(glyph_id);
    }
    let shadow_pad = 4u32;
    let img_w = (text_width.ceil() as u32) + shadow_pad * 2;
    let img_h = (font_size * 1.5).ceil() as u32;
    let mut img = RgbaImage::new(img_w, img_h);
    let text_x = shadow_pad as i32;
    let text_y = (font_size * 0.1) as i32;
    let shadow_color = Rgba([0, 0, 0, 160]);
    for (dx, dy) in [(1, 1), (1, 2), (2, 1), (2, 2)] {
        draw_text_rgba(
            &mut img,
            shadow_color,
            text_x + dx,
            text_y + dy,
            scale,
            font,
            text,
        );
    }
    draw_text_rgba(&mut img, Rgba(color), text_x, text_y, scale, font, text);
    Some(RenderedText {
        width: img_w,
        height: img_h,
        image: img,
    })
}
pub fn render_text_simple(
    text: &str,
    font_size: f32,
    color: [u8; 4],
    weight: FontWeight,
) -> Option<RenderedText> {
    let font = match weight {
        FontWeight::Bold => font_bold()?,
        FontWeight::Normal => font_regular()?,
    };
    render_text_simple_with_font(text, font_size, color, font)
}
pub fn render_text_simple_with_font(
    text: &str,
    font_size: f32,
    color: [u8; 4],
    font: &FontArc,
) -> Option<RenderedText> {
    let scale = PxScale::from(font_size);
    let scaled = font.as_scaled(scale);
    let mut text_width = 0.0f32;
    for ch in text.chars() {
        let glyph_id = scaled.glyph_id(ch);
        text_width += scaled.h_advance(glyph_id);
    }
    let img_w = (text_width.ceil() as u32).max(1);
    let img_h = (font_size * 1.3).ceil() as u32;
    let mut img = RgbaImage::new(img_w, img_h);
    draw_text_rgba(&mut img, Rgba(color), 0, 0, scale, font, text);
    Some(RenderedText {
        width: img_w,
        height: img_h,
        image: img,
    })
}
pub fn measure_text(text: &str, font_size: f32, weight: FontWeight) -> f32 {
    let font = match weight {
        FontWeight::Bold => font_bold(),
        FontWeight::Normal => font_regular(),
    };
    let Some(font) = font else {
        // Font loading can fail in stripped builds; keep layout deterministic with an approximate width.
        return text.len() as f32 * font_size * 0.55;
    };
    let scaled = font.as_scaled(PxScale::from(font_size));
    let mut width = 0.0f32;
    for ch in text.chars() {
        width += scaled.h_advance(scaled.glyph_id(ch));
    }
    width
}
pub fn measure_text_with_font(text: &str, font_size: f32, font: &FontArc) -> f32 {
    let scaled = font.as_scaled(PxScale::from(font_size));
    let mut width = 0.0f32;
    for ch in text.chars() {
        width += scaled.h_advance(scaled.glyph_id(ch));
    }
    width
}
pub fn escape_xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

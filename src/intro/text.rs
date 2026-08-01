use ab_glyph::{point, Font, FontArc, GlyphId, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};
use std::sync::OnceLock;
const FONT_UBUNTU_REGULAR: &[u8] = include_bytes!("../../assets/Ubuntu-Regular.ttf");
// Ubuntu covers latin, cyrillic and greek but has no CJK glyphs, and ab_glyph
// does no fallback of its own: it just draws .notdef, which is the empty box
// players were seeing on japanese titles.
const FONT_NOTO_SANS_JP: &[u8] = include_bytes!("../../assets/NotoSansJP-Regular.ttf");
const FONT_NOTO_SANS_KR: &[u8] = include_bytes!("../../assets/NotoSansKR-Regular.ttf");
static LOADED_UBUNTU_REGULAR: OnceLock<Option<FontArc>> = OnceLock::new();
static LOADED_NOTO_SANS_JP: OnceLock<Option<FontArc>> = OnceLock::new();
static LOADED_NOTO_SANS_KR: OnceLock<Option<FontArc>> = OnceLock::new();
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
fn font_noto_sans_jp() -> Option<&'static FontArc> {
    LOADED_NOTO_SANS_JP
        .get_or_init(|| FontArc::try_from_slice(FONT_NOTO_SANS_JP).ok())
        .as_ref()
}
fn font_noto_sans_kr() -> Option<&'static FontArc> {
    LOADED_NOTO_SANS_KR
        .get_or_init(|| FontArc::try_from_slice(FONT_NOTO_SANS_KR).ok())
        .as_ref()
}
/// Embedded CJK fallbacks, in the order they should be consulted. Callers that
/// build their own font stacks (the custom HUD) append these last so configured
/// fonts still win.
pub fn embedded_cjk_fallbacks() -> impl Iterator<Item = &'static FontArc> {
    [font_noto_sans_jp(), font_noto_sans_kr()]
        .into_iter()
        .flatten()
}
/// Font that actually has a glyph for `ch`, preferring the caller's own font.
///
/// The choice is per character, not per string: a title like
/// `Yorushika - 藍二乗` mixes scripts on the same line.
pub fn font_for_char(primary: &FontArc, ch: char) -> &FontArc {
    if primary.glyph_id(ch).0 != 0 {
        return primary;
    }
    [font_noto_sans_jp(), font_noto_sans_kr()]
        .into_iter()
        .flatten()
        .find(|font| font.glyph_id(ch).0 != 0)
        .unwrap_or(primary)
}
/// Width of `text` with every character measured on the font that will draw it.
/// Measuring everything against the primary font reports latin advances for
/// glyphs that a fallback ends up rendering much wider.
pub fn measure_text_with_fallback(font: &FontArc, scale: PxScale, text: &str) -> f32 {
    text.chars()
        .filter(|ch| *ch != '\n')
        .map(|ch| {
            let scaled = font_for_char(font, ch).as_scaled(scale);
            scaled.h_advance(scaled.glyph_id(ch))
        })
        .sum()
}
/// Baseline offset that keeps every font used by `text` inside the image.
/// Noto sits higher above the baseline than Ubuntu, so using the primary
/// ascent alone clipped the top of CJK glyphs.
fn fallback_ascent(font: &FontArc, scale: PxScale, text: &str) -> f32 {
    text.chars()
        .filter(|ch| *ch != '\n')
        .map(|ch| font_for_char(font, ch).as_scaled(scale).ascent())
        .fold(font.as_scaled(scale).ascent(), f32::max)
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
    let primary = font.as_scaled(scale);
    let line_height = (primary.height() + primary.line_gap()).max(scale.y);
    // All runs share one baseline; only its distance from the top has to account
    // for the tallest font involved.
    let mut caret = point(x as f32, y as f32 + fallback_ascent(font, scale, text));
    let start_x = caret.x;
    let mut previous = None::<(GlyphId, *const FontArc)>;
    for ch in text.chars() {
        if ch == '\n' {
            caret.x = start_x;
            caret.y += line_height;
            previous = None;
            continue;
        }
        let font = font_for_char(font, ch);
        let scaled = font.as_scaled(scale);
        let glyph_id = scaled.glyph_id(ch);
        // Kerning pairs only exist within a font, so a run boundary drops them.
        if let Some((previous_id, previous_font)) = previous {
            if std::ptr::eq(previous_font, font as *const FontArc) {
                caret.x += scaled.kern(previous_id, glyph_id);
            }
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
        previous = Some((glyph_id, font as *const FontArc));
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
    let text_width = measure_text_with_fallback(font, scale, text);
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
/// Like `render_text`, but shrinks the size until the text fits `max_width`.
pub fn render_text_fitted(
    text: &str,
    font_size: f32,
    color: [u8; 4],
    weight: FontWeight,
    max_width: u32,
) -> Option<RenderedText> {
    const MIN_FONT_SIZE: f32 = 10.0;
    let font = match weight {
        FontWeight::Bold => font_bold()?,
        FontWeight::Normal => font_regular()?,
    };
    let mut size = font_size;
    while size > MIN_FONT_SIZE {
        let width = measure_text_with_fallback(font, PxScale::from(size), text);
        if width.ceil() as u32 + 8 <= max_width {
            break;
        }
        // A proportional step converges in a couple of passes.
        let next = size * (max_width as f32 / (width.max(1.0) + 8.0)) * 0.98;
        size = next.max(MIN_FONT_SIZE).min(size - 0.5);
    }
    render_text(text, size, color, weight)
}

pub fn render_text_with_font(
    text: &str,
    font_size: f32,
    color: [u8; 4],
    font: &FontArc,
) -> Option<RenderedText> {
    let scale = PxScale::from(font_size);
    let text_width = measure_text_with_fallback(font, scale, text);
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
    let text_width = measure_text_with_fallback(font, scale, text);
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
    measure_text_with_fallback(font, PxScale::from(font_size), text)
}
pub fn measure_text_with_font(text: &str, font_size: f32, font: &FontArc) -> f32 {
    measure_text_with_fallback(font, PxScale::from(font_size), text)
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

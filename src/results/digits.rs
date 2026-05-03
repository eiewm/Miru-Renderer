use crate::intro::{render_text_simple, FontWeight};
use crate::types::SkinAssets;
use crate::utils::image_proc::{
    find_bottom_padding, find_left_padding, find_right_padding, find_top_padding, load_rgba,
};
use image::imageops::{crop_imm, FilterType};
use image::RgbaImage;
const RESULTS_DIGIT_ALPHA_FLOOR: u8 = 32;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DigitSpriteKind {
    Score,
    Count,
}
impl DigitSpriteKind {
    fn prefixes(self, skin: &SkinAssets) -> (&str, &'static str) {
        match self {
            Self::Score | Self::Count => (skin.config.score_prefix_or_default(), "score"),
        }
    }
}
fn glyph_name(ch: char) -> Option<&'static str> {
    match ch {
        '0' => Some("0"),
        '1' => Some("1"),
        '2' => Some("2"),
        '3' => Some("3"),
        '4' => Some("4"),
        '5' => Some("5"),
        '6' => Some("6"),
        '7' => Some("7"),
        '8' => Some("8"),
        '9' => Some("9"),
        '.' => Some("dot"),
        ',' => Some("comma"),
        '%' => Some("percent"),
        'x' | 'X' => Some("x"),
        '-' => Some("minus"),
        _ => None,
    }
}
fn load_digit_glyph(
    skin: &SkinAssets,
    kind: DigitSpriteKind,
    ch: char,
    target_height: u32,
) -> Option<(RgbaImage, f32)> {
    let glyph = glyph_name(ch)?;
    let (prefix, fallback) = kind.prefixes(skin);
    let candidates = [
        format!("{prefix}-{glyph}@2x.png"),
        format!("{prefix}-{glyph}.png"),
        format!("{fallback}-{glyph}@2x.png"),
        format!("{fallback}-{glyph}.png"),
    ];
    let refs: Vec<_> = candidates.iter().map(String::as_str).collect();
    let (name, data) = skin.find_first(&refs)?;
    let image = load_rgba(data)?;
    // @2x glyphs have double pixel density but the same logical skin height.
    let scale_factor = if name.contains("@2x") { 2.0 } else { 1.0 };
    let height = target_height.max(1);
    let logical_height = (image.height() as f32 / scale_factor).max(1.0);
    let width = ((image.width() as f32) * (height as f32 / image.height().max(1) as f32))
        .round()
        .max(1.0) as u32;
    let resized = resize_digit_glyph(&image, width, height);
    Some((
        prune_low_alpha_pixels(&resized, RESULTS_DIGIT_ALPHA_FLOOR),
        logical_height,
    ))
}
fn trim_transparent(image: &RgbaImage) -> RgbaImage {
    let left = find_left_padding(image, RESULTS_DIGIT_ALPHA_FLOOR);
    let right = find_right_padding(image, RESULTS_DIGIT_ALPHA_FLOOR);
    let top = find_top_padding(image, RESULTS_DIGIT_ALPHA_FLOOR);
    let bottom = find_bottom_padding(image, RESULTS_DIGIT_ALPHA_FLOOR);
    let width = image.width().saturating_sub(left + right).max(1);
    let height = image.height().saturating_sub(top + bottom).max(1);
    crop_imm(
        image,
        left.min(image.width().saturating_sub(1)),
        top.min(image.height().saturating_sub(1)),
        width.min(image.width()),
        height.min(image.height()),
    )
    .to_image()
}
fn prune_low_alpha_pixels(image: &RgbaImage, threshold: u8) -> RgbaImage {
    let mut out = image.clone();
    for pixel in out.pixels_mut() {
        // Skin digits often contain barely-visible antialiasing fringes that distort trimming.
        if pixel[3] <= threshold {
            *pixel = image::Rgba([0, 0, 0, 0]);
        }
    }
    out
}
fn resize_digit_glyph(image: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    if width == 0 || height == 0 {
        return RgbaImage::new(1, 1);
    }
    image::imageops::resize(image, width, height, FilterType::Nearest)
}
pub(crate) fn compose_digit_sprite(
    skin: &SkinAssets,
    kind: DigitSpriteKind,
    text: &str,
    target_height: u32,
    fallback_color: [u8; 4],
) -> Option<RgbaImage> {
    if text.is_empty() {
        return None;
    }
    let target_height = target_height.max(1);
    let char_count = text.chars().count();
    let fallback_font_size = (target_height as f32 * 0.92).max(10.0);
    let mut parts = Vec::with_capacity(char_count);
    let mut max_height = 0u32;
    // Numeric columns use a fixed advance so score and count strings do not wobble by digit shape.
    let fixed_digit_width = measure_fixed_digit_width(skin, kind, target_height)
        .unwrap_or((target_height as f32 * 0.62).round().max(1.0) as u32);
    let native_digit_height = measure_native_digit_height(skin, kind).unwrap_or(1.0);
    let overlap = scaled_overlap(skin, kind, target_height, native_digit_height);
    let mut total_width = 0u32;
    for ch in text.chars() {
        let (sprite, is_digit) =
            if let Some((sprite, _)) = load_digit_glyph(skin, kind, ch, target_height) {
                (sprite, ch.is_ascii_digit())
            } else {
                (
                    render_text_simple(
                        &ch.to_string(),
                        fallback_font_size,
                        fallback_color,
                        FontWeight::Bold,
                    )
                    .map(|rendered| rendered.image)?,
                    ch.is_ascii_digit(),
                )
            };
        let advance = if is_digit {
            fixed_digit_width.max(sprite.width())
        } else {
            sprite.width().max(1)
        };
        total_width = total_width.saturating_add(advance);
        max_height = max_height.max(sprite.height());
        parts.push((sprite, advance, is_digit));
        if !parts.is_empty() && char_count > 1 {
            total_width = total_width.saturating_sub(overlap);
        }
    }
    total_width = total_width.saturating_add(overlap);
    let mut out = RgbaImage::new(total_width.max(1), max_height.max(1));
    let mut cursor_x = 0u32;
    for (index, (sprite, advance, is_digit)) in parts.into_iter().enumerate() {
        let y = max_height.saturating_sub(sprite.height()) / 2;
        let x = if is_digit {
            cursor_x + advance.saturating_sub(sprite.width()) / 2
        } else {
            cursor_x
        };
        overlay(&mut out, &sprite, x as i32, y as i32);
        cursor_x = cursor_x.saturating_add(advance);
        if index + 1 < char_count {
            cursor_x = cursor_x.saturating_sub(overlap);
        }
    }
    Some(trim_transparent(&out))
}
fn measure_fixed_digit_width(
    skin: &SkinAssets,
    kind: DigitSpriteKind,
    target_height: u32,
) -> Option<u32> {
    let mut max_width = 0u32;
    for digit in ['5', '0', '8', '1', '2', '3', '4', '6', '7', '9'] {
        if let Some((sprite, _)) = load_digit_glyph(skin, kind, digit, target_height) {
            max_width = max_width.max(sprite.width());
        }
    }
    (max_width > 0).then_some(max_width)
}
fn measure_native_digit_height(skin: &SkinAssets, kind: DigitSpriteKind) -> Option<f32> {
    for digit in ['5', '0', '8'] {
        if let Some((_, height)) = load_digit_glyph(skin, kind, digit, 100) {
            return Some(height);
        }
    }
    None
}
fn scaled_overlap(
    skin: &SkinAssets,
    kind: DigitSpriteKind,
    target_height: u32,
    native_digit_height: f32,
) -> u32 {
    let base_overlap = match kind {
        DigitSpriteKind::Score | DigitSpriteKind::Count => skin.config.score_overlap.unwrap_or(0),
    }
    .max(0) as f32;
    if base_overlap <= 0.0 {
        return 0;
    }
    // scoreOverlap is authored in native skin pixels, so scale it to the rendered digit height.
    (base_overlap * (target_height as f32 / native_digit_height.max(1.0)))
        .round()
        .max(0.0) as u32
}
fn overlay(target: &mut RgbaImage, sprite: &RgbaImage, x: i32, y: i32) {
    for sy in 0..sprite.height() {
        let dy = y + sy as i32;
        if !(0..target.height() as i32).contains(&dy) {
            continue;
        }
        for sx in 0..sprite.width() {
            let dx = x + sx as i32;
            if !(0..target.width() as i32).contains(&dx) {
                continue;
            }
            let src = *sprite.get_pixel(sx, sy);
            if src[3] == 0 {
                continue;
            }
            let dst = target.get_pixel_mut(dx as u32, dy as u32);
            *dst = blend_pixel(*dst, src, 1.0);
        }
    }
}
fn blend_pixel(dst: image::Rgba<u8>, src: image::Rgba<u8>, opacity: f32) -> image::Rgba<u8> {
    let src_alpha = (src[3] as f32 / 255.0) * opacity.clamp(0.0, 1.0);
    if src_alpha <= 0.0 {
        return dst;
    }
    let dst_alpha = dst[3] as f32 / 255.0;
    let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);
    if out_alpha <= 0.0 {
        return image::Rgba([0, 0, 0, 0]);
    }
    let src_weight = src_alpha / out_alpha;
    let dst_weight = (dst_alpha * (1.0 - src_alpha)) / out_alpha;
    image::Rgba([
        (src[0] as f32 * src_weight + dst[0] as f32 * dst_weight).round() as u8,
        (src[1] as f32 * src_weight + dst[1] as f32 * dst_weight).round() as u8,
        (src[2] as f32 * src_weight + dst[2] as f32 * dst_weight).round() as u8,
        (out_alpha * 255.0).round() as u8,
    ])
}

use image::codecs::png::PngEncoder;
use image::{imageops::FilterType, ImageEncoder, RgbaImage};
pub const MAX_TEXTURE_DIM: u32 = 8192;
pub const ALPHA_THRESHOLD_HIGH: u8 = 240;
pub const ALPHA_THRESHOLD_LOW: u8 = 0;
pub const DEFAULT_BACKGROUND_BLUR_PERCENT: u8 = 35;
pub const BACKGROUND_BLUR_SIGMA_PER_PERCENT: f32 = 0.2;
pub const LEGACY_INTRO_BACKGROUND_BLUR_SIGMA: f32 =
    background_blur_sigma_from_percent(DEFAULT_BACKGROUND_BLUR_PERCENT);
pub fn load_rgba(data: &[u8]) -> Option<RgbaImage> {
    image::load_from_memory(data).ok().map(|img| img.to_rgba8())
}
pub fn get_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(std::io::Cursor::new(data))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}
pub fn resize_if_needed(img: RgbaImage, max_dim: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    if w <= max_dim && h <= max_dim {
        return img;
    }
    let scale = (max_dim as f32 / w as f32).min(max_dim as f32 / h as f32);
    let new_w = (w as f32 * scale).floor() as u32;
    let new_h = (h as f32 * scale).floor() as u32;
    image::imageops::resize(&img, new_w, new_h, FilterType::Lanczos3)
}
pub fn resize_exact(img: &RgbaImage, w: u32, h: u32) -> RgbaImage {
    if w == 0 || h == 0 {
        return RgbaImage::new(1, 1);
    }
    image::imageops::resize(img, w, h, FilterType::Lanczos3)
}
pub fn resize_exact_sprite_nearest(img: &RgbaImage, w: u32, h: u32) -> RgbaImage {
    if w == 0 || h == 0 {
        return RgbaImage::new(1, 1);
    }
    image::imageops::resize(img, w, h, FilterType::Nearest)
}
pub fn resize_exact_sharp_upscale(img: &RgbaImage, w: u32, h: u32) -> RgbaImage {
    if w == 0 || h == 0 {
        return RgbaImage::new(1, 1);
    }
    let (orig_w, orig_h) = img.dimensions();
    if w > orig_w || h > orig_h {
        image::imageops::resize(img, w, h, FilterType::Nearest)
    } else {
        resize_exact_alpha_safe(img, w, h)
    }
}
fn resize_exact_alpha_safe_with_filter(
    img: &RgbaImage,
    w: u32,
    h: u32,
    filter: FilterType,
) -> RgbaImage {
    // Resize in premultiplied alpha space to avoid dark fringes around transparent sprite edges.
    let mut premultiplied = img.clone();
    premultiply_alpha(&mut premultiplied);
    let mut resized = image::imageops::resize(&premultiplied, w, h, filter);
    unpremultiply_alpha(&mut resized);
    resized
}
pub fn resize_exact_alpha_safe(img: &RgbaImage, w: u32, h: u32) -> RgbaImage {
    if w == 0 || h == 0 {
        return RgbaImage::new(1, 1);
    }
    resize_exact_alpha_safe_with_filter(img, w, h, FilterType::Lanczos3)
}
pub fn resize_exact_gameplay_note(img: &RgbaImage, w: u32, h: u32) -> RgbaImage {
    if w == 0 || h == 0 {
        return RgbaImage::new(1, 1);
    }
    let (orig_w, orig_h) = img.dimensions();
    let filter = if w > orig_w || h > orig_h {
        // Upscaled note sprites should stay crisp, while downscaled notes need smoothing.
        FilterType::Triangle
    } else {
        FilterType::CatmullRom
    };
    resize_exact_alpha_safe_with_filter(img, w, h, filter)
}
pub fn cover_dimensions(src_w: u32, src_h: u32, target_w: u32, target_h: u32) -> (u32, u32) {
    if src_w == 0 || src_h == 0 || target_w == 0 || target_h == 0 {
        return (1, 1);
    }
    let scale_x = target_w as f32 / src_w as f32;
    let scale_y = target_h as f32 / src_h as f32;
    let scale = scale_x.max(scale_y);
    let scaled_w = (src_w as f32 * scale).round().max(1.0) as u32;
    let scaled_h = (src_h as f32 * scale).round().max(1.0) as u32;
    (scaled_w, scaled_h)
}
pub fn resize_cover(img: &RgbaImage, target_w: u32, target_h: u32) -> RgbaImage {
    if target_w == 0 || target_h == 0 {
        return RgbaImage::new(1, 1);
    }
    let (src_w, src_h) = img.dimensions();
    let (scaled_w, scaled_h) = cover_dimensions(src_w, src_h, target_w, target_h);
    let resized = image::imageops::resize(img, scaled_w, scaled_h, FilterType::Lanczos3);
    let crop_x = (scaled_w.saturating_sub(target_w)) / 2;
    let crop_y = (scaled_h.saturating_sub(target_h)) / 2;
    image::imageops::crop_imm(&resized, crop_x, crop_y, target_w, target_h).to_image()
}
pub fn resize_cover_with_offset(
    img: &RgbaImage,
    target_w: u32,
    target_h: u32,
    offset_x: f32,
    offset_y: f32,
) -> RgbaImage {
    if target_w == 0 || target_h == 0 {
        return RgbaImage::new(1, 1);
    }
    let (src_w, src_h) = img.dimensions();
    let (scaled_w, scaled_h) = cover_dimensions(src_w, src_h, target_w, target_h);
    let resized = image::imageops::resize(img, scaled_w, scaled_h, FilterType::Lanczos3);
    let safe_offset_x = if offset_x.is_finite() { offset_x } else { 0.0 };
    let safe_offset_y = if offset_y.is_finite() { offset_y } else { 0.0 };
    let left = ((target_w as f32 - scaled_w as f32) * 0.5 + safe_offset_x).round() as i64;
    let top = ((target_h as f32 - scaled_h as f32) * 0.5 + safe_offset_y).round() as i64;
    // Offsets can reveal empty edges, so fill exposed background with opaque black.
    let mut canvas = RgbaImage::from_pixel(target_w, target_h, image::Rgba([0, 0, 0, 255]));
    image::imageops::overlay(&mut canvas, &resized, left, top);
    canvas
}
pub const fn background_blur_sigma_from_percent(percent: u8) -> f32 {
    // The UI slider maps linearly to gaussian sigma for predictable intro/background blur.
    percent as f32 * BACKGROUND_BLUR_SIGMA_PER_PERCENT
}
pub fn apply_gaussian_blur(img: &RgbaImage, sigma: f32) -> RgbaImage {
    if !sigma.is_finite() || sigma <= 0.0 {
        return img.clone();
    }
    image::imageops::blur(img, sigma)
}
pub fn encode_png(img: &RgbaImage) -> Vec<u8> {
    let mut buf = Vec::new();
    let encoder = PngEncoder::new(&mut buf);
    let _ = encoder.write_image(
        img.as_raw(),
        img.width(),
        img.height(),
        image::ExtendedColorType::Rgba8,
    );
    buf
}
pub fn find_top_padding(img: &RgbaImage, threshold: u8) -> u32 {
    let (w, h) = img.dimensions();
    for y in 0..h {
        for x in 0..w {
            if img.get_pixel(x, y)[3] > threshold {
                return y;
            }
        }
    }
    h
}
pub fn find_bottom_padding(img: &RgbaImage, threshold: u8) -> u32 {
    let (w, h) = img.dimensions();
    for y in (0..h).rev() {
        for x in 0..w {
            if img.get_pixel(x, y)[3] > threshold {
                return h.saturating_sub(1).saturating_sub(y);
            }
        }
    }
    h
}
pub fn find_left_padding(img: &RgbaImage, threshold: u8) -> u32 {
    let (w, h) = img.dimensions();
    for x in 0..w {
        for y in 0..h {
            if img.get_pixel(x, y)[3] > threshold {
                return x;
            }
        }
    }
    w
}
pub fn find_right_padding(img: &RgbaImage, threshold: u8) -> u32 {
    let (w, h) = img.dimensions();
    for x in (0..w).rev() {
        for y in 0..h {
            if img.get_pixel(x, y)[3] > threshold {
                return w.saturating_sub(1).saturating_sub(x);
            }
        }
    }
    w
}
pub fn has_opaque_pixels(img: &RgbaImage, threshold: u8) -> bool {
    img.pixels().any(|p| p[3] > threshold)
}
pub fn visible_height(img: &RgbaImage, threshold: u8) -> u32 {
    let (_, h) = img.dimensions();
    // Visible height ignores transparent padding around skin digits and judgment sprites.
    let top = find_top_padding(img, threshold);
    let bot = find_bottom_padding(img, threshold);
    h.saturating_sub(top).saturating_sub(bot).max(1)
}
pub fn premultiply_alpha(img: &mut RgbaImage) {
    for pixel in img.pixels_mut() {
        let a = pixel[3] as f32 / 255.0;
        pixel[0] = (pixel[0] as f32 * a).round() as u8;
        pixel[1] = (pixel[1] as f32 * a).round() as u8;
        pixel[2] = (pixel[2] as f32 * a).round() as u8;
    }
}
pub fn unpremultiply_alpha(img: &mut RgbaImage) {
    for pixel in img.pixels_mut() {
        let alpha_u8 = pixel[3];
        if alpha_u8 == 0 {
            pixel[0] = 0;
            pixel[1] = 0;
            pixel[2] = 0;
            continue;
        }
        let alpha = alpha_u8 as f32 / 255.0;
        pixel[0] = (pixel[0] as f32 / alpha).round().clamp(0.0, 255.0) as u8;
        pixel[1] = (pixel[1] as f32 / alpha).round().clamp(0.0, 255.0) as u8;
        pixel[2] = (pixel[2] as f32 / alpha).round().clamp(0.0, 255.0) as u8;
    }
}
pub fn apply_red_tint(img: &RgbaImage, intensity: f32) -> RgbaImage {
    let mut out = img.clone();
    let inv = 1.0 - intensity;
    for pixel in out.pixels_mut() {
        let r = pixel[0] as f32;
        let g = pixel[1] as f32;
        let b = pixel[2] as f32;
        pixel[0] = (r + (255.0 - r) * intensity * 0.5).min(255.0) as u8;
        pixel[1] = (g * inv).round() as u8;
        pixel[2] = (b * inv).round() as u8;
    }
    out
}
pub fn recolor_to_combo_break_red(img: &RgbaImage) -> RgbaImage {
    let mut out = img.clone();
    for pixel in out.pixels_mut() {
        if pixel[3] > 0 {
            pixel[0] = 255;
            pixel[1] = 0;
            pixel[2] = 0;
        }
    }
    out
}
#[derive(Debug, Clone, Copy, Default)]
pub struct DigitHeightInfo {
    pub visible: u32,
    pub total: u32,
    pub top_pad: u32,
    pub bottom_pad: u32,
}
impl DigitHeightInfo {
    pub fn from_image(img: &RgbaImage) -> Self {
        let (_, h) = img.dimensions();
        let top = find_top_padding(img, ALPHA_THRESHOLD_HIGH);
        let bot = find_bottom_padding(img, ALPHA_THRESHOLD_HIGH);
        let visible = h.saturating_sub(top).saturating_sub(bot).max(1);
        Self {
            visible,
            total: h,
            top_pad: top,
            bottom_pad: bot,
        }
    }
}
pub fn extract_top(img: &RgbaImage, height: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    let crop_h = height.min(h);
    image::imageops::crop_imm(img, 0, 0, w, crop_h).to_image()
}
pub fn extract_vertical_range(img: &RgbaImage, start_y: u32, height: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    if h == 0 {
        return RgbaImage::new(w.max(1), 1);
    }
    let clamped_start = start_y.min(h.saturating_sub(1));
    let max_height = h.saturating_sub(clamped_start).max(1);
    let crop_h = height.min(max_height).max(1);
    image::imageops::crop_imm(img, 0, clamped_start, w, crop_h).to_image()
}

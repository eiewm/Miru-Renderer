use image::{DynamicImage, ImageBuffer, Rgba, RgbaImage};
use std::path::Path;
const DEFAULT_BG: [u8; 4] = [0, 0, 0, 255];
pub fn create_blurred_background(
    image_path: Option<&Path>,
    width: u32,
    height: u32,
    background_blur_percent: Option<u8>,
) -> Vec<u8> {
    if let Some(p) = image_path {
        if p.exists() {
            if let Ok(img) = image::open(p) {
                return process_background(img, width, height, background_blur_percent);
            }
        }
    }
    solid_background(width, height)
}
fn solid_background(width: u32, height: u32) -> Vec<u8> {
    let pixel_count = (width * height) as usize;
    let mut buf = Vec::with_capacity(pixel_count * 4);
    for _ in 0..pixel_count {
        buf.extend_from_slice(&DEFAULT_BG);
    }
    buf
}
fn process_background(
    img: DynamicImage,
    target_w: u32,
    target_h: u32,
    background_blur_percent: Option<u8>,
) -> Vec<u8> {
    let resized = crate::utils::image_proc::resize_cover(&img.to_rgba8(), target_w, target_h);
    let sigma = intro_background_blur_sigma(background_blur_percent);
    let blurred = crate::utils::image_proc::apply_gaussian_blur(&resized, sigma);
    let darkened = darken(&blurred, 0.6);
    darkened.into_raw()
}
fn intro_background_blur_sigma(background_blur_percent: Option<u8>) -> f32 {
    match background_blur_percent {
        Some(percent) => crate::utils::image_proc::background_blur_sigma_from_percent(percent),
        // Older configs predate the percent slider and expect the original intro blur strength.
        None => crate::utils::image_proc::LEGACY_INTRO_BACKGROUND_BLUR_SIGMA,
    }
}
fn darken(img: &RgbaImage, factor: f32) -> RgbaImage {
    let (w, h) = img.dimensions();
    let mut out = ImageBuffer::new(w, h);
    for (x, y, pixel) in img.enumerate_pixels() {
        let Rgba([r, g, b, a]) = *pixel;
        out.put_pixel(
            x,
            y,
            Rgba([
                (r as f32 * factor).round().min(255.0) as u8,
                (g as f32 * factor).round().min(255.0) as u8,
                (b as f32 * factor).round().min(255.0) as u8,
                a,
            ]),
        );
    }
    out
}
pub fn apply_opacity(buf: &mut [u8], opacity: f32) {
    if opacity >= 1.0 {
        return;
    }
    // Intro opacity darkens the backdrop; keeping alpha opaque avoids blending with transparent video.
    for chunk in buf.chunks_exact_mut(4) {
        chunk[0] = (chunk[0] as f32 * opacity).round() as u8;
        chunk[1] = (chunk[1] as f32 * opacity).round() as u8;
        chunk[2] = (chunk[2] as f32 * opacity).round() as u8;
    }
}

use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use std::path::Path;
const DEFAULT_AVATAR_GUEST: &[u8] = include_bytes!("../../assets/avatar-guest.png");
const PRIMARY_AVATAR: &[u8] = include_bytes!("../../assets/miru-avatar.png");
pub fn create_avatar(avatar_path: Option<&Path>, size: u32, border_radius: u32) -> RgbaImage {
    let img = load_avatar_image(avatar_path);
    let resized = resize_cover(&img, size, size);
    let masked = apply_rounded_mask(&resized, border_radius);
    add_border(&masked, border_radius)
}
fn load_avatar_image(path: Option<&Path>) -> DynamicImage {
    if let Some(p) = path {
        if p.exists() {
            if let Ok(img) = image::open(p) {
                return img;
            }
        }
        // Intro configs may store bundled avatar names instead of real paths.
        if let Some(bytes) = embedded_avatar_for_path(p) {
            if let Ok(img) = image::load_from_memory(bytes) {
                return img;
            }
        }
    }
    image::load_from_memory(DEFAULT_AVATAR_GUEST).unwrap_or_else(|_| {
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(64, 64, Rgba([0x4a, 0x4a, 0x6a, 255])))
    })
}
fn embedded_avatar_for_path(path: &Path) -> Option<&'static [u8]> {
    let name = path.file_name()?.to_string_lossy().to_lowercase();
    if name == "miru-avatar.png" {
        Some(PRIMARY_AVATAR)
    } else if name == "avatar-guest.png" {
        Some(DEFAULT_AVATAR_GUEST)
    } else {
        None
    }
}
fn resize_cover(img: &DynamicImage, w: u32, h: u32) -> RgbaImage {
    let (src_w, src_h) = img.dimensions();
    let scale = (w as f32 / src_w as f32).max(h as f32 / src_h as f32);
    let scaled_w = (src_w as f32 * scale).round() as u32;
    let scaled_h = (src_h as f32 * scale).round() as u32;
    let resized = img.resize_exact(scaled_w, scaled_h, image::imageops::FilterType::Lanczos3);
    let crop_x = (scaled_w.saturating_sub(w)) / 2;
    let crop_y = (scaled_h.saturating_sub(h)) / 2;
    resized.crop_imm(crop_x, crop_y, w, h).to_rgba8()
}
pub(crate) fn apply_rounded_mask(img: &RgbaImage, r: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    let r = r.min(w / 2).min(h / 2);
    let r2 = (r * r) as i32;
    let mut out = img.clone();
    for y in 0..h {
        for x in 0..w {
            // Corner pixels outside the radius become transparent while the center stays untouched.
            let in_corner = |cx: u32, cy: u32| -> bool {
                let dx = cx.abs_diff(x);
                let dy = cy.abs_diff(y);
                (dx * dx + dy * dy) as i32 > r2
            };
            let hide = (x < r && y < r && in_corner(r, r))
                || (x >= w - r && y < r && in_corner(w - r - 1, r))
                || (x < r && y >= h - r && in_corner(r, h - r - 1))
                || (x >= w - r && y >= h - r && in_corner(w - r - 1, h - r - 1));
            if hide {
                out.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            }
        }
    }
    out
}
fn add_border(img: &RgbaImage, r: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    let mut out = img.clone();
    let border_color = Rgba([255, 255, 255, 128]);
    let r = r.min(w / 2).min(h / 2);
    for x in r..(w - r) {
        blend_pixel(&mut out, x, 0, border_color);
        blend_pixel(&mut out, x, 1, border_color);
        blend_pixel(&mut out, x, h - 1, border_color);
        blend_pixel(&mut out, x, h - 2, border_color);
    }
    for y in r..(h - r) {
        blend_pixel(&mut out, 0, y, border_color);
        blend_pixel(&mut out, 1, y, border_color);
        blend_pixel(&mut out, w - 1, y, border_color);
        blend_pixel(&mut out, w - 2, y, border_color);
    }
    draw_corner_arc(&mut out, r, r, r, border_color, 0);
    draw_corner_arc(&mut out, w - r - 1, r, r, border_color, 1);
    draw_corner_arc(&mut out, r, h - r - 1, r, border_color, 2);
    draw_corner_arc(&mut out, w - r - 1, h - r - 1, r, border_color, 3);
    out
}
fn draw_corner_arc(img: &mut RgbaImage, cx: u32, cy: u32, r: u32, color: Rgba<u8>, quadrant: u8) {
    if r == 0 {
        return;
    }
    // The arc is a two-pixel ring so the border follows the rounded mask edge.
    let r_inner = (r.saturating_sub(2)) as i32;
    let r_outer = r as i32;
    for i in 0..=r {
        for j in 0..=r {
            let dist2 = (i * i + j * j) as i32;
            let inside = dist2 <= r_outer * r_outer && dist2 >= r_inner * r_inner;
            if inside {
                let (px, py) = match quadrant {
                    0 => (cx.saturating_sub(i), cy.saturating_sub(j)),
                    1 => (cx + i, cy.saturating_sub(j)),
                    2 => (cx.saturating_sub(i), cy + j),
                    _ => (cx + i, cy + j),
                };
                if px < img.width() && py < img.height() {
                    blend_pixel(img, px, py, color);
                }
            }
        }
    }
}
fn blend_pixel(img: &mut RgbaImage, x: u32, y: u32, color: Rgba<u8>) {
    let dst = img.get_pixel(x, y);
    let src_a = color.0[3] as f32 / 255.0;
    let dst_a = dst.0[3] as f32 / 255.0;
    let out_a = src_a + dst_a * (1.0 - src_a);
    if out_a > 0.001 {
        let blend = |s: u8, d: u8| -> u8 {
            ((s as f32 * src_a + d as f32 * dst_a * (1.0 - src_a)) / out_a).round() as u8
        };
        img.put_pixel(
            x,
            y,
            Rgba([
                blend(color.0[0], dst.0[0]),
                blend(color.0[1], dst.0[1]),
                blend(color.0[2], dst.0[2]),
                (out_a * 255.0).round() as u8,
            ]),
        );
    }
}

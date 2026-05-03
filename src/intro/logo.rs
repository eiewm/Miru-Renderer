use image::{DynamicImage, GenericImageView, RgbaImage};
use std::path::Path;
const DEFAULT_LOGO_BYTES: &[u8] = include_bytes!("../../assets/logo1.png");
pub struct Logo {
    pub image: RgbaImage,
    pub original_width: u32,
    pub original_height: u32,
    pub base_width: u32,
    pub base_height: u32,
}
impl Logo {
    pub fn load(logo_path: Option<&Path>, canvas_w: u32, canvas_h: u32) -> Option<Self> {
        let img = if let Some(p) = logo_path {
            if p.exists() {
                image::open(p).ok()
            } else {
                Self::load_embedded()
            }
        } else {
            Self::load_embedded()
        };
        let img = img?;
        let (orig_w, orig_h) = img.dimensions();
        let max_w = (canvas_w as f32 * 0.55) as u32;
        let max_h = (canvas_h as f32 * 0.55) as u32;
        let scale = (max_w as f32 / orig_w as f32).min(max_h as f32 / orig_h as f32);
        let base_w = (orig_w as f32 * scale).round() as u32;
        let base_h = (orig_h as f32 * scale).round() as u32;
        let resized = img.resize_exact(base_w, base_h, image::imageops::FilterType::Lanczos3);
        Some(Self {
            image: resized.to_rgba8(),
            original_width: orig_w,
            original_height: orig_h,
            base_width: base_w,
            base_height: base_h,
        })
    }
    fn load_embedded() -> Option<DynamicImage> {
        image::load_from_memory(DEFAULT_LOGO_BYTES).ok()
    }
    pub fn scaled(&self, scale: f32) -> RgbaImage {
        let w = (self.base_width as f32 * scale).round() as u32;
        let h = (self.base_height as f32 * scale).round() as u32;
        image::imageops::resize(&self.image, w, h, image::imageops::FilterType::Triangle)
    }
    pub fn center_pos(&self, canvas_w: u32, canvas_h: u32) -> (i32, i32) {
        let x = (canvas_w as i32 - self.base_width as i32) / 2;
        let y = (canvas_h as i32 - self.base_height as i32) / 2 - (canvas_h as f32 * 0.02) as i32;
        (x, y)
    }
    pub fn scaled_center_pos(&self, scale: f32, canvas_w: u32, canvas_h: u32) -> (i32, i32) {
        let (base_x, base_y) = self.center_pos(canvas_w, canvas_h);
        let scaled_w = (self.base_width as f32 * scale).round() as i32;
        let scaled_h = (self.base_height as f32 * scale).round() as i32;
        // Pulse scaling grows around the base center instead of drifting from the top-left corner.
        let x = base_x - (scaled_w - self.base_width as i32) / 2;
        let y = base_y - (scaled_h - self.base_height as i32) / 2;
        (x, y)
    }
}
pub fn apply_logo_opacity(img: &RgbaImage, opacity: f32) -> RgbaImage {
    if opacity >= 1.0 {
        return img.clone();
    }
    let mut out = img.clone();
    for pixel in out.pixels_mut() {
        pixel.0[3] = (pixel.0[3] as f32 * opacity).round() as u8;
    }
    out
}
pub fn composite_logo(
    bg: &mut [u8],
    logo: &RgbaImage,
    pos_x: i32,
    pos_y: i32,
    canvas_w: u32,
    canvas_h: u32,
) {
    let (logo_w, logo_h) = logo.dimensions();
    for ly in 0..logo_h {
        let dst_y = pos_y + ly as i32;
        if dst_y < 0 || dst_y >= canvas_h as i32 {
            continue;
        }
        for lx in 0..logo_w {
            let dst_x = pos_x + lx as i32;
            if dst_x < 0 || dst_x >= canvas_w as i32 {
                continue;
            }
            let src = logo.get_pixel(lx, ly).0;
            let src_a = src[3] as f32 / 255.0;
            if src_a < 0.001 {
                continue;
            }
            let idx = ((dst_y as u32 * canvas_w + dst_x as u32) * 4) as usize;
            // The intro buffer is straight alpha, so blend RGB against the destination alpha.
            let dst_a = bg[idx + 3] as f32 / 255.0;
            let out_a = src_a + dst_a * (1.0 - src_a);
            if out_a > 0.001 {
                let inv_out = 1.0 / out_a;
                bg[idx] = ((src[0] as f32 * src_a + bg[idx] as f32 * dst_a * (1.0 - src_a))
                    * inv_out) as u8;
                bg[idx + 1] = ((src[1] as f32 * src_a + bg[idx + 1] as f32 * dst_a * (1.0 - src_a))
                    * inv_out) as u8;
                bg[idx + 2] = ((src[2] as f32 * src_a + bg[idx + 2] as f32 * dst_a * (1.0 - src_a))
                    * inv_out) as u8;
                bg[idx + 3] = (out_a * 255.0) as u8;
            }
        }
    }
}

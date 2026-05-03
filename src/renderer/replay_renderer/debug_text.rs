use crate::intro::{render_text_simple, FontWeight};
use crate::types::JudgmentKind;
use image::RgbaImage;
use std::collections::HashMap;
pub struct DebugTextCache {
    textures: HashMap<String, DebugTextureEntry>,
    frame_id: u64,
}
impl Default for DebugTextCache {
    fn default() -> Self {
        Self::new()
    }
}
impl DebugTextCache {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            frame_id: 0,
        }
    }
    #[inline]
    fn next_frame_id(&mut self) -> u64 {
        self.frame_id = self.frame_id.wrapping_add(1);
        self.frame_id
    }
    pub fn len(&self) -> usize {
        self.textures.len()
    }
    pub fn compact(&mut self, max_entries: usize) {
        if self.textures.len() <= max_entries {
            return;
        }
        // Debug text can create one texture per note, so prune least-recently-used entries.
        let mut order: Vec<(String, u64)> = self
            .textures
            .iter()
            .map(|(k, v)| (k.clone(), v.last_used_frame))
            .collect();
        order.sort_by_key(|(_, frame)| *frame);
        let remove_count = self.textures.len().saturating_sub(max_entries);
        for (key, _) in order.into_iter().take(remove_count) {
            self.textures.remove(&key);
        }
    }
    pub fn get_or_create(
        &mut self,
        idx: usize,
        judgment: JudgmentKind,
        target_width: u32,
    ) -> Option<DebugTextureView<'_>> {
        let cache_key = format!("dbg|{}|{:?}|{}", idx, judgment, target_width);
        let frame = self.next_frame_id();
        if !self.textures.contains_key(&cache_key) {
            let (data, w, h) = self.create_debug_texture(idx, judgment, target_width)?;
            self.textures.insert(
                cache_key.clone(),
                DebugTextureEntry {
                    texture: DebugTexture { data, w, h },
                    last_used_frame: frame,
                },
            );
        }
        let entry = self.textures.get_mut(&cache_key)?;
        entry.last_used_frame = frame;
        let tex = &entry.texture;
        Some(DebugTextureView {
            data: tex.data.as_slice(),
            w: tex.w,
            h: tex.h,
        })
    }
    pub fn get_or_create_owned(
        &mut self,
        idx: usize,
        judgment: JudgmentKind,
        target_width: u32,
    ) -> Option<(Vec<u8>, u32, u32)> {
        let tex = self.get_or_create(idx, judgment, target_width)?;
        Some((tex.data.to_vec(), tex.w, tex.h))
    }
    fn create_debug_texture(
        &self,
        idx: usize,
        _judgment: JudgmentKind,
        target_width: u32,
    ) -> Option<(Vec<u8>, u32, u32)> {
        let font_size = (target_width as f32 * 0.38).max(13.0).min(24.0);
        let idx_text = idx.to_string();
        let idx_rendered =
            render_text_simple(&idx_text, font_size, [255, 255, 255, 255], FontWeight::Bold)?;
        let combined_width = idx_rendered.width.max(target_width);
        let combined_height = idx_rendered.height;
        let mut combined = RgbaImage::new(combined_width, combined_height);
        let idx_x = (combined_width.saturating_sub(idx_rendered.width)) / 2;
        copy_image_centered(&mut combined, &idx_rendered.image, idx_x, 0);
        let data = combined.into_raw();
        Some((data, combined_width, combined_height))
    }
    pub fn get_or_create_tail(
        &mut self,
        judgment: JudgmentKind,
        target_width: u32,
    ) -> Option<DebugTextureView<'_>> {
        let cache_key = format!("tail|{:?}|{}", judgment, target_width);
        let frame = self.next_frame_id();
        if !self.textures.contains_key(&cache_key) {
            let (data, w, h) = self.create_tail_texture(judgment, target_width)?;
            self.textures.insert(
                cache_key.clone(),
                DebugTextureEntry {
                    texture: DebugTexture { data, w, h },
                    last_used_frame: frame,
                },
            );
        }
        let entry = self.textures.get_mut(&cache_key)?;
        entry.last_used_frame = frame;
        let tex = &entry.texture;
        Some(DebugTextureView {
            data: tex.data.as_slice(),
            w: tex.w,
            h: tex.h,
        })
    }
    pub fn get_or_create_tail_owned(
        &mut self,
        judgment: JudgmentKind,
        target_width: u32,
    ) -> Option<(Vec<u8>, u32, u32)> {
        let tex = self.get_or_create_tail(judgment, target_width)?;
        Some((tex.data.to_vec(), tex.w, tex.h))
    }
    fn create_tail_texture(
        &self,
        judgment: JudgmentKind,
        target_width: u32,
    ) -> Option<(Vec<u8>, u32, u32)> {
        let font_size = (target_width as f32 * 0.5).max(12.0).min(26.0);
        let judgment_text = judgment_to_string(judgment);
        let color = judgment_to_color(judgment);
        let rendered = render_text_simple(&judgment_text, font_size, color, FontWeight::Bold)?;
        let final_width = rendered.width.max(target_width);
        if final_width == rendered.width {
            let w = rendered.width;
            let h = rendered.height;
            let data = rendered.image.into_raw();
            Some((data, w, h))
        } else {
            let mut img = RgbaImage::new(final_width, rendered.height);
            let x_offset = (final_width - rendered.width) / 2;
            copy_image_centered(&mut img, &rendered.image, x_offset, 0);
            let data = img.into_raw();
            Some((data, final_width, rendered.height))
        }
    }
}
#[derive(Debug, Clone)]
struct DebugTexture {
    data: Vec<u8>,
    w: u32,
    h: u32,
}
#[derive(Debug, Clone)]
struct DebugTextureEntry {
    texture: DebugTexture,
    last_used_frame: u64,
}
pub struct DebugTextureView<'a> {
    pub data: &'a [u8],
    pub w: u32,
    pub h: u32,
}
fn copy_image_centered(dest: &mut RgbaImage, src: &RgbaImage, x_offset: u32, y_offset: u32) {
    for (x, y, pixel) in src.enumerate_pixels() {
        let dest_x = x + x_offset;
        let dest_y = y + y_offset;
        if dest_x < dest.width() && dest_y < dest.height() {
            let src_alpha = pixel[3] as f32 / 255.0;
            if src_alpha > 0.0 {
                let dest_pixel = dest.get_pixel_mut(dest_x, dest_y);
                let dest_alpha = dest_pixel[3] as f32 / 255.0;
                if dest_alpha == 0.0 {
                    *dest_pixel = *pixel;
                } else {
                    // Text layers use straight alpha and may overlap when centered into one texture.
                    let out_alpha = src_alpha + dest_alpha * (1.0 - src_alpha);
                    for i in 0..3 {
                        let src_c = pixel[i] as f32 / 255.0;
                        let dest_c = dest_pixel[i] as f32 / 255.0;
                        let out_c = (src_c * src_alpha + dest_c * dest_alpha * (1.0 - src_alpha))
                            / out_alpha;
                        dest_pixel[i] = (out_c * 255.0).round() as u8;
                    }
                    dest_pixel[3] = (out_alpha * 255.0).round() as u8;
                }
            }
        }
    }
}
fn judgment_to_string(j: JudgmentKind) -> String {
    match j {
        JudgmentKind::Max => "MAX".to_string(),
        JudgmentKind::Hit300 => "300".to_string(),
        JudgmentKind::Hit200 => "200".to_string(),
        JudgmentKind::Hit100 => "100".to_string(),
        JudgmentKind::Hit50 => "50".to_string(),
        JudgmentKind::Miss => "MISS".to_string(),
    }
}
fn judgment_to_color(j: JudgmentKind) -> [u8; 4] {
    match j {
        JudgmentKind::Max => [255, 255, 255, 255],
        JudgmentKind::Hit300 => [249, 164, 2, 255],
        JudgmentKind::Hit200 => [43, 237, 71, 255],
        JudgmentKind::Hit100 => [68, 72, 242, 255],
        JudgmentKind::Hit50 => [180, 59, 235, 255],
        JudgmentKind::Miss => [196, 29, 49, 255],
    }
}

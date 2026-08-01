use super::error::RendererError;
use super::render::{
    lazer_replay_mod_texture_name, screen_right_hud_scale, stable_replay_mod_texture_stem,
    ReplayRenderer, LEGACY_COMBO_LAYOUT_SCALE, LEGACY_SONG_PROGRESS_LAYOUT_BASE_SIZE,
};
use crate::intro::generated_mod_icon_record;
use crate::types::ReplayOrigin;
use std::collections::BTreeSet;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
// Cap gameplay skin uploads so a large or hostile skin cannot exhaust GPU memory.
const MAX_GAMEPLAY_TEXTURE_AREA: u64 = 16_777_216;
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct SkinTextureLoadSummary {
    total_images: usize,
    selected_images: usize,
    uploaded_images: usize,
    aliased_images: usize,
    downscaled_images: usize,
    skipped_images: usize,
}
#[derive(Debug, Clone)]
pub struct GpuStats {
    pub initialized: bool,
    pub textures_loaded: bool,
    pub texture_count: usize,
    pub width: u32,
    pub height: u32,
    pub ln_bodies_prescaled: bool,
    pub combo_break_textures_created: bool,
}
#[derive(Debug, Clone, Default)]
pub struct HudDigitHeightCache {
    pub score: Option<ScoreDigitInfo>,
    pub combo: Option<u32>,
    pub percent_top_pad: Option<u32>,
    pub percent_padding: Option<PercentPadding>,
}
#[derive(Debug, Clone, Copy)]
pub struct ScoreDigitInfo {
    pub visible: u32,
    pub total: u32,
    pub top_pad: u32,
}
#[derive(Debug, Clone, Copy)]
pub struct PercentPadding {
    pub left: u32,
    pub right: u32,
    pub has_opaque: bool,
    pub width: u32,
    pub height: u32,
}
#[derive(Debug, Clone, Copy)]
pub struct TextureMeta {
    pub w: u32,
    pub h: u32,
}
#[derive(Debug, Clone, Copy)]
struct HudDigitAsset<'a> {
    name: &'a str,
    data: &'a [u8],
    scale_factor: f32,
}
#[derive(Debug, Clone, Copy)]
pub struct OpaqueBounds {
    pub min_x: u32,
    pub min_y: u32,
    pub max_x: u32,
    pub max_y: u32,
}
impl OpaqueBounds {
    pub fn width(self) -> u32 {
        self.max_x.saturating_sub(self.min_x) + 1
    }
    pub fn height(self) -> u32 {
        self.max_y.saturating_sub(self.min_y) + 1
    }
}
impl ReplayRenderer {
    fn find_skin_image_for_replay_mod<'a>(
        skin: &'a crate::types::SkinAssets,
        requested_name: &str,
    ) -> Option<&'a [u8]> {
        if let Some(data) = skin.find_image(requested_name) {
            return Some(data);
        }
        let requested_name = crate::types::SkinAssets::normalize_key(requested_name);
        let basename = requested_name.rsplit('/').next()?;
        // Stable skins often place mod icons at the archive root even when the renderer asks by folder path.
        skin.images.iter().find_map(|(key, data)| {
            key.rsplit('/')
                .next()
                .filter(|candidate| *candidate == basename)
                .map(|_| data.as_slice())
        })
    }
    fn load_replay_mod_texture_from_bytes(&mut self, texture_id: &str, encoded: &[u8]) -> bool {
        let Some((width, height)) = crate::utils::get_dimensions(encoded) else {
            return false;
        };
        self.load_texture_raw(texture_id, encoded, width, height)
    }
    fn prepare_lazer_replay_mod_texture(&mut self, acronym: &str) {
        let texture_id = lazer_replay_mod_texture_name(acronym);
        if self.has_texture(&texture_id) {
            return;
        }
        let Some(record) = generated_mod_icon_record(acronym) else {
            return;
        };
        self.load_replay_mod_texture_from_bytes(&texture_id, record.asset_bytes);
    }
    fn stable_replay_mod_texture_candidates(stem: &str) -> [String; 4] {
        [
            format!("{stem}@2x.png"),
            format!("{stem}.png"),
            format!("{stem}@2x"),
            stem.to_string(),
        ]
    }
    fn prepare_stable_replay_mod_texture(
        &mut self,
        skin: &crate::types::SkinAssets,
        acronym: &str,
    ) {
        let Some(stem) = stable_replay_mod_texture_stem(acronym) else {
            return;
        };
        let candidates = Self::stable_replay_mod_texture_candidates(stem);
        let mut resolved = false;
        for texture_id in &candidates {
            if self.has_texture(texture_id) {
                resolved = true;
                continue;
            }
            let Some(data) = Self::find_skin_image_for_replay_mod(skin, texture_id) else {
                continue;
            };
            if self.load_replay_mod_texture_from_bytes(texture_id, data) {
                resolved = true;
            }
        }
        if resolved {
            return;
        }
        let Some(record) = generated_mod_icon_record(acronym) else {
            return;
        };
        if let Some(texture_id) = candidates.get(1) {
            self.load_replay_mod_texture_from_bytes(texture_id, record.asset_bytes);
        }
    }
    pub fn prepare_replay_mod_textures(&mut self, skin: &crate::types::SkinAssets) {
        let Some(display) = self.replay_mod_display.clone() else {
            return;
        };
        for acronym in &display.acronyms {
            match display.origin {
                ReplayOrigin::StableLegacy => self.prepare_stable_replay_mod_texture(skin, acronym),
                ReplayOrigin::LazerExport => self.prepare_lazer_replay_mod_texture(acronym),
            }
        }
    }
    fn texture_upload_dimension_limit(&self) -> u32 {
        self.gpu_ctx
            .as_ref()
            .map(|ctx| {
                ctx.device
                    .limits()
                    .max_texture_dimension_2d
                    .min(crate::utils::image_proc::MAX_TEXTURE_DIM)
            })
            .unwrap_or(crate::utils::image_proc::MAX_TEXTURE_DIM)
    }
    fn strip_image_extension(name: &str) -> &str {
        name.trim_end_matches(".png")
            .trim_end_matches(".jpg")
            .trim_end_matches(".jpeg")
    }
    fn strip_hidpi_suffix(name: &str) -> &str {
        name.strip_suffix("@2x").unwrap_or(name)
    }
    fn canonical_texture_stem(name: &str) -> String {
        let normalized = crate::types::SkinAssets::normalize_key(name);
        let without_ext = Self::strip_image_extension(&normalized);
        Self::strip_hidpi_suffix(without_ext).to_string()
    }
    fn texture_family_matches(requested: &str, candidate: &str) -> bool {
        let requested_stem = Self::canonical_texture_stem(requested);
        let candidate_stem = Self::canonical_texture_stem(candidate);
        // Animated skin frames share the requested stem with a numeric suffix.
        candidate_stem == requested_stem || candidate_stem.starts_with(&(requested_stem + "-"))
    }
    fn extend_selected_texture_family(
        &self,
        selected: &mut BTreeSet<String>,
        image_keys: &[String],
        requested_name: &str,
    ) {
        let normalized = crate::types::SkinAssets::normalize_key(requested_name);
        if normalized.is_empty() {
            return;
        }
        if image_keys.iter().any(|key| key == &normalized) {
            selected.insert(normalized.clone());
        }
        for key in image_keys {
            if Self::texture_family_matches(&normalized, key) {
                selected.insert(key.clone());
            }
        }
        for ext in ["png", "jpg", "jpeg"] {
            let exact = format!("{}.{}", Self::strip_image_extension(&normalized), ext);
            if image_keys.iter().any(|key| key == &exact) {
                selected.insert(exact);
            }
        }
    }
    fn collect_gameplay_texture_keys(&self, skin: &crate::types::SkinAssets) -> Vec<String> {
        let image_keys = skin.images.keys().cloned().collect::<Vec<_>>();
        let mut selected = BTreeSet::new();
        let mut requested = Vec::new();
        let push_name = |requested: &mut Vec<String>, name: &str| {
            let normalized = crate::types::SkinAssets::normalize_key(name);
            if !normalized.is_empty() {
                requested.push(normalized);
            }
        };
        let push_opt = |requested: &mut Vec<String>, name: Option<&String>| {
            if let Some(name) = name {
                push_name(requested, name);
            }
        };
        for name in &skin.config.note_image {
            push_name(&mut requested, name);
        }
        for name in &skin.config.note_image_h {
            push_name(&mut requested, name);
        }
        for name in &skin.config.note_image_l {
            push_name(&mut requested, name);
        }
        for name in &skin.config.note_image_t {
            push_name(&mut requested, name);
        }
        for name in &skin.config.key_image {
            push_name(&mut requested, name);
        }
        for name in &skin.config.key_image_d {
            push_name(&mut requested, name);
        }
        for name in &skin.config.lighting_n {
            push_name(&mut requested, name);
        }
        for name in &skin.config.lighting_l {
            push_name(&mut requested, name);
        }
        push_opt(&mut requested, skin.config.stage_left.as_ref());
        push_opt(&mut requested, skin.config.stage_right.as_ref());
        push_opt(&mut requested, skin.config.stage_bottom.as_ref());
        push_opt(&mut requested, skin.config.stage_hint.as_ref());
        push_opt(&mut requested, skin.config.stage_light.as_ref());
        push_opt(&mut requested, skin.config.warning_arrow.as_ref());
        for name in &skin.config.combo_burst {
            push_name(&mut requested, name);
        }
        for name in [
            skin.config.hit_0.as_ref(),
            skin.config.hit_50.as_ref(),
            skin.config.hit_100.as_ref(),
            skin.config.hit_200.as_ref(),
            skin.config.hit_300.as_ref(),
            skin.config.hit_300g.as_ref(),
        ] {
            push_opt(&mut requested, name);
        }
        let score_prefix = skin.config.score_prefix_or_default();
        let combo_prefix = skin.config.combo_prefix_or_default();
        for digit in 0..=9 {
            requested.push(format!("{score_prefix}-{digit}.png"));
            requested.push(format!("{combo_prefix}-{digit}.png"));
        }
        for name in [
            format!("{score_prefix}-dot.png"),
            format!("{score_prefix}-comma.png"),
            format!("{score_prefix}-percent.png"),
            format!("{combo_prefix}-x.png"),
            "scorebar-bg".to_string(),
            "scorebar-colour".to_string(),
            "scorebar-marker".to_string(),
            "scorebar-ki".to_string(),
            "scorebar-kidanger".to_string(),
            "scorebar-kidanger2".to_string(),
            "mania-hit0".to_string(),
            "mania-hit50".to_string(),
            "mania-hit100".to_string(),
            "mania-hit200".to_string(),
            "mania-hit300".to_string(),
            "mania-hit300g".to_string(),
            "mania-note1".to_string(),
            "mania-note2".to_string(),
            "mania-note3".to_string(),
            "mania-note4".to_string(),
            "mania-notes".to_string(),
            "mania-note1h".to_string(),
            "mania-note2h".to_string(),
            "mania-note3h".to_string(),
            "mania-note4h".to_string(),
            "mania-notesh".to_string(),
            "mania-note1l".to_string(),
            "mania-note2l".to_string(),
            "mania-note3l".to_string(),
            "mania-note4l".to_string(),
            "mania-notesl".to_string(),
            "mania-note1t".to_string(),
            "mania-note2t".to_string(),
            "mania-note3t".to_string(),
            "mania-note4t".to_string(),
            "mania-notest".to_string(),
            "mania-key1".to_string(),
            "mania-key2".to_string(),
            "mania-key3".to_string(),
            "mania-key4".to_string(),
            "mania-keys".to_string(),
            "mania-key1d".to_string(),
            "mania-key2d".to_string(),
            "mania-key3d".to_string(),
            "mania-key4d".to_string(),
            "mania-keysd".to_string(),
            "mania-stage-left".to_string(),
            "mania-stage-right".to_string(),
            "mania-stage-bottom".to_string(),
            "mania-stage-hint".to_string(),
            "mania-stage-light".to_string(),
            "mania-warningarrow".to_string(),
            "lightingn".to_string(),
            "lightingl".to_string(),
            "comboburst-mania".to_string(),
            "comboburst".to_string(),
        ] {
            requested.push(name);
        }
        for name in requested {
            self.extend_selected_texture_family(&mut selected, &image_keys, &name);
        }
        selected.into_iter().collect()
    }
    fn collect_linear_sampled_gameplay_texture_keys(
        &self,
        skin: &crate::types::SkinAssets,
    ) -> BTreeSet<String> {
        let image_keys = skin.images.keys().cloned().collect::<Vec<_>>();
        let mut selected = BTreeSet::new();
        let mut requested = Vec::new();
        let push_name = |requested: &mut Vec<String>, name: &str| {
            let normalized = crate::types::SkinAssets::normalize_key(name);
            if !normalized.is_empty() {
                requested.push(normalized);
            }
        };
        for name in &skin.config.note_image {
            push_name(&mut requested, name);
        }
        for name in &skin.config.note_image_h {
            push_name(&mut requested, name);
        }
        for name in &skin.config.note_image_l {
            push_name(&mut requested, name);
        }
        for name in &skin.config.note_image_t {
            push_name(&mut requested, name);
        }
        for name in requested {
            self.extend_selected_texture_family(&mut selected, &image_keys, &name);
        }
        selected
    }
    fn resolve_texture_upload_source_name(
        skin: &crate::types::SkinAssets,
        requested_name: &str,
    ) -> Option<String> {
        let requested = crate::types::SkinAssets::normalize_key(requested_name);
        if requested.is_empty() {
            return None;
        }
        let stem = Self::strip_image_extension(&requested);
        for ext in ["png", "jpg", "jpeg"] {
            let candidate = format!("{stem}.{ext}");
            if skin.images.contains_key(&candidate) {
                return Some(candidate);
            }
        }
        skin.images.contains_key(&requested).then_some(requested)
    }
    fn alias_loaded_texture(&mut self, alias: &str, source: &str) -> bool {
        let alias = crate::types::SkinAssets::normalize_key(alias);
        let source = crate::types::SkinAssets::normalize_key(source);
        if alias.is_empty() || source.is_empty() {
            return false;
        }
        if alias == source {
            return self.loaded_textures.contains(&source);
        }
        if let Some(texture) = self.gpu_textures.get(&source).cloned() {
            self.gpu_textures.insert(alias.clone(), texture);
        } else if !self.loaded_textures.contains(&source) {
            return false;
        }
        self.loaded_textures.insert(alias.clone());
        if let Some(meta) = self.texture_meta.get(&source).copied() {
            self.texture_meta.insert(alias.clone(), meta);
        }
        if let Some(bounds) = self.texture_opaque_bounds.get(&source).copied() {
            self.texture_opaque_bounds.insert(alias.clone(), bounds);
        }
        if self.linear_sampled_textures.contains(&source) {
            self.linear_sampled_textures.insert(alias);
        }
        true
    }
    fn prepare_skin_texture_for_upload(&self, img: image::RgbaImage) -> (image::RgbaImage, bool) {
        let max_dim = self.texture_upload_dimension_limit().max(1);
        let (orig_w, orig_h) = img.dimensions();
        let mut scale = 1.0f64;
        // Respect both device texture limits and the project area cap before uploading untrusted skin images.
        if orig_w > max_dim || orig_h > max_dim {
            scale = scale.min(
                (max_dim as f64 / orig_w.max(1) as f64).min(max_dim as f64 / orig_h.max(1) as f64),
            );
        }
        let area = orig_w as u64 * orig_h as u64;
        if area > MAX_GAMEPLAY_TEXTURE_AREA {
            scale = scale.min((MAX_GAMEPLAY_TEXTURE_AREA as f64 / area as f64).sqrt());
        }
        if scale >= 1.0 {
            return (img, false);
        }
        let target_w = ((orig_w as f64 * scale).round() as u32).clamp(1, max_dim);
        let target_h = ((orig_h as f64 * scale).round() as u32).clamp(1, max_dim);
        let resized = image::imageops::resize(
            &img,
            target_w,
            target_h,
            image::imageops::FilterType::Lanczos3,
        );
        (resized, true)
    }
    fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
        if let Some(message) = payload.downcast_ref::<String>() {
            message.clone()
        } else if let Some(message) = payload.downcast_ref::<&str>() {
            (*message).to_string()
        } else {
            "unknown panic".to_string()
        }
    }
    fn try_load_texture_rgba(
        &mut self,
        name: &str,
        rgba: &[u8],
        w: u32,
        h: u32,
    ) -> Result<(), String> {
        if rgba.len() != (w * h * 4) as usize {
            return Err(format!("invalid rgba buffer size for {name} ({w}x{h})"));
        }
        let name = crate::types::SkinAssets::normalize_key(name);
        let ctx = match &self.gpu_ctx {
            Some(ctx) => ctx.clone(),
            None => {
                // Headless paths still record metadata so layout code can run without a GPU context.
                self.loaded_textures.insert(name.clone());
                self.texture_meta.insert(name, TextureMeta { w, h });
                return Ok(());
            }
        };
        // wgpu may panic on invalid device state; convert that into a skipped texture warning.
        let texture = catch_unwind(AssertUnwindSafe(|| {
            let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(name.as_str()),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            ctx.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * w),
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
            texture
        }))
        .map_err(Self::panic_message)?;
        if self.gpu_textures.contains_key(&name) {
            if let Some(pipeline) = self.pipeline.as_mut() {
                pipeline.clear_bind_group_cache();
            }
        }
        self.gpu_textures.insert(name.clone(), Arc::new(texture));
        self.loaded_textures.insert(name.clone());
        self.texture_meta.insert(name, TextureMeta { w, h });
        Ok(())
    }
    fn try_update_texture_rgba(
        &mut self,
        name: &str,
        rgba: &[u8],
        w: u32,
        h: u32,
    ) -> Result<bool, String> {
        if rgba.len() != (w * h * 4) as usize {
            return Err(format!("invalid rgba buffer size for {name} ({w}x{h})"));
        }
        let name = crate::types::SkinAssets::normalize_key(name);
        let ctx = match &self.gpu_ctx {
            Some(ctx) => ctx.clone(),
            None => return Ok(false),
        };
        let Some(texture) = self.gpu_textures.get(&name).cloned() else {
            return Ok(false);
        };
        let Some(meta) = self.texture_meta.get(&name).copied() else {
            return Ok(false);
        };
        if meta.w != w || meta.h != h {
            return Ok(false);
        }
        catch_unwind(AssertUnwindSafe(|| {
            ctx.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: texture.as_ref(),
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * w),
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
        }))
        .map_err(Self::panic_message)?;
        self.loaded_textures.insert(name.clone());
        self.texture_meta.insert(name, TextureMeta { w, h });
        Ok(true)
    }
    fn scaled_hud_texture_width(orig_w: u32, orig_h: u32, target_h: u32) -> u32 {
        if orig_h == 0 || target_h == 0 {
            return 0;
        }
        ((orig_w as f64 / orig_h as f64) * target_h as f64)
            .round()
            .clamp(1.0, crate::utils::image_proc::MAX_TEXTURE_DIM as f64) as u32
    }
    fn clamp_legacy_percent_width(target_w: u32, target_h: u32) -> u32 {
        target_w.min(target_h.saturating_mul(2).max(1))
    }
    fn compute_opaque_bounds(img: &image::RgbaImage) -> Option<OpaqueBounds> {
        let mut min_x = img.width();
        let mut min_y = img.height();
        let mut max_x = 0u32;
        let mut max_y = 0u32;
        let mut found = false;
        for (x, y, pixel) in img.enumerate_pixels() {
            if pixel[3] == 0 {
                continue;
            }
            found = true;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        found.then_some(OpaqueBounds {
            min_x,
            min_y,
            max_x,
            max_y,
        })
    }
    pub fn create_common_textures(&mut self) {
        if !self.initialized {
            return;
        }
        if !self.has_texture("solid_black") {
            self.create_solid_texture("solid_black", 4, 4, [0, 0, 0, 255]);
        }
        if !self.has_texture("solid_white") {
            self.create_solid_texture("solid_white", 4, 4, [255, 255, 255, 255]);
        }
        if !self.has_texture("solid_gray") {
            self.create_solid_texture("solid_gray", 4, 4, [70, 70, 70, 255]);
        }
        if !self.has_texture("vertical_black_fade") {
            self.create_vertical_fade_texture("vertical_black_fade", 4, 256, [0, 0, 0]);
        }
        self.pre_create_progress_circle_textures();
    }
    pub fn preload_debug_textures(
        &mut self,
        judgments: &[crate::renderer::replay_renderer::RenderJudgment],
        column_width: u32,
    ) {
        use crate::types::JudgmentKind;
        if !self.ln_debug || !self.initialized {
            return;
        }
        for j in judgments {
            let texture_name = Self::get_debug_texture_name(j.idx, j.kind);
            if self.has_texture(&texture_name) {
                continue;
            }
            if let Some((data, w, h)) =
                self.debug_text_cache
                    .get_or_create_owned(j.idx, j.kind, column_width)
            {
                self.load_texture_rgba(&texture_name, &data, w, h);
            }
        }
        let all_judgments = [
            JudgmentKind::Max,
            JudgmentKind::Hit300,
            JudgmentKind::Hit200,
            JudgmentKind::Hit100,
            JudgmentKind::Hit50,
            JudgmentKind::Miss,
        ];
        for kind in &all_judgments {
            let texture_name = Self::get_debug_tail_texture_name(*kind);
            if self.has_texture(&texture_name) {
                continue;
            }
            if let Some((data, w, h)) = self
                .debug_text_cache
                .get_or_create_tail_owned(*kind, column_width)
            {
                self.load_texture_rgba(&texture_name, &data, w, h);
            }
        }
    }
    pub fn get_debug_texture_name(idx: usize, kind: crate::types::JudgmentKind) -> String {
        format!("debug_note_{}_{:?}", idx, kind).to_ascii_lowercase()
    }
    pub fn get_debug_tail_texture_name(kind: crate::types::JudgmentKind) -> String {
        format!("debug_tail_{:?}", kind).to_ascii_lowercase()
    }
    fn create_solid_texture(&mut self, name: &str, w: u32, h: u32, rgba: [u8; 4]) {
        let mut img = image::RgbaImage::new(w, h);
        for pixel in img.pixels_mut() {
            *pixel = image::Rgba(rgba);
        }
        self.load_texture_rgba(name, img.as_raw(), w, h);
    }
    fn create_vertical_fade_texture(&mut self, name: &str, w: u32, h: u32, rgb: [u8; 3]) {
        let mut img = image::RgbaImage::new(w, h);
        let max_y = h.saturating_sub(1).max(1) as f32;
        for y in 0..h {
            let alpha = ((1.0 - y as f32 / max_y) * 255.0).round() as u8;
            for x in 0..w {
                img.put_pixel(x, y, image::Rgba([rgb[0], rgb[1], rgb[2], alpha]));
            }
        }
        self.load_texture_rgba(name, img.as_raw(), w, h);
    }
    fn pre_create_progress_circle_textures(&mut self) {
        let size = (LEGACY_SONG_PROGRESS_LAYOUT_BASE_SIZE * screen_right_hud_scale(self.cfg.width, self.cfg.height))
            .round()
            .max(1.0) as u32;
        // Progress circles are quantized to percent steps so frame rendering only binds a cached texture.
        for pct in 0..=100 {
            let name = format!("progress_circle_{}", pct);
            if self
                .texture_meta(&name)
                .is_some_and(|meta| meta.w == size && meta.h == size)
            {
                continue;
            }
            let progress = pct as f32 / 100.0;
            let img = Self::create_circle_texture(
                size,
                progress,
                [255, 255, 255, 255],
                [127, 127, 127, 200],
            );
            self.load_texture_rgba(&name, img.as_raw(), size, size);
        }
    }
    fn create_circle_texture(
        size: u32,
        progress: f32,
        outline_rgba: [u8; 4],
        fill_rgba: [u8; 4],
    ) -> image::RgbaImage {
        let mut img = image::RgbaImage::new(size, size);
        let cx = size as f32 / 2.0;
        let cy = size as f32 / 2.0;
        let r = (size as f32 / 2.0) - 1.0;
        let stroke = 2.0;
        let p = progress.clamp(0.0, 1.0);
        let start_angle = -std::f32::consts::FRAC_PI_2;
        let end_angle = start_angle + p * std::f32::consts::PI * 2.0;
        for y in 0..size {
            for x in 0..size {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let dx = px - cx;
                let dy = py - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist <= r {
                    let angle = dy.atan2(dx);
                    let mut rel_angle = angle - start_angle;
                    if rel_angle < 0.0 {
                        rel_angle += std::f32::consts::PI * 2.0;
                    }
                    let wedge_angle = end_angle - start_angle;
                    let in_wedge = p >= 1.0 || (p > 0.0 && rel_angle <= wedge_angle);
                    if in_wedge && dist <= r - stroke {
                        img.put_pixel(x, y, image::Rgba(fill_rgba));
                    } else if dist > r - stroke {
                        img.put_pixel(x, y, image::Rgba(outline_rgba));
                    }
                }
            }
        }
        let dot_size = 3i32;
        let dot_start = (size as i32 / 2) - dot_size / 2;
        for dy in 0..dot_size {
            for dx in 0..dot_size {
                let px = (dot_start + dx) as u32;
                let py = (dot_start + dy) as u32;
                if px < size && py < size {
                    img.put_pixel(px, py, image::Rgba(outline_rgba));
                }
            }
        }
        img
    }
    pub fn texture_count(&self) -> usize {
        self.loaded_textures.len()
    }
    pub fn has_texture(&self, name: &str) -> bool {
        self.loaded_textures.contains(name)
    }
    pub fn texture_meta(&self, name: &str) -> Option<TextureMeta> {
        self.texture_meta.get(name).copied()
    }
    pub fn load_skin_texture_metadata_for_layout(
        &mut self,
        skin: &crate::types::SkinAssets,
    ) -> usize {
        let selected_names = self.collect_gameplay_texture_keys(skin);
        let mut summary = SkinTextureLoadSummary {
            total_images: skin.images.len(),
            selected_images: selected_names.len(),
            ..Default::default()
        };

        for requested_name in selected_names {
            if self.loaded_textures.contains(&requested_name) {
                continue;
            }
            let Some(source_name) = Self::resolve_texture_upload_source_name(skin, &requested_name)
            else {
                continue;
            };
            if self.loaded_textures.contains(&source_name) {
                if self.alias_loaded_texture(&requested_name, &source_name) {
                    summary.aliased_images += 1;
                }
                continue;
            }
            let Some(data) = skin.images.get(&source_name) else {
                continue;
            };
            let Some(img) = crate::utils::image_proc::load_rgba(data) else {
                summary.skipped_images += 1;
                continue;
            };
            let (prepared, downscaled) = self.prepare_skin_texture_for_upload(img);
            let meta = TextureMeta {
                w: prepared.width(),
                h: prepared.height(),
            };
            let opaque_bounds = Self::compute_opaque_bounds(&prepared);

            self.loaded_textures.insert(source_name.clone());
            self.texture_meta.insert(source_name.clone(), meta);
            if let Some(bounds) = opaque_bounds {
                self.texture_opaque_bounds
                    .insert(source_name.clone(), bounds);
            } else {
                self.texture_opaque_bounds.remove(&source_name);
            }
            summary.uploaded_images += 1;
            if downscaled {
                summary.downscaled_images += 1;
            }
            if requested_name != source_name
                && self.alias_loaded_texture(&requested_name, &source_name)
            {
                summary.aliased_images += 1;
            }
        }

        println!(
            "   [skin] gameplay texture metadata: total={} selected={} loaded={} aliased={} downscaled={} skipped={}",
            summary.total_images,
            summary.selected_images,
            summary.uploaded_images,
            summary.aliased_images,
            summary.downscaled_images,
            summary.skipped_images
        );
        self.precompute_hud_digit_heights(skin);
        self.preload_combo_break_red_textures(skin);
        self.textures_loaded = true;
        summary.uploaded_images + summary.aliased_images
    }
    pub fn load_skin_textures(
        &mut self,
        skin: &crate::types::SkinAssets,
    ) -> Result<usize, RendererError> {
        if self.gpu_ctx.is_none() || !self.initialized {
            return Err(RendererError::NotInitialized);
        }
        // Upload only textures reachable from the resolved mania config instead of the whole skin archive.
        let selected_names = self.collect_gameplay_texture_keys(skin);
        let linear_sampled_names = self.collect_linear_sampled_gameplay_texture_keys(skin);
        let mut summary = SkinTextureLoadSummary {
            total_images: skin.images.len(),
            selected_images: selected_names.len(),
            ..Default::default()
        };
        for requested_name in selected_names {
            if self.loaded_textures.contains(&requested_name) {
                continue;
            }
            let Some(source_name) = Self::resolve_texture_upload_source_name(skin, &requested_name)
            else {
                continue;
            };
            if self.loaded_textures.contains(&source_name) {
                if self.alias_loaded_texture(&requested_name, &source_name) {
                    summary.aliased_images += 1;
                }
                continue;
            }
            let Some(data) = skin.images.get(&source_name) else {
                continue;
            };
            let Some(img) = crate::utils::image_proc::load_rgba(data) else {
                continue;
            };
            let should_use_linear_sampling = linear_sampled_names.contains(&requested_name)
                || linear_sampled_names.contains(&source_name);
            let (prepared, downscaled) = self.prepare_skin_texture_for_upload(img);
            let meta = TextureMeta {
                w: prepared.width(),
                h: prepared.height(),
            };
            let opaque_bounds = Self::compute_opaque_bounds(&prepared);
            match self.try_load_texture_rgba(
                &source_name,
                prepared.as_raw(),
                prepared.width(),
                prepared.height(),
            ) {
                Ok(()) => {
                    if should_use_linear_sampling {
                        self.linear_sampled_textures.insert(source_name.clone());
                    }
                    self.texture_meta.insert(source_name.clone(), meta);
                    if let Some(bounds) = opaque_bounds {
                        self.texture_opaque_bounds
                            .insert(source_name.clone(), bounds);
                    } else {
                        self.texture_opaque_bounds.remove(&source_name);
                    }
                    summary.uploaded_images += 1;
                    if downscaled {
                        summary.downscaled_images += 1;
                    }
                    if should_use_linear_sampling && requested_name == source_name {
                        self.linear_sampled_textures.insert(requested_name.clone());
                    }
                    if requested_name != source_name
                        && self.alias_loaded_texture(&requested_name, &source_name)
                    {
                        summary.aliased_images += 1;
                    }
                }
                Err(error) => {
                    println!(
                        "   warn: skipped gameplay texture {} (source {}) - {}",
                        requested_name, source_name, error
                    );
                    summary.skipped_images += 1;
                }
            }
        }
        println!(
            "   [skin] gameplay textures: total={} selected={} uploaded={} aliased={} downscaled={} skipped={}",
            summary.total_images,
            summary.selected_images,
            summary.uploaded_images,
            summary.aliased_images,
            summary.downscaled_images,
            summary.skipped_images
        );
        self.precompute_hud_digit_heights(skin);
        self.preload_combo_break_red_textures(skin);
        self.textures_loaded = true;
        Ok(summary.uploaded_images + summary.aliased_images)
    }
    pub fn load_texture_raw(&mut self, name: &str, data: &[u8], w: u32, h: u32) -> bool {
        let rgba_data = match image::load_from_memory(data) {
            Ok(img) => img.to_rgba8().into_raw(),
            Err(_) => {
                if data.len() == (w * h * 4) as usize {
                    return self.load_texture_rgba(name, data, w, h);
                }
                return false;
            }
        };
        match self.try_load_texture_rgba(name, &rgba_data, w, h) {
            Ok(()) => true,
            Err(error) => {
                println!("   warn: texture upload skipped for {} - {}", name, error);
                false
            }
        }
    }
    pub fn load_texture_rgba(&mut self, name: &str, rgba: &[u8], w: u32, h: u32) -> bool {
        match self.try_load_texture_rgba(name, rgba, w, h) {
            Ok(()) => true,
            Err(error) => {
                println!("   warn: texture upload skipped for {} - {}", name, error);
                false
            }
        }
    }
    pub fn update_or_load_texture_rgba(&mut self, name: &str, rgba: &[u8], w: u32, h: u32) -> bool {
        let normalized_name = crate::types::SkinAssets::normalize_key(name);
        match self.try_update_texture_rgba(name, rgba, w, h) {
            Ok(true) => true,
            Ok(false) => {
                if self.gpu_textures.contains_key(&normalized_name) {
                    if let Some(pipeline) = self.pipeline.as_mut() {
                        pipeline.clear_bind_group_cache();
                    }
                }
                self.load_texture_rgba(name, rgba, w, h)
            }
            Err(error) => {
                println!("   warn: texture update skipped for {} - {}", name, error);
                false
            }
        }
    }
    fn precompute_hud_digit_heights(&mut self, skin: &crate::types::SkinAssets) {
        let score_prefix = skin.config.score_prefix_or_default();
        let combo_prefix = skin.config.combo_prefix_or_default();
        let score_digit = Self::find_digit_texture(skin, score_prefix, "score");
        if let Some(asset) = score_digit {
            if let Some(img) = crate::utils::image_proc::load_rgba(asset.data) {
                let info = crate::utils::image_proc::DigitHeightInfo::from_image(&img);
                self.hud_digit_cache.score = Some(ScoreDigitInfo {
                    visible: Self::logical_digit_px(info.visible, asset.scale_factor),
                    total: Self::logical_digit_px(info.total, asset.scale_factor),
                    top_pad: Self::logical_digit_px_allow_zero(info.top_pad, asset.scale_factor),
                });
            }
        }
        if self.hud_digit_cache.score.is_none() {
            self.hud_digit_cache.score = Some(ScoreDigitInfo {
                visible: 30,
                total: 30,
                top_pad: 0,
            });
        }
        let combo_digit = Self::find_digit_texture(skin, combo_prefix, "combo");
        if let Some(asset) = combo_digit {
            if let Some(img) = crate::utils::image_proc::load_rgba(asset.data) {
                let info = crate::utils::image_proc::DigitHeightInfo::from_image(&img);
                let visible = Self::logical_digit_px(info.visible, asset.scale_factor);
                let top_pad = Self::logical_digit_px_allow_zero(info.top_pad, asset.scale_factor);
                let bottom_pad =
                    Self::logical_digit_px_allow_zero(info.bottom_pad, asset.scale_factor);
                let total_padding = top_pad + bottom_pad;
                // Legacy combo layout keeps a small share of transparent padding for vertical rhythm.
                let tec = visible + (total_padding * 4 / 10).min(7);
                self.hud_digit_cache.combo = Some(tec);
            }
        }
        if self.hud_digit_cache.combo.is_none() {
            self.hud_digit_cache.combo = Some(40);
        }
        self.compute_percent_padding(skin, score_prefix);
    }
    fn logical_digit_px(value: u32, scale_factor: f32) -> u32 {
        (value as f32 / scale_factor.max(1.0)).round().max(1.0) as u32
    }
    fn logical_digit_px_allow_zero(value: u32, scale_factor: f32) -> u32 {
        if value == 0 {
            0
        } else {
            Self::logical_digit_px(value, scale_factor)
        }
    }
    fn digit_scale_factor(name: &str) -> f32 {
        let normalized = crate::types::SkinAssets::normalize_key(name);
        let stem = Self::strip_image_extension(&normalized);
        if stem.ends_with("@2x") {
            2.0
        } else {
            1.0
        }
    }
    fn canonical_hud_digit_name(name: &str) -> String {
        let normalized = crate::types::SkinAssets::normalize_key(name);
        if let Some(stem) = normalized.strip_suffix("@2x.png") {
            format!("{stem}.png")
        } else if let Some(stem) = normalized.strip_suffix("@2x.jpg") {
            format!("{stem}.jpg")
        } else if let Some(stem) = normalized.strip_suffix("@2x.jpeg") {
            format!("{stem}.jpeg")
        } else {
            normalized
        }
    }
    fn digit_texture_candidates(
        prefix: &str,
        fallback_prefix: &str,
        char_name: &str,
        prefer_hidpi: bool,
    ) -> Vec<String> {
        let primary = format!("{prefix}-{char_name}");
        let fallback = format!("{fallback_prefix}-{char_name}");
        let stems = if primary == fallback {
            vec![primary]
        } else {
            vec![primary, fallback]
        };
        let mut candidates = Vec::with_capacity(stems.len() * 2);
        for stem in stems {
            if prefer_hidpi {
                candidates.push(format!("{stem}@2x.png"));
                candidates.push(format!("{stem}.png"));
            } else {
                candidates.push(format!("{stem}.png"));
                candidates.push(format!("{stem}@2x.png"));
            }
        }
        candidates
    }
    fn find_digit_asset<'a>(
        skin: &'a crate::types::SkinAssets,
        prefix: &str,
        fallback_prefix: &str,
        char_name: &str,
        prefer_hidpi: bool,
    ) -> Option<HudDigitAsset<'a>> {
        let candidates =
            Self::digit_texture_candidates(prefix, fallback_prefix, char_name, prefer_hidpi);
        let refs = candidates.iter().map(String::as_str).collect::<Vec<_>>();
        let (name, data) = skin.find_first(&refs)?;
        Some(HudDigitAsset {
            name,
            data,
            scale_factor: Self::digit_scale_factor(name),
        })
    }
    fn find_digit_texture<'a>(
        skin: &'a crate::types::SkinAssets,
        prefix: &str,
        fallback_prefix: &str,
    ) -> Option<HudDigitAsset<'a>> {
        for d in ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'] {
            if let Some(asset) =
                Self::find_digit_asset(skin, prefix, fallback_prefix, &d.to_string(), false)
            {
                return Some(asset);
            }
        }
        None
    }
    fn compute_percent_padding(&mut self, skin: &crate::types::SkinAssets, score_prefix: &str) {
        let score_info = self.hud_digit_cache.score.unwrap_or(ScoreDigitInfo {
            visible: 30,
            total: 30,
            top_pad: 0,
        });
        let h_acc = (score_info.total as f32 * 0.6).round().max(1.0) as u32;
        let percent_asset = Self::find_digit_asset(skin, score_prefix, "score", "percent", true);
        if let Some(asset) = percent_asset {
            if let Some(img) = crate::utils::image_proc::load_rgba(asset.data) {
                let (orig_w, orig_h) = img.dimensions();
                if orig_h == 0 {
                    self.hud_digit_cache.percent_top_pad = Some(score_info.top_pad);
                    return;
                }
                // The percent glyph is sized from score digits but clamped so wide custom art cannot push accuracy apart.
                let target_w = Self::clamp_legacy_percent_width(
                    Self::scaled_hud_texture_width(orig_w, orig_h, h_acc).max(1),
                    h_acc,
                );
                let resized = crate::utils::image_proc::resize_exact_sprite_nearest(
                    &img,
                    target_w.max(1),
                    h_acc,
                );
                let has_opaque = crate::utils::image_proc::has_opaque_pixels(&resized, 0);
                if has_opaque {
                    let min_cols = (target_w as f32 * 0.02).floor().max(1.0) as u32;
                    let top_pad = Self::find_top_pad_with_min_cols(&resized, 0, min_cols);
                    self.hud_digit_cache.percent_top_pad = Some(top_pad);
                } else {
                    self.hud_digit_cache.percent_top_pad = Some(score_info.top_pad);
                }
                let left = crate::utils::image_proc::find_left_padding(
                    &resized,
                    crate::utils::image_proc::ALPHA_THRESHOLD_HIGH,
                );
                let right = crate::utils::image_proc::find_right_padding(
                    &resized,
                    crate::utils::image_proc::ALPHA_THRESHOLD_HIGH,
                );
                let resized_has_opaque = crate::utils::image_proc::has_opaque_pixels(
                    &resized,
                    crate::utils::image_proc::ALPHA_THRESHOLD_HIGH,
                );
                self.hud_digit_cache.percent_padding = Some(PercentPadding {
                    left,
                    right,
                    has_opaque: resized_has_opaque,
                    width: resized.width(),
                    height: resized.height(),
                });
            }
        } else {
            self.hud_digit_cache.percent_top_pad = Some(score_info.top_pad);
        }
    }
    fn find_top_pad_with_min_cols(img: &image::RgbaImage, threshold: u8, min_cols: u32) -> u32 {
        let (w, h) = img.dimensions();
        for y in 0..h {
            let mut cnt = 0u32;
            for x in 0..w {
                if img.get_pixel(x, y)[3] > threshold {
                    cnt += 1;
                    if cnt >= min_cols {
                        return y;
                    }
                }
            }
        }
        h
    }
    pub fn prescale_hud_digits(&mut self, skin: &crate::types::SkinAssets) {
        let score_info = self.hud_digit_cache.score.unwrap_or(ScoreDigitInfo {
            visible: 30,
            total: 30,
            top_pad: 0,
        });
        let h_score = score_info.total;
        let h_acc = (score_info.total as f32 * 0.6).round().max(1.0) as u32;
        let scale_y = self.cfg.height as f32 / 480.0;
        let native_combo_h = self.hud_digit_cache.combo.unwrap_or(40);
        let h_combo = (native_combo_h as f32 * scale_y * LEGACY_COMBO_LAYOUT_SCALE)
            .round()
            .max(1.0) as u32;
        let score_prefix = skin.config.score_prefix_or_default();
        let combo_prefix = skin.config.combo_prefix_or_default();
        // HUD numbers are prescaled once per render size to avoid nearest-neighbor resizing in every frame.
        let score_chars = [
            "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "dot", "comma",
        ];
        for ch in score_chars {
            self.prescale_digit_char(skin, score_prefix, "score", ch, h_score, "hud_score");
        }
        let accuracy_chars = [
            "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "dot", "percent",
        ];
        for ch in accuracy_chars {
            self.prescale_digit_char(skin, score_prefix, "score", ch, h_acc, "hud_acc");
        }
        let combo_chars = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "x"];
        for ch in combo_chars {
            self.prescale_digit_char(skin, combo_prefix, "combo", ch, h_combo, "hud_combo");
        }
        let configured_combo_h = self.legacy_combo_height_for_current_hud_config(h_combo);
        if configured_combo_h != h_combo {
            for ch in combo_chars {
                self.prescale_digit_char(
                    skin,
                    combo_prefix,
                    "combo",
                    ch,
                    configured_combo_h,
                    "hud_combo",
                );
            }
        }
        self.prescale_judgment_textures(skin);
    }
    pub(super) fn legacy_combo_height_for_current_hud_config(&self, default_height: u32) -> u32 {
        let mut combo_height = default_height.max(1);
        let combo_cfg = self
            .hud_config
            .as_ref()
            .and_then(|config| config.elements.combo.as_ref());
        if let Some(cfg) = combo_cfg {
            if let Some(height) = cfg.height.filter(|value| value.is_finite() && *value > 0.0) {
                combo_height = height.round().max(1.0) as u32;
            } else if let Some(scale) = cfg
                .scale
                .or(cfg.size)
                .filter(|value| value.is_finite() && *value > 0.0)
            {
                combo_height = (combo_height as f32 * scale).round().max(1.0) as u32;
            }
        }
        combo_height
    }
    fn prescale_digit_char(
        &mut self,
        skin: &crate::types::SkinAssets,
        prefix: &str,
        fallback: &str,
        char_name: &str,
        target_h: u32,
        suffix: &str,
    ) {
        let Some(asset) = Self::find_digit_asset(skin, prefix, fallback, char_name, true) else {
            return;
        };
        let Some(img) = crate::utils::image_proc::load_rgba(asset.data) else {
            return;
        };
        let (orig_w, orig_h) = img.dimensions();
        let scaled_w = if char_name == "percent" {
            Self::clamp_legacy_percent_width(
                Self::scaled_hud_texture_width(orig_w, orig_h, target_h),
                target_h,
            )
        } else {
            Self::scaled_hud_texture_width(orig_w, orig_h, target_h)
        };
        if scaled_w == 0 {
            return;
        }
        let canonical_name = Self::canonical_hud_digit_name(asset.name);
        let scaled_name = format!("{}@{}_{}", canonical_name, suffix, target_h);
        if self.has_texture(&scaled_name) {
            return;
        }
        let scaled =
            crate::utils::image_proc::resize_exact_sprite_nearest(&img, scaled_w.max(1), target_h);
        self.load_texture_rgba(&scaled_name, scaled.as_raw(), scaled_w, target_h);
        if suffix == "hud_combo" {
            let red = crate::utils::image_proc::recolor_to_combo_break_red(&scaled);
            let red_name = format!("{scaled_name}@red");
            self.load_texture_rgba(&red_name, red.as_raw(), scaled_w, target_h);
            // Combo break digits reuse the same scaled geometry with a red recolor.
            self.combo_break_red_cache.insert(scaled_name, red_name);
        }
    }
    fn prescale_judgment_textures(&mut self, skin: &crate::types::SkinAssets) {
        let mut all_bases: Vec<String> = vec![
            "mania-hit0".to_string(),
            "mania-hit50".to_string(),
            "mania-hit100".to_string(),
            "mania-hit200".to_string(),
            "mania-hit300".to_string(),
            "mania-hit300g".to_string(),
        ];
        let custom_names = [
            &skin.config.hit_0,
            &skin.config.hit_50,
            &skin.config.hit_100,
            &skin.config.hit_200,
            &skin.config.hit_300,
            &skin.config.hit_300g,
        ];
        for name in custom_names.into_iter().flatten() {
            if name.is_empty() {
                continue;
            }
            let base = name
                .trim_end_matches(".png")
                .trim_end_matches(".jpg")
                .to_lowercase();
            if !all_bases.contains(&base) {
                all_bases.push(base);
            }
        }
        let mut root_files: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for key in skin.images.keys() {
            if !key.contains('/') && !key.contains('\\') {
                // Judgment popups are resolved from root-level skin files, matching stable mania skins.
                let basename = key.to_lowercase();
                root_files.insert(basename, key.clone());
            }
        }
        for base in all_bases {
            let mut frames: Vec<(String, Vec<u8>)> = Vec::new();
            let find_judgment = |name: &str| -> Option<(String, Vec<u8>)> {
                let bn = name.to_lowercase();
                if let Some(actual_key) = root_files.get(&bn) {
                    if let Some(data) = skin.images.get(actual_key) {
                        return Some((bn, data.clone()));
                    }
                }
                None
            };
            let anim_first = format!("{}-0.png", base);
            if find_judgment(&anim_first).is_some() {
                let mut idx = 0;
                loop {
                    let frame_name = format!("{}-{}.png", base, idx);
                    if let Some((norm_name, data)) = find_judgment(&frame_name) {
                        frames.push((norm_name, data));
                        idx += 1;
                    } else {
                        break;
                    }
                }
            } else {
                let static_name = format!("{}.png", base);
                if let Some((norm_name, data)) = find_judgment(&static_name) {
                    frames.push((norm_name, data));
                }
            }
            for (norm_name, data) in &frames {
                if let Some(mut img) = crate::utils::image_proc::load_rgba(data) {
                    let (w, h) = img.dimensions();
                    crate::utils::image_proc::premultiply_alpha(&mut img);
                    let scaled_name = format!("{}@judgment", norm_name);
                    self.load_texture_rgba(&scaled_name, img.as_raw(), w, h);
                    self.texture_meta
                        .insert(norm_name.clone(), TextureMeta { w, h });
                    self.texture_meta
                        .insert(scaled_name.clone(), TextureMeta { w, h });
                }
            }
        }
    }
    pub fn precompute_ln_body_atlases(
        &mut self,
        skin: &crate::types::SkinAssets,
        column_widths: &[u32],
    ) {
        let padding = skin.config.note_padding_x as u32;
        let width_ref = skin.config.width_for_note_height_scale;
        for (col, &col_width) in column_widths.iter().enumerate() {
            let note_idx = col.min(skin.config.note_image_l.len().saturating_sub(1));
            let body_name = skin
                .config
                .note_image_l
                .get(note_idx)
                .cloned()
                .unwrap_or_else(|| format!("mania-note{}L", col));
            let normalized = crate::types::SkinAssets::normalize_key(&body_name);
            let variants = [normalized.clone(), format!("{}.png", normalized)];
            let actual = variants.iter().find(|v| skin.images.contains_key(*v));
            let Some(actual_name) = actual else { continue };
            if self.ln_body_scaled.contains(actual_name) {
                continue;
            }
            let Some(data) = skin.images.get(actual_name) else {
                continue;
            };
            let Some(img) = crate::utils::image_proc::load_rgba(data) else {
                continue;
            };
            let (orig_w, orig_h) = img.dimensions();
            let body_w = col_width.saturating_sub(padding * 2).max(4);
            let ref_w = width_ref.unwrap_or(orig_w);
            let tile_h = ((orig_h as f32) * (body_w as f32 / ref_w as f32))
                .round()
                .max(1.0) as u32;
            let max_dim = crate::utils::image_proc::MAX_TEXTURE_DIM;
            let (final_tile_h, should_extract) = if tile_h > max_dim {
                (max_dim, true)
            } else {
                (tile_h, false)
            };
            // Oversized LN bodies keep the top slice; render.rs repeats the tile down the hold length.
            let scaled = if should_extract {
                let extract_h = ((max_dim as f32) * (ref_w as f32 / body_w as f32))
                    .round()
                    .min(orig_h as f32) as u32;
                let cropped = crate::utils::image_proc::extract_top(&img, extract_h);
                if self.linear_sampled_textures.contains(actual_name) {
                    crate::utils::image_proc::resize_exact_gameplay_note(
                        &cropped,
                        body_w,
                        final_tile_h,
                    )
                } else {
                    crate::utils::image_proc::resize_exact_sharp_upscale(
                        &cropped,
                        body_w,
                        final_tile_h,
                    )
                }
            } else if self.linear_sampled_textures.contains(actual_name) {
                crate::utils::image_proc::resize_exact_gameplay_note(&img, body_w, final_tile_h)
            } else {
                crate::utils::image_proc::resize_exact_sharp_upscale(&img, body_w, final_tile_h)
            };
            let scaled_name = format!("{}@scaled_{}x{}", actual_name, body_w, final_tile_h);
            self.load_texture_rgba(&scaled_name, scaled.as_raw(), body_w, final_tile_h);
            if self.linear_sampled_textures.contains(actual_name) {
                self.linear_sampled_textures.insert(scaled_name);
            }
            self.ln_body_scaled.insert(actual_name.clone());
        }
        self.ln_bodies_prescaled = true;
    }
    pub fn precompute_note_atlases(
        &mut self,
        skin: &crate::types::SkinAssets,
        column_widths: &[u32],
    ) {
        let padding = skin.config.note_padding_x as u32;
        let width_ref = skin.config.width_for_note_height_scale;
        for (col, &col_width) in column_widths.iter().enumerate() {
            let target_w = col_width.saturating_sub(padding * 2).max(1);
            let note_idx = col.min(skin.config.note_image.len().saturating_sub(1));
            if let Some(name) = skin
                .config
                .note_image
                .get(note_idx)
                .filter(|name| !name.is_empty())
            {
                self.prescale_note_texture(skin, name, target_w, width_ref);
            }
            let head_name = skin
                .config
                .note_image_h
                .get(note_idx)
                .or_else(|| skin.config.note_image.get(note_idx))
                .filter(|name| !name.is_empty());
            if let Some(name) = head_name {
                self.prescale_note_texture(skin, name, target_w, width_ref);
            }
            let tail_name = skin
                .config
                .note_image_t
                .get(note_idx)
                .or_else(|| skin.config.note_image.get(note_idx))
                .filter(|name| !name.is_empty());
            if let Some(name) = tail_name {
                self.prescale_note_texture(skin, name, target_w, width_ref);
            }
            if let Some(name) = skin
                .config
                .key_image
                .get(note_idx)
                .filter(|name| !name.is_empty())
            {
                self.prescale_key_texture(skin, name, target_w);
            }
            if let Some(name) = skin
                .config
                .key_image_d
                .get(note_idx)
                .filter(|name| !name.is_empty())
            {
                self.prescale_key_texture(skin, name, target_w);
            }
        }
    }
    fn prescale_note_texture(
        &mut self,
        skin: &crate::types::SkinAssets,
        name: &str,
        target_w: u32,
        width_for_note_height_scale: Option<u32>,
    ) {
        let Some(data) = skin.find_image(name) else {
            return;
        };
        let Some(img) = crate::utils::image_proc::load_rgba(data) else {
            return;
        };
        let (orig_w, orig_h) = img.dimensions();
        let scale_y = self.cfg.height as f32 / 480.0;
        let note_height = width_for_note_height_scale
            .map(|value| (value as f32 * scale_y).max(1.0))
            .unwrap_or(target_w.max(1) as f32);
        let target_h = ((orig_h as f32) * (note_height / orig_w.max(1) as f32))
            .round()
            .max(1.0) as u32;
        self.get_or_create_scaled_sprite(skin, name, target_w, target_h);
    }
    fn prescale_key_texture(&mut self, skin: &crate::types::SkinAssets, name: &str, target_w: u32) {
        let Some(data) = skin.find_image(name) else {
            return;
        };
        let Some(img) = crate::utils::image_proc::load_rgba(data) else {
            return;
        };
        let (_orig_w, orig_h) = img.dimensions();
        let scale_adjust = if name.contains("@2x") { 2.0 } else { 1.0 };
        let screen_scale = self.cfg.height.max(1) as f32 / 768.0;
        let target_h = ((orig_h.max(1) as f32) / scale_adjust * screen_scale)
            .round()
            .max(1.0) as u32;
        self.get_or_create_scaled_sprite(skin, name, target_w, target_h);
    }
    pub fn get_or_create_scaled_sprite(
        &mut self,
        skin: &crate::types::SkinAssets,
        name: &str,
        target_w: u32,
        target_h: u32,
    ) -> Option<String> {
        if name.is_empty() {
            return None;
        }
        let max_dim = crate::utils::image_proc::MAX_TEXTURE_DIM;
        let w = target_w.max(1).min(max_dim);
        let h = target_h.max(1).min(max_dim);
        let normalized_name = crate::types::SkinAssets::normalize_key(name);
        let cache_key = format!("{}|{}|{}", normalized_name, w, h);
        if let Some(cached) = self.sprite_scaled.get(&cache_key) {
            return Some(cached.clone());
        }
        if let Some(meta) = self.texture_meta.get(&normalized_name) {
            if meta.w == w && meta.h == h {
                self.sprite_scaled
                    .insert(cache_key, normalized_name.clone());
                return Some(normalized_name);
            }
        }
        let data = skin.find_image(&normalized_name)?;
        let img = crate::utils::image_proc::load_rgba(data)?;
        let scaled = if self.linear_sampled_textures.contains(&normalized_name) {
            crate::utils::image_proc::resize_exact_gameplay_note(&img, w, h)
        } else {
            crate::utils::image_proc::resize_exact_alpha_safe(&img, w, h)
        };
        let scaled_name =
            crate::types::SkinAssets::normalize_key(&format!("{}@{}x{}", normalized_name, w, h));
        if !self.load_texture_rgba(&scaled_name, scaled.as_raw(), w, h) {
            return None;
        }
        if self.linear_sampled_textures.contains(&normalized_name) {
            self.linear_sampled_textures.insert(scaled_name.clone());
        }
        self.sprite_scaled.insert(cache_key, scaled_name.clone());
        Some(scaled_name)
    }
    pub fn preload_combo_break_red_textures(&mut self, skin: &crate::types::SkinAssets) {
        if self.combo_break_textures_created {
            return;
        }
        let combo_prefix = skin.config.combo_prefix_or_default();
        let chars = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "x"];
        for ch in chars {
            let Some(asset) = Self::find_digit_asset(skin, combo_prefix, "combo", ch, true) else {
                continue;
            };
            let Some(img) = crate::utils::image_proc::load_rgba(asset.data) else {
                continue;
            };
            let (w, h) = img.dimensions();
            let red = crate::utils::image_proc::recolor_to_combo_break_red(&img);
            let canonical_name = Self::canonical_hud_digit_name(asset.name);
            let red_name = format!("{}@red", canonical_name);
            self.load_texture_rgba(&red_name, red.as_raw(), w, h);
            self.combo_break_red_cache.insert(canonical_name, red_name);
        }
        self.combo_break_textures_created = true;
    }
    pub fn get_gpu_stats(&self) -> GpuStats {
        GpuStats {
            initialized: self.initialized,
            textures_loaded: self.textures_loaded,
            texture_count: self.loaded_textures.len(),
            width: self.cfg.width,
            height: self.cfg.height,
            ln_bodies_prescaled: self.ln_bodies_prescaled,
            combo_break_textures_created: self.combo_break_textures_created,
        }
    }
}

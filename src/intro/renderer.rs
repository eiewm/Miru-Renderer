use crate::intro::{
    apply_rounded_mask, create_avatar, create_blurred_background, create_key_badge,
    create_mod_badges, create_mod_badges_from_specs, fade_opacity,
    fit_intro_mod_badge_specs_to_width, pulse_scale, render_text, render_text_fitted,
    FontWeight, IntroConfig,
    IntroFrame, Logo,
};
use crate::renderer::gpu::{
    GpuContext, SpriteBlendMode, SpriteInstance, SpritePipeline, TextureSampling,
};
use crate::utils::perf;
use image::{GenericImageView, RgbaImage};
use std::collections::VecDeque;
use std::sync::Arc;
struct IntroTextures {
    bg: Arc<wgpu::Texture>,
    logo: Arc<wgpu::Texture>,
    ui_overlay: Option<Arc<wgpu::Texture>>,
    logo_base_w: u32,
    logo_base_h: u32,
}
pub struct GpuIntroRenderer {
    pipeline: SpritePipeline,
    textures: IntroTextures,
    width: u32,
    height: u32,
}
impl GpuIntroRenderer {
    pub fn new(ctx: &GpuContext, cfg: &IntroConfig) -> Option<Self> {
        let w = cfg.width;
        let h = cfg.height;
        eprintln!("gpu-intro: pre-rendering {}x{}", w, h);
        let logo_path = if cfg.logo_path.as_os_str().is_empty() {
            None
        } else {
            Some(cfg.logo_path.as_path())
        };
        let logo = Logo::load(logo_path, w, h)?;
        let bg_path = cfg.background_path.as_deref();
        let bg_rgba = create_blurred_background(bg_path, w, h, cfg.background_blur_percent);
        let bg_tex = upload_rgba_texture(ctx, &bg_rgba, w, h, "intro_bg");
        let logo_tex = upload_rgba_texture(
            ctx,
            logo.image.as_raw(),
            logo.base_width,
            logo.base_height,
            "intro_logo",
        );
        let has_ui = cfg.player_name.is_some() || cfg.map_title.is_some();
        let ui_overlay = if has_ui {
            // Text and small UI assets are rasterized once because the per-frame GPU path only draws sprites.
            let ui_rgba = render_ui_overlay(cfg, &logo);
            Some(upload_rgba_texture(ctx, &ui_rgba, w, h, "intro_ui"))
        } else {
            None
        };
        let pipeline = SpritePipeline::new(ctx, w, h);
        Some(Self {
            pipeline,
            textures: IntroTextures {
                bg: bg_tex,
                logo: logo_tex,
                ui_overlay,
                logo_base_w: logo.base_width,
                logo_base_h: logo.base_height,
            },
            width: w,
            height: h,
        })
    }
    pub fn render_frames(&mut self, ctx: &GpuContext, cfg: &IntroConfig) -> Vec<IntroFrame> {
        let frame_count = ((cfg.duration_ms as f32 / 1000.0) * cfg.fps as f32).ceil() as u32;
        let width = self.width;
        let height = self.height;
        let mut frames = Vec::with_capacity(frame_count as usize);
        let result = self.render_frames_into(ctx, cfg, |_, time_ms, frame_data| {
            frames.push(IntroFrame::new(frame_data.to_vec(), time_ms, width, height));
            Ok::<(), std::convert::Infallible>(())
        });
        match result {
            Ok(()) => frames,
            Err(err) => match err {},
        }
    }
    pub fn render_frames_into<F, E>(
        &mut self,
        ctx: &GpuContext,
        cfg: &IntroConfig,
        mut on_frame: F,
    ) -> Result<(), E>
    where
        F: FnMut(u32, f32, &[u8]) -> Result<(), E>,
    {
        let frame_count = ((cfg.duration_ms as f32 / 1000.0) * cfg.fps as f32).ceil() as u32;
        let frame_dur = 1000.0 / cfg.fps as f32;
        // The sprite pipeline returns readback frames asynchronously, so metadata must follow FIFO order.
        let mut pending_meta: VecDeque<(u32, f32)> = VecDeque::with_capacity(frame_count as usize);
        eprintln!("gpu-intro: rendering {} frames", frame_count);
        let logo_center_x = (self.width as i32 - self.textures.logo_base_w as i32) / 2;
        let logo_center_y = (self.height as i32 - self.textures.logo_base_h as i32) / 2
            - (self.height as f32 * 0.02) as i32;
        for i in 0..frame_count {
            let time_ms = i as f32 * frame_dur;
            let scale = pulse_scale(time_ms, cfg.bpm);
            let opacity = fade_opacity(time_ms, cfg.duration_ms);
            let scaled_w = (self.textures.logo_base_w as f32 * scale).round() as u32;
            let scaled_h = (self.textures.logo_base_h as f32 * scale).round() as u32;
            let logo_x = logo_center_x - ((scaled_w - self.textures.logo_base_w) / 2) as i32;
            let logo_y = logo_center_y - ((scaled_h - self.textures.logo_base_h) / 2) as i32;
            let mut batches: Vec<(
                Arc<wgpu::Texture>,
                TextureSampling,
                SpriteBlendMode,
                Vec<SpriteInstance>,
            )> = Vec::new();
            batches.push((
                self.textures.bg.clone(),
                TextureSampling::Nearest,
                SpriteBlendMode::Alpha,
                vec![
                    SpriteInstance::new(0.0, 0.0, self.width as f32, self.height as f32)
                        .with_color(opacity, opacity, opacity, 1.0),
                ],
            ));
            batches.push((
                self.textures.logo.clone(),
                TextureSampling::Nearest,
                SpriteBlendMode::Alpha,
                vec![SpriteInstance::new(
                    logo_x as f32,
                    logo_y as f32,
                    scaled_w as f32,
                    scaled_h as f32,
                )
                .with_opacity(opacity)],
            ));
            if let Some(ref ui_tex) = self.textures.ui_overlay {
                batches.push((
                    ui_tex.clone(),
                    TextureSampling::Nearest,
                    SpriteBlendMode::Alpha,
                    vec![
                        SpriteInstance::new(0.0, 0.0, self.width as f32, self.height as f32)
                            .with_opacity(opacity),
                    ],
                ));
            }
            {
                let _scope = perf::scoped("render_frame");
                self.pipeline
                    .submit_batched(ctx, &batches, [0.0, 0.0, 0.0, 1.0]);
            }
            pending_meta.push_back((i, time_ms));
            while let Some(frame_data) = self.pipeline.poll_ready_frame(ctx) {
                let (frame_idx, frame_time_ms) =
                    pending_meta.pop_front().expect("intro frame metadata");
                on_frame(frame_idx, frame_time_ms, frame_data)?;
            }
            if i % 30 == 0 || i == frame_count - 1 {
                eprintln!("gpu-intro: {}/{}", i + 1, frame_count);
            }
        }
        while let Some(frame_data) = self.pipeline.drain_ready_frame_blocking(ctx) {
            let (frame_idx, frame_time_ms) =
                pending_meta.pop_front().expect("intro frame metadata");
            on_frame(frame_idx, frame_time_ms, frame_data)?;
        }
        eprintln!("gpu-intro: done");
        Ok(())
    }
}
fn upload_rgba_texture(
    ctx: &GpuContext,
    data: &[u8],
    width: u32,
    height: u32,
    label: &str,
) -> Arc<wgpu::Texture> {
    let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
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
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    Arc::new(texture)
}
fn render_ui_overlay(cfg: &IntroConfig, logo: &Logo) -> Vec<u8> {
    let w = cfg.width;
    let h = cfg.height;
    let mut canvas = vec![0u8; (w * h * 4) as usize];
    let text_scale = 1.15;
    let margin = (w as f32 * 0.03).round() as i32;
    let avatar_size = (h as f32 * 0.13).round() as u32;
    let avatar_radius = (avatar_size as f32 * 0.22).round() as u32;
    let name_font = (h as f32 * 0.04 * text_scale).round();
    let small_font = (h as f32 * 0.022 * text_scale).round();
    let title_font = (h as f32 * 0.032 * text_scale).round();
    let stats_font = (20.0 * text_scale).round();
    let avatar = create_avatar(cfg.avatar_path.as_deref(), avatar_size, avatar_radius);
    blit_image(&mut canvas, &avatar, margin, margin, w, h);
    let text_left = margin + avatar_size as i32 + 12;
    let name_h = (name_font * 1.5).round() as i32;
    let flag_h = (h as f32 * 0.022 * text_scale).round() as i32;
    let total_h = name_h + flag_h;
    let group_y = margin + ((avatar_size as i32 - total_h) / 2);
    if let Some(name) = &cfg.player_name {
        if let Some(rendered) = render_text(name, name_font, [255, 255, 255, 255], FontWeight::Bold)
        {
            blit_image(&mut canvas, &rendered.image, text_left - 4, group_y, w, h);
        }
    }
    let flag_row_y = group_y + name_h;
    let mut cur_x = text_left;
    if let Some(ref flag_path) = cfg.flag_path {
        if flag_path.exists() {
            if let Ok(flag_img) = image::open(flag_path) {
                let (fw, fh) = flag_img.dimensions();
                let target_h = flag_h as u32;
                let target_w = (target_h as f32 * fw as f32 / fh as f32).round() as u32;
                let resized = flag_img
                    .resize_exact(target_w, target_h, image::imageops::FilterType::Lanczos3)
                    .to_rgba8();
                let flag_radius = (target_h as f32 * 0.22).round().max(2.0) as u32;
                let rounded = apply_rounded_mask(&resized, flag_radius);
                blit_image(&mut canvas, &rounded, cur_x, flag_row_y, w, h);
                cur_x += target_w as i32 + 4;
            }
        }
    }
    if let Some(ref code) = cfg.country_code {
        if let Some(rendered) = render_text(
            &code.to_uppercase(),
            small_font,
            [200, 200, 200, 230],
            FontWeight::Normal,
        ) {
            blit_image(&mut canvas, &rendered.image, cur_x - 4, flag_row_y, w, h);
            cur_x += rendered.width as i32;
        }
    }
    if let Some(ref badge_path) = cfg.team_badge_path {
        if badge_path.exists() {
            if let Ok(badge_img) = image::open(badge_path) {
                let (bw, bh) = badge_img.dimensions();
                let target_h = flag_h as u32;
                let target_w = (target_h as f32 * bw as f32 / bh as f32).round() as u32;
                let resized = badge_img
                    .resize_exact(target_w, target_h, image::imageops::FilterType::Lanczos3)
                    .to_rgba8();
                cur_x += 6;
                blit_image(&mut canvas, &resized, cur_x, flag_row_y, w, h);
            }
        }
    }
    let mod_badge_size = (avatar_size as f32 * 0.6).round() as u32;
    let mod_spacing = 6i32;
    let mod_badges = cfg
        .display_mods
        .as_ref()
        .map(|mods| {
            let reserved_left = margin + avatar_size as i32 + 32;
            let max_total_width =
                (w as i32 - margin - reserved_left).max(mod_badge_size as i32) as u32;
            // Summary text is optional; badge icons keep priority when the row is narrow.
            let fitted_specs = fit_intro_mod_badge_specs_to_width(
                mods,
                mod_badge_size,
                mod_spacing as u32,
                max_total_width,
            );
            create_mod_badges_from_specs(&fitted_specs, mod_badge_size)
        })
        .unwrap_or_else(|| create_mod_badges(cfg.mods, mod_badge_size));
    let total_mods_w: i32 = mod_badges.iter().map(|b| b.width as i32).sum::<i32>()
        + (mod_badges.len().saturating_sub(1) as i32 * mod_spacing);
    let mut mod_x = w as i32 - margin - total_mods_w;
    for badge in &mod_badges {
        blit_image(&mut canvas, &badge.image, mod_x, margin, w, h);
        mod_x += badge.width as i32 + mod_spacing;
    }
    const MAP_INFO_LINE_EXTRA_GAP_PX: i32 = 3;
    let stats_base_y = (h as f32 * 0.87).round() as i32;
    let key_badge = create_key_badge(cfg.key_count, cfg.star_rating);
    let stats_str = format!(
        "{:.2}%  -  {}/{}",
        cfg.accuracy, cfg.best_combo, cfg.max_combo
    );
    let stats_text = render_text(
        &stats_str,
        stats_font,
        [255, 255, 255, 230],
        FontWeight::Normal,
    );
    let key_w = key_badge.as_ref().map(|b| b.width).unwrap_or(55);
    let key_h = key_badge.as_ref().map(|b| b.height).unwrap_or(32);
    let stats_w = stats_text.as_ref().map(|s| s.width).unwrap_or(100);
    let bottom_margin = (h as f32 * 0.03).round() as i32;
    let max_stats_y = h as i32 - bottom_margin - key_h as i32;
    let stats_y = (stats_base_y + MAP_INFO_LINE_EXTRA_GAP_PX * 2)
        .min(max_stats_y)
        .max(0);
    let stats_text_y = stats_text
        .as_ref()
        .map(|stats| stats_y + ((key_h as i32 - stats.height as i32) / 2))
        .unwrap_or(stats_y);
    let stats_text_h = stats_text
        .as_ref()
        .map(|stats| stats.height as i32)
        .unwrap_or(key_h as i32);
    let (_, logo_y) = logo.center_pos(w, h);
    let map_top = logo_y + logo.base_height as i32 + (h as f32 * 0.04).round() as i32;
    let map_bottom = stats_base_y - (h as f32 * 0.02).round() as i32;
    // Map text lives in the vertical gap between the pulsing logo and the fixed stats row.
    let line_gap = (h as f32 * 0.008).round() as i32;
    let title_artist = format!(
        "{} - {}",
        cfg.map_title.as_deref().unwrap_or("Unknown"),
        cfg.map_artist.as_deref().unwrap_or("")
    );
    // Font sizes come from the height, so a long title overflows a 9:16 canvas.
    let text_max_width = (w as f32 * 0.92).round() as u32;
    if let Some(title) = render_text_fitted(
        &title_artist,
        title_font,
        [255, 255, 255, 255],
        FontWeight::Bold,
        text_max_width,
    ) {
        let diff_line = format!(
            "[{}] mapped by {}",
            cfg.map_difficulty.as_deref().unwrap_or(""),
            cfg.map_creator.as_deref().unwrap_or("")
        );
        let diff = render_text_fitted(
            &diff_line,
            small_font,
            [200, 200, 200, 230],
            FontWeight::Normal,
            text_max_width,
        );
        let diff_block_h = diff
            .as_ref()
            .map(|value| line_gap + value.height as i32)
            .unwrap_or(0);
        let block_h = title.height as i32 + diff_block_h;
        let block_y = if map_bottom > map_top + block_h {
            map_top + (map_bottom - map_top - block_h) / 2
        } else {
            map_top
        };
        let tx = (w as i32 - title.width as i32) / 2;
        blit_image(&mut canvas, &title.image, tx, block_y, w, h);
        if let Some(diff) = diff {
            let dx = (w as i32 - diff.width as i32) / 2;
            let diff_y = centered_row_y_between(
                block_y,
                title.height as i32,
                stats_text_y,
                stats_text_h,
                diff.height as i32,
            );
            blit_image(&mut canvas, &diff.image, dx, diff_y, w, h);
        }
    }
    let spacing = 8i32;
    let total_w = key_w as i32 + spacing + stats_w as i32;
    let mut sx = (w as i32 - total_w) / 2;
    if let Some(key) = &key_badge {
        blit_image(&mut canvas, &key.image, sx, stats_y, w, h);
        sx += key.width as i32 + spacing;
    }
    if let Some(stats) = &stats_text {
        blit_image(&mut canvas, &stats.image, sx, stats_text_y, w, h);
    }
    canvas
}
fn centered_row_y_between(
    upper_y: i32,
    upper_h: i32,
    lower_y: i32,
    lower_h: i32,
    row_h: i32,
) -> i32 {
    let upper_center = upper_y as f32 + upper_h as f32 * 0.5;
    let lower_center = lower_y as f32 + lower_h as f32 * 0.5;
    ((upper_center + lower_center) * 0.5 - row_h as f32 * 0.5).round() as i32
}
fn blit_image(canvas: &mut [u8], img: &RgbaImage, x: i32, y: i32, canvas_w: u32, canvas_h: u32) {
    let (iw, ih) = img.dimensions();
    for iy in 0..ih {
        let dy = y + iy as i32;
        if dy < 0 || dy >= canvas_h as i32 {
            continue;
        }
        for ix in 0..iw {
            let dx = x + ix as i32;
            if dx < 0 || dx >= canvas_w as i32 {
                continue;
            }
            let src = img.get_pixel(ix, iy).0;
            let src_a = src[3] as f32 / 255.0;
            if src_a < 0.001 {
                continue;
            }
            let idx = ((dy as u32 * canvas_w + dx as u32) * 4) as usize;
            let dst_a = canvas[idx + 3] as f32 / 255.0;
            let out_a = src_a + dst_a * (1.0 - src_a);
            if out_a > 0.001 {
                let inv = 1.0 / out_a;
                canvas[idx] = ((src[0] as f32 * src_a + canvas[idx] as f32 * dst_a * (1.0 - src_a))
                    * inv) as u8;
                canvas[idx + 1] = ((src[1] as f32 * src_a
                    + canvas[idx + 1] as f32 * dst_a * (1.0 - src_a))
                    * inv) as u8;
                canvas[idx + 2] = ((src[2] as f32 * src_a
                    + canvas[idx + 2] as f32 * dst_a * (1.0 - src_a))
                    * inv) as u8;
                canvas[idx + 3] = (out_a * 255.0) as u8;
            }
        }
    }
}

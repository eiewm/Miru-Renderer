use super::animation::{results_animation_state, AnimatedElement};
use super::assets::{resolve_results_assets, ResultsAssets, SkinSprite};
use super::digits::{compose_digit_sprite, DigitSpriteKind};
use super::layout::{
    compute_results_layout, JudgmentRowLayout, LayoutPoint, LayoutRect, ResultsLayout,
};
use super::{ResultsScreenData, ResultsTimingSummary};
use crate::intro::{
    create_mod_badge_from_image_spec, create_mod_badge_from_spec, font_ubuntu_regular,
    measure_text, measure_text_with_font, render_text, render_text_simple,
    render_text_simple_with_font, render_text_with_font, FontWeight,
};
use crate::types::{ReplayOrigin, SkinAssets};
use crate::utils::image_proc::resize_exact;
use ab_glyph::{Font, PxScale, ScaleFont};
use image::{load_from_memory, Rgba, RgbaImage};
#[derive(Debug, Clone)]
struct JudgmentRowSprite {
    label_asset: Option<RgbaImage>,
    label_fallback: Option<RgbaImage>,
    count: Option<RgbaImage>,
    layout: JudgmentRowLayout,
}
#[derive(Debug, Clone, Default)]
struct TimingTextBlock {
    lines: [Option<RgbaImage>; 3],
    size: (u32, u32),
}
#[derive(Debug, Clone)]
struct BaselineTextImage {
    image: RgbaImage,
    baseline_offset: i32,
}
#[derive(Debug, Clone, Default)]
struct PreparedResultsAssets {
    panel: Option<RgbaImage>,
    title: Option<RgbaImage>,
    graph: Option<RgbaImage>,
    accuracy_label: Option<RgbaImage>,
    maxcombo_label: Option<RgbaImage>,
    perfect_ribbon: Option<RgbaImage>,
    grade: Option<RgbaImage>,
    judgment_labels: [Option<RgbaImage>; 6],
    mod_badges: Vec<RgbaImage>,
}
pub(crate) struct ResultsSceneRenderer {
    width: u32,
    layout: ResultsLayout,
    data: ResultsScreenData,
    base_background: RgbaImage,
    prepared_assets: PreparedResultsAssets,
    score_sprite: Option<RgbaImage>,
    accuracy_sprite: Option<RgbaImage>,
    combo_sprite: Option<RgbaImage>,
    combo_label_fallback: Option<RgbaImage>,
    accuracy_label_fallback: Option<RgbaImage>,
    perfect_fallback: Option<RgbaImage>,
    grade_fallback: Option<RgbaImage>,
    title_lines: [Option<BaselineTextImage>; 3],
    timing_block: TimingTextBlock,
    judgment_rows: Vec<JudgmentRowSprite>,
}
impl ResultsSceneRenderer {
    pub(crate) fn new(
        background_rgba: &[u8],
        width: u32,
        height: u32,
        skin: &SkinAssets,
        data: &ResultsScreenData,
    ) -> Self {
        let layout = compute_results_layout(width, height);
        let assets = resolve_results_assets(skin, data.grade);
        let mut prepared_assets = prepare_assets(&assets, &layout);
        prepared_assets.mod_badges = prepare_results_mod_badges(skin, data, &layout);
        let played_by_line = format_played_by_line(data);
        let title_asset_left = prepared_assets
            .title
            .as_ref()
            .map(|title| {
                width as i32
                    - layout.title_right_inset
                    - title.width() as i32
                    - scaled(layout.scale, 16.0)
            })
            .unwrap_or_else(|| width as i32 - scaled(layout.scale, 24.0));
        // Text must stop before the optional skin title sprite, which is anchored on the right.
        let title_max_width = (title_asset_left - layout.title_line_origins[0].x).max(1) as u32;
        let title_lines = [
            fit_baseline_text_image_with_optional_font(
                &format!("{} - {} [{}]", data.artist, data.title, data.difficulty),
                title_max_width,
                30.0 * layout.scale,
                24.0 * layout.scale,
                [0xFF, 0xFF, 0xFF, 0xFF],
                FontWeight::Normal,
                false,
                font_ubuntu_regular(),
            ),
            fit_baseline_text_image_with_optional_font(
                &format!("Beatmap by {}", data.creator),
                title_max_width,
                22.0 * layout.scale,
                18.0 * layout.scale,
                [0xFF, 0xFF, 0xFF, 0xFF],
                FontWeight::Normal,
                false,
                font_ubuntu_regular(),
            ),
            fit_baseline_text_image_with_optional_font(
                &played_by_line,
                title_max_width,
                22.0 * layout.scale,
                18.0 * layout.scale,
                [0xFF, 0xFF, 0xFF, 0xFF],
                FontWeight::Normal,
                false,
                font_ubuntu_regular(),
            ),
        ];
        let timing_block = build_timing_text_block(layout.scale, data.timing_summary);
        let score_sprite = compose_digit_sprite(
            skin,
            DigitSpriteKind::Score,
            &format_result_score(data.score),
            scaled_u32(layout.scale, 62.0),
            [0xFF, 0xFF, 0xFF, 0xFF],
        );
        let accuracy_sprite = compose_digit_sprite(
            skin,
            DigitSpriteKind::Score,
            &format!("{:.2}%", data.accuracy.clamp(0.0, 100.0)),
            scaled_u32(layout.scale, 46.0),
            [0xFF, 0xFF, 0xFF, 0xFF],
        );
        let combo_sprite = compose_digit_sprite(
            skin,
            DigitSpriteKind::Score,
            &format!("{}x", data.max_combo),
            scaled_u32(layout.scale, 46.0),
            [0xFF, 0xFF, 0xFF, 0xFF],
        );
        let combo_label_fallback = render_text_simple(
            "Max Combo",
            20.0 * layout.scale,
            [0xFF, 0xFF, 0xFF, 0xFF],
            FontWeight::Bold,
        )
        .map(|rendered| rendered.image);
        let accuracy_label_fallback = render_text_simple(
            "Accuracy",
            20.0 * layout.scale,
            [0xFF, 0xFF, 0xFF, 0xFF],
            FontWeight::Bold,
        )
        .map(|rendered| rendered.image);
        let perfect_fallback = render_text(
            "Perfect",
            22.0 * layout.scale,
            [0xFF, 0xFF, 0xFF, 0xFF],
            FontWeight::Bold,
        )
        .map(|rendered| rendered.image);
        let grade_fallback = render_text(
            data.grade.label(),
            118.0 * layout.scale,
            data.grade.fallback_color(),
            FontWeight::Bold,
        )
        .map(|rendered| rendered.image);
        let judgment_rows = build_judgment_rows(skin, &prepared_assets, &layout, data);
        Self {
            width,
            layout,
            data: data.clone(),
            base_background: background_canvas(background_rgba, width, height),
            prepared_assets,
            score_sprite,
            accuracy_sprite,
            combo_sprite,
            combo_label_fallback,
            accuracy_label_fallback,
            perfect_fallback,
            grade_fallback,
            title_lines,
            timing_block,
            judgment_rows,
        }
    }
    pub(crate) fn render_frame(&self, frame_index: u64, total_frames: u64) -> Vec<u8> {
        let progress = if total_frames <= 1 {
            1.0
        } else {
            frame_index as f32 / (total_frames - 1) as f32
        };
        self.render_with_progress(progress)
    }
    pub(crate) fn render_with_progress(&self, progress: f32) -> Vec<u8> {
        let state = results_animation_state(progress);
        let mut canvas = self.base_background.clone();
        self.draw_panel(&mut canvas, state.panel);
        self.draw_title(&mut canvas, state.title);
        self.draw_mod_badges(&mut canvas, state.title);
        self.draw_score(&mut canvas, state.score);
        self.draw_grade(&mut canvas, state.grade);
        for (row, anim) in self
            .judgment_rows
            .iter()
            .zip(state.judgments.iter().copied())
        {
            self.draw_judgment_row(&mut canvas, row, anim);
        }
        self.draw_combo(&mut canvas, state.combo);
        self.draw_accuracy(&mut canvas, state.accuracy);
        self.draw_graph(&mut canvas, state.graph_frame, state.graph_line_progress);
        self.draw_timing_summary(&mut canvas, state.timing);
        self.draw_perfect(&mut canvas, state.perfect);
        canvas.into_raw()
    }
    fn draw_title(&self, canvas: &mut RgbaImage, anim: AnimatedElement) {
        let bar_height = scaled_u32(self.layout.scale, 96.0);
        fill_rect(
            canvas,
            0,
            0,
            canvas.width(),
            bar_height,
            with_opacity([0x00, 0x00, 0x00, 0xCC], anim.alpha),
        );
        if let Some(title_asset) = &self.prepared_assets.title {
            let x = self.width as i32 - self.layout.title_right_inset - title_asset.width() as i32;
            let y = self.layout.title_top;
            alpha_blit(
                canvas,
                title_asset,
                x + anim.offset_x.round() as i32,
                y + anim.offset_y.round() as i32,
                anim.alpha,
            );
        }
        for (index, line) in self.title_lines.iter().enumerate() {
            let Some(line) = line else {
                continue;
            };
            let base_origin = self.layout.title_line_origins[index];
            let origin = LayoutPoint {
                x: base_origin.x + anim.offset_x.round() as i32,
                y: base_origin.y - line.baseline_offset + anim.offset_y.round() as i32,
            };
            alpha_blit(canvas, &line.image, origin.x, origin.y, anim.alpha);
        }
    }
    fn draw_panel(&self, canvas: &mut RgbaImage, anim: AnimatedElement) {
        let Some(panel) = &self.prepared_assets.panel else {
            return;
        };
        alpha_blit(
            canvas,
            panel,
            self.layout.panel_anchor.x + anim.offset_x.round() as i32,
            self.layout.panel_anchor.y + anim.offset_y.round() as i32,
            anim.alpha,
        );
    }
    fn draw_mod_badges(&self, canvas: &mut RgbaImage, anim: AnimatedElement) {
        if self.prepared_assets.mod_badges.is_empty() {
            return;
        }
        for (index, badge) in self.prepared_assets.mod_badges.iter().enumerate() {
            let center = LayoutPoint {
                x: self.layout.mod_badges_first_center.x
                    + self.layout.mod_badges_step_x * index as i32,
                y: self.layout.mod_badges_first_center.y,
            };
            let x = center.x - badge.width() as i32 / 2 + anim.offset_x.round() as i32;
            let y = center.y - badge.height() as i32 / 2 + anim.offset_y.round() as i32;
            alpha_blit(canvas, badge, x, y, anim.alpha);
        }
    }
    fn draw_score(&self, canvas: &mut RgbaImage, anim: AnimatedElement) {
        let Some(score_sprite) = &self.score_sprite else {
            return;
        };
        let origin = translated_point(self.layout.score_origin, anim);
        alpha_blit(canvas, score_sprite, origin.x, origin.y, anim.alpha);
    }
    fn draw_grade(&self, canvas: &mut RgbaImage, anim: AnimatedElement) {
        if let Some(grade_asset) = &self.prepared_assets.grade {
            draw_centered(canvas, grade_asset, self.layout.grade_center, anim);
        } else if let Some(grade_fallback) = &self.grade_fallback {
            draw_centered(canvas, grade_fallback, self.layout.grade_center, anim);
        }
    }
    fn draw_judgment_row(
        &self,
        canvas: &mut RgbaImage,
        row: &JudgmentRowSprite,
        anim: AnimatedElement,
    ) {
        if let Some(label) = &row.label_asset {
            draw_centered(canvas, label, row.layout.label_center, anim);
        } else if let Some(label) = &row.label_fallback {
            draw_centered(canvas, label, row.layout.label_center, anim);
        }
        if let Some(count) = &row.count {
            let count_origin = translated_point(row.layout.count_origin, anim);
            alpha_blit(canvas, count, count_origin.x, count_origin.y, anim.alpha);
        }
    }
    fn draw_graph(&self, canvas: &mut RgbaImage, anim: AnimatedElement, line_progress: f32) {
        let Some(graph_asset) = &self.prepared_assets.graph else {
            return;
        };
        let graph_x = self.layout.graph_rect.x + anim.offset_x.round() as i32;
        let graph_y = self.layout.graph_rect.y + anim.offset_y.round() as i32;
        alpha_blit(canvas, graph_asset, graph_x, graph_y, anim.alpha);
        let graph_rect = LayoutRect {
            x: self.layout.graph_rect.x + anim.offset_x.round() as i32,
            y: self.layout.graph_rect.y + anim.offset_y.round() as i32,
            w: self.layout.graph_rect.w,
            h: self.layout.graph_rect.h,
        };
        let inner = graph_inner_rect(graph_rect);
        draw_graph_line(
            canvas,
            inner,
            &self.data.graph_points,
            anim.alpha,
            line_progress,
        );
    }
    fn draw_timing_summary(&self, canvas: &mut RgbaImage, anim: AnimatedElement) {
        if self.timing_block.size.0 == 0 || self.timing_block.size.1 == 0 {
            return;
        }
        let border = scaled_u32(self.layout.scale, 1.0);
        let radius = scaled_u32(self.layout.scale, 8.0);
        let padding_x = scaled(self.layout.scale, 5.0);
        let padding_y = scaled(self.layout.scale, 6.0);
        let origin = translated_point(self.layout.timing_box_origin, anim);
        fill_rounded_rect(
            canvas,
            origin.x,
            origin.y,
            self.timing_block.size.0,
            self.timing_block.size.1,
            radius,
            with_opacity([0x00, 0x00, 0x00, 0xCC], anim.alpha),
        );
        draw_rounded_rect_outline(
            canvas,
            origin.x,
            origin.y,
            self.timing_block.size.0,
            self.timing_block.size.1,
            radius,
            border,
            with_opacity([0xFF, 0xFF, 0xFF, 0xD0], anim.alpha),
        );
        for (index, line) in self.timing_block.lines.iter().enumerate() {
            let Some(line) = line else {
                continue;
            };
            let line_y = origin.y + padding_y + index as i32 * self.layout.timing_line_gap;
            alpha_blit(canvas, line, origin.x + padding_x, line_y, anim.alpha);
        }
    }
    fn draw_accuracy(&self, canvas: &mut RgbaImage, anim: AnimatedElement) {
        if let Some(label) = &self.prepared_assets.accuracy_label {
            let origin = translated_point(self.layout.accuracy_label_anchor, anim);
            alpha_blit(canvas, label, origin.x, origin.y, anim.alpha);
        } else if let Some(label) = &self.accuracy_label_fallback {
            let origin = translated_point(self.layout.accuracy_label_anchor, anim);
            alpha_blit(canvas, label, origin.x, origin.y, anim.alpha);
        }
        if let Some(value) = &self.accuracy_sprite {
            let origin = translated_point(self.layout.accuracy_value_origin, anim);
            alpha_blit(canvas, value, origin.x, origin.y, anim.alpha);
        }
    }
    fn draw_combo(&self, canvas: &mut RgbaImage, anim: AnimatedElement) {
        if let Some(label) = &self.prepared_assets.maxcombo_label {
            let origin = translated_point(self.layout.combo_label_anchor, anim);
            alpha_blit(canvas, label, origin.x, origin.y, anim.alpha);
        } else if let Some(label) = &self.combo_label_fallback {
            let origin = translated_point(self.layout.combo_label_anchor, anim);
            alpha_blit(canvas, label, origin.x, origin.y, anim.alpha);
        }
        if let Some(value) = &self.combo_sprite {
            let origin = translated_point(self.layout.combo_value_origin, anim);
            alpha_blit(canvas, value, origin.x, origin.y, anim.alpha);
        }
    }
    fn draw_perfect(&self, canvas: &mut RgbaImage, anim: AnimatedElement) {
        if !self.data.perfect_combo {
            return;
        }
        if let Some(ribbon) = &self.prepared_assets.perfect_ribbon {
            draw_centered(canvas, ribbon, self.layout.perfect_center, anim);
        } else if let Some(label) = &self.perfect_fallback {
            draw_centered(canvas, label, self.layout.perfect_center, anim);
        }
    }
}
pub(crate) fn format_played_by_line(data: &ResultsScreenData) -> String {
    format!("Played by {}", data.player_name)
}
pub(crate) fn format_result_score(score: u32) -> String {
    format!("{:07}", score.min(99_999_999))
}
pub(crate) fn format_timing_summary_lines(summary: ResultsTimingSummary) -> [String; 3] {
    [
        "Accuracy:".to_string(),
        format!(
            "Error: {:.2}ms - {:.2}ms avg",
            summary.avg_early_ms, summary.avg_late_ms
        ),
        format!("Unstable Rate: {:.2}", summary.unstable_rate),
    ]
}
fn build_timing_text_block(scale: f32, summary: ResultsTimingSummary) -> TimingTextBlock {
    let Some(font) = font_ubuntu_regular() else {
        return TimingTextBlock::default();
    };
    let font_size = 12.0 * scale;
    let padding_x = scaled_u32(scale, 5.0);
    let padding_y = scaled_u32(scale, 6.0);
    let line_gap = scaled_u32(scale, 14.0);
    let lines = format_timing_summary_lines(summary).map(|line| {
        render_text_simple_with_font(&line, font_size, [0xFF, 0xFF, 0xFF, 0xFF], font)
            .map(|rendered| rendered.image)
    });
    let width = lines
        .iter()
        .flatten()
        .map(RgbaImage::width)
        .max()
        .unwrap_or(0);
    let height = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            line.as_ref()
                .map(|image| index as u32 * line_gap + image.height())
        })
        .max()
        .unwrap_or(0);
    TimingTextBlock {
        lines,
        size: (
            width.saturating_add(padding_x * 2),
            height.saturating_add(padding_y * 2),
        ),
    }
}
fn build_judgment_rows(
    skin: &SkinAssets,
    prepared_assets: &PreparedResultsAssets,
    layout: &ResultsLayout,
    data: &ResultsScreenData,
) -> Vec<JudgmentRowSprite> {
    let rows = [
        (
            "300",
            data.statistics.hit300,
            prepared_assets.judgment_labels[1].clone(),
            layout.judgment_rows[0],
        ),
        (
            "200",
            data.statistics.hit200,
            prepared_assets.judgment_labels[2].clone(),
            layout.judgment_rows[1],
        ),
        (
            "50",
            data.statistics.hit50,
            prepared_assets.judgment_labels[4].clone(),
            layout.judgment_rows[2],
        ),
        (
            "MAX",
            data.statistics.max,
            prepared_assets.judgment_labels[0].clone(),
            layout.judgment_rows[3],
        ),
        (
            "100",
            data.statistics.hit100,
            prepared_assets.judgment_labels[3].clone(),
            layout.judgment_rows[4],
        ),
        (
            "MISS",
            data.statistics.miss,
            prepared_assets.judgment_labels[5].clone(),
            layout.judgment_rows[5],
        ),
    ];
    rows.into_iter()
        .map(
            |(label, value, label_asset, row_layout)| JudgmentRowSprite {
                label_fallback: fit_text_image(
                    label,
                    row_layout.max_icon_size.0,
                    24.0 * layout.scale,
                    14.0 * layout.scale,
                    [0xFF, 0xFF, 0xFF, 0xFF],
                    FontWeight::Bold,
                    false,
                ),
                label_asset,
                count: compose_digit_sprite(
                    skin,
                    DigitSpriteKind::Count,
                    &format!("{}x", value),
                    scaled_u32(layout.scale, 44.0),
                    [0xFF, 0xFF, 0xFF, 0xFF],
                ),
                layout: row_layout,
            },
        )
        .collect()
}
fn prepare_assets(assets: &ResultsAssets, layout: &ResultsLayout) -> PreparedResultsAssets {
    PreparedResultsAssets {
        panel: assets
            .panel
            .as_ref()
            .map(|sprite| scale_panel_sprite(sprite, layout)),
        title: assets
            .title
            .as_ref()
            .map(|sprite| scale_reference_sprite(sprite, layout.scale)),
        graph: assets
            .graph
            .as_ref()
            .map(|sprite| scale_reference_sprite(sprite, layout.scale)),
        accuracy_label: assets
            .accuracy_label
            .as_ref()
            .map(|sprite| scale_reference_sprite(sprite, layout.scale)),
        maxcombo_label: assets
            .maxcombo_label
            .as_ref()
            .map(|sprite| scale_reference_sprite(sprite, layout.scale)),
        perfect_ribbon: assets
            .perfect_ribbon
            .as_ref()
            .map(|sprite| scale_reference_sprite(sprite, layout.scale)),
        grade: assets
            .grade
            .as_ref()
            .map(|sprite| scale_reference_sprite(sprite, layout.scale)),
        judgment_labels: [
            assets.judgments.max.as_ref().map(|sprite| {
                contain_reference_sprite(
                    sprite,
                    layout.scale,
                    layout.judgment_rows[0].max_icon_size.0,
                    layout.judgment_rows[0].max_icon_size.1,
                )
            }),
            assets.judgments.hit300.as_ref().map(|sprite| {
                contain_reference_sprite(
                    sprite,
                    layout.scale,
                    layout.judgment_rows[1].max_icon_size.0,
                    layout.judgment_rows[1].max_icon_size.1,
                )
            }),
            assets.judgments.hit200.as_ref().map(|sprite| {
                contain_reference_sprite(
                    sprite,
                    layout.scale,
                    layout.judgment_rows[2].max_icon_size.0,
                    layout.judgment_rows[2].max_icon_size.1,
                )
            }),
            assets.judgments.hit100.as_ref().map(|sprite| {
                contain_reference_sprite(
                    sprite,
                    layout.scale,
                    layout.judgment_rows[3].max_icon_size.0,
                    layout.judgment_rows[3].max_icon_size.1,
                )
            }),
            assets.judgments.hit50.as_ref().map(|sprite| {
                contain_reference_sprite(
                    sprite,
                    layout.scale,
                    layout.judgment_rows[4].max_icon_size.0,
                    layout.judgment_rows[4].max_icon_size.1,
                )
            }),
            assets.judgments.miss.as_ref().map(|sprite| {
                contain_reference_sprite(
                    sprite,
                    layout.scale,
                    layout.judgment_rows[5].max_icon_size.0,
                    layout.judgment_rows[5].max_icon_size.1,
                )
            }),
        ],
        mod_badges: Vec::new(),
    }
}
fn prepare_results_mod_badges(
    skin: &SkinAssets,
    data: &ResultsScreenData,
    layout: &ResultsLayout,
) -> Vec<RgbaImage> {
    let badge_height = scaled_u32(layout.scale, 72.0);
    data.mod_badges
        .iter()
        .filter_map(|spec| match data.mod_origin {
            // Stable skins often ship selection-mod-* images; lazer exports use generated badges.
            ReplayOrigin::StableLegacy => create_stable_results_mod_badge(skin, spec, badge_height)
                .or_else(|| {
                    create_mod_badge_from_spec(spec, badge_height).map(|badge| badge.image)
                }),
            ReplayOrigin::LazerExport => {
                create_mod_badge_from_spec(spec, badge_height).map(|badge| badge.image)
            }
        })
        .collect()
}
fn create_stable_results_mod_badge(
    skin: &SkinAssets,
    spec: &crate::intro::IntroModBadgeSpec,
    badge_height: u32,
) -> Option<RgbaImage> {
    let stem = stable_results_mod_texture_stem(&spec.acronym)?;
    for candidate in stable_results_mod_texture_candidates(stem) {
        let Some(data) = find_skin_image_for_replay_mod(skin, &candidate) else {
            continue;
        };
        let source = load_from_memory(data).ok()?.into_rgba8();
        return create_mod_badge_from_image_spec(spec, badge_height, source)
            .map(|badge| badge.image);
    }
    None
}
fn stable_results_mod_texture_candidates(stem: &str) -> [String; 4] {
    [
        format!("{stem}@2x.png"),
        format!("{stem}.png"),
        format!("{stem}@2x"),
        stem.to_string(),
    ]
}
fn find_skin_image_for_replay_mod<'a>(
    skin: &'a SkinAssets,
    requested_name: &str,
) -> Option<&'a [u8]> {
    if let Some(data) = skin.find_image(requested_name) {
        return Some(data);
    }
    // Some skins store selection images in nested folders, so basename lookup preserves compatibility.
    let requested_name = SkinAssets::normalize_key(requested_name);
    let basename = requested_name.rsplit('/').next()?;
    skin.images.iter().find_map(|(key, data)| {
        key.rsplit('/')
            .next()
            .filter(|candidate| *candidate == basename)
            .map(|_| data.as_slice())
    })
}
fn stable_results_mod_texture_stem(acronym: &str) -> Option<&'static str> {
    match acronym.trim().to_ascii_uppercase().as_str() {
        "NF" => Some("selection-mod-nofail"),
        "EZ" => Some("selection-mod-easy"),
        "HD" => Some("selection-mod-hidden"),
        "HR" => Some("selection-mod-hardrock"),
        "SD" => Some("selection-mod-suddendeath"),
        "PF" => Some("selection-mod-perfect"),
        "DT" => Some("selection-mod-doubletime"),
        "NC" => Some("selection-mod-nightcore"),
        "HT" => Some("selection-mod-halftime"),
        "FL" => Some("selection-mod-flashlight"),
        "FI" => Some("selection-mod-fadein"),
        "MR" => Some("selection-mod-mirror"),
        "V2" => Some("selection-mod-scorev2"),
        "AT" => Some("selection-mod-autoplay"),
        "CO" => Some("selection-mod-keycoop"),
        _ => None,
    }
}
fn scale_panel_sprite(sprite: &SkinSprite, layout: &ResultsLayout) -> RgbaImage {
    scale_reference_sprite(sprite, layout.scale)
}
fn scale_reference_sprite(sprite: &SkinSprite, scale: f32) -> RgbaImage {
    let (width, height) = reference_dimensions(sprite, scale);
    resize_exact(&sprite.image, width.max(1), height.max(1))
}
fn contain_reference_sprite(
    sprite: &SkinSprite,
    scale: f32,
    max_width: u32,
    max_height: u32,
) -> RgbaImage {
    let (width, height) = reference_dimensions(sprite, scale);
    if width <= max_width && height <= max_height {
        return resize_exact(&sprite.image, width.max(1), height.max(1));
    }
    let fit = (max_width.max(1) as f32 / width.max(1) as f32)
        .min(max_height.max(1) as f32 / height.max(1) as f32);
    let width = (width as f32 * fit).round().max(1.0) as u32;
    let height = (height as f32 * fit).round().max(1.0) as u32;
    resize_exact(&sprite.image, width, height)
}
fn reference_dimensions(sprite: &SkinSprite, scale: f32) -> (u32, u32) {
    let width = ((sprite.image.width() as f32 / sprite.scale_factor) * scale)
        .round()
        .max(1.0) as u32;
    let height = ((sprite.image.height() as f32 / sprite.scale_factor) * scale)
        .round()
        .max(1.0) as u32;
    (width, height)
}
fn fit_text_image(
    text: &str,
    max_width: u32,
    start_size: f32,
    min_size: f32,
    color: [u8; 4],
    weight: FontWeight,
    shadow: bool,
) -> Option<RgbaImage> {
    fit_text_image_with_optional_font(
        text, max_width, start_size, min_size, color, weight, shadow, None,
    )
}
fn fit_text_image_with_optional_font(
    text: &str,
    max_width: u32,
    start_size: f32,
    min_size: f32,
    color: [u8; 4],
    weight: FontWeight,
    shadow: bool,
    font: Option<&ab_glyph::FontArc>,
) -> Option<RgbaImage> {
    if text.is_empty() {
        return None;
    }
    let max_width = max_width.max(1) as f32;
    let mut font_size = start_size.max(min_size).max(1.0);
    // Results titles are single-line images; shrink the font instead of wrapping over skin art.
    while font_size > min_size.max(1.0)
        && match font {
            Some(font) => measure_text_with_font(text, font_size, font),
            None => measure_text(text, font_size, weight),
        } > max_width
    {
        font_size -= 1.0;
    }
    let rendered = match (font, shadow) {
        (Some(font), true) => render_text_with_font(text, font_size, color, font),
        (Some(font), false) => {
            crate::intro::render_text_simple_with_font(text, font_size, color, font)
        }
        (None, true) => render_text(text, font_size, color, weight),
        (None, false) => render_text_simple(text, font_size, color, weight),
    }?;
    Some(rendered.image)
}
fn fit_baseline_text_image_with_optional_font(
    text: &str,
    max_width: u32,
    start_size: f32,
    min_size: f32,
    color: [u8; 4],
    weight: FontWeight,
    shadow: bool,
    font: Option<&ab_glyph::FontArc>,
) -> Option<BaselineTextImage> {
    if text.is_empty() {
        return None;
    }
    let max_width = max_width.max(1) as f32;
    let mut font_size = start_size.max(min_size).max(1.0);
    while font_size > min_size.max(1.0)
        && match font {
            Some(font) => measure_text_with_font(text, font_size, font),
            None => measure_text(text, font_size, weight),
        } > max_width
    {
        font_size -= 1.0;
    }
    let rendered = match (font, shadow) {
        (Some(font), true) => render_text_with_font(text, font_size, color, font),
        (Some(font), false) => render_text_simple_with_font(text, font_size, color, font),
        (None, true) => render_text(text, font_size, color, weight),
        (None, false) => render_text_simple(text, font_size, color, weight),
    }?;
    let baseline_offset = font
        .map(|font| text_baseline_offset(font, font_size))
        .unwrap_or_else(|| (font_size * 0.93).round() as i32);
    Some(BaselineTextImage {
        image: rendered.image,
        baseline_offset,
    })
}
fn text_baseline_offset(font: &ab_glyph::FontArc, font_size: f32) -> i32 {
    font.as_scaled(PxScale::from(font_size))
        .ascent()
        .round()
        .max(0.0) as i32
}
fn draw_centered(
    canvas: &mut RgbaImage,
    sprite: &RgbaImage,
    center: LayoutPoint,
    anim: AnimatedElement,
) {
    let x = center.x - sprite.width() as i32 / 2 + anim.offset_x.round() as i32;
    let y = center.y - sprite.height() as i32 / 2 + anim.offset_y.round() as i32;
    alpha_blit(canvas, sprite, x, y, anim.alpha);
}
fn translated_point(point: LayoutPoint, anim: AnimatedElement) -> LayoutPoint {
    LayoutPoint {
        x: point.x + anim.offset_x.round() as i32,
        y: point.y + anim.offset_y.round() as i32,
    }
}
fn draw_graph_line(
    canvas: &mut RgbaImage,
    rect: LayoutRect,
    points: &[super::ResultsGraphPoint],
    opacity: f32,
    line_progress: f32,
) {
    if points.len() < 2 || opacity <= 0.0 || rect.w <= 1 || rect.h <= 1 {
        return;
    }
    let max_segments = points.len() - 1;
    let visible_segments = ((max_segments as f32) * line_progress.clamp(0.0, 1.0)).ceil() as usize;
    let mut last_point = graph_point(rect, points[0]);
    for index in 1..=visible_segments.min(max_segments) {
        let next_point = graph_point(rect, points[index]);
        let mean_hp = (points[index - 1].life + points[index].life) * 0.5;
        let line_color = if mean_hp > 0.5 {
            with_opacity([0x33, 0xFF, 0x33, 0xFF], opacity)
        } else {
            with_opacity([0xFF, 0x33, 0x33, 0xFF], opacity)
        };
        draw_graph_segment(canvas, rect, last_point, next_point, line_color);
        last_point = next_point;
    }
}
pub(crate) fn graph_inner_rect(rect: LayoutRect) -> LayoutRect {
    // The life graph image has transparent padding; these ratios target the actual plot area.
    const GRAPH_REF_W: f32 = 308.0;
    const GRAPH_REF_H: f32 = 156.0;
    const GRAPH_INSET_LEFT: f32 = 8.0;
    const GRAPH_INSET_TOP: f32 = 8.0;
    const GRAPH_INNER_W: f32 = 298.0;
    const GRAPH_INNER_H: f32 = 137.6;
    let x = rect.x + ((rect.w as f32) * (GRAPH_INSET_LEFT / GRAPH_REF_W)).round() as i32;
    let y = rect.y + ((rect.h as f32) * (GRAPH_INSET_TOP / GRAPH_REF_H)).round() as i32;
    let max_width = rect.w.saturating_sub((x - rect.x).max(0) as u32);
    let max_height = rect.h.saturating_sub((y - rect.y).max(0) as u32);
    LayoutRect {
        x,
        y,
        w: (((rect.w as f32) * (GRAPH_INNER_W / GRAPH_REF_W))
            .round()
            .max(1.0) as u32)
            .min(max_width.max(1)),
        h: (((rect.h as f32) * (GRAPH_INNER_H / GRAPH_REF_H))
            .round()
            .max(1.0) as u32)
            .min(max_height.max(1)),
    }
}
fn graph_point(rect: LayoutRect, point: super::ResultsGraphPoint) -> (f32, f32) {
    let x = rect.x as f32 + point.progress.clamp(0.0, 1.0) * rect.w.saturating_sub(1) as f32;
    // Square-root scaling gives low-health changes more visible vertical space.
    let life = point.life.clamp(0.0, 1.0).sqrt();
    let y = rect.y as f32 + (1.0 - life) * rect.h.saturating_sub(1) as f32;
    (x, y)
}
fn draw_graph_segment(
    canvas: &mut RgbaImage,
    rect: LayoutRect,
    start: (f32, f32),
    end: (f32, f32),
    color: Rgba<u8>,
) {
    // Draw two adjacent scanlines to keep the graph readable after video encoding.
    draw_graph_segment_offset(canvas, rect, start, end, color, 0.0);
    draw_graph_segment_offset(canvas, rect, start, end, color, 1.0);
}
fn draw_graph_segment_offset(
    canvas: &mut RgbaImage,
    rect: LayoutRect,
    start: (f32, f32),
    end: (f32, f32),
    color: Rgba<u8>,
    y_offset: f32,
) {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let steps = dx.abs().max(dy.abs()).ceil().max(1.0) as usize;
    for step in 0..=steps {
        let t = step as f32 / steps.max(1) as f32;
        let x = (start.0 + dx * t).round() as i32;
        let y = (start.1 + dy * t + y_offset).round() as i32;
        if x < rect.x
            || y < rect.y
            || x >= rect.x + rect.w as i32
            || y >= rect.y + rect.h as i32
            || !(0..canvas.width() as i32).contains(&x)
            || !(0..canvas.height() as i32).contains(&y)
        {
            continue;
        }
        let dst = canvas.get_pixel_mut(x as u32, y as u32);
        *dst = blend_pixel(*dst, color, 1.0);
    }
}
fn background_canvas(raw: &[u8], width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_raw(width.max(1), height.max(1), raw.to_vec())
        .unwrap_or_else(|| RgbaImage::from_pixel(width.max(1), height.max(1), Rgba([0, 0, 0, 0])))
}
fn alpha_blit(canvas: &mut RgbaImage, sprite: &RgbaImage, x: i32, y: i32, opacity: f32) {
    let opacity = opacity.clamp(0.0, 1.0);
    if opacity <= 0.0 {
        return;
    }
    for sy in 0..sprite.height() {
        let dy = y + sy as i32;
        if !(0..canvas.height() as i32).contains(&dy) {
            continue;
        }
        for sx in 0..sprite.width() {
            let dx = x + sx as i32;
            if !(0..canvas.width() as i32).contains(&dx) {
                continue;
            }
            let src = *sprite.get_pixel(sx, sy);
            if src[3] == 0 {
                continue;
            }
            let dst = canvas.get_pixel_mut(dx as u32, dy as u32);
            *dst = blend_pixel(*dst, src, opacity);
        }
    }
}
fn fill_rect(canvas: &mut RgbaImage, x: i32, y: i32, width: u32, height: u32, color: Rgba<u8>) {
    for dy in 0..height {
        let py = y + dy as i32;
        if !(0..canvas.height() as i32).contains(&py) {
            continue;
        }
        for dx in 0..width {
            let px = x + dx as i32;
            if !(0..canvas.width() as i32).contains(&px) {
                continue;
            }
            let dst = canvas.get_pixel_mut(px as u32, py as u32);
            *dst = blend_pixel(*dst, color, 1.0);
        }
    }
}
fn fill_rounded_rect(
    canvas: &mut RgbaImage,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    radius: u32,
    color: Rgba<u8>,
) {
    if width == 0 || height == 0 {
        return;
    }
    for dy in 0..height {
        let py = y + dy as i32;
        if !(0..canvas.height() as i32).contains(&py) {
            continue;
        }
        for dx in 0..width {
            if !rounded_rect_contains(dx, dy, width, height, radius) {
                continue;
            }
            let px = x + dx as i32;
            if !(0..canvas.width() as i32).contains(&px) {
                continue;
            }
            let dst = canvas.get_pixel_mut(px as u32, py as u32);
            *dst = blend_pixel(*dst, color, 1.0);
        }
    }
}
fn draw_rounded_rect_outline(
    canvas: &mut RgbaImage,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    radius: u32,
    thickness: u32,
    color: Rgba<u8>,
) {
    if width == 0 || height == 0 || thickness == 0 {
        return;
    }
    let inner_width = width.saturating_sub(thickness.saturating_mul(2));
    let inner_height = height.saturating_sub(thickness.saturating_mul(2));
    let inner_radius = radius.saturating_sub(thickness);
    for dy in 0..height {
        let py = y + dy as i32;
        if !(0..canvas.height() as i32).contains(&py) {
            continue;
        }
        for dx in 0..width {
            if !rounded_rect_contains(dx, dy, width, height, radius) {
                continue;
            }
            let inside_inner = if inner_width > 0
                && inner_height > 0
                && dx >= thickness
                && dx < width.saturating_sub(thickness)
                && dy >= thickness
                && dy < height.saturating_sub(thickness)
            {
                rounded_rect_contains(
                    dx - thickness,
                    dy - thickness,
                    inner_width,
                    inner_height,
                    inner_radius,
                )
            } else {
                false
            };
            if inside_inner {
                continue;
            }
            let px = x + dx as i32;
            if !(0..canvas.width() as i32).contains(&px) {
                continue;
            }
            let dst = canvas.get_pixel_mut(px as u32, py as u32);
            *dst = blend_pixel(*dst, color, 1.0);
        }
    }
}
fn rounded_rect_contains(px: u32, py: u32, width: u32, height: u32, radius: u32) -> bool {
    if width == 0 || height == 0 {
        return false;
    }
    let radius = radius.min(width / 2).min(height / 2);
    if radius == 0 {
        return true;
    }
    if px >= radius && px < width.saturating_sub(radius) {
        return true;
    }
    if py >= radius && py < height.saturating_sub(radius) {
        return true;
    }
    let px = px as f32 + 0.5;
    let py = py as f32 + 0.5;
    let radius_f = radius as f32;
    let left = radius_f - 0.5;
    let right = width as f32 - radius_f - 0.5;
    let top = radius_f - 0.5;
    let bottom = height as f32 - radius_f - 0.5;
    let center_x = if px < radius_f { left } else { right };
    let center_y = if py < radius_f { top } else { bottom };
    let dx = px - center_x;
    let dy = py - center_y;
    dx * dx + dy * dy <= radius_f * radius_f
}
fn blend_pixel(dst: Rgba<u8>, src: Rgba<u8>, opacity: f32) -> Rgba<u8> {
    let src_alpha = (src[3] as f32 / 255.0) * opacity.clamp(0.0, 1.0);
    if src_alpha <= 0.0 {
        return dst;
    }
    // All result sprites use straight alpha, including skin PNGs and generated text.
    let dst_alpha = dst[3] as f32 / 255.0;
    let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);
    if out_alpha <= 0.0 {
        return Rgba([0, 0, 0, 0]);
    }
    let src_weight = src_alpha / out_alpha;
    let dst_weight = (dst_alpha * (1.0 - src_alpha)) / out_alpha;
    Rgba([
        (src[0] as f32 * src_weight + dst[0] as f32 * dst_weight).round() as u8,
        (src[1] as f32 * src_weight + dst[1] as f32 * dst_weight).round() as u8,
        (src[2] as f32 * src_weight + dst[2] as f32 * dst_weight).round() as u8,
        (out_alpha * 255.0).round() as u8,
    ])
}
fn with_opacity(color: [u8; 4], opacity: f32) -> Rgba<u8> {
    let alpha = ((color[3] as f32) * opacity.clamp(0.0, 1.0)).round() as u8;
    Rgba([color[0], color[1], color[2], alpha])
}
fn scaled(scale: f32, value: f32) -> i32 {
    (value * scale).round() as i32
}
fn scaled_u32(scale: f32, value: f32) -> u32 {
    (value * scale).round().max(1.0) as u32
}

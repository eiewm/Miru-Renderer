use super::ResultsGrade;
use crate::types::SkinAssets;
use crate::utils::image_proc::load_rgba;
use image::RgbaImage;
#[derive(Debug, Clone)]
pub(crate) struct SkinSprite {
    pub(crate) image: RgbaImage,
    pub(crate) scale_factor: f32,
}
#[derive(Debug, Clone, Default)]
pub(crate) struct JudgmentSpriteSet {
    pub(crate) max: Option<SkinSprite>,
    pub(crate) hit300: Option<SkinSprite>,
    pub(crate) hit200: Option<SkinSprite>,
    pub(crate) hit100: Option<SkinSprite>,
    pub(crate) hit50: Option<SkinSprite>,
    pub(crate) miss: Option<SkinSprite>,
}
#[derive(Debug, Clone, Default)]
pub(crate) struct ResultsAssets {
    pub(crate) panel: Option<SkinSprite>,
    pub(crate) title: Option<SkinSprite>,
    pub(crate) graph: Option<SkinSprite>,
    pub(crate) accuracy_label: Option<SkinSprite>,
    pub(crate) maxcombo_label: Option<SkinSprite>,
    pub(crate) perfect_ribbon: Option<SkinSprite>,
    pub(crate) grade: Option<SkinSprite>,
    pub(crate) judgments: JudgmentSpriteSet,
}
fn normalize_asset_stem(name: &str) -> String {
    // Skin configs may include extensions or @2x suffixes; lookup works on normalized stems.
    name.trim()
        .trim_end_matches(".png")
        .trim_end_matches(".jpg")
        .trim_end_matches(".jpeg")
        .trim_end_matches("@2x")
        .to_ascii_lowercase()
}
fn png_candidates(bases: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(bases.len() * 2);
    for base in bases {
        // Prefer high-resolution skin assets but fall back to the standard PNG name.
        out.push(format!("{base}@2x.png"));
        out.push(format!("{base}.png"));
    }
    out
}
fn load_first_image(skin: &SkinAssets, names: &[String]) -> Option<SkinSprite> {
    let refs: Vec<_> = names.iter().map(String::as_str).collect();
    let (name, data) = skin.find_first(&refs)?;
    let image = load_rgba(data)?;
    // @2x images draw at half their pixel dimensions in osu! skin coordinates.
    let scale_factor = if name.contains("@2x") { 2.0 } else { 1.0 };
    Some(SkinSprite {
        image,
        scale_factor,
    })
}
fn resolve_judgment_image(
    skin: &SkinAssets,
    custom_name: Option<&str>,
    default_base: &str,
) -> Option<SkinSprite> {
    let mut bases = Vec::new();
    if let Some(custom_name) = custom_name {
        let custom = normalize_asset_stem(custom_name);
        if !custom.is_empty() {
            bases.push(custom);
        }
    }
    // Custom judgment names override the default, but missing assets still fall back to mania defaults.
    if !bases.iter().any(|candidate| candidate == default_base) {
        bases.push(default_base.to_string());
    }
    load_first_image(skin, &png_candidates(&bases))
}
pub(crate) fn resolve_results_panel_image(skin: &SkinAssets) -> Option<SkinSprite> {
    let candidates = png_candidates(&["ranking-panel".to_string()]);
    load_first_image(skin, &candidates)
}
pub(crate) fn resolve_results_rank_image(
    skin: &SkinAssets,
    grade: ResultsGrade,
) -> Option<SkinSprite> {
    let suffix = grade.skin_suffix().to_ascii_lowercase();
    load_first_image(skin, &png_candidates(&[format!("ranking-{suffix}")]))
}
pub(crate) fn resolve_results_assets(skin: &SkinAssets, grade: ResultsGrade) -> ResultsAssets {
    ResultsAssets {
        panel: resolve_results_panel_image(skin),
        title: load_first_image(
            skin,
            &png_candidates(&[
                "ranking-title-mania".to_string(),
                "ranking-title".to_string(),
            ]),
        ),
        graph: load_first_image(
            skin,
            &png_candidates(&[
                "ranking-graph-mania".to_string(),
                "ranking-graph".to_string(),
            ]),
        ),
        accuracy_label: load_first_image(
            skin,
            &png_candidates(&[
                "ranking-accuracy-mania".to_string(),
                "ranking-accuracy".to_string(),
            ]),
        ),
        maxcombo_label: load_first_image(
            skin,
            &png_candidates(&[
                "ranking-maxcombo-mania".to_string(),
                "ranking-maxcombo".to_string(),
            ]),
        ),
        perfect_ribbon: load_first_image(
            skin,
            &png_candidates(&[
                "ranking-perfect-mania".to_string(),
                "ranking-perfect".to_string(),
            ]),
        ),
        grade: resolve_results_rank_image(skin, grade),
        judgments: JudgmentSpriteSet {
            max: resolve_judgment_image(skin, skin.config.hit_300g.as_deref(), "mania-hit300g"),
            hit300: resolve_judgment_image(skin, skin.config.hit_300.as_deref(), "mania-hit300"),
            hit200: resolve_judgment_image(skin, skin.config.hit_200.as_deref(), "mania-hit200"),
            hit100: resolve_judgment_image(skin, skin.config.hit_100.as_deref(), "mania-hit100"),
            hit50: resolve_judgment_image(skin, skin.config.hit_50.as_deref(), "mania-hit50"),
            miss: resolve_judgment_image(skin, skin.config.hit_0.as_deref(), "mania-hit0"),
        },
    }
}

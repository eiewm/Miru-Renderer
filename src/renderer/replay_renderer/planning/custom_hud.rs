use super::super::render::{HudAssetFrame, HudJudgmentCounterAnimation, ReplayRenderer};
use super::super::sprites::{z_order, SpriteCommand, SpritePlanner};
use crate::hud::{
    HudAssetRefConfig, HudElementConfig, HudFontRefConfig, HudLayerConfig, HudLayerTransformConfig,
};
use crate::intro::{font_bold, font_regular, FontWeight, RenderedText};
use crate::renderer::gpu::SpriteBlendMode;
use crate::renderer::replay_renderer::state::{HitErrorWindows, HudFrameState};
use crate::types::JudgmentKind;
use ab_glyph::{point, Font, FontArc, GlyphId, PxScale, ScaleFont};
use image::{AnimationDecoder, ImageDecoder, Rgba, RgbaImage};
use serde_json::{json, Value};
use std::collections::hash_map::DefaultHasher;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{BufReader, Cursor};
use std::path::{Path, PathBuf};
const JUDGMENT_LABELS: [&str; 6] = ["MAX", "300", "200", "100", "50", "MISS"];
const JUDGMENT_KEYS: [&str; 6] = ["max", "hit300", "hit200", "hit100", "hit50", "miss"];
const KEY_COUNTER_TAIL_SPEED_MULTIPLIER: f32 = 1.45;
const HUD_TEXT_SUPERSAMPLE: f32 = 2.0;
const HUD_TEXT_VISUAL_SCALE: f32 = 1.24;
const HUD_TEXT_FALLBACK_FAMILIES: &[&str] = &["Noto Sans JP", "Noto Sans"];
// Custom HUD assets are user-supplied, so every decode path enforces file, dimension, and frame caps.
const HUD_ASSET_MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
const HUD_ASSET_MAX_DIMENSION: u32 = 4096;
const HUD_ASSET_MAX_PIXELS: u64 = 8_294_400;
const HUD_GIF_MAX_FRAMES: usize = 120;
const HUD_GIF_MAX_TOTAL_PIXELS: u64 = 60_000_000;
const JUDGMENT_COLORS: [[f32; 4]; 6] = [
    [0.56, 0.87, 1.0, 1.0],
    [0.98, 0.64, 0.01, 1.0],
    [0.78, 0.82, 0.88, 1.0],
    [0.99, 0.41, 0.71, 1.0],
    [0.47, 0.82, 0.29, 1.0],
    [0.89, 0.33, 0.40, 1.0],
];
fn value_string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}
fn value_f32(value: &Value, key: &str, fallback: f32) -> f32 {
    value
        .get(key)
        .and_then(Value::as_f64)
        .map(|value| value as f32)
        .filter(|value| value.is_finite())
        .unwrap_or(fallback)
}
fn number_value(value: &Value) -> Option<f32> {
    value
        .as_f64()
        .map(|value| value as f32)
        .filter(|value| value.is_finite())
}
fn props_bool(props: &Value, key: &str, fallback: bool) -> bool {
    props.get(key).and_then(Value::as_bool).unwrap_or(fallback)
}
fn sparkline_ratio(value: f32, max: f32, props: &Value) -> f32 {
    let raw = (value / max.max(1.0)).max(0.0);
    if !props_bool(props, "softCeiling", true) {
        return raw.clamp(0.0, 1.0);
    }
    // Soft ceiling compresses KPS spikes without flattening the lower range of the graph.
    let soft_start = value_f32(props, "softCeilingStart", 0.9).clamp(0.5, 0.98);
    let ceiling = value_f32(props, "ceilingRatio", 0.96).clamp(soft_start + 0.01, 0.995);
    let strength = value_f32(props, "softCeilingStrength", 1.35).clamp(0.1, 5.0);
    if raw <= soft_start {
        return raw;
    }
    let compressed =
        soft_start + (1.0 - (-(raw - soft_start) * strength).exp()) * (ceiling - soft_start);
    compressed.min(ceiling)
}
fn spin_rotation_degrees(layer: &HudLayerConfig, timestamp_ms: i32) -> f32 {
    if !props_bool(&layer.props, "spinEnabled", false) {
        return 0.0;
    }
    let speed = value_f32(&layer.props, "spinSpeed", 90.0);
    (timestamp_ms.max(0) as f32 / 1000.0) * speed
}
fn props_key_value<'a>(props: &'a Value, key_index: usize, key: &str) -> Option<&'a Value> {
    // Per-key overrides shadow shared props so one key can differ without duplicating the component.
    props
        .get("perKeyOverrides")
        .and_then(Value::as_object)
        .and_then(|overrides| overrides.get(&key_index.to_string()))
        .and_then(|override_props| override_props.get(key))
        .or_else(|| props.get(key))
}
fn props_key_bool(props: &Value, key_index: usize, key: &str, fallback: bool) -> bool {
    props_key_value(props, key_index, key)
        .and_then(Value::as_bool)
        .unwrap_or(fallback)
}
fn props_key_f32(props: &Value, key_index: usize, key: &str, fallback: f32) -> f32 {
    props_key_value(props, key_index, key)
        .and_then(Value::as_f64)
        .map(|value| value as f32)
        .filter(|value| value.is_finite())
        .unwrap_or(fallback)
}
fn props_key_string<'a>(
    props: &'a Value,
    key_index: usize,
    key: &str,
    fallback: &'a str,
) -> &'a str {
    props_key_value(props, key_index, key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback)
}
fn props_judgment_value<'a>(props: &'a Value, judgment_key: &str, key: &str) -> Option<&'a Value> {
    // Per-judgment overrides use stable judgment keys from JUDGMENT_KEYS.
    props
        .get("perJudgmentOverrides")
        .and_then(Value::as_object)
        .and_then(|overrides| overrides.get(judgment_key))
        .and_then(|override_props| override_props.get(key))
        .or_else(|| props.get(key))
}
fn props_judgment_f32(props: &Value, judgment_key: &str, key: &str, fallback: f32) -> f32 {
    props_judgment_value(props, judgment_key, key)
        .and_then(Value::as_f64)
        .map(|value| value as f32)
        .filter(|value| value.is_finite())
        .unwrap_or(fallback)
}
fn props_judgment_string<'a>(
    props: &'a Value,
    judgment_key: &str,
    key: &str,
    fallback: &'a str,
) -> &'a str {
    props_judgment_value(props, judgment_key, key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback)
}
fn judgment_counter_visible(props: &Value, judgment_key: &str) -> bool {
    let global_visible = props
        .get("visibleJudgments")
        .and_then(Value::as_object)
        .and_then(|visible| visible.get(judgment_key))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let override_visible = props
        .get("perJudgmentOverrides")
        .and_then(Value::as_object)
        .and_then(|overrides| overrides.get(judgment_key))
        .and_then(|override_props| override_props.get("visible"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    global_visible && override_visible
}
fn judgment_kind_index(kind: JudgmentKind) -> usize {
    match kind {
        JudgmentKind::Max => 0,
        JudgmentKind::Hit300 => 1,
        JudgmentKind::Hit200 => 2,
        JudgmentKind::Hit100 => 3,
        JudgmentKind::Hit50 => 4,
        JudgmentKind::Miss => 5,
    }
}
fn ease_out_cubic(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}
fn smoothstep(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn pop_pulse(elapsed: f32) -> f32 {
    const PEAK_AT: f32 = 0.34;
    if elapsed <= PEAK_AT {
        ease_out_cubic(elapsed / PEAK_AT)
    } else {
        1.0 - ease_out_cubic((elapsed - PEAK_AT) / (1.0 - PEAK_AT))
    }
}
fn key_counter_label(props: &Value, key_index: usize) -> String {
    props
        .get("labels")
        .and_then(Value::as_array)
        .and_then(|labels| labels.get(key_index))
        .and_then(Value::as_str)
        .filter(|label| !label.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("K{}", key_index + 1))
}
fn key_counter_tail_speed_px_per_ms(
    props: &Value,
    key_index: usize,
    fallback_height: f32,
    fallback_duration_ms: f32,
) -> f32 {
    let configured = props_key_f32(props, key_index, "tailReleaseSpeed", 0.0);
    if configured.is_finite() && configured > 0.0 {
        (configured / 1000.0).max(0.001)
    } else {
        (fallback_height / fallback_duration_ms.max(1.0) * KEY_COUNTER_TAIL_SPEED_MULTIPLIER)
            .max(0.001)
    }
}
fn parse_hex_color(input: &str, fallback: [f32; 4]) -> [f32; 4] {
    let trimmed = input.trim();
    let Some(hex) = trimmed.strip_prefix('#') else {
        return fallback;
    };
    let expanded;
    let hex = if hex.len() == 3 {
        expanded = hex.chars().flat_map(|ch| [ch, ch]).collect::<String>();
        expanded.as_str()
    } else {
        hex
    };
    if hex.len() != 6 && hex.len() != 8 {
        return fallback;
    }
    let Ok(raw) = u32::from_str_radix(hex, 16) else {
        return fallback;
    };
    let (r, g, b, a) = if hex.len() == 8 {
        (
            ((raw >> 24) & 0xff) as u8,
            ((raw >> 16) & 0xff) as u8,
            ((raw >> 8) & 0xff) as u8,
            (raw & 0xff) as u8,
        )
    } else {
        (
            ((raw >> 16) & 0xff) as u8,
            ((raw >> 8) & 0xff) as u8,
            (raw & 0xff) as u8,
            255,
        )
    };
    [
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    ]
}
fn parse_css_color(input: &str, fallback: [f32; 4]) -> [f32; 4] {
    let trimmed = input.trim();
    if trimmed.eq_ignore_ascii_case("transparent") {
        return [0.0, 0.0, 0.0, 0.0];
    }
    if trimmed.starts_with('#') {
        return parse_hex_color(trimmed, fallback);
    }
    let lower = trimmed.to_ascii_lowercase();
    let Some(args) = lower
        .strip_prefix("rgba(")
        .and_then(|value| value.strip_suffix(')'))
        .or_else(|| {
            lower
                .strip_prefix("rgb(")
                .and_then(|value| value.strip_suffix(')'))
        })
    else {
        return fallback;
    };
    let parts = args.split(',').map(str::trim).collect::<Vec<_>>();
    if parts.len() < 3 {
        return fallback;
    }
    let parse_channel = |value: &str| -> Option<f32> {
        value
            .trim_end_matches('%')
            .parse::<f32>()
            .ok()
            .map(|number| {
                if value.ends_with('%') {
                    (number / 100.0).clamp(0.0, 1.0)
                } else {
                    (number / 255.0).clamp(0.0, 1.0)
                }
            })
    };
    let (Some(r), Some(g), Some(b)) = (
        parse_channel(parts[0]),
        parse_channel(parts[1]),
        parse_channel(parts[2]),
    ) else {
        return fallback;
    };
    let a = parts
        .get(3)
        .and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    [r, g, b, a]
}
fn parse_color_value(value: Option<&str>, fallback: [f32; 4]) -> [f32; 4] {
    value
        .map(|raw| parse_css_color(raw, fallback))
        .unwrap_or(fallback)
}
fn to_u8_color(color: [f32; 4], opacity: f32) -> [u8; 4] {
    [
        (color[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[2].clamp(0.0, 1.0) * 255.0).round() as u8,
        ((color[3] * opacity).clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}
fn normalize_font_weight_number(weight: f32) -> u16 {
    let rounded = (weight / 100.0).round() * 100.0;
    rounded.clamp(100.0, 900.0) as u16
}
fn fallback_font_weight(weight: u16) -> FontWeight {
    if weight >= 600 {
        FontWeight::Bold
    } else {
        FontWeight::Normal
    }
}
fn hud_font_weight_number(style: &Value, props: &Value) -> u16 {
    let weight = value_f32(style, "fontWeight", value_f32(props, "fontWeight", 700.0));
    normalize_font_weight_number(weight)
}
fn hud_font_family<'a>(style: &'a Value, props: &'a Value) -> Option<&'a str> {
    value_string(style, "fontFamily")
        .or_else(|| value_string(props, "fontFamily"))
        .filter(|value| !value.trim().is_empty())
}
fn font_weight_number(weight: FontWeight) -> u16 {
    match weight {
        FontWeight::Bold => 700,
        FontWeight::Normal => 400,
    }
}
fn normalize_font_lookup_key(value: &str) -> String {
    let normalized = value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_ascii_lowercase();
    normalized
        .strip_suffix(" variable")
        .map(str::trim)
        .unwrap_or(&normalized)
        .to_string()
}
fn normalized_family_key(family: Option<&str>) -> String {
    family.map(normalize_font_lookup_key).unwrap_or_default()
}
fn hud_text_family_scale(family: Option<&str>) -> f32 {
    match normalized_family_key(family).as_str() {
        "league-spartan" | "league spartan" => 0.74,
        "newsreader" => 0.81,
        _ => 1.0,
    }
}
fn hud_synthetic_embolden_amount(family: Option<&str>, weight: u16) -> f32 {
    if weight < 600 {
        return 0.0;
    }
    match normalized_family_key(family).as_str() {
        "heebo" | "titillium-web" | "titillium web" => 0.0,
        _ if weight >= 800 => 0.46,
        _ if weight >= 700 => 0.34,
        _ => 0.22,
    }
}
fn font_ref_matches(font: &HudFontRefConfig, family: &str) -> bool {
    let requested_keys: Vec<String> = family
        .split(',')
        .map(normalize_font_lookup_key)
        .filter(|key| !key.is_empty())
        .collect();
    [
        font.id.as_str(),
        font.family.as_str(),
        font.css_family.as_deref().unwrap_or_default(),
    ]
    .iter()
    .any(|candidate| {
        !candidate.is_empty()
            && requested_keys
                .iter()
                .any(|key| normalize_font_lookup_key(candidate) == *key)
    })
}
fn configured_weight_path(font: &HudFontRefConfig, weight: u16) -> Option<&str> {
    if let Some(path) = font.weight_paths.get(&weight.to_string()) {
        return Some(path.as_str());
    }
    font.weight_paths
        .iter()
        .filter_map(|(key, path)| {
            let parsed = key.parse::<u16>().ok()?;
            Some((parsed.abs_diff(weight), parsed, path.as_str()))
        })
        .min_by_key(|(distance, parsed, _)| (*distance, parsed.abs_diff(400)))
        .map(|(_, _, path)| path)
}
fn configured_legacy_font_paths_for_weight(font: &HudFontRefConfig, weight: u16) -> Vec<&str> {
    let mut paths = Vec::new();
    match fallback_font_weight(weight) {
        FontWeight::Bold => {
            if let Some(path) = font.bold_path.as_deref() {
                paths.push(path);
            }
            if let Some(path) = font.path.as_deref() {
                paths.push(path);
            }
            if let Some(path) = font.normal_path.as_deref() {
                paths.push(path);
            }
        }
        FontWeight::Normal => {
            if let Some(path) = font.normal_path.as_deref() {
                paths.push(path);
            }
            if let Some(path) = font.path.as_deref() {
                paths.push(path);
            }
            if let Some(path) = font.bold_path.as_deref() {
                paths.push(path);
            }
        }
    }
    paths
}
fn push_unique_font_candidate(paths: &mut Vec<String>, path: impl Into<String>) {
    let path = path.into();
    if path.trim().is_empty() || paths.iter().any(|candidate| candidate == &path) {
        return;
    }
    paths.push(path);
}
fn fontsource_package_files_dir(root: &Path, package_name: &str) -> PathBuf {
    let mut files_dir = root.to_path_buf();
    for part in package_name.split('/') {
        if !part.is_empty() {
            files_dir.push(part);
        }
    }
    files_dir.push("files");
    files_dir
}
fn fontsource_package_candidates(font: &HudFontRefConfig) -> Vec<String> {
    let mut candidates = Vec::new();
    if !font.id.trim().is_empty() {
        candidates.push(format!("@fontsource/{}", font.id.trim()));
    }
    if let Some(package_name) = font
        .package_name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let package_name = package_name.to_string();
        if !candidates
            .iter()
            .any(|candidate| candidate == &package_name)
        {
            candidates.push(package_name);
        }
    }
    candidates
}
fn push_node_modules_roots_from_base(roots: &mut Vec<PathBuf>, base: &Path) {
    // The renderer may launch from the Rust crate, the web app, or a packaged binary.
    let candidates = [
        base.join("node_modules"),
        base.join("frontend").join("node_modules"),
        base.join("Miru Renderer Web")
            .join("frontend")
            .join("node_modules"),
    ];
    for candidate in candidates {
        if candidate.exists() && !roots.iter().any(|root| root == &candidate) {
            roots.push(candidate);
        }
    }
}
fn hud_font_node_modules_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for env_key in [
        "HUD_FONT_NODE_MODULES_ROOT",
        "MIRU_HUD_FONT_NODE_MODULES_ROOT",
    ] {
        if let Ok(value) = std::env::var(env_key) {
            let path = PathBuf::from(value);
            if path.exists() && !roots.iter().any(|root| root == &path) {
                roots.push(path);
            }
        }
    }
    if let Ok(current_dir) = std::env::current_dir() {
        for ancestor in current_dir.ancestors() {
            push_node_modules_roots_from_base(&mut roots, ancestor);
        }
    }
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            for ancestor in parent.ancestors() {
                push_node_modules_roots_from_base(&mut roots, ancestor);
            }
        }
    }
    roots
}
fn font_weight_candidate_numbers(weight: u16) -> Vec<u16> {
    let preferred = if weight >= 600 {
        [weight, 800, 700, 600, 500, 400, 300]
    } else {
        [weight, 400, 500, 300, 600, 700, 800]
    };
    let mut out = Vec::new();
    for candidate in preferred {
        if !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    out
}
fn pick_fontsource_file(files_dir: &Path, font_id: &str, weight: u16) -> Option<PathBuf> {
    let weight_numbers = font_weight_candidate_numbers(weight);
    let mut preferred: Vec<String> = weight_numbers
        .iter()
        .map(|weight| format!("{font_id}-japanese-{weight}-normal.woff2"))
        .collect();
    // Prefer Japanese subsets before Latin so mixed beatmap metadata keeps glyph coverage.
    preferred.extend(
        weight_numbers
            .iter()
            .map(|weight| format!("{font_id}-latin-{weight}-normal.woff2")),
    );
    preferred.extend(
        weight_numbers
            .iter()
            .map(|weight| format!("{font_id}-latin-ext-{weight}-normal.woff2")),
    );
    preferred.extend(
        weight_numbers
            .iter()
            .map(|weight| format!("{font_id}-0-{weight}-normal.woff2")),
    );
    preferred.extend(
        weight_numbers
            .iter()
            .map(|weight| format!("{font_id}-1-{weight}-normal.woff2")),
    );
    preferred.extend([
        format!("{font_id}-japanese-wght-normal.woff2"),
        format!("{font_id}-japanese-standard-normal.woff2"),
        format!("{font_id}-latin-wght-normal.woff2"),
        format!("{font_id}-latin-standard-normal.woff2"),
        format!("{font_id}-latin-opsz-normal.woff2"),
        format!("{font_id}-latin-ext-wght-normal.woff2"),
    ]);
    for filename in preferred {
        let candidate = files_dir.join(filename);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
fn font_path_candidates_for_weight(font: &HudFontRefConfig, weight: u16) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(path) = configured_weight_path(font, weight) {
        push_unique_font_candidate(&mut candidates, path.to_string());
    }
    if !font.id.trim().is_empty() {
        for root in hud_font_node_modules_roots() {
            for package_name in fontsource_package_candidates(font) {
                let files_dir = fontsource_package_files_dir(&root, &package_name);
                let Some(path) = pick_fontsource_file(&files_dir, &font.id, weight) else {
                    continue;
                };
                push_unique_font_candidate(&mut candidates, path.to_string_lossy().to_string());
            }
        }
    }
    for path in configured_legacy_font_paths_for_weight(font, weight) {
        push_unique_font_candidate(&mut candidates, path.to_string());
    }
    candidates
}
fn load_font_arc_from_path(path: &Path) -> Result<FontArc, String> {
    let bytes = std::fs::read(path)
        .map_err(|err| format!("failed to read font file {}: {err}", path.display()))?;
    let font_bytes = if woff2_patched::decode::is_woff2(&bytes) {
        woff2_patched::convert_woff2_to_ttf(&mut Cursor::new(bytes))
            .map_err(|err| format!("failed to decode WOFF2 {}: {err}", path.display()))?
    } else {
        bytes
    };
    FontArc::try_from_vec(font_bytes)
        .map_err(|_| format!("font parser rejected {}", path.display()))
}
fn with_opacity(mut color: [f32; 4], opacity: f32) -> [f32; 4] {
    color[3] = (color[3] * opacity).clamp(0.0, 1.0);
    color
}
fn hud_rotation_radians(degrees: f32) -> f32 {
    degrees.to_radians()
}
fn sprite_position(sprite: &SpriteCommand) -> [f32; 2] {
    sprite
        .precise_position
        .unwrap_or([sprite.x as f32, sprite.y as f32])
}
fn rotate_sprite_around_transform(sprite: &mut SpriteCommand, transform: &HudLayerTransformConfig) {
    let pos = sprite_position(sprite);
    // HUD rotation pivots around the layer rectangle, not the sprite's cropped top-left corner.
    sprite.origin = [
        transform.x + transform.width * 0.5 - pos[0],
        transform.y + transform.height * 0.5 - pos[1],
    ];
    sprite.rotation = hud_rotation_radians(transform.rotation);
}
fn add_sprite_around_transform(
    planner: &mut SpritePlanner,
    mut sprite: SpriteCommand,
    transform: &HudLayerTransformConfig,
) {
    rotate_sprite_around_transform(&mut sprite, transform);
    planner.add_sprite(sprite);
}
fn add_hud_text_sprite_clipped(
    planner: &mut SpritePlanner,
    texture_id: &str,
    draw_x: f32,
    draw_y: f32,
    draw_w: f32,
    draw_h: f32,
    clip_x: f32,
    clip_y: f32,
    clip_w: f32,
    clip_h: f32,
    z: i32,
    rotation_transform: &HudLayerTransformConfig,
) {
    let draw_w = draw_w.max(1.0);
    let draw_h = draw_h.max(1.0);
    let visible_x = draw_x.max(clip_x);
    let visible_y = draw_y.max(clip_y);
    let visible_right = (draw_x + draw_w).min(clip_x + clip_w);
    let visible_bottom = (draw_y + draw_h).min(clip_y + clip_h);
    let visible_w = visible_right - visible_x;
    let visible_h = visible_bottom - visible_y;
    if visible_w <= 0.5 || visible_h <= 0.5 {
        return;
    }
    // Clipping changes UVs instead of resizing the texture so scrolling text stays pixel-stable.
    let mut cmd = SpriteCommand::new(
        texture_id.to_string(),
        visible_x.round() as i32,
        visible_y.round() as i32,
        visible_w.round().max(1.0) as u32,
        visible_h.round().max(1.0) as u32,
    )
    .with_z(z);
    cmd.precise_position = Some([visible_x, visible_y]);
    cmd.precise_size = Some([visible_w, visible_h]);
    cmd.uv_rect = [
        ((visible_x - draw_x) / draw_w).clamp(0.0, 1.0),
        ((visible_y - draw_y) / draw_h).clamp(0.0, 1.0),
        ((visible_right - draw_x) / draw_w).clamp(0.0, 1.0),
        ((visible_bottom - draw_y) / draw_h).clamp(0.0, 1.0),
    ];
    rotate_sprite_around_transform(&mut cmd, rotation_transform);
    cmd.blend_mode = SpriteBlendMode::Alpha;
    planner.add_sprite(cmd);
}
fn rect_sprite(layer: &HudLayerConfig, color: [f32; 4], z: i32) -> SpriteCommand {
    let t = &layer.transform;
    let mut cmd = SpriteCommand::new(
        "solid_white",
        t.x.round() as i32,
        t.y.round() as i32,
        t.width.round().max(1.0) as u32,
        t.height.round().max(1.0) as u32,
    )
    .with_tint(with_opacity(color, t.opacity))
    .with_z(z);
    cmd.precise_position = Some([t.x, t.y]);
    cmd.precise_size = Some([t.width.max(1.0), t.height.max(1.0)]);
    cmd.origin = [t.width.max(1.0) * 0.5, t.height.max(1.0) * 0.5];
    cmd.rotation = hud_rotation_radians(t.rotation);
    cmd.blend_mode = SpriteBlendMode::Alpha;
    cmd
}
fn media_sprite(
    texture_id: impl Into<String>,
    layer: &HudLayerConfig,
    width: f32,
    height: f32,
    z: i32,
) -> SpriteCommand {
    let t = &layer.transform;
    let mut cmd = SpriteCommand::new(
        texture_id,
        t.x.round() as i32,
        t.y.round() as i32,
        width.round().max(1.0) as u32,
        height.round().max(1.0) as u32,
    )
    .with_tint([1.0, 1.0, 1.0, t.opacity.clamp(0.0, 1.0)])
    .with_z(z);
    cmd.precise_position = Some([t.x, t.y]);
    cmd.precise_size = Some([width.max(1.0), height.max(1.0)]);
    cmd.origin = [width.max(1.0) * 0.5, height.max(1.0) * 0.5];
    cmd.rotation = hud_rotation_radians(t.rotation);
    cmd.blend_mode = SpriteBlendMode::Alpha;
    cmd
}
fn color_to_rgba(color: [f32; 4]) -> Rgba<u8> {
    Rgba([
        (color[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[2].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[3].clamp(0.0, 1.0) * 255.0).round() as u8,
    ])
}
fn inside_rounded_rect(x: f32, y: f32, width: f32, height: f32, radius: f32) -> bool {
    if x < 0.0 || y < 0.0 || x > width || y > height {
        return false;
    }
    let radius = radius.max(0.0).min(width.min(height) * 0.5);
    if radius <= 0.0 {
        return true;
    }
    let cx = if x < radius {
        radius
    } else if x > width - radius {
        width - radius
    } else {
        x
    };
    let cy = if y < radius {
        radius
    } else if y > height - radius {
        height - radius
    } else {
        y
    };
    let dx = x - cx;
    let dy = y - cy;
    dx * dx + dy * dy <= radius * radius
}
fn render_rounded_rect_image(
    width: u32,
    height: u32,
    fill: [f32; 4],
    stroke: [f32; 4],
    stroke_width: f32,
    radius: f32,
) -> RgbaImage {
    let mut image = RgbaImage::new(width.max(1), height.max(1));
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let stroke_width = stroke_width.max(0.0);
    // Rects are rasterized once into cached textures because the sprite pipeline has no vector primitive.
    const AA_SAMPLES: usize = 4;
    const AA_SAMPLE_COUNT: f32 = (AA_SAMPLES * AA_SAMPLES) as f32;
    for y in 0..height.max(1) {
        for x in 0..width.max(1) {
            let mut outer_samples = 0_u32;
            let mut inner_samples = 0_u32;
            for sy in 0..AA_SAMPLES {
                for sx in 0..AA_SAMPLES {
                    let px = x as f32 + (sx as f32 + 0.5) / AA_SAMPLES as f32;
                    let py = y as f32 + (sy as f32 + 0.5) / AA_SAMPLES as f32;
                    if !inside_rounded_rect(px, py, w, h, radius) {
                        continue;
                    }
                    outer_samples += 1;
                    if stroke_width > 0.0
                        && inside_rounded_rect(
                            px - stroke_width,
                            py - stroke_width,
                            (w - stroke_width * 2.0).max(0.0),
                            (h - stroke_width * 2.0).max(0.0),
                            (radius - stroke_width).max(0.0),
                        )
                    {
                        inner_samples += 1;
                    }
                }
            }
            if outer_samples == 0 {
                continue;
            }
            let outer_coverage = outer_samples as f32 / AA_SAMPLE_COUNT;
            let inner_coverage = if stroke_width > 0.0 {
                inner_samples as f32 / AA_SAMPLE_COUNT
            } else {
                outer_coverage
            };
            let stroke_coverage = (outer_coverage - inner_coverage).max(0.0);
            let fill_alpha = fill[3].clamp(0.0, 1.0) * inner_coverage;
            let stroke_alpha = stroke[3].clamp(0.0, 1.0) * stroke_coverage;
            let out_alpha = (fill_alpha + stroke_alpha).clamp(0.0, 1.0);
            if out_alpha <= 0.0 {
                continue;
            }
            let red = (fill[0].clamp(0.0, 1.0) * fill_alpha
                + stroke[0].clamp(0.0, 1.0) * stroke_alpha)
                / out_alpha;
            let green = (fill[1].clamp(0.0, 1.0) * fill_alpha
                + stroke[1].clamp(0.0, 1.0) * stroke_alpha)
                / out_alpha;
            let blue = (fill[2].clamp(0.0, 1.0) * fill_alpha
                + stroke[2].clamp(0.0, 1.0) * stroke_alpha)
                / out_alpha;
            image.put_pixel(
                x,
                y,
                Rgba([
                    (red * 255.0).round() as u8,
                    (green * 255.0).round() as u8,
                    (blue * 255.0).round() as u8,
                    (out_alpha * 255.0).round() as u8,
                ]),
            );
        }
    }
    image
}
fn blend_pixel(image: &mut RgbaImage, x: i32, y: i32, color: Rgba<u8>) {
    if x < 0 || y < 0 || x >= image.width() as i32 || y >= image.height() as i32 {
        return;
    }
    let dst = image.get_pixel_mut(x as u32, y as u32);
    let src_a = color[3] as f32 / 255.0;
    let dst_a = dst[3] as f32 / 255.0;
    let out_a = src_a + dst_a * (1.0 - src_a);
    if out_a <= 0.0 {
        *dst = Rgba([0, 0, 0, 0]);
        return;
    }
    for channel in 0..3 {
        let src = color[channel] as f32 / 255.0;
        let old = dst[channel] as f32 / 255.0;
        dst[channel] = (((src * src_a + old * dst_a * (1.0 - src_a)) / out_a) * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8;
    }
    dst[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
}
fn blend_text_pixel(image: &mut RgbaImage, x: i32, y: i32, color: Rgba<u8>, coverage: f32) {
    if coverage <= 0.0 {
        return;
    }
    let mut covered = color;
    covered[3] = ((covered[3] as f32) * coverage.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    blend_pixel(image, x, y, covered);
}
fn draw_line_rgba(
    image: &mut RgbaImage,
    from: (f32, f32),
    to: (f32, f32),
    width: f32,
    color: Rgba<u8>,
) {
    let min_x = from.0.min(to.0).floor() as i32 - width.ceil() as i32 - 1;
    let max_x = from.0.max(to.0).ceil() as i32 + width.ceil() as i32 + 1;
    let min_y = from.1.min(to.1).floor() as i32 - width.ceil() as i32 - 1;
    let max_y = from.1.max(to.1).ceil() as i32 + width.ceil() as i32 + 1;
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let len_sq = (dx * dx + dy * dy).max(0.001);
    let radius = (width.max(1.0) * 0.5).max(0.5);
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let t = (((px - from.0) * dx + (py - from.1) * dy) / len_sq).clamp(0.0, 1.0);
            let cx = from.0 + dx * t;
            let cy = from.1 + dy * t;
            let dist = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
            if dist <= radius {
                blend_pixel(image, x, y, color);
            }
        }
    }
}
fn fill_rect_rgba(image: &mut RgbaImage, x: f32, y: f32, width: f32, height: f32, color: Rgba<u8>) {
    let left = x.floor().max(0.0) as i32;
    let top = y.floor().max(0.0) as i32;
    let right = (x + width).ceil().min(image.width() as f32) as i32;
    let bottom = (y + height).ceil().min(image.height() as f32) as i32;
    for py in top..bottom {
        for px in left..right {
            blend_pixel(image, px, py, color);
        }
    }
}
fn fill_circle_rgba(image: &mut RgbaImage, cx: f32, cy: f32, radius: f32, color: Rgba<u8>) {
    let radius = radius.max(0.5);
    let left = (cx - radius).floor() as i32 - 1;
    let right = (cx + radius).ceil() as i32 + 1;
    let top = (cy - radius).floor() as i32 - 1;
    let bottom = (cy + radius).ceil() as i32 + 1;
    for y in top..=bottom {
        for x in left..=right {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let distance = (dx * dx + dy * dy).sqrt();
            if distance <= radius {
                let coverage = (radius - distance + 0.5).clamp(0.0, 1.0);
                blend_text_pixel(image, x, y, color, coverage);
            }
        }
    }
}
fn fill_right_triangle_rgba(
    image: &mut RgbaImage,
    tip_x: f32,
    center_y: f32,
    width: f32,
    height: f32,
    color: Rgba<u8>,
) {
    let width = width.max(1.0);
    let half_h = (height.max(1.0) * 0.5).max(0.5);
    let left = (tip_x - width).floor() as i32 - 1;
    let right = tip_x.ceil() as i32 + 1;
    let top = (center_y - half_h).floor() as i32 - 1;
    let bottom = (center_y + half_h).ceil() as i32 + 1;
    for y in top..=bottom {
        let dy = (y as f32 + 0.5 - center_y).abs();
        let span = width * (1.0 - (dy / half_h).clamp(0.0, 1.0));
        let row_left = tip_x - span;
        for x in left..=right {
            let px = x as f32 + 0.5;
            if px >= row_left && px <= tip_x {
                blend_pixel(image, x, y, color);
            }
        }
    }
}
fn darken_color(mut color: [f32; 4], amount: f32) -> [f32; 4] {
    let factor = (1.0 - amount).clamp(0.0, 1.0);
    color[0] *= factor;
    color[1] *= factor;
    color[2] *= factor;
    color
}
fn interpolate_polyline_y(points: &[(f32, f32)], x: f32) -> Option<f32> {
    if points.is_empty() {
        return None;
    }
    if points.len() == 1 {
        return Some(points[0].1);
    }
    let first_x = points.first()?.0;
    let last_x = points.last()?.0;
    if x < first_x.min(last_x) || x > first_x.max(last_x) {
        return None;
    }
    for pair in points.windows(2) {
        let (x1, y1) = pair[0];
        let (x2, y2) = pair[1];
        if (x >= x1.min(x2) && x <= x1.max(x2)) || (x2 - x1).abs() < 0.001 {
            let t = if (x2 - x1).abs() < 0.001 {
                0.0
            } else {
                ((x - x1) / (x2 - x1)).clamp(0.0, 1.0)
            };
            return Some(y1 + (y2 - y1) * t);
        }
    }
    None
}
fn glyph_is_missing(glyph_id: GlyphId, ch: char) -> bool {
    !ch.is_whitespace() && !ch.is_control() && glyph_id.0 == 0
}
fn select_hud_text_font_for_char(
    fonts: &[&FontArc],
    scale: PxScale,
    ch: char,
) -> Option<(usize, GlyphId)> {
    // Select fallback fonts per glyph so one missing CJK character does not discard the requested family.
    for (index, font) in fonts.iter().enumerate() {
        let scaled = font.as_scaled(scale);
        let glyph_id = scaled.glyph_id(ch);
        if !glyph_is_missing(glyph_id, ch) {
            return Some((index, glyph_id));
        }
    }
    let primary = fonts.first()?;
    let scaled = primary.as_scaled(scale);
    Some((0, scaled.glyph_id('?')))
}
fn render_hud_text_with_fonts(
    text: &str,
    font_size: f32,
    color: [u8; 4],
    fonts: &[&FontArc],
) -> Option<RenderedText> {
    let primary = *fonts.first()?;
    let scale = PxScale::from(font_size);
    let mut max_ascent = primary.as_scaled(scale).ascent();
    let mut min_descent = primary.as_scaled(scale).descent();
    let mut max_line_gap = primary.as_scaled(scale).line_gap();
    for font in fonts.iter().skip(1) {
        let scaled = font.as_scaled(scale);
        max_ascent = max_ascent.max(scaled.ascent());
        min_descent = min_descent.min(scaled.descent());
        max_line_gap = max_line_gap.max(scaled.line_gap());
    }
    let line_height = (max_ascent - min_descent + max_line_gap).max(font_size * 1.2);
    let mut max_width = 0.0_f32;
    let mut line_width = 0.0_f32;
    let mut line_count = 1_u32;
    let mut previous = None::<(usize, GlyphId)>;
    for ch in text.chars() {
        if ch == '\n' {
            max_width = max_width.max(line_width);
            line_width = 0.0;
            line_count += 1;
            previous = None;
            continue;
        }
        let Some((font_index, glyph_id)) = select_hud_text_font_for_char(fonts, scale, ch) else {
            continue;
        };
        let scaled = fonts[font_index].as_scaled(scale);
        if let Some((previous_index, previous_id)) = previous {
            if previous_index == font_index {
                line_width += scaled.kern(previous_id, glyph_id);
            }
        }
        line_width += scaled.h_advance(glyph_id);
        previous = Some((font_index, glyph_id));
    }
    max_width = max_width.max(line_width);
    let img_w = max_width.ceil().max(1.0) as u32;
    let img_h = (line_height * line_count as f32).ceil().max(1.0) as u32;
    let mut image = RgbaImage::new(img_w, img_h);
    let mut caret_x = 0.0_f32;
    let mut baseline_y = max_ascent;
    let mut previous = None::<(usize, GlyphId)>;
    let color = Rgba(color);
    for ch in text.chars() {
        if ch == '\n' {
            caret_x = 0.0;
            baseline_y += line_height;
            previous = None;
            continue;
        }
        let Some((font_index, glyph_id)) = select_hud_text_font_for_char(fonts, scale, ch) else {
            continue;
        };
        let font = fonts[font_index];
        let scaled = font.as_scaled(scale);
        if let Some((previous_index, previous_id)) = previous {
            if previous_index == font_index {
                caret_x += scaled.kern(previous_id, glyph_id);
            }
        }
        let glyph = glyph_id.with_scale_and_position(scale, point(caret_x, baseline_y));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            let origin_x = bounds.min.x.floor() as i32;
            let origin_y = bounds.min.y.floor() as i32;
            outlined.draw(|gx, gy, coverage| {
                blend_text_pixel(
                    &mut image,
                    origin_x + gx as i32,
                    origin_y + gy as i32,
                    color,
                    coverage,
                );
            });
        }
        caret_x += scaled.h_advance(glyph_id);
        previous = Some((font_index, glyph_id));
    }
    Some(RenderedText {
        image,
        width: img_w,
        height: img_h,
    })
}
fn key_index(raw: &str) -> Option<u32> {
    let trimmed = raw.trim().to_ascii_lowercase();
    let number = trimmed.strip_prefix('k')?.parse::<u32>().ok()?;
    (1..=32).contains(&number).then_some(number - 1)
}
fn props_keys(props: &Value) -> Vec<String> {
    props
        .get("keys")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .take(10)
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| vec!["k1".into(), "k2".into(), "k3".into(), "k4".into()])
}
fn layer_asset_id(layer: &HudLayerConfig) -> Option<&str> {
    value_string(&layer.props, "assetId")
        .or_else(|| value_string(&layer.props, "asset_id"))
        .filter(|id| !id.trim().is_empty())
}
fn format_song_time(ms: i32) -> String {
    let total_seconds = (ms.max(0) / 1000).max(0);
    format!("{:02}:{:02}", total_seconds / 60, total_seconds % 60)
}
fn binding_text(binding: &str, hud_state: &HudFrameState) -> Option<String> {
    let text = match binding {
        "song.elapsed" => format_song_time(hud_state.song_elapsed_ms),
        "song.duration" => format_song_time(hud_state.song_duration_ms),
        "beatmap.title" => hud_state.beatmap.title.clone(),
        "beatmap.titleRomanized" => hud_state.beatmap.title_romanized.clone(),
        "beatmap.artist" => hud_state.beatmap.artist.clone(),
        "beatmap.artistRomanized" => hud_state.beatmap.artist_romanized.clone(),
        "beatmap.difficulty" => hud_state.beatmap.difficulty.clone(),
        "beatmap.mapper" => hud_state.beatmap.mapper.clone(),
        "beatmap.source" => hud_state.beatmap.source.clone(),
        "beatmap.tags" => hud_state.beatmap.tags.clone(),
        "beatmap.bpm" => hud_state.beatmap.bpm_text.clone(),
        _ => return None,
    };
    Some(text)
}
fn binding_number(binding: &str, hud_state: &HudFrameState, key_mask: u32) -> Option<f32> {
    match binding {
        "judgments.max" => Some(hud_state.judgment_counts[0] as f32),
        "judgments.hit300" => Some(hud_state.judgment_counts[1] as f32),
        "judgments.hit200" => Some(hud_state.judgment_counts[2] as f32),
        "judgments.hit100" => Some(hud_state.judgment_counts[3] as f32),
        "judgments.hit50" => Some(hud_state.judgment_counts[4] as f32),
        "judgments.miss" => Some(hud_state.judgment_counts[5] as f32),
        "score.current" => Some(hud_state.score as f32),
        "score.accuracy" => Some((hud_state.accuracy * 100.0) as f32),
        "score.ratio" => Some((hud_state.accuracy * 100.0) as f32),
        "score.combo" => Some(hud_state.combo as f32),
        "song.progress" => Some(hud_state.progress),
        "song.elapsed" => Some((hud_state.song_elapsed_ms.max(0) as f32) / 1000.0),
        "song.duration" => Some((hud_state.song_duration_ms.max(0) as f32) / 1000.0),
        "beatmap.beatmapId" => hud_state.beatmap.beatmap_id.map(|value| value as f32),
        "beatmap.beatmapSetId" => hud_state.beatmap.beatmapset_id.map(|value| value as f32),
        "beatmap.keyCount" => Some(hud_state.beatmap.key_count as f32),
        "beatmap.cs" => Some(hud_state.beatmap.cs),
        "beatmap.od" => Some(hud_state.beatmap.od),
        "beatmap.hp" => Some(hud_state.beatmap.hp),
        "beatmap.bpm" => Some(hud_state.beatmap.bpm),
        "beatmap.noteCount" => Some(hud_state.beatmap.note_count as f32),
        "beatmap.maxCombo" => Some(hud_state.beatmap.max_combo as f32),
        "input.totalKps" => Some(hud_state.total_kps),
        "input.ur" => hud_state.unstable_rate,
        "pp.current" => hud_state.pp_current,
        "pp.final" => hud_state.pp_final,
        key if key.starts_with("keys.") && key.ends_with(".kps") => {
            let key_name = key.trim_start_matches("keys.").trim_end_matches(".kps");
            key_index(key_name).map(|idx| hud_state.key_kps[idx as usize])
        }
        key if key.starts_with("keys.") && key.ends_with(".pressDurationMs") => {
            let key_name = key
                .trim_start_matches("keys.")
                .trim_end_matches(".pressDurationMs");
            key_index(key_name).map(|idx| hud_state.key_press_duration_ms[idx as usize] as f32)
        }
        key if key.starts_with("keys.") && key.ends_with(".down") => {
            let key_name = key.trim_start_matches("keys.").trim_end_matches(".down");
            key_index(key_name).map(|idx| ((key_mask & (1_u32 << idx)) != 0) as u8 as f32)
        }
        _ => None,
    }
}
fn binding_bool(binding: &str, hud_state: &HudFrameState, key_mask: u32) -> bool {
    binding_number(binding, hud_state, key_mask)
        .map(|value| value > 0.0)
        .unwrap_or(false)
}
fn judgment_binding_index(binding: &str) -> Option<usize> {
    match binding {
        "judgments.max" => Some(0),
        "judgments.hit300" => Some(1),
        "judgments.hit200" => Some(2),
        "judgments.hit100" => Some(3),
        "judgments.hit50" => Some(4),
        "judgments.miss" => Some(5),
        _ => None,
    }
}
fn judgment_change_age_for_binding(hud_state: &HudFrameState, binding: &str) -> Option<i32> {
    let expected = judgment_binding_index(binding)?;
    hud_state
        .last_judgment
        .filter(|judgment| judgment_kind_index(judgment.kind) == expected)
        .map(|judgment| judgment.age_ms)
        .filter(|age| *age >= 0)
}
fn effect_progress(
    layer: &HudLayerConfig,
    trigger: &str,
    binding: Option<&str>,
    duration_ms: i32,
    timestamp_ms: i32,
    hud_state: &HudFrameState,
    key_mask: u32,
) -> f32 {
    match trigger {
        "continuous" => {
            let duration = duration_ms.max(80) as f32;
            ((timestamp_ms as f32 / duration).sin() + 1.0) * 0.5
        }
        "onPress" => binding
            .map(|binding| binding_bool(binding, hud_state, key_mask) as u8 as f32)
            .unwrap_or(0.0),
        "onRelease" => binding
            .map(|binding| (!binding_bool(binding, hud_state, key_mask)) as u8 as f32)
            .unwrap_or(0.0),
        "onChange" => {
            // onChange currently tracks judgment bindings through the last visible judgment animation window.
            let age = binding
                .or(layer.binding.as_deref())
                .and_then(|binding| judgment_change_age_for_binding(hud_state, binding));
            age.map(|age| 1.0 - (age as f32 / duration_ms.max(1) as f32).clamp(0.0, 1.0))
                .unwrap_or(0.0)
        }
        _ => 0.0,
    }
}
fn layer_with_effects(
    layer: &HudLayerConfig,
    hud_state: &HudFrameState,
    key_mask: u32,
    timestamp_ms: i32,
) -> (HudLayerConfig, Option<[f32; 4]>, f32) {
    let mut next = layer.clone();
    let mut translate_x = 0.0;
    let mut translate_y = 0.0;
    let mut scale = 1.0;
    let mut fill_flash = None;
    let mut stroke_glow = 0.0_f32;
    for effect in &layer.effects {
        let binding = effect.binding.as_deref().or(layer.binding.as_deref());
        let progress = effect_progress(
            layer,
            &effect.trigger,
            binding,
            effect.duration_ms,
            timestamp_ms.saturating_sub(effect.delay_ms),
            hud_state,
            key_mask,
        );
        if progress <= 0.0 {
            continue;
        }
        let from = number_value(&effect.from).unwrap_or(0.0);
        let to = number_value(&effect.to).unwrap_or(1.0);
        let amount = from + (to - from) * progress;
        match effect.property.as_str() {
            "translateX" => translate_x += amount,
            "translateY" => translate_y += amount,
            "scale" => scale *= amount.max(0.01),
            "opacity" => next.transform.opacity = amount.clamp(0.0, 1.0),
            "strokeGlow" => stroke_glow = stroke_glow.max(amount.clamp(0.0, 1.0)),
            "fillFlash" => {
                if let Some(color) = effect.to.as_str() {
                    fill_flash = Some(parse_css_color(color, [1.0, 1.0, 1.0, 1.0]));
                }
            }
            _ => {}
        }
    }
    if translate_x != 0.0 || translate_y != 0.0 || (scale - 1.0).abs() > f32::EPSILON {
        let cx = next.transform.x + next.transform.width * 0.5;
        let cy = next.transform.y + next.transform.height * 0.5;
        next.transform.width = (next.transform.width * scale).max(1.0);
        next.transform.height = (next.transform.height * scale).max(1.0);
        next.transform.x = cx - next.transform.width * 0.5 + translate_x;
        next.transform.y = cy - next.transform.height * 0.5 + translate_y;
    }
    let spin_rotation = spin_rotation_degrees(layer, timestamp_ms);
    if spin_rotation != 0.0 {
        next.transform.rotation += spin_rotation;
    }
    (next, fill_flash, stroke_glow)
}
struct EffectiveHudLayer {
    layer: HudLayerConfig,
    z_index: i32,
}
#[derive(Clone, Copy)]
struct HudTransformContext {
    cos: f32,
    sin: f32,
    cx: f32,
    cy: f32,
    rotation_degrees: f32,
    opacity: f32,
}
impl HudTransformContext {
    fn identity() -> Self {
        Self {
            cos: 1.0,
            sin: 0.0,
            cx: 0.0,
            cy: 0.0,
            rotation_degrees: 0.0,
            opacity: 1.0,
        }
    }
    fn group_rotation(transform: &HudLayerTransformConfig) -> Self {
        let radians = transform.rotation.to_radians();
        Self {
            cos: radians.cos(),
            sin: radians.sin(),
            cx: transform.x + transform.width * 0.5,
            cy: transform.y + transform.height * 0.5,
            rotation_degrees: transform.rotation,
            opacity: transform.opacity.clamp(0.0, 1.0),
        }
    }
    fn has_transform(self) -> bool {
        self.rotation_degrees.abs() > f32::EPSILON || (self.opacity - 1.0).abs() > f32::EPSILON
    }
    fn apply_point(self, x: f32, y: f32) -> (f32, f32) {
        let dx = x - self.cx;
        let dy = y - self.cy;
        (
            self.cx + dx * self.cos - dy * self.sin,
            self.cy + dx * self.sin + dy * self.cos,
        )
    }
    fn apply_layer(self, node: &HudLayerConfig) -> HudLayerConfig {
        if !self.has_transform() {
            return node.clone();
        }
        let mut next = node.clone();
        let center_x = next.transform.x + next.transform.width * 0.5;
        let center_y = next.transform.y + next.transform.height * 0.5;
        let (rotated_x, rotated_y) = self.apply_point(center_x, center_y);
        next.transform.x = rotated_x - next.transform.width * 0.5;
        next.transform.y = rotated_y - next.transform.height * 0.5;
        next.transform.rotation += self.rotation_degrees;
        next.transform.opacity = (next.transform.opacity * self.opacity).clamp(0.0, 1.0);
        next
    }
    fn chain(self, next: Self) -> Self {
        if !self.has_transform() {
            return next;
        }
        if !next.has_transform() {
            return self;
        }
        let (cx, cy) = self.apply_point(next.cx, next.cy);
        let rotation_degrees = self.rotation_degrees + next.rotation_degrees;
        let radians = rotation_degrees.to_radians();
        Self {
            cos: radians.cos(),
            sin: radians.sin(),
            cx,
            cy,
            rotation_degrees,
            opacity: (self.opacity * next.opacity).clamp(0.0, 1.0),
        }
    }
}
fn collect_hud_nodes(
    nodes: &[HudLayerConfig],
    out: &mut Vec<EffectiveHudLayer>,
    parent_z: Option<i32>,
    parent_visible: bool,
    parent_transform: HudTransformContext,
    timestamp_ms: i32,
) {
    for node in nodes {
        if !parent_visible {
            continue;
        }
        // Parent z values reserve a 1000-slot range for each top-level node.
        let effective_z = parent_z
            .map(|base| base + node.z_index.clamp(0, 999))
            .unwrap_or_else(|| node.z_index.saturating_mul(1000));
        let layer = parent_transform.apply_layer(node);
        out.push(EffectiveHudLayer {
            layer: layer.clone(),
            z_index: effective_z,
        });
        if !node.children.is_empty() {
            let child_base = if node.layer_type == "group" {
                Some(effective_z)
            } else {
                parent_z
            };
            let child_transform = if node.layer_type == "group" {
                let mut group_transform = node.transform.clone();
                group_transform.rotation += spin_rotation_degrees(node, timestamp_ms);
                parent_transform.chain(HudTransformContext::group_rotation(&group_transform))
            } else {
                parent_transform
            };
            collect_hud_nodes(
                &node.children,
                out,
                child_base,
                node.visible,
                child_transform,
                timestamp_ms,
            );
        }
    }
}
fn hud_asset_path(asset: &HudAssetRefConfig) -> Option<std::path::PathBuf> {
    asset
        .path
        .as_deref()
        .map(Path::new)
        .filter(|path| path.is_file())
        .map(Path::to_path_buf)
}
fn hud_asset_file_within_limits(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|metadata| metadata.len() > 0 && metadata.len() <= HUD_ASSET_MAX_FILE_BYTES)
        .unwrap_or(false)
}
fn hud_asset_dimensions_within_limits(width: u32, height: u32) -> bool {
    if width == 0 || height == 0 {
        return false;
    }
    if width > HUD_ASSET_MAX_DIMENSION || height > HUD_ASSET_MAX_DIMENSION {
        return false;
    }
    (width as u64).saturating_mul(height as u64) <= HUD_ASSET_MAX_PIXELS
}
fn sanitize_texture_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "asset".to_string()
    } else {
        sanitized
    }
}
impl ReplayRenderer {
    fn warn_hud_font_once(&mut self, key: String, message: String) {
        if self.hud_font_warning_cache.insert(key) {
            eprintln!("warn: {message}");
        }
    }
    fn load_hud_texture_rgba_linear(
        &mut self,
        texture_id: &str,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) -> bool {
        if self.update_or_load_texture_rgba(texture_id, rgba, width.max(1), height.max(1)) {
            self.linear_sampled_textures
                .insert(crate::types::SkinAssets::normalize_key(texture_id));
            true
        } else {
            false
        }
    }
    pub(crate) fn plan_builtin_hit_error_meter(
        &mut self,
        planner: &mut SpritePlanner,
        hud_state: &HudFrameState,
        config: Option<&HudElementConfig>,
        timestamp_ms: i32,
        z: i32,
    ) {
        if config.and_then(|cfg| cfg.visible) == Some(false) {
            return;
        }

        let canvas_w = self.cfg.width.max(1) as f32;
        let canvas_h = self.cfg.height.max(1) as f32;
        let scale = (canvas_h / 720.0).max(0.5);
        let min_margin = 16.0 * scale;
        let max_width = (canvas_w - min_margin * 2.0).max(96.0);
        let default_width = (canvas_w * 0.28)
            .clamp(230.0 * scale, 420.0 * scale)
            .min(max_width);
        let default_height = (22.0 * scale).clamp(16.0, 32.0);
        let size_scale = config
            .and_then(|cfg| cfg.scale)
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or(1.0);
        let width = (config
            .and_then(|cfg| cfg.width)
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or(default_width)
            * size_scale)
            .min(max_width)
            .max(64.0);
        let height = (config
            .and_then(|cfg| cfg.height.or(cfg.size))
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or(default_height)
            * size_scale)
            .min((canvas_h * 0.18).max(16.0))
            .max(10.0);
        let x = config
            .and_then(|cfg| cfg.x)
            .filter(|value| value.is_finite())
            .unwrap_or((canvas_w - width) * 0.5);
        let bottom_margin = (24.0 * scale).clamp(16.0, 44.0);
        let y = config
            .and_then(|cfg| cfg.y)
            .filter(|value| value.is_finite())
            .unwrap_or(canvas_h - height - bottom_margin);
        let opacity = 0.96;
        let rotation = self.hud_element_runtime_rotation(config, timestamp_ms);
        let layer = HudLayerConfig {
            id: "builtinHitErrorMeter".to_string(),
            name: "Built-in hit error meter".to_string(),
            layer_type: "component.hitErrorMeter".to_string(),
            visible: true,
            transform: HudLayerTransformConfig {
                x,
                y,
                width,
                height,
                rotation,
                opacity,
            },
            props: json!({
                "orientation": "horizontal",
                "labelStyle": "none",
                "padding": 3.0 * scale,
                "axisHeight": 8.0 * scale,
                "colorBarHeight": 2.0 * scale,
                "centreMarkerStyle": "line",
                "centreMarkerSize": 10.0 * scale,
                "judgementLineThickness": 4.0 * scale,
                "judgementFadeMs": 5000.0,
                "colourBarVisibility": true,
                "showMovingAverage": true,
            }),
            ..Default::default()
        };
        self.plan_hud_hit_error_meter(planner, &layer, hud_state, z);
    }
    fn ensure_hud_rect_texture(
        &mut self,
        width: u32,
        height: u32,
        fill: [f32; 4],
        stroke: [f32; 4],
        stroke_width: f32,
        radius: f32,
    ) -> Option<String> {
        let mut hasher = DefaultHasher::new();
        width.hash(&mut hasher);
        height.hash(&mut hasher);
        to_u8_color(fill, 1.0).hash(&mut hasher);
        to_u8_color(stroke, 1.0).hash(&mut hasher);
        stroke_width.to_bits().hash(&mut hasher);
        radius.to_bits().hash(&mut hasher);
        // Geometry and style form the cache key so identical HUD rects reuse one texture.
        let texture_id = format!("hud_rect_{:016x}", hasher.finish());
        if !self.loaded_textures.contains(&texture_id) {
            let image =
                render_rounded_rect_image(width, height, fill, stroke, stroke_width, radius);
            if !self.load_hud_texture_rgba_linear(
                &texture_id,
                image.as_raw(),
                width.max(1),
                height.max(1),
            ) {
                return None;
            }
        }
        self.linear_sampled_textures.insert(texture_id.clone());
        Some(texture_id)
    }
    fn ensure_hud_tail_texture(
        &mut self,
        height: u32,
        color: [f32; 4],
        opacity: f32,
        fade_from_start: bool,
    ) -> Option<String> {
        let height = height.max(1);
        let mut hasher = DefaultHasher::new();
        height.hash(&mut hasher);
        to_u8_color(color, opacity).hash(&mut hasher);
        fade_from_start.hash(&mut hasher);
        let texture_id = format!("hud_tail_{:016x}", hasher.finish());
        if !self.loaded_textures.contains(&texture_id) {
            let mut image = RgbaImage::new(1, height);
            let fade_edge = 40.0_f32.min(height as f32).max(1.0);
            for y in 0..height {
                let pos = y as f32 + 0.5;
                let fade = if fade_from_start {
                    (pos / fade_edge).clamp(0.0, 1.0)
                } else {
                    ((height as f32 - pos) / fade_edge).clamp(0.0, 1.0)
                };
                image.put_pixel(0, y, color_to_rgba(with_opacity(color, opacity * fade)));
            }
            if !self.load_hud_texture_rgba_linear(&texture_id, image.as_raw(), 1, height) {
                return None;
            }
        }
        self.linear_sampled_textures.insert(texture_id.clone());
        Some(texture_id)
    }
    pub(crate) fn plan_hud_layers(
        &mut self,
        planner: &mut SpritePlanner,
        hud_state: &HudFrameState,
        key_mask: u32,
        timestamp_ms: i32,
    ) {
        let Some(config) = self.hud_config.as_ref() else {
            return;
        };
        if config.version != Some(4) {
            return;
        }
        // v4 supports both the new tree field and the older flat layer list.
        let roots = if !config.nodes.is_empty() {
            config.nodes.clone()
        } else {
            config.layers.clone()
        };
        if roots.is_empty() {
            return;
        }
        let mut layers = Vec::new();
        collect_hud_nodes(
            &roots,
            &mut layers,
            None,
            true,
            HudTransformContext::identity(),
            timestamp_ms,
        );
        layers.sort_by_key(|entry| entry.z_index);
        for entry in layers {
            let EffectiveHudLayer { layer, z_index } = entry;
            if !layer.visible {
                continue;
            }
            let (layer, flash_fill, stroke_glow) =
                layer_with_effects(&layer, hud_state, key_mask, timestamp_ms);
            let z = z_order::HUD + 60 + z_index;
            match layer.layer_type.as_str() {
                "group" => {}
                "shape.rect" => self.plan_hud_rect(planner, &layer, flash_fill, stroke_glow, z),
                "shape.line" => self.plan_hud_line(planner, &layer, z),
                "media.image" | "media.gif" | "icon.static" => {
                    self.plan_hud_media(planner, &layer, timestamp_ms, z)
                }
                "text.static" => {
                    self.plan_hud_text_layer(planner, &layer, hud_state, key_mask, timestamp_ms, z)
                }
                "text.bound" => {
                    self.plan_hud_text_layer(planner, &layer, hud_state, key_mask, timestamp_ms, z)
                }
                "graph.bar" => self.plan_hud_bar(planner, &layer, hud_state, key_mask, z),
                "graph.sparkline" => {
                    self.plan_hud_sparkline(planner, &layer, hud_state, timestamp_ms, z)
                }
                "component.keyCounter" => self.plan_hud_component_key_counter(
                    planner,
                    &layer,
                    hud_state,
                    key_mask,
                    timestamp_ms,
                    z,
                ),
                "component.judgmentCounter" => self.plan_hud_component_judgment_counter(
                    planner,
                    &layer,
                    hud_state,
                    timestamp_ms,
                    z,
                ),
                "repeater.keyColumns" => {
                    self.plan_hud_key_press(planner, &layer, hud_state, key_mask, z)
                }
                "widget.judgmentCounter" => {
                    self.plan_hud_judgment_counter(planner, &layer, hud_state, z)
                }
                "widget.keyPress" => {
                    self.plan_hud_key_press(planner, &layer, hud_state, key_mask, z)
                }
                "widget.kpsCounter" => self.plan_hud_badge(
                    planner,
                    &layer,
                    "KPS",
                    &format!("{:.0}", hud_state.total_kps),
                    z,
                ),
                "widget.ppCounter" => self.plan_hud_badge(
                    planner,
                    &layer,
                    "PP",
                    &hud_state
                        .pp_current
                        .or(hud_state.pp_final)
                        .map(|pp| format!("{pp:.0}"))
                        .unwrap_or_else(|| "--".to_string()),
                    z,
                ),
                "widget.kpsGraph" => self.plan_hud_kps_graph(planner, &layer, z),
                "widget.hitErrorMeter" | "component.hitErrorMeter" => {
                    self.plan_hud_hit_error_meter(planner, &layer, hud_state, z)
                }
                _ => {}
            }
        }
    }
    fn plan_hud_rect(
        &mut self,
        planner: &mut SpritePlanner,
        layer: &HudLayerConfig,
        flash_fill: Option<[f32; 4]>,
        stroke_glow: f32,
        z: i32,
    ) {
        let fill = flash_fill.unwrap_or_else(|| {
            parse_color_value(
                value_string(&layer.style, "fill").or_else(|| value_string(&layer.props, "fill")),
                [0.05, 0.07, 0.11, 0.78],
            )
        });
        let fill = with_opacity(fill, value_f32(&layer.style, "fillOpacity", 1.0));
        let stroke = with_opacity(
            parse_color_value(
                value_string(&layer.style, "stroke")
                    .or_else(|| value_string(&layer.style, "borderColor")),
                [0.0, 0.0, 0.0, 0.0],
            ),
            value_f32(&layer.style, "strokeOpacity", 1.0),
        );
        let stroke_width = value_f32(
            &layer.style,
            "strokeWidth",
            value_f32(&layer.style, "borderWidth", 0.0),
        )
        .max(0.0);
        let radius = value_f32(&layer.style, "radius", 0.0).max(0.0);
        if stroke_glow > 0.0 {
            let active_stroke = parse_color_value(
                value_string(&layer.style, "activeStroke"),
                [0.44, 0.90, 1.0, 1.0],
            );
            let grow = (stroke_width.max(2.0) * 3.0 + 6.0) * stroke_glow;
            let glow_layer = HudLayerConfig {
                transform: HudLayerTransformConfig {
                    x: layer.transform.x - grow,
                    y: layer.transform.y - grow,
                    width: layer.transform.width + grow * 2.0,
                    height: layer.transform.height + grow * 2.0,
                    rotation: layer.transform.rotation,
                    opacity: layer.transform.opacity,
                },
                ..layer.clone()
            };
            if let Some(texture_id) = self.ensure_hud_rect_texture(
                glow_layer.transform.width.round().max(1.0) as u32,
                glow_layer.transform.height.round().max(1.0) as u32,
                [0.0, 0.0, 0.0, 0.0],
                with_opacity(active_stroke, 0.42 * stroke_glow),
                (stroke_width.max(1.0) + grow * 0.45).max(1.0),
                radius + grow,
            ) {
                planner.add_sprite(media_sprite(
                    texture_id,
                    &glow_layer,
                    glow_layer.transform.width,
                    glow_layer.transform.height,
                    z - 1,
                ));
            }
        }
        if stroke_width <= 0.0 && radius <= 0.0 {
            planner.add_sprite(rect_sprite(layer, fill, z));
            return;
        }
        let width = layer.transform.width.round().max(1.0) as u32;
        let height = layer.transform.height.round().max(1.0) as u32;
        if let Some(texture_id) =
            self.ensure_hud_rect_texture(width, height, fill, stroke, stroke_width, radius)
        {
            planner.add_sprite(media_sprite(
                texture_id,
                layer,
                layer.transform.width,
                layer.transform.height,
                z,
            ));
        }
    }
    fn plan_hud_line(&mut self, planner: &mut SpritePlanner, layer: &HudLayerConfig, z: i32) {
        let stroke = parse_color_value(
            value_string(&layer.style, "stroke").or_else(|| value_string(&layer.props, "stroke")),
            [0.95, 0.97, 1.0, 1.0],
        );
        let stroke_width =
            value_f32(&layer.style, "strokeWidth", layer.transform.height.max(1.0)).max(1.0);
        let line_layer = HudLayerConfig {
            transform: crate::hud::HudLayerTransformConfig {
                y: layer.transform.y + (layer.transform.height - stroke_width) * 0.5,
                height: stroke_width,
                ..layer.transform.clone()
            },
            ..layer.clone()
        };
        planner.add_sprite(rect_sprite(&line_layer, stroke, z));
    }
    fn plan_hud_media(
        &mut self,
        planner: &mut SpritePlanner,
        layer: &HudLayerConfig,
        timestamp_ms: i32,
        z: i32,
    ) {
        let Some(asset_id) = layer_asset_id(layer) else {
            return;
        };
        let Some(frames) = self.ensure_hud_asset_frames(asset_id) else {
            return;
        };
        if frames.is_empty() {
            return;
        }
        let total_delay: u32 = frames.iter().map(|frame| frame.delay_ms.max(20)).sum();
        // GIF playback loops by accumulated frame delays, with a 20 ms floor for zero-delay frames.
        let mut cursor = if total_delay > 0 {
            timestamp_ms.rem_euclid(total_delay as i32) as u32
        } else {
            0
        };
        let mut selected = frames[0].clone();
        for frame in frames {
            let delay = frame.delay_ms.max(20);
            if cursor < delay {
                selected = frame;
                break;
            }
            cursor = cursor.saturating_sub(delay);
        }
        let target_w = if layer.transform.width > 0.0 {
            layer.transform.width
        } else {
            selected.width.max(1) as f32
        };
        let target_h = if layer.transform.height > 0.0 {
            layer.transform.height
        } else {
            selected.height.max(1) as f32
        };
        planner.add_sprite(media_sprite(
            selected.texture_id,
            layer,
            target_w,
            target_h,
            z,
        ));
    }
    fn plan_hud_bar(
        &mut self,
        planner: &mut SpritePlanner,
        layer: &HudLayerConfig,
        hud_state: &HudFrameState,
        key_mask: u32,
        z: i32,
    ) {
        let bg = parse_color_value(
            value_string(&layer.style, "background")
                .or_else(|| value_string(&layer.style, "backgroundColor")),
            [1.0, 1.0, 1.0, 0.08],
        );
        let radius = value_f32(&layer.style, "radius", 0.0).max(0.0);
        let stroke_width = value_f32(
            &layer.style,
            "strokeWidth",
            value_f32(&layer.style, "borderWidth", 0.0),
        )
        .max(0.0);
        let stroke = parse_color_value(
            value_string(&layer.style, "stroke")
                .or_else(|| value_string(&layer.style, "borderColor")),
            [0.0, 0.0, 0.0, 0.0],
        );
        if radius > 0.0 || stroke_width > 0.0 {
            if let Some(texture_id) = self.ensure_hud_rect_texture(
                layer.transform.width.round().max(1.0) as u32,
                layer.transform.height.round().max(1.0) as u32,
                bg,
                stroke,
                stroke_width,
                radius,
            ) {
                planner.add_sprite(media_sprite(
                    texture_id,
                    layer,
                    layer.transform.width,
                    layer.transform.height,
                    z,
                ));
            }
        } else {
            planner.add_sprite(rect_sprite(layer, bg, z));
        }
        let max = value_f32(&layer.props, "max", 1.0).max(0.0001);
        let value = layer
            .binding
            .as_deref()
            .and_then(|binding| binding_number(binding, hud_state, key_mask))
            .unwrap_or(0.0);
        let ratio = (value / max).clamp(0.0, 1.0);
        let fill = parse_color_value(value_string(&layer.style, "fill"), [0.42, 0.90, 1.0, 1.0]);
        let fill_layer = HudLayerConfig {
            transform: crate::hud::HudLayerTransformConfig {
                width: (layer.transform.width * ratio).max(1.0),
                ..layer.transform.clone()
            },
            ..layer.clone()
        };
        if radius > 0.0 {
            if let Some(texture_id) = self.ensure_hud_rect_texture(
                fill_layer.transform.width.round().max(1.0) as u32,
                fill_layer.transform.height.round().max(1.0) as u32,
                fill,
                [0.0, 0.0, 0.0, 0.0],
                0.0,
                radius,
            ) {
                add_sprite_around_transform(
                    planner,
                    media_sprite(
                        texture_id,
                        &fill_layer,
                        fill_layer.transform.width,
                        fill_layer.transform.height,
                        z + 1,
                    ),
                    &layer.transform,
                );
            }
        } else {
            add_sprite_around_transform(
                planner,
                rect_sprite(&fill_layer, fill, z + 1),
                &layer.transform,
            );
        }
    }
    fn plan_hud_sparkline(
        &mut self,
        planner: &mut SpritePlanner,
        layer: &HudLayerConfig,
        hud_state: &HudFrameState,
        timestamp_ms: i32,
        z: i32,
    ) {
        let t = &layer.transform;
        let mut background = parse_color_value(
            value_string(&layer.style, "backgroundColor")
                .or_else(|| value_string(&layer.style, "background")),
            [0.02, 0.04, 0.07, 1.0],
        );
        background[3] *= value_f32(&layer.props, "backgroundOpacity", 0.72).clamp(0.0, 1.0);
        let width = t.width.round().max(1.0) as u32;
        let height = t.height.round().max(1.0) as u32;
        let border_width = value_f32(&layer.style, "borderWidth", 1.0).max(0.0);
        let border_color = parse_color_value(
            value_string(&layer.style, "borderColor"),
            [0.15, 0.22, 0.28, 1.0],
        );
        let radius = value_f32(&layer.style, "radius", 0.0).max(0.0);
        let mut image = render_rounded_rect_image(
            width,
            height,
            background,
            border_color,
            border_width,
            radius,
        );
        let padding = value_f32(
            &layer.props,
            "padding",
            (t.width.min(t.height) * 0.08).clamp(8.0, 18.0),
        )
        .clamp(0.0, 40.0);
        let inner_x = padding;
        let inner_y = padding;
        let inner_w = (width as f32 - padding * 2.0).max(1.0);
        let inner_h = (height as f32 - padding * 2.0).max(1.0);
        if props_bool(&layer.props, "showGrid", true) {
            let mut grid_color = parse_color_value(
                value_string(&layer.style, "gridColor"),
                [0.44, 0.90, 1.0, 1.0],
            );
            grid_color[3] *= value_f32(&layer.props, "gridOpacity", 0.16).clamp(0.0, 1.0);
            let grid_color = color_to_rgba(grid_color);
            let columns = value_f32(&layer.props, "gridColumns", 6.0)
                .round()
                .clamp(1.0, 16.0) as usize;
            let rows = value_f32(&layer.props, "gridRows", 4.0)
                .round()
                .clamp(1.0, 12.0) as usize;
            for column in 1..columns {
                let x = inner_x + inner_w * column as f32 / columns as f32;
                draw_line_rgba(
                    &mut image,
                    (x, inner_y),
                    (x, inner_y + inner_h),
                    1.0,
                    grid_color,
                );
            }
            for row in 1..rows {
                let y = inner_y + inner_h * row as f32 / rows as f32;
                draw_line_rgba(
                    &mut image,
                    (inner_x, y),
                    (inner_x + inner_w, y),
                    1.0,
                    grid_color,
                );
            }
        }
        let sample_count = value_f32(&layer.props, "sampleCount", 48.0)
            .round()
            .clamp(8.0, 96.0) as usize;
        let sample_interval_ms = value_f32(&layer.props, "sampleIntervalMs", 120.0)
            .round()
            .clamp(40.0, 1000.0) as i32;
        let sample_window_ms = value_f32(&layer.props, "sampleWindowMs", 5000.0)
            .round()
            .clamp(1000.0, 30_000.0) as i32;
        let graph_w = inner_w * value_f32(&layer.props, "horizontalFill", 1.0).clamp(0.5, 1.0);
        let cutoff_time = timestamp_ms.saturating_sub(sample_window_ms);
        let mut samples: Vec<(i32, f32)> = self
            .hud_kps_samples
            .iter()
            .rev()
            .filter(|(sample_time, _)| *sample_time >= cutoff_time)
            .take(sample_count)
            .copied()
            .collect();
        samples.reverse();
        // Add a live point between retained samples so the graph moves every frame.
        if samples.is_empty() {
            samples.push((timestamp_ms, hud_state.total_kps));
        }
        let max = value_f32(&layer.props, "max", 18.0).max(1.0);
        let has_live_point = samples
            .last()
            .is_some_and(|(last_time, _)| timestamp_ms > *last_time);
        let draw_span =
            (samples.len().saturating_sub(1) + usize::from(has_live_point)).max(1) as f32;
        let mut points: Vec<(f32, f32)> = samples
            .iter()
            .enumerate()
            .map(|(index, (_, value))| {
                let phase = (index as f32 / draw_span).clamp(0.0, 1.0);
                let ratio = sparkline_ratio(*value, max, &layer.props);
                (
                    inner_x + phase * graph_w,
                    inner_y + inner_h - ratio * inner_h,
                )
            })
            .collect();
        if has_live_point {
            if let Some((last_time, _)) = samples.last().copied() {
                let live_progress = ((timestamp_ms - last_time) as f32
                    / sample_interval_ms.max(1) as f32)
                    .clamp(0.0, 1.0);
                let live_index =
                    ((samples.len().saturating_sub(1) as f32) + live_progress).min(draw_span);
                let ratio = sparkline_ratio(hud_state.total_kps, max, &layer.props);
                points.push((
                    inner_x + (live_index / draw_span) * graph_w,
                    inner_y + inner_h - ratio * inner_h,
                ));
            }
        }
        let line_color = parse_color_value(
            value_string(&layer.style, "stroke").or_else(|| value_string(&layer.style, "fill")),
            [0.44, 0.90, 1.0, 1.0],
        );
        if props_bool(&layer.props, "fillEnabled", true) && points.len() > 1 {
            let mut fill = parse_color_value(value_string(&layer.style, "fill"), line_color);
            fill[3] *= value_f32(&layer.props, "fillOpacity", 0.14).clamp(0.0, 1.0);
            let fill = color_to_rgba(fill);
            let left = inner_x.floor() as i32;
            let right = (inner_x + inner_w).ceil() as i32;
            let bottom = (inner_y + inner_h).ceil() as i32;
            for x in left..=right {
                if let Some(top_y) = interpolate_polyline_y(&points, x as f32) {
                    for y in top_y.ceil() as i32..=bottom {
                        blend_pixel(&mut image, x, y, fill);
                    }
                }
            }
        }
        let stroke_width = value_f32(&layer.style, "strokeWidth", 2.0).max(1.0);
        for pair in points.windows(2) {
            draw_line_rgba(
                &mut image,
                pair[0],
                pair[1],
                stroke_width,
                color_to_rgba(line_color),
            );
        }
        let texture_id = format!(
            "hud_sparkline_{}_{}x{}",
            sanitize_texture_component(&layer.id),
            width,
            height
        );
        if !self.load_hud_texture_rgba_linear(&texture_id, image.as_raw(), width, height) {
            return;
        }
        planner.add_sprite(media_sprite(texture_id, layer, t.width, t.height, z));
    }
    fn plan_hud_text_layer(
        &mut self,
        planner: &mut SpritePlanner,
        layer: &HudLayerConfig,
        hud_state: &HudFrameState,
        key_mask: u32,
        timestamp_ms: i32,
        z: i32,
    ) {
        let text = if layer.layer_type == "text.bound" {
            layer
                .binding
                .as_deref()
                .map(|binding| format_binding_value(binding, hud_state, key_mask, &layer.props))
                .unwrap_or_else(|| {
                    format_binding_value("score.current", hud_state, key_mask, &layer.props)
                })
        } else {
            value_string(&layer.props, "text")
                .filter(|text| !text.trim().is_empty())
                .unwrap_or("Miru HUD")
                .to_string()
        };
        if text.is_empty() {
            return;
        }
        let font_size = value_f32(
            &layer.style,
            "fontSize",
            value_f32(&layer.props, "fontSize", 32.0),
        );
        let color = parse_color_value(
            value_string(&layer.style, "color").or_else(|| value_string(&layer.props, "color")),
            [0.97, 0.98, 0.99, 1.0],
        );
        self.plan_hud_text(
            planner,
            layer,
            &text,
            font_size,
            color,
            hud_font_weight_number(&layer.style, &layer.props),
            timestamp_ms,
            z,
        );
    }
    fn plan_hud_text(
        &mut self,
        planner: &mut SpritePlanner,
        layer: &HudLayerConfig,
        text: &str,
        font_size: f32,
        color: [f32; 4],
        weight: u16,
        timestamp_ms: i32,
        z: i32,
    ) {
        let font_family = hud_font_family(&layer.style, &layer.props);
        let Some((mut texture_id, mut meta_w, mut meta_h)) = self.ensure_hud_text_texture(
            text,
            font_size.max(8.0),
            to_u8_color(color, layer.transform.opacity),
            weight,
            font_family,
        ) else {
            return;
        };
        let max_w = layer.transform.width.max(1.0);
        let mut draw_h = meta_h.max(1.0);
        let mut natural_w = meta_w.max(1.0);
        let overflow_mode = value_string(&layer.props, "overflowMode").unwrap_or("scroll");
        if overflow_mode == "ellipsis" && natural_w > max_w {
            let ratio = (max_w / natural_w).clamp(0.02, 1.0);
            let chars: Vec<char> = text.chars().collect();
            if chars.len() > 3 {
                let keep = ((chars.len() as f32 * ratio).floor() as usize)
                    .saturating_sub(3)
                    .max(1)
                    .min(chars.len());
                let truncated = format!("{}...", chars.iter().take(keep).collect::<String>());
                if let Some((next_texture_id, next_w, next_h)) = self.ensure_hud_text_texture(
                    &truncated,
                    font_size.max(8.0),
                    to_u8_color(color, layer.transform.opacity),
                    weight,
                    font_family,
                ) {
                    texture_id = next_texture_id;
                    meta_w = next_w;
                    meta_h = next_h;
                    draw_h = meta_h.max(1.0);
                    natural_w = meta_w.max(1.0);
                }
            }
        }
        let draw_y = layer.transform.y + (layer.transform.height - draw_h) * 0.5;
        if overflow_mode == "scroll" && natural_w > max_w {
            let gap = max_w.mul_add(0.18, 0.0).clamp(32.0, 96.0);
            let distance = (natural_w + gap).max(1.0);
            let speed = value_f32(&layer.props, "overflowSpeed", 36.0).clamp(4.0, 260.0);
            let elapsed = timestamp_ms.max(0) as f32 / 1000.0;
            // Scrolling text draws repeated clipped copies so there is no blank gap at wraparound.
            if value_string(&layer.props, "overflowDirection").unwrap_or("left") == "right" {
                let mut draw_x =
                    layer.transform.x + max_w - natural_w + (elapsed * speed) % distance;
                while draw_x > layer.transform.x - natural_w {
                    add_hud_text_sprite_clipped(
                        planner,
                        &texture_id,
                        draw_x,
                        draw_y,
                        natural_w,
                        draw_h,
                        layer.transform.x,
                        layer.transform.y,
                        max_w,
                        layer.transform.height.max(1.0),
                        z,
                        &layer.transform,
                    );
                    draw_x -= distance;
                }
            } else {
                let mut draw_x = layer.transform.x - (elapsed * speed) % distance;
                while draw_x < layer.transform.x + max_w {
                    add_hud_text_sprite_clipped(
                        planner,
                        &texture_id,
                        draw_x,
                        draw_y,
                        natural_w,
                        draw_h,
                        layer.transform.x,
                        layer.transform.y,
                        max_w,
                        layer.transform.height.max(1.0),
                        z,
                        &layer.transform,
                    );
                    draw_x += distance;
                }
            }
            return;
        }
        let target_w = natural_w.min(max_w).max(1.0);
        let align = value_string(&layer.style, "align").unwrap_or("left");
        let draw_x = match align {
            "center" => layer.transform.x + (max_w - target_w) * 0.5,
            "right" => layer.transform.x + (max_w - target_w),
            _ => layer.transform.x,
        };
        add_hud_text_sprite_clipped(
            planner,
            &texture_id,
            draw_x,
            draw_y,
            target_w,
            draw_h,
            layer.transform.x,
            layer.transform.y,
            max_w,
            layer.transform.height.max(1.0),
            z,
            &layer.transform,
        );
    }
    fn plan_hud_judgment_counter(
        &mut self,
        planner: &mut SpritePlanner,
        layer: &HudLayerConfig,
        hud_state: &HudFrameState,
        z: i32,
    ) {
        let t = &layer.transform;
        let panel = HudLayerConfig {
            transform: t.clone(),
            ..layer.clone()
        };
        planner.add_sprite(rect_sprite(&panel, [0.04, 0.06, 0.09, 0.88], z));
        let rail_w = (t.width * 0.025).round().max(2.0);
        for (x, color) in [
            (t.x, [0.22, 0.76, 1.0, 1.0]),
            (t.x + t.width - rail_w, [0.94, 0.71, 0.26, 1.0]),
        ] {
            let rail = HudLayerConfig {
                transform: crate::hud::HudLayerTransformConfig {
                    x,
                    y: t.y,
                    width: rail_w,
                    height: t.height,
                    rotation: 0.0,
                    opacity: t.opacity,
                },
                ..layer.clone()
            };
            planner.add_sprite(rect_sprite(&rail, color, z + 1));
        }
        let header_h = (t.height * 0.16).max(14.0);
        let header = HudLayerConfig {
            transform: crate::hud::HudLayerTransformConfig {
                x: t.x + rail_w,
                y: t.y,
                width: (t.width - rail_w * 2.0).max(1.0),
                height: header_h,
                rotation: 0.0,
                opacity: t.opacity,
            },
            ..layer.clone()
        };
        planner.add_sprite(rect_sprite(&header, [0.08, 0.11, 0.16, 1.0], z + 1));
        self.plan_text_at(
            planner,
            "TELEMETRY JC",
            t.x + rail_w + 8.0,
            t.y + 3.0,
            t.width - rail_w * 2.0 - 16.0,
            header_h - 4.0,
            [0.61, 0.69, 0.78, 1.0],
            z + 2,
            t.opacity,
            FontWeight::Bold,
        );
        let body_y = t.y + header_h;
        let body_h = (t.height - header_h).max(1.0);
        let row_h = (body_h / 6.0).max(8.0);
        for index in 0..6 {
            let y = body_y + row_h * index as f32;
            let row = HudLayerConfig {
                transform: crate::hud::HudLayerTransformConfig {
                    x: t.x + rail_w,
                    y,
                    width: (t.width - rail_w * 2.0).max(1.0),
                    height: if index == 5 {
                        t.y + t.height - y
                    } else {
                        row_h
                    },
                    rotation: 0.0,
                    opacity: t.opacity,
                },
                ..layer.clone()
            };
            let bg = if index % 2 == 0 {
                [0.07, 0.10, 0.15, 1.0]
            } else {
                [0.06, 0.09, 0.13, 1.0]
            };
            planner.add_sprite(rect_sprite(&row, bg, z + 1));
            let label_color = JUDGMENT_COLORS[index];
            self.plan_text_at(
                planner,
                JUDGMENT_LABELS[index],
                row.transform.x + 8.0,
                row.transform.y + row.transform.height * 0.12,
                row.transform.width * 0.42,
                row.transform.height * 0.76,
                label_color,
                z + 2,
                t.opacity,
                FontWeight::Bold,
            );
            self.plan_text_at(
                planner,
                &format!("{:04}", hud_state.judgment_counts[index]),
                row.transform.x + row.transform.width * 0.56,
                row.transform.y + row.transform.height * 0.12,
                row.transform.width * 0.38,
                row.transform.height * 0.76,
                [0.85, 0.89, 0.94, 1.0],
                z + 2,
                t.opacity,
                FontWeight::Bold,
            );
        }
    }
    fn plan_hud_component_judgment_counter(
        &mut self,
        planner: &mut SpritePlanner,
        layer: &HudLayerConfig,
        hud_state: &HudFrameState,
        timestamp_ms: i32,
        z: i32,
    ) {
        let props = &layer.props;
        let t = &layer.transform;
        let visible: Vec<(usize, &str)> = JUDGMENT_KEYS
            .iter()
            .enumerate()
            .filter(|(_, key)| judgment_counter_visible(props, key))
            .map(|(index, key)| (index, *key))
            .collect();
        if visible.is_empty() {
            return;
        }
        let layout = value_string(props, "layoutDirection").unwrap_or("horizontal");
        let is_horizontal = layout == "horizontal";
        let gap = value_f32(props, "gap", 14.0).max(0.0);
        let item_w = value_f32(props, "itemWidth", 58.0).max(8.0);
        let item_h = value_f32(props, "itemHeight", 30.0).max(8.0);
        let total_main = if is_horizontal {
            visible.len() as f32 * item_w + visible.len().saturating_sub(1) as f32 * gap
        } else {
            visible.len() as f32 * item_h + visible.len().saturating_sub(1) as f32 * gap
        };
        let available_main = if is_horizontal { t.width } else { t.height };
        let align = value_string(props, "align").unwrap_or("center");
        let main_offset = match align {
            "start" => 0.0,
            "end" => (available_main - total_main).max(0.0),
            _ => ((available_main - total_main) * 0.5).max(0.0),
        };
        let item_cross_x = if is_horizontal {
            0.0
        } else {
            ((t.width - item_w) * 0.5).max(0.0)
        };
        let item_cross_y = if is_horizontal {
            ((t.height - item_h) * 0.5).max(0.0)
        } else {
            0.0
        };
        let value_pad = props
            .get("valuePad")
            .and_then(Value::as_u64)
            .map(|value| value.min(8) as usize)
            .unwrap_or(4);
        for (visible_index, (judgment_index, judgment_key)) in visible.iter().enumerate() {
            let x = t.x
                + if is_horizontal {
                    main_offset + visible_index as f32 * (item_w + gap)
                } else {
                    item_cross_x
                };
            let y = t.y
                + if is_horizontal {
                    item_cross_y
                } else {
                    main_offset + visible_index as f32 * (item_h + gap)
                };
            let effect = match props_judgment_string(props, judgment_key, "incrementEffect", "pop")
            {
                "none" => "none",
                "slide" => "slide",
                _ => "pop",
            };
            let duration =
                props_judgment_f32(props, judgment_key, "incrementDurationMs", 180.0).max(40.0);
            let value = hud_state.judgment_counts[*judgment_index];
            let anim_key = format!("{}:{}", layer.id, judgment_key);
            // Counter animations are keyed per layer and judgment to preserve independent transitions.
            let anim = match self.hud_judgment_counter_animations.get(&anim_key).copied() {
                Some(previous) if previous.current_value != value => {
                    let next = HudJudgmentCounterAnimation {
                        previous_value: previous.current_value,
                        current_value: value,
                        changed_at_ms: timestamp_ms,
                    };
                    self.hud_judgment_counter_animations
                        .insert(anim_key.clone(), next);
                    next
                }
                Some(previous) => previous,
                None => {
                    let next = HudJudgmentCounterAnimation {
                        previous_value: value,
                        current_value: value,
                        changed_at_ms: timestamp_ms - duration.round() as i32,
                    };
                    self.hud_judgment_counter_animations
                        .insert(anim_key.clone(), next);
                    next
                }
            };
            let changed_age = (timestamp_ms - anim.changed_at_ms).max(0);
            let elapsed = (changed_age as f32 / duration).clamp(0.0, 1.0);
            let active = anim.previous_value != anim.current_value && elapsed < 1.0;
            let pop_amount = if effect == "pop" && active {
                pop_pulse(elapsed)
            } else {
                0.0
            };
            let scale = if pop_amount > 0.0 {
                1.0 + (props_judgment_f32(props, judgment_key, "incrementScale", 1.14).max(1.0)
                    - 1.0)
                    * pop_amount
            } else {
                1.0
            };
            let draw_w = item_w * scale;
            let draw_h = item_h * scale;
            let draw_x = x - (draw_w - item_w) * 0.5;
            let draw_y = y
                - (draw_h - item_h) * 0.5
                - if pop_amount > 0.0 {
                    pop_amount * 1.5
                } else {
                    0.0
                };
            let previous = if active {
                anim.previous_value
            } else {
                value.saturating_sub(1)
            };
            let text_width = value_pad
                .max(value.to_string().len())
                .max(previous.to_string().len());
            let value_text = format!("{value:0>width$}", width = text_width);
            let font_size =
                props_judgment_f32(props, judgment_key, "fontSize", 24.0).max(8.0) * scale;
            let font_weight = normalize_font_weight_number(props_judgment_f32(
                props,
                judgment_key,
                "fontWeight",
                700.0,
            ));
            let font_family = props_judgment_value(props, judgment_key, "fontFamily")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty());
            let text_h = font_size.min(draw_h).max(8.0);
            let base_text_color = parse_css_color(
                props_judgment_string(props, judgment_key, "textColor", "#f5f8ff"),
                [0.96, 0.97, 1.0, 1.0],
            );
            if effect == "slide" && active {
                let slide_progress = smoothstep(elapsed);
                let previous_text = format!("{previous:0>width$}", width = text_width);
                let current_digits: Vec<char> = value_text.chars().collect();
                let mut previous_digits: Vec<char> = previous_text.chars().collect();
                if previous_digits.len() < current_digits.len() {
                    let mut padded = vec!['0'; current_digits.len() - previous_digits.len()];
                    padded.extend(previous_digits);
                    previous_digits = padded;
                }
                let digit_widths_raw = current_digits
                    .iter()
                    .enumerate()
                    .map(|(digit_index, current_digit)| {
                        let previous_digit = previous_digits
                            .get(digit_index)
                            .copied()
                            .unwrap_or(*current_digit);
                        self.hud_text_natural_width(
                            &current_digit.to_string(),
                            text_h,
                            base_text_color,
                            t.opacity,
                            font_weight,
                            font_family,
                        )
                        .max(self.hud_text_natural_width(
                            &previous_digit.to_string(),
                            text_h,
                            base_text_color,
                            t.opacity,
                            font_weight,
                            font_family,
                        ))
                        .max(1.0)
                    })
                    .collect::<Vec<_>>();
                let natural_total = digit_widths_raw.iter().sum::<f32>().max(1.0);
                let max_width = (item_w - 4.0).max(1.0);
                let width_scale = if natural_total > max_width {
                    max_width / natural_total
                } else {
                    1.0
                };
                let digit_widths = digit_widths_raw
                    .iter()
                    .map(|width| (width * width_scale).max(1.0))
                    .collect::<Vec<_>>();
                let total_width = digit_widths.iter().sum::<f32>();
                let mut cursor_x = x + (item_w - total_width) * 0.5;
                let center_y = y + item_h * 0.5;
                for (digit_index, current_digit) in current_digits.iter().enumerate() {
                    let digit_w = digit_widths.get(digit_index).copied().unwrap_or(1.0);
                    let digit_center_x = cursor_x + digit_w * 0.5;
                    let previous_digit = previous_digits
                        .get(digit_index)
                        .copied()
                        .unwrap_or(*current_digit);
                    if previous_digit == *current_digit {
                        self.plan_text_centered_clipped_on_point(
                            planner,
                            &current_digit.to_string(),
                            digit_center_x,
                            center_y,
                            digit_w + 1.0,
                            x,
                            y,
                            item_w,
                            item_h,
                            base_text_color,
                            z + 4,
                            t.opacity,
                            font_weight,
                            font_family,
                            t,
                            text_h,
                        );
                    } else {
                        self.plan_text_centered_clipped_on_point(
                            planner,
                            &previous_digit.to_string(),
                            digit_center_x,
                            center_y - item_h * slide_progress,
                            digit_w + 1.0,
                            x,
                            y,
                            item_w,
                            item_h,
                            base_text_color,
                            z + 4,
                            t.opacity,
                            font_weight,
                            font_family,
                            t,
                            text_h,
                        );
                        self.plan_text_centered_clipped_on_point(
                            planner,
                            &current_digit.to_string(),
                            digit_center_x,
                            center_y + item_h * (1.0 - slide_progress),
                            digit_w + 1.0,
                            x,
                            y,
                            item_w,
                            item_h,
                            base_text_color,
                            z + 4,
                            t.opacity,
                            font_weight,
                            font_family,
                            t,
                            text_h,
                        );
                    }
                    cursor_x += digit_w;
                }
                continue;
            }
            self.plan_text_centered_at(
                planner,
                &value_text,
                draw_x,
                draw_y + (draw_h - text_h).max(0.0) * 0.5,
                draw_w,
                text_h,
                base_text_color,
                z + 4,
                t.opacity,
                font_weight,
                font_family,
                Some(t),
            );
        }
    }
    fn plan_hud_component_key_counter(
        &mut self,
        planner: &mut SpritePlanner,
        layer: &HudLayerConfig,
        hud_state: &HudFrameState,
        key_mask: u32,
        timestamp_ms: i32,
        z: i32,
    ) {
        let props = &layer.props;
        let t = &layer.transform;
        let key_count = value_f32(props, "keyCount", 4.0).round().clamp(1.0, 9.0) as usize;
        let key_size = value_f32(props, "keySize", 54.0).max(12.0);
        let gap = value_f32(props, "gap", 8.0).max(0.0);
        let total_width = key_count as f32 * key_size + key_count.saturating_sub(1) as f32 * gap;
        let start_x = t.x + (t.width - total_width) * 0.5;
        let base_tail_direction = value_string(props, "tailDirection").unwrap_or("up");
        let key_y = if base_tail_direction == "down" {
            t.y
        } else {
            t.y + t.height - key_size
        };
        for index in 0..key_count {
            let key_x = start_x + index as f32 * (key_size + gap);
            let key_bit = 1_u32 << index;
            let down = key_mask & key_bit != 0;
            let tail_enabled = props_key_bool(props, index, "tailEnabled", true);
            let tail_direction =
                props_key_string(props, index, "tailDirection", base_tail_direction);
            let tail_max_height = props_key_f32(props, index, "tailMaxHeight", 210.0).max(0.0);
            let tail_max_duration =
                props_key_f32(props, index, "tailMaxDurationMs", 700.0).max(1.0);
            let tail_release_speed =
                key_counter_tail_speed_px_per_ms(props, index, tail_max_height, tail_max_duration);
            let release_boundary_travel = tail_max_height;
            if tail_enabled && tail_max_height > 0.0 {
                let tail_color = parse_css_color(
                    props_key_string(props, index, "tailColor", "#9ca3af"),
                    [0.61, 0.64, 0.69, 1.0],
                );
                let tail_opacity = props_key_f32(props, index, "tailOpacity", 0.5).clamp(0.0, 1.0);
                let release_lifetime_ms =
                    (release_boundary_travel / tail_release_speed.max(0.001)).round() as i32;
                // Released tails keep moving after key-up using the press duration captured in HudFrameState.
                let releases = self
                    .hud_key_tail_releases
                    .iter()
                    .copied()
                    .filter(|release| release.key_index == index)
                    .collect::<Vec<_>>();
                for release in releases {
                    let age = timestamp_ms.saturating_sub(release.released_at_ms);
                    if age < 0 || age > release_lifetime_ms {
                        continue;
                    }
                    let tail_height =
                        (release.duration_ms.max(0) as f32 * tail_release_speed).max(0.0);
                    if tail_height <= 0.5 {
                        continue;
                    }
                    let release_travel = age as f32 * tail_release_speed;
                    let raw_y = if tail_direction == "down" {
                        key_y + key_size + release_travel
                    } else {
                        key_y - tail_height - release_travel
                    };
                    let raw_bottom = raw_y + tail_height;
                    let (visible_y, visible_bottom) = if tail_direction == "down" {
                        (
                            raw_y,
                            raw_bottom.min(key_y + key_size + release_boundary_travel),
                        )
                    } else {
                        ((raw_y).max(key_y - release_boundary_travel), raw_bottom)
                    };
                    let visible_height = visible_bottom - visible_y;
                    if visible_height <= 0.5 {
                        continue;
                    }
                    let tail_layer = HudLayerConfig {
                        transform: crate::hud::HudLayerTransformConfig {
                            x: key_x,
                            y: visible_y,
                            width: key_size,
                            height: visible_height,
                            rotation: 0.0,
                            opacity: t.opacity,
                        },
                        ..layer.clone()
                    };
                    let consumed = visible_height < tail_height - 0.5;
                    if consumed {
                        let fade_from_start = tail_direction != "down";
                        if let Some(texture_id) = self.ensure_hud_tail_texture(
                            visible_height.round().max(1.0) as u32,
                            tail_color,
                            tail_opacity,
                            fade_from_start,
                        ) {
                            add_sprite_around_transform(
                                planner,
                                media_sprite(texture_id, &tail_layer, key_size, visible_height, z),
                                t,
                            );
                        }
                    } else {
                        add_sprite_around_transform(
                            planner,
                            rect_sprite(&tail_layer, with_opacity(tail_color, tail_opacity), z),
                            t,
                        );
                    }
                }
                if down {
                    let tail_height = (hud_state.key_press_duration_ms[index].max(0) as f32
                        * tail_release_speed)
                        .max(0.0);
                    if tail_height > 0.5 {
                        let raw_y = if tail_direction == "down" {
                            key_y + key_size
                        } else {
                            key_y - tail_height
                        };
                        let raw_bottom = raw_y + tail_height;
                        let (visible_y, visible_bottom) = if tail_direction == "down" {
                            (
                                raw_y,
                                raw_bottom.min(key_y + key_size + release_boundary_travel),
                            )
                        } else {
                            ((raw_y).max(key_y - release_boundary_travel), raw_bottom)
                        };
                        let visible_height = visible_bottom - visible_y;
                        if visible_height <= 0.5 {
                            continue;
                        }
                        let tail_layer = HudLayerConfig {
                            transform: crate::hud::HudLayerTransformConfig {
                                x: key_x,
                                y: visible_y,
                                width: key_size,
                                height: visible_height,
                                rotation: 0.0,
                                opacity: t.opacity,
                            },
                            ..layer.clone()
                        };
                        let consumed = visible_height < tail_height - 0.5;
                        if consumed {
                            let fade_from_start = tail_direction != "down";
                            if let Some(texture_id) = self.ensure_hud_tail_texture(
                                visible_height.round().max(1.0) as u32,
                                tail_color,
                                tail_opacity,
                                fade_from_start,
                            ) {
                                add_sprite_around_transform(
                                    planner,
                                    media_sprite(
                                        texture_id,
                                        &tail_layer,
                                        key_size,
                                        visible_height,
                                        z,
                                    ),
                                    t,
                                );
                            }
                        } else {
                            add_sprite_around_transform(
                                planner,
                                rect_sprite(&tail_layer, with_opacity(tail_color, tail_opacity), z),
                                t,
                            );
                        }
                    }
                }
            }
            let key_layer = HudLayerConfig {
                transform: crate::hud::HudLayerTransformConfig {
                    x: key_x,
                    y: key_y,
                    width: key_size,
                    height: key_size,
                    rotation: 0.0,
                    opacity: t.opacity,
                },
                ..layer.clone()
            };
            let radius = props_key_f32(props, index, "radius", 2.0).max(0.0);
            let fill_mode = props_key_string(props, index, "fillMode", "transparent");
            if fill_mode == "solid" {
                let fill_color = parse_css_color(
                    props_key_string(props, index, "fillColor", "#111820"),
                    [0.07, 0.09, 0.13, 1.0],
                );
                let fill_opacity = props_key_f32(props, index, "fillOpacity", 0.7).clamp(0.0, 1.0);
                if let Some(texture_id) = self.ensure_hud_rect_texture(
                    key_size.round().max(1.0) as u32,
                    key_size.round().max(1.0) as u32,
                    with_opacity(fill_color, fill_opacity),
                    [0.0, 0.0, 0.0, 0.0],
                    0.0,
                    radius,
                ) {
                    add_sprite_around_transform(
                        planner,
                        media_sprite(texture_id, &key_layer, key_size, key_size, z + 1),
                        t,
                    );
                }
            }
            let press_effect = props_key_string(props, index, "pressEffect", "fillGlow");
            let press_color = parse_css_color(
                props_key_string(props, index, "pressColor", "#ffffff"),
                [1.0, 1.0, 1.0, 1.0],
            );
            let press_opacity = props_key_f32(props, index, "pressOpacity", 0.28).clamp(0.0, 1.0);
            if down && press_effect == "fillGlow" {
                if let Some(texture_id) = self.ensure_hud_rect_texture(
                    key_size.round().max(1.0) as u32,
                    key_size.round().max(1.0) as u32,
                    with_opacity(press_color, press_opacity),
                    [0.0, 0.0, 0.0, 0.0],
                    0.0,
                    radius,
                ) {
                    add_sprite_around_transform(
                        planner,
                        media_sprite(texture_id, &key_layer, key_size, key_size, z + 2),
                        t,
                    );
                }
            }
            let border_width = props_key_f32(props, index, "borderWidth", 2.0).max(0.0);
            if border_width > 0.0 {
                let border_color = if down && press_effect == "borderGlow" {
                    with_opacity(press_color, press_opacity)
                } else {
                    parse_css_color(
                        props_key_string(props, index, "borderColor", "#ffffff"),
                        [1.0, 1.0, 1.0, 1.0],
                    )
                };
                if down && press_effect == "borderGlow" {
                    let glow_layer = HudLayerConfig {
                        transform: crate::hud::HudLayerTransformConfig {
                            x: key_x - border_width,
                            y: key_y - border_width,
                            width: key_size + border_width * 2.0,
                            height: key_size + border_width * 2.0,
                            rotation: 0.0,
                            opacity: t.opacity,
                        },
                        ..key_layer.clone()
                    };
                    if let Some(texture_id) = self.ensure_hud_rect_texture(
                        glow_layer.transform.width.round().max(1.0) as u32,
                        glow_layer.transform.height.round().max(1.0) as u32,
                        [0.0, 0.0, 0.0, 0.0],
                        with_opacity(press_color, press_opacity * 0.28),
                        border_width * 1.5,
                        radius + border_width,
                    ) {
                        add_sprite_around_transform(
                            planner,
                            media_sprite(
                                texture_id,
                                &glow_layer,
                                glow_layer.transform.width,
                                glow_layer.transform.height,
                                z + 2,
                            ),
                            t,
                        );
                    }
                }
                if let Some(texture_id) = self.ensure_hud_rect_texture(
                    key_size.round().max(1.0) as u32,
                    key_size.round().max(1.0) as u32,
                    [0.0, 0.0, 0.0, 0.0],
                    border_color,
                    border_width,
                    radius,
                ) {
                    add_sprite_around_transform(
                        planner,
                        media_sprite(texture_id, &key_layer, key_size, key_size, z + 3),
                        t,
                    );
                }
            }
            if props_key_bool(props, index, "showText", true) {
                let label_mode = props_key_string(props, index, "labelMode", "fixed");
                let text = if label_mode == "kps" {
                    format!("{:.0}", hud_state.key_kps[index])
                } else {
                    key_counter_label(props, index)
                };
                if !text.trim().is_empty() {
                    let font_size = props_key_f32(props, index, "fontSize", 18.0).max(8.0);
                    let text_h = font_size.min(key_size).max(8.0);
                    let text_color = parse_css_color(
                        props_key_string(props, index, "textColor", "#ffffff"),
                        [1.0, 1.0, 1.0, 1.0],
                    );
                    self.plan_text_centered_at(
                        planner,
                        &text,
                        key_x,
                        key_y + (key_size - text_h) * 0.5,
                        key_size,
                        text_h,
                        text_color,
                        z + 4,
                        t.opacity,
                        normalize_font_weight_number(props_key_f32(
                            props,
                            index,
                            "fontWeight",
                            800.0,
                        )),
                        props_key_value(props, index, "fontFamily")
                            .and_then(Value::as_str)
                            .filter(|value| !value.trim().is_empty()),
                        Some(t),
                    );
                }
            }
        }
    }
    fn plan_hud_key_press(
        &mut self,
        planner: &mut SpritePlanner,
        layer: &HudLayerConfig,
        hud_state: &HudFrameState,
        key_mask: u32,
        z: i32,
    ) {
        let keys = props_keys(&layer.props);
        let t = &layer.transform;
        let gap = (t.width * 0.018).max(4.0);
        let key_w =
            ((t.width - gap * (keys.len().saturating_sub(1)) as f32) / keys.len() as f32).max(8.0);
        let accent = parse_color_value(
            value_string(&layer.style, "accent").or_else(|| value_string(&layer.props, "accent")),
            [0.22, 0.76, 1.0, 1.0],
        );
        let show_kps = props_bool(&layer.props, "showKps", true);
        let show_trails = props_bool(&layer.props, "showTrails", true);
        for (index, key) in keys.iter().enumerate() {
            let key_idx = key_index(key);
            let down = key_idx
                .map(|idx| (key_mask & (1_u32 << idx)) != 0)
                .unwrap_or(false);
            let key_layer = HudLayerConfig {
                transform: crate::hud::HudLayerTransformConfig {
                    x: t.x + index as f32 * (key_w + gap),
                    y: t.y,
                    width: key_w,
                    height: t.height,
                    rotation: 0.0,
                    opacity: t.opacity,
                },
                ..layer.clone()
            };
            planner.add_sprite(rect_sprite(
                &key_layer,
                if down {
                    accent
                } else {
                    [0.05, 0.07, 0.11, 0.82]
                },
                z,
            ));
            if show_trails {
                if let Some(idx) = key_idx {
                    let duration = hud_state.key_press_duration_ms[idx as usize].max(0) as f32;
                    if duration > 0.0 {
                        let ratio = (duration / 700.0).clamp(0.05, 1.0);
                        let trail = HudLayerConfig {
                            transform: crate::hud::HudLayerTransformConfig {
                                x: key_layer.transform.x,
                                y: key_layer.transform.y - 5.0,
                                width: key_layer.transform.width * ratio,
                                height: 3.0,
                                rotation: 0.0,
                                opacity: (1.0 - (duration / 900.0).clamp(0.0, 0.75)) * t.opacity,
                            },
                            ..layer.clone()
                        };
                        planner.add_sprite(rect_sprite(&trail, accent, z + 1));
                    }
                }
            }
            self.plan_text_at(
                planner,
                &key.to_ascii_uppercase(),
                key_layer.transform.x,
                key_layer.transform.y
                    + key_layer.transform.height * if show_kps { 0.14 } else { 0.22 },
                key_layer.transform.width,
                key_layer.transform.height * if show_kps { 0.44 } else { 0.56 },
                if down {
                    [0.02, 0.07, 0.10, 1.0]
                } else {
                    [0.86, 0.92, 1.0, 1.0]
                },
                z + 1,
                t.opacity,
                FontWeight::Bold,
            );
            if show_kps {
                let kps = key_idx
                    .map(|idx| hud_state.key_kps[idx as usize])
                    .unwrap_or(0.0);
                self.plan_text_at(
                    planner,
                    &format!("{:.0} kps", kps),
                    key_layer.transform.x,
                    key_layer.transform.y + key_layer.transform.height * 0.62,
                    key_layer.transform.width,
                    key_layer.transform.height * 0.25,
                    if down {
                        [0.02, 0.07, 0.10, 0.92]
                    } else {
                        [0.58, 0.66, 0.76, 0.92]
                    },
                    z + 1,
                    t.opacity,
                    FontWeight::Normal,
                );
            }
        }
    }
    fn plan_hud_badge(
        &mut self,
        planner: &mut SpritePlanner,
        layer: &HudLayerConfig,
        label: &str,
        value: &str,
        z: i32,
    ) {
        planner.add_sprite(rect_sprite(layer, [0.05, 0.07, 0.11, 0.84], z));
        let accent = parse_color_value(
            value_string(&layer.style, "accent").or_else(|| value_string(&layer.props, "accent")),
            [0.65, 0.95, 0.82, 1.0],
        );
        self.plan_text_at(
            planner,
            label,
            layer.transform.x + 10.0,
            layer.transform.y + layer.transform.height * 0.2,
            layer.transform.width * 0.42,
            layer.transform.height * 0.58,
            accent,
            z + 1,
            layer.transform.opacity,
            FontWeight::Bold,
        );
        self.plan_text_at(
            planner,
            value,
            layer.transform.x + layer.transform.width * 0.56,
            layer.transform.y + layer.transform.height * 0.18,
            layer.transform.width * 0.34,
            layer.transform.height * 0.62,
            [0.97, 0.98, 0.99, 1.0],
            z + 1,
            layer.transform.opacity,
            FontWeight::Bold,
        );
    }
    fn plan_hud_kps_graph(&mut self, planner: &mut SpritePlanner, layer: &HudLayerConfig, z: i32) {
        planner.add_sprite(rect_sprite(layer, [0.05, 0.07, 0.11, 0.84], z));
        let accent = parse_color_value(
            value_string(&layer.style, "accent").or_else(|| value_string(&layer.props, "accent")),
            [0.94, 0.71, 0.26, 1.0],
        );
        let bars = 12;
        let gap = 3.0;
        let bar_w = ((layer.transform.width - 20.0) / bars as f32 - gap).max(2.0);
        for index in 0..bars {
            let phase = index as f32 / (bars - 1) as f32;
            let h = (layer.transform.height - 20.0)
                * (0.24 + (phase * std::f32::consts::PI * 2.0).sin().abs() * 0.58);
            let bar = HudLayerConfig {
                transform: crate::hud::HudLayerTransformConfig {
                    x: layer.transform.x + 10.0 + index as f32 * (bar_w + gap),
                    y: layer.transform.y + layer.transform.height - 10.0 - h,
                    width: bar_w,
                    height: h.max(1.0),
                    rotation: 0.0,
                    opacity: layer.transform.opacity,
                },
                ..layer.clone()
            };
            planner.add_sprite(rect_sprite(&bar, accent, z + 1));
        }
    }
    fn plan_hud_hit_error_meter_horizontal(
        &mut self,
        planner: &mut SpritePlanner,
        layer: &HudLayerConfig,
        hud_state: &HudFrameState,
        z: i32,
    ) {
        let t = &layer.transform;
        let width = t.width.round().max(1.0) as u32;
        let height = t.height.round().max(1.0) as u32;
        let mut image = RgbaImage::new(width, height);
        let padding = value_f32(&layer.props, "padding", 3.0).max(0.0);
        let bar_left = padding.min(width as f32 * 0.45);
        let bar_right = (width as f32 - padding).max(bar_left + 1.0);
        let bar_w = (bar_right - bar_left).max(1.0);
        let center_x = bar_left + bar_w * 0.5;
        let axis_h = value_f32(&layer.props, "axisHeight", height as f32 - padding * 2.0)
            .max(2.0)
            .min((height as f32 - padding * 2.0).max(2.0));
        let center_y = height as f32 * 0.5;
        let bar_top = center_y - axis_h * 0.5;
        let bar_bottom = center_y + axis_h * 0.5;
        let color_h = value_f32(&layer.props, "colorBarHeight", axis_h * 0.35)
            .max(1.0)
            .min(axis_h);
        let windows = hud_state.hit_error_windows;
        let max_window = value_f32(
            &layer.props,
            "maxHitWindow",
            windows.map(|w| w.hit50).unwrap_or(150) as f32,
        )
        .max(1.0);
        let x_for_offset = |offset_ms: f32| -> f32 {
            let ratio = ((offset_ms / max_window) + 1.0) * 0.5;
            bar_left + ratio.clamp(0.0, 1.0) * bar_w
        };

        fill_rect_rgba(
            &mut image,
            bar_left,
            center_y - color_h * 0.5,
            bar_w,
            color_h,
            color_to_rgba([0.04, 0.05, 0.07, 0.42]),
        );

        let show_color_bars = layer
            .props
            .get("colourBarVisibility")
            .and_then(Value::as_bool)
            .or_else(|| {
                layer
                    .props
                    .get("colorBarVisibility")
                    .and_then(Value::as_bool)
            })
            .unwrap_or(true);
        if show_color_bars {
            let fallback_windows = HitErrorWindows {
                max: (max_window * 0.13).round() as i32,
                hit300: (max_window * 0.32).round() as i32,
                hit200: (max_window * 0.54).round() as i32,
                hit100: (max_window * 0.74).round() as i32,
                hit50: max_window.round() as i32,
            };
            let w = windows.unwrap_or(fallback_windows);
            let bands = [
                (w.hit50, JUDGMENT_COLORS[4]),
                (w.hit100, JUDGMENT_COLORS[3]),
                (w.hit200, JUDGMENT_COLORS[2]),
                (w.hit300, JUDGMENT_COLORS[1]),
                (w.max, JUDGMENT_COLORS[0]),
            ];
            for (window, mut color) in bands {
                let ratio = (window.max(1) as f32 / max_window).clamp(0.0, 1.0);
                let band_w = bar_w * 0.5 * ratio;
                color[3] *= 0.88;
                fill_rect_rgba(
                    &mut image,
                    center_x - band_w,
                    center_y - color_h * 0.5,
                    band_w * 2.0,
                    color_h,
                    color_to_rgba(color),
                );
            }
        }

        let marker_style = value_string(&layer.props, "centreMarkerStyle")
            .or_else(|| value_string(&layer.props, "centerMarkerStyle"))
            .unwrap_or("line");
        if marker_style != "none" {
            let marker_size = value_f32(&layer.props, "centreMarkerSize", axis_h).max(3.0);
            let marker_color = parse_color_value(
                value_string(&layer.style, "centreMarkerColor")
                    .or_else(|| value_string(&layer.props, "centreMarkerColor")),
                JUDGMENT_COLORS[0],
            );
            match marker_style {
                "circle" => {
                    fill_circle_rgba(
                        &mut image,
                        center_x,
                        center_y,
                        marker_size * 0.5,
                        color_to_rgba(marker_color),
                    );
                    fill_circle_rgba(
                        &mut image,
                        center_x,
                        center_y,
                        marker_size * 0.24,
                        color_to_rgba(darken_color(marker_color, 0.35)),
                    );
                }
                _ => {
                    let line_w = (marker_size / 4.0).max(1.0);
                    draw_line_rgba(
                        &mut image,
                        (center_x, bar_top),
                        (center_x, bar_bottom),
                        line_w,
                        color_to_rgba(with_opacity(marker_color, 0.9)),
                    );
                }
            }
        }

        let line_thickness = value_f32(&layer.props, "judgementLineThickness", 4.0)
            .max(1.0)
            .min(axis_h);
        let fade_ms = value_f32(&layer.props, "judgementFadeMs", 5000.0).max(250.0);
        for judgment in &hud_state.hit_error_judgments {
            let age = judgment.age_ms.max(0) as f32;
            let fade_in = (age / 100.0).clamp(0.0, 1.0);
            let fade_out = (1.0 - age / fade_ms).clamp(0.0, 1.0);
            let alpha = 0.72 * fade_in.min(fade_out);
            if alpha <= 0.0 {
                continue;
            }
            let x = x_for_offset(judgment.offset_ms as f32);
            let color = with_opacity(JUDGMENT_COLORS[judgment_kind_index(judgment.kind)], alpha);
            draw_line_rgba(
                &mut image,
                (x, bar_top),
                (x, bar_bottom),
                line_thickness,
                color_to_rgba(color),
            );
        }

        if props_bool(&layer.props, "showMovingAverage", true) {
            let average_x = x_for_offset(hud_state.hit_error_moving_avg_ms.unwrap_or(0.0));
            let color = parse_color_value(
                value_string(&layer.style, "movingAverageColor")
                    .or_else(|| value_string(&layer.props, "movingAverageColor")),
                [0.96, 0.98, 1.0, 1.0],
            );
            draw_line_rgba(
                &mut image,
                (average_x, bar_top - 1.0),
                (average_x, bar_bottom + 1.0),
                2.0,
                color_to_rgba(with_opacity(color, 0.92)),
            );
        }

        let texture_id = format!(
            "hud_hit_error_{}_{}x{}",
            sanitize_texture_component(&layer.id),
            width,
            height
        );
        if !self.load_hud_texture_rgba_linear(&texture_id, image.as_raw(), width, height) {
            return;
        }
        planner.add_sprite(media_sprite(texture_id, layer, t.width, t.height, z));
    }
    fn plan_hud_hit_error_meter(
        &mut self,
        planner: &mut SpritePlanner,
        layer: &HudLayerConfig,
        hud_state: &HudFrameState,
        z: i32,
    ) {
        if value_string(&layer.props, "orientation")
            .is_some_and(|orientation| orientation.eq_ignore_ascii_case("horizontal"))
        {
            self.plan_hud_hit_error_meter_horizontal(planner, layer, hud_state, z);
            return;
        }
        let t = &layer.transform;
        let width = t.width.round().max(1.0) as u32;
        let height = t.height.round().max(1.0) as u32;
        let mut image = RgbaImage::new(width, height);
        let label_h = 0.0;
        let padding = value_f32(&layer.props, "padding", 4.0).max(0.0);
        let bar_top = (padding + label_h).min(height as f32 * 0.45);
        let bar_bottom = (height as f32 - padding - label_h).max(bar_top + 1.0);
        let bar_h = (bar_bottom - bar_top).max(1.0);
        let center_y = bar_top + bar_h * 0.5;
        let half_h = (bar_h * 0.5).max(1.0);
        let axis_w = value_f32(&layer.props, "axisWidth", 14.0)
            .max(4.0)
            .min(width as f32);
        let color_w = value_f32(&layer.props, "colorBarWidth", 2.0)
            .max(1.0)
            .min(axis_w);
        let chevron_size = value_f32(&layer.props, "chevronSize", 8.0).max(4.0);
        let axis_center_x =
            (padding + chevron_size + axis_w * 0.5 + 2.0).min(width as f32 - axis_w * 0.5);
        let axis_left = axis_center_x - axis_w * 0.5;
        let color_left = axis_center_x - color_w * 0.5;
        let windows = hud_state.hit_error_windows;
        let max_window = value_f32(
            &layer.props,
            "maxHitWindow",
            windows.map(|w| w.hit50).unwrap_or(150) as f32,
        )
        .max(1.0);
        let y_for_offset = |offset_ms: f32| -> f32 {
            let ratio = ((offset_ms / max_window) + 1.0) * 0.5;
            bar_top + ratio.clamp(0.0, 1.0) * bar_h
        };
        let show_color_bars = layer
            .props
            .get("colourBarVisibility")
            .and_then(Value::as_bool)
            .or_else(|| {
                layer
                    .props
                    .get("colorBarVisibility")
                    .and_then(Value::as_bool)
            })
            .unwrap_or(true);
        if show_color_bars {
            let fallback_windows = HitErrorWindows {
                max: (max_window * 0.13).round() as i32,
                hit300: (max_window * 0.32).round() as i32,
                hit200: (max_window * 0.54).round() as i32,
                hit100: (max_window * 0.74).round() as i32,
                hit50: max_window.round() as i32,
            };
            let w = windows.unwrap_or(fallback_windows);
            let bands = [
                (w.hit50, JUDGMENT_COLORS[4]),
                (w.hit100, JUDGMENT_COLORS[3]),
                (w.hit200, JUDGMENT_COLORS[2]),
                (w.hit300, JUDGMENT_COLORS[1]),
                (w.max, JUDGMENT_COLORS[0]),
            ];
            for (window, mut color) in bands {
                let ratio = (window.max(1) as f32 / max_window).clamp(0.0, 1.0);
                let band_h = half_h * ratio;
                color[3] *= 0.88;
                fill_rect_rgba(
                    &mut image,
                    color_left,
                    center_y - band_h,
                    color_w,
                    band_h * 2.0,
                    color_to_rgba(color),
                );
            }
        }
        let marker_style = value_string(&layer.props, "centreMarkerStyle")
            .or_else(|| value_string(&layer.props, "centerMarkerStyle"))
            .unwrap_or("circle");
        if marker_style != "none" {
            let marker_size = value_f32(&layer.props, "centreMarkerSize", 8.0).max(3.0);
            let marker_color = parse_color_value(
                value_string(&layer.style, "centreMarkerColor")
                    .or_else(|| value_string(&layer.props, "centreMarkerColor")),
                JUDGMENT_COLORS[0],
            );
            match marker_style {
                "line" => {
                    let line_h = (marker_size / 3.0).max(1.0);
                    fill_rect_rgba(
                        &mut image,
                        axis_left,
                        center_y - line_h * 0.5,
                        axis_w,
                        line_h,
                        color_to_rgba(marker_color),
                    );
                    fill_rect_rgba(
                        &mut image,
                        axis_left + 1.0,
                        center_y - line_h * 0.5 + 1.0,
                        (axis_w - 2.0).max(1.0),
                        (line_h - 2.0).max(1.0),
                        color_to_rgba(darken_color(marker_color, 0.3)),
                    );
                }
                _ => {
                    fill_circle_rgba(
                        &mut image,
                        axis_center_x,
                        center_y,
                        marker_size * 0.5,
                        color_to_rgba(marker_color),
                    );
                    fill_circle_rgba(
                        &mut image,
                        axis_center_x,
                        center_y,
                        marker_size * 0.25,
                        color_to_rgba(darken_color(marker_color, 0.3)),
                    );
                }
            }
        }
        let line_thickness = value_f32(&layer.props, "judgementLineThickness", 4.0)
            .max(1.0)
            .min(axis_w);
        let fade_ms = value_f32(&layer.props, "judgementFadeMs", 5000.0).max(250.0);
        for judgment in &hud_state.hit_error_judgments {
            let age = judgment.age_ms.max(0) as f32;
            let fade_in = (age / 100.0).clamp(0.0, 1.0);
            let fade_out = (1.0 - age / fade_ms).clamp(0.0, 1.0);
            let alpha = 0.6 * fade_in.min(fade_out);
            if alpha <= 0.0 {
                continue;
            }
            let y = y_for_offset(judgment.offset_ms as f32);
            let color = with_opacity(JUDGMENT_COLORS[judgment_kind_index(judgment.kind)], alpha);
            draw_line_rgba(
                &mut image,
                (axis_left, y),
                (axis_left + axis_w, y),
                line_thickness,
                color_to_rgba(color),
            );
        }
        if props_bool(&layer.props, "showMovingAverage", true) {
            let average_y = y_for_offset(hud_state.hit_error_moving_avg_ms.unwrap_or(0.0));
            let color = parse_color_value(
                value_string(&layer.style, "movingAverageColor")
                    .or_else(|| value_string(&layer.props, "movingAverageColor")),
                [0.96, 0.98, 1.0, 1.0],
            );
            fill_right_triangle_rgba(
                &mut image,
                axis_left - 2.0,
                average_y,
                chevron_size,
                chevron_size,
                color_to_rgba(with_opacity(color, 0.92)),
            );
        }
        let texture_id = format!(
            "hud_hit_error_{}_{}x{}",
            sanitize_texture_component(&layer.id),
            width,
            height
        );
        if !self.load_hud_texture_rgba_linear(&texture_id, image.as_raw(), width, height) {
            return;
        }
        planner.add_sprite(media_sprite(texture_id, layer, t.width, t.height, z));
    }
    fn plan_text_at(
        &mut self,
        planner: &mut SpritePlanner,
        text: &str,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [f32; 4],
        z: i32,
        opacity: f32,
        weight: FontWeight,
    ) {
        let layer = HudLayerConfig {
            transform: crate::hud::HudLayerTransformConfig {
                x,
                y,
                width: width.max(1.0),
                height: height.max(1.0),
                rotation: 0.0,
                opacity,
            },
            ..Default::default()
        };
        self.plan_hud_text(
            planner,
            &layer,
            text,
            height.max(8.0),
            color,
            font_weight_number(weight),
            0,
            z,
        );
    }
    fn plan_text_centered_at(
        &mut self,
        planner: &mut SpritePlanner,
        text: &str,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [f32; 4],
        z: i32,
        opacity: f32,
        weight: u16,
        family: Option<&str>,
        rotation_transform: Option<&HudLayerTransformConfig>,
    ) {
        let height = height.max(8.0);
        let Some((texture_id, meta_w, meta_h)) =
            self.ensure_hud_text_texture(text, height, to_u8_color(color, opacity), weight, family)
        else {
            return;
        };
        let natural_w = meta_w.max(1.0);
        let draw_h = meta_h.max(1.0);
        let target_w = natural_w.min(width.max(1.0)).max(1.0);
        let draw_x = x + (width.max(1.0) - target_w) * 0.5;
        let draw_y = y + (height - draw_h) * 0.5;
        let mut cmd = SpriteCommand::new(
            texture_id,
            draw_x.round() as i32,
            draw_y.round() as i32,
            target_w.round().max(1.0) as u32,
            draw_h.round().max(1.0) as u32,
        )
        .with_z(z);
        cmd.precise_position = Some([draw_x, draw_y]);
        cmd.precise_size = Some([target_w, draw_h]);
        if let Some(transform) = rotation_transform {
            rotate_sprite_around_transform(&mut cmd, transform);
        } else {
            cmd.origin = [target_w * 0.5, draw_h * 0.5];
        }
        cmd.blend_mode = SpriteBlendMode::Alpha;
        planner.add_sprite(cmd);
    }
    fn plan_text_centered_clipped_on_point(
        &mut self,
        planner: &mut SpritePlanner,
        text: &str,
        center_x: f32,
        center_y: f32,
        max_width: f32,
        clip_x: f32,
        clip_y: f32,
        clip_w: f32,
        clip_h: f32,
        color: [f32; 4],
        z: i32,
        opacity: f32,
        weight: u16,
        family: Option<&str>,
        rotation_transform: &HudLayerTransformConfig,
        font_size: f32,
    ) {
        let Some((texture_id, meta_w, meta_h)) = self.ensure_hud_text_texture(
            text,
            font_size.max(8.0),
            to_u8_color(color, opacity),
            weight,
            family,
        ) else {
            return;
        };
        let draw_h = meta_h.max(1.0);
        let natural_w = meta_w.max(1.0);
        let draw_w = natural_w.min(max_width.max(1.0)).max(1.0);
        add_hud_text_sprite_clipped(
            planner,
            &texture_id,
            center_x - draw_w * 0.5,
            center_y - draw_h * 0.5,
            draw_w,
            draw_h,
            clip_x,
            clip_y,
            clip_w,
            clip_h,
            z,
            rotation_transform,
        );
    }
    fn hud_text_natural_width(
        &mut self,
        text: &str,
        font_size: f32,
        color: [f32; 4],
        opacity: f32,
        weight: u16,
        family: Option<&str>,
    ) -> f32 {
        self.ensure_hud_text_texture(
            text,
            font_size.max(8.0),
            to_u8_color(color, opacity),
            weight,
            family,
        )
        .map(|(_, meta_w, _)| meta_w.max(1.0))
        .unwrap_or_else(|| text.chars().count().max(1) as f32 * font_size.max(8.0) * 0.55)
    }
    fn ensure_hud_asset_frames(&mut self, asset_id: &str) -> Option<Vec<HudAssetFrame>> {
        if let Some(frames) = self.hud_asset_cache.get(asset_id) {
            return Some(frames.clone());
        }
        let asset = self
            .hud_config
            .as_ref()?
            .assets
            .iter()
            .find(|asset| asset.id == asset_id)
            .cloned()?;
        let path = hud_asset_path(&asset)?;
        let frames = if asset.kind.eq_ignore_ascii_case("gif")
            || path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("gif"))
        {
            self.load_hud_gif_asset(asset_id, &path)?
        } else {
            self.load_hud_image_asset(asset_id, &path)?
        };
        // Cache decoded frames under the asset id; config changes should invalidate the renderer instance.
        self.hud_asset_cache
            .insert(asset_id.to_string(), frames.clone());
        Some(frames)
    }
    fn load_hud_image_asset(&mut self, asset_id: &str, path: &Path) -> Option<Vec<HudAssetFrame>> {
        if !hud_asset_file_within_limits(path) {
            return None;
        }
        let (header_width, header_height) = image::image_dimensions(path).ok()?;
        if !hud_asset_dimensions_within_limits(header_width, header_height) {
            return None;
        }
        let image = image::open(path).ok()?.to_rgba8();
        let (width, height) = image.dimensions();
        if !hud_asset_dimensions_within_limits(width, height) {
            return None;
        }
        let texture_id = format!("hud_asset_{}_0", sanitize_texture_component(asset_id));
        if !self.load_texture_rgba(&texture_id, image.as_raw(), width, height) {
            return None;
        }
        self.linear_sampled_textures.insert(texture_id.clone());
        Some(vec![HudAssetFrame {
            texture_id,
            width,
            height,
            delay_ms: 1000,
        }])
    }
    fn load_hud_gif_asset(&mut self, asset_id: &str, path: &Path) -> Option<Vec<HudAssetFrame>> {
        if !hud_asset_file_within_limits(path) {
            return None;
        }
        let file = File::open(path).ok()?;
        let decoder = image::codecs::gif::GifDecoder::new(BufReader::new(file)).ok()?;
        let (header_width, header_height) = decoder.dimensions();
        if !hud_asset_dimensions_within_limits(header_width, header_height) {
            return None;
        }
        let mut total_pixels = 0u64;
        let mut out = Vec::new();
        for (index, frame_result) in decoder.into_frames().enumerate() {
            if index >= HUD_GIF_MAX_FRAMES {
                return None;
            }
            let frame = frame_result.ok()?;
            let delay = frame.delay();
            let (num, denom) = delay.numer_denom_ms();
            let delay_ms = if denom == 0 {
                100
            } else {
                ((num as f32 / denom as f32).round() as u32).max(20)
            };
            let buffer = frame.into_buffer();
            let (width, height) = buffer.dimensions();
            if !hud_asset_dimensions_within_limits(width, height) {
                return None;
            }
            total_pixels =
                total_pixels.saturating_add((width as u64).saturating_mul(height as u64));
            // The total-pixel cap catches GIFs with many individually valid frames.
            if total_pixels > HUD_GIF_MAX_TOTAL_PIXELS {
                return None;
            }
            let texture_id = format!(
                "hud_asset_{}_{}",
                sanitize_texture_component(asset_id),
                index
            );
            if self.load_texture_rgba(&texture_id, buffer.as_raw(), width, height) {
                self.linear_sampled_textures.insert(texture_id.clone());
                out.push(HudAssetFrame {
                    texture_id,
                    width,
                    height,
                    delay_ms,
                });
            }
        }
        (!out.is_empty()).then_some(out)
    }
    fn load_hud_font(&mut self, family: Option<&str>, weight: u16) -> Option<FontArc> {
        let family = family.filter(|value| !value.trim().is_empty())?;
        let font = {
            let config = self.hud_config.as_ref()?;
            config
                .fonts
                .iter()
                .find(|font| font_ref_matches(font, family))
                .cloned()
        };
        let Some(font) = font else {
            self.warn_hud_font_once(
                format!("missing-ref:{family}"),
                format!(
                    "HUD font '{family}' is not in the hud-config font catalog; using fallback"
                ),
            );
            return None;
        };
        let candidates = font_path_candidates_for_weight(&font, weight);
        if candidates.is_empty() {
            self.warn_hud_font_once(
                format!("missing-path:{family}:{weight}"),
                format!(
                    "HUD font '{}' has no usable file path for weight {}; using fallback",
                    family, weight
                ),
            );
            return None;
        }
        let mut missing_paths = Vec::new();
        let mut failed_paths = Vec::new();
        for path in candidates {
            let path_ref = Path::new(&path);
            if !path_ref.is_file() {
                missing_paths.push(path);
                continue;
            }
            if !self.hud_font_cache.contains_key(&path) {
                let loaded = match load_font_arc_from_path(path_ref) {
                    Ok(font) => Some(font),
                    Err(err) => {
                        failed_paths.push(err);
                        None
                    }
                };
                self.hud_font_cache.insert(path.clone(), loaded);
            } else if self
                .hud_font_cache
                .get(&path)
                .is_some_and(|cached| cached.is_none())
            {
                failed_paths.push(format!("previous load failed for {}", path_ref.display()));
            }
            if let Some(font) = self.hud_font_cache.get(&path).and_then(Clone::clone) {
                return Some(font);
            }
        }
        let mut details = Vec::new();
        if !missing_paths.is_empty() {
            details.push(format!("missing: {}", missing_paths.join(", ")));
        }
        if !failed_paths.is_empty() {
            details.push(format!("failed: {}", failed_paths.join("; ")));
        }
        self.warn_hud_font_once(
            format!("load-failed:{family}:{weight}"),
            format!(
                "HUD font '{}' weight {} could not be loaded ({}); using fallback",
                family,
                weight,
                details.join(" | ")
            ),
        );
        None
    }
    fn hud_has_font_family(&self, family: &str) -> bool {
        self.hud_config.as_ref().is_some_and(|config| {
            config
                .fonts
                .iter()
                .any(|font| font_ref_matches(font, family))
        })
    }
    fn load_hud_text_font_stack(&mut self, family: Option<&str>, weight: u16) -> Vec<FontArc> {
        let requested_family = family.map(normalize_font_lookup_key);
        let mut fonts = Vec::new();
        if let Some(font) = self.load_hud_font(family, weight) {
            fonts.push(font);
        }
        for fallback_family in HUD_TEXT_FALLBACK_FAMILIES {
            if requested_family
                .as_ref()
                .is_some_and(|family| *family == normalize_font_lookup_key(fallback_family))
            {
                continue;
            }
            if !self.hud_has_font_family(fallback_family) {
                continue;
            }
            if let Some(font) = self.load_hud_font(Some(fallback_family), weight) {
                fonts.push(font);
            }
        }
        // Embedded fonts are appended later so configured HUD fonts always win.
        fonts
    }
    fn embolden_hud_text(&self, rendered: &mut RenderedText, family: Option<&str>, weight: u16) {
        if rendered.width < 2 || rendered.height < 1 {
            return;
        }
        let amount = hud_synthetic_embolden_amount(family, weight);
        if amount <= 0.0 {
            return;
        }
        let source = rendered.image.clone();
        for y in 0..source.height() {
            for x in 0..source.width().saturating_sub(1) {
                let pixel = source.get_pixel(x, y);
                if pixel[3] == 0 {
                    continue;
                }
                let mut color = *pixel;
                color[3] = ((color[3] as f32) * amount).round().clamp(0.0, 255.0) as u8;
                blend_pixel(&mut rendered.image, x as i32 + 1, y as i32, color);
            }
        }
    }
    fn ensure_hud_text_texture(
        &mut self,
        text: &str,
        font_size: f32,
        color: [u8; 4],
        weight: u16,
        family: Option<&str>,
    ) -> Option<(String, f32, f32)> {
        let family_scale = hud_text_family_scale(family);
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        font_size.to_bits().hash(&mut hasher);
        color.hash(&mut hasher);
        weight.hash(&mut hasher);
        HUD_TEXT_SUPERSAMPLE.to_bits().hash(&mut hasher);
        HUD_TEXT_VISUAL_SCALE.to_bits().hash(&mut hasher);
        family_scale.to_bits().hash(&mut hasher);
        family
            .map(normalize_font_lookup_key)
            .unwrap_or_default()
            .hash(&mut hasher);
        // Text textures are content-addressed so repeated labels across layers share GPU memory.
        let texture_id = format!("hud_text_{:016x}", hasher.finish());
        if !self.loaded_textures.contains(&texture_id) {
            let render_size =
                font_size * HUD_TEXT_VISUAL_SCALE * family_scale * HUD_TEXT_SUPERSAMPLE;
            let font_stack = self.load_hud_text_font_stack(family, weight);
            let mut font_refs = font_stack.iter().collect::<Vec<_>>();
            let embedded_fallback = match fallback_font_weight(weight) {
                FontWeight::Bold => font_bold(),
                FontWeight::Normal => font_regular(),
            };
            if let Some(font) = embedded_fallback {
                font_refs.push(font);
            }
            let mut rendered = render_hud_text_with_fonts(text, render_size, color, &font_refs)?;
            self.embolden_hud_text(&mut rendered, family, weight);
            let width = rendered.width;
            let height = rendered.height;
            let raw = rendered.into_raw();
            if !self.load_hud_texture_rgba_linear(&texture_id, &raw, width, height) {
                return None;
            }
            self.hud_text_cache
                .insert(texture_id.clone(), texture_id.clone());
        }
        self.linear_sampled_textures.insert(texture_id.clone());
        let meta = self.texture_meta.get(&texture_id).copied()?;
        Some((
            texture_id,
            (meta.w as f32 / HUD_TEXT_SUPERSAMPLE).max(1.0),
            (meta.h as f32 / HUD_TEXT_SUPERSAMPLE).max(1.0),
        ))
    }
}
fn format_binding_value(
    binding: &str,
    hud_state: &HudFrameState,
    key_mask: u32,
    props: &Value,
) -> String {
    if binding.starts_with("pp.") && !hud_state.pp_available {
        let fallback = value_string(props, "fallback").unwrap_or("--");
        return if fallback == "hide" {
            String::new()
        } else {
            fallback.to_string()
        };
    }
    if let Some(value) = binding_text(binding, hud_state).filter(|value| !value.trim().is_empty()) {
        let prefix = value_string(props, "prefix").unwrap_or("");
        let suffix = value_string(props, "suffix").unwrap_or("");
        return format!("{prefix}{value}{suffix}");
    }
    if let Some(value) = binding_number(binding, hud_state, key_mask) {
        let prefix = value_string(props, "prefix").unwrap_or("");
        let suffix = value_string(props, "suffix").unwrap_or("");
        let decimals = props
            .get("decimals")
            .and_then(Value::as_i64)
            .map(|value| value.clamp(0, 3) as usize);
        let pad = props
            .get("pad")
            .and_then(Value::as_u64)
            .map(|value| value.min(8) as usize)
            .unwrap_or(0);
        let mut text = if let Some(decimals) = decimals {
            format!("{value:.decimals$}")
        } else {
            format!("{:.0}", value)
        };
        if pad > 0 && text.chars().all(|ch| ch.is_ascii_digit()) {
            text = format!("{text:0>pad$}");
        }
        return format!("{prefix}{text}{suffix}");
    }
    value_string(props, "fallback").unwrap_or("--").to_string()
}

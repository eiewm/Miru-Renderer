//! The user's own results screen, drawn from a HUD layout instead of the
//! built-in composition.
//!
//! Coordinates are authored in the editor against its canvas (`space`), so
//! everything is scaled to the real one before drawing. Text goes through the
//! same font stack as the rest of the renderer, which is what keeps Japanese
//! titles from turning back into boxes on this exact screen.

use crate::hud::{
    hud_asset_dimensions_within_limits, hud_asset_file_within_limits, hud_asset_path,
    HudAssetRefConfig, HudLayerConfig, HudResultsSceneConfig, HUD_GIF_MAX_FRAMES,
    HUD_GIF_MAX_TOTAL_PIXELS,
};
use crate::intro::{font_ubuntu_regular, render_text_simple_with_font};
use crate::results::animation::ResultsAnimationState;
use crate::results::elements::{ResultsElement, ResultsElementSprites};
use crate::results::model::ResultsScreenData;
use crate::utils::image_proc::resize_exact_alpha_safe;
use image::{AnimationDecoder, Rgba, RgbaImage};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Where the authored canvas maps onto the real one.
#[derive(Debug, Clone, Copy)]
struct Space {
    scale_x: f32,
    scale_y: f32,
}

impl Space {
    fn new(scene: &HudResultsSceneConfig, width: u32, height: u32) -> Self {
        let authored_w = scene
            .space
            .as_ref()
            .and_then(|s| s.width)
            .filter(|v| *v > 0.0)
            .unwrap_or(width as f32);
        let authored_h = scene
            .space
            .as_ref()
            .and_then(|s| s.height)
            .filter(|v| *v > 0.0)
            .unwrap_or(height as f32);
        Self {
            scale_x: width as f32 / authored_w,
            scale_y: height as f32 / authored_h,
        }
    }

    fn x(self, v: f32) -> i32 {
        (v * self.scale_x).round() as i32
    }

    fn y(self, v: f32) -> i32 {
        (v * self.scale_y).round() as i32
    }

    fn w(self, v: f32) -> u32 {
        (v * self.scale_x).round().max(0.0) as u32
    }

    fn h(self, v: f32) -> u32 {
        (v * self.scale_y).round().max(0.0) as u32
    }

    /// Type scales with the smaller axis so letters never end up stretched.
    fn font(self, v: f32) -> f32 {
        v * self.scale_x.min(self.scale_y)
    }
}

/// One decoded frame. A still image decodes to a single one.
struct MediaFrame {
    image: RgbaImage,
    delay_ms: u32,
}

/// The images the scene points at, decoded once instead of per frame.
#[derive(Default)]
pub(crate) struct ResultsMedia {
    by_asset: HashMap<String, Vec<MediaFrame>>,
}

impl ResultsMedia {
    pub(crate) fn load(scene: &HudResultsSceneConfig, assets: &[HudAssetRefConfig]) -> Self {
        let mut ids = Vec::new();
        for layer in &scene.layers {
            collect_asset_ids(layer, &mut ids);
        }
        let mut by_asset = HashMap::new();
        for id in ids {
            if by_asset.contains_key(&id) {
                continue;
            }
            let Some(asset) = assets.iter().find(|asset| asset.id == id) else {
                continue;
            };
            if let Some(frames) = decode_asset(asset) {
                by_asset.insert(id, frames);
            }
        }
        Self { by_asset }
    }

    /// GIFs loop on accumulated delays, with the same 20 ms floor as the gameplay HUD.
    fn frame_at(&self, asset_id: &str, elapsed_ms: u32) -> Option<&RgbaImage> {
        let frames = self.by_asset.get(asset_id)?;
        let (first, rest) = frames.split_first()?;
        if rest.is_empty() {
            return Some(&first.image);
        }
        let total: u32 = frames.iter().map(|frame| frame.delay_ms.max(20)).sum();
        if total == 0 {
            return Some(&first.image);
        }
        let mut cursor = elapsed_ms % total;
        for frame in frames {
            let delay = frame.delay_ms.max(20);
            if cursor < delay {
                return Some(&frame.image);
            }
            cursor -= delay;
        }
        Some(&first.image)
    }
}

/// Whether the design builds the screen out of the built-in pieces instead of
/// drawing over a finished one.
pub(crate) fn scene_uses_elements(scene: &HudResultsSceneConfig) -> bool {
    fn any(layers: &[HudLayerConfig]) -> bool {
        layers.iter().any(|layer| {
            ResultsElement::from_layer_type(&layer.layer_type).is_some() || any(&layer.children)
        })
    }
    any(&scene.layers)
}

fn collect_asset_ids(layer: &HudLayerConfig, out: &mut Vec<String>) {
    if let Some(id) = layer_asset_id(layer) {
        out.push(id.to_string());
    }
    for child in &layer.children {
        collect_asset_ids(child, out);
    }
}

fn layer_asset_id(layer: &HudLayerConfig) -> Option<&str> {
    str_prop(&layer.props, "assetId")
        .or_else(|| str_prop(&layer.props, "asset_id"))
        .map(str::trim)
        .filter(|id| !id.is_empty())
}

fn decode_asset(asset: &HudAssetRefConfig) -> Option<Vec<MediaFrame>> {
    let path = hud_asset_path(asset)?;
    if !hud_asset_file_within_limits(&path) {
        return None;
    }
    let is_gif = asset.kind.eq_ignore_ascii_case("gif")
        || path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("gif"));
    if is_gif {
        decode_gif(&path)
    } else {
        decode_still(&path)
    }
}

fn decode_still(path: &Path) -> Option<Vec<MediaFrame>> {
    let (header_width, header_height) = image::image_dimensions(path).ok()?;
    if !hud_asset_dimensions_within_limits(header_width, header_height) {
        return None;
    }
    let image = image::open(path).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    if !hud_asset_dimensions_within_limits(width, height) {
        return None;
    }
    Some(vec![MediaFrame { image, delay_ms: 0 }])
}

fn decode_gif(path: &Path) -> Option<Vec<MediaFrame>> {
    let file = File::open(path).ok()?;
    let decoder = image::codecs::gif::GifDecoder::new(BufReader::new(file)).ok()?;
    let mut total_pixels = 0u64;
    let mut out = Vec::new();
    for (index, frame_result) in decoder.into_frames().enumerate() {
        if index >= HUD_GIF_MAX_FRAMES {
            return None;
        }
        let frame = frame_result.ok()?;
        let (num, denom) = frame.delay().numer_denom_ms();
        let delay_ms = if denom == 0 {
            100
        } else {
            ((num as f32 / denom as f32).round() as u32).max(20)
        };
        let image = frame.into_buffer();
        let (width, height) = image.dimensions();
        if !hud_asset_dimensions_within_limits(width, height) {
            return None;
        }
        total_pixels = total_pixels.saturating_add((width as u64).saturating_mul(height as u64));
        // Catches GIFs whose frames are each within limits but add up.
        if total_pixels > HUD_GIF_MAX_TOTAL_PIXELS {
            return None;
        }
        out.push(MediaFrame { image, delay_ms });
    }
    (!out.is_empty()).then_some(out)
}

fn played_at(data: &ResultsScreenData) -> String {
    // Windows ticks since year 1, which is what an .osr carries.
    let Some(ticks) = data.replay_timestamp else {
        return String::new();
    };
    const TICKS_PER_SECOND: i64 = 10_000_000;
    const SECONDS_TO_UNIX_EPOCH: i64 = 62_135_596_800;
    let unix = ticks / TICKS_PER_SECOND - SECONDS_TO_UNIX_EPOCH;
    if unix <= 0 {
        return String::new();
    }
    let days = unix / 86_400;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Howard Hinnant's civil-from-days, so a date needs no extra dependency.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

/// Resolves a binding path against the finished play.
///
/// The paths are the editor's, so gameplay bindings that still make sense at
/// the end keep working and only what the end adds lives under `results.`.
fn resolve_binding(binding: &str, data: &ResultsScreenData) -> Option<String> {
    let stats = data.statistics;
    Some(match binding {
        "results.rank" => data.grade.label().to_string(),
        "results.player" => data.player_name.clone(),
        "results.maxCombo" => data.max_combo.to_string(),
        "results.fullCombo" => {
            if data.perfect_combo {
                "FC".to_string()
            } else {
                String::new()
            }
        }
        "results.mods" => data
            .mod_badges
            .iter()
            .map(|badge| badge.acronym.clone())
            .collect::<Vec<_>>()
            .join(""),
        "results.playedAt" => played_at(data),
        "score.current" => data.score.to_string(),
        "score.accuracy" | "score.ratio" => format!("{:.2}", data.accuracy),
        "score.combo" => data.final_combo.to_string(),
        "beatmap.title" | "beatmap.titleRomanized" => data.title.clone(),
        "beatmap.artist" | "beatmap.artistRomanized" => data.artist.clone(),
        "beatmap.difficulty" => data.difficulty.clone(),
        "beatmap.mapper" => data.creator.clone(),
        "judgments.max" => stats.max.to_string(),
        "judgments.hit300" => stats.hit300.to_string(),
        "judgments.hit200" => stats.hit200.to_string(),
        "judgments.hit100" => stats.hit100.to_string(),
        "judgments.hit50" => stats.hit50.to_string(),
        "judgments.miss" => stats.miss.to_string(),
        _ => return None,
    })
}

fn str_prop<'a>(props: &'a Value, key: &str) -> Option<&'a str> {
    props.get(key).and_then(Value::as_str)
}

fn num_prop(props: &Value, key: &str) -> Option<f64> {
    props.get(key).and_then(Value::as_f64)
}

/// `#rgb`, `#rrggbb` and `#rrggbbaa`.
fn parse_color(raw: Option<&str>, fallback: [u8; 4]) -> [u8; 4] {
    let Some(text) = raw.map(str::trim) else {
        return fallback;
    };
    let hex = text.strip_prefix('#').unwrap_or(text);
    let parse = |slice: &str| u8::from_str_radix(slice, 16).ok();
    match hex.len() {
        3 => {
            let expand = |c: char| parse(&format!("{c}{c}"));
            let mut chars = hex.chars();
            match (
                chars.next().and_then(expand),
                chars.next().and_then(expand),
                chars.next().and_then(expand),
            ) {
                (Some(r), Some(g), Some(b)) => [r, g, b, fallback[3]],
                _ => fallback,
            }
        }
        6 | 8 => {
            let r = parse(&hex[0..2]);
            let g = parse(&hex[2..4]);
            let b = parse(&hex[4..6]);
            let a = if hex.len() == 8 {
                parse(&hex[6..8])
            } else {
                Some(fallback[3])
            };
            match (r, g, b, a) {
                (Some(r), Some(g), Some(b), Some(a)) => [r, g, b, a],
                _ => fallback,
            }
        }
        _ => fallback,
    }
}

fn fill_rect(canvas: &mut RgbaImage, x: i32, y: i32, w: u32, h: u32, color: [u8; 4], opacity: f32) {
    if w == 0 || h == 0 || opacity <= 0.0 {
        return;
    }
    let alpha = (color[3] as f32 * opacity.clamp(0.0, 1.0)) as u8;
    if alpha == 0 {
        return;
    }
    for dy in 0..h {
        let py = y + dy as i32;
        if py < 0 || py >= canvas.height() as i32 {
            continue;
        }
        for dx in 0..w {
            let px = x + dx as i32;
            if px < 0 || px >= canvas.width() as i32 {
                continue;
            }
            blend_pixel(canvas, px as u32, py as u32, [color[0], color[1], color[2], alpha]);
        }
    }
}

fn blend_pixel(canvas: &mut RgbaImage, x: u32, y: u32, src: [u8; 4]) {
    let dst = canvas.get_pixel_mut(x, y);
    let a = src[3] as f32 / 255.0;
    if a <= 0.0 {
        return;
    }
    for i in 0..3 {
        dst[i] = (src[i] as f32 * a + dst[i] as f32 * (1.0 - a)).round() as u8;
    }
    dst[3] = ((a + dst[3] as f32 / 255.0 * (1.0 - a)) * 255.0).round().min(255.0) as u8;
}

fn bool_prop(props: &Value, key: &str) -> bool {
    props.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// The angle the layer draws at: what the user set, plus the turn it has taken
/// by now if it spins. Same rule as the gameplay HUD.
fn layer_rotation_degrees(layer: &HudLayerConfig, elapsed_ms: u32) -> f32 {
    let spin = if bool_prop(&layer.props, "spinEnabled") {
        let speed = num_prop(&layer.props, "spinSpeed").unwrap_or(90.0) as f32;
        (elapsed_ms as f32 / 1000.0) * speed
    } else {
        0.0
    };
    layer.transform.rotation + spin
}

/// Blits a sprite turned around the centre of its box. Reads back through the
/// inverse rotation and samples bilinearly, which is what keeps a turned rank
/// or logo from coming out with stair edges.
fn blit_rotated(
    canvas: &mut RgbaImage,
    sprite: &RgbaImage,
    x: i32,
    y: i32,
    degrees: f32,
    opacity: f32,
) {
    let opacity = opacity.clamp(0.0, 1.0);
    if opacity <= 0.0 || sprite.width() == 0 || sprite.height() == 0 {
        return;
    }
    let radians = degrees.to_radians();
    let (sin, cos) = radians.sin_cos();
    let width = sprite.width() as f32;
    let height = sprite.height() as f32;
    let center_x = x as f32 + width / 2.0;
    let center_y = y as f32 + height / 2.0;
    // The turned box needs a bigger area to draw into than the sprite itself.
    let half_w = (width * cos.abs() + height * sin.abs()) / 2.0;
    let half_h = (width * sin.abs() + height * cos.abs()) / 2.0;
    let min_x = (center_x - half_w).floor().max(0.0) as i32;
    let min_y = (center_y - half_h).floor().max(0.0) as i32;
    let max_x = (center_x + half_w).ceil().min(canvas.width() as f32) as i32;
    let max_y = (center_y + half_h).ceil().min(canvas.height() as f32) as i32;

    for py in min_y..max_y {
        for px in min_x..max_x {
            let dx = px as f32 + 0.5 - center_x;
            let dy = py as f32 + 0.5 - center_y;
            let sx = dx * cos + dy * sin + width / 2.0 - 0.5;
            let sy = -dx * sin + dy * cos + height / 2.0 - 0.5;
            let Some(pixel) = sample_bilinear(sprite, sx, sy) else {
                continue;
            };
            if pixel[3] == 0 {
                continue;
            }
            let alpha = (pixel[3] as f32 * opacity) as u8;
            blend_pixel(canvas, px as u32, py as u32, [pixel[0], pixel[1], pixel[2], alpha]);
        }
    }
}

/// Samples with straight alpha weighting, so transparent neighbours cannot
/// bleed their colour into the edge.
fn sample_bilinear(sprite: &RgbaImage, sx: f32, sy: f32) -> Option<[u8; 4]> {
    let x0 = sx.floor();
    let y0 = sy.floor();
    if sx < -0.5 || sy < -0.5 || sx > sprite.width() as f32 - 0.5 || sy > sprite.height() as f32 - 0.5
    {
        return None;
    }
    let fx = sx - x0;
    let fy = sy - y0;
    let mut color = [0.0f32; 3];
    let mut alpha = 0.0f32;
    let mut color_weight = 0.0f32;
    for (ox, oy, weight) in [
        (0, 0, (1.0 - fx) * (1.0 - fy)),
        (1, 0, fx * (1.0 - fy)),
        (0, 1, (1.0 - fx) * fy),
        (1, 1, fx * fy),
    ] {
        if weight <= 0.0 {
            continue;
        }
        let px = (x0 as i32 + ox).clamp(0, sprite.width() as i32 - 1) as u32;
        let py = (y0 as i32 + oy).clamp(0, sprite.height() as i32 - 1) as u32;
        let Rgba([r, g, b, a]) = *sprite.get_pixel(px, py);
        let a = a as f32 / 255.0;
        alpha += weight * a;
        let contribution = weight * a;
        color[0] += r as f32 * contribution;
        color[1] += g as f32 * contribution;
        color[2] += b as f32 * contribution;
        color_weight += contribution;
    }
    if alpha <= 0.0 || color_weight <= 0.0 {
        return None;
    }
    Some([
        (color[0] / color_weight).round().clamp(0.0, 255.0) as u8,
        (color[1] / color_weight).round().clamp(0.0, 255.0) as u8,
        (color[2] / color_weight).round().clamp(0.0, 255.0) as u8,
        (alpha * 255.0).round().clamp(0.0, 255.0) as u8,
    ])
}

fn blit(canvas: &mut RgbaImage, sprite: &RgbaImage, x: i32, y: i32, opacity: f32) {
    let opacity = opacity.clamp(0.0, 1.0);
    if opacity <= 0.0 {
        return;
    }
    for (sx, sy, pixel) in sprite.enumerate_pixels() {
        let px = x + sx as i32;
        let py = y + sy as i32;
        if px < 0 || py < 0 || px >= canvas.width() as i32 || py >= canvas.height() as i32 {
            continue;
        }
        let Rgba([r, g, b, a]) = *pixel;
        if a == 0 {
            continue;
        }
        let alpha = (a as f32 * opacity) as u8;
        blend_pixel(canvas, px as u32, py as u32, [r, g, b, alpha]);
    }
}

fn text_for_layer(layer: &HudLayerConfig, data: &ResultsScreenData) -> Option<String> {
    match layer.layer_type.as_str() {
        "text.static" => Some(str_prop(&layer.props, "text").unwrap_or("").to_string()),
        "text.bound" => {
            let binding = layer.binding.as_deref().or(str_prop(&layer.props, "binding"))?;
            let value = resolve_binding(binding, data);
            let fallback = str_prop(&layer.props, "fallback").unwrap_or("");
            let resolved = match value {
                Some(v) if !v.is_empty() => v,
                // Same rule as the editor: nothing to show means the fallback,
                // so a blank field never leaves a hole where a number goes.
                _ => fallback.to_string(),
            };
            let prefix = str_prop(&layer.props, "prefix").unwrap_or("");
            let suffix = str_prop(&layer.props, "suffix").unwrap_or("");
            Some(format!("{prefix}{resolved}{suffix}"))
        }
        _ => None,
    }
}

/// Draws an image, GIF frame or icon at the layer's box, keeping its own size
/// when the layer has none.
fn draw_media(
    canvas: &mut RgbaImage,
    layer: &HudLayerConfig,
    space: Space,
    media: &ResultsMedia,
    elapsed_ms: u32,
    opacity: f32,
) {
    let Some(asset_id) = layer_asset_id(layer) else {
        return;
    };
    let Some(source) = media.frame_at(asset_id, elapsed_ms) else {
        return;
    };
    let width = if layer.transform.width > 0.0 {
        space.w(layer.transform.width)
    } else {
        space.w(source.width().max(1) as f32)
    };
    let height = if layer.transform.height > 0.0 {
        space.h(layer.transform.height)
    } else {
        space.h(source.height().max(1) as f32)
    };
    if width == 0 || height == 0 {
        return;
    }
    let scaled = resize_exact_alpha_safe(source, width, height);
    blit_placed(
        canvas,
        &scaled,
        space.x(layer.transform.x),
        space.y(layer.transform.y),
        layer_rotation_degrees(layer, elapsed_ms),
        opacity,
    );
}

/// Straight blit when the layer sits square, turned blit when it does not.
fn blit_placed(
    canvas: &mut RgbaImage,
    sprite: &RgbaImage,
    x: i32,
    y: i32,
    degrees: f32,
    opacity: f32,
) {
    if degrees.abs() < f32::EPSILON {
        blit(canvas, sprite, x, y, opacity);
        return;
    }
    blit_rotated(canvas, sprite, x, y, degrees, opacity);
}

/// A filled box that can be turned. Only pays for the extra buffer when it is.
fn fill_rect_placed(
    canvas: &mut RgbaImage,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    color: [u8; 4],
    degrees: f32,
    opacity: f32,
) {
    if degrees.abs() < f32::EPSILON {
        fill_rect(canvas, x, y, w, h, color, opacity);
        return;
    }
    if w == 0 || h == 0 {
        return;
    }
    let sprite = RgbaImage::from_pixel(w, h, Rgba(color));
    blit_rotated(canvas, &sprite, x, y, degrees, opacity);
}

/// The box the piece had when the editor turned it into a layer.
///
/// It is not the box to draw in. A title, a rank or the life graph change size
/// with the play, so forcing the layer's box on them squashes the real sprite:
/// the box is only there to read how far the user moved it and how much bigger
/// they made it.
struct ElementBase {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

fn element_base(layer: &HudLayerConfig) -> Option<ElementBase> {
    let value = |key| num_prop(&layer.props, key).map(|v| v as f32);
    let base = ElementBase {
        x: value("baseX")?,
        y: value("baseY")?,
        width: value("baseWidth")?,
        height: value("baseHeight")?,
    };
    (base.width > 0.0 && base.height > 0.0).then_some(base)
}

/// One of the built-in pieces, drawn where the layer puts it. The sprite is the
/// same one the editor shows, so what the canvas promised is what comes out.
#[allow(clippy::too_many_arguments)]
fn draw_element_layer(
    canvas: &mut RgbaImage,
    element: ResultsElement,
    layer: &HudLayerConfig,
    space: Space,
    data: &ResultsScreenData,
    elements: &ResultsElementSprites,
    state: &ResultsAnimationState,
    elapsed_ms: u32,
    opacity: f32,
) {
    // The ribbon exists as a sprite so it can be placed, but it only belongs on
    // a play that kept the combo.
    if element == ResultsElement::Perfect && !data.perfect_combo {
        return;
    }
    let Some(sprite) = elements.get(element) else {
        return;
    };
    let natural_w = sprite.image.width() as f32;
    let natural_h = sprite.image.height() as f32;
    let (mut x, mut y, width, height) = match element_base(layer) {
        // The piece keeps its own size and place, moved and scaled by what the
        // user did to it. Untouched, that is exactly the built-in screen.
        Some(base) => {
            let scale = |size: f32, base: f32| if size > 0.0 { size / base } else { 1.0 };
            let scale_x = scale(layer.transform.width, base.width);
            let scale_y = scale(layer.transform.height, base.height);
            (
                sprite.x + space.x(layer.transform.x) - space.x(base.x),
                sprite.y + space.y(layer.transform.y) - space.y(base.y),
                (natural_w * scale_x).round().max(0.0) as u32,
                (natural_h * scale_y).round().max(0.0) as u32,
            )
        }
        // A design from before the box was recorded: the layer's box is all
        // there is to go on.
        None => (
            space.x(layer.transform.x),
            space.y(layer.transform.y),
            if layer.transform.width > 0.0 {
                space.w(layer.transform.width)
            } else {
                sprite.image.width()
            },
            if layer.transform.height > 0.0 {
                space.h(layer.transform.height)
            } else {
                sprite.image.height()
            },
        ),
    };
    if width == 0 || height == 0 {
        return;
    }
    let anim = element.animation(state);
    x += anim.offset_x.round() as i32;
    y += anim.offset_y.round() as i32;
    let opacity = opacity * anim.alpha;
    let degrees = layer_rotation_degrees(layer, elapsed_ms);
    let same_size = width == sprite.image.width() && height == sprite.image.height();
    if degrees.abs() >= f32::EPSILON {
        let turned;
        let source = if same_size {
            &sprite.image
        } else {
            turned = resize_exact_alpha_safe(&sprite.image, width, height);
            &turned
        };
        blit_rotated(canvas, source, x, y, degrees, opacity);
        return;
    }
    // The built-in painter's blend, not this file's: the same pixels have to
    // come out whether the piece is a layer or part of the default screen.
    if same_size {
        super::render::alpha_blit(canvas, &sprite.image, x, y, opacity);
        return;
    }
    let scaled = resize_exact_alpha_safe(&sprite.image, width, height);
    super::render::alpha_blit(canvas, &scaled, x, y, opacity);
}

#[allow(clippy::too_many_arguments)]
fn draw_layer(
    canvas: &mut RgbaImage,
    layer: &HudLayerConfig,
    space: Space,
    data: &ResultsScreenData,
    media: &ResultsMedia,
    elapsed_ms: u32,
    inherited_opacity: f32,
    scene_fade: f32,
    elements: &ResultsElementSprites,
    state: &ResultsAnimationState,
) {
    if !layer.visible {
        return;
    }
    let opacity = inherited_opacity * layer.transform.opacity.clamp(0.0, 1.0);
    if opacity <= 0.0 {
        return;
    }

    let x = space.x(layer.transform.x);
    let y = space.y(layer.transform.y);
    let w = space.w(layer.transform.width);
    let h = space.h(layer.transform.height);

    if let Some(element) = ResultsElement::from_layer_type(&layer.layer_type) {
        // Built-in pieces bring their own entry animation, so the scene-wide
        // fade would count twice.
        draw_element_layer(
            canvas, element, layer, space, data, elements, state, elapsed_ms, opacity,
        );
        draw_children(
            canvas,
            layer,
            space,
            data,
            media,
            elapsed_ms,
            opacity,
            scene_fade,
            elements,
            state,
        );
        return;
    }

    // The scene-wide fade paints the leaf; children get the clean chain so it
    // is not applied once per level.
    let painted = opacity * scene_fade;
    if painted > 0.0 {
        let degrees = layer_rotation_degrees(layer, elapsed_ms);
        match layer.layer_type.as_str() {
            "group" => {}
            "shape.rect" => {
                let fill = parse_color(str_prop(&layer.style, "fill"), [0xFF, 0xFF, 0xFF, 0xFF]);
                fill_rect_placed(canvas, x, y, w, h, fill, degrees, painted);
            }
            "shape.line" => {
                let stroke =
                    parse_color(str_prop(&layer.style, "stroke"), [0xFF, 0xFF, 0xFF, 0xFF]);
                let thickness = num_prop(&layer.style, "strokeWidth").unwrap_or(2.0) as f32;
                fill_rect_placed(
                    canvas,
                    x,
                    y,
                    w,
                    space.h(thickness).max(1),
                    stroke,
                    degrees,
                    painted,
                );
            }
            "media.image" | "media.gif" | "icon.static" => {
                draw_media(canvas, layer, space, media, elapsed_ms, painted);
            }
            _ => {
                if let Some(text) = text_for_layer(layer, data).filter(|text| !text.is_empty()) {
                    let size =
                        space.font(num_prop(&layer.style, "fontSize").unwrap_or(24.0) as f32);
                    let color =
                        parse_color(str_prop(&layer.style, "color"), [0xFF, 0xFF, 0xFF, 0xFF]);
                    if let Some(font) = font_ubuntu_regular() {
                        if let Some(rendered) =
                            render_text_simple_with_font(&text, size.max(1.0), color, font)
                        {
                            let align = str_prop(&layer.style, "textAlign").unwrap_or("left");
                            let offset = match align {
                                "center" => (w as i32 - rendered.width as i32) / 2,
                                "right" => w as i32 - rendered.width as i32,
                                _ => 0,
                            };
                            blit_placed(
                                canvas,
                                &rendered.image,
                                x + offset,
                                y,
                                degrees,
                                painted,
                            );
                        }
                    }
                }
            }
        }
    }

    draw_children(
        canvas, layer, space, data, media, elapsed_ms, opacity, scene_fade, elements, state,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_children(
    canvas: &mut RgbaImage,
    layer: &HudLayerConfig,
    space: Space,
    data: &ResultsScreenData,
    media: &ResultsMedia,
    elapsed_ms: u32,
    opacity: f32,
    scene_fade: f32,
    elements: &ResultsElementSprites,
    state: &ResultsAnimationState,
) {
    // Siblings draw in the editor's order, which sorts per level.
    let mut children: Vec<&HudLayerConfig> = layer.children.iter().collect();
    children.sort_by_key(|child| child.z_index);
    for child in children {
        draw_layer(
            canvas, child, space, data, media, elapsed_ms, opacity, scene_fade, elements, state,
        );
    }
}

/// Draws the custom results screen over `canvas`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_custom_results(
    canvas: &mut RgbaImage,
    scene: &HudResultsSceneConfig,
    data: &ResultsScreenData,
    media: &ResultsMedia,
    elapsed_ms: u32,
    scene_fade: f32,
    elements: &ResultsElementSprites,
    state: &ResultsAnimationState,
) {
    let space = Space::new(scene, canvas.width(), canvas.height());
    let mut layers: Vec<&HudLayerConfig> = scene.layers.iter().collect();
    layers.sort_by_key(|layer| layer.z_index);
    for layer in layers {
        draw_layer(
            canvas, layer, space, data, media, elapsed_ms, 1.0, scene_fade, elements, state,
        );
    }
}

#[cfg(test)]
mod tests;

use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
const MAX_HUD_CONFIG_JSON_BYTES: usize = 512 * 1024;
// HUD configs can come from user-edited JSON, so keep tree and asset counts bounded.
const MAX_HUD_ASSETS: usize = 128;
const MAX_HUD_FONTS: usize = 64;
const MAX_HUD_NODES: usize = 512;
const MAX_HUD_DEPTH: usize = 18;
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HudElementConfig {
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub size: Option<f32>,
    pub scale: Option<f32>,
    pub rotation: Option<f32>,
    pub spin_enabled: Option<bool>,
    pub spin_speed: Option<f32>,
    pub visible: Option<bool>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HudColumnItemConfig {
    pub index: Option<usize>,
    pub x: Option<f32>,
    pub width: Option<f32>,
    pub opacity: Option<f32>,
    pub gap_after: Option<f32>,
    pub visible: Option<bool>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HudColumnsConfig {
    #[serde(flatten)]
    pub base: HudElementConfig,
    pub opacity: Option<f32>,
    pub gap: Option<f32>,
    pub items: Vec<HudColumnItemConfig>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HudCanvasConfig {
    pub width: Option<f32>,
    pub height: Option<f32>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HudLayerTransformConfig {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub rotation: f32,
    pub opacity: f32,
}
impl Default for HudLayerTransformConfig {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
            rotation: 0.0,
            opacity: 1.0,
        }
    }
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HudEffectConfig {
    pub id: String,
    pub trigger: String,
    pub property: String,
    pub from: Value,
    pub to: Value,
    pub duration_ms: i32,
    pub delay_ms: i32,
    pub easing: String,
    pub binding: Option<String>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HudLayerConfig {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub layer_type: String,
    pub visible: bool,
    pub locked: bool,
    pub z_index: i32,
    pub transform: HudLayerTransformConfig,
    pub style: Value,
    pub animation: Value,
    pub effects: Vec<HudEffectConfig>,
    pub binding: Option<String>,
    pub props: Value,
    pub children: Vec<HudLayerConfig>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HudAssetRefConfig {
    pub id: String,
    pub kind: String,
    pub name: Option<String>,
    pub mime_type: Option<String>,
    pub storage_key: Option<String>,
    pub url: Option<String>,
    pub path: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_ms: Option<u32>,
    pub frame_count: Option<u32>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HudFontRefConfig {
    pub id: String,
    pub family: String,
    pub css_family: Option<String>,
    pub package_name: Option<String>,
    pub license: String,
    pub source_url: String,
    pub license_url: String,
    pub path: Option<String>,
    pub normal_path: Option<String>,
    pub bold_path: Option<String>,
    pub weight_paths: HashMap<String, String>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HudElements {
    pub score: Option<HudElementConfig>,
    pub accuracy: Option<HudElementConfig>,
    #[serde(alias = "hitErrorMeter", alias = "precisionMeter")]
    pub hit_error_meter: Option<HudElementConfig>,
    pub mods: Option<HudElementConfig>,
    #[serde(alias = "lifeBar")]
    pub life_bar: Option<HudElementConfig>,
    #[serde(alias = "progress_circle")]
    pub progress_circle: Option<HudElementConfig>,
    pub combo: Option<HudElementConfig>,
    #[serde(alias = "judgment_pop")]
    pub judgment_pop: Option<HudElementConfig>,
    pub columns: Option<HudColumnsConfig>,
    #[serde(alias = "stageBottom")]
    pub stage_bottom: Option<HudElementConfig>,
    #[serde(alias = "stageLeft")]
    pub stage_left: Option<HudElementConfig>,
    #[serde(alias = "stageRight")]
    pub stage_right: Option<HudElementConfig>,
    #[serde(alias = "stageHint")]
    pub stage_hint: Option<HudElementConfig>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HudSpaceConfig {
    pub width: Option<f32>,
    pub height: Option<f32>,
}
/// The user's own results screen. Absent means the built-in screen is drawn,
/// which is not the same as present-and-empty: that would draw nothing at all.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HudResultsSceneConfig {
    pub space: Option<HudSpaceConfig>,
    pub layers: Vec<HudLayerConfig>,
    /// `replace` swaps the built-in screen; anything else draws over it.
    pub mode: Option<String>,
}
impl HudResultsSceneConfig {
    pub fn replaces_default_screen(&self) -> bool {
        self.mode
            .as_deref()
            .is_some_and(|mode| mode.eq_ignore_ascii_case("replace"))
    }
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HudConfig {
    pub version: Option<u32>,
    pub mode: Option<String>,
    pub canvas: Option<HudCanvasConfig>,
    pub nodes: Vec<HudLayerConfig>,
    pub layers: Vec<HudLayerConfig>,
    pub assets: Vec<HudAssetRefConfig>,
    pub fonts: Vec<HudFontRefConfig>,
    pub templates: Value,
    pub metadata: Value,
    pub space: Option<HudSpaceConfig>,
    pub elements: HudElements,
    pub results: Option<HudResultsSceneConfig>,
}
pub fn parse_hud_config_json(input: &str) -> Result<HudConfig, String> {
    if input.len() > MAX_HUD_CONFIG_JSON_BYTES {
        return Err(format!(
            "hud-config JSON too large: {} bytes > {} bytes",
            input.len(),
            MAX_HUD_CONFIG_JSON_BYTES
        ));
    }
    let config = serde_json::from_str::<HudConfig>(input)
        .map_err(|err| format!("invalid hud-config JSON: {}", err))?;
    validate_hud_config_limits(&config)?;
    Ok(config)
}
fn count_layers(layers: &[HudLayerConfig], depth: usize) -> Result<usize, String> {
    if depth > MAX_HUD_DEPTH {
        return Err("hud-config node tree is too deep".to_string());
    }
    let mut count = 0usize;
    for layer in layers {
        count = count.saturating_add(1);
        count = count.saturating_add(count_layers(&layer.children, depth + 1)?);
        if count > MAX_HUD_NODES {
            return Err(format!(
                "hud-config has too many nodes: {count} > {MAX_HUD_NODES}"
            ));
        }
    }
    Ok(count)
}
fn validate_hud_config_limits(config: &HudConfig) -> Result<(), String> {
    if config.assets.len() > MAX_HUD_ASSETS {
        return Err(format!(
            "hud-config has too many assets: {} > {}",
            config.assets.len(),
            MAX_HUD_ASSETS
        ));
    }
    if config.fonts.len() > MAX_HUD_FONTS {
        return Err(format!(
            "hud-config has too many fonts: {} > {}",
            config.fonts.len(),
            MAX_HUD_FONTS
        ));
    }
    // The results scene counts against the same cap: otherwise the limit is
    // sidestepped by sending the extra nodes through it.
    let results_count = match config.results.as_ref() {
        Some(scene) => count_layers(&scene.layers, 1)?,
        None => 0,
    };
    let node_count = count_layers(&config.nodes, 1)?
        .saturating_add(count_layers(&config.layers, 1)?)
        .saturating_add(results_count);
    if node_count > MAX_HUD_NODES {
        return Err(format!(
            "hud-config has too many nodes: {node_count} > {MAX_HUD_NODES}"
        ));
    }
    Ok(())
}
fn scale_value(value: Option<f32>, factor: f32) -> Option<f32> {
    value.filter(|v| v.is_finite()).map(|v| v * factor)
}
fn resolve_element(el: &HudElementConfig, sx: f32, sy: f32) -> HudElementConfig {
    HudElementConfig {
        x: scale_value(el.x, sx),
        y: scale_value(el.y, sy),
        width: scale_value(el.width, sx),
        height: scale_value(el.height, sy),
        size: scale_value(el.size, sy),
        scale: el.scale,
        rotation: el.rotation,
        spin_enabled: el.spin_enabled,
        spin_speed: el.spin_speed,
        visible: el.visible,
    }
}
fn resolve_column_item(item: &HudColumnItemConfig, sx: f32) -> HudColumnItemConfig {
    HudColumnItemConfig {
        index: item.index,
        x: scale_value(item.x, sx),
        width: scale_value(item.width, sx),
        opacity: item.opacity,
        gap_after: scale_value(item.gap_after, sx),
        visible: item.visible,
    }
}
fn resolve_columns(cols: &HudColumnsConfig, sx: f32, sy: f32) -> HudColumnsConfig {
    HudColumnsConfig {
        base: resolve_element(&cols.base, sx, sy),
        opacity: cols.opacity,
        gap: scale_value(cols.gap, sx),
        items: cols
            .items
            .iter()
            .map(|i| resolve_column_item(i, sx))
            .collect(),
    }
}
fn resolve_layer_transform(
    transform: &HudLayerTransformConfig,
    sx: f32,
    sy: f32,
) -> HudLayerTransformConfig {
    HudLayerTransformConfig {
        x: transform.x * sx,
        y: transform.y * sy,
        width: (transform.width * sx).max(1.0),
        height: (transform.height * sy).max(1.0),
        rotation: transform.rotation,
        opacity: transform.opacity.clamp(0.0, 1.0),
    }
}
fn json_number_as_f32(value: &Value) -> Option<f32> {
    value
        .as_f64()
        .map(|value| value as f32)
        .filter(|value| value.is_finite())
}
fn scaled_json_number(value: &Value, factor: f32) -> Option<Value> {
    json_number_as_f32(value).map(|number| Value::from(number * factor))
}
fn scale_json_field_value(key: &str, value: &Value, sx: f32, sy: f32) -> Option<Value> {
    let uniform = (sx.abs() + sy.abs()) * 0.5;
    // Custom HUD layers store layout in free-form props/style JSON, not only typed fields.
    let factor = match key {
        "x" | "left" | "right" | "offsetX" | "minX" | "maxX" | "itemWidth" | "widthPx"
        | "tailWidth" | "columnGap" | "gapAfter" => sx,
        "y" | "top" | "bottom" | "offsetY" | "minY" | "maxY" | "itemHeight" | "heightPx"
        | "tailMaxHeight" | "tailReleaseSpeed" => sy,
        "fontSize"
        | "labelFontSize"
        | "gap"
        | "keySize"
        | "radius"
        | "borderWidth"
        | "strokeWidth"
        | "itemBorderWidth"
        | "separatorWidth"
        | "lineWidth"
        | "gridLineWidth"
        | "judgementLineThickness"
        | "axisWidth"
        | "axisHeight"
        | "colorBarWidth"
        | "colorBarHeight"
        | "centreMarkerSize"
        | "chevronSize"
        | "labelGap"
        | "padding"
        | "shadowBlur"
        | "glowBlur"
        | "glowWidth" => uniform,
        _ => return None,
    };
    scaled_json_number(value, factor)
}
fn scale_json_layout_values(value: &Value, sx: f32, sy: f32) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = Map::with_capacity(map.len());
            for (key, raw_value) in map {
                let next_value = scale_json_field_value(key, raw_value, sx, sy)
                    .unwrap_or_else(|| scale_json_layout_values(raw_value, sx, sy));
                out.insert(key.clone(), next_value);
            }
            Value::Object(out)
        }
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|item| scale_json_layout_values(item, sx, sy))
                .collect(),
        ),
        _ => value.clone(),
    }
}
fn resolve_effect_value(property: &str, value: &Value, sx: f32, sy: f32) -> Value {
    let uniform = (sx.abs() + sy.abs()) * 0.5;
    let factor = match property {
        "translateX" => sx,
        "translateY" => sy,
        "strokeWidth" | "borderWidth" | "radius" | "shadowBlur" | "glowBlur" => uniform,
        _ => return value.clone(),
    };
    scaled_json_number(value, factor).unwrap_or_else(|| value.clone())
}
fn resolve_effect(effect: &HudEffectConfig, sx: f32, sy: f32) -> HudEffectConfig {
    HudEffectConfig {
        from: resolve_effect_value(&effect.property, &effect.from, sx, sy),
        to: resolve_effect_value(&effect.property, &effect.to, sx, sy),
        ..effect.clone()
    }
}
fn resolve_layer(layer: &HudLayerConfig, sx: f32, sy: f32) -> HudLayerConfig {
    HudLayerConfig {
        transform: resolve_layer_transform(&layer.transform, sx, sy),
        style: scale_json_layout_values(&layer.style, sx, sy),
        props: scale_json_layout_values(&layer.props, sx, sy),
        animation: scale_json_layout_values(&layer.animation, sx, sy),
        effects: layer
            .effects
            .iter()
            .map(|effect| resolve_effect(effect, sx, sy))
            .collect(),
        children: layer
            .children
            .iter()
            .map(|child| resolve_layer(child, sx, sy))
            .collect(),
        ..layer.clone()
    }
}
pub fn resolve_hud_config(config: &HudConfig, canvas_w: f32, canvas_h: f32) -> HudConfig {
    // Scale from the editor's design space into the actual render canvas.
    let space_w = config
        .space
        .as_ref()
        .and_then(|space| space.width)
        .filter(|w| *w > 0.0);
    let space_h = config
        .space
        .as_ref()
        .and_then(|space| space.height)
        .or_else(|| config.canvas.as_ref().and_then(|canvas| canvas.height))
        .filter(|h| *h > 0.0);
    let space_w = space_w
        .or_else(|| config.canvas.as_ref().and_then(|canvas| canvas.width))
        .filter(|w| *w > 0.0);
    let sx = space_w.map(|sw| canvas_w / sw).unwrap_or(1.0);
    let sy = space_h.map(|sh| canvas_h / sh).unwrap_or(1.0);
    let elements = &config.elements;
    HudConfig {
        version: config.version,
        mode: config.mode.clone(),
        canvas: config.canvas.clone(),
        // Unscaled: the scene carries its own `space` and adjusts when drawn.
        results: config.results.clone(),
        nodes: config
            .nodes
            .iter()
            .map(|node| resolve_layer(node, sx, sy))
            .collect(),
        layers: config
            .layers
            .iter()
            .map(|layer| resolve_layer(layer, sx, sy))
            .collect(),
        assets: config.assets.clone(),
        fonts: config.fonts.clone(),
        templates: config.templates.clone(),
        metadata: config.metadata.clone(),
        space: config.space.clone(),
        elements: HudElements {
            score: elements.score.as_ref().map(|e| resolve_element(e, sx, sy)),
            accuracy: elements
                .accuracy
                .as_ref()
                .map(|e| resolve_element(e, sx, sy)),
            hit_error_meter: elements
                .hit_error_meter
                .as_ref()
                .map(|e| resolve_element(e, sx, sy)),
            mods: elements.mods.as_ref().map(|e| resolve_element(e, sx, sy)),
            life_bar: elements
                .life_bar
                .as_ref()
                .map(|e| resolve_element(e, sx, sy)),
            progress_circle: elements
                .progress_circle
                .as_ref()
                .map(|e| resolve_element(e, sx, sy)),
            combo: elements.combo.as_ref().map(|e| resolve_element(e, sx, sy)),
            judgment_pop: elements
                .judgment_pop
                .as_ref()
                .map(|e| resolve_element(e, sx, sy)),
            columns: elements
                .columns
                .as_ref()
                .map(|c| resolve_columns(c, sx, sy)),
            stage_bottom: elements
                .stage_bottom
                .as_ref()
                .map(|e| resolve_element(e, sx, sy)),
            stage_left: elements
                .stage_left
                .as_ref()
                .map(|e| resolve_element(e, sx, sy)),
            stage_right: elements
                .stage_right
                .as_ref()
                .map(|e| resolve_element(e, sx, sy)),
            stage_hint: elements
                .stage_hint
                .as_ref()
                .map(|e| resolve_element(e, sx, sy)),
        },
    }
}

use std::collections::HashMap;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ComboBurstStyle {
    Left,
    #[default]
    Right,
    Both,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpecialStyle {
    #[default]
    None,
    Left,
    Right,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotePart {
    Note,
    Head,
    Body,
    Tail,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NoteBodyStyle {
    Stretch,
    #[default]
    RepeatBottom,
    RepeatTop,
    RepeatTopAndBottom,
}
#[derive(Debug, Clone, Default)]
pub struct LegacyHudLayout {
    pub score: Option<LegacyHudDrawableLayout>,
    pub accuracy: Option<LegacyHudDrawableLayout>,
    pub song_progress: Option<LegacyHudDrawableLayout>,
}
#[derive(Debug, Clone, Copy)]
pub struct LegacyHudDrawableLayout {
    pub x: f32,
    pub y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub anchor: i32,
    pub origin: i32,
}
#[derive(Debug, Clone, Default)]
pub struct SkinConfig {
    pub name: String,
    pub author: String,
    pub version: String,
    pub score_prefix: String,
    pub combo_prefix: String,
    pub hit_prefix: String,
    pub note_image: Vec<String>,
    pub note_image_h: Vec<String>,
    pub note_image_l: Vec<String>,
    pub note_image_t: Vec<String>,
    pub key_image: Vec<String>,
    pub key_image_d: Vec<String>,
    pub lighting_n: Vec<String>,
    pub lighting_l: Vec<String>,
    pub note_padding_x: i32,
    pub width_for_note_height_scale: Option<u32>,
    pub stage_left: Option<String>,
    pub stage_right: Option<String>,
    pub stage_bottom: Option<String>,
    pub stage_hint: Option<String>,
    pub stage_light: Option<String>,
    pub warning_arrow: Option<String>,
    pub column_colors: Vec<Option<[f32; 4]>>,
    pub colour_light: Vec<Option<[f32; 4]>>,
    pub colour_column_line: Option<[f32; 4]>,
    pub colour_barline: Option<[f32; 4]>,
    pub colour_judgement_line: Option<[f32; 4]>,
    pub colour_key_warning: Option<[f32; 4]>,
    pub colour_hold: Option<[f32; 4]>,
    pub colour_break: Option<[f32; 4]>,
    pub light_position: Option<i32>,
    pub light_frame_per_second: Option<u32>,
    pub judgement_line: Option<bool>,
    pub upside_down: Option<bool>,
    pub combo_burst_style: Option<ComboBurstStyle>,
    pub combo_burst_random: Option<bool>,
    pub special_style: Option<SpecialStyle>,
    pub lighting_n_width: Option<Vec<i32>>,
    pub lighting_l_width: Option<Vec<i32>>,
    pub column_line_width: Option<Vec<f32>>,
    pub barline_height: Option<f32>,
    pub key_flip_when_upside_down: Option<bool>,
    pub key_flip_when_upside_down_col: Vec<Option<bool>>,
    pub key_flip_when_upside_down_pressed: Vec<Option<bool>>,
    pub note_flip_when_upside_down: Option<bool>,
    pub note_flip_when_upside_down_col: Vec<Option<bool>>,
    pub note_flip_when_upside_down_h: Vec<Option<bool>>,
    pub note_flip_when_upside_down_l: Vec<Option<bool>>,
    pub note_flip_when_upside_down_t: Vec<Option<bool>>,
    pub note_body_style: Option<NoteBodyStyle>,
    pub note_body_style_col: Vec<Option<NoteBodyStyle>>,
    pub combo_burst: Vec<String>,
    pub score_overlap: Option<i32>,
    pub combo_overlap: Option<i32>,
    pub combo_pos_y: Option<i32>,
    pub score_y: Option<i32>,
    pub animation_framerate: Option<u32>,
    pub hit_0: Option<String>,
    pub hit_50: Option<String>,
    pub hit_100: Option<String>,
    pub hit_200: Option<String>,
    pub hit_300: Option<String>,
    pub hit_300g: Option<String>,
    pub column_start: Option<i32>,
    pub column_right: Option<i32>,
    pub column_widths: Option<Vec<i32>>,
    pub column_spacing: Option<Vec<i32>>,
    pub hit_position: Option<i32>,
    pub keys_under_notes: bool,
}
impl SkinConfig {
    const DEFAULT_LIGHT_POSITION: i32 = 413;
    const DEFAULT_LIGHT_FPS: u32 = 24;
    const DEFAULT_COLOUR_LIGHT: [f32; 4] = [55.0 / 255.0, 1.0, 1.0, 1.0];
    const DEFAULT_RGBA_WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    const DEFAULT_RGB_WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    const DEFAULT_KEY_WARNING: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
    pub fn score_prefix_or_default(&self) -> &str {
        if self.score_prefix.is_empty() {
            "score"
        } else {
            &self.score_prefix
        }
    }
    pub fn combo_prefix_or_default(&self) -> &str {
        if self.combo_prefix.is_empty() {
            self.score_prefix_or_default()
        } else {
            &self.combo_prefix
        }
    }
    pub fn is_skin_version_at_least(&self, major: u32, minor: u32) -> bool {
        let version = self.version.trim();
        if version.is_empty() {
            return false;
        }
        if version.eq_ignore_ascii_case("latest") {
            // Some skins use "latest" instead of a numeric version in skin.ini.
            return true;
        }
        let mut parts = version.split('.');
        let major_value = parts
            .next()
            .and_then(|part| part.trim().parse::<u32>().ok())
            .unwrap_or(0);
        let minor_value = parts
            .next()
            .and_then(|part| part.trim().parse::<u32>().ok())
            .unwrap_or(0);
        (major_value, minor_value) >= (major, minor)
    }
    pub fn uses_modern_mania_defaults(&self) -> bool {
        self.is_skin_version_at_least(2, 5)
    }
    pub fn resolved_light_position(&self) -> i32 {
        self.light_position.unwrap_or(Self::DEFAULT_LIGHT_POSITION)
    }
    pub fn resolved_light_frame_rate(&self) -> u32 {
        self.light_frame_per_second
            .filter(|fps| *fps > 0)
            .unwrap_or(Self::DEFAULT_LIGHT_FPS)
    }
    pub fn resolved_judgement_line(&self) -> bool {
        self.judgement_line.unwrap_or(true)
    }
    pub fn resolved_upside_down(&self) -> bool {
        self.upside_down.unwrap_or(false)
    }
    pub fn resolved_combo_burst_style(&self) -> ComboBurstStyle {
        self.combo_burst_style.unwrap_or(ComboBurstStyle::Right)
    }
    pub fn resolved_combo_burst_random(&self) -> bool {
        self.combo_burst_random.unwrap_or(false)
    }
    pub fn resolved_special_style(&self) -> SpecialStyle {
        self.special_style.unwrap_or(SpecialStyle::None)
    }
    pub fn resolved_note_body_style(&self, column: usize) -> NoteBodyStyle {
        if let Some(Some(style)) = self.note_body_style_col.get(column) {
            return *style;
        }
        if let Some(style) = self.note_body_style {
            return style;
        }
        // Skin v2.5 changed the mania LN body default from stretch to repeat-bottom.
        if self.uses_modern_mania_defaults() {
            NoteBodyStyle::RepeatBottom
        } else {
            NoteBodyStyle::Stretch
        }
    }
    pub fn resolved_colour_column_line(&self) -> [f32; 4] {
        self.colour_column_line.unwrap_or(Self::DEFAULT_RGBA_WHITE)
    }
    pub fn resolved_colour_barline(&self) -> [f32; 4] {
        self.colour_barline.unwrap_or(Self::DEFAULT_RGBA_WHITE)
    }
    pub fn resolved_colour_judgement_line(&self) -> [f32; 4] {
        self.colour_judgement_line
            .unwrap_or(Self::DEFAULT_RGB_WHITE)
    }
    pub fn resolved_colour_light(&self, column: usize) -> [f32; 4] {
        self.colour_light
            .get(column)
            .and_then(|color| *color)
            .unwrap_or(Self::DEFAULT_COLOUR_LIGHT)
    }
    pub fn resolved_colour_key_warning(&self) -> [f32; 4] {
        self.colour_key_warning.unwrap_or(Self::DEFAULT_KEY_WARNING)
    }
    pub fn resolved_colour_hold(&self) -> Option<[f32; 4]> {
        self.colour_hold
    }
    pub fn resolved_colour_break(&self) -> Option<[f32; 4]> {
        self.colour_break
    }
    pub fn resolved_lighting_n_width(&self, column: usize) -> Option<i32> {
        self.lighting_n_width
            .as_ref()
            .and_then(|widths| widths.get(column))
            .copied()
            .filter(|width| *width > 0)
    }
    pub fn resolved_lighting_l_width(&self, column: usize) -> Option<i32> {
        self.lighting_l_width
            .as_ref()
            .and_then(|widths| widths.get(column))
            .copied()
            .filter(|width| *width > 0)
    }
    pub fn resolved_column_line_width(&self, column: usize) -> f32 {
        match &self.column_line_width {
            Some(widths) => widths
                .get(column)
                .copied()
                .filter(|width| width.is_finite() && *width >= 0.0)
                .unwrap_or(0.0),
            None => 2.0,
        }
    }
    pub fn resolved_barline_height(&self) -> f32 {
        self.barline_height
            .filter(|height| height.is_finite() && *height > 0.0)
            .unwrap_or(1.2)
    }
    pub fn resolved_key_flip_when_upside_down(&self, column: usize, pressed: bool) -> bool {
        // Modern mania skins default to flipping keys in upside-down mode unless overridden.
        let modern_default = self.uses_modern_mania_defaults();
        let global = self.key_flip_when_upside_down.unwrap_or(modern_default);
        let column_value = self
            .key_flip_when_upside_down_col
            .get(column)
            .and_then(|value| *value)
            .unwrap_or(global);
        if pressed {
            self.key_flip_when_upside_down_pressed
                .get(column)
                .and_then(|value| *value)
                .unwrap_or(column_value)
        } else {
            column_value
        }
    }
    pub fn resolved_note_flip_when_upside_down(&self, column: usize, part: NotePart) -> bool {
        // Note parts inherit the column flip unless a part-specific override exists.
        let modern_default = self.uses_modern_mania_defaults();
        let global = self.note_flip_when_upside_down.unwrap_or(modern_default);
        let column_value = self
            .note_flip_when_upside_down_col
            .get(column)
            .and_then(|value| *value)
            .unwrap_or(global);
        match part {
            NotePart::Note => column_value,
            NotePart::Head => self
                .note_flip_when_upside_down_h
                .get(column)
                .and_then(|value| *value)
                .unwrap_or(column_value),
            NotePart::Body => self
                .note_flip_when_upside_down_l
                .get(column)
                .and_then(|value| *value)
                .unwrap_or(column_value),
            NotePart::Tail => self
                .note_flip_when_upside_down_t
                .get(column)
                .and_then(|value| *value)
                .unwrap_or(column_value),
        }
    }
}
pub struct SkinAssets {
    pub config: SkinConfig,
    pub images: HashMap<String, Vec<u8>>,
    pub legacy_hud_layout: Option<LegacyHudLayout>,
}
impl SkinAssets {
    pub fn new() -> Self {
        Self {
            config: SkinConfig::default(),
            images: HashMap::new(),
            legacy_hud_layout: None,
        }
    }
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            config: SkinConfig::default(),
            images: HashMap::with_capacity(cap),
            legacy_hud_layout: None,
        }
    }
    #[inline]
    pub fn has_image(&self, name: &str) -> bool {
        self.images.contains_key(name)
    }
    #[inline]
    pub fn get_image(&self, name: &str) -> Option<&[u8]> {
        self.images.get(name).map(|v| v.as_slice())
    }
    pub fn image_count(&self) -> usize {
        self.images.len()
    }
    pub fn normalize_key(key: &str) -> String {
        // Skin archives can mix Windows and Unix separators; lookups use lower-case forward slashes.
        key.to_lowercase().replace('\\', "/")
    }
    pub fn find_image(&self, name: &str) -> Option<&[u8]> {
        let norm = Self::normalize_key(name);
        self.images.get(&norm).map(|v| v.as_slice())
    }
    pub fn find_first<'a>(&'a self, names: &[&str]) -> Option<(&'a str, &'a [u8])> {
        for name in names {
            let norm = Self::normalize_key(name);
            if let Some((k, v)) = self.images.get_key_value(&norm) {
                return Some((k.as_str(), v.as_slice()));
            }
        }
        None
    }
}
impl Default for SkinAssets {
    fn default() -> Self {
        Self::new()
    }
}

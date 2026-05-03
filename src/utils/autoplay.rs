use crate::types::{ApiModEntry, ReplayModInfo, ReplayOrigin};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
const AUTOPLAY_MODS_VERSION: u8 = 1;
const AUTOPLAY_MODS_MODE: &str = "autoplay";
const SCORE_PROFILE_ACRONYMS: &[&str] = &["SV2", "SV1"];
// Autoplay still sets the legacy AT bit so downstream replay paths treat generated input as autoplay.
const INTERNAL_AUTOPLAY_BIT: u32 = 1 << 11;
// Older saved configs exposed these as toggles; normalization now drops them instead of failing.
const REMOVED_PUBLIC_AUTOPLAY_MODS: &[&str] = &["AT", "EZ", "HR", "NF", "SD", "PF", "AC", "NR"];
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoplayModsConfig {
    pub version: u8,
    pub mode: String,
    #[serde(default)]
    pub mods: Vec<AutoplayModSelectionEntry>,
}
impl Default for AutoplayModsConfig {
    fn default() -> Self {
        default_autoplay_mods_config()
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoplayModSelectionEntry {
    pub acronym: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub settings: serde_json::Value,
}
#[derive(Debug, Clone, Serialize)]
pub struct AutoplayModCatalog {
    pub version: u8,
    pub mods: Vec<AutoplayModDescriptor>,
}
#[derive(Debug, Clone, Serialize)]
pub struct AutoplayModDescriptor {
    pub acronym: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub kind: &'static str,
    pub configurable: bool,
    pub default_enabled: bool,
    pub supported: bool,
    pub description: &'static str,
    pub settings_schema: BTreeMap<String, AutoplayModSettingSchema>,
    pub conflicts_with: Vec<&'static str>,
    pub display_priority: u16,
}
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AutoplayModSettingSchema {
    Number {
        label: &'static str,
        min: f64,
        max: f64,
        step: f64,
        default: f64,
        unit: Option<&'static str>,
    },
    Boolean {
        label: &'static str,
        default: bool,
    },
    Enum {
        label: &'static str,
        default: &'static str,
        options: Vec<AutoplayModEnumOption>,
    },
}
#[derive(Debug, Clone, Serialize)]
pub struct AutoplayModEnumOption {
    pub value: &'static str,
    pub label: &'static str,
}
#[derive(Debug, Clone)]
pub struct NormalizedAutoplayMods {
    pub config: AutoplayModsConfig,
    pub legacy_bits: u32,
    pub api_mods: Vec<ApiModEntry>,
    pub display_mods: Vec<String>,
    pub has_classic: bool,
    pub has_score_v2: bool,
    pub origin: ReplayOrigin,
}
impl NormalizedAutoplayMods {
    pub fn replay_mod_info(&self) -> ReplayModInfo {
        ReplayModInfo {
            legacy_bits: self.legacy_bits,
            api_mods: self.api_mods.clone(),
            has_classic: self.has_classic,
            has_score_v2: self.has_score_v2,
            display_mods: Some(self.display_mods.clone()),
        }
    }
}
pub fn autoplay_mod_catalog() -> AutoplayModCatalog {
    AutoplayModCatalog {
        version: AUTOPLAY_MODS_VERSION,
        mods: autoplay_mod_descriptors(),
    }
}
pub fn default_autoplay_mods_config() -> AutoplayModsConfig {
    AutoplayModsConfig {
        version: AUTOPLAY_MODS_VERSION,
        mode: AUTOPLAY_MODS_MODE.to_string(),
        mods: autoplay_mod_descriptors()
            .into_iter()
            .filter(|descriptor| descriptor.default_enabled)
            .map(|descriptor| AutoplayModSelectionEntry {
                acronym: descriptor.acronym.to_string(),
                enabled: true,
                settings: default_settings_object(&descriptor.settings_schema),
            })
            .collect(),
    }
}
pub fn parse_autoplay_mods_config_json(raw: &str) -> Result<AutoplayModsConfig, String> {
    let parsed = serde_json::from_str::<AutoplayModsConfig>(raw)
        .map_err(|error| format!("invalid autoplay-mods JSON: {error}"))?;
    normalize_autoplay_mods_config(&parsed).map(|normalized| normalized.config)
}
pub fn normalize_autoplay_mods_config(
    config: &AutoplayModsConfig,
) -> Result<NormalizedAutoplayMods, String> {
    if config.version != AUTOPLAY_MODS_VERSION {
        return Err(format!(
            "unsupported autoplay-mods version: {}",
            config.version
        ));
    }
    if !config.mode.eq_ignore_ascii_case(AUTOPLAY_MODS_MODE) {
        return Err(format!("unsupported autoplay-mods mode: {}", config.mode));
    }
    let descriptors = autoplay_mod_descriptors();
    let descriptor_map: BTreeMap<&'static str, AutoplayModDescriptor> = descriptors
        .iter()
        .cloned()
        .map(|descriptor| (descriptor.acronym, descriptor))
        .collect();
    let mut seen = BTreeSet::new();
    let mut normalized_entries = Vec::with_capacity(config.mods.len());
    for entry in &config.mods {
        let acronym = entry.acronym.trim().to_ascii_uppercase();
        if acronym.is_empty() {
            return Err("autoplay mod acronym cannot be empty".to_string());
        }
        // Removed public toggles are ignored for backward compatibility with saved UI configs.
        if is_removed_public_autoplay_mod(&acronym) {
            continue;
        }
        if !seen.insert(acronym.clone()) {
            return Err(format!("duplicate autoplay mod: {acronym}"));
        }
        let descriptor = descriptor_map
            .get(acronym.as_str())
            .ok_or_else(|| format!("unknown autoplay mod: {acronym}"))?;
        if !descriptor.supported {
            return Err(format!("autoplay mod not supported: {acronym}"));
        }
        let settings = normalize_settings(entry.settings.clone(), &descriptor.settings_schema)?;
        normalized_entries.push(AutoplayModSelectionEntry {
            acronym,
            enabled: entry.enabled,
            settings,
        });
    }
    ensure_default_score_profile(&mut normalized_entries);
    let mut active = normalized_entries
        .iter()
        .filter(|entry| entry.enabled)
        .filter_map(|entry| {
            descriptor_map
                .get(entry.acronym.as_str())
                .cloned()
                .map(|descriptor| (entry, descriptor))
        })
        .collect::<Vec<_>>();
    // Display order follows explicit priority so speed/profile mods stay ahead of visual details.
    active.sort_by(|(_, left), (_, right)| {
        right
            .display_priority
            .cmp(&left.display_priority)
            .then_with(|| left.acronym.cmp(right.acronym))
    });
    let active_set = active
        .iter()
        .map(|(entry, _)| entry.acronym.as_str())
        .collect::<BTreeSet<_>>();
    for (_, descriptor) in &active {
        for conflict in &descriptor.conflicts_with {
            if active_set.contains(conflict) {
                return Err(format!(
                    "autoplay mods conflict: {} vs {}",
                    descriptor.acronym, conflict
                ));
            }
        }
    }
    let legacy_bits = active
        .iter()
        .fold(INTERNAL_AUTOPLAY_BIT, |bits, (entry, _)| {
            bits | legacy_bit_for_acronym(entry.acronym.as_str()).unwrap_or(0)
        });
    let api_mods = active
        .iter()
        .map(|(entry, _)| ApiModEntry {
            acronym: entry.acronym.clone(),
            settings: entry.settings.clone(),
        })
        .collect::<Vec<_>>();
    let mut display_mods = vec!["AT".to_string()];
    display_mods.extend(
        active
            .iter()
            // ScoreV1 is the default profile marker and should not take a visible badge slot.
            .filter(|(entry, _)| !entry.acronym.eq_ignore_ascii_case("SV1"))
            .map(|(entry, _)| entry.acronym.clone()),
    );
    let has_classic = active_set.contains("CL");
    let has_score_v2 = active_set.contains("SV2");
    let origin = if api_mods.is_empty() {
        ReplayOrigin::StableLegacy
    } else {
        ReplayOrigin::LazerExport
    };
    Ok(NormalizedAutoplayMods {
        config: AutoplayModsConfig {
            version: AUTOPLAY_MODS_VERSION,
            mode: AUTOPLAY_MODS_MODE.to_string(),
            mods: normalized_entries,
        },
        legacy_bits,
        api_mods,
        display_mods,
        has_classic,
        has_score_v2,
        origin,
    })
}
fn normalize_settings(
    raw_settings: serde_json::Value,
    schema: &BTreeMap<String, AutoplayModSettingSchema>,
) -> Result<serde_json::Value, String> {
    let mut object = match raw_settings {
        serde_json::Value::Null => serde_json::Map::new(),
        serde_json::Value::Object(object) => object,
        _ => return Err("autoplay mod settings must be an object".to_string()),
    };
    for key in object.keys() {
        if !schema.contains_key(key) {
            return Err(format!("unknown autoplay mod setting: {key}"));
        }
    }
    let mut normalized = serde_json::Map::new();
    for (key, descriptor) in schema {
        let value = match object.remove(key) {
            Some(value) => validate_setting_value(key, value, descriptor)?,
            None => default_setting_value(descriptor),
        };
        normalized.insert(key.clone(), value);
    }
    Ok(serde_json::Value::Object(normalized))
}
fn validate_setting_value(
    key: &str,
    value: serde_json::Value,
    descriptor: &AutoplayModSettingSchema,
) -> Result<serde_json::Value, String> {
    match descriptor {
        AutoplayModSettingSchema::Number { min, max, .. } => {
            let Some(number) = value.as_f64() else {
                return Err(format!("autoplay mod setting must be a number: {key}"));
            };
            if !number.is_finite() || number < *min || number > *max {
                return Err(format!(
                    "autoplay mod setting out of range: {key} ({number})"
                ));
            }
            Ok(serde_json::Value::from(number))
        }
        AutoplayModSettingSchema::Boolean { .. } => {
            let Some(boolean) = value.as_bool() else {
                return Err(format!("autoplay mod setting must be a boolean: {key}"));
            };
            Ok(serde_json::Value::from(boolean))
        }
        AutoplayModSettingSchema::Enum { options, .. } => {
            let Some(raw) = value.as_str() else {
                return Err(format!("autoplay mod setting must be a string: {key}"));
            };
            let normalized = raw.trim();
            let valid = options
                .iter()
                .any(|option| option.value.eq_ignore_ascii_case(normalized));
            if !valid {
                return Err(format!("autoplay mod setting has invalid value: {key}"));
            }
            let canonical = options
                .iter()
                .find(|option| option.value.eq_ignore_ascii_case(normalized))
                .map(|option| option.value)
                .unwrap_or(normalized);
            Ok(serde_json::Value::from(canonical))
        }
    }
}
fn default_settings_object(
    schema: &BTreeMap<String, AutoplayModSettingSchema>,
) -> serde_json::Value {
    let mut object = serde_json::Map::new();
    for (key, descriptor) in schema {
        object.insert(key.clone(), default_setting_value(descriptor));
    }
    serde_json::Value::Object(object)
}
fn default_setting_value(descriptor: &AutoplayModSettingSchema) -> serde_json::Value {
    match descriptor {
        AutoplayModSettingSchema::Number { default, .. } => serde_json::Value::from(*default),
        AutoplayModSettingSchema::Boolean { default, .. } => serde_json::Value::from(*default),
        AutoplayModSettingSchema::Enum { default, .. } => serde_json::Value::from(*default),
    }
}
fn legacy_bit_for_acronym(acronym: &str) -> Option<u32> {
    match acronym {
        "HD" => Some(1 << 3),
        "DT" => Some(1 << 6),
        "HT" => Some(1 << 8),
        "NC" => Some(1 << 9),
        "FL" => Some(1 << 10),
        "FI" => Some(1 << 20),
        "MR" => Some(1 << 30),
        _ => None,
    }
}
fn is_removed_public_autoplay_mod(acronym: &str) -> bool {
    REMOVED_PUBLIC_AUTOPLAY_MODS
        .iter()
        .any(|removed| acronym.eq_ignore_ascii_case(removed))
}
fn ensure_default_score_profile(entries: &mut Vec<AutoplayModSelectionEntry>) {
    // Judgment code needs an explicit score profile even when the user only selected visual mods.
    let has_active_profile = entries.iter().any(|entry| {
        entry.enabled
            && SCORE_PROFILE_ACRONYMS
                .iter()
                .any(|acronym| entry.acronym.eq_ignore_ascii_case(acronym))
    });
    if has_active_profile {
        return;
    }
    if let Some(entry) = entries
        .iter_mut()
        .find(|entry| entry.acronym.eq_ignore_ascii_case("SV1"))
    {
        entry.enabled = true;
        if !entry.settings.is_object() {
            entry.settings = serde_json::Value::Object(serde_json::Map::new());
        }
    } else {
        entries.push(AutoplayModSelectionEntry {
            acronym: "SV1".to_string(),
            enabled: true,
            settings: serde_json::Value::Object(serde_json::Map::new()),
        });
    }
}
fn autoplay_mod_descriptors() -> Vec<AutoplayModDescriptor> {
    vec![
        descriptor(
            "DT",
            "Double Time",
            "speed",
            "shared_rate",
            true,
            false,
            true,
            "Increase playback speed with lazer-style settings.",
            200,
        )
        .with_schema(number_setting(
            "speed_change",
            "Speed",
            1.01,
            2.0,
            0.01,
            1.5,
            Some("x"),
        ))
        .with_schema(boolean_setting("adjust_pitch", "Adjust Pitch", false))
        .with_conflicts(&["NC", "HT", "DC", "AS", "WU", "WD"])
        .build(),
        descriptor(
            "NC",
            "Nightcore",
            "speed",
            "shared_rate",
            true,
            false,
            true,
            "Nightcore timing with configurable speed.",
            190,
        )
        .with_schema(number_setting(
            "speed_change",
            "Speed",
            1.01,
            2.0,
            0.01,
            1.5,
            Some("x"),
        ))
        .with_conflicts(&["DT", "HT", "DC", "AS", "WU", "WD"])
        .build(),
        descriptor(
            "HT",
            "Half Time",
            "speed",
            "shared_rate",
            true,
            false,
            true,
            "Slow down autoplay and optionally keep pitch.",
            180,
        )
        .with_schema(number_setting(
            "speed_change",
            "Speed",
            0.5,
            0.99,
            0.01,
            0.75,
            Some("x"),
        ))
        .with_schema(boolean_setting("adjust_pitch", "Adjust Pitch", false))
        .with_conflicts(&["DT", "NC", "DC", "AS", "WU", "WD"])
        .build(),
        descriptor(
            "DC",
            "Daycore",
            "speed",
            "api_rate",
            true,
            false,
            true,
            "Slow down with daycore pitch profile.",
            170,
        )
        .with_schema(number_setting(
            "speed_change",
            "Speed",
            0.5,
            0.99,
            0.01,
            0.75,
            Some("x"),
        ))
        .with_conflicts(&["DT", "NC", "HT", "AS", "WU", "WD"])
        .build(),
        descriptor(
            "AS",
            "Adaptive Speed",
            "speed",
            "api_rate",
            true,
            false,
            true,
            "Continuously adapt playback rate over the map.",
            160,
        )
        .with_schema(number_setting(
            "initial_rate",
            "Initial Rate",
            0.5,
            2.0,
            0.01,
            1.0,
            Some("x"),
        ))
        .with_schema(boolean_setting("adjust_pitch", "Adjust Pitch", true))
        .with_conflicts(&["DT", "NC", "HT", "DC", "WU", "WD"])
        .build(),
        descriptor(
            "WU",
            "Wind Up",
            "speed",
            "api_rate",
            true,
            false,
            true,
            "Ramp playback speed up over time.",
            150,
        )
        .with_schema(number_setting(
            "initial_rate",
            "Initial Rate",
            0.5,
            2.0,
            0.01,
            1.0,
            Some("x"),
        ))
        .with_schema(number_setting(
            "final_rate",
            "Final Rate",
            0.5,
            2.0,
            0.01,
            1.5,
            Some("x"),
        ))
        .with_schema(boolean_setting("adjust_pitch", "Adjust Pitch", true))
        .with_conflicts(&["DT", "NC", "HT", "DC", "AS", "WD"])
        .build(),
        descriptor(
            "WD",
            "Wind Down",
            "speed",
            "api_rate",
            true,
            false,
            true,
            "Ramp playback speed down over time.",
            140,
        )
        .with_schema(number_setting(
            "initial_rate",
            "Initial Rate",
            0.5,
            2.0,
            0.01,
            1.0,
            Some("x"),
        ))
        .with_schema(number_setting(
            "final_rate",
            "Final Rate",
            0.5,
            2.0,
            0.01,
            0.75,
            Some("x"),
        ))
        .with_schema(boolean_setting("adjust_pitch", "Adjust Pitch", true))
        .with_conflicts(&["DT", "NC", "HT", "DC", "AS", "WU"])
        .build(),
        descriptor(
            "MR",
            "Mirror",
            "pattern",
            "legacy_toggle",
            false,
            false,
            true,
            "Mirror the column layout.",
            58,
        )
        .with_conflicts(&[])
        .build(),
        descriptor(
            "FI",
            "Fade In",
            "visibility",
            "shared_visual",
            false,
            false,
            true,
            "Fade notes into view near the receptor.",
            80,
        )
        .with_conflicts(&["HD", "FL", "CO"])
        .build(),
        descriptor(
            "HD",
            "Hidden",
            "visibility",
            "shared_visual",
            false,
            false,
            true,
            "Hide notes before they reach the receptor.",
            75,
        )
        .with_conflicts(&["FI", "FL", "CO"])
        .build(),
        descriptor(
            "FL",
            "Flashlight",
            "visibility",
            "shared_visual",
            true,
            false,
            true,
            "Restrict visibility around the receptor with adjustable flashlight size.",
            70,
        )
        .with_schema(number_setting(
            "size_multiplier",
            "Size",
            0.5,
            3.0,
            0.1,
            1.0,
            Some("x"),
        ))
        .with_schema(boolean_setting(
            "combo_based_size",
            "Combo Based Size",
            false,
        ))
        .with_conflicts(&["FI", "HD", "CO"])
        .build(),
        descriptor(
            "CO",
            "Cover",
            "visibility",
            "api_visual",
            true,
            false,
            true,
            "Cover the playfield with configurable direction and coverage.",
            65,
        )
        .with_schema(number_setting(
            "coverage", "Coverage", 0.2, 0.8, 0.01, 0.5, None,
        ))
        .with_schema(enum_setting(
            "direction",
            "Direction",
            "Downwards",
            &[("Downwards", "Along Scroll"), ("Upwards", "Against Scroll")],
        ))
        .with_conflicts(&["FI", "HD", "FL"])
        .build(),
        descriptor(
            "IN",
            "Invert",
            "pattern",
            "api_pattern",
            false,
            false,
            true,
            "Invert note durations into different patterns.",
            60,
        )
        .with_conflicts(&["HO"])
        .build(),
        descriptor(
            "HO",
            "Hold Off",
            "pattern",
            "api_pattern",
            false,
            false,
            true,
            "Convert holds into taps.",
            55,
        )
        .with_conflicts(&["IN"])
        .build(),
        descriptor(
            "SV2",
            "ScoreV2",
            "score",
            "score_profile",
            false,
            false,
            true,
            "Use osu!stable ScoreV2 scoring and windows.",
            45,
        )
        .with_conflicts(&["SV1"])
        .build(),
        descriptor(
            "SV1",
            "ScoreV1",
            "score",
            "score_profile",
            false,
            true,
            true,
            "Use osu!stable ScoreV1 scoring and windows.",
            44,
        )
        .with_conflicts(&["SV2"])
        .build(),
        descriptor(
            "MU",
            "Muted",
            "audio",
            "api_audio",
            true,
            false,
            true,
            "Mute gameplay audio using combo-driven automation.",
            35,
        )
        .with_schema(boolean_setting("inverse_muting", "Inverse Muting", false))
        .with_schema(boolean_setting(
            "enable_metronome",
            "Enable Metronome",
            true,
        ))
        .with_schema(number_setting(
            "mute_combo_count",
            "Mute Combo Count",
            1.0,
            500.0,
            1.0,
            100.0,
            None,
        ))
        .with_schema(boolean_setting(
            "affects_hit_sounds",
            "Affects Hit Sounds",
            true,
        ))
        .with_conflicts(&[])
        .build(),
    ]
}
fn number_setting(
    key: &'static str,
    label: &'static str,
    min: f64,
    max: f64,
    step: f64,
    default: f64,
    unit: Option<&'static str>,
) -> (&'static str, AutoplayModSettingSchema) {
    (
        key,
        AutoplayModSettingSchema::Number {
            label,
            min,
            max,
            step,
            default,
            unit,
        },
    )
}
fn boolean_setting(
    key: &'static str,
    label: &'static str,
    default: bool,
) -> (&'static str, AutoplayModSettingSchema) {
    (key, AutoplayModSettingSchema::Boolean { label, default })
}
fn enum_setting(
    key: &'static str,
    label: &'static str,
    default: &'static str,
    options: &[(&'static str, &'static str)],
) -> (&'static str, AutoplayModSettingSchema) {
    (
        key,
        AutoplayModSettingSchema::Enum {
            label,
            default,
            options: options
                .iter()
                .map(|(value, text)| AutoplayModEnumOption { value, label: text })
                .collect(),
        },
    )
}
struct DescriptorBuilder {
    descriptor: AutoplayModDescriptor,
}
impl DescriptorBuilder {
    fn with_schema(mut self, setting: (&'static str, AutoplayModSettingSchema)) -> Self {
        self.descriptor
            .settings_schema
            .insert(setting.0.to_string(), setting.1);
        self
    }
    fn with_conflicts(mut self, conflicts: &[&'static str]) -> Self {
        self.descriptor.conflicts_with = conflicts.to_vec();
        self
    }
    fn build(mut self) -> AutoplayModDescriptor {
        self.descriptor.configurable = !self.descriptor.settings_schema.is_empty();
        self.descriptor
    }
}
fn descriptor(
    acronym: &'static str,
    label: &'static str,
    group: &'static str,
    kind: &'static str,
    configurable: bool,
    default_enabled: bool,
    supported: bool,
    description: &'static str,
    display_priority: u16,
) -> DescriptorBuilder {
    DescriptorBuilder {
        descriptor: AutoplayModDescriptor {
            acronym,
            label,
            group,
            kind,
            configurable,
            default_enabled,
            supported,
            description,
            settings_schema: BTreeMap::new(),
            conflicts_with: Vec::new(),
            display_priority,
        },
    }
}

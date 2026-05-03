use super::*;
use serde::Deserialize;
use std::io::{Read, Seek};
const DEFAULT_OSK: &str = "assets/skin/- Lain memories.osk";
const EMBEDDED_DEFAULT_OSK: &[u8] = include_bytes!("../../assets/skin/- Lain memories.osk");

fn default_skin_path() -> PathBuf {
    let mut candidates = Vec::new();

    if let Some(path) = std::env::var_os("MIRU_DEFAULT_SKIN").filter(|value| !value.is_empty()) {
        candidates.push(PathBuf::from(path));
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join("../../assets/skin/- Lain memories.osk"));
            candidates.push(exe_dir.join("../assets/skin/- Lain memories.osk"));
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("assets/skin/- Lain memories.osk"));
        candidates.push(cwd.join("../miru-renderer-rust/assets/skin/- Lain memories.osk"));
    }

    candidates.push(PathBuf::from(DEFAULT_OSK));
    candidates.push(PathBuf::from("./skins/default"));

    for candidate in candidates {
        if candidate.exists() {
            return candidate;
        }
    }

    materialize_embedded_default_skin().unwrap_or_else(|| PathBuf::from("./skins/default"))
}

fn materialize_embedded_default_skin() -> Option<PathBuf> {
    let skin_dir = std::env::temp_dir().join("miru-renderer").join("skin");
    let skin_path = skin_dir.join("- Lain memories.osk");
    if embedded_default_skin_is_ready(&skin_path) {
        return Some(skin_path);
    }

    if let Err(error) = std::fs::create_dir_all(&skin_dir) {
        println!("   [skin] warn: failed to prepare embedded default skin: {error}");
        return None;
    }

    let temp_path = skin_dir.join(format!(".default-skin-{}.osk.tmp", std::process::id()));
    if let Err(error) = std::fs::write(&temp_path, EMBEDDED_DEFAULT_OSK) {
        println!("   [skin] warn: failed to write embedded default skin: {error}");
        return None;
    }

    if skin_path.exists() {
        let _ = std::fs::remove_file(&skin_path);
    }
    match std::fs::rename(&temp_path, &skin_path) {
        Ok(()) => Some(skin_path),
        Err(_) if embedded_default_skin_is_ready(&skin_path) => {
            let _ = std::fs::remove_file(temp_path);
            Some(skin_path)
        }
        Err(error) => {
            let _ = std::fs::remove_file(temp_path);
            println!("   [skin] warn: failed to install embedded default skin: {error}");
            None
        }
    }
}

fn embedded_default_skin_is_ready(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|metadata| metadata.len() == EMBEDDED_DEFAULT_OSK.len() as u64)
        .unwrap_or(false)
}
fn is_audio_filename(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "wav" | "ogg" | "mp3"))
        .unwrap_or(false)
}
fn is_skin_archive_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "osk" | "zip"))
        .unwrap_or(false)
}
fn detect_skin_archive_root_prefix<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<Option<String>, ConvertError> {
    // Many .osk/.zip skins wrap all files in one folder; treat that folder as the skin root.
    let mut nested_skin_ini_prefixes = Vec::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|err| ConvertError::Resolve(format!("skin archive entry error: {err}")))?;
        if entry.is_dir() {
            continue;
        }
        let raw_name = entry.name().to_string();
        let normalized = crate::utils::sanitize_archive_entry_name(&raw_name).ok_or_else(|| {
            ConvertError::Resolve(format!(
                "unsafe skin archive entry path rejected: {raw_name}"
            ))
        })?;
        let lower = normalized.to_lowercase();
        if lower == "skin.ini" {
            return Ok(None);
        }
        if let Some(parent) = lower.strip_suffix("/skin.ini") {
            if !parent.is_empty() && !parent.starts_with("__macosx/") {
                nested_skin_ini_prefixes.push(format!("{parent}/"));
            }
        }
    }
    nested_skin_ini_prefixes.sort_by_key(|prefix| prefix.matches('/').count());
    nested_skin_ini_prefixes.dedup();
    Ok(nested_skin_ini_prefixes.into_iter().next())
}
fn normalize_skin_archive_entry_name(
    raw_name: &str,
    root_prefix: Option<&str>,
) -> Result<Option<String>, ConvertError> {
    let normalized = crate::utils::sanitize_archive_entry_name(raw_name).ok_or_else(|| {
        ConvertError::Resolve(format!(
            "unsafe skin archive entry path rejected: {raw_name}"
        ))
    })?;
    let lower = normalized.to_lowercase();
    if let Some(prefix) = root_prefix {
        if !lower.starts_with(prefix) {
            return Ok(None);
        }
        let stripped = lower[prefix.len()..].trim_start_matches('/').to_string();
        if stripped.is_empty() {
            return Ok(None);
        }
        Ok(Some(stripped))
    } else {
        Ok(Some(lower))
    }
}
fn make_temp_skin_audio_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("miru_skin_audio_{nanos}"))
}
#[derive(Debug, Clone, Copy)]
struct SkinLoadLimits {
    max_archive_entries: usize,
    max_directory_files: usize,
    max_image_files: usize,
    max_audio_files: usize,
    max_metadata_bytes: u64,
    max_image_file_bytes: u64,
    max_total_image_bytes: u64,
    max_audio_file_bytes: u64,
    max_total_audio_bytes: u64,
    max_loaded_images: usize,
    max_image_dimension: u32,
    max_image_pixels: u64,
}
const DEFAULT_SKIN_LOAD_LIMITS: SkinLoadLimits = SkinLoadLimits {
    // Skins are untrusted user input, so cap file counts, bytes, and decoded image size.
    max_archive_entries: 4096,
    max_directory_files: 4096,
    max_image_files: 2048,
    max_audio_files: 512,
    max_metadata_bytes: 512 * 1024,
    max_image_file_bytes: 64 * 1024 * 1024,
    max_total_image_bytes: 512 * 1024 * 1024,
    max_audio_file_bytes: 32 * 1024 * 1024,
    max_total_audio_bytes: 128 * 1024 * 1024,
    max_loaded_images: 4096,
    max_image_dimension: crate::utils::MAX_TEXTURE_DIM,
    max_image_pixels: 64 * 1024 * 1024,
};
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum LegacyMainHudLayoutFile {
    Versioned(LegacyMainHudLayoutVersioned),
    LegacyList(Vec<LegacyMainHudDrawableInfo>),
}
#[derive(Debug, Deserialize)]
struct LegacyMainHudLayoutVersioned {
    #[serde(rename = "DrawableInfo")]
    drawable_info: std::collections::HashMap<String, Vec<LegacyMainHudDrawableInfo>>,
}
#[derive(Debug, Deserialize)]
struct LegacyMainHudDrawableInfo {
    #[serde(rename = "Type")]
    drawable_type: String,
    #[serde(rename = "Position")]
    position: LegacyMainHudVec2,
    #[serde(rename = "Scale")]
    scale: LegacyMainHudVec2,
    #[serde(rename = "Anchor")]
    anchor: i32,
    #[serde(rename = "Origin")]
    origin: i32,
}
#[derive(Debug, Deserialize)]
struct LegacyMainHudVec2 {
    x: f32,
    y: f32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum GameplayAssetResolution {
    Resolved(String),
    MissingExplicit,
    Unspecified,
}
impl ManiaVideoConverter {
    fn normalize_skin_prefix(prefix: &str) -> String {
        let mut normalized = prefix
            .trim()
            .trim_end_matches('-')
            .to_lowercase()
            .replace('\\', "/");
        while normalized.contains("//") {
            normalized = normalized.replace("//", "/");
        }
        normalized
    }
    fn insert_skin_image(
        skin: &mut SkinAssets,
        normalized: String,
        data: Vec<u8>,
        limits: SkinLoadLimits,
    ) -> Result<(), ConvertError> {
        let base_key = Path::new(&normalized)
            .file_stem()
            .and_then(|name| name.to_str())
            .map(|basename| basename.to_lowercase());
        let will_insert_primary = !skin.images.contains_key(&normalized);
        let will_insert_alias = base_key
            .as_ref()
            .map(|base| base != &normalized && !skin.images.contains_key(base))
            .unwrap_or(false);
        let projected_count =
            skin.images.len() + usize::from(will_insert_primary) + usize::from(will_insert_alias);
        if projected_count > limits.max_loaded_images {
            return Err(ConvertError::Resolve(format!(
                "skin image alias count exceeds limit: {} > {}",
                projected_count, limits.max_loaded_images
            )));
        }
        skin.images.insert(normalized.clone(), data);
        // Keep both full archive paths and basename aliases because skin.ini uses both forms.
        if let Some(base_key) = base_key {
            if will_insert_alias {
                if let Some(existing) = skin.images.get(&normalized).cloned() {
                    skin.images.insert(base_key, existing);
                }
            }
        }
        Ok(())
    }
    fn parse_legacy_main_hud_layout(input: &str) -> Option<crate::types::LegacyHudLayout> {
        let parsed = serde_json::from_str::<LegacyMainHudLayoutFile>(input).ok()?;
        match parsed {
            LegacyMainHudLayoutFile::Versioned(versioned) => versioned
                .drawable_info
                .get("global")
                .and_then(|drawables| Self::collect_legacy_main_hud_layout(drawables)),
            LegacyMainHudLayoutFile::LegacyList(drawables) => {
                Self::collect_legacy_main_hud_layout(&drawables)
            }
        }
    }
    fn collect_legacy_main_hud_layout(
        drawables: &[LegacyMainHudDrawableInfo],
    ) -> Option<crate::types::LegacyHudLayout> {
        let mut layout = crate::types::LegacyHudLayout::default();
        for drawable in drawables {
            let component = crate::types::LegacyHudDrawableLayout {
                x: drawable.position.x,
                y: drawable.position.y,
                scale_x: drawable.scale.x,
                scale_y: drawable.scale.y,
                anchor: drawable.anchor,
                origin: drawable.origin,
            };
            if drawable.drawable_type.contains("LegacyScoreCounter") {
                layout.score = Some(component);
            } else if drawable.drawable_type.contains("LegacyAccuracyCounter") {
                layout.accuracy = Some(component);
            } else if drawable.drawable_type.contains("LegacySongProgress") {
                layout.song_progress = Some(component);
            }
        }
        if layout.score.is_none() && layout.accuracy.is_none() && layout.song_progress.is_none() {
            None
        } else {
            Some(layout)
        }
    }
    pub(crate) fn load_skin(
        &self,
        skin_path: Option<&Path>,
        _set_dir: &Path,
        target_keys: u8,
    ) -> Result<SkinAssets, ConvertError> {
        let target_keys = target_keys.max(1);
        let default_path = default_skin_path();
        let (default_skin, default_has_mania) = if default_path.exists() {
            self.load_skin_from_path(&default_path, target_keys)?
        } else {
            println!(
                "   [skin] warn: default skin not found at {}, falling back to ./skins/default",
                default_path.display()
            );
            let fallback = PathBuf::from("./skins/default");
            self.load_skin_from_path(&fallback, target_keys)?
        };
        println!(
            "   [skin] default: keys={} mania_section={}",
            target_keys, default_has_mania
        );
        if skin_path.is_none() {
            let (score_prefix, combo_prefix) = self.resolve_prefixes(&default_skin, &default_skin);
            let mut images = default_skin.images.clone();
            self.copy_required_assets(
                &mut images,
                &default_skin.images,
                &default_skin.images,
                target_keys,
                &score_prefix,
                &combo_prefix,
                default_skin.config.resolved_special_style(),
            );
            let config = self.merge_skin_config(
                &default_skin,
                &default_skin,
                target_keys,
                default_has_mania,
                true,
                &images,
                &score_prefix,
                &combo_prefix,
            );
            let mut skin = SkinAssets {
                config,
                images,
                legacy_hud_layout: default_skin.legacy_hud_layout.clone(),
            };
            self.ensure_image_aliases(&mut skin);
            return Ok(skin);
        }
        let custom_path = skin_path.unwrap();
        let (custom_skin, custom_has_mania) =
            match self.load_skin_from_path(custom_path, target_keys) {
                Ok(v) => v,
                Err(e) => {
                    println!(
                        "   [skin] warn: failed to load custom skin ({}), using default",
                        e
                    );
                    // A broken custom skin should not prevent rendering with the bundled fallback.
                    let (score_prefix, combo_prefix) =
                        self.resolve_prefixes(&default_skin, &default_skin);
                    let mut images = default_skin.images.clone();
                    self.copy_required_assets(
                        &mut images,
                        &default_skin.images,
                        &default_skin.images,
                        target_keys,
                        &score_prefix,
                        &combo_prefix,
                        default_skin.config.resolved_special_style(),
                    );
                    let config = self.merge_skin_config(
                        &default_skin,
                        &default_skin,
                        target_keys,
                        default_has_mania,
                        true,
                        &images,
                        &score_prefix,
                        &combo_prefix,
                    );
                    let mut skin = SkinAssets {
                        config,
                        images,
                        legacy_hud_layout: default_skin.legacy_hud_layout.clone(),
                    };
                    self.ensure_image_aliases(&mut skin);
                    return Ok(skin);
                }
            };
        println!(
            "   [skin] custom: keys={} mania_section={}",
            target_keys, custom_has_mania
        );
        let (score_prefix, combo_prefix) = self.resolve_prefixes(&custom_skin, &default_skin);
        let mut images = custom_skin.images.clone();
        self.copy_required_assets(
            &mut images,
            &custom_skin.images,
            &default_skin.images,
            target_keys,
            &score_prefix,
            &combo_prefix,
            custom_skin
                .config
                .special_style
                .or(default_skin.config.special_style)
                .unwrap_or_default(),
        );
        let config = self.merge_skin_config(
            &custom_skin,
            &default_skin,
            target_keys,
            custom_has_mania,
            !custom_has_mania,
            &images,
            &score_prefix,
            &combo_prefix,
        );
        let mut skin = SkinAssets {
            config,
            images,
            legacy_hud_layout: custom_skin
                .legacy_hud_layout
                .clone()
                .or_else(|| default_skin.legacy_hud_layout.clone()),
        };
        self.ensure_image_aliases(&mut skin);
        Ok(skin)
    }
    pub(crate) fn load_skin_from_path(
        &self,
        skin_path: &Path,
        target_keys: u8,
    ) -> Result<(SkinAssets, bool), ConvertError> {
        self.load_skin_from_path_with_limits(skin_path, target_keys, DEFAULT_SKIN_LOAD_LIMITS)
    }
    fn load_skin_from_path_with_limits(
        &self,
        skin_path: &Path,
        target_keys: u8,
        limits: SkinLoadLimits,
    ) -> Result<(SkinAssets, bool), ConvertError> {
        use std::io::BufReader;
        let mut skin = SkinAssets::new();
        let mut skin_ini_content: Option<String> = None;
        let mut main_hud_layout_content: Option<String> = None;
        let mut total_image_bytes = 0u64;
        let mut image_files = 0usize;
        if skin_path.exists() {
            if is_skin_archive_path(skin_path) {
                let file = fs::File::open(skin_path).map_err(ConvertError::Io)?;
                let reader = BufReader::new(file);
                let mut archive = zip::ZipArchive::new(reader)
                    .map_err(|err| ConvertError::Resolve(format!("invalid skin archive: {err}")))?;
                if archive.len() > limits.max_archive_entries {
                    return Err(ConvertError::Resolve(format!(
                        "skin archive has too many entries: {} > {}",
                        archive.len(),
                        limits.max_archive_entries
                    )));
                }
                let archive_root_prefix = detect_skin_archive_root_prefix(&mut archive)?;
                for i in 0..archive.len() {
                    let mut entry = archive.by_index(i).map_err(|err| {
                        ConvertError::Resolve(format!("skin archive entry error: {err}"))
                    })?;
                    if entry.is_dir() {
                        continue;
                    }
                    let raw_name = entry.name().to_string();
                    let Some(lower) = normalize_skin_archive_entry_name(
                        &raw_name,
                        archive_root_prefix.as_deref(),
                    )?
                    else {
                        continue;
                    };
                    if lower == "skin.ini" {
                        let content = read_skin_text_reader_with_limit(
                            &mut entry,
                            limits.max_metadata_bytes,
                            &raw_name,
                        )?;
                        skin_ini_content = Some(content);
                        continue;
                    }
                    if lower == "mainhudcomponents.json"
                        || lower.ends_with("/mainhudcomponents.json")
                    {
                        let content = read_skin_text_reader_with_limit(
                            &mut entry,
                            limits.max_metadata_bytes,
                            &raw_name,
                        )?;
                        main_hud_layout_content = Some(content);
                        continue;
                    }
                    if lower.ends_with(".png")
                        || lower.ends_with(".jpg")
                        || lower.ends_with(".jpeg")
                    {
                        image_files += 1;
                        if image_files > limits.max_image_files {
                            return Err(ConvertError::Resolve(format!(
                                "skin has too many image files: {} > {}",
                                image_files, limits.max_image_files
                            )));
                        }
                        let declared_size = entry.size();
                        if declared_size > limits.max_image_file_bytes {
                            println!(
                                "   [skin] warn: skipping oversized image {} ({} > {} bytes)",
                                raw_name, declared_size, limits.max_image_file_bytes
                            );
                            continue;
                        }
                        total_image_bytes = total_image_bytes
                            .checked_add(declared_size)
                            .ok_or_else(|| {
                                ConvertError::Resolve("skin image byte budget overflow".to_string())
                            })?;
                        if total_image_bytes > limits.max_total_image_bytes {
                            return Err(ConvertError::Resolve(format!(
                                "skin image bytes exceed total limit: {} > {} bytes",
                                total_image_bytes, limits.max_total_image_bytes
                            )));
                        }
                        let data = read_skin_binary_reader_with_limit(
                            &mut entry,
                            limits.max_image_file_bytes,
                            &raw_name,
                        )?;
                        if let Err(err) = validate_skin_image_bytes(&data, &raw_name, limits) {
                            println!("   [skin] warn: skipping image {} ({})", raw_name, err);
                            continue;
                        }
                        if let Err(err) = Self::insert_skin_image(&mut skin, lower, data, limits) {
                            println!("   [skin] warn: skipping image {} ({})", raw_name, err);
                            continue;
                        }
                    }
                }
            } else if skin_path.is_dir() {
                let ini_path = skin_path.join("skin.ini");
                if ini_path.exists() {
                    let content =
                        read_skin_text_file_with_limit(&ini_path, limits.max_metadata_bytes)?;
                    skin_ini_content = Some(content);
                }
                let layout_path = skin_path.join("MainHUDComponents.json");
                if layout_path.exists() {
                    let content =
                        read_skin_text_file_with_limit(&layout_path, limits.max_metadata_bytes)?;
                    main_hud_layout_content = Some(content);
                }
                let relative_files =
                    list_skin_files_recursive_with_limit(skin_path, limits.max_directory_files)?;
                for rel_name in relative_files {
                    let Some(ext) = Path::new(&rel_name).extension().and_then(|e| e.to_str())
                    else {
                        continue;
                    };
                    if !matches!(ext.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg") {
                        continue;
                    }
                    let path = skin_path.join(Path::new(&rel_name));
                    image_files += 1;
                    if image_files > limits.max_image_files {
                        return Err(ConvertError::Resolve(format!(
                            "skin has too many image files: {} > {}",
                            image_files, limits.max_image_files
                        )));
                    }
                    let file_len = fs::metadata(&path).map_err(ConvertError::Io)?.len();
                    if file_len > limits.max_image_file_bytes {
                        println!(
                            "   [skin] warn: skipping oversized image {} ({} > {} bytes)",
                            path.display(),
                            file_len,
                            limits.max_image_file_bytes
                        );
                        continue;
                    }
                    total_image_bytes =
                        total_image_bytes.checked_add(file_len).ok_or_else(|| {
                            ConvertError::Resolve("skin image byte budget overflow".to_string())
                        })?;
                    if total_image_bytes > limits.max_total_image_bytes {
                        return Err(ConvertError::Resolve(format!(
                            "skin image bytes exceed total limit: {} > {} bytes",
                            total_image_bytes, limits.max_total_image_bytes
                        )));
                    }
                    let data = fs::read(&path).map_err(ConvertError::Io)?;
                    if let Err(err) = validate_skin_image_bytes(&data, &rel_name, limits) {
                        println!("   [skin] warn: skipping image {} ({})", rel_name, err);
                        continue;
                    }
                    let key = SkinAssets::normalize_key(&rel_name);
                    if let Err(err) = Self::insert_skin_image(&mut skin, key, data, limits) {
                        println!("   [skin] warn: skipping image {} ({})", rel_name, err);
                        continue;
                    }
                }
            } else {
                return Err(ConvertError::Resolve(format!(
                    "skin path is not a directory or supported skin archive (.osk/.zip): {}",
                    skin_path.display()
                )));
            }
        } else {
            return Err(ConvertError::Resolve(format!(
                "skin path not found: {}",
                skin_path.display()
            )));
        }
        let mut has_target_mania_section = false;
        if let Some(content) = skin_ini_content {
            has_target_mania_section =
                self.parse_skin_ini_to_config(&content, &mut skin.config, target_keys);
        }
        skin.legacy_hud_layout = main_hud_layout_content
            .as_deref()
            .and_then(Self::parse_legacy_main_hud_layout);
        self.normalize_images(&mut skin, limits)?;
        self.infer_mania_assets_from_images(&mut skin, target_keys, !has_target_mania_section);
        if skin.image_count() == 0 {
            return Err(ConvertError::Resolve(format!(
                "skin has no images: {}",
                skin_path.display()
            )));
        }
        Ok((skin, has_target_mania_section))
    }
    fn infer_mania_assets_from_images(
        &self,
        skin: &mut SkinAssets,
        target_keys: u8,
        allow_stage_asset_inference: bool,
    ) {
        if target_keys == 4 {
            // Several dance-style 4K skins use arrow folder names instead of standard mania names.
            let directions = ["left", "down", "up", "right"];
            for (col, direction) in directions.iter().enumerate() {
                Self::fill_missing_skin_asset(
                    &mut skin.config.note_image,
                    col,
                    &[
                        format!("mania/arrows/{direction}"),
                        format!("mania/arrows/bluee/{direction}"),
                    ],
                    &skin.images,
                );
                Self::fill_missing_skin_asset(
                    &mut skin.config.key_image,
                    col,
                    &[
                        format!("mania/arrows/k_{direction}"),
                        format!("mania/arrows/key_{direction}"),
                        format!("mania/arrows/receptor_{direction}"),
                    ],
                    &skin.images,
                );
                Self::fill_missing_skin_asset(
                    &mut skin.config.key_image_d,
                    col,
                    &[
                        format!("mania/arrows/kd_{direction}"),
                        format!("mania/arrows/k_{direction}_d"),
                        format!("mania/arrows/k_{direction}_down"),
                    ],
                    &skin.images,
                );
            }
            for col in 0..target_keys as usize {
                Self::fill_missing_skin_asset(
                    &mut skin.config.note_image_h,
                    col,
                    &[
                        "mania/arrows/mania-note1h".to_string(),
                        "mania/arrows/recep old/mania-note1h".to_string(),
                    ],
                    &skin.images,
                );
                Self::fill_missing_skin_asset(
                    &mut skin.config.note_image_l,
                    col,
                    &["mania/arrows/mania-note1l".to_string()],
                    &skin.images,
                );
                Self::fill_missing_skin_asset(
                    &mut skin.config.note_image_t,
                    col,
                    &["mania/arrows/mania-note1t".to_string()],
                    &skin.images,
                );
            }
        }
        if !allow_stage_asset_inference {
            return;
        }
        if skin.config.stage_left.is_none() {
            skin.config.stage_left =
                Self::first_existing_skin_asset(&["mania-stage-left"], &skin.images);
        }
        if skin.config.stage_right.is_none() {
            skin.config.stage_right =
                Self::first_existing_skin_asset(&["mania-stage-right"], &skin.images);
        }
        if skin.config.stage_bottom.is_none() {
            skin.config.stage_bottom =
                Self::first_existing_skin_asset(&["mania-stage-bottom"], &skin.images);
        }
        if skin.config.stage_hint.is_none() {
            skin.config.stage_hint =
                Self::first_existing_skin_asset(&["mania-stage-hint"], &skin.images);
        }
        if skin.config.stage_light.is_none() {
            skin.config.stage_light =
                Self::first_existing_skin_asset(&["mania-stage-light"], &skin.images);
        }
        if skin.config.warning_arrow.is_none() {
            skin.config.warning_arrow =
                Self::first_existing_skin_asset(&["mania-warningarrow"], &skin.images);
        }
    }
    fn fill_missing_skin_asset(
        values: &mut Vec<String>,
        index: usize,
        candidates: &[String],
        images: &HashMap<String, Vec<u8>>,
    ) {
        while values.len() <= index {
            values.push(String::new());
        }
        if !values[index].trim().is_empty() {
            return;
        }
        for candidate in candidates {
            if let Some(asset_name) = Self::first_existing_skin_asset(&[candidate.as_str()], images)
            {
                values[index] = asset_name;
                return;
            }
        }
    }
    fn first_existing_skin_asset(
        candidates: &[&str],
        images: &HashMap<String, Vec<u8>>,
    ) -> Option<String> {
        candidates.iter().find_map(|candidate| {
            let normalized = SkinAssets::normalize_key(candidate);
            if images.contains_key(&normalized) {
                return Some(normalized);
            }
            let file_name = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
            if file_name.rsplit_once('.').is_some_and(|(_, ext)| {
                !ext.is_empty() && ext.chars().all(|c| c.is_ascii_alphanumeric())
            }) {
                return None;
            }
            [".png", ".jpg", ".jpeg"].iter().find_map(|extension| {
                let with_extension = format!("{normalized}{extension}");
                images
                    .contains_key(&with_extension)
                    .then_some(with_extension)
            })
        })
    }
    pub(crate) fn resolve_skin_audio_sources(
        &self,
        skin_path: Option<&Path>,
    ) -> (Vec<crate::video::AudioSearchSource>, Vec<TempDirGuard>) {
        let mut sources = Vec::new();
        let mut guards = Vec::new();
        let mut seen_roots = std::collections::HashSet::new();
        let mut try_push =
            |path: &Path, label: &str| match self.load_audio_source_from_skin_path(path) {
                Ok(Some((source, guard))) => {
                    // Custom and default skins can resolve to the same extracted root.
                    let root_key = source.root.to_string_lossy().to_ascii_lowercase();
                    if seen_roots.insert(root_key) {
                        sources.push(source);
                        if let Some(guard) = guard {
                            guards.push(guard);
                        }
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    println!(
                        "   [skin-audio] warn: failed to load {} audio source ({}), skipping",
                        label, err
                    );
                }
            };
        if let Some(custom_skin_path) = skin_path {
            try_push(custom_skin_path, "custom skin");
        }
        let default_path = default_skin_path();
        if default_path.exists() {
            try_push(&default_path, "default skin");
        }
        (sources, guards)
    }
    fn load_audio_source_from_skin_path(
        &self,
        skin_path: &Path,
    ) -> Result<Option<(crate::video::AudioSearchSource, Option<TempDirGuard>)>, ConvertError> {
        self.load_audio_source_from_skin_path_with_limits(skin_path, DEFAULT_SKIN_LOAD_LIMITS)
    }
    fn load_audio_source_from_skin_path_with_limits(
        &self,
        skin_path: &Path,
        limits: SkinLoadLimits,
    ) -> Result<Option<(crate::video::AudioSearchSource, Option<TempDirGuard>)>, ConvertError> {
        if !skin_path.exists() {
            return Ok(None);
        }
        if is_skin_archive_path(skin_path) {
            return self.extract_skin_audio_archive_with_limits(skin_path, limits);
        }
        if skin_path.is_dir() {
            let mut total_audio_bytes = 0u64;
            let mut audio_files = 0usize;
            let mut files = Vec::new();
            for name in list_skin_files_recursive_with_limit(skin_path, limits.max_directory_files)?
            {
                if !is_audio_filename(&name) {
                    continue;
                }
                audio_files += 1;
                if audio_files > limits.max_audio_files {
                    return Err(ConvertError::Resolve(format!(
                        "skin has too many audio files: {} > {}",
                        audio_files, limits.max_audio_files
                    )));
                }
                let path = skin_path.join(Path::new(&name));
                let file_len = fs::metadata(&path).map_err(ConvertError::Io)?.len();
                if file_len > limits.max_audio_file_bytes {
                    return Err(ConvertError::Resolve(format!(
                        "skin audio exceeds per-file limit: {} > {} bytes ({})",
                        file_len,
                        limits.max_audio_file_bytes,
                        path.display()
                    )));
                }
                total_audio_bytes = total_audio_bytes.checked_add(file_len).ok_or_else(|| {
                    ConvertError::Resolve("skin audio byte budget overflow".to_string())
                })?;
                if total_audio_bytes > limits.max_total_audio_bytes {
                    return Err(ConvertError::Resolve(format!(
                        "skin audio bytes exceed total limit: {} > {} bytes",
                        total_audio_bytes, limits.max_total_audio_bytes
                    )));
                }
                files.push(name);
            }
            if files.is_empty() {
                return Ok(None);
            }
            return Ok(Some((
                crate::video::AudioSearchSource::new(skin_path.to_path_buf(), files),
                None,
            )));
        }
        Ok(None)
    }
    fn extract_skin_audio_archive_with_limits(
        &self,
        skin_path: &Path,
        limits: SkinLoadLimits,
    ) -> Result<Option<(crate::video::AudioSearchSource, Option<TempDirGuard>)>, ConvertError> {
        let temp_dir = make_temp_skin_audio_dir();
        self.extract_skin_audio_archive_into_temp_dir_with_limits(skin_path, &temp_dir, limits)
    }
    fn extract_skin_audio_archive_into_temp_dir_with_limits(
        &self,
        skin_path: &Path,
        temp_dir: &Path,
        limits: SkinLoadLimits,
    ) -> Result<Option<(crate::video::AudioSearchSource, Option<TempDirGuard>)>, ConvertError> {
        use std::io::BufReader;
        let file = fs::File::open(skin_path).map_err(ConvertError::Io)?;
        let reader = BufReader::new(file);
        let mut archive = zip::ZipArchive::new(reader)
            .map_err(|err| ConvertError::Resolve(format!("invalid skin archive: {err}")))?;
        if archive.len() > limits.max_archive_entries {
            return Err(ConvertError::Resolve(format!(
                "skin archive has too many entries: {} > {}",
                archive.len(),
                limits.max_archive_entries
            )));
        }
        let archive_root_prefix = detect_skin_archive_root_prefix(&mut archive)?;
        let extraction = (|| {
            let mut extracted_files = Vec::new();
            let mut audio_files = 0usize;
            let mut total_audio_bytes = 0u64;
            for idx in 0..archive.len() {
                let mut entry = archive.by_index(idx).map_err(|err| {
                    ConvertError::Resolve(format!("skin archive entry error: {err}"))
                })?;
                if entry.is_dir() {
                    continue;
                }
                let raw_name = entry.name().to_string();
                let Some(normalized) =
                    normalize_skin_archive_entry_name(&raw_name, archive_root_prefix.as_deref())?
                else {
                    continue;
                };
                if !is_audio_filename(&normalized) {
                    continue;
                }
                audio_files += 1;
                if audio_files > limits.max_audio_files {
                    return Err(ConvertError::Resolve(format!(
                        "skin has too many audio files: {} > {}",
                        audio_files, limits.max_audio_files
                    )));
                }
                let declared_size = entry.size();
                if declared_size > limits.max_audio_file_bytes {
                    return Err(ConvertError::Resolve(format!(
                        "skin audio exceeds per-file limit: {} > {} bytes ({})",
                        declared_size, limits.max_audio_file_bytes, raw_name
                    )));
                }
                total_audio_bytes =
                    total_audio_bytes
                        .checked_add(declared_size)
                        .ok_or_else(|| {
                            ConvertError::Resolve("skin audio byte budget overflow".to_string())
                        })?;
                if total_audio_bytes > limits.max_total_audio_bytes {
                    return Err(ConvertError::Resolve(format!(
                        "skin audio bytes exceed total limit: {} > {} bytes",
                        total_audio_bytes, limits.max_total_audio_bytes
                    )));
                }
                let output_path = temp_dir.join(&normalized);
                if let Some(parent) = output_path.parent() {
                    fs::create_dir_all(parent).map_err(ConvertError::Io)?;
                }
                copy_reader_to_file_with_limit(
                    &mut entry,
                    &output_path,
                    limits.max_audio_file_bytes,
                    &raw_name,
                    "skin audio",
                )?;
                extracted_files.push(normalized);
            }
            if extracted_files.is_empty() {
                return Ok(None);
            }
            Ok(Some((
                crate::video::AudioSearchSource::new(temp_dir.to_path_buf(), extracted_files),
                Some(TempDirGuard::new(temp_dir.to_path_buf())),
            )))
        })();
        if extraction.is_err() {
            // Failed audio extraction must not leave a partial search source behind.
            let _ = fs::remove_dir_all(temp_dir);
        } else if matches!(extraction, Ok(None)) {
            let _ = fs::remove_dir_all(temp_dir);
        }
        extraction
    }
    pub(crate) fn resolve_prefixes(
        &self,
        custom: &SkinAssets,
        default: &SkinAssets,
    ) -> (String, String) {
        let default_score = Self::normalize_skin_prefix(default.config.score_prefix_or_default());
        let custom_score = if custom.config.score_prefix.is_empty() {
            default_score.clone()
        } else {
            Self::normalize_skin_prefix(&custom.config.score_prefix)
        };
        let score_prefix = if Self::prefix_has_digits(&custom_score, &custom.images) {
            custom_score
        } else if Self::prefix_has_digits(&default_score, &default.images) {
            default_score
        } else {
            "score".to_string()
        };
        let custom_combo = if custom.config.combo_prefix.is_empty() {
            String::new()
        } else {
            Self::normalize_skin_prefix(&custom.config.combo_prefix)
        };
        let combo_prefix =
            if !custom_combo.is_empty() && Self::prefix_has_digits(&custom_combo, &custom.images) {
                custom_combo
            } else {
                score_prefix.clone()
            };
        (score_prefix, combo_prefix)
    }
    pub(crate) fn prefix_has_digits(prefix: &str, images: &HashMap<String, Vec<u8>>) -> bool {
        let p = Self::normalize_skin_prefix(prefix);
        for d in 0..=9 {
            let name = format!("{}-{}", p, d);
            let name_png = format!("{}.png", name);
            if images.contains_key(&name) || images.contains_key(&name_png) {
                return true;
            }
        }
        false
    }
    pub(crate) fn copy_required_assets(
        &self,
        merged: &mut HashMap<String, Vec<u8>>,
        custom: &HashMap<String, Vec<u8>>,
        default: &HashMap<String, Vec<u8>>,
        target_keys: u8,
        score_prefix: &str,
        combo_prefix: &str,
        special_style: crate::types::SpecialStyle,
    ) {
        use std::collections::HashSet;
        let mut required: HashSet<String> = HashSet::new();
        for col in 0..target_keys {
            let family = self.map_skin_column_family(target_keys, col, special_style);
            let (note, head, body, tail) = self.standard_note_names(family);
            let (key, key_d) = self.standard_key_names(family);
            required.insert(note);
            required.insert(head);
            required.insert(body);
            required.insert(tail);
            required.insert(key);
            required.insert(key_d);
        }
        // Merge only required defaults so custom skins keep their own look where assets exist.
        for name in required {
            if merged.contains_key(&name) {
                continue;
            }
            if let Some(data) = default.get(&name) {
                merged.insert(name, data.clone());
            }
        }
        let judgment_bases = [
            "mania-hit0",
            "mania-hit50",
            "mania-hit100",
            "mania-hit200",
            "mania-hit300",
            "mania-hit300g",
        ];
        for base in judgment_bases {
            if Self::has_judgment(custom, base) {
                continue;
            }
            for (key, data) in default.iter() {
                if Self::is_judgment_key(key, base) {
                    merged.entry(key.clone()).or_insert_with(|| data.clone());
                }
            }
        }
        self.copy_digit_assets(score_prefix, merged, default, true);
        self.copy_digit_assets(combo_prefix, merged, default, false);
        for family in [
            "mania-stage-left",
            "mania-stage-right",
            "mania-stage-bottom",
            "mania-stage-hint",
            "mania-stage-light",
            "mania-warningarrow",
            "lightingn",
            "lightingl",
        ] {
            self.copy_family_assets_if_missing(family, merged, custom, default);
        }
        for family in [
            "selection-mod-nofail",
            "selection-mod-easy",
            "selection-mod-hidden",
            "selection-mod-hardrock",
            "selection-mod-suddendeath",
            "selection-mod-perfect",
            "selection-mod-doubletime",
            "selection-mod-nightcore",
            "selection-mod-halftime",
            "selection-mod-flashlight",
            "selection-mod-fadein",
            "selection-mod-mirror",
            "selection-mod-scorev2",
            "selection-mod-autoplay",
            "selection-mod-keycoop",
        ] {
            self.copy_family_assets_if_missing(family, merged, custom, default);
        }
        if !self.images_have_prefix(custom, "comboburst-mania")
            && !self.images_have_prefix(custom, "comboburst")
        {
            if self.images_have_prefix(default, "comboburst-mania") {
                self.copy_prefixed_assets("comboburst-mania", merged, default);
            } else {
                self.copy_prefixed_assets("comboburst", merged, default);
            }
        }
    }
    pub(crate) fn has_judgment(images: &HashMap<String, Vec<u8>>, base: &str) -> bool {
        let base = base.to_lowercase();
        images.keys().any(|k| k.starts_with(&base))
    }
    pub(crate) fn is_judgment_key(key: &str, base: &str) -> bool {
        let key = key.to_lowercase();
        let base = base.to_lowercase();
        if !key.starts_with(&base) {
            return false;
        }
        let rest = &key[base.len()..];
        if rest.is_empty() {
            return true;
        }
        rest.starts_with(".png")
            || rest.starts_with(".jpg")
            || rest.starts_with('-')
            || rest.starts_with("@")
    }
    pub(crate) fn copy_digit_assets(
        &self,
        prefix: &str,
        merged: &mut HashMap<String, Vec<u8>>,
        default: &HashMap<String, Vec<u8>>,
        include_score_punct: bool,
    ) {
        let p = prefix.to_lowercase();
        for d in 0..=9 {
            let name = format!("{}-{}", p, d);
            let name_png = format!("{}.png", name);
            if !merged.contains_key(&name) {
                if let Some(data) = default.get(&name) {
                    merged.insert(name.clone(), data.clone());
                } else if let Some(data) = default.get(&name_png) {
                    merged.insert(name_png.clone(), data.clone());
                }
            }
            if !merged.contains_key(&name_png) {
                if let Some(data) = default.get(&name_png) {
                    merged.insert(name_png.clone(), data.clone());
                } else if let Some(data) = default.get(&name) {
                    merged.insert(name.clone(), data.clone());
                }
            }
        }
        if include_score_punct {
            let extras = ["comma", "dot", "percent"];
            for extra in extras {
                let name = format!("{}-{}", p, extra);
                let name_png = format!("{}.png", name);
                if !merged.contains_key(&name) {
                    if let Some(data) = default.get(&name) {
                        merged.insert(name.clone(), data.clone());
                    } else if let Some(data) = default.get(&name_png) {
                        merged.insert(name_png.clone(), data.clone());
                    }
                }
                if !merged.contains_key(&name_png) {
                    if let Some(data) = default.get(&name_png) {
                        merged.insert(name_png.clone(), data.clone());
                    } else if let Some(data) = default.get(&name) {
                        merged.insert(name.clone(), data.clone());
                    }
                }
            }
        } else {
            let name = format!("{}-x", p);
            let name_png = format!("{}.png", name);
            if !merged.contains_key(&name) {
                if let Some(data) = default.get(&name) {
                    merged.insert(name.clone(), data.clone());
                } else if let Some(data) = default.get(&name_png) {
                    merged.insert(name_png.clone(), data.clone());
                }
            }
            if !merged.contains_key(&name_png) {
                if let Some(data) = default.get(&name_png) {
                    merged.insert(name_png.clone(), data.clone());
                } else if let Some(data) = default.get(&name) {
                    merged.insert(name.clone(), data.clone());
                }
            }
        }
    }
    fn images_have_prefix(&self, images: &HashMap<String, Vec<u8>>, prefix: &str) -> bool {
        let prefix = prefix.to_ascii_lowercase();
        images.keys().any(|key| {
            let key = key.to_ascii_lowercase();
            key == prefix
                || key == format!("{prefix}.png")
                || key == format!("{prefix}.jpg")
                || key == format!("{prefix}.jpeg")
                || key.starts_with(&format!("{prefix}-"))
                || key.starts_with(&format!("{prefix}@"))
                || key.starts_with(&format!("{prefix}/"))
        })
    }
    fn copy_prefixed_assets(
        &self,
        prefix: &str,
        merged: &mut HashMap<String, Vec<u8>>,
        source: &HashMap<String, Vec<u8>>,
    ) {
        let prefix = prefix.to_ascii_lowercase();
        for (key, data) in source {
            let key_lower = key.to_ascii_lowercase();
            if key_lower == prefix
                || key_lower == format!("{prefix}.png")
                || key_lower == format!("{prefix}.jpg")
                || key_lower == format!("{prefix}.jpeg")
                || key_lower.starts_with(&format!("{prefix}-"))
                || key_lower.starts_with(&format!("{prefix}@"))
                || key_lower.starts_with(&format!("{prefix}/"))
            {
                merged.entry(key.clone()).or_insert_with(|| data.clone());
            }
        }
    }
    fn copy_family_assets_if_missing(
        &self,
        family: &str,
        merged: &mut HashMap<String, Vec<u8>>,
        custom: &HashMap<String, Vec<u8>>,
        default: &HashMap<String, Vec<u8>>,
    ) {
        if self.images_have_prefix(custom, family) {
            return;
        }
        self.copy_prefixed_assets(family, merged, default);
    }
    pub(crate) fn merge_skin_config(
        &self,
        custom: &SkinAssets,
        default: &SkinAssets,
        target_keys: u8,
        custom_has_mania: bool,
        allow_default_optional_stage_assets: bool,
        merged_images: &HashMap<String, Vec<u8>>,
        score_prefix: &str,
        combo_prefix: &str,
    ) -> crate::types::SkinConfig {
        let mut cfg = custom.config.clone();
        let column_count = target_keys as usize;
        cfg.column_start = Self::merge_option(cfg.column_start, default.config.column_start);
        cfg.column_right = Self::merge_option(cfg.column_right, default.config.column_right);
        cfg.column_widths = Self::merge_i32_vec(
            cfg.column_widths.as_deref(),
            default.config.column_widths.as_deref(),
            column_count,
        );
        cfg.column_spacing = Self::merge_i32_vec(
            cfg.column_spacing.as_deref(),
            default.config.column_spacing.as_deref(),
            column_count.saturating_sub(1),
        );
        cfg.hit_position = Self::merge_option(cfg.hit_position, default.config.hit_position);
        if !custom_has_mania {
            cfg.keys_under_notes = default.config.keys_under_notes;
        }
        cfg.combo_pos_y = Self::merge_option(cfg.combo_pos_y, default.config.combo_pos_y);
        cfg.score_y = Self::merge_option(cfg.score_y, default.config.score_y);
        cfg.column_colors = Self::merge_option_color_vec(
            &cfg.column_colors,
            &default.config.column_colors,
            column_count,
        );
        cfg.colour_light = Self::merge_option_color_vec(
            &cfg.colour_light,
            &default.config.colour_light,
            column_count,
        );
        cfg.colour_column_line =
            Self::merge_option(cfg.colour_column_line, default.config.colour_column_line);
        cfg.colour_barline = Self::merge_option(cfg.colour_barline, default.config.colour_barline);
        cfg.colour_judgement_line = Self::merge_option(
            cfg.colour_judgement_line,
            default.config.colour_judgement_line,
        );
        cfg.colour_key_warning =
            Self::merge_option(cfg.colour_key_warning, default.config.colour_key_warning);
        cfg.colour_hold = Self::merge_option(cfg.colour_hold, default.config.colour_hold);
        cfg.colour_break = Self::merge_option(cfg.colour_break, default.config.colour_break);
        cfg.light_position = Self::merge_option(cfg.light_position, default.config.light_position);
        cfg.light_frame_per_second = Self::merge_option(
            cfg.light_frame_per_second,
            default.config.light_frame_per_second,
        );
        cfg.judgement_line = Self::merge_option(cfg.judgement_line, default.config.judgement_line);
        cfg.upside_down = Self::merge_option(cfg.upside_down, default.config.upside_down);
        cfg.combo_burst_style =
            Self::merge_option(cfg.combo_burst_style, default.config.combo_burst_style);
        cfg.combo_burst_random =
            Self::merge_option(cfg.combo_burst_random, default.config.combo_burst_random);
        cfg.special_style = Self::merge_option(cfg.special_style, default.config.special_style);
        cfg.lighting_n_width = Self::merge_i32_vec(
            cfg.lighting_n_width.as_deref(),
            default.config.lighting_n_width.as_deref(),
            column_count,
        );
        cfg.lighting_l_width = Self::merge_i32_vec(
            cfg.lighting_l_width.as_deref(),
            default.config.lighting_l_width.as_deref(),
            column_count,
        );
        cfg.column_line_width = Self::merge_f32_vec(
            cfg.column_line_width.as_deref(),
            default.config.column_line_width.as_deref(),
            column_count.saturating_sub(1),
        );
        cfg.barline_height = Self::merge_option(cfg.barline_height, default.config.barline_height);
        cfg.key_flip_when_upside_down = Self::merge_option(
            cfg.key_flip_when_upside_down,
            default.config.key_flip_when_upside_down,
        );
        cfg.key_flip_when_upside_down_col = Self::merge_option_bool_vec(
            &cfg.key_flip_when_upside_down_col,
            &default.config.key_flip_when_upside_down_col,
            column_count,
        );
        cfg.key_flip_when_upside_down_pressed = Self::merge_option_bool_vec(
            &cfg.key_flip_when_upside_down_pressed,
            &default.config.key_flip_when_upside_down_pressed,
            column_count,
        );
        cfg.note_flip_when_upside_down = Self::merge_option(
            cfg.note_flip_when_upside_down,
            default.config.note_flip_when_upside_down,
        );
        cfg.note_flip_when_upside_down_col = Self::merge_option_bool_vec(
            &cfg.note_flip_when_upside_down_col,
            &default.config.note_flip_when_upside_down_col,
            column_count,
        );
        cfg.note_flip_when_upside_down_h = Self::merge_option_bool_vec(
            &cfg.note_flip_when_upside_down_h,
            &default.config.note_flip_when_upside_down_h,
            column_count,
        );
        cfg.note_flip_when_upside_down_l = Self::merge_option_bool_vec(
            &cfg.note_flip_when_upside_down_l,
            &default.config.note_flip_when_upside_down_l,
            column_count,
        );
        cfg.note_flip_when_upside_down_t = Self::merge_option_bool_vec(
            &cfg.note_flip_when_upside_down_t,
            &default.config.note_flip_when_upside_down_t,
            column_count,
        );
        cfg.note_body_style =
            Self::merge_option(cfg.note_body_style, default.config.note_body_style);
        cfg.note_body_style_col = Self::merge_option_note_body_style_vec(
            &cfg.note_body_style_col,
            &default.config.note_body_style_col,
            column_count,
        );
        cfg.score_prefix = Self::normalize_skin_prefix(score_prefix);
        cfg.combo_prefix = Self::normalize_skin_prefix(combo_prefix);
        if cfg.hit_prefix.is_empty() {
            cfg.hit_prefix = default.config.hit_prefix.clone();
        }
        cfg.animation_framerate =
            Self::merge_option(cfg.animation_framerate, default.config.animation_framerate);
        cfg.score_overlap = Self::merge_option(cfg.score_overlap, default.config.score_overlap);
        cfg.combo_overlap = Self::merge_option(cfg.combo_overlap, default.config.combo_overlap);
        cfg.note_padding_x = if cfg.note_padding_x != 0 || default.config.note_padding_x == 0 {
            cfg.note_padding_x
        } else {
            default.config.note_padding_x
        };
        cfg.width_for_note_height_scale = Self::merge_option(
            cfg.width_for_note_height_scale,
            default.config.width_for_note_height_scale,
        );
        let allow_optional_stage_fallback = !custom_has_mania;
        cfg.stage_left = self.pick_optional_stage_asset_name(
            custom.config.stage_left.as_ref(),
            default.config.stage_left.as_ref(),
            "mania-stage-left",
            &custom.images,
            &default.images,
            merged_images,
            allow_default_optional_stage_assets,
            allow_optional_stage_fallback,
        );
        cfg.stage_right = self.pick_optional_stage_asset_name(
            custom.config.stage_right.as_ref(),
            default.config.stage_right.as_ref(),
            "mania-stage-right",
            &custom.images,
            &default.images,
            merged_images,
            allow_default_optional_stage_assets,
            allow_optional_stage_fallback,
        );
        cfg.stage_bottom = self.pick_optional_stage_asset_name(
            custom.config.stage_bottom.as_ref(),
            default.config.stage_bottom.as_ref(),
            "mania-stage-bottom",
            &custom.images,
            &default.images,
            merged_images,
            allow_default_optional_stage_assets,
            allow_optional_stage_fallback,
        );
        cfg.stage_hint = self.pick_optional_stage_asset_name(
            custom.config.stage_hint.as_ref(),
            default.config.stage_hint.as_ref(),
            "mania-stage-hint",
            &custom.images,
            &default.images,
            merged_images,
            allow_default_optional_stage_assets,
            allow_optional_stage_fallback,
        );
        cfg.stage_light = self.pick_optional_stage_asset_name(
            custom.config.stage_light.as_ref(),
            default.config.stage_light.as_ref(),
            "mania-stage-light",
            &custom.images,
            &default.images,
            merged_images,
            allow_default_optional_stage_assets,
            allow_optional_stage_fallback,
        );
        cfg.warning_arrow = self.pick_optional_stage_asset_name(
            custom.config.warning_arrow.as_ref(),
            default.config.warning_arrow.as_ref(),
            "mania-warningarrow",
            &custom.images,
            &default.images,
            merged_images,
            allow_default_optional_stage_assets,
            allow_optional_stage_fallback,
        );
        cfg.lighting_n = self
            .pick_asset_name(
                custom.config.lighting_n.first(),
                default.config.lighting_n.first(),
                "lightingn",
                merged_images,
            )
            .into_iter()
            .collect();
        cfg.lighting_l = self
            .pick_asset_name(
                custom.config.lighting_l.first(),
                default.config.lighting_l.first(),
                "lightingl",
                merged_images,
            )
            .into_iter()
            .collect();
        cfg.combo_burst = self.resolve_combo_burst_assets(merged_images);
        cfg.hit_0 = self.pick_asset_name(
            custom.config.hit_0.as_ref(),
            default.config.hit_0.as_ref(),
            "mania-hit0",
            merged_images,
        );
        cfg.hit_50 = self.pick_asset_name(
            custom.config.hit_50.as_ref(),
            default.config.hit_50.as_ref(),
            "mania-hit50",
            merged_images,
        );
        cfg.hit_100 = self.pick_asset_name(
            custom.config.hit_100.as_ref(),
            default.config.hit_100.as_ref(),
            "mania-hit100",
            merged_images,
        );
        cfg.hit_200 = self.pick_asset_name(
            custom.config.hit_200.as_ref(),
            default.config.hit_200.as_ref(),
            "mania-hit200",
            merged_images,
        );
        cfg.hit_300 = self.pick_asset_name(
            custom.config.hit_300.as_ref(),
            default.config.hit_300.as_ref(),
            "mania-hit300",
            merged_images,
        );
        cfg.hit_300g = self.pick_asset_name(
            custom.config.hit_300g.as_ref(),
            default.config.hit_300g.as_ref(),
            "mania-hit300g",
            merged_images,
        );
        let special_style = cfg.resolved_special_style();
        let (note_image, note_image_h, note_image_l, note_image_t, key_image, key_image_d) =
            self.resolve_note_key_lists(custom, default, merged_images, target_keys, special_style);
        cfg.note_image = note_image;
        cfg.note_image_h = note_image_h;
        cfg.note_image_l = note_image_l;
        cfg.note_image_t = note_image_t;
        cfg.key_image = key_image;
        cfg.key_image_d = key_image_d;
        cfg
    }
    fn merge_option<T: Copy>(custom: Option<T>, default: Option<T>) -> Option<T> {
        custom.or(default)
    }
    fn merge_i32_vec(
        custom: Option<&[i32]>,
        default: Option<&[i32]>,
        len: usize,
    ) -> Option<Vec<i32>> {
        let mut result = Vec::with_capacity(len);
        let mut has_any = false;
        for idx in 0..len {
            let value = custom
                .and_then(|values| values.get(idx))
                .copied()
                .or_else(|| default.and_then(|values| values.get(idx)).copied());
            if let Some(value) = value {
                has_any = true;
                result.push(value);
            } else {
                break;
            }
        }
        has_any.then_some(result)
    }
    fn merge_f32_vec(
        custom: Option<&[f32]>,
        default: Option<&[f32]>,
        len: usize,
    ) -> Option<Vec<f32>> {
        let mut result = Vec::with_capacity(len);
        let mut has_any = false;
        for idx in 0..len {
            let value = custom
                .and_then(|values| values.get(idx))
                .copied()
                .or_else(|| default.and_then(|values| values.get(idx)).copied());
            if let Some(value) = value {
                has_any = true;
                result.push(value);
            } else {
                break;
            }
        }
        has_any.then_some(result)
    }
    fn merge_option_bool_vec(
        custom: &[Option<bool>],
        default: &[Option<bool>],
        len: usize,
    ) -> Vec<Option<bool>> {
        (0..len)
            .map(|idx| {
                custom
                    .get(idx)
                    .copied()
                    .flatten()
                    .or_else(|| default.get(idx).copied().flatten())
            })
            .collect()
    }
    fn merge_option_color_vec(
        custom: &[Option<[f32; 4]>],
        default: &[Option<[f32; 4]>],
        len: usize,
    ) -> Vec<Option<[f32; 4]>> {
        (0..len)
            .map(|idx| {
                custom
                    .get(idx)
                    .copied()
                    .flatten()
                    .or_else(|| default.get(idx).copied().flatten())
            })
            .collect()
    }
    fn merge_option_note_body_style_vec(
        custom: &[Option<crate::types::NoteBodyStyle>],
        default: &[Option<crate::types::NoteBodyStyle>],
        len: usize,
    ) -> Vec<Option<crate::types::NoteBodyStyle>> {
        (0..len)
            .map(|idx| {
                custom
                    .get(idx)
                    .copied()
                    .flatten()
                    .or_else(|| default.get(idx).copied().flatten())
            })
            .collect()
    }
    fn resolve_combo_burst_assets(&self, merged_images: &HashMap<String, Vec<u8>>) -> Vec<String> {
        let mut result = Self::collect_numbered_asset_set(merged_images, "comboburst-mania");
        if result.is_empty() {
            result = Self::collect_numbered_asset_set(merged_images, "comboburst");
        }
        result
    }
    fn collect_numbered_asset_set(images: &HashMap<String, Vec<u8>>, base: &str) -> Vec<String> {
        let base = base.to_ascii_lowercase();
        let mut result = Vec::new();
        if images.contains_key(&format!("{base}.png")) {
            result.push(format!("{base}.png"));
        }
        for idx in 0..256 {
            let candidate = format!("{base}-{idx}.png");
            if images.contains_key(&candidate) {
                result.push(candidate);
            } else if idx > 0 {
                break;
            }
        }
        result
    }
    fn resolve_existing_asset_name(
        &self,
        name: &str,
        merged_images: &HashMap<String, Vec<u8>>,
    ) -> Option<String> {
        self.resolve_existing_asset_name_in(name, merged_images)
    }
    fn resolve_existing_asset_name_in(
        &self,
        name: &str,
        images: &HashMap<String, Vec<u8>>,
    ) -> Option<String> {
        let norm = SkinAssets::normalize_key(name);
        if images.contains_key(&norm) {
            return Some(norm);
        }
        let stem = norm
            .trim_end_matches(".png")
            .trim_end_matches(".jpg")
            .trim_end_matches(".jpeg");
        for start in [0, 1] {
            for ext in ["png", "jpg", "jpeg"] {
                let candidate = format!("{stem}-{start}.{ext}");
                if images.contains_key(&candidate) {
                    return Some(candidate);
                }
            }
        }
        None
    }
    fn resolve_column_lookup_asset(
        &self,
        configured: Option<&String>,
        fallback: &str,
        images: &HashMap<String, Vec<u8>>,
    ) -> Option<String> {
        if let Some(name) = configured
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            return self.resolve_existing_asset_name_in(name, images);
        }
        self.resolve_existing_asset_name_in(fallback, images)
    }
    fn pick_column_image_chain(
        &self,
        custom_chain: &[(Option<&String>, &str)],
        default_chain: &[(Option<&String>, &str)],
        merged_images: &HashMap<String, Vec<u8>>,
        custom_images: &HashMap<String, Vec<u8>>,
        default_images: &HashMap<String, Vec<u8>>,
    ) -> String {
        for (configured, fallback) in custom_chain {
            if let Some(found) =
                self.resolve_column_lookup_asset(*configured, fallback, custom_images)
            {
                return found;
            }
        }
        for (configured, fallback) in default_chain {
            if let Some(found) =
                self.resolve_column_lookup_asset(*configured, fallback, default_images)
            {
                return found;
            }
        }
        let fallback = custom_chain
            .first()
            .map(|(_, fallback)| *fallback)
            .unwrap_or("mania-note1");
        self.resolve_existing_asset_name(fallback, merged_images)
            .unwrap_or_else(|| SkinAssets::normalize_key(fallback))
    }
    fn resolve_configured_gameplay_asset(
        &self,
        name: Option<&String>,
        merged_images: &HashMap<String, Vec<u8>>,
    ) -> GameplayAssetResolution {
        let Some(name) = name
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        else {
            return GameplayAssetResolution::Unspecified;
        };
        self.resolve_existing_asset_name(name, merged_images)
            .map(GameplayAssetResolution::Resolved)
            .unwrap_or(GameplayAssetResolution::MissingExplicit)
    }
    pub(crate) fn pick_asset_name(
        &self,
        custom: Option<&String>,
        default: Option<&String>,
        fallback_base: &str,
        merged_images: &HashMap<String, Vec<u8>>,
    ) -> Option<String> {
        self.pick_asset_name_with_generic_fallback(
            custom,
            default,
            fallback_base,
            merged_images,
            true,
        )
    }
    fn pick_optional_stage_asset_name(
        &self,
        custom: Option<&String>,
        default: Option<&String>,
        fallback_base: &str,
        custom_images: &HashMap<String, Vec<u8>>,
        default_images: &HashMap<String, Vec<u8>>,
        merged_images: &HashMap<String, Vec<u8>>,
        allow_default: bool,
        allow_default_generic_fallback: bool,
    ) -> Option<String> {
        match self.resolve_configured_gameplay_asset(custom, merged_images) {
            GameplayAssetResolution::Resolved(found) => return Some(found),
            // An explicit missing stage asset means "do not fall back to a generic stage."
            GameplayAssetResolution::MissingExplicit => return Some(String::new()),
            GameplayAssetResolution::Unspecified => {}
        }
        if allow_default {
            if let GameplayAssetResolution::Resolved(found) =
                self.resolve_configured_gameplay_asset(default, merged_images)
            {
                return Some(found);
            }
        }
        let fallback = self.convert_skin_path(fallback_base);
        if let Some(found) = self.resolve_existing_asset_name_in(&fallback, custom_images) {
            return Some(found);
        }
        if allow_default_generic_fallback {
            return self.resolve_existing_asset_name_in(&fallback, default_images);
        }
        None
    }
    fn pick_asset_name_with_generic_fallback(
        &self,
        custom: Option<&String>,
        default: Option<&String>,
        fallback_base: &str,
        merged_images: &HashMap<String, Vec<u8>>,
        allow_generic_fallback: bool,
    ) -> Option<String> {
        match self.resolve_configured_gameplay_asset(custom, merged_images) {
            GameplayAssetResolution::Resolved(found) => return Some(found),
            // Preserve explicit missing gameplay assets as empty names instead of guessing.
            GameplayAssetResolution::MissingExplicit => return Some(String::new()),
            GameplayAssetResolution::Unspecified => {}
        }
        if let GameplayAssetResolution::Resolved(found) =
            self.resolve_configured_gameplay_asset(default, merged_images)
        {
            return Some(found);
        }
        if !allow_generic_fallback {
            return None;
        }
        let fallback = self.convert_skin_path(fallback_base);
        self.resolve_existing_asset_name(&fallback, merged_images)
    }
    pub(crate) fn resolve_note_key_lists(
        &self,
        custom: &SkinAssets,
        default: &SkinAssets,
        merged_images: &HashMap<String, Vec<u8>>,
        target_keys: u8,
        special_style: crate::types::SpecialStyle,
    ) -> (
        Vec<String>,
        Vec<String>,
        Vec<String>,
        Vec<String>,
        Vec<String>,
        Vec<String>,
    ) {
        let mut note_image = Vec::with_capacity(target_keys as usize);
        let mut note_image_h = Vec::with_capacity(target_keys as usize);
        let mut note_image_l = Vec::with_capacity(target_keys as usize);
        let mut note_image_t = Vec::with_capacity(target_keys as usize);
        let mut key_image = Vec::with_capacity(target_keys as usize);
        let mut key_image_d = Vec::with_capacity(target_keys as usize);
        for col in 0..target_keys as usize {
            let family = self.map_skin_column_family(target_keys, col as u8, special_style);
            let (fb_note, fb_head, fb_body, fb_tail) = self.standard_note_names(family);
            let (fb_key, fb_key_d) = self.standard_key_names(family);
            note_image.push(self.pick_column_image_chain(
                &[(custom.config.note_image.get(col), fb_note.as_str())],
                &[(default.config.note_image.get(col), fb_note.as_str())],
                merged_images,
                &custom.images,
                &default.images,
            ));
            note_image_h.push(self.pick_column_image_chain(
                &[
                    (custom.config.note_image_h.get(col), fb_head.as_str()),
                    (custom.config.note_image.get(col), fb_note.as_str()),
                ],
                &[
                    (default.config.note_image_h.get(col), fb_head.as_str()),
                    (default.config.note_image.get(col), fb_note.as_str()),
                ],
                merged_images,
                &custom.images,
                &default.images,
            ));
            note_image_l.push(self.pick_column_image_chain(
                &[(custom.config.note_image_l.get(col), fb_body.as_str())],
                &[(default.config.note_image_l.get(col), fb_body.as_str())],
                merged_images,
                &custom.images,
                &default.images,
            ));
            note_image_t.push(self.pick_column_image_chain(
                &[
                    (custom.config.note_image_t.get(col), fb_tail.as_str()),
                    (custom.config.note_image_h.get(col), fb_head.as_str()),
                    (custom.config.note_image.get(col), fb_note.as_str()),
                ],
                &[
                    (default.config.note_image_t.get(col), fb_tail.as_str()),
                    (default.config.note_image_h.get(col), fb_head.as_str()),
                    (default.config.note_image.get(col), fb_note.as_str()),
                ],
                merged_images,
                &custom.images,
                &default.images,
            ));
            key_image.push(self.pick_column_image_chain(
                &[(custom.config.key_image.get(col), fb_key.as_str())],
                &[(default.config.key_image.get(col), fb_key.as_str())],
                merged_images,
                &custom.images,
                &default.images,
            ));
            key_image_d.push(self.pick_column_image_chain(
                &[(custom.config.key_image_d.get(col), fb_key_d.as_str())],
                &[(default.config.key_image_d.get(col), fb_key_d.as_str())],
                merged_images,
                &custom.images,
                &default.images,
            ));
        }
        (
            note_image,
            note_image_h,
            note_image_l,
            note_image_t,
            key_image,
            key_image_d,
        )
    }
    pub(crate) fn map_skin_column_family(
        &self,
        key_count: u8,
        column: u8,
        special_style: crate::types::SpecialStyle,
    ) -> ManiaFamily {
        let column_index = column as usize;
        let key_count_usize = key_count as usize;
        if key_count > 4 && key_count.is_multiple_of(2) {
            let left_special = key_count_usize / 2 - 1;
            let right_special = key_count_usize / 2;
            let use_special = match special_style {
                crate::types::SpecialStyle::None => false,
                crate::types::SpecialStyle::Left => column_index == left_special,
                crate::types::SpecialStyle::Right => column_index == right_special,
            };
            if use_special {
                return ManiaFamily::Special;
            }
        }
        map_column_family(key_count, column, None)
    }
    pub(crate) fn standard_note_names(
        &self,
        family: ManiaFamily,
    ) -> (String, String, String, String) {
        let (base, head, body, tail) = match family {
            ManiaFamily::One => (
                "mania-note1",
                "mania-note1H",
                "mania-note1L",
                "mania-note1T",
            ),
            ManiaFamily::Two => (
                "mania-note2",
                "mania-note2H",
                "mania-note2L",
                "mania-note2T",
            ),
            ManiaFamily::Special => (
                "mania-noteS",
                "mania-noteSH",
                "mania-noteSL",
                "mania-noteST",
            ),
        };
        (
            self.convert_skin_path(base),
            self.convert_skin_path(head),
            self.convert_skin_path(body),
            self.convert_skin_path(tail),
        )
    }
    pub(crate) fn standard_key_names(&self, family: ManiaFamily) -> (String, String) {
        let (base, down) = match family {
            ManiaFamily::One => ("mania-key1", "mania-key1D"),
            ManiaFamily::Two => ("mania-key2", "mania-key2D"),
            ManiaFamily::Special => ("mania-keyS", "mania-keySD"),
        };
        (self.convert_skin_path(base), self.convert_skin_path(down))
    }
    fn parse_skin_bool(value: &str) -> Option<bool> {
        match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" => Some(true),
            "0" | "false" | "no" => Some(false),
            _ => None,
        }
    }
    fn strip_skin_inline_comment(value: &str) -> &str {
        value.find("//").map_or(value, |index| &value[..index])
    }
    fn parse_i32_list(value: &str) -> Vec<i32> {
        Self::strip_skin_inline_comment(value)
            .split(',')
            .filter_map(|part| part.trim().parse::<i32>().ok())
            .collect()
    }
    fn parse_f32_list(value: &str) -> Vec<f32> {
        Self::strip_skin_inline_comment(value)
            .split(',')
            .filter_map(|part| part.trim().parse::<f32>().ok())
            .collect()
    }
    fn parse_combo_burst_style(value: &str) -> Option<crate::types::ComboBurstStyle> {
        match value.trim().to_ascii_lowercase().as_str() {
            "0" | "left" => Some(crate::types::ComboBurstStyle::Left),
            "1" | "right" => Some(crate::types::ComboBurstStyle::Right),
            "2" | "both" => Some(crate::types::ComboBurstStyle::Both),
            _ => None,
        }
    }
    fn parse_special_style(value: &str) -> Option<crate::types::SpecialStyle> {
        match value.trim().to_ascii_lowercase().as_str() {
            "0" | "none" => Some(crate::types::SpecialStyle::None),
            "1" | "left" | "outer" => Some(crate::types::SpecialStyle::Left),
            "2" | "right" | "inner" => Some(crate::types::SpecialStyle::Right),
            _ => None,
        }
    }
    fn parse_note_body_style(value: &str) -> Option<crate::types::NoteBodyStyle> {
        match value.trim().to_ascii_lowercase().as_str() {
            "0" | "stretch" => Some(crate::types::NoteBodyStyle::Stretch),
            "1" | "repeatbottom" | "bottom" => Some(crate::types::NoteBodyStyle::RepeatBottom),
            "2" | "repeattop" | "top" => Some(crate::types::NoteBodyStyle::RepeatTop),
            "3" | "repeattopandbottom" | "topandbottom" | "both" => {
                Some(crate::types::NoteBodyStyle::RepeatTopAndBottom)
            }
            _ => None,
        }
    }
    pub(crate) fn parse_skin_ini_to_config(
        &self,
        content: &str,
        config: &mut crate::types::SkinConfig,
        target_keys: u8,
    ) -> bool {
        let mut current_section = String::new();
        let mut in_target_mania_section = false;
        let mut has_target_mania_section = false;
        let target_keys_i32 = target_keys as i32;
        for line in content.lines() {
            let line = Self::strip_skin_inline_comment(line).trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                let sect = &line[1..line.len() - 1];
                let sect_lower = sect.to_lowercase();
                if sect_lower.starts_with("mania") {
                    current_section = "Mania".to_string();
                    if let Some(keys_pos) = sect_lower.find("keys:") {
                        let after_keys = &sect[keys_pos + 5..].trim();
                        if let Ok(keys) = after_keys
                            .split_whitespace()
                            .next()
                            .unwrap_or("0")
                            .parse::<i32>()
                        {
                            in_target_mania_section = keys == target_keys_i32;
                            if in_target_mania_section {
                                has_target_mania_section = true;
                            }
                        }
                    } else {
                        in_target_mania_section = false;
                    }
                } else {
                    current_section = sect.to_string();
                    in_target_mania_section = false;
                }
                continue;
            }
            let sep_pos = line.find(':').or_else(|| line.find('='));
            if sep_pos.is_none() {
                continue;
            }
            let sep = sep_pos.unwrap();
            let key = line[..sep].trim();
            let value = line[sep + 1..].trim();
            let key_lower = key.to_ascii_lowercase();
            match current_section.as_str() {
                "General" | "" => match key_lower.as_str() {
                    "name" => {
                        config.name = value.to_string();
                    }
                    "author" => {
                        config.author = value.to_string();
                    }
                    "version" => {
                        config.version = value.to_string();
                    }
                    "animationframerate" => {
                        if let Ok(n) = value.parse::<u32>() {
                            config.animation_framerate = Some(n);
                        }
                    }
                    _ => {}
                },
                "Fonts" => match key_lower.as_str() {
                    "scoreoverlap" => {
                        if let Ok(n) = value.parse::<i32>() {
                            config.score_overlap = Some(n);
                        }
                    }
                    "combooverlap" => {
                        if let Ok(n) = value.parse::<i32>() {
                            config.combo_overlap = Some(n);
                        }
                    }
                    "scoreprefix" => {
                        config.score_prefix = Self::normalize_skin_prefix(value);
                    }
                    "comboprefix" => {
                        config.combo_prefix = Self::normalize_skin_prefix(value);
                    }
                    "hitprefix" => {
                        config.hit_prefix = Self::normalize_skin_prefix(value);
                    }
                    _ => {}
                },
                "Mania" => {
                    if key_lower == "keys" {
                        // A skin.ini can contain multiple Mania sections; only parse the target key count.
                        if let Ok(keys) = value.parse::<i32>() {
                            in_target_mania_section = keys == target_keys_i32;
                            if in_target_mania_section {
                                has_target_mania_section = true;
                            }
                        } else {
                            in_target_mania_section = false;
                        }
                        continue;
                    }
                    if !in_target_mania_section {
                        continue;
                    }
                    if key_lower == "comboposition" {
                        if let Ok(n) = value.parse::<i32>() {
                            config.combo_pos_y = Some(n);
                        }
                    }
                    if key_lower == "scoreposition" {
                        let parts: Vec<&str> = value.split(',').collect();
                        if parts.len() == 1 {
                            if let Ok(n) = parts[0].trim().parse::<i32>() {
                                config.score_y = Some(n);
                            }
                        } else if parts.len() >= 2 {
                            if let Ok(n) = parts[1].trim().parse::<i32>() {
                                config.score_y = Some(n);
                            }
                        }
                    }
                    if key_lower == "hit0" {
                        config.hit_0 = Some(self.convert_skin_path(value));
                    }
                    if key_lower == "hit50" {
                        config.hit_50 = Some(self.convert_skin_path(value));
                    }
                    if key_lower == "hit100" {
                        config.hit_100 = Some(self.convert_skin_path(value));
                    }
                    if key_lower == "hit200" {
                        config.hit_200 = Some(self.convert_skin_path(value));
                    }
                    if key_lower == "hit300" {
                        config.hit_300 = Some(self.convert_skin_path(value));
                    }
                    if key_lower == "hit300g" {
                        config.hit_300g = Some(self.convert_skin_path(value));
                    }
                    if key_lower.starts_with("keyimage") && key_lower.ends_with('d') {
                        if let Some(col) = Self::extract_column_number(key, "KeyImage", "D") {
                            if col < target_keys as usize {
                                while config.key_image_d.len() <= col {
                                    config.key_image_d.push(String::new());
                                }
                                config.key_image_d[col] = self.convert_skin_path(value);
                            }
                        }
                    } else if key_lower.starts_with("keyimage") {
                        if let Some(col) = Self::extract_column_number(key, "KeyImage", "") {
                            if col < target_keys as usize {
                                while config.key_image.len() <= col {
                                    config.key_image.push(String::new());
                                }
                                config.key_image[col] = self.convert_skin_path(value);
                            }
                        }
                    }
                    if key_lower.starts_with("noteimage")
                        && key_lower.ends_with('h')
                        && !key_lower.ends_with("lh")
                        && !key_lower.ends_with("th")
                    {
                        if let Some(col) = Self::extract_column_number(key, "NoteImage", "H") {
                            if col < target_keys as usize {
                                while config.note_image_h.len() <= col {
                                    config.note_image_h.push(String::new());
                                }
                                config.note_image_h[col] = self.convert_skin_path(value);
                            }
                        }
                    } else if key_lower.starts_with("noteimage") && key_lower.ends_with('l') {
                        if let Some(col) = Self::extract_column_number(key, "NoteImage", "L") {
                            if col < target_keys as usize {
                                while config.note_image_l.len() <= col {
                                    config.note_image_l.push(String::new());
                                }
                                config.note_image_l[col] = self.convert_skin_path(value);
                            }
                        }
                    } else if key_lower.starts_with("noteimage") && key_lower.ends_with('t') {
                        if let Some(col) = Self::extract_column_number(key, "NoteImage", "T") {
                            if col < target_keys as usize {
                                while config.note_image_t.len() <= col {
                                    config.note_image_t.push(String::new());
                                }
                                config.note_image_t[col] = self.convert_skin_path(value);
                            }
                        }
                    } else if key.starts_with("NoteImage") {
                        if let Some(col) = Self::extract_column_number(key, "NoteImage", "") {
                            if col < target_keys as usize {
                                while config.note_image.len() <= col {
                                    config.note_image.push(String::new());
                                }
                                config.note_image[col] = self.convert_skin_path(value);
                            }
                        }
                    }
                    if key_lower == "lightingn" {
                        config.lighting_n = vec![self.convert_skin_path(value)];
                    }
                    if key_lower == "lightingl" {
                        config.lighting_l = vec![self.convert_skin_path(value)];
                    }
                    match key_lower.as_str() {
                        "stageleft" => {
                            config.stage_left = Some(self.convert_skin_path(value));
                        }
                        "stageright" => {
                            config.stage_right = Some(self.convert_skin_path(value));
                        }
                        "stagebottom" => {
                            config.stage_bottom = Some(self.convert_skin_path(value));
                        }
                        "stagehint" => {
                            config.stage_hint = Some(self.convert_skin_path(value));
                        }
                        "stagelight" => {
                            config.stage_light = Some(self.convert_skin_path(value));
                        }
                        "warningarrow" => {
                            config.warning_arrow = Some(self.convert_skin_path(value));
                        }
                        _ => {}
                    }
                    if let Some(col) = Self::extract_mania_light_colour_index(key) {
                        if col < target_keys as usize {
                            if let Some(color) = Self::parse_rgb_or_rgba_color(value, true) {
                                while config.colour_light.len() <= col {
                                    config.colour_light.push(None);
                                }
                                config.colour_light[col] = Some(color);
                            }
                        }
                    }
                    if let Some(col) = Self::extract_mania_colour_index(key) {
                        if col < target_keys as usize {
                            if let Some(color) = Self::parse_rgba_color(value) {
                                while config.column_colors.len() <= col {
                                    config.column_colors.push(None);
                                }
                                config.column_colors[col] = Some(color);
                            }
                        }
                    }
                    if key_lower == "columnwidth" {
                        let parts = Self::parse_i32_list(value);
                        if !parts.is_empty() {
                            let take = (target_keys as usize).min(parts.len());
                            config.column_widths = Some(parts[..take].to_vec());
                        }
                    }
                    if key_lower == "columnspacing" {
                        let parts = Self::parse_i32_list(value);
                        if !parts.is_empty() {
                            let take = ((target_keys as usize).saturating_sub(1)).min(parts.len());
                            config.column_spacing = Some(parts[..take].to_vec());
                        }
                    }
                    if key_lower == "columnlinewidth" {
                        let parts = Self::parse_f32_list(value);
                        if !parts.is_empty() {
                            let take = ((target_keys as usize).saturating_sub(1)).min(parts.len());
                            config.column_line_width = Some(parts[..take].to_vec());
                        }
                    }
                    if key_lower == "barlineheight" {
                        if let Ok(n) = value.parse::<f32>() {
                            config.barline_height = Some(n);
                        }
                    }
                    if key_lower == "lightingnwidth" {
                        let parts = Self::parse_i32_list(value);
                        if !parts.is_empty() {
                            let take = (target_keys as usize).min(parts.len());
                            config.lighting_n_width = Some(parts[..take].to_vec());
                        }
                    }
                    if key_lower == "lightinglwidth" {
                        let parts = Self::parse_i32_list(value);
                        if !parts.is_empty() {
                            let take = (target_keys as usize).min(parts.len());
                            config.lighting_l_width = Some(parts[..take].to_vec());
                        }
                    }
                    if key_lower == "columnstart" {
                        if let Ok(n) = value.parse::<i32>() {
                            config.column_start = Some(n);
                        }
                    }
                    if key_lower == "columnright" {
                        if let Ok(n) = value.parse::<i32>() {
                            config.column_right = Some(n);
                        }
                    }
                    if key_lower == "hitposition" {
                        if let Ok(n) = value.parse::<i32>() {
                            config.hit_position = Some(n.clamp(240, 480));
                        }
                    }
                    if key_lower == "lightposition" {
                        if let Ok(n) = value.parse::<i32>() {
                            config.light_position = Some(n);
                        }
                    }
                    if key_lower == "lightframepersecond" {
                        if let Ok(n) = value.parse::<u32>() {
                            config.light_frame_per_second = Some(n);
                        }
                    }
                    if key_lower == "judgementline" {
                        config.judgement_line = Self::parse_skin_bool(value);
                    }
                    if key_lower == "keysundernotes" {
                        config.keys_under_notes = Self::parse_skin_bool(value).unwrap_or(false);
                    }
                    if key_lower == "upsidedown" {
                        config.upside_down = Self::parse_skin_bool(value);
                    }
                    if key_lower == "comboburststyle" {
                        config.combo_burst_style = Self::parse_combo_burst_style(value);
                    }
                    if key_lower == "comboburstrandom" {
                        config.combo_burst_random = Self::parse_skin_bool(value);
                    }
                    if key_lower == "specialstyle" {
                        config.special_style = Self::parse_special_style(value);
                    }
                    if key_lower == "widthfornoteheightscale" {
                        if let Ok(n) = value.parse::<u32>() {
                            config.width_for_note_height_scale = Some(n);
                        }
                    }
                    if key_lower == "colourcolumnline" {
                        config.colour_column_line = Self::parse_rgba_color(value);
                    }
                    if key_lower == "colourbarline" {
                        config.colour_barline = Self::parse_rgba_color(value);
                    }
                    if key_lower == "colourjudgementline" {
                        config.colour_judgement_line = Self::parse_rgb_or_rgba_color(value, true);
                    }
                    if key_lower == "colourkeywarning" {
                        config.colour_key_warning = Self::parse_rgb_or_rgba_color(value, true);
                    }
                    if key_lower == "colourhold" {
                        config.colour_hold = Self::parse_rgb_or_rgba_color(value, true);
                    }
                    if key_lower == "colourbreak" {
                        config.colour_break = Self::parse_rgb_or_rgba_color(value, true);
                    }
                    if key_lower == "keyflipwhenupsidedown" {
                        config.key_flip_when_upside_down = Self::parse_skin_bool(value);
                    }
                    if key_lower == "noteflipwhenupsidedown" {
                        config.note_flip_when_upside_down = Self::parse_skin_bool(value);
                    }
                    if key_lower == "notebodystyle" {
                        config.note_body_style = Self::parse_note_body_style(value);
                    }
                    if let Some(col) =
                        Self::extract_column_number(key, "KeyFlipWhenUpsideDown", "D")
                    {
                        if col < target_keys as usize {
                            while config.key_flip_when_upside_down_pressed.len() <= col {
                                config.key_flip_when_upside_down_pressed.push(None);
                            }
                            config.key_flip_when_upside_down_pressed[col] =
                                Self::parse_skin_bool(value);
                        }
                    } else if let Some(col) =
                        Self::extract_column_number(key, "KeyFlipWhenUpsideDown", "")
                    {
                        if col < target_keys as usize {
                            while config.key_flip_when_upside_down_col.len() <= col {
                                config.key_flip_when_upside_down_col.push(None);
                            }
                            config.key_flip_when_upside_down_col[col] =
                                Self::parse_skin_bool(value);
                        }
                    }
                    if let Some(col) =
                        Self::extract_column_number(key, "NoteFlipWhenUpsideDown", "H")
                    {
                        if col < target_keys as usize {
                            while config.note_flip_when_upside_down_h.len() <= col {
                                config.note_flip_when_upside_down_h.push(None);
                            }
                            config.note_flip_when_upside_down_h[col] = Self::parse_skin_bool(value);
                        }
                    } else if let Some(col) =
                        Self::extract_column_number(key, "NoteFlipWhenUpsideDown", "L")
                    {
                        if col < target_keys as usize {
                            while config.note_flip_when_upside_down_l.len() <= col {
                                config.note_flip_when_upside_down_l.push(None);
                            }
                            config.note_flip_when_upside_down_l[col] = Self::parse_skin_bool(value);
                        }
                    } else if let Some(col) =
                        Self::extract_column_number(key, "NoteFlipWhenUpsideDown", "T")
                    {
                        if col < target_keys as usize {
                            while config.note_flip_when_upside_down_t.len() <= col {
                                config.note_flip_when_upside_down_t.push(None);
                            }
                            config.note_flip_when_upside_down_t[col] = Self::parse_skin_bool(value);
                        }
                    } else if let Some(col) =
                        Self::extract_column_number(key, "NoteFlipWhenUpsideDown", "")
                    {
                        if col < target_keys as usize {
                            while config.note_flip_when_upside_down_col.len() <= col {
                                config.note_flip_when_upside_down_col.push(None);
                            }
                            config.note_flip_when_upside_down_col[col] =
                                Self::parse_skin_bool(value);
                        }
                    }
                    if let Some(col) = Self::extract_column_number(key, "NoteBodyStyle", "") {
                        if col < target_keys as usize {
                            while config.note_body_style_col.len() <= col {
                                config.note_body_style_col.push(None);
                            }
                            config.note_body_style_col[col] = Self::parse_note_body_style(value);
                        }
                    }
                }
                _ => {}
            }
        }
        println!(
            "   [skin.ini] parsed ({}K) note_image: {:?}",
            target_keys, config.note_image
        );
        println!(
            "   [skin.ini] parsed ({}K) key_image: {:?}",
            target_keys, config.key_image
        );
        println!("   [skin.ini] scorePrefix: {}", config.score_prefix);
        has_target_mania_section
    }
    pub(crate) fn extract_column_number(key: &str, prefix: &str, suffix: &str) -> Option<usize> {
        let key_upper = key.to_uppercase();
        let prefix_upper = prefix.to_uppercase();
        let suffix_upper = suffix.to_uppercase();
        if !key_upper.starts_with(&prefix_upper) {
            return None;
        }
        if !suffix.is_empty() && !key_upper.ends_with(&suffix_upper) {
            return None;
        }
        let start = prefix.len();
        let end = if suffix.is_empty() {
            key.len()
        } else {
            key.len() - suffix.len()
        };
        if start >= end {
            return None;
        }
        key[start..end].parse().ok()
    }
    pub(crate) fn extract_mania_colour_index(key: &str) -> Option<usize> {
        let key_upper = key.to_uppercase();
        if !key_upper.starts_with("COLOUR") {
            return None;
        }
        let suffix = &key[6..];
        if suffix.is_empty() || !suffix.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let index = suffix.parse::<usize>().ok()?;
        index.checked_sub(1)
    }
    pub(crate) fn extract_mania_light_colour_index(key: &str) -> Option<usize> {
        let key_upper = key.to_uppercase();
        if !key_upper.starts_with("COLOURLIGHT") {
            return None;
        }
        let suffix = &key[11..];
        if suffix.is_empty() || !suffix.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let index = suffix.parse::<usize>().ok()?;
        index.checked_sub(1)
    }
    pub(crate) fn parse_rgba_color(value: &str) -> Option<[f32; 4]> {
        let parts: Vec<&str> = value.split(',').map(|part| part.trim()).collect();
        if parts.len() < 3 || parts.len() > 4 {
            return None;
        }
        let parse_channel = |part: &str| -> Option<f32> {
            let value = part.parse::<u8>().ok()?;
            Some(value as f32 / 255.0)
        };
        let r = parse_channel(parts[0])?;
        let g = parse_channel(parts[1])?;
        let b = parse_channel(parts[2])?;
        let a = parts
            .get(3)
            .and_then(|part| parse_channel(part))
            .unwrap_or(1.0);
        Some([r, g, b, a])
    }
    pub(crate) fn parse_rgb_or_rgba_color(value: &str, force_alpha: bool) -> Option<[f32; 4]> {
        let mut color = Self::parse_rgba_color(value)?;
        if force_alpha {
            color[3] = 1.0;
        }
        Some(color)
    }
    #[allow(dead_code)]
    pub(crate) fn debug_texture_lookup(&self, skin: &SkinAssets, name: &str) -> bool {
        let exists = skin.images.contains_key(name);
        if !exists {
            let without_ext = name.trim_end_matches(".png").trim_end_matches(".jpg");
            let exists2 = skin.images.contains_key(without_ext);
            println!(
                "   [texture] '{}' -> exists={}, without_ext='{}' -> {}",
                name, exists, without_ext, exists2
            );
            return exists2;
        }
        true
    }
    pub(crate) fn convert_skin_path(&self, path: &str) -> String {
        let normalized = path.to_lowercase().replace('\\', "/");
        if regex_lite::Regex::new(r"\.[a-z0-9]+$")
            .unwrap()
            .is_match(&normalized)
        {
            normalized
        } else {
            format!("{}.png", normalized)
        }
    }
    fn normalize_images(
        &self,
        skin: &mut SkinAssets,
        limits: SkinLoadLimits,
    ) -> Result<(), ConvertError> {
        let entries: Vec<String> = skin.images.keys().cloned().collect();
        let frame_re = regex_lite::Regex::new(r"^(.*)-(0|1)(\.[a-z0-9]+)$").unwrap();
        for key in &entries {
            if let Some(caps) = frame_re.captures(key) {
                // osu! skins often store frame 0/1 files where render code asks for the base name.
                let base = format!("{}{}", &caps[1], &caps[3]).to_lowercase();
                if !skin.images.contains_key(&base) {
                    if skin.images.len() + 1 > limits.max_loaded_images {
                        return Err(ConvertError::Resolve(format!(
                            "skin image alias count exceeds limit: {} > {}",
                            skin.images.len() + 1,
                            limits.max_loaded_images
                        )));
                    }
                    if let Some(buf) = skin.images.get(key).cloned() {
                        skin.images.insert(base.clone(), buf);
                    }
                }
            }
        }
        let at2x_re = regex_lite::Regex::new(r"^(.*)@2x(\.[a-z0-9]+)$").unwrap();
        for key in &entries {
            if let Some(caps) = at2x_re.captures(key) {
                // Add a normal-resolution alias for @2x assets so old configs still resolve.
                let base = format!("{}{}", &caps[1], &caps[2]).to_lowercase();
                if !skin.images.contains_key(&base) {
                    if skin.images.len() + 1 > limits.max_loaded_images {
                        return Err(ConvertError::Resolve(format!(
                            "skin image alias count exceeds limit: {} > {}",
                            skin.images.len() + 1,
                            limits.max_loaded_images
                        )));
                    }
                    if let Some(buf) = skin.images.get(key) {
                        if let Some(downscaled) = self.downscale_image_half(buf, limits) {
                            skin.images.insert(base, downscaled);
                        } else {
                            skin.images.insert(base, buf.clone());
                        }
                    }
                }
            }
        }
        Ok(())
    }
    fn downscale_image_half(&self, data: &[u8], limits: SkinLoadLimits) -> Option<Vec<u8>> {
        use image::GenericImageView;
        let (width, height) = crate::utils::get_dimensions(data)?;
        let pixels = u64::from(width).checked_mul(u64::from(height))?;
        if width > limits.max_image_dimension
            || height > limits.max_image_dimension
            || pixels > limits.max_image_pixels
        {
            return None;
        }
        let img = image::load_from_memory(data).ok()?;
        let (w, h) = img.dimensions();
        let new_w = (w / 2).max(1);
        let new_h = (h / 2).max(1);
        let resized =
            image::imageops::resize(&img, new_w, new_h, image::imageops::FilterType::CatmullRom);
        let mut buf = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buf);
        resized
            .write_to(&mut cursor, image::ImageFormat::Png)
            .ok()?;
        Some(buf)
    }
    pub(crate) fn ensure_image_aliases(&self, skin: &mut SkinAssets) {
        let mut by_basename: HashMap<String, String> = HashMap::new();
        for key in skin.images.keys() {
            let basename = key.split('/').next_back().unwrap_or(key).to_lowercase();
            by_basename.entry(basename).or_insert_with(|| key.clone());
        }
        // Resolve configured texture names against basename aliases for skins with nested folders.
        let mut wanted: Vec<String> = Vec::new();
        if let Some(n) = skin.config.hit_0.as_ref().filter(|name| !name.is_empty()) {
            wanted.push(n.clone());
        }
        if let Some(n) = skin.config.hit_50.as_ref().filter(|name| !name.is_empty()) {
            wanted.push(n.clone());
        }
        if let Some(n) = skin.config.hit_100.as_ref().filter(|name| !name.is_empty()) {
            wanted.push(n.clone());
        }
        if let Some(n) = skin.config.hit_200.as_ref().filter(|name| !name.is_empty()) {
            wanted.push(n.clone());
        }
        if let Some(n) = skin.config.hit_300.as_ref().filter(|name| !name.is_empty()) {
            wanted.push(n.clone());
        }
        if let Some(n) = skin
            .config
            .hit_300g
            .as_ref()
            .filter(|name| !name.is_empty())
        {
            wanted.push(n.clone());
        }
        for name in &skin.config.note_image {
            if !name.is_empty() {
                wanted.push(name.clone());
            }
        }
        for name in &skin.config.note_image_h {
            if !name.is_empty() {
                wanted.push(name.clone());
            }
        }
        for name in &skin.config.note_image_l {
            if !name.is_empty() {
                wanted.push(name.clone());
            }
        }
        for name in &skin.config.note_image_t {
            if !name.is_empty() {
                wanted.push(name.clone());
            }
        }
        for name in &skin.config.key_image {
            if !name.is_empty() {
                wanted.push(name.clone());
            }
        }
        for name in &skin.config.key_image_d {
            if !name.is_empty() {
                wanted.push(name.clone());
            }
        }
        for name in &skin.config.lighting_n {
            if !name.is_empty() {
                wanted.push(name.clone());
            }
        }
        for name in &skin.config.lighting_l {
            if !name.is_empty() {
                wanted.push(name.clone());
            }
        }
        if let Some(n) = skin
            .config
            .stage_left
            .as_ref()
            .filter(|name| !name.is_empty())
        {
            wanted.push(n.clone());
        }
        if let Some(n) = skin
            .config
            .stage_right
            .as_ref()
            .filter(|name| !name.is_empty())
        {
            wanted.push(n.clone());
        }
        if let Some(n) = skin
            .config
            .stage_bottom
            .as_ref()
            .filter(|name| !name.is_empty())
        {
            wanted.push(n.clone());
        }
        if let Some(n) = skin
            .config
            .stage_hint
            .as_ref()
            .filter(|name| !name.is_empty())
        {
            wanted.push(n.clone());
        }
        if let Some(n) = skin
            .config
            .stage_light
            .as_ref()
            .filter(|name| !name.is_empty())
        {
            wanted.push(n.clone());
        }
        if let Some(n) = skin
            .config
            .warning_arrow
            .as_ref()
            .filter(|name| !name.is_empty())
        {
            wanted.push(n.clone());
        }
        for name in &skin.config.combo_burst {
            if !name.is_empty() {
                wanted.push(name.clone());
            }
        }
        let score_prefix = skin.config.score_prefix_or_default();
        let combo_prefix = skin.config.combo_prefix_or_default();
        for d in 0..=9 {
            wanted.push(format!("{}-{}.png", score_prefix, d));
            wanted.push(format!("{}-{}.png", combo_prefix, d));
        }
        wanted.push(format!("{}-comma.png", score_prefix));
        wanted.push(format!("{}-dot.png", score_prefix));
        wanted.push(format!("{}-percent.png", score_prefix));
        wanted.push(format!("{}-x.png", combo_prefix));
        let default_textures = [
            "mania-note1.png",
            "mania-note2.png",
            "mania-note3.png",
            "mania-note4.png",
            "mania-notes.png",
            "mania-note1h.png",
            "mania-note2h.png",
            "mania-note3h.png",
            "mania-note4h.png",
            "mania-notesh.png",
            "mania-note1l.png",
            "mania-note2l.png",
            "mania-note3l.png",
            "mania-note4l.png",
            "mania-notesl.png",
            "mania-note1t.png",
            "mania-note2t.png",
            "mania-note3t.png",
            "mania-note4t.png",
            "mania-notest.png",
            "mania-key1.png",
            "mania-key2.png",
            "mania-key3.png",
            "mania-key4.png",
            "mania-keys.png",
            "mania-key1d.png",
            "mania-key2d.png",
            "mania-key3d.png",
            "mania-key4d.png",
            "mania-keysd.png",
            "mania-stage-left.png",
            "mania-stage-right.png",
            "mania-stage-bottom.png",
            "mania-stage-hint.png",
            "mania-stage-light.png",
            "mania-warningarrow.png",
            "lightingn.png",
            "lightingl.png",
            "comboburst-mania.png",
            "comboburst.png",
        ];
        for name in default_textures {
            wanted.push(name.to_string());
        }
        let mut aliases_to_add: Vec<(String, Vec<u8>)> = Vec::new();
        for ref_name in &wanted {
            let key = ref_name.to_lowercase().replace('\\', "/");
            if skin.images.contains_key(&key) {
                continue;
            }
            if Self::is_strict_stage_fallback_name(&key) {
                continue;
            }
            let basename = key.split('/').next_back().unwrap_or(&key);
            if let Some(found_key) = by_basename.get(basename) {
                if let Some(data) = skin.images.get(found_key) {
                    aliases_to_add.push((key.clone(), data.clone()));
                    println!("   [alias] {} -> {} (found by basename)", key, found_key);
                }
            } else {
                let without_ext = basename.trim_end_matches(".png").trim_end_matches(".jpg");
                if let Some(found_key) = by_basename.get(without_ext) {
                    if let Some(data) = skin.images.get(found_key) {
                        aliases_to_add.push((key.clone(), data.clone()));
                        println!("   [alias] {} -> {} (found without ext)", key, found_key);
                    }
                }
            }
        }
        for (key, data) in aliases_to_add {
            skin.images.insert(key, data);
        }
    }
    fn is_strict_stage_fallback_name(key: &str) -> bool {
        matches!(
            key,
            "mania-stage-left.png"
                | "mania-stage-right.png"
                | "mania-stage-bottom.png"
                | "mania-stage-hint.png"
                | "mania-stage-light.png"
                | "mania_stage_left.png"
                | "mania_stage_right.png"
                | "mania_stage_bottom.png"
                | "mania_stage_hint.png"
                | "mania_stage_light.png"
        )
    }
}
fn list_skin_files_recursive_with_limit(
    dir: &Path,
    max_files: usize,
) -> Result<Vec<String>, ConvertError> {
    let mut result = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        if let Ok(entries) = fs::read_dir(&current) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if let Ok(rel) = path.strip_prefix(dir) {
                    if let Some(rel_str) = rel.to_str() {
                        result.push(rel_str.replace('\\', "/"));
                        if result.len() > max_files {
                            return Err(ConvertError::Resolve(format!(
                                "skin directory has too many files: {} > {}",
                                result.len(),
                                max_files
                            )));
                        }
                    }
                }
            }
        }
    }
    Ok(result)
}
fn read_skin_text_file_with_limit(path: &Path, max_bytes: u64) -> Result<String, ConvertError> {
    let len = fs::metadata(path).map_err(ConvertError::Io)?.len();
    if len > max_bytes {
        return Err(ConvertError::Resolve(format!(
            "skin metadata file exceeds limit: {} > {} bytes ({})",
            len,
            max_bytes,
            path.display()
        )));
    }
    let bytes = fs::read(path).map_err(ConvertError::Io)?;
    String::from_utf8(bytes).map_err(|_| {
        ConvertError::Resolve(format!(
            "skin metadata file is not valid UTF-8: {}",
            path.display()
        ))
    })
}
fn read_skin_text_reader_with_limit<R: Read>(
    reader: &mut R,
    max_bytes: u64,
    name: &str,
) -> Result<String, ConvertError> {
    let bytes = read_skin_binary_reader_with_limit(reader, max_bytes, name)?;
    String::from_utf8(bytes)
        .map_err(|_| ConvertError::Resolve(format!("skin metadata is not valid UTF-8: {name}")))
}
fn read_skin_binary_reader_with_limit<R: Read>(
    reader: &mut R,
    max_bytes: u64,
    name: &str,
) -> Result<Vec<u8>, ConvertError> {
    let mut output = Vec::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer).map_err(ConvertError::Io)?;
        if read == 0 {
            break;
        }
        let projected = output
            .len()
            .checked_add(read)
            .ok_or_else(|| ConvertError::Resolve(format!("skin entry size overflow: {name}")))?;
        if projected as u64 > max_bytes {
            return Err(ConvertError::Resolve(format!(
                "skin entry exceeds limit while streaming: {} > {} bytes ({})",
                projected, max_bytes, name
            )));
        }
        output.extend_from_slice(&buffer[..read]);
    }
    Ok(output)
}
fn copy_reader_to_file_with_limit<R: Read>(
    reader: &mut R,
    output_path: &Path,
    max_bytes: u64,
    name: &str,
    label: &str,
) -> Result<u64, ConvertError> {
    use std::io::Write;
    let mut output = fs::File::create(output_path).map_err(ConvertError::Io)?;
    let mut buffer = [0u8; 64 * 1024];
    let mut written = 0u64;
    loop {
        let read = reader.read(&mut buffer).map_err(ConvertError::Io)?;
        if read == 0 {
            break;
        }
        written = written
            .checked_add(read as u64)
            .ok_or_else(|| ConvertError::Resolve(format!("{label} size overflow: {name}")))?;
        if written > max_bytes {
            drop(output);
            let _ = fs::remove_file(output_path);
            return Err(ConvertError::Resolve(format!(
                "{label} exceeds limit while streaming: {} > {} bytes ({})",
                written, max_bytes, name
            )));
        }
        output
            .write_all(&buffer[..read])
            .map_err(ConvertError::Io)?;
    }
    Ok(written)
}
fn validate_skin_image_bytes(
    data: &[u8],
    name: &str,
    limits: SkinLoadLimits,
) -> Result<(), ConvertError> {
    let (width, height) = crate::utils::get_dimensions(data).ok_or_else(|| {
        ConvertError::Resolve(format!("skin image has unreadable dimensions: {name}"))
    })?;
    if width == 0 || height == 0 {
        return Err(ConvertError::Resolve(format!(
            "skin image has invalid dimensions: {}x{} ({})",
            width, height, name
        )));
    }
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| ConvertError::Resolve(format!("skin image pixel count overflow: {name}")))?;
    if pixels > limits.max_image_pixels {
        return Err(ConvertError::Resolve(format!(
            "skin image pixel count exceeds limit: {} > {} ({})",
            pixels, limits.max_image_pixels, name
        )));
    }
    if width > limits.max_image_dimension || height > limits.max_image_dimension {
        println!(
            "   [skin] warn: loading oversized image {}x{} ({}) for CPU-side scaling",
            width, height, name
        );
    }
    Ok(())
}

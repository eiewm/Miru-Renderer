use crate::types::replay::ManiaReplayData;
use std::fs;
use std::path::{Path, PathBuf};
#[derive(Debug, Clone)]
pub struct ResolveResult {
    pub osu_path: PathBuf,
    pub set_dir: PathBuf,
    pub set_files: Vec<String>,
    pub beatmap_id: Option<u32>,
    pub beatmapset_id: Option<u32>,
    pub md5: String,
}
#[derive(Debug, Clone, Default)]
pub struct ResolveOptions {
    pub osu: Option<PathBuf>,
}
#[derive(Debug)]
pub enum ResolveError {
    NotFound(String),
    IoError(std::io::Error),
}
impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(s) => write!(f, "beatmap not found: {s}"),
            Self::IoError(e) => write!(f, "IO error: {e}"),
        }
    }
}
impl std::error::Error for ResolveError {}
impl From<std::io::Error> for ResolveError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}
pub fn resolve_beatmap_from_replay(
    replay: &ManiaReplayData,
    opts: &ResolveOptions,
) -> Result<ResolveResult, ResolveError> {
    let replay_md5 = replay.replay.beatmap_hash.trim();
    if !replay_md5.is_empty() {
        println!("[resolver] replay beatmap checksum: {replay_md5}");
    }
    let osu_path = opts
        .osu
        .as_ref()
        .ok_or_else(|| ResolveError::NotFound("provide --osu or --mapset".to_string()))?;
    resolve_with_override(osu_path, replay_md5)
}
fn resolve_with_override(osu_path: &Path, replay_md5: &str) -> Result<ResolveResult, ResolveError> {
    if !osu_path.exists() {
        return Err(ResolveError::NotFound(format!(
            "override .osu not found: {}",
            osu_path.display()
        )));
    }
    let actual_md5 = md5_file(osu_path)?;
    if !replay_md5.is_empty() && !actual_md5.eq_ignore_ascii_case(replay_md5) {
        return Err(ResolveError::NotFound(format!(
            "replay checksum mismatch: provided .osu has MD5 {actual_md5} but replay requires {replay_md5}"
        )));
    }
    let set_dir = osu_path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let set_files = list_dir_files(&set_dir);
    let (beatmap_id, beatmapset_id) = parse_beatmap_ids(osu_path);
    Ok(ResolveResult {
        osu_path: osu_path.to_path_buf(),
        set_dir,
        set_files,
        beatmap_id,
        beatmapset_id,
        md5: actual_md5,
    })
}
pub fn resolve_audio(set_dir: &Path, audio_filename: Option<&str>) -> Option<PathBuf> {
    let files = walk_dir_all_files(set_dir);
    let audio_extensions = [".mp3", ".ogg", ".wav", ".flac", ".m4a"];
    // Beatmaps often reference audio with casing that differs from the extracted file.
    let lc_map: std::collections::HashMap<String, PathBuf> = files
        .iter()
        .filter_map(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .map(|name| (name.to_lowercase(), path.clone()))
        })
        .collect();
    if let Some(filename) = audio_filename {
        let wanted = filename.trim().to_lowercase();
        let mut candidates = vec![wanted.clone()];
        if let Some(sanitized) = crate::utils::sanitize_archive_entry_name(filename.trim()) {
            if let Some(name) = Path::new(&sanitized)
                .file_name()
                .and_then(|name| name.to_str())
            {
                candidates.push(name.to_lowercase());
            }
        }
        candidates.dedup();

        for candidate in candidates {
            if let Some(path) = lc_map.get(&candidate) {
                if path.exists() {
                    return Some(path.clone());
                }
            }
            if let Some(base) = Path::new(&candidate).file_stem().and_then(|s| s.to_str()) {
                // Some maps keep the stem but ship a different common audio extension.
                for ext in &audio_extensions {
                    let candidate = format!("{base}{ext}");
                    if let Some(path) = lc_map.get(&candidate) {
                        if path.exists() {
                            return Some(path.clone());
                        }
                    }
                }
            }
        }
    }
    for (name, path) in &lc_map {
        for ext in &audio_extensions {
            if name.ends_with(ext) && path.exists() {
                return Some(path.clone());
            }
        }
    }
    None
}
fn walk_dir_all_files(dir: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        if let Ok(entries) = fs::read_dir(&current) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    result.push(path);
                }
            }
        }
    }
    result
}
fn list_dir_files(dir: &Path) -> Vec<String> {
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
                    }
                }
            }
        }
    }
    result
}
fn parse_beatmap_ids(osu_path: &Path) -> (Option<u32>, Option<u32>) {
    let content = match fs::read_to_string(osu_path) {
        Ok(c) => c,
        Err(_) => return (None, None),
    };
    let mut beatmap_id = None;
    let mut beatmapset_id = None;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("BeatmapID:") {
            if let Ok(id) = trimmed
                .strip_prefix("BeatmapID:")
                .unwrap_or("")
                .trim()
                .parse::<i32>()
            {
                if id > 0 {
                    beatmap_id = Some(id as u32);
                }
            }
        } else if trimmed.starts_with("BeatmapSetID:") {
            if let Ok(id) = trimmed
                .strip_prefix("BeatmapSetID:")
                .unwrap_or("")
                .trim()
                .parse::<i32>()
            {
                if id > 0 {
                    beatmapset_id = Some(id as u32);
                }
            }
        }
    }
    (beatmap_id, beatmapset_id)
}
fn md5_file(path: &Path) -> Result<String, ResolveError> {
    let data = fs::read(path)?;
    Ok(format!("{:x}", md5::compute(&data)))
}
pub fn likely_songs_dirs() -> Vec<PathBuf> {
    Vec::new()
}

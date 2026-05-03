use super::limits::{
    extract_archive_to_dir, ArchiveExtractionLimits, DEFAULT_ARCHIVE_EXTRACTION_LIMITS,
};
use crate::parser;
use crate::types::replay::ManiaReplayData;
use serde::Serialize;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
#[derive(Debug, Clone)]
pub struct MapsetDiff {
    pub path: PathBuf,
    pub md5: String,
    pub version: String,
    pub title: String,
    pub artist: String,
    pub creator: String,
    pub beatmap_id: Option<u32>,
    pub beatmapset_id: Option<u32>,
    pub keys: u8,
    pub note_count: usize,
    pub has_background_video: bool,
    pub has_storyboard: bool,
}
#[derive(Debug, Clone)]
pub struct MapsetContext {
    pub root_dir: PathBuf,
    pub diffs: Vec<MapsetDiff>,
}
#[derive(Debug, Serialize)]
pub struct MapsetDiffJson {
    pub index: usize,
    pub md5: String,
    pub version: String,
    pub title: String,
    pub artist: String,
    pub creator: String,
    pub beatmap_id: Option<u32>,
    pub beatmapset_id: Option<u32>,
    pub keys: u8,
    pub note_count: usize,
    pub has_background_video: bool,
    pub has_storyboard: bool,
    pub path: String,
}
#[derive(Debug)]
pub enum MapsetError {
    NotFound(String),
    Unsupported(String),
    NoOsuFiles,
    NoManiaDiffs,
    Md5NotFound {
        md5: String,
    },
    DifficultyNotFound {
        name: String,
    },
    DifficultyMismatch {
        requested: String,
        found: String,
    },
    ReplayChecksumMismatch {
        replay_md5: String,
        selected_md5: String,
        version: String,
    },
    MultipleDiffs {
        versions: Vec<String>,
    },
    ReplaySelectionAmbiguous {
        replay_md5: Option<String>,
        versions: Vec<String>,
    },
    Parse(String),
    Io(std::io::Error),
}
impl std::fmt::Display for MapsetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(msg) => write!(f, "mapset not found: {msg}"),
            Self::Unsupported(msg) => write!(f, "mapset unsupported: {msg}"),
            Self::NoOsuFiles => write!(f, "mapset has no .osu files"),
            Self::NoManiaDiffs => write!(f, "mapset has no osu!mania diffs"),
            Self::Md5NotFound { md5 } => write!(f, "no .osu matches replay MD5 {md5}"),
            Self::DifficultyNotFound { name } => write!(f, "difficulty not found: {name}"),
            Self::DifficultyMismatch { requested, found } => write!(
                f,
                "difficulty mismatch: requested \"{requested}\" but replay matches \"{found}\""
            ),
            Self::ReplayChecksumMismatch {
                replay_md5,
                selected_md5,
                version,
            } => write!(
                f,
                "replay checksum mismatch: selected \"{}\" has MD5 {} but replay requires {}; use the exact .osu/.osz for this replay",
                normalize_version_for_display(version),
                selected_md5,
                replay_md5
            ),
            Self::MultipleDiffs { versions } => {
                let printable: Vec<String> = versions
                    .iter()
                    .map(|v| normalize_version_for_display(v))
                    .collect();
                write!(
                    f,
                    "multiple diffs found, use --diff-index or --difficulty (available: {})",
                    printable.join(", ")
                )
            }
            Self::ReplaySelectionAmbiguous {
                replay_md5,
                versions,
            } => {
                let printable: Vec<String> = versions
                    .iter()
                    .map(|v| normalize_version_for_display(v))
                    .collect();
                if let Some(replay_md5) = replay_md5 {
                    write!(
                        f,
                        "replay checksum {} did not match a diff in this mapset; use --diff-index or --difficulty (available: {})",
                        replay_md5,
                        printable.join(", ")
                    )
                } else {
                    write!(
                        f,
                        "could not identify the replay diff from this mapset; use --diff-index or --difficulty (available: {})",
                        printable.join(", ")
                    )
                }
            }
            Self::Parse(msg) => write!(f, "parse error: {msg}"),
            Self::Io(e) => write!(f, "io error: {e}"),
        }
    }
}
impl std::error::Error for MapsetError {}
impl From<std::io::Error> for MapsetError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
pub fn diffs_to_json(diffs: &[MapsetDiff]) -> Vec<MapsetDiffJson> {
    diffs
        .iter()
        .enumerate()
        .map(|(idx, d)| MapsetDiffJson {
            index: idx,
            md5: d.md5.clone(),
            version: d.version.clone(),
            title: d.title.clone(),
            artist: d.artist.clone(),
            creator: d.creator.clone(),
            beatmap_id: d.beatmap_id,
            beatmapset_id: d.beatmapset_id,
            keys: d.keys,
            note_count: d.note_count,
            has_background_video: d.has_background_video,
            has_storyboard: d.has_storyboard,
            path: d.path.to_string_lossy().to_string(),
        })
        .collect()
}
pub fn load_mapset(mapset_path: &Path, cache_base: &Path) -> Result<MapsetContext, MapsetError> {
    load_mapset_with_options(
        mapset_path,
        cache_base,
        parser::ParseBeatmapOptions::default(),
    )
}

pub fn load_mapset_with_options(
    mapset_path: &Path,
    cache_base: &Path,
    parse_options: parser::ParseBeatmapOptions,
) -> Result<MapsetContext, MapsetError> {
    if !mapset_path.exists() {
        return Err(MapsetError::NotFound(mapset_path.display().to_string()));
    }
    let root_dir = if mapset_path.is_dir() {
        mapset_path.to_path_buf()
    } else {
        let ext = mapset_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ext != "osz" && ext != "zip" {
            return Err(MapsetError::Unsupported(format!(
                "expected .osz/.zip or directory: {}",
                mapset_path.display()
            )));
        }
        fs::create_dir_all(cache_base)?;
        // Cache extracted archives by file hash so repeated loads reuse the same unpacked mapset.
        let archive_md5 = md5_file(mapset_path)?;
        let dest_dir = cache_base.join(archive_md5);
        if !has_osu_files(&dest_dir) {
            extract_zip(mapset_path, &dest_dir)?;
        }
        dest_dir
    };
    let mut osu_files = walk_dir_osu(&root_dir);
    if osu_files.is_empty() {
        return Err(MapsetError::NoOsuFiles);
    }
    osu_files.sort();
    let mut diffs = Vec::new();
    for path in osu_files {
        let beatmap = match parser::parse_osu_file_with_options(&path, parse_options) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("warn: failed to parse {}: {}", path.display(), e);
                continue;
            }
        };
        if beatmap.metadata.mode != 3 {
            continue;
        }
        let md5 = md5_file(&path)?;
        let (beatmap_id, beatmapset_id) = parse_beatmap_ids(&path);
        let has_background_video = beatmap.has_background_video();
        let has_storyboard = beatmap.has_storyboard();
        diffs.push(MapsetDiff {
            path,
            md5,
            version: beatmap.metadata.version,
            title: beatmap.metadata.title,
            artist: beatmap.metadata.artist,
            creator: beatmap.metadata.creator,
            beatmap_id,
            beatmapset_id,
            keys: beatmap.difficulty.key_count(),
            note_count: beatmap.hit_objects.len(),
            has_background_video,
            has_storyboard,
        });
    }
    if diffs.is_empty() {
        return Err(MapsetError::NoManiaDiffs);
    }
    Ok(MapsetContext { root_dir, diffs })
}
pub fn select_diff_by_index(
    diffs: &[MapsetDiff],
    index: usize,
) -> Result<&MapsetDiff, MapsetError> {
    diffs.get(index).ok_or(MapsetError::DifficultyNotFound {
        name: format!("index {} (mapset has {} diffs)", index, diffs.len()),
    })
}
pub fn select_diff_by_index_for_replay<'a>(
    diffs: &'a [MapsetDiff],
    index: usize,
    replay: &ManiaReplayData,
) -> Result<&'a MapsetDiff, MapsetError> {
    let diff = select_diff_by_index(diffs, index)?;
    let replay_md5 = replay.replay.beatmap_hash.trim();
    if !replay_md5.is_empty() && !diff.md5.eq_ignore_ascii_case(replay_md5) {
        return Err(MapsetError::ReplayChecksumMismatch {
            replay_md5: replay_md5.to_string(),
            selected_md5: diff.md5.clone(),
            version: diff.version.clone(),
        });
    }
    Ok(diff)
}
pub fn select_diff<'a>(
    diffs: &'a [MapsetDiff],
    difficulty: Option<&str>,
    replay_md5: Option<&str>,
) -> Result<&'a MapsetDiff, MapsetError> {
    if let Some(md5) = replay_md5 {
        let diff = diffs.iter().find(|d| d.md5 == md5);
        if let Some(diff) = diff {
            if let Some(name) = difficulty {
                if !version_eq(&diff.version, name) {
                    return Err(MapsetError::DifficultyMismatch {
                        requested: name.to_string(),
                        found: diff.version.clone(),
                    });
                }
            }
            return Ok(diff);
        }
        return Err(MapsetError::Md5NotFound {
            md5: md5.to_string(),
        });
    }
    if let Some(name) = difficulty {
        if let Some(diff) = diffs.iter().find(|d| version_eq(&d.version, name)) {
            return Ok(diff);
        }
        return Err(MapsetError::DifficultyNotFound {
            name: name.to_string(),
        });
    }
    if diffs.len() == 1 {
        return Ok(&diffs[0]);
    }
    let versions: Vec<String> = diffs.iter().map(|d| d.version.clone()).collect();
    Err(MapsetError::MultipleDiffs { versions })
}
pub fn select_diff_for_replay<'a>(
    diffs: &'a [MapsetDiff],
    difficulty: Option<&str>,
    replay: &ManiaReplayData,
) -> Result<&'a MapsetDiff, MapsetError> {
    let replay_md5 = replay.replay.beatmap_hash.trim();
    if !replay_md5.is_empty() {
        // The replay hash is authoritative; if it is present, do not guess from nearby metadata.
        if let Some(diff) = diffs
            .iter()
            .find(|d| d.md5.eq_ignore_ascii_case(replay_md5))
        {
            if let Some(name) = difficulty {
                if !version_eq(&diff.version, name) {
                    return Err(MapsetError::DifficultyMismatch {
                        requested: name.to_string(),
                        found: diff.version.clone(),
                    });
                }
            }
            return Ok(diff);
        }
        return Err(MapsetError::Md5NotFound {
            md5: replay_md5.to_string(),
        });
    }
    if let Some(name) = difficulty {
        if let Some(diff) = diffs.iter().find(|d| version_eq(&d.version, name)) {
            return Ok(diff);
        }
        return Err(MapsetError::DifficultyNotFound {
            name: name.to_string(),
        });
    }
    if diffs.len() == 1 {
        return Ok(&diffs[0]);
    }
    if let Some(diff) = select_strict_replay_fallback(diffs, replay) {
        return Ok(diff);
    }
    let versions: Vec<String> = diffs.iter().map(|d| d.version.clone()).collect();
    Err(MapsetError::ReplaySelectionAmbiguous {
        replay_md5: (!replay_md5.is_empty()).then(|| replay_md5.to_string()),
        versions,
    })
}
fn version_eq(a: &str, b: &str) -> bool {
    let a_trimmed = a.trim();
    let b_trimmed = b.trim();
    if a_trimmed.eq_ignore_ascii_case(b_trimmed) {
        return true;
    }
    normalize_version_for_match(a_trimmed) == normalize_version_for_match(b_trimmed)
}
fn normalize_version_for_display(input: &str) -> String {
    normalize_version(input)
}
fn normalize_version_for_match(input: &str) -> String {
    normalize_version(input).to_ascii_lowercase()
}
fn normalize_version(input: &str) -> String {
    let mut normalized = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        // Some map versions arrive with literal or escaped quotes; match the visible name.
        if ch == '\\' && matches!(chars.peek(), Some('"')) {
            continue;
        }
        if ch == '"' || ch == '\'' {
            continue;
        }
        normalized.push(ch);
    }
    normalized.trim().to_string()
}
fn select_strict_replay_fallback<'a>(
    diffs: &'a [MapsetDiff],
    replay: &ManiaReplayData,
) -> Option<&'a MapsetDiff> {
    // Use replay statistics only when they isolate a single diff.
    let replay_note_count = replay.replay.basic_statistics().total();
    if replay_note_count == 0 {
        return None;
    }
    let inferred_keys = infer_replay_key_count(replay);
    let mut candidates = diffs
        .iter()
        .filter(|diff| diff.note_count == replay_note_count as usize);
    if let Some(keys) = inferred_keys {
        let filtered: Vec<_> = candidates
            .by_ref()
            .filter(|diff| diff.keys == keys)
            .collect();
        return (filtered.len() == 1).then_some(filtered[0]);
    }
    let filtered: Vec<_> = candidates.collect();
    (filtered.len() == 1).then_some(filtered[0])
}
fn infer_replay_key_count(replay: &ManiaReplayData) -> Option<u8> {
    let mods = replay.replay.mods;
    // osu!mania key mods encode the intended column count in the replay mods bitfield.
    const KEY_MODS: &[(u32, u8)] = &[
        (1 << 26, 1),
        (1 << 28, 2),
        (1 << 27, 3),
        (1 << 15, 4),
        (1 << 16, 5),
        (1 << 17, 6),
        (1 << 18, 7),
        (1 << 19, 8),
        (1 << 24, 9),
    ];
    KEY_MODS
        .iter()
        .find_map(|(mask, keys)| (mods & mask != 0).then_some(*keys))
}
fn has_osu_files(dir: &Path) -> bool {
    if !dir.exists() {
        return false;
    }
    !walk_dir_osu(dir).is_empty()
}
fn walk_dir_osu(dir: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        if let Ok(entries) = fs::read_dir(&current) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().map(|e| e == "osu").unwrap_or(false) {
                    result.push(path);
                }
            }
        }
    }
    result
}
fn extract_zip(zip_path: &Path, dest_dir: &Path) -> Result<(), MapsetError> {
    extract_zip_with_limits(zip_path, dest_dir, DEFAULT_ARCHIVE_EXTRACTION_LIMITS)
}
fn extract_zip_with_limits(
    zip_path: &Path,
    dest_dir: &Path,
    limits: ArchiveExtractionLimits,
) -> Result<(), MapsetError> {
    extract_archive_to_dir(zip_path, dest_dir, limits).map_err(MapsetError::Parse)
}
fn md5_file(path: &Path) -> Result<String, MapsetError> {
    let mut file = File::open(path)?;
    let mut ctx = md5::Context::new();
    let mut buf = [0u8; 8192];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        ctx.consume(&buf[..read]);
    }
    Ok(format!("{:x}", ctx.compute()))
}
fn parse_beatmap_ids(path: &Path) -> (Option<u32>, Option<u32>) {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return (None, None),
    };
    let mut beatmap_id = None;
    let mut beatmapset_id = None;
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if beatmap_id.is_none() && line.starts_with("BeatmapID:") {
            let value = line.trim_start_matches("BeatmapID:").trim();
            beatmap_id = value.parse::<u32>().ok().filter(|parsed| *parsed > 0);
            continue;
        }
        if beatmapset_id.is_none() && line.starts_with("BeatmapSetID:") {
            let value = line.trim_start_matches("BeatmapSetID:").trim();
            beatmapset_id = value.parse::<u32>().ok().filter(|parsed| *parsed > 0);
        }
    }
    (beatmap_id, beatmapset_id)
}

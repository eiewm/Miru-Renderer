use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use miru_renderer::beatmaps;
use miru_renderer::converter::{
    ConverterSettings, ManiaVideoConverter, ReplayIntegrityReport, ResolveOpts,
};
use miru_renderer::hud::parse_hud_config_json;
use miru_renderer::modes::mania::layout::ScrollDirection;
use miru_renderer::parser;
use miru_renderer::renderer::gpu::context::GpuPreference;
use miru_renderer::utils::{
    autoplay_mod_catalog, parse_autoplay_mods_config_json, AutoplayModsConfig, IntroUserData,
};
use miru_renderer::video::{BgComposeMode, VideoEncoder};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
fn default_cache_dir() -> PathBuf {
    std::env::temp_dir().join("miru_songs_cache")
}
struct MapsetCacheCleanup {
    root_dir: PathBuf,
    cache_base: PathBuf,
    default_songs_dir: Option<PathBuf>,
}
impl Drop for MapsetCacheCleanup {
    fn drop(&mut self) {
        if self.root_dir.exists() {
            if let Err(error) = fs::remove_dir_all(&self.root_dir) {
                eprintln!(
                    "warn: failed to remove temporary mapset cache {}: {}",
                    self.root_dir.display(),
                    error
                );
            }
        }
        let _ = fs::remove_dir(&self.cache_base);
        if let Some(dir) = self.default_songs_dir.as_ref() {
            let _ = fs::remove_dir(dir);
        }
    }
}
fn mapset_cache_cleanup_for(
    mapset_path: &Path,
    root_dir: &Path,
    cache_base: &Path,
    default_songs_dir: Option<PathBuf>,
) -> Option<MapsetCacheCleanup> {
    if mapset_path.is_dir() {
        return None;
    }
    let root_dir = fs::canonicalize(root_dir).ok()?;
    let cache_base = fs::canonicalize(cache_base).ok()?;
    if root_dir == cache_base || !root_dir.starts_with(&cache_base) {
        return None;
    }
    if root_dir.parent() != Some(cache_base.as_path()) {
        return None;
    }
    Some(MapsetCacheCleanup {
        root_dir,
        cache_base,
        default_songs_dir,
    })
}
const DEFAULT_MAX_RENDER_DURATION_MS: i64 = 600_000;
// 9:16 for phone-shaped video; the last four are the versus split per player.
const SUPPORTED_RENDER_RESOLUTIONS: &[(u32, u32)] = &[
    (1280, 720),
    (1920, 1080),
    (720, 1280),
    (1080, 1920),
    (720, 640),
    (1080, 960),
    (720, 576),
    (1080, 864),
];
const SUPPORTED_FPS: &[u32] = &[60];
fn parse_volume_percent(value: &str) -> Result<f32, String> {
    let parsed = value
        .parse::<f32>()
        .map_err(|_| format!("invalid percentage: {value}"))?;
    if !(0.0..=100.0).contains(&parsed) {
        return Err(format!("percentage must be in 0..=100 (got {parsed})"));
    }
    Ok(parsed)
}
fn parse_u8_percent(value: &str) -> Result<u8, String> {
    let parsed = value
        .parse::<u16>()
        .map_err(|_| format!("invalid percentage: {value}"))?;
    if parsed > 100 {
        return Err(format!("percentage must be in 0..=100 (got {parsed})"));
    }
    Ok(parsed as u8)
}
#[derive(Parser, Debug)]
#[command(name = "miru")]
#[command(author = "eie")]
#[command(version)]
#[command(about = "Render osu!mania replays", long_about = None)]
struct Cli {
    #[arg(long, short = 'r', value_name = "FILE")]
    replay: Option<PathBuf>,
    #[arg(long = "out", short = 'o', value_name = "FILE")]
    output: Option<PathBuf>,
    #[arg(long, short = 's', value_name = "DIR")]
    skin: Option<PathBuf>,
    #[arg(long = "ss", visible_alias = "sp", default_value = "30.0")]
    scroll_speed: f32,
    // "sd" is already taken by --songs-dir.
    #[arg(
        long = "scroll-direction",
        visible_alias = "sdir",
        value_name = "down|up"
    )]
    scroll_direction: Option<String>,
    #[arg(long = "lead-in", visible_alias = "li", default_value = "1500")]
    lead_in: i32,
    #[arg(long, short = 'f', default_value = "60")]
    fps: u32,
    #[arg(
        long = "music-volume",
        visible_alias = "mv",
        default_value = "100",
        value_parser = parse_volume_percent
    )]
    music_volume: f32,
    #[arg(
        long = "hitsound-volume",
        visible_alias = "hv",
        default_value = "100",
        value_parser = parse_volume_percent
    )]
    hitsound_volume: f32,
    #[arg(long, short = 'w', default_value = "1920")]
    width: u32,
    #[arg(long, visible_alias = "ht", default_value = "1080")]
    height: u32,
    #[arg(long, short = 'p', default_value = "veryfast")]
    preset: String,
    #[arg(long = "motion-blur", visible_alias = "mb", default_value = "0")]
    motion_blur: u8,
    #[arg(long = "gpu-preference", visible_alias = "gp", default_value = "high")]
    gpu_preference: String,
    #[arg(long = "encoder", short = 'e', default_value = "x264")]
    encoder: String,
    #[arg(long = "ffmpeg-threads", visible_alias = "ft")]
    ffmpeg_threads: Option<u32>,
    #[arg(long = "max-render-duration-ms", visible_alias = "mrd")]
    max_render_duration_ms: Option<i64>,
    #[arg(long = "songs-dir", visible_alias = "sd", value_name = "DIR")]
    songs_dir: Option<PathBuf>,
    #[arg(long = "osu", short = 'u', value_name = "FILE")]
    osu: Option<PathBuf>,
    #[arg(long = "mapset", visible_alias = "ms", value_name = "PATH")]
    mapset: Option<PathBuf>,
    #[arg(
        long = "difficulty",
        visible_alias = "df",
        value_name = "NAME",
        allow_hyphen_values = true
    )]
    difficulty: Option<String>,
    #[arg(long = "diff-index", visible_alias = "di", value_name = "N")]
    diff_index: Option<usize>,
    #[arg(long = "list-diffs", visible_alias = "ld")]
    list_diffs: bool,
    #[arg(long = "list-autoplay-mods", visible_alias = "lam")]
    list_autoplay_mods: bool,
    #[arg(long = "inspect-beatmap", visible_alias = "ib")]
    inspect_beatmap: bool,
    #[arg(
        long = "autoplay-mods-config",
        visible_alias = "amc",
        value_name = "FILE"
    )]
    autoplay_mods_config: Option<PathBuf>,
    #[arg(long = "skip-hud", visible_alias = "sh")]
    skip_hud: bool,
    #[arg(long = "no-lighting", visible_alias = "nli")]
    no_lighting: bool,
    #[arg(long = "barlines", visible_alias = "bl")]
    barlines: bool,
    #[arg(long = "no-storyboard", visible_alias = "nsb")]
    no_storyboard: bool,
    #[arg(long = "no-skin-animations", visible_alias = "nsa")]
    no_skin_animations: bool,
    #[arg(
        long = "no-combo-burst",
        visible_alias = "ncb",
        alias = "no-combo-images"
    )]
    no_combo_burst: bool,
    #[arg(long = "hud-config", visible_alias = "hc", value_name = "FILE")]
    hud_config: Option<PathBuf>,
    #[arg(long = "preview-out", visible_alias = "pv", value_name = "FILE")]
    preview_out: Option<PathBuf>,
    #[arg(long = "preview-time-ms", visible_alias = "pt", value_name = "MS")]
    preview_time_ms: Option<i32>,
    #[arg(long = "hud-editor-preview", visible_alias = "hep")]
    hud_editor_preview: bool,
    #[arg(long = "preview-results", visible_alias = "pvr")]
    preview_results: bool,
    #[arg(
        long = "preview-results-elements",
        visible_alias = "pvre",
        value_name = "FILE"
    )]
    preview_results_elements: Option<PathBuf>,
    #[arg(long = "hud-editor-layout-only", visible_alias = "helo")]
    hud_editor_layout_only: bool,
    #[arg(long = "report-out", visible_alias = "ro", value_name = "FILE")]
    report_out: Option<PathBuf>,
    #[arg(long = "bg-opacity", visible_alias = "bo")]
    bg_opacity: Option<f32>,
    #[arg(
        long = "bg-blur",
        visible_alias = "bb",
        value_name = "PERCENT",
        num_args = 0..=1,
        default_missing_value = "35",
        value_parser = parse_u8_percent
    )]
    bg_blur: Option<u8>,
    #[arg(long = "bg-offset-x", visible_alias = "bgx", default_value_t = 0.0)]
    bg_offset_x: f32,
    #[arg(long = "bg-offset-y", visible_alias = "bgy", default_value_t = 0.0)]
    bg_offset_y: f32,
    #[arg(long = "bg-compose", visible_alias = "bg", default_value = "auto")]
    bg_compose: String,
    #[arg(long = "no-bg-video", visible_alias = "nbv")]
    no_bg_video: bool,
    #[arg(long = "start", visible_alias = "st")]
    start: Option<f32>,
    #[arg(long = "end", visible_alias = "et")]
    end: Option<f32>,
    #[arg(long = "no-intro", visible_alias = "ni")]
    no_intro: bool,
    #[arg(long = "intro-user-json", visible_alias = "iuj", value_name = "FILE")]
    intro_user_json: Option<PathBuf>,
    #[arg(long = "no-sv", visible_alias = "nsv")]
    no_sv: bool,
    #[arg(long = "dry-run", visible_alias = "dr")]
    dry_run: bool,
    #[arg(long = "nd", short = 'd')]
    note_debug: bool,
    #[arg(long = "ap", short = 'a')]
    all_presses: bool,
}
fn validate_cli(cli: &Cli) -> Result<(), String> {
    if cli.list_autoplay_mods {
        return Ok(());
    }
    if cli.inspect_beatmap {
        if cli.replay.is_none() && cli.osu.is_none() {
            return Err("--inspect-beatmap requires --replay or --osu".to_string());
        }
        if cli.mapset.is_some() {
            return Err("--inspect-beatmap is incompatible with --mapset".to_string());
        }
        if cli.replay.is_some() && cli.osu.is_none() {
            return Err("--inspect-beatmap with --replay requires --osu".to_string());
        }
        return Ok(());
    }
    if cli.list_diffs && cli.mapset.is_none() {
        return Err("--list-diffs requires --mapset".to_string());
    }
    if cli.mapset.is_some() && cli.osu.is_some() {
        return Err("--mapset is incompatible with --osu".to_string());
    }
    if cli.diff_index.is_some() && cli.difficulty.is_some() {
        return Err("--diff-index and --difficulty are mutually exclusive".to_string());
    }
    if cli.autoplay_mods_config.is_some() && cli.replay.is_some() {
        return Err(
            "--autoplay-mods-config is only valid for autoplay (--osu/--mapset)".to_string(),
        );
    }
    let standalone_hud_preview = cli.preview_out.is_some()
        && (cli.hud_editor_preview || cli.preview_results)
        && cli.replay.is_none()
        && cli.osu.is_none()
        && cli.mapset.is_none();
    if cli.replay.is_none() && cli.osu.is_none() && cli.mapset.is_none() && !standalone_hud_preview
    {
        return Err("need --replay or --osu or --mapset".to_string());
    }
    if cli.replay.is_some() && cli.osu.is_none() && cli.mapset.is_none() {
        return Err("provide --osu or --mapset when using --replay".to_string());
    }
    if cli.preview_out.is_some() && cli.replay.is_none() && !standalone_hud_preview {
        return Err("--preview-out requires --replay".to_string());
    }
    if !cli.list_diffs && cli.output.is_none() && !cli.dry_run {
        let preview_ok =
            (cli.replay.is_some() && cli.preview_out.is_some()) || standalone_hud_preview;
        if !preview_ok {
            return Err(
                "--out required (unless --dry-run or --preview-out with --replay)".to_string(),
            );
        }
    }
    if let (Some(start), Some(end)) = (cli.start, cli.end) {
        if start >= end {
            return Err("--start must be less than --end".to_string());
        }
    }
    if cli.motion_blur > 100 {
        return Err(format!(
            "--motion-blur must be in 0..=100 (got {})",
            cli.motion_blur
        ));
    }
    if cli.width == 0 || cli.height == 0 {
        return Err(format!(
            "render resolution must be positive (got {}x{})",
            cli.width, cli.height
        ));
    }
    if !SUPPORTED_RENDER_RESOLUTIONS.contains(&(cli.width, cli.height)) {
        let supported = SUPPORTED_RENDER_RESOLUTIONS
            .iter()
            .map(|(width, height)| format!("{width}x{height}"))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "unsupported render resolution {}x{} (supported: {})",
            cli.width, cli.height, supported
        ));
    }
    if cli.fps == 0 {
        return Err("FPS must be > 0".to_string());
    }
    if !SUPPORTED_FPS.contains(&cli.fps) {
        let supported = SUPPORTED_FPS
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "unsupported FPS {} (supported: {})",
            cli.fps, supported
        ));
    }
    if !cli.scroll_speed.is_finite() {
        return Err("scroll speed must be finite".to_string());
    }
    if cli.scroll_speed <= 0.0 {
        return Err(format!(
            "scroll speed must be > 0 (got {})",
            cli.scroll_speed
        ));
    }
    if cli.lead_in < 0 {
        return Err(format!("lead-in must be >= 0 (got {})", cli.lead_in));
    }
    if let Some(direction) = cli.scroll_direction.as_deref() {
        if ScrollDirection::parse(direction).is_none() {
            return Err(format!(
                "unsupported scroll direction {direction} (supported: down, up)"
            ));
        }
    }
    if let Some(start) = cli.start {
        if !start.is_finite() {
            return Err("--start must be finite".to_string());
        }
        if start < 0.0 {
            return Err("--start must be >= 0".to_string());
        }
    }
    if let Some(end) = cli.end {
        if !end.is_finite() {
            return Err("--end must be finite".to_string());
        }
        if end < 0.0 {
            return Err("--end must be >= 0".to_string());
        }
    }
    if let Some(max_render_duration_ms) = cli.max_render_duration_ms {
        if max_render_duration_ms <= 0 {
            return Err("--max-render-duration-ms must be > 0".to_string());
        }
    }
    if let (Some(start), Some(end)) = (cli.start, cli.end) {
        let span_ms = ((f64::from(end) - f64::from(start)) * 1000.0).round() as i64;
        let budget_ms = cli
            .max_render_duration_ms
            .unwrap_or(DEFAULT_MAX_RENDER_DURATION_MS);
        if span_ms > budget_ms {
            return Err(format!(
                "render timeline span exceeds max budget ({}ms > {}ms)",
                span_ms, budget_ms
            ));
        }
    }
    Ok(())
}
fn load_autoplay_mods_config(path: &Path) -> Result<AutoplayModsConfig, String> {
    let raw = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read --autoplay-mods-config {}: {error}",
            path.display()
        )
    })?;
    parse_autoplay_mods_config_json(&raw)
}
fn load_intro_user_data(path: &Path) -> Result<IntroUserData, String> {
    let raw = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read --intro-user-json {}: {error}",
            path.display()
        )
    })?;
    let mut data: IntroUserData = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "failed to parse --intro-user-json {}: {error}",
            path.display()
        )
    })?;
    let base_dir = path.parent().unwrap_or(Path::new("."));
    data.resolve_relative_to(base_dir);
    validate_intro_user_data_paths(&data, path)?;
    Ok(data)
}
fn validate_intro_user_data_paths(
    data: &IntroUserData,
    manifest_path: &Path,
) -> Result<(), String> {
    for (field_name, path) in [
        ("avatar_path", data.avatar_path.as_ref()),
        ("flag_path", data.flag_path.as_ref()),
        ("team_badge_path", data.team_badge_path.as_ref()),
    ] {
        if let Some(path) = path {
            if !path.exists() {
                return Err(format!(
                    "--intro-user-json {} references missing {} {}",
                    manifest_path.display(),
                    field_name,
                    path.display()
                ));
            }
        }
    }
    Ok(())
}
#[derive(Debug, Serialize)]
struct BeatmapInspectionJson {
    md5: Option<String>,
    beatmap_id: Option<u32>,
    beatmapset_id: Option<u32>,
    has_background_video: bool,
    has_storyboard: bool,
    osu_path: String,
}
fn parse_beatmap_ids_from_content(content: &str) -> (Option<u32>, Option<u32>) {
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
fn inspect_beatmap_from_osu(osu_path: &Path) -> Result<BeatmapInspectionJson, String> {
    let content = fs::read_to_string(osu_path)
        .map_err(|e| format!("failed to read {}: {e}", osu_path.display()))?;
    let beatmap = parser::parse_osu_file(osu_path).map_err(|e| e.to_string())?;
    let md5 = fs::read(osu_path)
        .map(|bytes| format!("{:x}", md5::compute(bytes)))
        .map_err(|e| format!("failed to read {}: {e}", osu_path.display()))?;
    let (beatmap_id, beatmapset_id) = parse_beatmap_ids_from_content(&content);
    Ok(BeatmapInspectionJson {
        md5: Some(md5),
        beatmap_id,
        beatmapset_id,
        has_background_video: beatmap.has_background_video(),
        has_storyboard: beatmap.has_storyboard(),
        osu_path: osu_path.display().to_string(),
    })
}
fn inspect_beatmap_from_replay(cli: &Cli) -> Result<BeatmapInspectionJson, String> {
    let replay_path = cli
        .replay
        .as_ref()
        .ok_or_else(|| "--inspect-beatmap requires --replay or --osu".to_string())?;
    let replay = parser::parse_osr_file(replay_path).map_err(|e| e.to_string())?;
    let resolve_options = beatmaps::ResolveOptions {
        osu: cli.osu.clone(),
    };
    let resolved = beatmaps::resolve_beatmap_from_replay(&replay, &resolve_options)
        .map_err(|e| e.to_string())?;
    let beatmap = parser::parse_osu_file(&resolved.osu_path).map_err(|e| e.to_string())?;
    Ok(BeatmapInspectionJson {
        md5: Some(resolved.md5),
        beatmap_id: resolved.beatmap_id,
        beatmapset_id: resolved.beatmapset_id,
        has_background_video: beatmap.has_background_video(),
        has_storyboard: beatmap.has_storyboard(),
        osu_path: resolved.osu_path.display().to_string(),
    })
}
fn main() -> ExitCode {
    let cli = Cli::parse();
    if cli.list_autoplay_mods {
        match serde_json::to_string_pretty(&autoplay_mod_catalog()) {
            Ok(json) => {
                println!("{json}");
                return ExitCode::SUCCESS;
            }
            Err(error) => {
                eprintln!("err: failed to serialize autoplay mod catalog: {error}");
                return ExitCode::from(1);
            }
        }
    }
    if let Err(error) = validate_cli(&cli) {
        eprintln!("err: {error}");
        return ExitCode::from(1);
    }
    if cli.list_diffs {
        let songs_dir = cli.songs_dir.clone().unwrap_or_else(default_cache_dir);
        let default_songs_dir = if cli.songs_dir.is_none() {
            Some(songs_dir.clone())
        } else {
            None
        };
        let cache_base = songs_dir.join("MapsetCache");
        let ctx = match beatmaps::load_mapset_with_options(
            cli.mapset.as_ref().unwrap(),
            &cache_base,
            parser::ParseBeatmapOptions {
                storyboard_enabled: !cli.no_storyboard,
            },
        ) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("err: {}", e);
                return ExitCode::from(1);
            }
        };
        let _mapset_cache_cleanup = mapset_cache_cleanup_for(
            cli.mapset.as_ref().unwrap(),
            &ctx.root_dir,
            &cache_base,
            default_songs_dir,
        );
        let json = beatmaps::diffs_to_json(&ctx.diffs);
        match serde_json::to_string_pretty(&json) {
            Ok(text) => {
                println!("{text}");
                return ExitCode::SUCCESS;
            }
            Err(e) => {
                eprintln!("err: {}", e);
                return ExitCode::from(1);
            }
        }
    }
    if cli.inspect_beatmap {
        let inspection = if let Some(osu_path) = cli.osu.as_ref() {
            inspect_beatmap_from_osu(osu_path)
        } else {
            inspect_beatmap_from_replay(&cli)
        };
        match inspection.and_then(|payload| {
            serde_json::to_string_pretty(&payload)
                .map_err(|e| format!("failed to serialize inspect output: {e}"))
        }) {
            Ok(text) => {
                println!("{text}");
                return ExitCode::SUCCESS;
            }
            Err(error) => {
                eprintln!("err: {error}");
                return ExitCode::from(1);
            }
        }
    }
    let motion_blur_percent = cli.motion_blur;
    let encoder = match cli.encoder.parse::<VideoEncoder>() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("err: {}", e);
            return ExitCode::from(1);
        }
    };
    let gpu_preference = match cli.gpu_preference.parse::<GpuPreference>() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("err: {}", e);
            return ExitCode::from(1);
        }
    };
    let bg_compose_mode = match cli.bg_compose.parse::<BgComposeMode>() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("err: {}", e);
            return ExitCode::from(1);
        }
    };
    let parsed_hud_config = if let Some(path) = cli.hud_config.as_ref() {
        let raw = match fs::read_to_string(path) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("err: failed to read --hud-config {}: {}", path.display(), e);
                return ExitCode::from(1);
            }
        };
        match parse_hud_config_json(&raw) {
            Ok(cfg) => Some(cfg),
            Err(e) => {
                eprintln!("err: {}", e);
                return ExitCode::from(1);
            }
        }
    } else {
        None
    };
    let autoplay_mods_config = if let Some(path) = cli.autoplay_mods_config.as_ref() {
        match load_autoplay_mods_config(path) {
            Ok(config) => Some(config),
            Err(error) => {
                eprintln!("err: {error}");
                return ExitCode::from(1);
            }
        }
    } else {
        None
    };
    let intro_user_data = if let Some(path) = cli.intro_user_json.as_ref() {
        match load_intro_user_data(path) {
            Ok(data) => Some(data),
            Err(error) => {
                eprintln!("err: {error}");
                return ExitCode::from(1);
            }
        }
    } else {
        None
    };
    let settings = ConverterSettings {
        width: cli.width,
        height: cli.height,
        fps: cli.fps,
        scroll_speed: cli.scroll_speed,
        lead_in_ms: cli.lead_in,
        preset: cli.preset.clone(),
        encoder,
        motion_blur_percent,
        enable_hud: !cli.skip_hud,
        enable_lighting: !cli.no_lighting,
        enable_barlines: cli.barlines,
        intro_enabled: !cli.no_intro,
        sv_enabled: !cli.no_sv,
        skin_animations_enabled: !cli.no_skin_animations,
        combo_images_enabled: !cli.no_combo_burst,
        background_dim: cli
            .bg_opacity
            .map(|v| v.clamp(0.0, 100.0) / 100.0)
            .unwrap_or(1.0),
        background_blur_percent: cli.bg_blur,
        background_offset_x: cli.bg_offset_x,
        background_offset_y: cli.bg_offset_y,
        background_video_enabled: !cli.no_bg_video,
        storyboard_enabled: !cli.no_storyboard,
        ffmpeg_threads: cli.ffmpeg_threads,
        bg_compose_mode,
        gpu_preference,
        music_volume_percent: cli.music_volume,
        hitsound_volume_percent: cli.hitsound_volume,
        scroll_direction: cli
            .scroll_direction
            .as_deref()
            .and_then(ScrollDirection::parse),
        ..Default::default()
    };
    let mut selected_osu = cli.osu.clone();
    let mut _mapset_cache_cleanup: Option<MapsetCacheCleanup> = None;
    if let Some(mapset_path) = cli.mapset.as_ref() {
        let songs_dir = cli.songs_dir.clone().unwrap_or_else(default_cache_dir);
        let default_songs_dir = if cli.songs_dir.is_none() {
            Some(songs_dir.clone())
        } else {
            None
        };
        let cache_base = songs_dir.join("MapsetCache");
        let ctx = match beatmaps::load_mapset_with_options(
            mapset_path,
            &cache_base,
            parser::ParseBeatmapOptions {
                storyboard_enabled: settings.storyboard_enabled,
            },
        ) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("err: {}", e);
                return ExitCode::from(1);
            }
        };
        _mapset_cache_cleanup =
            mapset_cache_cleanup_for(mapset_path, &ctx.root_dir, &cache_base, default_songs_dir);
        let parsed_replay = if let Some(replay_path) = cli.replay.as_ref() {
            if !replay_path.exists() {
                eprintln!("err: replay file not found: {}", replay_path.display());
                return ExitCode::from(1);
            }
            let replay = match parser::parse_osr_file(replay_path) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("err: {}", e);
                    return ExitCode::from(1);
                }
            };
            Some(replay)
        } else {
            None
        };
        let diff = match if let Some(idx) = cli.diff_index {
            if let Some(replay) = parsed_replay.as_ref() {
                beatmaps::select_diff_by_index_for_replay(&ctx.diffs, idx, replay)
            } else {
                beatmaps::select_diff_by_index(&ctx.diffs, idx)
            }
        } else if let Some(replay) = parsed_replay.as_ref() {
            beatmaps::select_diff_for_replay(&ctx.diffs, cli.difficulty.as_deref(), replay)
        } else {
            beatmaps::select_diff(&ctx.diffs, cli.difficulty.as_deref(), None)
        } {
            Ok(d) => d,
            Err(e) => {
                eprintln!("err: {}", e);
                return ExitCode::from(1);
            }
        };
        selected_osu = Some(diff.path.clone());
    }
    let resolve_opts = ResolveOpts {
        osu: selected_osu.clone(),
        start_seconds: cli.start,
        end_seconds: cli.end,
        audio_local_only: cli.mapset.is_some() || cli.osu.is_some(),
        autoplay_mods: autoplay_mods_config,
        intro_user_data,
        ..Default::default()
    };
    let mut converter = ManiaVideoConverter::new(settings);
    converter.set_hud_config(parsed_hud_config);
    if cli.note_debug {
        converter.set_note_debug(true);
        converter.set_ln_debug(true);
    }
    if cli.all_presses {
        converter.set_all_presses(true);
    }
    if cli.preview_out.is_some() {
        converter.set_progress_callback(Box::new(move |pct, msg| {
            let safe_msg = msg.replace(['\r', '\n'], " ");
            println!("[preview-progress] {} {}", pct.min(100), safe_msg);
        }));
    }
    if cli.preview_results {
        if let Some(preview_path) = &cli.preview_out {
            match converter.render_results_preview_frame(
                cli.replay.as_deref(),
                preview_path,
                cli.skin.as_deref(),
                &resolve_opts,
                cli.preview_results_elements.as_deref(),
            ) {
                Ok(path) => {
                    println!("ok: preview saved to {}", path.display());
                    return ExitCode::SUCCESS;
                }
                Err(e) => {
                    eprintln!("err: preview failed: {}", e);
                    return ExitCode::from(1);
                }
            }
        }
    }
    if cli.replay.is_none() && selected_osu.is_none() && cli.hud_editor_preview {
        if let Some(preview_path) = &cli.preview_out {
            match converter.render_static_hud_editor_preview_frame(
                preview_path,
                cli.skin.as_deref(),
                cli.hud_editor_layout_only,
            ) {
                Ok(path) => {
                    println!("ok: preview saved to {}", path.display());
                    return ExitCode::SUCCESS;
                }
                Err(e) => {
                    eprintln!("err: preview failed: {}", e);
                    return ExitCode::from(1);
                }
            }
        }
    }
    let result = if let Some(replay_path) = cli.replay {
        if !replay_path.exists() {
            eprintln!("err: replay file not found: {}", replay_path.display());
            return ExitCode::from(1);
        }
        if cli.dry_run {
            println!("-> dry-run mode: computing judgments only...");
            match converter.analyze_judgments_only(&replay_path, &resolve_opts) {
                Ok(report) => {
                    if let (Some(report_path), Some(report)) =
                        (cli.report_out.as_ref(), report.as_ref())
                    {
                        write_replay_integrity_report(report_path, report);
                    }
                    println!("ok: dry-run completed");
                    return ExitCode::SUCCESS;
                }
                Err(e) => {
                    eprintln!("err: {}", e);
                    return ExitCode::from(1);
                }
            }
        }
        if let Some(preview_path) = &cli.preview_out {
            match converter.render_preview_frame(
                &replay_path,
                preview_path,
                cli.skin.as_deref(),
                &resolve_opts,
                cli.preview_time_ms,
                cli.hud_editor_preview,
            ) {
                Ok(path) => {
                    println!("ok: preview saved to {}", path.display());
                    if cli.output.is_none() {
                        return ExitCode::SUCCESS;
                    }
                }
                Err(e) => {
                    eprintln!("err: preview failed: {}", e);
                    return ExitCode::from(1);
                }
            }
        }
        let output = cli.output.unwrap();
        let pb = ProgressBar::new(100);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}% {msg}")
                .unwrap()
                .progress_chars("##-"),
        );
        converter.set_progress_callback(Box::new(move |pct, msg| {
            pb.set_position(pct as u64);
            pb.set_message(msg.to_string());
        }));
        converter.convert_replay_to_video(&replay_path, &output, cli.skin.as_deref(), &resolve_opts)
    } else if let Some(osu_path) = selected_osu {
        if !osu_path.exists() {
            eprintln!("err: beatmap file not found: {}", osu_path.display());
            return ExitCode::from(1);
        }
        if cli.dry_run {
            println!("-> dry-run mode: autoplay judgments only...");
            match converter.analyze_autoplay_judgments_only(&osu_path, &resolve_opts) {
                Ok(report) => {
                    if let (Some(report_path), Some(report)) =
                        (cli.report_out.as_ref(), report.as_ref())
                    {
                        write_replay_integrity_report(report_path, report);
                    }
                    println!("ok: dry-run completed");
                    return ExitCode::SUCCESS;
                }
                Err(e) => {
                    eprintln!("err: {}", e);
                    return ExitCode::from(1);
                }
            }
        }
        let output = cli.output.unwrap();
        let pb = ProgressBar::new(100);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}% {msg}")
                .unwrap()
                .progress_chars("##-"),
        );
        converter.set_progress_callback(Box::new(move |pct, msg| {
            pb.set_position(pct as u64);
            pb.set_message(msg.to_string());
        }));
        converter.convert_autoplay_to_video(&osu_path, &output, cli.skin.as_deref(), &resolve_opts)
    } else {
        unreachable!()
    };
    match result {
        Ok(res) => {
            if let (Some(report_path), Some(report)) =
                (cli.report_out.as_ref(), res.replay_integrity.as_ref())
            {
                write_replay_integrity_report(report_path, report);
            }
            println!("ok: video generated -> {}", res.output_path.display());
            println!(
                "   frames: {}, time: {}ms",
                res.frames_rendered, res.elapsed_ms
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("err: {}", e);
            ExitCode::from(1)
        }
    }
}
fn write_replay_integrity_report(report_path: &Path, report: &ReplayIntegrityReport) {
    if let Some(parent) = report_path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            eprintln!(
                "warn: failed to create report directory {}: {}",
                parent.display(),
                err
            );
        }
    }
    match serde_json::to_string_pretty(report) {
        Ok(json) => {
            if let Err(err) = fs::write(report_path, json) {
                eprintln!(
                    "warn: failed to write report {}: {}",
                    report_path.display(),
                    err
                );
            }
        }
        Err(err) => {
            eprintln!("warn: failed to serialize report: {}", err);
        }
    }
}

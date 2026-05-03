use super::audio::detect_audio_sample_rate;
use super::playback::PlaybackClock;
use crate::utils::mods::{PlaybackAudioMode, PlaybackRateProfile};
use crate::utils::perf;
use std::collections::VecDeque;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoEncoder {
    X264,
    Auto,
    Nvenc,
    Amf,
    Qsv,
}
impl VideoEncoder {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::X264 => "x264",
            Self::Auto => "auto",
            Self::Nvenc => "nvenc",
            Self::Amf => "amf",
            Self::Qsv => "qsv",
        }
    }
    pub fn is_hardware(self) -> bool {
        matches!(self, Self::Nvenc | Self::Amf | Self::Qsv)
    }
}
impl std::str::FromStr for VideoEncoder {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "x264" => Ok(Self::X264),
            "auto" => Ok(Self::Auto),
            "nvenc" => Ok(Self::Nvenc),
            "amf" => Ok(Self::Amf),
            "qsv" => Ok(Self::Qsv),
            _ => Err(format!("unknown encoder: {s}")),
        }
    }
}
#[derive(Debug, Default)]
struct EncoderSupport {
    nvenc: bool,
    amf: bool,
    qsv: bool,
}
#[derive(Debug, Default)]
struct FilterSupport {
    overlay_cuda: bool,
    scale_cuda: bool,
    overlay_qsv: bool,
    scale_qsv: bool,
    overlay_vulkan: bool,
    scale_vulkan: bool,
    overlay_opencl: bool,
    scale_opencl: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilterBackend {
    Cuda,
    Qsv,
    Vulkan,
    OpenCl,
}
#[derive(Debug)]
struct VideoFilterPlan {
    graph: String,
    hw_args: Vec<String>,
    bg_input_args: Vec<String>,
    uses_hw: bool,
}
#[derive(Debug, Clone)]
struct MotionBlurPlan {
    taps: u16,
    filter: String,
    mode: &'static str,
    nominal_taps: u16,
}
#[derive(Debug)]
struct BuiltFfmpegArgs {
    args: Vec<String>,
    filter_complex_script_path: Option<PathBuf>,
    temp_input_paths: Vec<PathBuf>,
}
#[derive(Debug)]
struct PreparedInputPath {
    path: String,
    temp_path: Option<PathBuf>,
}
#[derive(Debug, Default)]
struct TempInputPaths {
    paths: Vec<PathBuf>,
    armed: bool,
}
impl TempInputPaths {
    fn push(&mut self, path: PathBuf) {
        self.armed = true;
        self.paths.push(path);
    }
    fn into_paths(mut self) -> Vec<PathBuf> {
        self.armed = false;
        std::mem::take(&mut self.paths)
    }
}
impl Drop for TempInputPaths {
    fn drop(&mut self) {
        if self.armed {
            remove_temp_files(&mut self.paths);
        }
    }
}
fn filter_backend_name(backend: FilterBackend) -> &'static str {
    match backend {
        FilterBackend::Cuda => "cuda",
        FilterBackend::Qsv => "qsv",
        FilterBackend::Vulkan => "vulkan",
        FilterBackend::OpenCl => "opencl",
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BgComposeMode {
    #[default]
    Auto,
    Cpu,
    Hw,
}
impl BgComposeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Cpu => "cpu",
            Self::Hw => "hw",
        }
    }
}
impl std::str::FromStr for BgComposeMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "cpu" => Ok(Self::Cpu),
            "hw" => Ok(Self::Hw),
            _ => Err(format!("unknown bg compose mode: {s}")),
        }
    }
}
fn probe_video_dimensions(path: &str) -> Option<(u32, u32)> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0:s=x",
            path,
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let dims = text.trim();
    if dims.is_empty() {
        return None;
    }
    let mut parts = dims.split('x');
    let w = parts.next()?.trim().parse::<u32>().ok()?;
    let h = parts.next()?.trim().parse::<u32>().ok()?;
    Some((w, h))
}
fn background_needs_scale(bg: &BackgroundInput, w: u32, h: u32) -> bool {
    if !matches!(bg.kind, BackgroundKind::Video) {
        return true;
    }
    match probe_video_dimensions(&bg.path) {
        Some((bw, bh)) => bw != w || bh != h,
        None => true,
    }
}
fn build_background_cover_filter(w: u32, h: u32) -> String {
    format!(
        "scale=w='if(gte(iw*{h},ih*{w}),-2,{w})':h='if(gte(iw*{h},ih*{w}),{h},-2)',crop={w}:{h}:(iw-{w})/2:(ih-{h})/2"
    )
}
fn probe_ffmpeg_encoders() -> EncoderSupport {
    let output = Command::new("ffmpeg")
        .args(["-hide_banner", "-encoders"])
        .output();
    let Ok(out) = output else {
        return EncoderSupport::default();
    };
    let mut text = String::new();
    text.push_str(&String::from_utf8_lossy(&out.stdout));
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    let text = text.to_ascii_lowercase();
    EncoderSupport {
        nvenc: text.contains("h264_nvenc"),
        amf: text.contains("h264_amf"),
        qsv: text.contains("h264_qsv"),
    }
}
fn probe_ffmpeg_filters() -> FilterSupport {
    let output = Command::new("ffmpeg")
        .args(["-hide_banner", "-filters"])
        .output();
    let Ok(out) = output else {
        return FilterSupport::default();
    };
    let mut text = String::new();
    text.push_str(&String::from_utf8_lossy(&out.stdout));
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    let text = text.to_ascii_lowercase();
    FilterSupport {
        overlay_cuda: text.contains("overlay_cuda"),
        scale_cuda: text.contains("scale_cuda"),
        overlay_qsv: text.contains("overlay_qsv"),
        scale_qsv: text.contains("scale_qsv"),
        overlay_vulkan: text.contains("overlay_vulkan"),
        scale_vulkan: text.contains("scale_vulkan"),
        overlay_opencl: text.contains("overlay_opencl"),
        scale_opencl: text.contains("scale_opencl"),
    }
}
#[cfg(windows)]
fn dll_search_paths() -> Vec<std::path::PathBuf> {
    let mut paths: Vec<std::path::PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(windir) = std::env::var_os("WINDIR") {
        let base = std::path::PathBuf::from(windir);
        paths.push(base.join("System32"));
        paths.push(base.join("SysWOW64"));
    }
    paths
}
#[cfg(windows)]
fn dll_on_path(names: &[&str]) -> bool {
    let paths = dll_search_paths();
    for name in names {
        for dir in &paths {
            if dir.join(name).exists() {
                return true;
            }
        }
    }
    false
}
#[cfg(windows)]
fn dll_available_for_encoder(encoder: VideoEncoder) -> bool {
    match encoder {
        VideoEncoder::Nvenc => dll_on_path(&["nvcuda.dll", "nvencodeapi64.dll", "nvencodeapi.dll"]),
        VideoEncoder::Amf => dll_on_path(&["amfrt64.dll", "amfrt.dll"]),
        VideoEncoder::Qsv => dll_on_path(&["libmfxhw64.dll", "libmfx.dll", "libmfx64.dll"]),
        _ => true,
    }
}
#[cfg(not(windows))]
fn dll_available_for_encoder(_encoder: VideoEncoder) -> bool {
    true
}
fn validate_hw_encoder(encoder: VideoEncoder) -> bool {
    if !dll_available_for_encoder(encoder) {
        return false;
    }
    // Encoder listings can lie when drivers are missing, so encode one tiny lavfi frame.
    let codec = match encoder {
        VideoEncoder::Nvenc => "h264_nvenc",
        VideoEncoder::Amf => "h264_amf",
        VideoEncoder::Qsv => "h264_qsv",
        _ => return true,
    };
    let mut args = vec![
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "color=c=black:s=1280x720:d=0.1:r=30",
        "-frames:v",
        "1",
        "-c:v",
        codec,
    ];
    if matches!(encoder, VideoEncoder::Amf | VideoEncoder::Qsv) {
        args.extend(["-pix_fmt", "nv12"]);
    }
    args.extend(["-f", "null", "-"]);
    let status = Command::new("ffmpeg")
        .args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    status.map(|s| s.success()).unwrap_or(false)
}
fn encoder_candidates(requested: VideoEncoder) -> &'static [VideoEncoder] {
    match requested {
        VideoEncoder::Auto => &[VideoEncoder::Nvenc, VideoEncoder::Amf, VideoEncoder::Qsv],
        VideoEncoder::Nvenc => &[VideoEncoder::Nvenc],
        VideoEncoder::Amf => &[VideoEncoder::Amf],
        VideoEncoder::Qsv => &[VideoEncoder::Qsv],
        VideoEncoder::X264 => &[VideoEncoder::X264],
    }
}
fn resolve_encoder(requested: VideoEncoder) -> VideoEncoder {
    if requested == VideoEncoder::X264 {
        return VideoEncoder::X264;
    }
    let support = probe_ffmpeg_encoders();
    for &candidate in encoder_candidates(requested) {
        let available = match candidate {
            VideoEncoder::Nvenc => support.nvenc,
            VideoEncoder::Amf => support.amf,
            VideoEncoder::Qsv => support.qsv,
            VideoEncoder::X264 | VideoEncoder::Auto => false,
        };
        if !available {
            continue;
        }
        if !dll_available_for_encoder(candidate) {
            continue;
        }
        if validate_hw_encoder(candidate) {
            return candidate;
        }
    }
    VideoEncoder::X264
}
fn select_hw_backend(encoder: VideoEncoder, support: &FilterSupport) -> Option<FilterBackend> {
    match encoder {
        VideoEncoder::Nvenc => {
            if support.overlay_cuda && support.scale_cuda {
                Some(FilterBackend::Cuda)
            } else {
                None
            }
        }
        VideoEncoder::Qsv => {
            if support.overlay_qsv && support.scale_qsv {
                Some(FilterBackend::Qsv)
            } else {
                None
            }
        }
        VideoEncoder::Amf => {
            if support.overlay_vulkan && support.scale_vulkan {
                Some(FilterBackend::Vulkan)
            } else if support.overlay_opencl && support.scale_opencl {
                Some(FilterBackend::OpenCl)
            } else {
                None
            }
        }
        _ => None,
    }
}
struct HwBackendConfig {
    overlay: &'static str,
    hwupload: &'static str,
    overlay_supports_shortest: bool,
    init_args: Vec<String>,
    bg_input_args: Vec<String>,
    target_format: &'static str,
}
fn decide_bg_compose_backend(
    requested: BgComposeMode,
    resolved_encoder: VideoEncoder,
    _motion_blur_percent: u8,
    support: &FilterSupport,
) -> Option<FilterBackend> {
    let prefer_cpu = match requested {
        BgComposeMode::Cpu => true,
        // AMF hardware overlay paths are less reliable than CPU composition on common Windows setups.
        BgComposeMode::Auto => resolved_encoder == VideoEncoder::Amf,
        BgComposeMode::Hw => false,
    };
    if prefer_cpu {
        return None;
    }
    if !resolved_encoder.is_hardware() {
        return None;
    }
    select_hw_backend(resolved_encoder, support)
}
fn hw_backend_config(backend: FilterBackend) -> HwBackendConfig {
    match backend {
        FilterBackend::Cuda => HwBackendConfig {
            overlay: "overlay_cuda",
            hwupload: "hwupload_cuda",
            overlay_supports_shortest: true,
            init_args: vec![
                "-init_hw_device".into(),
                "cuda=hw:0".into(),
                "-filter_hw_device".into(),
                "hw".into(),
            ],
            bg_input_args: vec![
                "-hwaccel".into(),
                "cuda".into(),
                "-hwaccel_output_format".into(),
                "cuda".into(),
            ],
            target_format: "rgba",
        },
        FilterBackend::Qsv => HwBackendConfig {
            overlay: "overlay_qsv",
            hwupload: "hwupload=extra_hw_frames=64",
            overlay_supports_shortest: true,
            init_args: vec![
                "-init_hw_device".into(),
                "qsv=hw".into(),
                "-filter_hw_device".into(),
                "hw".into(),
            ],
            bg_input_args: vec![
                "-hwaccel".into(),
                "qsv".into(),
                "-hwaccel_output_format".into(),
                "qsv".into(),
            ],
            target_format: "nv12",
        },
        FilterBackend::Vulkan => HwBackendConfig {
            overlay: "overlay_vulkan",
            hwupload: "hwupload",
            overlay_supports_shortest: false,
            init_args: vec![
                "-init_hw_device".into(),
                "vulkan=hw".into(),
                "-filter_hw_device".into(),
                "hw".into(),
            ],
            bg_input_args: Vec::new(),
            target_format: "rgba",
        },
        FilterBackend::OpenCl => HwBackendConfig {
            overlay: "overlay_opencl",
            hwupload: "hwupload",
            overlay_supports_shortest: false,
            init_args: vec![
                "-init_hw_device".into(),
                "opencl=hw".into(),
                "-filter_hw_device".into(),
                "hw".into(),
            ],
            bg_input_args: Vec::new(),
            target_format: "rgba",
        },
    }
}
fn build_video_filter_cpu(
    w: u32,
    h: u32,
    fps: u32,
    bg: Option<&BackgroundInput>,
    bg_idx: Option<usize>,
    bg_needs_scale: bool,
    bg_time_filter: Option<&str>,
    bg_tail_pad_filter: Option<&str>,
    bg_blur_filter: Option<&str>,
    motion_blur_filter: Option<&str>,
    output_fmt: &str,
) -> Option<String> {
    if let (Some(bg), Some(bg_idx)) = (bg, bg_idx) {
        let mut bg_chain: Vec<String> = Vec::new();
        if bg_needs_scale {
            bg_chain.push(build_background_cover_filter(w, h));
        }
        if let Some(time_filter) = bg_time_filter {
            bg_chain.push(time_filter.to_string());
        }
        if matches!(bg.kind, BackgroundKind::Video) {
            bg_chain.push(format!("fps={}", fps));
        }
        if let Some(tail_pad_filter) = bg_tail_pad_filter {
            bg_chain.push(tail_pad_filter.to_string());
        }
        if let Some(blur_filter) = bg_blur_filter {
            bg_chain.push(blur_filter.to_string());
        }
        let visible = (1.0 - bg.dim).clamp(0.0, 1.0);
        if visible < 1.0 - 1e-6 {
            let f = format!("{:.3}", visible);
            bg_chain.push(format!("colorchannelmixer=rr={f}:gg={f}:bb={f}:aa=1"));
        }
        let fg_chain = "null";
        let mut steps = vec![
            format!("[{}:v]{}[bgprep]", bg_idx, bg_chain.join(",")),
            format!("[0:v]{}[fgprep]", fg_chain),
            "[bgprep][fgprep]overlay=shortest=1:format=gbrp[vtmp]".into(),
        ];
        let mut last = "vtmp".to_string();
        if let Some(blur) = motion_blur_filter {
            steps.push(format!("[{last}]{blur}[vblur]"));
            last = "vblur".to_string();
        }
        steps.push(format!(
            // Raw renderer frames are full-range RGB; encode output as BT.709 limited video.
            "[{last}]zscale=matrixin=gbr:matrix=709:rangein=full:range=limited,format={output_fmt}[outv]"
        ));
        return Some(steps.join(";"));
    }
    motion_blur_filter.map(|blur| format!("[0:v]{blur},format={output_fmt}[outv]"))
}
fn build_video_filter_hw(
    w: u32,
    h: u32,
    fps: u32,
    bg: &BackgroundInput,
    bg_idx: usize,
    bg_needs_scale: bool,
    bg_time_filter: Option<&str>,
    bg_tail_pad_filter: Option<&str>,
    bg_blur_filter: Option<&str>,
    backend: FilterBackend,
    output_fmt: &str,
    motion_blur_filter: Option<&str>,
) -> VideoFilterPlan {
    let cfg = hw_backend_config(backend);
    let visible = (1.0 - bg.dim).clamp(0.0, 1.0);
    let mut bg_chain: Vec<String> = Vec::new();
    if let Some(time_filter) = bg_time_filter {
        bg_chain.push(time_filter.to_string());
    }
    if bg_needs_scale {
        bg_chain.push(build_background_cover_filter(w, h));
    }
    if let Some(blur_filter) = bg_blur_filter {
        bg_chain.push(blur_filter.to_string());
    }
    bg_chain.push("format=rgba".into());
    bg_chain.push(format!("fps={}", fps));
    if let Some(tail_pad_filter) = bg_tail_pad_filter {
        bg_chain.push(tail_pad_filter.to_string());
    }
    if visible < 1.0 - 1e-6 {
        let f = format!("{:.3}", visible);
        bg_chain.push(format!("colorchannelmixer=rr={f}:gg={f}:bb={f}:aa=1"));
    }
    if cfg.target_format == "nv12" {
        bg_chain.push("format=nv12".into());
    }
    bg_chain.push(cfg.hwupload.into());
    let mut fg_chain: Vec<String> = Vec::new();
    if cfg.target_format == "nv12" {
        fg_chain.push("format=nv12".into());
    }
    fg_chain.push(cfg.hwupload.into());
    let overlay_step = if cfg.overlay_supports_shortest {
        format!("[bgprep][fgprep]{}=shortest=1[vtmp]", cfg.overlay)
    } else {
        format!("[bgprep][fgprep]{}[vtmp]", cfg.overlay)
    };
    let mut steps = vec![
        format!("[{}:v]{}[bgprep]", bg_idx, bg_chain.join(",")),
        format!("[0:v]{}[fgprep]", fg_chain.join(",")),
        overlay_step,
    ];
    let mut last = "vtmp".to_string();
    if let Some(blur) = motion_blur_filter {
        // tmix runs on CPU, so hardware-composed frames must be downloaded before motion blur.
        steps.push(format!(
            "[{last}]hwdownload,format=rgba,format=gbrp[{last}_cpu]"
        ));
        last = format!("{last}_cpu");
        steps.push(format!("[{last}]{blur}[vblur]"));
        last = "vblur".to_string();
    } else if cfg.target_format == "rgba" {
        steps.push(format!(
            "[{last}]hwdownload,format=rgba,format=gbrp[{last}_cpu]"
        ));
        last = format!("{last}_cpu");
    } else {
        steps.push(format!("[{last}]hwdownload,format={output_fmt}[outv]"));
        return VideoFilterPlan {
            graph: steps.join(";"),
            hw_args: cfg.init_args,
            bg_input_args: cfg.bg_input_args,
            uses_hw: true,
        };
    }
    steps.push(format!(
        "[{last}]zscale=matrixin=gbr:matrix=709:rangein=full:range=limited,format={output_fmt}[outv]"
    ));
    VideoFilterPlan {
        graph: steps.join(";"),
        hw_args: cfg.init_args,
        bg_input_args: cfg.bg_input_args,
        uses_hw: true,
    }
}
#[derive(Debug, Clone, Copy)]
enum PresetTier {
    Speed,
    Balanced,
    Quality,
}
fn preset_tier(preset: &str) -> PresetTier {
    match preset.trim().to_ascii_lowercase().as_str() {
        "ultrafast" | "superfast" | "veryfast" => PresetTier::Speed,
        "faster" | "fast" | "medium" => PresetTier::Balanced,
        "slow" | "slower" | "veryslow" => PresetTier::Quality,
        _ => PresetTier::Balanced,
    }
}
fn nvenc_preset(preset: &str) -> &'static str {
    match preset_tier(preset) {
        PresetTier::Speed => "p1",
        PresetTier::Balanced => "p4",
        PresetTier::Quality => "p7",
    }
}
fn amf_quality(preset: &str) -> &'static str {
    match preset_tier(preset) {
        PresetTier::Speed => "speed",
        PresetTier::Balanced => "balanced",
        PresetTier::Quality => "quality",
    }
}
fn qsv_preset(preset: &str) -> &'static str {
    match preset_tier(preset) {
        PresetTier::Speed => "veryfast",
        PresetTier::Balanced => "medium",
        PresetTier::Quality => "slow",
    }
}
fn output_pix_fmt(encoder: VideoEncoder) -> &'static str {
    match encoder {
        VideoEncoder::Amf | VideoEncoder::Qsv => "nv12",
        _ => "yuv420p",
    }
}
fn format_seconds(value: f64) -> String {
    format!("{value:.6}")
}
fn volume_ratio_from_percent(percent: f32) -> f64 {
    if percent.is_finite() {
        (percent.clamp(0.0, 100.0) as f64) / 100.0
    } else {
        1.0
    }
}
fn round_ms_to_u32(value: f64) -> u32 {
    value.max(0.0).round().clamp(0.0, u32::MAX as f64) as u32
}
fn push_audio_rate_adjust_filters(
    filters: &mut Vec<String>,
    audio_frequency_rate: f64,
    audio_tempo_rate: f64,
    input_sample_rate: Option<u32>,
) {
    if (audio_frequency_rate - 1.0).abs() >= 1e-6 {
        let input_sample_rate = input_sample_rate.unwrap_or(44_100);
        // asetrate changes pitch; aresample returns the stream to the original sample rate.
        filters.push(format!(
            "asetrate={}",
            format_seconds(input_sample_rate as f64 * audio_frequency_rate)
        ));
        filters.push(format!("aresample={input_sample_rate}"));
    }
    if (audio_tempo_rate - 1.0).abs() >= 1e-6 {
        filters.push(format!("atempo={}", format_seconds(audio_tempo_rate)));
    }
}
const VARIABLE_AUDIO_SEGMENT_MS: i32 = 250;
const ADAPTIVE_AUDIO_COMMAND_MS: i32 = 100;
const ADAPTIVE_AUDIO_FILTER_NAME: &str = "miru_as";
const FAIL_AUDIO_EFFECT_MS: u32 = 1_300;
const FAIL_AUDIO_SEGMENT_MS: i32 = 80;
const FAIL_AUDIO_MIN_PITCH: f64 = 0.55;
const FAIL_AUDIO_LOWPASS_HZ: u32 = 550;
fn instant_audio_rates(rate: f64, adjust_pitch: bool) -> (f64, f64) {
    if adjust_pitch {
        (rate, 1.0)
    } else {
        (1.0, rate)
    }
}
fn ease_out_cubic_f64(progress: f64) -> f64 {
    let progress = progress.clamp(0.0, 1.0);
    1.0 - (1.0 - progress).powi(3)
}
fn build_segment_boundaries_ms(
    start_ms: i32,
    end_ms: i32,
    segment_ms: i32,
    forced_points_ms: &[i32],
) -> Vec<i32> {
    let mut boundaries = vec![start_ms, end_ms];
    let segment_ms = segment_ms.max(1);
    // Forced boundaries keep abrupt rate changes from being averaged across a segment.
    let mut next_boundary = start_ms.div_euclid(segment_ms) * segment_ms;
    if next_boundary <= start_ms {
        next_boundary += segment_ms;
    }
    while next_boundary < end_ms {
        boundaries.push(next_boundary);
        next_boundary += segment_ms;
    }
    boundaries.extend(
        forced_points_ms
            .iter()
            .copied()
            .filter(|point_ms| *point_ms > start_ms && *point_ms < end_ms),
    );
    boundaries.sort_unstable();
    boundaries.dedup();
    boundaries
}
fn build_adaptive_audio_command_updates(
    playback_profile: &PlaybackRateProfile,
    start_ms: i32,
    end_ms: i32,
) -> Vec<(f64, f64)> {
    let end_ms = end_ms.max(start_ms + 1);
    let mut updates = Vec::new();
    let mut beatmap_ms = start_ms + ADAPTIVE_AUDIO_COMMAND_MS;
    // asendcmd updates only when the rate changes enough to matter, keeping filter scripts small.
    let mut last_rate = playback_profile
        .rate_at_beatmap_time_ms(start_ms as f64)
        .max(1e-6);
    while beatmap_ms < end_ms {
        let rate = playback_profile
            .rate_at_beatmap_time_ms(beatmap_ms as f64)
            .max(1e-6);
        if (rate - last_rate).abs() >= 1e-4 {
            updates.push(((beatmap_ms - start_ms) as f64 / 1000.0, rate));
            last_rate = rate;
        }
        beatmap_ms += ADAPTIVE_AUDIO_COMMAND_MS;
    }
    let tail_rate = playback_profile
        .rate_at_beatmap_time_ms(end_ms as f64)
        .max(1e-6);
    let tail_time_sec = (end_ms - start_ms).max(0) as f64 / 1000.0;
    if tail_time_sec > 0.0 && (tail_rate - last_rate).abs() >= 1e-4 {
        updates.push((tail_time_sec, tail_rate));
    }
    updates
}
fn build_adaptive_audio_command_string(
    playback_profile: &PlaybackRateProfile,
    start_ms: i32,
    end_ms: i32,
    adjust_pitch: bool,
) -> String {
    let target = if adjust_pitch {
        format!("rubberband@{ADAPTIVE_AUDIO_FILTER_NAME}")
    } else {
        format!("atempo@{ADAPTIVE_AUDIO_FILTER_NAME}")
    };
    build_adaptive_audio_command_updates(playback_profile, start_ms, end_ms)
        .into_iter()
        .map(|(time_sec, rate)| {
            let rate = format_seconds(rate);
            if adjust_pitch {
                format!(
                    "{:.6} [enter] {target} tempo {rate},[enter] {target} pitch {rate}",
                    time_sec
                )
            } else {
                format!("{:.6} [enter] {target} tempo {rate}", time_sec)
            }
        })
        .collect::<Vec<_>>()
        .join(";")
}
fn build_piecewise_output_time_expr(
    playback_profile: &PlaybackRateProfile,
    timeline_start_ms: i32,
    start_ms: i32,
    end_ms: i32,
    beatmap_time_expr_sec: &str,
) -> String {
    let playback_clock = PlaybackClock::new(timeline_start_ms, playback_profile.clone());
    let boundaries = build_segment_boundaries_ms(
        start_ms,
        end_ms.max(start_ms + 1),
        VARIABLE_AUDIO_SEGMENT_MS,
        &playback_profile.key_beatmap_boundaries_ms(),
    );
    // FFmpeg setpts needs a nested expression, so approximate variable rate by short piecewise windows.
    let mut windows = boundaries
        .windows(2)
        .map(|window| {
            let seg_start_ms = window[0];
            let seg_end_ms = window[1];
            let mid_ms = seg_start_ms as f64 + (seg_end_ms - seg_start_ms) as f64 / 2.0;
            let rate = playback_profile.rate_at_beatmap_time_ms(mid_ms).max(1e-6);
            let output_start_sec =
                playback_clock.output_elapsed_ms_for_beatmap_time(seg_start_ms as f64) / 1000.0;
            (
                seg_start_ms as f64 / 1000.0,
                seg_end_ms as f64 / 1000.0,
                output_start_sec,
                rate,
            )
        })
        .collect::<Vec<_>>();
    if windows.is_empty() {
        let rate = playback_profile.initial_rate().max(1e-6);
        return format!("(({beatmap_time_expr_sec})/{rate:.12})");
    }
    let (last_start_sec, _, last_output_start_sec, last_rate) = *windows.last().unwrap();
    let mut expr = format!(
        "{last_output_start_sec:.12}+((({beatmap_time_expr_sec})-({last_start_sec:.12}))/{last_rate:.12})"
    );
    for (seg_start_sec, seg_end_sec, output_start_sec, rate) in
        windows.drain(..windows.len() - 1).rev()
    {
        let current = format!(
            "{output_start_sec:.12}+((({beatmap_time_expr_sec})-({seg_start_sec:.12}))/{rate:.12})"
        );
        expr =
            format!("if(lt(({beatmap_time_expr_sec})\\,{seg_end_sec:.12})\\,{current}\\,{expr})");
    }
    expr
}
fn build_rate_driven_segmented_audio_chain(
    input_label: &str,
    output_label: &str,
    start_ms: i32,
    end_ms: i32,
    segment_ms: i32,
    delay_ms: Option<u32>,
    playback_profile: PlaybackRateProfile,
    adjust_pitch: bool,
    input_sample_rate: Option<u32>,
    forced_points_ms: &[i32],
) -> Vec<String> {
    let end_ms = end_ms.max(start_ms + 1);
    let boundaries = build_segment_boundaries_ms(start_ms, end_ms, segment_ms, forced_points_ms);
    let base_output_label = format!("{output_label}_base");
    let mut steps = Vec::new();
    let mut segment_labels = Vec::new();
    for (idx, window) in boundaries.windows(2).enumerate() {
        let seg_start_ms = window[0];
        let seg_end_ms = window[1];
        if seg_end_ms <= seg_start_ms {
            continue;
        }
        let seg_label = format!("{output_label}_seg{idx}");
        let mid_ms = seg_start_ms as f64 + (seg_end_ms - seg_start_ms) as f64 / 2.0;
        // Each audio segment uses the midpoint rate to approximate non-linear playback profiles.
        let rate = playback_profile.rate_at_beatmap_time_ms(mid_ms).max(1e-6);
        let (audio_frequency_rate, audio_tempo_rate) = instant_audio_rates(rate, adjust_pitch);
        steps.push(build_audio_filter_chain(
            input_label,
            &seg_label,
            Some(seg_start_ms as f64 / 1000.0),
            Some((seg_end_ms - seg_start_ms) as f64 / 1000.0),
            None,
            audio_frequency_rate,
            audio_tempo_rate,
            input_sample_rate,
            &[],
        ));
        segment_labels.push(seg_label);
    }
    if segment_labels.is_empty() {
        steps.push(build_audio_filter_chain(
            input_label,
            &base_output_label,
            Some(start_ms as f64 / 1000.0),
            Some(0.001),
            None,
            1.0,
            1.0,
            input_sample_rate,
            &[],
        ));
    } else if segment_labels.len() == 1 {
        steps.push(format!("[{}]anull[{base_output_label}]", segment_labels[0]));
    } else {
        let inputs = segment_labels
            .iter()
            .map(|label| format!("[{label}]"))
            .collect::<String>();
        steps.push(format!(
            "{inputs}concat=n={}:v=0:a=1[{base_output_label}]",
            segment_labels.len()
        ));
    }
    if let Some(delay_ms) = delay_ms.filter(|delay_ms| *delay_ms > 0) {
        steps.push(format!(
            "[{base_output_label}]adelay={delay_ms}|{delay_ms}[{output_label}]"
        ));
    } else {
        steps.push(format!("[{base_output_label}]anull[{output_label}]"));
    }
    steps
}
fn build_adaptive_command_driven_audio_chain(
    input_label: &str,
    output_label: &str,
    start_ms: i32,
    end_ms: i32,
    delay_ms: Option<u32>,
    playback_profile: PlaybackRateProfile,
    adjust_pitch: bool,
) -> String {
    let end_ms = end_ms.max(start_ms + 1);
    let initial_rate = playback_profile
        .rate_at_beatmap_time_ms(start_ms as f64)
        .max(1e-6);
    let mut filters = Vec::new();
    filters.push(format!(
        "atrim=start={}:duration={}",
        format_seconds(start_ms.max(0) as f64 / 1000.0),
        format_seconds((end_ms - start_ms) as f64 / 1000.0)
    ));
    filters.push("asetpts=PTS-STARTPTS".to_string());
    let commands =
        build_adaptive_audio_command_string(&playback_profile, start_ms, end_ms, adjust_pitch);
    if !commands.is_empty() {
        filters.push(format!("asendcmd=c='{commands}'"));
    }
    if adjust_pitch {
        filters.push(format!(
            "rubberband@{ADAPTIVE_AUDIO_FILTER_NAME}=tempo={}:pitch={}:channels=together",
            format_seconds(initial_rate),
            format_seconds(initial_rate)
        ));
    } else {
        filters.push(format!(
            "atempo@{ADAPTIVE_AUDIO_FILTER_NAME}={}",
            format_seconds(initial_rate)
        ));
    }
    if let Some(delay_ms) = delay_ms.filter(|delay_ms| *delay_ms > 0) {
        filters.push(format!("adelay={delay_ms}|{delay_ms}"));
    }
    format!("{input_label}{}[{output_label}]", filters.join(","))
}
fn build_audio_filter_chain(
    input_label: &str,
    output_label: &str,
    trim_start_sec: Option<f64>,
    trim_duration_sec: Option<f64>,
    delay_ms: Option<u32>,
    audio_frequency_rate: f64,
    audio_tempo_rate: f64,
    input_sample_rate: Option<u32>,
    extra_filters: &[String],
) -> String {
    let mut filters = Vec::new();
    if trim_start_sec.is_some() || trim_duration_sec.is_some() {
        let mut atrim = String::from("atrim");
        let mut parts = Vec::new();
        if let Some(start_sec) = trim_start_sec {
            parts.push(format!("start={}", format_seconds(start_sec.max(0.0))));
        }
        if let Some(duration_sec) = trim_duration_sec {
            parts.push(format!(
                "duration={}",
                format_seconds(duration_sec.max(0.0))
            ));
        }
        if !parts.is_empty() {
            atrim.push('=');
            atrim.push_str(&parts.join(":"));
        }
        filters.push(atrim);
    }
    filters.push("asetpts=PTS-STARTPTS".to_string());
    push_audio_rate_adjust_filters(
        &mut filters,
        audio_frequency_rate,
        audio_tempo_rate,
        input_sample_rate,
    );
    filters.extend(extra_filters.iter().cloned());
    if let Some(delay_ms) = delay_ms.filter(|delay_ms| *delay_ms > 0) {
        filters.push(format!("adelay={delay_ms}|{delay_ms}"));
    }
    format!("{input_label}{}[{output_label}]", filters.join(","))
}
fn fail_audio_start_output_ms(opts: &ComposeOpts) -> Option<f64> {
    let fail_time_ms = opts.fail_time_ms?;
    let playback_clock = PlaybackClock::new(opts.timeline_start_ms, opts.playback_profile.clone());
    let output_ms = opts.intro_duration_ms as f64
        + playback_clock.output_elapsed_ms_for_beatmap_time(fail_time_ms as f64);
    output_ms.is_finite().then_some(output_ms.max(0.0))
}
fn compose_output_duration_seconds(opts: &ComposeOpts) -> Option<f64> {
    let fps = opts.fps.max(1) as f64;
    let playback_clock = PlaybackClock::new(opts.timeline_start_ms, opts.playback_profile.clone());
    let output_ms = opts.intro_duration_ms as f64
        + playback_clock.output_elapsed_ms_for_beatmap_time(opts.timeline_end_ms as f64);
    if !output_ms.is_finite() || output_ms <= 0.0 {
        return None;
    }
    let frame_count = (output_ms / 1000.0 * fps).ceil().max(1.0);
    Some((frame_count + 2.0) / fps)
}
fn build_background_tail_pad_filter(output_duration_sec: Option<f64>) -> Option<String> {
    let duration_sec = output_duration_sec?;
    if !duration_sec.is_finite() || duration_sec <= 0.0 {
        return None;
    }
    // Video backgrounds must outlive raw frame input so overlay=shortest follows gameplay, not bg EOF.
    Some(format!(
        "tpad=stop_mode=clone:stop_duration={}",
        format_seconds(duration_sec + 1.0)
    ))
}
fn build_fail_audio_effect_steps(
    input_label: &str,
    output_label: &str,
    fail_start_sec: f64,
    duration_sec: f64,
    input_sample_rate: Option<u32>,
) -> Vec<String> {
    let fail_start_sec = fail_start_sec.max(0.0);
    let duration_sec = duration_sec.max(0.001);
    let segment_count = ((duration_sec * 1000.0) / FAIL_AUDIO_SEGMENT_MS as f64)
        .ceil()
        .max(1.0) as usize;
    let has_pre_fail = fail_start_sec > 0.0005;
    let split_count = segment_count + usize::from(has_pre_fail);
    let split_labels = (0..split_count)
        .map(|idx| format!("{output_label}_src{idx}"))
        .collect::<Vec<_>>();
    let mut steps = Vec::new();
    steps.push(format!(
        "[{input_label}]asplit={split_count}{}",
        split_labels
            .iter()
            .map(|label| format!("[{label}]"))
            .collect::<String>()
    ));
    let mut next_split = 0usize;
    let pre_label = if has_pre_fail {
        let label = format!("{output_label}_pre");
        steps.push(format!(
            "[{}]atrim=end={},asetpts=PTS-STARTPTS[{label}]",
            split_labels[next_split],
            format_seconds(fail_start_sec)
        ));
        next_split += 1;
        Some(label)
    } else {
        None
    };
    let input_sample_rate = input_sample_rate.unwrap_or(44_100);
    let mut segment_labels = Vec::with_capacity(segment_count);
    for segment_idx in 0..segment_count {
        let local_start_sec = segment_idx as f64 * FAIL_AUDIO_SEGMENT_MS as f64 / 1000.0;
        let local_end_sec =
            ((segment_idx + 1) as f64 * FAIL_AUDIO_SEGMENT_MS as f64 / 1000.0).min(duration_sec);
        let segment_duration_sec = (local_end_sec - local_start_sec).max(0.001);
        let midpoint = (local_start_sec + segment_duration_sec * 0.5) / duration_sec;
        let eased = ease_out_cubic_f64(midpoint);
        // The fail effect lowers pitch in slices while compensating tempo to preserve duration.
        let pitch = 1.0 + (FAIL_AUDIO_MIN_PITCH - 1.0) * eased;
        let tempo = 1.0 / pitch.max(0.01);
        let segment_label = format!("{output_label}_seg{segment_idx}");
        steps.push(format!(
            "[{}]atrim=start={}:duration={},asetpts=PTS-STARTPTS,asetrate={},aresample={input_sample_rate},atempo={}[{segment_label}]",
            split_labels[next_split],
            format_seconds(fail_start_sec + local_start_sec),
            format_seconds(segment_duration_sec),
            format_seconds(input_sample_rate as f64 * pitch),
            format_seconds(tempo)
        ));
        next_split += 1;
        segment_labels.push(segment_label);
    }
    let fail_raw_label = format!("{output_label}_fail_raw");
    if segment_labels.len() == 1 {
        steps.push(format!("[{}]anull[{fail_raw_label}]", segment_labels[0]));
    } else {
        let inputs = segment_labels
            .iter()
            .map(|label| format!("[{label}]"))
            .collect::<String>();
        steps.push(format!(
            "{inputs}concat=n={}:v=0:a=1[{fail_raw_label}]",
            segment_labels.len()
        ));
    }
    let fail_tail_label = format!("{output_label}_fail_tail");
    steps.push(format!(
        "[{fail_raw_label}]lowpass=f={FAIL_AUDIO_LOWPASS_HZ},afade=t=out:st=0:d={}[{fail_tail_label}]",
        format_seconds(duration_sec)
    ));
    if let Some(pre_label) = pre_label {
        steps.push(format!(
            "[{pre_label}][{fail_tail_label}]concat=n=2:v=0:a=1[{output_label}]"
        ));
    } else {
        steps.push(format!("[{fail_tail_label}]anull[{output_label}]"));
    }
    steps
}
fn build_audio_filter_steps(
    opts: &ComposeOpts,
    audio_index: usize,
    gain_index: Option<usize>,
    music_overlay_indices: &[usize],
    effects_overlay_indices: &[usize],
    input_sample_rate: Option<u32>,
) -> Vec<String> {
    let input_label = format!("[{audio_index}:a]");
    let preview_available = opts.intro_duration_ms > 0 && opts.preview_time_ms > 0;
    let visual_playback_clock =
        PlaybackClock::new(opts.timeline_start_ms, opts.playback_profile.clone());
    let audio_playback_profile = opts
        .audio_playback_profile
        .as_ref()
        .unwrap_or(&opts.playback_profile);
    let main_start_sec = visual_playback_clock
        .beatmap_time_for_output_elapsed_ms(0.0)
        .max(0.0)
        / 1000.0;
    let preroll_ms = round_ms_to_u32(visual_playback_clock.output_preroll_ms());
    let base_output = "music_base";
    let mut steps = Vec::new();
    match opts.audio_mode {
        PlaybackAudioMode::StaticSplit => {
            if preview_available {
                let intro_sec = opts.intro_duration_ms as f64 / 1000.0;
                let preview_sec = opts.preview_time_ms as f64 / 1000.0;
                let intro_source_duration_sec = visual_playback_clock
                    .source_duration_ms_for_output_duration(opts.intro_duration_ms as f64)
                    / 1000.0;
                let intro_effects = vec![
                    "lowpass=f=400".to_string(),
                    format!(
                        "volume='if(isnan(t),1.5,1.5-1.2*t/{})':eval=frame",
                        format_seconds(intro_sec.max(0.1))
                    ),
                    format!(
                        "afade=t=out:st={}:d=0.3",
                        format_seconds((intro_sec - 0.3).max(0.0))
                    ),
                ];
                steps.push(build_audio_filter_chain(
                    &input_label,
                    "intro_audio",
                    Some(preview_sec),
                    Some(intro_source_duration_sec),
                    None,
                    opts.audio_frequency_rate,
                    opts.audio_tempo_rate,
                    input_sample_rate,
                    &intro_effects,
                ));
                steps.push(build_audio_filter_chain(
                    &input_label,
                    "main_audio",
                    (main_start_sec > 0.0).then_some(main_start_sec),
                    None,
                    Some(preroll_ms),
                    opts.audio_frequency_rate,
                    opts.audio_tempo_rate,
                    input_sample_rate,
                    &[],
                ));
                steps.push(format!(
                    "[intro_audio][main_audio]concat=n=2:v=0:a=1[{base_output}]"
                ));
            } else {
                let total_delay_ms = round_ms_to_u32(
                    visual_playback_clock.output_preroll_ms() + opts.intro_duration_ms as f64,
                );
                steps.push(build_audio_filter_chain(
                    &input_label,
                    base_output,
                    (main_start_sec > 0.0).then_some(main_start_sec),
                    None,
                    Some(total_delay_ms),
                    opts.audio_frequency_rate,
                    opts.audio_tempo_rate,
                    input_sample_rate,
                    &[],
                ));
            }
        }
        PlaybackAudioMode::RateDriven { adjust_pitch } => {
            let main_start_ms = visual_playback_clock
                .beatmap_time_for_output_elapsed_ms(0.0)
                .max(0.0)
                .round() as i32;
            if preview_available {
                let intro_sec = opts.intro_duration_ms as f64 / 1000.0;
                let preview_rate = if audio_playback_profile.is_adaptive() {
                    audio_playback_profile.initial_rate()
                } else {
                    audio_playback_profile.rate_at_beatmap_time_ms(opts.preview_time_ms as f64)
                }
                .max(1e-6);
                let (audio_frequency_rate, audio_tempo_rate) =
                    instant_audio_rates(preview_rate, adjust_pitch);
                let preview_sec = opts.preview_time_ms as f64 / 1000.0;
                let intro_source_duration_sec =
                    opts.intro_duration_ms as f64 * preview_rate / 1000.0;
                let intro_effects = vec![
                    "lowpass=f=400".to_string(),
                    format!(
                        "volume='if(isnan(t),1.5,1.5-1.2*t/{})':eval=frame",
                        format_seconds(intro_sec.max(0.1))
                    ),
                    format!(
                        "afade=t=out:st={}:d=0.3",
                        format_seconds((intro_sec - 0.3).max(0.0))
                    ),
                ];
                steps.push(build_audio_filter_chain(
                    &input_label,
                    "intro_audio",
                    Some(preview_sec),
                    Some(intro_source_duration_sec),
                    None,
                    audio_frequency_rate,
                    audio_tempo_rate,
                    input_sample_rate,
                    &intro_effects,
                ));
                if audio_playback_profile.is_adaptive() {
                    steps.push(build_adaptive_command_driven_audio_chain(
                        &input_label,
                        "main_audio",
                        main_start_ms,
                        opts.timeline_end_ms.max(main_start_ms + 1),
                        Some(preroll_ms),
                        audio_playback_profile.clone(),
                        adjust_pitch,
                    ));
                } else {
                    let mut forced_points_ms = audio_playback_profile.key_beatmap_boundaries_ms();
                    forced_points_ms.extend([
                        opts.preview_time_ms as i32,
                        main_start_ms,
                        opts.timeline_end_ms,
                    ]);
                    steps.extend(build_rate_driven_segmented_audio_chain(
                        &input_label,
                        "main_audio",
                        main_start_ms,
                        opts.timeline_end_ms.max(main_start_ms + 1),
                        VARIABLE_AUDIO_SEGMENT_MS,
                        Some(preroll_ms),
                        audio_playback_profile.clone(),
                        adjust_pitch,
                        input_sample_rate,
                        &forced_points_ms,
                    ));
                }
                steps.push(format!(
                    "[intro_audio][main_audio]concat=n=2:v=0:a=1[{base_output}]"
                ));
            } else {
                let total_delay_ms = round_ms_to_u32(
                    visual_playback_clock.output_preroll_ms() + opts.intro_duration_ms as f64,
                );
                if audio_playback_profile.is_adaptive() {
                    steps.push(build_adaptive_command_driven_audio_chain(
                        &input_label,
                        base_output,
                        main_start_ms,
                        opts.timeline_end_ms.max(main_start_ms + 1),
                        Some(total_delay_ms),
                        audio_playback_profile.clone(),
                        adjust_pitch,
                    ));
                } else {
                    let mut forced_points_ms = audio_playback_profile.key_beatmap_boundaries_ms();
                    forced_points_ms.extend([
                        opts.preview_time_ms as i32,
                        main_start_ms,
                        opts.timeline_end_ms,
                    ]);
                    steps.extend(build_rate_driven_segmented_audio_chain(
                        &input_label,
                        base_output,
                        main_start_ms,
                        opts.timeline_end_ms.max(main_start_ms + 1),
                        VARIABLE_AUDIO_SEGMENT_MS,
                        Some(total_delay_ms),
                        audio_playback_profile.clone(),
                        adjust_pitch,
                        input_sample_rate,
                        &forced_points_ms,
                    ));
                }
            }
        }
    }
    let mut current_music_output = base_output.to_string();
    if let Some(gain_index) = gain_index {
        steps.push(format!(
            "[{current_music_output}][{gain_index}:a]amultiply[music_gain]"
        ));
        current_music_output = "music_gain".to_string();
    }
    if !music_overlay_indices.is_empty() {
        let overlay_inputs = music_overlay_indices
            .iter()
            .map(|overlay_index| format!("[{overlay_index}:a]"))
            .collect::<Vec<_>>()
            .join("");
        let input_count = music_overlay_indices.len() + 1;
        steps.push(format!(
            "[{current_music_output}]{overlay_inputs}amix=inputs={input_count}:duration=longest:dropout_transition=0:normalize=0[music_bus]"
        ));
        current_music_output = "music_bus".to_string();
    } else if current_music_output != "music_bus" {
        steps.push(format!("[{current_music_output}]anull[music_bus]"));
        current_music_output = "music_bus".to_string();
    }
    let music_volume = volume_ratio_from_percent(opts.music_volume_percent);
    if (music_volume - 1.0).abs() >= 1e-6 {
        steps.push(format!(
            "[{current_music_output}]volume={}[music_bus_scaled]",
            format_seconds(music_volume)
        ));
        current_music_output = "music_bus_scaled".to_string();
    }
    if let Some(fail_start_ms) = fail_audio_start_output_ms(opts) {
        let output_label = "music_fail_effect";
        steps.extend(build_fail_audio_effect_steps(
            &current_music_output,
            output_label,
            fail_start_ms / 1000.0,
            FAIL_AUDIO_EFFECT_MS as f64 / 1000.0,
            input_sample_rate,
        ));
        current_music_output = output_label.to_string();
    }
    let effects_output = if effects_overlay_indices.is_empty() {
        None
    } else {
        if effects_overlay_indices.len() == 1 {
            steps.push(format!(
                "[{}:a]anull[effects_bus]",
                effects_overlay_indices[0]
            ));
        } else {
            let overlay_inputs = effects_overlay_indices
                .iter()
                .map(|overlay_index| format!("[{overlay_index}:a]"))
                .collect::<Vec<_>>()
                .join("");
            steps.push(format!(
                "{overlay_inputs}amix=inputs={}:duration=longest:dropout_transition=0:normalize=0[effects_bus]",
                effects_overlay_indices.len()
            ));
        }
        let effects_volume = volume_ratio_from_percent(opts.hitsound_volume_percent);
        if (effects_volume - 1.0).abs() >= 1e-6 {
            steps.push(format!(
                "[effects_bus]volume={}[effects_bus_scaled]",
                format_seconds(effects_volume)
            ));
            Some("effects_bus_scaled")
        } else {
            Some("effects_bus")
        }
    };
    if let Some(effects_output) = effects_output {
        steps.push(format!(
            "[{current_music_output}][{effects_output}]amix=inputs=2:duration=longest:dropout_transition=0:normalize=0[audio_mix]"
        ));
    } else {
        steps.push(format!("[{current_music_output}]anull[audio_mix]"));
    }
    steps.push("[audio_mix]apad[final_audio]".to_string());
    steps
}
fn build_background_time_filter(
    bg: &BackgroundInput,
    timeline_start_ms: i32,
    timeline_end_ms: i32,
    playback_profile: PlaybackRateProfile,
) -> Option<String> {
    if !matches!(bg.kind, BackgroundKind::Video) {
        return None;
    }
    let playback_clock = PlaybackClock::new(timeline_start_ms, playback_profile.clone());
    match playback_profile {
        PlaybackRateProfile::Constant { .. } => {
            let output_offset_sec =
                playback_clock.output_elapsed_ms_for_beatmap_time(bg.start_time_ms as f64) / 1000.0;
            let mut expr = if (playback_clock.clock_rate() - 1.0).abs() < 1e-6 {
                "PTS".to_string()
            } else {
                format!("PTS/{}", format_seconds(playback_clock.clock_rate()))
            };
            if output_offset_sec.abs() > 1e-6 {
                let sign = if output_offset_sec < 0.0 { '-' } else { '+' };
                expr.push(sign);
                expr.push_str(&format!("{:.6}/TB", output_offset_sec.abs()));
            }
            Some(format!("setpts={expr}"))
        }
        PlaybackRateProfile::LinearRamp { .. } => {
            let beatmap_time_expr_sec = if bg.start_time_ms == 0 {
                "T".to_string()
            } else {
                format!("{:.6}+T", bg.start_time_ms as f64 / 1000.0)
            };
            let output_time_expr_sec =
                playback_clock.output_seconds_expr_for_beatmap_time_expr(&beatmap_time_expr_sec);
            Some(format!("setpts=({output_time_expr_sec})/TB"))
        }
        PlaybackRateProfile::Adaptive { .. } => {
            let beatmap_time_expr_sec = if bg.start_time_ms == 0 {
                "T".to_string()
            } else {
                format!("{:.6}+T", bg.start_time_ms as f64 / 1000.0)
            };
            let output_time_expr_sec = build_piecewise_output_time_expr(
                &playback_profile,
                timeline_start_ms,
                bg.start_time_ms.min(timeline_start_ms),
                timeline_end_ms.max(bg.start_time_ms.min(timeline_start_ms) + 1),
                &beatmap_time_expr_sec,
            );
            Some(format!("setpts=({output_time_expr_sec})/TB"))
        }
    }
}
fn build_background_blur_filter(percent: u8) -> Option<String> {
    if percent == 0 {
        return None;
    }
    Some(format!(
        "gblur=sigma={:.3}",
        crate::utils::image_proc::background_blur_sigma_from_percent(percent)
    ))
}
fn build_motion_blur_plan(percent: u8, fps: u32) -> Option<MotionBlurPlan> {
    if percent == 0 {
        return None;
    }
    const MAX_SHUTTER_MS: f32 = 130.0;
    const SHUTTER_CURVE: f32 = 1.05;
    const MIN_DECAY: f32 = 0.02;
    const MAX_DECAY: f32 = 0.58;
    const DECAY_CURVE: f32 = 0.70;

    let fps = fps.max(1) as f32;
    let s = percent as f32 / 100.0;
    let frame_ms = 1000.0 / fps;
    let shutter_ms = MAX_SHUTTER_MS * s.powf(SHUTTER_CURVE);
    let mut nominal_taps = (1.0 + shutter_ms / frame_ms).ceil() as u16;
    nominal_taps = nominal_taps.clamp(2, 16);
    let effective_taps_max = if fps >= 120.0 {
        10
    } else if fps >= 90.0 {
        12
    } else {
        16
    };
    let taps = nominal_taps.min(effective_taps_max);
    let decay = MIN_DECAY + (MAX_DECAY - MIN_DECAY) * s.powf(DECAY_CURVE);
    let mut weights = Vec::with_capacity(taps as usize);
    let age_scale = if taps > 1 {
        (nominal_taps.saturating_sub(1) as f32) / (taps.saturating_sub(1) as f32)
    } else {
        1.0
    };
    for i in 0..taps {
        let age = (taps - 1 - i) as f32 * age_scale;
        let w = decay.powf(age);
        weights.push(format!("{:.3}", w));
    }
    Some(MotionBlurPlan {
        taps,
        filter: format!("tmix=frames={taps}:weights='{}'", weights.join(" ")),
        mode: "balanced-readable",
        nominal_taps,
    })
}
fn write_filter_complex_script(graph: &str, output_path: &str) -> io::Result<PathBuf> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    let mut path = Path::new(output_path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(std::env::temp_dir);
    // Large composed graphs can exceed command-line limits, so pass them through a temp script.
    path.push(format!("miru_filter_complex_{pid}_{nanos}.ffscript"));
    fs::write(&path, graph)?;
    Ok(path)
}
fn remove_temp_file(path: &Path) {
    if let Err(err) = fs::remove_file(path) {
        if err.kind() != io::ErrorKind::NotFound {
            eprintln!(
                "   warn: failed to remove temp file {}: {}",
                path.display(),
                err
            );
        }
    }
}
fn remove_temp_files(paths: &mut Vec<PathBuf>) {
    for path in paths.drain(..) {
        remove_temp_file(&path);
    }
}
pub struct FrameComposer {
    child: Option<Child>,
    frame_tx: Option<SyncSender<Vec<u8>>>,
    writer_thread: Option<thread::JoinHandle<io::Result<()>>>,
    writer_error: Arc<Mutex<Option<String>>>,
    written_frames: Arc<AtomicU64>,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
    stderr_thread: Option<thread::JoinHandle<()>>,
    filter_complex_script_path: Option<PathBuf>,
    temp_input_paths: Vec<PathBuf>,
    requested_encoder: VideoEncoder,
    resolved_encoder: VideoEncoder,
    width: u32,
    height: u32,
    frame_size: usize,
}
const STDERR_TAIL_CAPACITY: usize = 24;
const FRAME_QUEUE_DEPTH: usize = 3;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposeFailureStage {
    Spawn,
    Write,
    Finish,
}
impl ComposeFailureStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Spawn => "spawn",
            Self::Write => "write",
            Self::Finish => "finish",
        }
    }
}
#[derive(Debug, Clone)]
pub struct ComposeFailure {
    pub requested_encoder: VideoEncoder,
    pub resolved_encoder: VideoEncoder,
    pub frames_written: u64,
    pub exit_code: Option<i32>,
    pub stderr_tail: Vec<String>,
    pub stage: ComposeFailureStage,
    pub message: String,
}
impl ComposeFailure {
    pub fn is_hardware_failure(&self) -> bool {
        self.resolved_encoder.is_hardware()
    }
}
impl std::fmt::Display for ComposeFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ffmpeg {} failed (requested {}, resolved {}, frames_written={}",
            self.stage.as_str(),
            self.requested_encoder.as_str(),
            self.resolved_encoder.as_str(),
            self.frames_written
        )?;
        if let Some(code) = self.exit_code {
            write!(f, ", exit_code={code}")?;
        }
        write!(f, "): {}", self.message)?;
        if !self.stderr_tail.is_empty() {
            write!(f, "\nffmpeg stderr tail:")?;
            for line in &self.stderr_tail {
                write!(f, "\n  {line}")?;
            }
        }
        Ok(())
    }
}
impl std::error::Error for ComposeFailure {}
#[derive(Debug)]
pub enum ComposeError {
    Io(io::Error),
    Ffmpeg(ComposeFailure),
}
impl std::fmt::Display for ComposeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "io: {err}"),
            Self::Ffmpeg(err) => write!(f, "{err}"),
        }
    }
}
impl std::error::Error for ComposeError {}
impl From<io::Error> for ComposeError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}
#[derive(Debug, Clone)]
pub enum BackgroundKind {
    Image,
    Video,
}
#[derive(Debug, Clone)]
pub struct BackgroundInput {
    pub kind: BackgroundKind,
    pub path: String,
    pub start_time_ms: i32,
    pub dim: f32,
}
#[derive(Debug, Clone)]
pub struct ComposeOpts {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub audio_path: Option<String>,
    pub output_path: String,
    pub preset: String,
    pub crf: u8,
    pub motion_blur_percent: u8,
    pub timeline_start_ms: i32,
    pub timeline_end_ms: i32,
    pub intro_duration_ms: u32,
    pub preview_time_ms: u32,
    pub background: Option<BackgroundInput>,
    pub background_blur_percent: u8,
    pub bg_compose_mode: BgComposeMode,
    pub encoder: VideoEncoder,
    pub ffmpeg_threads: Option<u32>,
    pub playback_profile: PlaybackRateProfile,
    pub audio_playback_profile: Option<PlaybackRateProfile>,
    pub audio_mode: PlaybackAudioMode,
    pub audio_frequency_rate: f64,
    pub audio_tempo_rate: f64,
    pub fail_time_ms: Option<i32>,
    pub music_volume_percent: f32,
    pub hitsound_volume_percent: f32,
    pub main_audio_gain_path: Option<String>,
    pub music_overlay_paths: Vec<String>,
    pub effects_overlay_paths: Vec<String>,
}
impl Default for ComposeOpts {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 60,
            audio_path: None,
            output_path: String::new(),
            preset: "veryfast".into(),
            crf: 18,
            motion_blur_percent: 0,
            timeline_start_ms: 0,
            timeline_end_ms: 0,
            intro_duration_ms: 0,
            preview_time_ms: 0,
            background: None,
            background_blur_percent: 0,
            bg_compose_mode: BgComposeMode::Auto,
            encoder: VideoEncoder::X264,
            ffmpeg_threads: None,
            playback_profile: PlaybackRateProfile::constant(1.0),
            audio_playback_profile: None,
            audio_mode: PlaybackAudioMode::StaticSplit,
            audio_frequency_rate: 1.0,
            audio_tempo_rate: 1.0,
            fail_time_ms: None,
            music_volume_percent: 100.0,
            hitsound_volume_percent: 100.0,
            main_audio_gain_path: None,
            music_overlay_paths: Vec::new(),
            effects_overlay_paths: Vec::new(),
        }
    }
}
fn push_stderr_tail_line(tail: &Arc<Mutex<VecDeque<String>>>, line: &str) {
    let normalized = line.trim();
    if normalized.is_empty() {
        return;
    }
    let mut tail = tail.lock().unwrap();
    if tail.len() == STDERR_TAIL_CAPACITY {
        // Keep only the latest stderr lines for compact failure reports.
        tail.pop_front();
    }
    tail.push_back(normalized.to_string());
}
fn flush_stderr_pending_line(tail: &Arc<Mutex<VecDeque<String>>>, pending: &mut String) {
    if pending.is_empty() {
        return;
    }
    push_stderr_tail_line(tail, pending);
    pending.clear();
}
fn capture_stderr_text(tail: &Arc<Mutex<VecDeque<String>>>, pending: &mut String, text: &str) {
    for ch in text.chars() {
        if matches!(ch, '\n' | '\r') {
            flush_stderr_pending_line(tail, pending);
        } else {
            pending.push(ch);
        }
    }
}
fn spawn_stderr_reader(
    stderr: ChildStderr,
) -> (Arc<Mutex<VecDeque<String>>>, thread::JoinHandle<()>) {
    let tail = Arc::new(Mutex::new(VecDeque::with_capacity(STDERR_TAIL_CAPACITY)));
    let thread_tail = Arc::clone(&tail);
    let handle = thread::spawn(move || {
        let mut stderr = stderr;
        let mut buf = [0u8; 4096];
        let mut pending = String::new();
        loop {
            match stderr.read(&mut buf) {
                Ok(0) => break,
                Ok(count) => {
                    let text = String::from_utf8_lossy(&buf[..count]);
                    eprint!("{text}");
                    let _ = io::stderr().flush();
                    capture_stderr_text(&thread_tail, &mut pending, &text);
                }
                Err(_) => break,
            }
        }
        flush_stderr_pending_line(&thread_tail, &mut pending);
    });
    (tail, handle)
}
fn run_frame_writer<W: Write>(
    mut stdin: W,
    frame_rx: Receiver<Vec<u8>>,
    written_frames: Arc<AtomicU64>,
    writer_error: Arc<Mutex<Option<String>>>,
) -> io::Result<()> {
    for frame in frame_rx {
        let _scope = perf::scoped("ffmpeg_write");
        if let Err(err) = stdin.write_all(&frame) {
            *writer_error.lock().unwrap() = Some(err.to_string());
            return Err(err);
        }
        written_frames.fetch_add(1, Ordering::Relaxed);
    }
    stdin.flush()?;
    Ok(())
}
impl FrameComposer {
    fn spawn_child(
        args: &[String],
    ) -> Result<(Child, Arc<Mutex<VecDeque<String>>>, thread::JoinHandle<()>), ComposeError> {
        let mut child = Command::new("ffmpeg")
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("ffmpeg stderr not captured"))?;
        let (stderr_tail, stderr_thread) = spawn_stderr_reader(stderr);
        Ok((child, stderr_tail, stderr_thread))
    }
    fn prepare_bg_path(bg: &BackgroundInput) -> PreparedInputPath {
        if !matches!(bg.kind, BackgroundKind::Image) {
            return PreparedInputPath {
                path: bg.path.clone(),
                temp_path: None,
            };
        }
        let ext = Path::new(&bg.path)
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase());
        let is_jpeg = matches!(ext.as_deref(), Some("jpg" | "jpeg" | "jfif"));
        if !is_jpeg {
            return PreparedInputPath {
                path: bg.path.clone(),
                temp_path: None,
            };
        }
        let Ok(img) = image::open(&bg.path) else {
            return PreparedInputPath {
                path: bg.path.clone(),
                temp_path: None,
            };
        };
        let rgba = img.to_rgba8();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let mut out_path = std::env::temp_dir();
        out_path.push(format!("miru_bg_{nanos}.png"));
        if rgba.save(&out_path).is_ok() {
            PreparedInputPath {
                path: out_path.to_string_lossy().into_owned(),
                temp_path: Some(out_path),
            }
        } else {
            PreparedInputPath {
                path: bg.path.clone(),
                temp_path: None,
            }
        }
    }
    pub fn spawn(opts: &ComposeOpts) -> Result<Self, ComposeError> {
        let w = opts.width.max(1);
        let h = opts.height.max(1);
        let fps = opts.fps.max(1);
        if let Some(parent) = Path::new(&opts.output_path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        let requested_encoder = opts.encoder;
        let resolved_encoder = resolve_encoder(requested_encoder);
        if resolved_encoder != requested_encoder && requested_encoder != VideoEncoder::Auto {
            eprintln!(
                "   warn: encoder {} unavailable, falling back to {}",
                requested_encoder.as_str(),
                resolved_encoder.as_str()
            );
        }
        println!(
            "   [ffmpeg] encoder: {} (requested: {})",
            resolved_encoder.as_str(),
            requested_encoder.as_str()
        );
        let output_fmt = output_pix_fmt(resolved_encoder);
        let motion_blur_plan = build_motion_blur_plan(opts.motion_blur_percent, fps);
        if let Some(plan) = motion_blur_plan.as_ref() {
            if plan.nominal_taps != plan.taps {
                println!(
                    "   [ffmpeg] motion blur: {}% -> {} taps (nominal {}) ({})",
                    opts.motion_blur_percent, plan.taps, plan.nominal_taps, plan.mode
                );
            } else {
                println!(
                    "   [ffmpeg] motion blur: {}% -> {} taps ({})",
                    opts.motion_blur_percent, plan.taps, plan.mode
                );
            }
        }
        let motion_blur_filter = motion_blur_plan.as_ref().map(|p| p.filter.as_str());
        let background_blur_filter = build_background_blur_filter(opts.background_blur_percent);
        let background_index = opts.background.as_ref().map(|_| 1usize);
        let bg_time_filter = opts.background.as_ref().and_then(|bg| {
            build_background_time_filter(
                bg,
                opts.timeline_start_ms,
                opts.timeline_end_ms,
                opts.playback_profile.clone(),
            )
        });
        let output_duration_sec = compose_output_duration_seconds(opts);
        let bg_tail_pad_filter = opts
            .background
            .as_ref()
            .filter(|bg| matches!(bg.kind, BackgroundKind::Video))
            .and_then(|_| build_background_tail_pad_filter(output_duration_sec));
        let bg_needs_scale = opts
            .background
            .as_ref()
            .map(|bg| background_needs_scale(bg, w, h))
            .unwrap_or(true);
        let cpu_graph = build_video_filter_cpu(
            w,
            h,
            fps,
            opts.background.as_ref(),
            background_index,
            bg_needs_scale,
            bg_time_filter.as_deref(),
            bg_tail_pad_filter.as_deref(),
            background_blur_filter.as_deref(),
            motion_blur_filter,
            output_fmt,
        );
        let cpu_plan = cpu_graph.map(|graph| VideoFilterPlan {
            graph,
            hw_args: Vec::new(),
            bg_input_args: Vec::new(),
            uses_hw: false,
        });
        let mut hw_plan: Option<VideoFilterPlan> = None;
        let mut hw_backend_used: Option<FilterBackend> = None;
        let mut bg_compose_log: Option<String> = None;
        if let Some(ref bg) = opts.background {
            if matches!(bg.kind, BackgroundKind::Video) {
                let requested = opts.bg_compose_mode;
                let prefer_cpu = matches!(requested, BgComposeMode::Cpu)
                    || (matches!(requested, BgComposeMode::Auto)
                        && resolved_encoder == VideoEncoder::Amf);
                let should_probe = !prefer_cpu && resolved_encoder.is_hardware();
                if should_probe {
                    let filter_support = probe_ffmpeg_filters();
                    if let Some(backend) = decide_bg_compose_backend(
                        requested,
                        resolved_encoder,
                        opts.motion_blur_percent,
                        &filter_support,
                    ) {
                        if let Some(bg_idx) = background_index {
                            hw_plan = Some(build_video_filter_hw(
                                w,
                                h,
                                fps,
                                bg,
                                bg_idx,
                                bg_needs_scale,
                                bg_time_filter.as_deref(),
                                bg_tail_pad_filter.as_deref(),
                                background_blur_filter.as_deref(),
                                backend,
                                output_fmt,
                                motion_blur_filter,
                            ));
                            hw_backend_used = Some(backend);
                        }
                    } else if matches!(requested, BgComposeMode::Hw) {
                        eprintln!(
                            "   warn: bg compose hw requested but hw filters are unavailable; using cpu"
                        );
                    }
                } else if matches!(requested, BgComposeMode::Hw) && !resolved_encoder.is_hardware()
                {
                    eprintln!(
                        "   warn: bg compose hw requested but encoder is not hardware; using cpu"
                    );
                }
                bg_compose_log = Some(if let Some(backend) = hw_backend_used {
                    format!("hw({})", filter_backend_name(backend))
                } else {
                    "cpu".to_string()
                });
            }
        }
        let build_args = |plan: Option<&VideoFilterPlan>| -> io::Result<BuiltFfmpegArgs> {
            let mut args: Vec<String> = vec!["-y".into()];
            let mut filter_complex_script_path: Option<PathBuf> = None;
            let mut temp_input_paths = TempInputPaths::default();
            if let Some(plan) = plan {
                if plan.uses_hw {
                    args.extend(plan.hw_args.clone());
                }
            }
            args.extend([
                "-f".into(),
                "rawvideo".into(),
                "-pix_fmt".into(),
                "rgba".into(),
                "-s".into(),
                format!("{}x{}", w, h),
                "-framerate".into(),
                fps.to_string(),
                "-i".into(),
                "-".into(),
            ]);
            let mut audio_index: Option<usize> = None;
            let mut main_audio_gain_index: Option<usize> = None;
            let mut music_overlay_indices = Vec::new();
            let mut effects_overlay_indices = Vec::new();
            let mut input_index = 1usize;
            if let Some(ref bg) = opts.background {
                let bg_path = Self::prepare_bg_path(bg);
                if let Some(temp_path) = bg_path.temp_path.as_ref() {
                    temp_input_paths.push(temp_path.clone());
                }
                if let Some(plan) = plan {
                    if plan.uses_hw && matches!(bg.kind, BackgroundKind::Video) {
                        args.extend(plan.bg_input_args.clone());
                    }
                }
                match bg.kind {
                    BackgroundKind::Video => {
                        args.extend(["-i".into(), bg_path.path.clone()]);
                    }
                    BackgroundKind::Image => {
                        args.extend(["-loop".into(), "1".into()]);
                        args.extend(["-framerate".into(), fps.to_string()]);
                        args.extend(["-i".into(), bg_path.path.clone()]);
                    }
                }
                input_index += 1;
            }
            if let Some(ref audio) = opts.audio_path {
                if Path::new(audio).exists() {
                    let cur_audio_index = input_index;
                    args.extend(["-i".into(), audio.clone()]);
                    audio_index = Some(cur_audio_index);
                    input_index += 1;
                }
            }
            if let Some(ref gain_track) = opts.main_audio_gain_path {
                if Path::new(gain_track).exists() {
                    let cur_gain_index = input_index;
                    args.extend(["-i".into(), gain_track.clone()]);
                    main_audio_gain_index = Some(cur_gain_index);
                    input_index += 1;
                }
            }
            for overlay in &opts.music_overlay_paths {
                if Path::new(overlay).exists() {
                    let cur_overlay_index = input_index;
                    args.extend(["-i".into(), overlay.clone()]);
                    music_overlay_indices.push(cur_overlay_index);
                    input_index += 1;
                }
            }
            for overlay in &opts.effects_overlay_paths {
                if Path::new(overlay).exists() {
                    let cur_overlay_index = input_index;
                    args.extend(["-i".into(), overlay.clone()]);
                    effects_overlay_indices.push(cur_overlay_index);
                    input_index += 1;
                }
            }
            let mut video_filter_complex = plan.map(|p| p.graph.clone());
            let mut use_filter_complex_audio = false;
            if let Some(a_idx) = audio_index {
                let input_sample_rate = if (opts.audio_frequency_rate - 1.0).abs() >= 1e-6 {
                    let audio_path = opts
                        .audio_path
                        .as_deref()
                        .ok_or_else(|| io::Error::other("missing audio path"))?;
                    Some(
                        detect_audio_sample_rate(Path::new(audio_path))
                            .map_err(|err| io::Error::other(err.to_string()))?,
                    )
                } else {
                    None
                };
                if let PlaybackAudioMode::RateDriven { .. } = opts.audio_mode {
                    if let Some(audio_profile) = opts.audio_playback_profile.as_ref() {
                        if audio_profile.is_adaptive() {
                            let visual_playback_clock = PlaybackClock::new(
                                opts.timeline_start_ms,
                                opts.playback_profile.clone(),
                            );
                            let main_start_ms = visual_playback_clock
                                .beatmap_time_for_output_elapsed_ms(0.0)
                                .max(0.0)
                                .round() as i32;
                            let updates = build_adaptive_audio_command_updates(
                                audio_profile,
                                main_start_ms,
                                opts.timeline_end_ms.max(main_start_ms + 1),
                            )
                            .len();
                            eprintln!(
                                "   [audio] adaptive playback: command-driven ({} updates)",
                                updates
                            );
                        }
                    }
                }
                let mut filter_steps = build_audio_filter_steps(
                    opts,
                    a_idx,
                    main_audio_gain_index,
                    &music_overlay_indices,
                    &effects_overlay_indices,
                    input_sample_rate,
                );
                let mut video_filter_used = false;
                if let Some(vf) = video_filter_complex.take() {
                    filter_steps.insert(0, vf);
                    video_filter_used = true;
                }
                let filter_complex_script =
                    write_filter_complex_script(&filter_steps.join(";"), &opts.output_path)?;
                args.extend([
                    "-filter_complex_script".into(),
                    filter_complex_script.to_string_lossy().into_owned(),
                    "-map".into(),
                    if video_filter_used {
                        "[outv]".to_string()
                    } else {
                        "0:v".to_string()
                    },
                    "-map".into(),
                    "[final_audio]".into(),
                ]);
                filter_complex_script_path = Some(filter_complex_script);
                use_filter_complex_audio = true;
                video_filter_complex = None;
            }
            if !use_filter_complex_audio {
                if let Some(vf) = video_filter_complex {
                    let filter_complex_script =
                        write_filter_complex_script(&vf, &opts.output_path)?;
                    args.extend([
                        "-filter_complex_script".into(),
                        filter_complex_script.to_string_lossy().into_owned(),
                        "-map".into(),
                        "[outv]".into(),
                    ]);
                    filter_complex_script_path = Some(filter_complex_script);
                } else {
                    args.extend(["-map".into(), "0:v".into()]);
                }
                if let Some(a_idx) = audio_index {
                    args.extend(["-map".into(), format!("{a_idx}:a")]);
                }
            }
            if let Some(threads) = opts.ffmpeg_threads {
                if threads > 0 {
                    args.extend(["-threads".into(), threads.to_string()]);
                }
            }
            if let Some(duration_sec) = compose_output_duration_seconds(opts) {
                args.extend(["-t".into(), format_seconds(duration_sec)]);
            }
            args.push("-shortest".into());
            match resolved_encoder {
                VideoEncoder::X264 => {
                    args.extend([
                        "-c:v".into(),
                        "libx264".into(),
                        "-pix_fmt".into(),
                        output_fmt.into(),
                        "-preset".into(),
                        opts.preset.clone(),
                        "-crf".into(),
                        opts.crf.to_string(),
                    ]);
                    let x264_params =
                        "colorprim=bt709:transfer=bt709:colormatrix=bt709:fullrange=off"
                            .to_string();
                    args.extend(["-x264-params".into(), x264_params]);
                }
                VideoEncoder::Nvenc => {
                    args.extend([
                        "-c:v".into(),
                        "h264_nvenc".into(),
                        "-pix_fmt".into(),
                        output_fmt.into(),
                        "-preset".into(),
                        nvenc_preset(&opts.preset).into(),
                        "-rc".into(),
                        "vbr".into(),
                        "-cq".into(),
                        opts.crf.to_string(),
                        "-color_primaries".into(),
                        "bt709".into(),
                        "-color_trc".into(),
                        "bt709".into(),
                        "-colorspace".into(),
                        "bt709".into(),
                        "-color_range".into(),
                        "tv".into(),
                    ]);
                }
                VideoEncoder::Amf => {
                    let crf = opts.crf.to_string();
                    args.extend([
                        "-c:v".into(),
                        "h264_amf".into(),
                        "-pix_fmt".into(),
                        output_fmt.into(),
                        "-quality".into(),
                        amf_quality(&opts.preset).into(),
                        "-rc".into(),
                        "cqp".into(),
                        "-qp_i".into(),
                        crf.clone(),
                        "-qp_p".into(),
                        crf.clone(),
                        "-qp_b".into(),
                        crf,
                        "-color_primaries".into(),
                        "bt709".into(),
                        "-color_trc".into(),
                        "bt709".into(),
                        "-colorspace".into(),
                        "bt709".into(),
                        "-color_range".into(),
                        "tv".into(),
                    ]);
                }
                VideoEncoder::Qsv => {
                    args.extend([
                        "-c:v".into(),
                        "h264_qsv".into(),
                        "-pix_fmt".into(),
                        output_fmt.into(),
                        "-preset".into(),
                        qsv_preset(&opts.preset).into(),
                        "-global_quality".into(),
                        opts.crf.to_string(),
                        "-color_primaries".into(),
                        "bt709".into(),
                        "-color_trc".into(),
                        "bt709".into(),
                        "-colorspace".into(),
                        "bt709".into(),
                        "-color_range".into(),
                        "tv".into(),
                    ]);
                }
                VideoEncoder::Auto => {
                    args.extend([
                        "-c:v".into(),
                        "libx264".into(),
                        "-pix_fmt".into(),
                        output_fmt.into(),
                        "-preset".into(),
                        opts.preset.clone(),
                        "-crf".into(),
                        opts.crf.to_string(),
                    ]);
                }
            }
            if opts.audio_path.is_some() && !use_filter_complex_audio {
                args.extend(["-c:a".into(), "aac".into(), "-b:a".into(), "192k".into()]);
            } else if use_filter_complex_audio {
                args.extend(["-c:a".into(), "aac".into(), "-b:a".into(), "192k".into()]);
            }
            args.extend([
                "-movflags".into(),
                "+faststart".into(),
                opts.output_path.clone(),
            ]);
            Ok(BuiltFfmpegArgs {
                args,
                filter_complex_script_path,
                temp_input_paths: temp_input_paths.into_paths(),
            })
        };
        let mut plan = hw_plan.as_ref().or(cpu_plan.as_ref());
        if let Some(mode) = bg_compose_log {
            println!("   [ffmpeg] bg compose: {}", mode);
        }
        let mut built_args = build_args(plan)?;
        let mut args = built_args.args;
        let mut filter_complex_script_path = built_args.filter_complex_script_path;
        let mut temp_input_paths = built_args.temp_input_paths;
        eprintln!("   ffmpeg {}", args.join(" "));
        let (mut child, mut stderr_tail, mut stderr_thread) = match Self::spawn_child(&args) {
            Ok(result) => result,
            Err(err) => {
                if let Some(path) = filter_complex_script_path.take() {
                    remove_temp_file(&path);
                }
                remove_temp_files(&mut temp_input_paths);
                return Err(err);
            }
        };
        if let Some(active_plan) = plan {
            if active_plan.uses_hw {
                let mut early_exit = None;
                for _ in 0..12 {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            early_exit = Some(status);
                            break;
                        }
                        Ok(None) => thread::sleep(Duration::from_millis(25)),
                        Err(_) => break,
                    }
                }
                if let Some(status) = early_exit {
                    eprintln!(
                        "   warn: ffmpeg hw filters exited early (code {:?}), retrying CPU filters",
                        status.code()
                    );
                    if let Some(cpu_plan) = cpu_plan.as_ref() {
                        // Some FFmpeg builds expose HW filters that fail at graph init; CPU is the safe retry.
                        let _ = child.wait();
                        let _ = stderr_thread.join();
                        if let Some(path) = filter_complex_script_path.take() {
                            remove_temp_file(&path);
                        }
                        remove_temp_files(&mut temp_input_paths);
                        plan = Some(cpu_plan);
                        built_args = build_args(plan)?;
                        args = built_args.args;
                        filter_complex_script_path = built_args.filter_complex_script_path;
                        temp_input_paths = built_args.temp_input_paths;
                        eprintln!("   ffmpeg {}", args.join(" "));
                        (child, stderr_tail, stderr_thread) = match Self::spawn_child(&args) {
                            Ok(result) => result,
                            Err(err) => {
                                if let Some(path) = filter_complex_script_path.take() {
                                    remove_temp_file(&path);
                                }
                                remove_temp_files(&mut temp_input_paths);
                                return Err(err);
                            }
                        };
                    } else {
                        let _ = child.wait();
                        let _ = stderr_thread.join();
                        if let Some(path) = filter_complex_script_path.take() {
                            remove_temp_file(&path);
                        }
                        remove_temp_files(&mut temp_input_paths);
                        let stderr_tail = stderr_tail.lock().unwrap().iter().cloned().collect();
                        return Err(ComposeError::Ffmpeg(ComposeFailure {
                            requested_encoder,
                            resolved_encoder,
                            frames_written: 0,
                            exit_code: status.code(),
                            stderr_tail,
                            stage: ComposeFailureStage::Spawn,
                            message: "ffmpeg exited before frame piping started".into(),
                        }));
                    }
                }
            }
        }
        let frame_size = (w * h * 4) as usize;
        let stdin = match child.stdin.take() {
            Some(stdin) => stdin,
            None => {
                remove_temp_files(&mut temp_input_paths);
                return Err(ComposeError::Io(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "ffmpeg stdin not captured",
                )));
            }
        };
        // A shallow sync queue provides backpressure without blocking the renderer on every write.
        let (frame_tx, frame_rx) = mpsc::sync_channel(FRAME_QUEUE_DEPTH);
        let writer_error = Arc::new(Mutex::new(None));
        let written_frames = Arc::new(AtomicU64::new(0));
        let writer_thread = {
            let writer_error = Arc::clone(&writer_error);
            let written_frames = Arc::clone(&written_frames);
            thread::spawn(move || run_frame_writer(stdin, frame_rx, written_frames, writer_error))
        };
        Ok(Self {
            child: Some(child),
            frame_tx: Some(frame_tx),
            writer_thread: Some(writer_thread),
            writer_error,
            written_frames,
            stderr_tail,
            stderr_thread: Some(stderr_thread),
            filter_complex_script_path,
            temp_input_paths,
            requested_encoder,
            resolved_encoder,
            width: w,
            height: h,
            frame_size,
        })
    }
    fn join_stderr_thread(&mut self) {
        if let Some(handle) = self.stderr_thread.take() {
            let _ = handle.join();
        }
    }
    fn stderr_tail_snapshot(&self) -> Vec<String> {
        self.stderr_tail.lock().unwrap().iter().cloned().collect()
    }
    fn ffmpeg_failure(
        &mut self,
        stage: ComposeFailureStage,
        message: impl Into<String>,
        exit_code: Option<i32>,
    ) -> ComposeError {
        self.join_stderr_thread();
        ComposeError::Ffmpeg(ComposeFailure {
            requested_encoder: self.requested_encoder,
            resolved_encoder: self.resolved_encoder,
            frames_written: self.frames_written(),
            exit_code,
            stderr_tail: self.stderr_tail_snapshot(),
            stage,
            message: message.into(),
        })
    }
    fn take_writer_error(&self) -> Option<String> {
        self.writer_error.lock().unwrap().clone()
    }
    fn join_writer_thread(&mut self) -> Result<(), io::Error> {
        let Some(handle) = self.writer_thread.take() else {
            return Ok(());
        };
        match handle.join() {
            Ok(result) => result,
            Err(_) => Err(io::Error::other("ffmpeg writer thread panicked")),
        }
    }
    fn cleanup_failed_child(&mut self) -> Option<ExitStatus> {
        self.frame_tx.take();
        let _ = self.join_writer_thread();
        let mut child = self.child.take()?;
        let status = match child.try_wait() {
            Ok(Some(status)) => Some(status),
            Ok(None) => {
                let _ = child.kill();
                child.wait().ok()
            }
            Err(_) => child.wait().ok(),
        };
        self.join_stderr_thread();
        status
    }
    pub fn push_frame(&mut self, rgba: &[u8]) -> Result<(), ComposeError> {
        if rgba.len() != self.frame_size {
            return Err(ComposeError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "frame size mismatch: got {}, expected {}",
                    rgba.len(),
                    self.frame_size
                ),
            )));
        }
        if self.child.is_none() {
            return Err(ComposeError::Io(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "ffmpeg process is no longer running",
            )));
        }
        let frame_tx = self.frame_tx.as_ref().ok_or_else(|| {
            ComposeError::Io(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "ffmpeg frame queue closed",
            ))
        })?;
        let enqueue_start = if perf::enabled() {
            Some(std::time::Instant::now())
        } else {
            None
        };
        if let Err(err) = frame_tx.send(rgba.to_vec()) {
            if let Some(start) = enqueue_start {
                perf::record("ffmpeg_enqueue", start.elapsed());
            }
            let status = self.cleanup_failed_child();
            let message = self
                .take_writer_error()
                .unwrap_or_else(|| format!("ffmpeg frame queue failed: {err}"));
            return Err(self.ffmpeg_failure(
                ComposeFailureStage::Write,
                message,
                status.and_then(|status| status.code()),
            ));
        }
        if let Some(start) = enqueue_start {
            perf::record("ffmpeg_enqueue", start.elapsed());
        }
        Ok(())
    }
    pub fn finish(mut self) -> Result<(), ComposeError> {
        let _scope = perf::scoped("ffmpeg_finish");
        self.frame_tx.take();
        if let Err(err) = self.join_writer_thread() {
            let status = self.cleanup_failed_child();
            let message = self.take_writer_error().unwrap_or_else(|| err.to_string());
            return Err(self.ffmpeg_failure(
                ComposeFailureStage::Write,
                message,
                status.and_then(|status| status.code()),
            ));
        }
        let mut child = self.child.take().ok_or_else(|| {
            ComposeError::Io(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "ffmpeg process is no longer running",
            ))
        })?;
        let status = child.wait()?;
        self.join_stderr_thread();
        if status.success() {
            Ok(())
        } else {
            Err(self.ffmpeg_failure(
                ComposeFailureStage::Finish,
                "ffmpeg exited with a non-zero status",
                status.code(),
            ))
        }
    }
    pub fn frames_written(&self) -> u64 {
        self.written_frames.load(Ordering::Relaxed)
    }
    pub fn requested_encoder(&self) -> VideoEncoder {
        self.requested_encoder
    }
    pub fn resolved_encoder(&self) -> VideoEncoder {
        self.resolved_encoder
    }
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}
impl Drop for FrameComposer {
    fn drop(&mut self) {
        self.frame_tx.take();
        let _ = self.join_writer_thread();
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.join_stderr_thread();
        if let Some(path) = self.filter_complex_script_path.take() {
            remove_temp_file(&path);
        }
        remove_temp_files(&mut self.temp_input_paths);
    }
}
pub fn is_ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
pub fn build_intro_audio_filter(
    intro_ms: u32,
    _preview_ms: u32,
    main_audio_idx: u32,
    intro_audio_idx: u32,
    preroll_ms: u32,
) -> String {
    let intro_sec = intro_ms as f32 / 1000.0;
    let safe_intro = intro_sec.max(0.1);
    let intro_filter = format!(
        "[{}:a]atrim=duration={:.3},asetpts=PTS-STARTPTS,lowpass=f=400,volume='if(isnan(t),1.5,1.5-1.2*t/{:.3})':eval=frame,afade=t=out:st={:.3}:d=0.3[intro_audio]",
        intro_audio_idx, intro_sec, safe_intro, (intro_sec - 0.3).max(0.0)
    );
    let main_delay = intro_ms + preroll_ms;
    let main_filter = format!(
        "[{}:a]asetpts=PTS-STARTPTS,adelay={}|{}[main_audio]",
        main_audio_idx, main_delay, main_delay
    );
    let mix_filter = "[intro_audio][main_audio]amix=inputs=2:duration=longest:dropout_transition=0:normalize=0[final_audio]";
    format!("{};{};{}", intro_filter, main_filter, mix_filter)
}

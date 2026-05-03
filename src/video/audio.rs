use std::path::Path;
use std::process::{Command, Stdio};
#[derive(Debug)]
pub enum AudioError {
    NotFound(String),
    FfprobeFailed(String),
    ConversionFailed(String),
    InvalidDuration,
}
impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(p) => write!(f, "audio file not found: {}", p),
            Self::FfprobeFailed(e) => write!(f, "ffprobe failed: {}", e),
            Self::ConversionFailed(e) => write!(f, "audio conversion failed: {}", e),
            Self::InvalidDuration => write!(f, "invalid audio duration"),
        }
    }
}
impl std::error::Error for AudioError {}
pub fn detect_audio_duration(path: &Path) -> Result<u32, AudioError> {
    if !path.exists() {
        return Err(AudioError::NotFound(path.display().to_string()));
    }
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| AudioError::FfprobeFailed(e.to_string()))?;
    if !output.status.success() {
        return Err(AudioError::FfprobeFailed("non-zero exit".into()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let secs: f64 = stdout
        .trim()
        .parse()
        .map_err(|_| AudioError::InvalidDuration)?;
    if secs <= 0.0 || !secs.is_finite() {
        return Err(AudioError::InvalidDuration);
    }
    Ok((secs * 1000.0).round() as u32)
}
pub fn detect_audio_sample_rate(path: &Path) -> Result<u32, AudioError> {
    if !path.exists() {
        return Err(AudioError::NotFound(path.display().to_string()));
    }
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=sample_rate",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| AudioError::FfprobeFailed(e.to_string()))?;
    if !output.status.success() {
        return Err(AudioError::FfprobeFailed("non-zero exit".into()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let sample_rate: u32 = stdout
        .trim()
        .parse()
        .map_err(|_| AudioError::InvalidDuration)?;
    if sample_rate == 0 {
        return Err(AudioError::InvalidDuration);
    }
    Ok(sample_rate)
}
pub fn convert_to_wav(input: &Path, output: &Path) -> Result<(), AudioError> {
    if output.exists() {
        std::fs::remove_file(output).ok();
    }
    let status = Command::new("ffmpeg")
        .args(["-y", "-i"])
        .arg(input)
        .args(["-c:a", "pcm_s16le"])
        .arg(output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| AudioError::ConversionFailed(e.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(AudioError::ConversionFailed("ffmpeg failed".into()))
    }
}
pub fn normalize_audio(input: &Path, output: &Path) -> Result<(), AudioError> {
    let status = Command::new("ffmpeg")
        .args(["-y", "-i"])
        .arg(input)
        .args(["-filter:a", "loudnorm", "-c:a", "aac", "-b:a", "192k"])
        .arg(output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| AudioError::ConversionFailed(e.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(AudioError::ConversionFailed("normalize failed".into()))
    }
}
pub fn calc_intro_audio_delay(intro_duration_ms: u32, timeline_start_ms: i32) -> u32 {
    let base = intro_duration_ms;
    if timeline_start_ms < 0 {
        // Negative timeline starts need extra silence so beatmap time 0 still lands after the intro.
        base + timeline_start_ms.unsigned_abs()
    } else {
        base
    }
}
pub fn is_audio_readable(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    detect_audio_duration(path).is_ok()
}
pub fn process_audio(path: &Path) -> Result<std::path::PathBuf, AudioError> {
    if !path.exists() {
        return Err(AudioError::NotFound(path.display().to_string()));
    }
    if is_audio_readable(path) {
        return Ok(path.to_path_buf());
    }
    // Some source formats fail later pipeline steps; convert once to WAV and reuse that path.
    let wav_path = path.with_extension("wav");
    convert_to_wav(path, &wav_path)?;
    Ok(wav_path)
}

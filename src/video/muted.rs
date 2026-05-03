use super::playback::PlaybackClock;
use crate::converter::ReplayMutedConfig;
use crate::modes::mania::judgment::{ScoreJudgmentEvent, ScoreJudgmentPart};
use crate::renderer::replay_renderer::ComboEvent;
use crate::types::{Beatmap, HitObject, TimingPoint};
use crate::utils::mods::PlaybackRateProfile;
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
const OUTPUT_SAMPLE_RATE: u32 = 44_100;
const OUTPUT_CHANNELS: u16 = 2;
const OUTPUT_BITS_PER_SAMPLE: u16 = 16;
const MUTED_TRANSITION_MS: i32 = 500;
const METRONOME_BYTES: &[u8] = include_bytes!("../../assets/muted/metronome.wav");
const COMBOBREAK_SAMPLE_NAME: &str = "combobreak";
const FAIL_SAMPLE_NAME: &str = "failsound";
#[derive(Debug, Clone)]
pub struct AudioSearchSource {
    pub root: PathBuf,
    pub files: Vec<String>,
}
impl AudioSearchSource {
    pub fn new(root: PathBuf, files: Vec<String>) -> Self {
        Self { root, files }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GameplayEffectEvent {
    time_ms: i32,
    sample_name: &'static str,
}
#[derive(Debug)]
pub enum MutedAudioError {
    Io(std::io::Error),
    Wav(hound::Error),
    InvalidSample(String),
    Ffmpeg(String),
}
impl std::fmt::Display for MutedAudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "muted audio io error: {err}"),
            Self::Wav(err) => write!(f, "muted audio wav error: {err}"),
            Self::InvalidSample(err) => write!(f, "muted audio sample error: {err}"),
            Self::Ffmpeg(err) => write!(f, "muted audio ffmpeg error: {err}"),
        }
    }
}
impl std::error::Error for MutedAudioError {}
impl From<std::io::Error> for MutedAudioError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<hound::Error> for MutedAudioError {
    fn from(value: hound::Error) -> Self {
        Self::Wav(value)
    }
}
#[derive(Debug, Clone, Copy)]
struct MutedTransition {
    start_ms: i32,
    end_ms: i32,
    start_dim: f32,
    end_dim: f32,
}
impl MutedTransition {
    fn value_at(self, beatmap_time_ms: f64) -> f32 {
        if beatmap_time_ms <= self.start_ms as f64 {
            return self.start_dim;
        }
        if beatmap_time_ms >= self.end_ms as f64 {
            return self.end_dim;
        }
        let progress = ((beatmap_time_ms - self.start_ms as f64)
            / (self.end_ms - self.start_ms).max(1) as f64)
            .clamp(0.0, 1.0) as f32;
        // Muted volume changes ease out so combo-driven muting avoids abrupt audio clicks.
        let eased = 1.0 - (1.0 - progress).powi(5);
        self.start_dim + (self.end_dim - self.start_dim) * eased
    }
}
#[derive(Debug, Clone)]
pub struct MutedAutomation {
    base_dim: f32,
    transitions: Vec<MutedTransition>,
    affects_hit_sounds: bool,
}
#[derive(Debug, Default, Clone, Copy)]
struct AutomationCursor {
    active_index: Option<usize>,
    next_index: usize,
}
impl MutedAutomation {
    pub fn main_volume_at_beatmap_time(&self, beatmap_time_ms: f64) -> f32 {
        let mut cursor = AutomationCursor::default();
        self.main_volume_at_beatmap_time_with_cursor(beatmap_time_ms, &mut cursor)
    }
    pub fn metronome_volume_at_beatmap_time(&self, beatmap_time_ms: f64) -> f32 {
        let mut cursor = AutomationCursor::default();
        self.metronome_volume_at_beatmap_time_with_cursor(beatmap_time_ms, &mut cursor)
    }
    fn main_volume_at_beatmap_time_with_cursor(
        &self,
        beatmap_time_ms: f64,
        cursor: &mut AutomationCursor,
    ) -> f32 {
        1.0 - self.dim_at_beatmap_time_with_cursor(beatmap_time_ms, cursor)
    }
    fn metronome_volume_at_beatmap_time_with_cursor(
        &self,
        beatmap_time_ms: f64,
        cursor: &mut AutomationCursor,
    ) -> f32 {
        self.dim_at_beatmap_time_with_cursor(beatmap_time_ms, cursor)
    }
    fn hitsound_volume_at_beatmap_time(&self, beatmap_time_ms: f64) -> f32 {
        if self.affects_hit_sounds {
            self.main_volume_at_beatmap_time(beatmap_time_ms)
        } else {
            1.0
        }
    }
    fn dim_at_beatmap_time_with_cursor(
        &self,
        beatmap_time_ms: f64,
        cursor: &mut AutomationCursor,
    ) -> f32 {
        // Callers sample in increasing time order, so the cursor avoids rescanning old transitions.
        while let Some(next_transition) = self.transitions.get(cursor.next_index) {
            if beatmap_time_ms >= next_transition.start_ms as f64 {
                cursor.active_index = Some(cursor.next_index);
                cursor.next_index += 1;
            } else {
                break;
            }
        }
        cursor
            .active_index
            .and_then(|index| self.transitions.get(index).copied())
            .map(|transition| transition.value_at(beatmap_time_ms))
            .unwrap_or(self.base_dim)
            .clamp(0.0, 1.0)
    }
}
#[derive(Clone)]
struct SampleBuffer {
    frames: Vec<[f32; 2]>,
}
impl SampleBuffer {
    fn from_wav_bytes(name: &str, bytes: &[u8]) -> Result<Self, MutedAudioError> {
        let reader = WavReader::new(Cursor::new(bytes))?;
        Self::from_reader(name, reader)
    }
    fn from_audio_path(path: &Path) -> Result<Self, MutedAudioError> {
        let spec = WavReader::open(path).ok().map(|reader| reader.spec());
        if let Some(spec) = spec {
            if spec.sample_rate == OUTPUT_SAMPLE_RATE
                && spec.channels == OUTPUT_CHANNELS
                && spec.bits_per_sample == OUTPUT_BITS_PER_SAMPLE
                && spec.sample_format == SampleFormat::Int
            {
                return Self::from_reader(
                    &path.display().to_string(),
                    WavReader::open(path).map_err(MutedAudioError::Wav)?,
                );
            }
        }
        let temp_path = make_temp_wav_path("muted_sample");
        convert_audio_to_output_wav(path, &temp_path)?;
        let sample = Self::from_reader(
            &path.display().to_string(),
            WavReader::open(&temp_path).map_err(MutedAudioError::Wav)?,
        );
        let _ = std::fs::remove_file(&temp_path);
        sample
    }
    fn from_reader<R: std::io::Read>(
        name: &str,
        reader: WavReader<R>,
    ) -> Result<Self, MutedAudioError> {
        let spec = reader.spec();
        if spec.sample_rate != OUTPUT_SAMPLE_RATE
            || spec.channels != OUTPUT_CHANNELS
            || spec.bits_per_sample != OUTPUT_BITS_PER_SAMPLE
            || spec.sample_format != SampleFormat::Int
        {
            return Err(MutedAudioError::InvalidSample(format!(
                "{name} must be {OUTPUT_SAMPLE_RATE} Hz stereo 16-bit PCM"
            )));
        }
        let samples = reader
            .into_samples::<i16>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(MutedAudioError::Wav)?;
        let frames = samples
            .chunks_exact(2)
            .map(|chunk| {
                [
                    chunk[0] as f32 / i16::MAX as f32,
                    chunk[1] as f32 / i16::MAX as f32,
                ]
            })
            .collect::<Vec<_>>();
        Ok(Self { frames })
    }
    fn resampled_with_frequency(&self, frequency: f32) -> Self {
        let frequency = frequency.max(0.01);
        if (frequency - 1.0).abs() < 1e-6 {
            return self.clone();
        }
        let output_len = ((self.frames.len() as f32) / frequency).ceil().max(1.0) as usize;
        let mut frames = Vec::with_capacity(output_len);
        for output_idx in 0..output_len {
            let source_pos = output_idx as f32 * frequency;
            let source_floor = source_pos.floor() as usize;
            let source_ceil = source_floor
                .saturating_add(1)
                .min(self.frames.len().saturating_sub(1));
            let blend = (source_pos - source_floor as f32).clamp(0.0, 1.0);
            let left = self.frames[source_floor][0]
                + (self.frames[source_ceil][0] - self.frames[source_floor][0]) * blend;
            let right = self.frames[source_floor][1]
                + (self.frames[source_ceil][1] - self.frames[source_floor][1]) * blend;
            frames.push([left, right]);
        }
        Self { frames }
    }
}
pub(crate) fn build_muted_automation(
    combo_events: &[ComboEvent],
    config: ReplayMutedConfig,
) -> MutedAutomation {
    let base_dim = dim_for_combo(0, config);
    let mut transitions = Vec::with_capacity(combo_events.len());
    for event in combo_events {
        let start_ms = event.time;
        let start_dim = transitions
            .last()
            .copied()
            .map(|transition: MutedTransition| transition.value_at(start_ms as f64))
            .unwrap_or(base_dim);
        let end_dim = dim_for_combo(event.combo_after, config);
        transitions.push(MutedTransition {
            start_ms,
            end_ms: start_ms.saturating_add(MUTED_TRANSITION_MS),
            start_dim,
            end_dim,
        });
    }
    MutedAutomation {
        base_dim,
        transitions,
        affects_hit_sounds: config.affects_hit_sounds,
    }
}
fn collect_gameplay_effect_events(
    combo_events: &[ComboEvent],
    fail_time_ms: Option<i32>,
) -> Vec<GameplayEffectEvent> {
    let mut events = combo_events
        .iter()
        .filter_map(|event| {
            event
                .combo_break_start
                .filter(|combo_before_break| *combo_before_break >= 20)
                // osu! only plays combobreak feedback after a meaningful combo has built up.
                .map(|_| GameplayEffectEvent {
                    time_ms: event.time,
                    sample_name: COMBOBREAK_SAMPLE_NAME,
                })
        })
        .collect::<Vec<_>>();
    if let Some(fail_time_ms) = fail_time_ms {
        events.push(GameplayEffectEvent {
            time_ms: fail_time_ms,
            sample_name: FAIL_SAMPLE_NAME,
        });
    }
    events.sort_by_key(|event| event.time_ms);
    events.dedup_by(|rhs, lhs| rhs.time_ms == lhs.time_ms && rhs.sample_name == lhs.sample_name);
    events
}
pub(crate) fn generate_main_audio_gain_track(
    playback_clock: &PlaybackClock,
    intro_duration_ms: u32,
    timeline_end_ms: i32,
    automation: &MutedAutomation,
    output_path: &Path,
) -> Result<(), MutedAudioError> {
    let total_frames = total_output_frames(playback_clock, intro_duration_ms, timeline_end_ms, 0);
    let mut frames = vec![[0.0f32, 0.0f32]; total_frames];
    let mut cursor = AutomationCursor::default();
    for (frame_idx, frame) in frames.iter_mut().enumerate() {
        let output_time_ms = output_time_ms_for_frame(frame_idx);
        let gain = if output_time_ms < intro_duration_ms as f64 {
            1.0
        } else {
            let beatmap_time_ms = playback_clock
                .beatmap_time_for_output_elapsed_ms(output_time_ms - intro_duration_ms as f64);
            automation.main_volume_at_beatmap_time_with_cursor(beatmap_time_ms, &mut cursor)
        };
        *frame = [gain, gain];
    }
    write_wav(output_path, &frames)
}
pub(crate) fn generate_metronome_overlay(
    beatmap: &Beatmap,
    playback_clock: &PlaybackClock,
    audio_playback_profile: Option<&PlaybackRateProfile>,
    intro_duration_ms: u32,
    timeline_end_ms: i32,
    automation: &MutedAutomation,
    output_path: &Path,
) -> Result<(), MutedAudioError> {
    let first_hit_time = beatmap
        .hit_objects
        .iter()
        .map(|hit_object| hit_object.time)
        .min()
        .unwrap_or(0);
    let mut red_points = beatmap
        .timing_points
        .iter()
        .copied()
        .filter(|timing_point| timing_point.uninherited && timing_point.beat_length > 0.0)
        .collect::<Vec<_>>();
    red_points.sort_by(|lhs, rhs| lhs.time.total_cmp(&rhs.time));
    if red_points.is_empty() {
        red_points.push(TimingPoint::default());
    }
    let base_sample = SampleBuffer::from_wav_bytes("metronome", METRONOME_BYTES)?;
    let mut sample_cache: HashMap<u32, SampleBuffer> = HashMap::new();
    sample_cache.insert(quantized_frequency_key(1.0), base_sample.clone());
    sample_cache.insert(
        quantized_frequency_key(0.5),
        base_sample.resampled_with_frequency(0.5),
    );
    let mut longest_sample_len = sample_cache
        .values()
        .map(|sample| sample.frames.len())
        .max()
        .unwrap_or(0);
    let total_frames = total_output_frames(
        playback_clock,
        intro_duration_ms,
        timeline_end_ms,
        longest_sample_len,
    );
    let mut mix = vec![[0.0f32, 0.0f32]; total_frames];
    for (idx, timing_point) in red_points.iter().enumerate() {
        let segment_end = red_points
            .get(idx + 1)
            .map(|next| next.time.min(timeline_end_ms as f64))
            .unwrap_or(timeline_end_ms as f64);
        let beat_length = timing_point.beat_length.max(1.0);
        let meter = timing_point.meter.max(1) as i32;
        // Start one measure before the first hit so players hear a count-in when possible.
        let playback_start = (first_hit_time as f64 - beat_length * meter as f64).max(0.0);
        let first_index = ((timing_point.time.max(playback_start) - timing_point.time)
            / beat_length)
            .ceil()
            .max(0.0) as i32;
        let last_index = ((segment_end - timing_point.time) / beat_length)
            .ceil()
            .max(0.0) as i32;
        for beat_index in first_index..last_index {
            let beat_time = timing_point.time + beat_index as f64 * beat_length;
            if beat_time < playback_start || beat_time >= timeline_end_ms as f64 {
                continue;
            }
            let metronome_volume = automation.metronome_volume_at_beatmap_time(beat_time);
            if metronome_volume <= 0.0001 {
                continue;
            }
            let base_frequency = if beat_index % meter == 0 {
                1.0f32
            } else {
                0.5f32
            };
            let frequency =
                base_frequency * gameplay_sample_frequency(audio_playback_profile, beat_time);
            let frequency_key = quantized_frequency_key(frequency);
            sample_cache.entry(frequency_key).or_insert_with(|| {
                // Quantized frequencies keep adaptive-rate metronome caching bounded.
                let resampled = base_sample.resampled_with_frequency(frequency);
                longest_sample_len = longest_sample_len.max(resampled.frames.len());
                resampled
            });
            let sample = sample_cache
                .get(&frequency_key)
                .expect("metronome sample should be cached");
            let output_time_ms = intro_duration_ms as f64
                + playback_clock.output_elapsed_ms_for_beatmap_time(beat_time);
            mix_sample(&mut mix, output_time_ms, sample, metronome_volume);
        }
    }
    write_wav(output_path, &mix)
}
pub(crate) fn generate_hitsound_overlay(
    beatmap: &Beatmap,
    audio_sources: &[AudioSearchSource],
    score_judgments: &[ScoreJudgmentEvent],
    playback_clock: &PlaybackClock,
    audio_playback_profile: Option<&PlaybackRateProfile>,
    intro_duration_ms: u32,
    timeline_end_ms: i32,
    automation: Option<&MutedAutomation>,
    output_path: &Path,
) -> Result<(), MutedAudioError> {
    let mut sample_cache: HashMap<PathBuf, SampleBuffer> = HashMap::new();
    let mut resampled_cache: HashMap<(PathBuf, u32), SampleBuffer> = HashMap::new();
    let mut scheduled_samples: Vec<(f64, PathBuf, f32, u32, f32)> = Vec::new();
    let mut longest_sample_len = 0usize;
    for event in score_judgments {
        if event.kind == crate::types::JudgmentKind::Miss
            || !matches!(
                event.part,
                ScoreJudgmentPart::Tap | ScoreJudgmentPart::LnHead
            )
        {
            continue;
        }
        let Some(hit_object) = beatmap.hit_objects.get(event.note_index) else {
            continue;
        };
        let output_time_ms = intro_duration_ms as f64
            + playback_clock.output_elapsed_ms_for_beatmap_time(event.event_time as f64);
        let sample_frequency =
            gameplay_sample_frequency(audio_playback_profile, event.event_time as f64);
        let sample_frequency_key = quantized_frequency_key(sample_frequency);
        let muted_gain = automation
            .map(|automation| automation.hitsound_volume_at_beatmap_time(event.event_time as f64))
            .unwrap_or(1.0);
        if muted_gain <= 0.0001 {
            continue;
        }
        for (sample_path, sample_gain) in
            resolve_hitsound_samples(beatmap, hit_object, audio_sources)
        {
            let sample = sample_cache.entry(sample_path.clone()).or_insert_with(|| {
                SampleBuffer::from_audio_path(&sample_path)
                    .unwrap_or(SampleBuffer { frames: Vec::new() })
            });
            let sample_len = if (sample_frequency - 1.0).abs() < 1e-6 {
                sample.frames.len()
            } else {
                let key = (sample_path.clone(), sample_frequency_key);
                let resampled = resampled_cache
                    .entry(key)
                    .or_insert_with(|| sample.resampled_with_frequency(sample_frequency));
                resampled.frames.len()
            };
            longest_sample_len = longest_sample_len.max(sample_len);
            if !sample.frames.is_empty() {
                scheduled_samples.push((
                    output_time_ms,
                    sample_path,
                    sample_gain * muted_gain,
                    sample_frequency_key,
                    sample_frequency,
                ));
            }
        }
    }
    let total_frames = total_output_frames(
        playback_clock,
        intro_duration_ms,
        timeline_end_ms,
        longest_sample_len,
    );
    let mut mix = vec![[0.0f32, 0.0f32]; total_frames];
    for (output_time_ms, sample_path, gain, sample_frequency_key, sample_frequency) in
        scheduled_samples
    {
        let sample = if (sample_frequency - 1.0).abs() < 1e-6 {
            sample_cache.get(&sample_path)
        } else {
            let key = (sample_path.clone(), sample_frequency_key);
            if !resampled_cache.contains_key(&key) {
                if let Some(base_sample) = sample_cache.get(&sample_path) {
                    resampled_cache.insert(
                        key.clone(),
                        base_sample.resampled_with_frequency(sample_frequency),
                    );
                }
            }
            resampled_cache.get(&key)
        };
        if let Some(sample) = sample {
            mix_sample(&mut mix, output_time_ms, sample, gain);
        }
    }
    write_wav(output_path, &mix)
}
pub(crate) fn generate_gameplay_effect_overlay(
    audio_sources: &[AudioSearchSource],
    combo_events: &[ComboEvent],
    fail_time_ms: Option<i32>,
    playback_clock: &PlaybackClock,
    audio_playback_profile: Option<&PlaybackRateProfile>,
    intro_duration_ms: u32,
    timeline_end_ms: i32,
    output_path: &Path,
) -> Result<(), MutedAudioError> {
    let effect_events = collect_gameplay_effect_events(combo_events, fail_time_ms);
    let mut sample_cache: HashMap<PathBuf, SampleBuffer> = HashMap::new();
    let mut resampled_cache: HashMap<(PathBuf, u32), SampleBuffer> = HashMap::new();
    let mut scheduled_samples: Vec<(f64, PathBuf, u32, f32)> = Vec::new();
    let mut longest_sample_len = 0usize;
    for event in effect_events {
        let Some(sample_path) = resolve_effect_sample_path(audio_sources, event.sample_name) else {
            continue;
        };
        let output_time_ms = intro_duration_ms as f64
            + playback_clock.output_elapsed_ms_for_beatmap_time(event.time_ms as f64);
        let sample_frequency =
            gameplay_sample_frequency(audio_playback_profile, event.time_ms as f64);
        let sample_frequency_key = quantized_frequency_key(sample_frequency);
        let sample = sample_cache.entry(sample_path.clone()).or_insert_with(|| {
            SampleBuffer::from_audio_path(&sample_path)
                .unwrap_or(SampleBuffer { frames: Vec::new() })
        });
        let sample_len = if (sample_frequency - 1.0).abs() < 1e-6 {
            sample.frames.len()
        } else {
            let key = (sample_path.clone(), sample_frequency_key);
            let resampled = resampled_cache
                .entry(key)
                .or_insert_with(|| sample.resampled_with_frequency(sample_frequency));
            resampled.frames.len()
        };
        longest_sample_len = longest_sample_len.max(sample_len);
        if !sample.frames.is_empty() {
            scheduled_samples.push((
                output_time_ms,
                sample_path,
                sample_frequency_key,
                sample_frequency,
            ));
        }
    }
    let total_frames = total_output_frames(
        playback_clock,
        intro_duration_ms,
        timeline_end_ms,
        longest_sample_len,
    );
    let mut mix = vec![[0.0f32, 0.0f32]; total_frames];
    for (output_time_ms, sample_path, sample_frequency_key, sample_frequency) in scheduled_samples {
        let sample = if (sample_frequency - 1.0).abs() < 1e-6 {
            sample_cache.get(&sample_path)
        } else {
            let key = (sample_path.clone(), sample_frequency_key);
            if !resampled_cache.contains_key(&key) {
                if let Some(base_sample) = sample_cache.get(&sample_path) {
                    resampled_cache.insert(
                        key.clone(),
                        base_sample.resampled_with_frequency(sample_frequency),
                    );
                }
            }
            resampled_cache.get(&key)
        };
        if let Some(sample) = sample {
            mix_sample(&mut mix, output_time_ms, sample, 1.0);
        }
    }
    write_wav(output_path, &mix)
}
fn dim_for_combo(combo: u32, config: ReplayMutedConfig) -> f32 {
    let mut dim = if config.mute_combo_count == 0 {
        1.0
    } else {
        (combo as f32 / config.mute_combo_count as f32).clamp(0.0, 1.0)
    };
    if config.inverse_muting {
        dim = 1.0 - dim;
    }
    dim.clamp(0.0, 1.0)
}
fn total_output_frames(
    playback_clock: &PlaybackClock,
    intro_duration_ms: u32,
    timeline_end_ms: i32,
    extra_frames: usize,
) -> usize {
    let total_output_ms = intro_duration_ms as f64
        + playback_clock
            .output_elapsed_ms_for_beatmap_time(timeline_end_ms as f64)
            .max(0.0);
    ((total_output_ms / 1000.0) * OUTPUT_SAMPLE_RATE as f64).ceil() as usize
        + extra_frames
        + OUTPUT_SAMPLE_RATE as usize / 5
}
fn output_time_ms_for_frame(frame_idx: usize) -> f64 {
    frame_idx as f64 * 1000.0 / OUTPUT_SAMPLE_RATE as f64
}
fn gameplay_sample_frequency(
    audio_playback_profile: Option<&PlaybackRateProfile>,
    beatmap_time_ms: f64,
) -> f32 {
    audio_playback_profile
        .filter(|profile| profile.is_adaptive())
        .map(|profile| profile.rate_at_beatmap_time_ms(beatmap_time_ms).max(0.01) as f32)
        .unwrap_or(1.0)
}
fn quantized_frequency_key(frequency: f32) -> u32 {
    (frequency.max(0.01) * 1000.0).round() as u32
}
fn write_wav(path: &Path, frames: &[[f32; 2]]) -> Result<(), MutedAudioError> {
    let spec = WavSpec {
        channels: OUTPUT_CHANNELS,
        sample_rate: OUTPUT_SAMPLE_RATE,
        bits_per_sample: OUTPUT_BITS_PER_SAMPLE,
        sample_format: SampleFormat::Int,
    };
    let mut writer = WavWriter::create(path, spec)?;
    for frame in frames {
        let left = (frame[0].clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        let right = (frame[1].clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        writer.write_sample(left)?;
        writer.write_sample(right)?;
    }
    writer.finalize()?;
    Ok(())
}
fn mix_sample(mix: &mut [[f32; 2]], output_time_ms: f64, sample: &SampleBuffer, gain: f32) {
    let start_frame = ((output_time_ms / 1000.0) * OUTPUT_SAMPLE_RATE as f64).round();
    if start_frame.is_sign_negative() {
        return;
    }
    let start_frame = start_frame as usize;
    for (idx, frame) in sample.frames.iter().enumerate() {
        let out_idx = start_frame + idx;
        if out_idx >= mix.len() {
            break;
        }
        mix[out_idx][0] += frame[0] * gain;
        mix[out_idx][1] += frame[1] * gain;
    }
}
fn resolve_hitsound_samples(
    beatmap: &Beatmap,
    hit_object: &HitObject,
    audio_sources: &[AudioSearchSource],
) -> Vec<(PathBuf, f32)> {
    let timing_point = timing_point_at(beatmap, hit_object.time);
    let timing_sample_set = timing_point
        .map(|point| point.sample_set)
        .unwrap_or(1)
        .max(1);
    let sample_index = if hit_object.hit_sample.index > 0 {
        hit_object.hit_sample.index
    } else {
        timing_point
            .map(|point| point.sample_index)
            .unwrap_or(1)
            .max(1)
    };
    let volume = if hit_object.hit_sample.volume > 0 {
        hit_object.hit_sample.volume
    } else {
        timing_point.map(|point| point.volume).unwrap_or(100)
    };
    let gain = (volume as f32 / 100.0).clamp(0.0, 1.0);
    if !hit_object.hit_sample.filename.trim().is_empty() {
        return resolve_sample_path(audio_sources, hit_object.hit_sample.filename.trim())
            .map(|path| vec![(path, gain)])
            .unwrap_or_default();
    }
    let normal_set = if hit_object.hit_sample.normal_set > 0 {
        hit_object.hit_sample.normal_set
    } else {
        timing_sample_set
    }
    .max(1);
    let addition_set = if hit_object.hit_sample.addition_set > 0 {
        hit_object.hit_sample.addition_set
    } else {
        normal_set
    }
    .max(1);
    let mut samples = Vec::new();
    if let Some(path) = resolve_named_sample(audio_sources, normal_set, "hitnormal", sample_index) {
        samples.push((path, gain));
    }
    if hit_object.hit_sound & 2 != 0 {
        // HitSound bits follow osu!'s whistle=2, finish=4, clap=8 layout.
        if let Some(path) =
            resolve_named_sample(audio_sources, addition_set, "hitwhistle", sample_index)
        {
            samples.push((path, gain));
        }
    }
    if hit_object.hit_sound & 4 != 0 {
        if let Some(path) =
            resolve_named_sample(audio_sources, addition_set, "hitfinish", sample_index)
        {
            samples.push((path, gain));
        }
    }
    if hit_object.hit_sound & 8 != 0 {
        if let Some(path) =
            resolve_named_sample(audio_sources, addition_set, "hitclap", sample_index)
        {
            samples.push((path, gain));
        }
    }
    samples
}
fn resolve_effect_sample_path(
    audio_sources: &[AudioSearchSource],
    sample_name: &str,
) -> Option<PathBuf> {
    resolve_sample_path(audio_sources, sample_name)
}
fn resolve_named_sample(
    audio_sources: &[AudioSearchSource],
    sample_set: u8,
    hit_kind: &str,
    sample_index: u8,
) -> Option<PathBuf> {
    let prefix = match sample_set {
        2 => "soft",
        3 => "drum",
        _ => "normal",
    };
    let base = format!("{prefix}-{hit_kind}");
    if sample_index > 1 {
        let indexed = format!("{base}{sample_index}");
        if let Some(path) = resolve_sample_path(audio_sources, &indexed) {
            return Some(path);
        }
    }
    resolve_sample_path(audio_sources, &base)
}
fn resolve_sample_path(audio_sources: &[AudioSearchSource], sample_name: &str) -> Option<PathBuf> {
    let candidate_path = Path::new(sample_name);
    if candidate_path.extension().is_some() {
        return audio_sources.iter().find_map(|source| {
            resolve_asset_case_insensitive(&source.root, &source.files, sample_name)
        });
    }
    for ext in ["wav", "ogg", "mp3"] {
        let candidate = format!("{sample_name}.{ext}");
        if let Some(path) = audio_sources.iter().find_map(|source| {
            resolve_asset_case_insensitive(&source.root, &source.files, &candidate)
        }) {
            return Some(path);
        }
    }
    None
}
fn sanitize_relative_audio_path(path: &Path) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            // Skin sample names are untrusted and must stay within the selected audio source root.
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if normalized.as_os_str().is_empty() {
        None
    } else {
        Some(normalized)
    }
}
fn normalized_audio_lookup_key(path: &Path) -> Option<String> {
    let normalized = sanitize_relative_audio_path(path)?;
    let parts = normalized
        .components()
        .map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().to_ascii_lowercase()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}
fn resolve_rooted_audio_path(root: &Path, candidate: &str) -> Option<PathBuf> {
    let relative = sanitize_relative_audio_path(Path::new(candidate))?;
    let resolved = root.join(&relative);
    if !resolved.exists() {
        return None;
    }
    let canonical_root = root.canonicalize().ok()?;
    let canonical_resolved = resolved.canonicalize().ok()?;
    if canonical_resolved.starts_with(&canonical_root) {
        Some(resolved)
    } else {
        None
    }
}
fn resolve_asset_case_insensitive(dir: &Path, files: &[String], filename: &str) -> Option<PathBuf> {
    let target = normalized_audio_lookup_key(Path::new(filename))?;
    for file in files {
        if normalized_audio_lookup_key(Path::new(file)).as_deref() == Some(target.as_str()) {
            if let Some(resolved) = resolve_rooted_audio_path(dir, file) {
                return Some(resolved);
            }
        }
    }
    let target_base = sanitize_relative_audio_path(Path::new(filename))?
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase())?;
    for file in files {
        let Some(base) = sanitize_relative_audio_path(Path::new(file)).and_then(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_ascii_lowercase())
        }) else {
            continue;
        };
        if base == target_base {
            if let Some(resolved) = resolve_rooted_audio_path(dir, file) {
                return Some(resolved);
            }
        }
    }
    None
}
fn timing_point_at(beatmap: &Beatmap, time: i32) -> Option<&TimingPoint> {
    let mut chosen = None;
    for timing_point in &beatmap.timing_points {
        if timing_point.time as i32 <= time {
            chosen = Some(timing_point);
        } else {
            break;
        }
    }
    chosen
}
fn convert_audio_to_output_wav(input: &Path, output: &Path) -> Result<(), MutedAudioError> {
    if output.exists() {
        let _ = std::fs::remove_file(output);
    }
    let status = Command::new("ffmpeg")
        .args(["-y", "-i"])
        .arg(input)
        .args(["-ac", "2", "-ar", "44100", "-c:a", "pcm_s16le"])
        .arg(output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| MutedAudioError::Ffmpeg(err.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(MutedAudioError::Ffmpeg(format!(
            "ffmpeg failed to convert {}",
            input.display()
        )))
    }
}
fn make_temp_wav_path(stem: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("miru_{stem}_{nanos}.wav"))
}

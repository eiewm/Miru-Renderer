use super::playback::PlaybackClock;
use crate::types::{Beatmap, TimingPoint};
use crate::utils::mods::PlaybackRateProfile;
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use std::io::Cursor;
use std::path::Path;
const OUTPUT_SAMPLE_RATE: u32 = 44_100;
const OUTPUT_CHANNELS: u16 = 2;
const OUTPUT_BITS_PER_SAMPLE: u16 = 16;
const BARS_PER_SEGMENT: usize = 4;
// TimingPoint effects bit 3 suppresses the first bar line in osu! beatmaps.
const EFFECT_OMIT_FIRST_BAR_LINE: u8 = 1 << 3;
const SAMPLE_GAIN_HAT: f32 = 0.45;
const SAMPLE_GAIN_CLAP: f32 = 0.65;
const SAMPLE_GAIN_KICK: f32 = 0.75;
const SAMPLE_GAIN_FINISH: f32 = 0.85;
const HAT_BYTES: &[u8] = include_bytes!("../../assets/nightcore/hat.wav");
const CLAP_BYTES: &[u8] = include_bytes!("../../assets/nightcore/clap.wav");
const KICK_BYTES: &[u8] = include_bytes!("../../assets/nightcore/kick.wav");
const FINISH_BYTES: &[u8] = include_bytes!("../../assets/nightcore/finish.wav");
#[derive(Debug)]
pub enum NightcoreOverlayError {
    Io(std::io::Error),
    Wav(hound::Error),
    InvalidSample(String),
}
impl std::fmt::Display for NightcoreOverlayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "nightcore overlay io error: {err}"),
            Self::Wav(err) => write!(f, "nightcore overlay wav error: {err}"),
            Self::InvalidSample(err) => write!(f, "nightcore overlay sample error: {err}"),
        }
    }
}
impl std::error::Error for NightcoreOverlayError {}
impl From<std::io::Error> for NightcoreOverlayError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<hound::Error> for NightcoreOverlayError {
    fn from(value: hound::Error) -> Self {
        Self::Wav(value)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NightcoreSampleKind {
    Hat,
    Clap,
    Kick,
    Finish,
}
#[derive(Debug, Clone, Copy, PartialEq)]
struct NightcoreEvent {
    output_time_ms: f64,
    kind: NightcoreSampleKind,
}
struct SampleBuffer {
    frames: Vec<[f32; 2]>,
}
impl SampleBuffer {
    fn from_wav_bytes(name: &str, bytes: &[u8]) -> Result<Self, NightcoreOverlayError> {
        let reader = WavReader::new(Cursor::new(bytes))?;
        let spec = reader.spec();
        if spec.channels != OUTPUT_CHANNELS
            || spec.sample_rate != OUTPUT_SAMPLE_RATE
            || spec.bits_per_sample != OUTPUT_BITS_PER_SAMPLE
            || spec.sample_format != SampleFormat::Int
        {
            return Err(NightcoreOverlayError::InvalidSample(format!(
                "{name} must be {OUTPUT_SAMPLE_RATE} Hz stereo 16-bit PCM"
            )));
        }
        let mut samples = Vec::new();
        for sample in reader.into_samples::<i16>() {
            samples.push(sample? as f32 / i16::MAX as f32);
        }
        if samples.len() % OUTPUT_CHANNELS as usize != 0 {
            return Err(NightcoreOverlayError::InvalidSample(format!(
                "{name} has incomplete stereo frame data"
            )));
        }
        let frames = samples
            .chunks_exact(2)
            .map(|chunk| [chunk[0], chunk[1]])
            .collect();
        Ok(Self { frames })
    }
    fn len(&self) -> usize {
        self.frames.len()
    }
}
struct NightcoreSamples {
    hat: SampleBuffer,
    clap: SampleBuffer,
    kick: SampleBuffer,
    finish: SampleBuffer,
}
impl NightcoreSamples {
    fn load() -> Result<Self, NightcoreOverlayError> {
        Ok(Self {
            hat: SampleBuffer::from_wav_bytes("hat", HAT_BYTES)?,
            clap: SampleBuffer::from_wav_bytes("clap", CLAP_BYTES)?,
            kick: SampleBuffer::from_wav_bytes("kick", KICK_BYTES)?,
            finish: SampleBuffer::from_wav_bytes("finish", FINISH_BYTES)?,
        })
    }
    fn sample(&self, kind: NightcoreSampleKind) -> (&SampleBuffer, f32) {
        match kind {
            NightcoreSampleKind::Hat => (&self.hat, SAMPLE_GAIN_HAT),
            NightcoreSampleKind::Clap => (&self.clap, SAMPLE_GAIN_CLAP),
            NightcoreSampleKind::Kick => (&self.kick, SAMPLE_GAIN_KICK),
            NightcoreSampleKind::Finish => (&self.finish, SAMPLE_GAIN_FINISH),
        }
    }
    fn longest_len(&self) -> usize {
        [
            self.hat.len(),
            self.clap.len(),
            self.kick.len(),
            self.finish.len(),
        ]
        .into_iter()
        .max()
        .unwrap_or(0)
    }
}
pub fn generate_nightcore_overlay(
    beatmap: &Beatmap,
    timeline_start_ms: i32,
    timeline_end_ms: i32,
    intro_duration_ms: u32,
    clock_rate: f64,
    output_path: &Path,
) -> Result<(), NightcoreOverlayError> {
    let samples = NightcoreSamples::load()?;
    let events = generate_nightcore_events(
        &beatmap.timing_points,
        beatmap.difficulty.slider_tick_rate,
        PlaybackClock::new(timeline_start_ms, PlaybackRateProfile::constant(clock_rate)),
        timeline_start_ms,
        timeline_end_ms,
        intro_duration_ms,
    );
    let total_output_ms = intro_duration_ms as f64
        + PlaybackClock::new(timeline_start_ms, PlaybackRateProfile::constant(clock_rate))
            .output_elapsed_ms_for_beatmap_time(timeline_end_ms as f64);
    let total_frames = ((total_output_ms / 1000.0) * OUTPUT_SAMPLE_RATE as f64).ceil() as usize
        + samples.longest_len()
        + OUTPUT_SAMPLE_RATE as usize / 5;
    let mut mix = vec![[0.0f32, 0.0f32]; total_frames];
    for event in events {
        let start_frame = ((event.output_time_ms / 1000.0) * OUTPUT_SAMPLE_RATE as f64).round();
        if start_frame.is_sign_negative() {
            continue;
        }
        let start_frame = start_frame as usize;
        let (sample, gain) = samples.sample(event.kind);
        for (idx, frame) in sample.frames.iter().enumerate() {
            let out_idx = start_frame + idx;
            if out_idx >= mix.len() {
                break;
            }
            mix[out_idx][0] += frame[0] * gain;
            mix[out_idx][1] += frame[1] * gain;
        }
    }
    let spec = WavSpec {
        channels: OUTPUT_CHANNELS,
        sample_rate: OUTPUT_SAMPLE_RATE,
        bits_per_sample: OUTPUT_BITS_PER_SAMPLE,
        sample_format: SampleFormat::Int,
    };
    let mut writer = WavWriter::create(output_path, spec)?;
    for frame in mix {
        let left = (frame[0].clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        let right = (frame[1].clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        writer.write_sample(left)?;
        writer.write_sample(right)?;
    }
    writer.finalize()?;
    Ok(())
}
fn generate_nightcore_events(
    timing_points: &[TimingPoint],
    slider_tick_rate: f32,
    clock: PlaybackClock,
    timeline_start_ms: i32,
    timeline_end_ms: i32,
    intro_duration_ms: u32,
) -> Vec<NightcoreEvent> {
    let mut red_points: Vec<TimingPoint> = timing_points
        .iter()
        .copied()
        .filter(|tp| tp.uninherited && tp.beat_length > 0.0)
        .collect();
    red_points.sort_by(|a, b| a.time.total_cmp(&b.time));
    if red_points.is_empty() {
        red_points.push(TimingPoint::default());
    }
    // Stable Nightcore percussion only plays hats on maps with even slider tick rates.
    let play_hats = (slider_tick_rate % 2.0).abs() < 1e-4;
    let mut events = Vec::new();
    for (idx, tp) in red_points.iter().enumerate() {
        let segment_start = tp.time.max(timeline_start_ms as f64);
        let segment_end = red_points
            .get(idx + 1)
            .map(|next| next.time.min(timeline_end_ms as f64))
            .unwrap_or(timeline_end_ms as f64);
        if segment_end <= segment_start {
            continue;
        }
        // Nightcore overlays schedule kick/clap/hat on half-beat positions.
        let half_beat = tp.beat_length / 2.0;
        if !half_beat.is_finite() || half_beat <= 0.0 {
            continue;
        }
        let meter = tp.meter.max(1) as usize;
        let segment_length = meter * 2 * BARS_PER_SEGMENT;
        let first_index = ((segment_start - tp.time) / half_beat).ceil().max(0.0) as usize;
        let last_index = ((segment_end - tp.time) / half_beat).ceil().max(0.0) as usize;
        for beat_index in first_index..last_index {
            let beat_time = tp.time + beat_index as f64 * half_beat;
            if beat_time < timeline_start_ms as f64 || beat_time >= timeline_end_ms as f64 {
                continue;
            }
            let beat_in_segment = beat_index % segment_length;
            let output_time_ms =
                intro_duration_ms as f64 + clock.output_elapsed_ms_for_beatmap_time(beat_time);
            // Finish marks the first beat of each four-bar phrase unless the map suppresses it.
            if beat_in_segment == 0
                && (beat_index > 0 || (tp.effects & EFFECT_OMIT_FIRST_BAR_LINE) == 0)
            {
                events.push(NightcoreEvent {
                    output_time_ms,
                    kind: NightcoreSampleKind::Finish,
                });
            }
            match meter {
                3 => match beat_in_segment % 6 {
                    0 => events.push(NightcoreEvent {
                        output_time_ms,
                        kind: NightcoreSampleKind::Kick,
                    }),
                    3 => events.push(NightcoreEvent {
                        output_time_ms,
                        kind: NightcoreSampleKind::Clap,
                    }),
                    _ if play_hats => events.push(NightcoreEvent {
                        output_time_ms,
                        kind: NightcoreSampleKind::Hat,
                    }),
                    _ => {}
                },
                4 => match beat_in_segment % 4 {
                    0 => events.push(NightcoreEvent {
                        output_time_ms,
                        kind: NightcoreSampleKind::Kick,
                    }),
                    2 => events.push(NightcoreEvent {
                        output_time_ms,
                        kind: NightcoreSampleKind::Clap,
                    }),
                    _ if play_hats => events.push(NightcoreEvent {
                        output_time_ms,
                        kind: NightcoreSampleKind::Hat,
                    }),
                    _ => {}
                },
                _ => {}
            }
        }
    }
    events.sort_by(|a, b| a.output_time_ms.total_cmp(&b.output_time_ms));
    events
}

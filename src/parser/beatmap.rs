use super::storyboard::{
    decode_storyboard_variables, parse_storyboard_content, parse_storyboard_lines,
    parse_storyboard_variable_line, StoryboardVariables,
};
use crate::types::{
    BackgroundEvent, Beatmap, BeatmapEvents, BeatmapMetadata, BreakPeriod, Difficulty, HitObject,
    HitSample, Storyboard, TimingPoint,
};
use anyhow::Result;
use std::fs;
use std::path::Path;
#[derive(PartialEq)]
enum Section {
    None,
    General,
    Metadata,
    Difficulty,
    Events,
    Variables,
    TimingPoints,
    HitObjects,
}
struct ParsedBeatmap {
    beatmap: Beatmap,
    storyboard_disabled: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct ParseBeatmapOptions {
    pub storyboard_enabled: bool,
}

impl Default for ParseBeatmapOptions {
    fn default() -> Self {
        Self {
            storyboard_enabled: true,
        }
    }
}

pub fn parse_osu_file(path: &Path) -> Result<Beatmap> {
    parse_osu_file_with_options(path, ParseBeatmapOptions::default())
}

pub fn parse_osu_file_with_options(path: &Path, options: ParseBeatmapOptions) -> Result<Beatmap> {
    let content = read_text_file(path)?;
    let ParsedBeatmap {
        mut beatmap,
        mut storyboard_disabled,
    } = parse_osu_content_inner(&content, options.storyboard_enabled)?;
    if options.storyboard_enabled && !storyboard_disabled {
        if let Some(dir) = path.parent() {
            if let Ok(entries) = fs::read_dir(dir) {
                // .osb sidecars extend the [Events] storyboard for every difficulty in the folder.
                let mut osb_files: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| {
                        p.extension()
                            .and_then(|s| s.to_str())
                            .map(|s| s.eq_ignore_ascii_case("osb"))
                            .unwrap_or(false)
                    })
                    .collect();
                osb_files.sort();
                for osb in osb_files {
                    if storyboard_disabled {
                        break;
                    }
                    let sb_content = match read_text_file(&osb) {
                        Ok(content) => content,
                        Err(err) => {
                            eprintln!("warn: storyboard disabled: {err}");
                            storyboard_disabled = true;
                            break;
                        }
                    };
                    match parse_storyboard_content(&sb_content) {
                        Ok(sb) => beatmap.storyboard.objects.extend(sb.objects),
                        Err(err) => {
                            eprintln!("warn: storyboard disabled: {err}");
                            storyboard_disabled = true;
                            break;
                        }
                    }
                }
            }
        }
    }
    if storyboard_disabled {
        beatmap.storyboard = Storyboard::default();
    }
    Ok(beatmap)
}
pub fn parse_osu_content(content: &str) -> Result<Beatmap> {
    Ok(parse_osu_content_inner(content, true)?.beatmap)
}
fn parse_osu_content_inner(content: &str, storyboard_enabled: bool) -> Result<ParsedBeatmap> {
    let mut metadata = BeatmapMetadata::default();
    let mut difficulty = Difficulty::default();
    let mut timing_points = Vec::new();
    let mut hit_objects = Vec::new();
    let mut events = BeatmapEvents::default();
    let mut event_lines: Vec<String> = Vec::new();
    let mut storyboard_variables = StoryboardVariables::new();
    let mut section = Section::None;
    let mut key_count: u8 = 4;
    let mut storyboard_disabled = false;
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = match line {
                "[General]" => Section::General,
                "[Metadata]" => Section::Metadata,
                "[Difficulty]" => Section::Difficulty,
                "[Events]" => Section::Events,
                "[Variables]" => Section::Variables,
                "[TimingPoints]" => Section::TimingPoints,
                "[HitObjects]" => Section::HitObjects,
                _ => Section::None,
            };
            continue;
        }
        match section {
            Section::General => parse_general_line(line, &mut metadata),
            Section::Metadata => parse_metadata_line(line, &mut metadata),
            Section::Difficulty => {
                parse_difficulty_line(line, &mut difficulty);
                if line.starts_with("CircleSize:") {
                    // In osu!mania, CircleSize is the key count used to map hit-object x positions.
                    key_count = difficulty.key_count();
                }
            }
            Section::Variables => {
                parse_storyboard_variable_line(raw_line, &mut storyboard_variables);
            }
            Section::Events => {
                let storyboard_line = decode_storyboard_variables(raw_line, &storyboard_variables);
                // [Events] contains both background metadata and storyboard command lines.
                parse_events_line(storyboard_line.trim(), &mut events);
                if storyboard_enabled && !storyboard_disabled {
                    event_lines.push(storyboard_line);
                }
            }
            Section::TimingPoints => {
                if let Some(tp) = parse_timing_point(line) {
                    timing_points.push(tp);
                }
            }
            Section::HitObjects => {
                if let Some(ho) = parse_hit_object(line, key_count) {
                    hit_objects.push(ho);
                }
            }
            Section::None => {}
        }
    }
    let storyboard = if !storyboard_enabled || storyboard_disabled {
        Storyboard::default()
    } else {
        match parse_storyboard_lines(&event_lines) {
            Ok(storyboard) => storyboard,
            Err(err) => {
                eprintln!("warn: storyboard disabled: {err}");
                storyboard_disabled = true;
                Storyboard::default()
            }
        }
    };
    Ok(ParsedBeatmap {
        beatmap: Beatmap {
            metadata,
            difficulty,
            timing_points,
            hit_objects,
            events,
            storyboard,
        },
        storyboard_disabled,
    })
}
fn read_text_file(path: &Path) -> Result<String> {
    Ok(fs::read_to_string(path)?)
}
fn parse_general_line(line: &str, meta: &mut BeatmapMetadata) {
    let Some((key, val)) = line.split_once(':') else {
        return;
    };
    let val = val.trim();
    match key.trim() {
        "AudioFilename" => meta.audio_filename = val.to_string(),
        "PreviewTime" => meta.preview_time = val.parse().unwrap_or(0),
        "Mode" => meta.mode = val.parse().unwrap_or(0),
        _ => {}
    }
}
fn parse_metadata_line(line: &str, meta: &mut BeatmapMetadata) {
    let Some((key, val)) = line.split_once(':') else {
        return;
    };
    let val = val.trim();
    match key.trim() {
        "Title" => meta.title = val.to_string(),
        "TitleUnicode" => meta.title_unicode = val.to_string(),
        "Artist" => meta.artist = val.to_string(),
        "ArtistUnicode" => meta.artist_unicode = val.to_string(),
        "Creator" => meta.creator = val.to_string(),
        "Version" => meta.version = val.to_string(),
        "Source" => meta.source = val.to_string(),
        "Tags" => meta.tags = val.to_string(),
        "BeatmapID" => meta.beatmap_id = val.parse().ok().filter(|id| *id > 0),
        "BeatmapSetID" => meta.beatmapset_id = val.parse().ok().filter(|id| *id > 0),
        _ => {}
    }
}
fn parse_difficulty_line(line: &str, diff: &mut Difficulty) {
    let Some((key, val)) = line.split_once(':') else {
        return;
    };
    let val = val.trim();
    match key.trim() {
        "HPDrainRate" => diff.hp = val.parse().unwrap_or(5.0),
        "CircleSize" => diff.cs = val.parse().unwrap_or(4.0),
        "OverallDifficulty" => diff.od = val.parse().unwrap_or(5.0),
        "ApproachRate" => diff.ar = val.parse().unwrap_or(5.0),
        "SliderMultiplier" => diff.slider_multiplier = val.parse().unwrap_or(1.4),
        "SliderTickRate" => diff.slider_tick_rate = val.parse().unwrap_or(1.0),
        _ => {}
    }
}
fn parse_events_line(line: &str, events: &mut BeatmapEvents) {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() < 2 {
        return;
    }
    let event_type = parts[0].trim().to_lowercase();
    if event_type == "0" || event_type == "background" {
        if let Some(filename) = extract_quoted_string(line) {
            let x = parts
                .get(3)
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
            let y = parts
                .get(4)
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
            events.backgrounds.push(BackgroundEvent::Image {
                filename,
                x_offset: x,
                y_offset: y,
            });
        }
    } else if event_type == "1" || event_type == "video" {
        if let Some(filename) = extract_quoted_string(line) {
            let start = parts
                .get(1)
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
            let x = parts
                .get(3)
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
            let y = parts
                .get(4)
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
            events.backgrounds.push(BackgroundEvent::Video {
                filename,
                start_time: start,
                x_offset: x,
                y_offset: y,
            });
        }
    } else if (event_type == "2" || event_type == "break") && parts.len() >= 3 {
        if let (Ok(start), Ok(end)) = (parts[1].trim().parse(), parts[2].trim().parse()) {
            events.breaks.push(BreakPeriod { start, end });
        }
    }
}
fn extract_quoted_string(line: &str) -> Option<String> {
    let start = line.find('"')? + 1;
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}
fn parse_timing_point(line: &str) -> Option<TimingPoint> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() < 8 {
        return None;
    }
    // Timing point fields are positional in the .osu format.
    Some(TimingPoint {
        time: parts[0].trim().parse().ok()?,
        beat_length: parts[1].trim().parse().ok()?,
        meter: parts[2].trim().parse().unwrap_or(4),
        sample_set: parts[3].trim().parse().unwrap_or(0),
        sample_index: parts[4].trim().parse().unwrap_or(0),
        volume: parts[5].trim().parse().unwrap_or(100),
        uninherited: parts[6].trim() == "1",
        effects: parts[7].trim().parse().unwrap_or(0),
    })
}
fn parse_hit_object(line: &str, key_count: u8) -> Option<HitObject> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() < 5 {
        return None;
    }
    let x: i32 = parts[0].trim().parse().ok()?;
    let y: i32 = parts[1].trim().parse().ok()?;
    let time: i32 = parts[2].trim().parse().ok()?;
    let obj_type: u8 = parts[3].trim().parse().ok()?;
    let hit_sound: u8 = parts[4].trim().parse().ok()?;
    let column = HitObject::column_from_x(x, key_count);
    // Mania long notes store end time before the hit-sample fields in field 5.
    let (end_time, sample_str) = if (obj_type & 128) != 0 && parts.len() > 5 {
        let mut split = parts[5].splitn(2, ':');
        let end = split.next().and_then(|s| s.trim().parse::<i32>().ok());
        let sample = split.next().unwrap_or("");
        (end, sample)
    } else {
        (None, parts.get(5).copied().unwrap_or(""))
    };
    let hit_sample = parse_hit_sample(sample_str);
    Some(HitObject {
        x,
        y,
        time,
        obj_type,
        hit_sound,
        end_time,
        column,
        hit_sample,
    })
}
fn parse_hit_sample(sample_str: &str) -> HitSample {
    let mut sample = HitSample::default();
    if sample_str.is_empty() {
        return sample;
    }
    // Missing hit-sample components inherit from timing points later in playback logic.
    let parts: Vec<&str> = sample_str.split(':').collect();
    sample.normal_set = parts
        .first()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    sample.addition_set = parts
        .get(1)
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    sample.index = parts
        .get(2)
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    sample.volume = parts
        .get(3)
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    sample.filename = parts
        .get(4)
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    sample
}

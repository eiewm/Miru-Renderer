use crate::types::{
    ApiModEntry, ManiaReplayData, ReplayData, ReplayFrame, ReplayModInfo, ReplayOrigin,
    ReplayScoreInfo,
};
use anyhow::{anyhow, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use serde::Deserialize;
use std::fs;
use std::io::{self, Cursor, Read, Write};
use std::path::Path;
// Lazer exports use high synthetic replay versions while stable replays stay below this range.
const FIRST_LAZER_VERSION: u32 = 30_000_000;
const FIRST_LAZER_SCORE_INFO_VERSION: u32 = 30_000_001;
// Replays are untrusted binary input; every variable-sized field has a hard cap.
const MAX_OSU_STRING_BYTES: usize = 1024 * 1024;
const MAX_LEGACY_BYTE_ARRAY_BYTES: usize = 8 * 1024 * 1024;
const MAX_REPLAY_FRAME_DATA_BYTES: usize = 4 * 1024 * 1024;
const MAX_LAZER_SCORE_INFO_BYTES: usize = 2 * 1024 * 1024;
const MAX_REPLAY_FRAMES: usize = 250_000;
const MAX_REPLAY_ABSOLUTE_TIME_MS: i32 = 6 * 60 * 60 * 1000;
pub fn parse_osr_file(path: &Path) -> Result<ManiaReplayData> {
    let data = fs::read(path)?;
    parse_osr_data(&data)
}
pub fn parse_osr_data(data: &[u8]) -> Result<ManiaReplayData> {
    let mut cursor = Cursor::new(data);
    let game_mode = cursor.read_u8()?;
    let version = cursor.read_u32::<LittleEndian>()?;
    let beatmap_hash = read_osu_string(&mut cursor)?;
    let player_name = read_osu_string(&mut cursor)?;
    let replay_hash = read_osu_string(&mut cursor)?;
    let count_300 = cursor.read_u16::<LittleEndian>()?;
    let count_100 = cursor.read_u16::<LittleEndian>()?;
    let count_50 = cursor.read_u16::<LittleEndian>()?;
    let count_geki = cursor.read_u16::<LittleEndian>()?;
    let count_katu = cursor.read_u16::<LittleEndian>()?;
    let count_miss = cursor.read_u16::<LittleEndian>()?;
    let total_score = cursor.read_u32::<LittleEndian>()?;
    let max_combo = cursor.read_u16::<LittleEndian>()?;
    let perfect_combo = cursor.read_u8()? == 1;
    let mods = cursor.read_u32::<LittleEndian>()?;
    let life_bar = read_osu_string(&mut cursor)?;
    let timestamp = cursor.read_i64::<LittleEndian>()?;
    let compressed = read_legacy_byte_array(&mut cursor)?.unwrap_or_default();
    let online_score_id = cursor.read_i64::<LittleEndian>().unwrap_or(0);
    let origin = if version >= FIRST_LAZER_VERSION {
        ReplayOrigin::LazerExport
    } else {
        ReplayOrigin::StableLegacy
    };
    let mut mod_info = ReplayModInfo {
        legacy_bits: mods,
        api_mods: Vec::new(),
        has_classic: false,
        has_score_v2: crate::utils::mods::has_scorev2(mods),
        display_mods: None,
    };
    let mut score_info = None;
    if version >= FIRST_LAZER_SCORE_INFO_VERSION {
        // Newer lazer exports append score JSON as an optional compressed byte array.
        if let Some(extra_blob) = try_read_optional_legacy_byte_array(&mut cursor)? {
            if let Some(parsed) = parse_lazer_score_info_blob(&extra_blob, mods) {
                mod_info = parsed.mod_info;
                score_info = Some(parsed.score_info);
            }
        }
    }
    let decompressed = decompress_lzma(&compressed, MAX_REPLAY_FRAME_DATA_BYTES)?;
    let frame_str = String::from_utf8_lossy(&decompressed);
    let frames = parse_replay_frames(&frame_str)?;
    let key_count = detect_key_count(mods);
    let key_actions = ManiaReplayData::derive_key_actions(&frames, key_count);
    let replay = ReplayData {
        game_mode,
        version,
        beatmap_hash,
        player_name,
        replay_hash,
        count_300,
        count_100,
        count_50,
        count_geki,
        count_katu,
        count_miss,
        total_score,
        max_combo,
        perfect_combo,
        mods,
        life_bar,
        timestamp,
        online_score_id,
        origin,
        mod_info,
        score_info,
    };
    Ok(ManiaReplayData {
        replay,
        frames,
        key_actions,
        beatmap_file: None,
    })
}
#[derive(Debug, Deserialize)]
struct RawLazerScoreInfo {
    #[serde(default)]
    online_id: Option<i64>,
    #[serde(default)]
    mods: Vec<RawApiModEntry>,
    #[serde(default)]
    statistics: std::collections::HashMap<String, i32>,
    #[serde(default)]
    maximum_statistics: std::collections::HashMap<String, i32>,
    #[serde(default)]
    client_version: Option<String>,
    #[serde(default)]
    pauses: Vec<i32>,
}
#[derive(Debug, Deserialize)]
struct RawApiModEntry {
    #[serde(default)]
    acronym: String,
    #[serde(default)]
    settings: serde_json::Value,
}
#[derive(Debug)]
struct ParsedLazerScoreInfo {
    mod_info: ReplayModInfo,
    score_info: ReplayScoreInfo,
}
fn read_osu_string(cursor: &mut Cursor<&[u8]>) -> Result<String> {
    let indicator = cursor.read_u8()?;
    if indicator == 0x00 {
        return Ok(String::new());
    }
    if indicator != 0x0b {
        return Err(anyhow!("invalid string indicator: {:#x}", indicator));
    }
    // osu! strings store byte length as ULEB128 after the 0x0b marker.
    let len = read_uleb128(cursor)?;
    if len > MAX_OSU_STRING_BYTES {
        return Err(anyhow!(
            "osu string exceeds max size: {} bytes > {} bytes",
            len,
            MAX_OSU_STRING_BYTES
        ));
    }
    if remaining_bytes(cursor) < len {
        return Err(anyhow!("osu string length exceeds remaining replay bytes"));
    }
    let mut buf = vec![0u8; len];
    cursor.read_exact(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}
fn read_uleb128(cursor: &mut Cursor<&[u8]>) -> Result<usize> {
    let mut result = 0usize;
    let mut shift = 0;
    loop {
        let byte = cursor.read_u8()?;
        result |= ((byte & 0x7f) as usize) << shift;
        if (byte & 0x80) == 0 {
            break;
        }
        shift += 7;
        if shift > 35 {
            return Err(anyhow!("uleb128 overflow"));
        }
    }
    Ok(result)
}
fn read_legacy_byte_array(cursor: &mut Cursor<&[u8]>) -> Result<Option<Vec<u8>>> {
    let len = cursor.read_i32::<LittleEndian>()?;
    if len < 0 {
        return Ok(None);
    }
    let len = len as usize;
    if len > MAX_LEGACY_BYTE_ARRAY_BYTES {
        return Err(anyhow!(
            "legacy byte array exceeds max size: {} bytes > {} bytes",
            len,
            MAX_LEGACY_BYTE_ARRAY_BYTES
        ));
    }
    if remaining_bytes(cursor) < len {
        return Err(anyhow!(
            "legacy byte array length exceeds remaining replay bytes"
        ));
    }
    let mut buf = vec![0u8; len];
    cursor.read_exact(&mut buf)?;
    Ok(Some(buf))
}
fn try_read_optional_legacy_byte_array(cursor: &mut Cursor<&[u8]>) -> Result<Option<Vec<u8>>> {
    let remaining = cursor
        .get_ref()
        .len()
        .saturating_sub(cursor.position() as usize);
    if remaining < 4 {
        return Ok(None);
    }
    let len = cursor.read_i32::<LittleEndian>()?;
    if len < 0 {
        return Ok(None);
    }
    let len = len as usize;
    if len > MAX_LEGACY_BYTE_ARRAY_BYTES {
        return Err(anyhow!(
            "legacy optional byte array exceeds max size: {} bytes > {} bytes",
            len,
            MAX_LEGACY_BYTE_ARRAY_BYTES
        ));
    }
    let remaining = cursor
        .get_ref()
        .len()
        .saturating_sub(cursor.position() as usize);
    if remaining < len {
        return Ok(None);
    }
    let mut buf = vec![0u8; len];
    cursor.read_exact(&mut buf)?;
    Ok(Some(buf))
}
fn remaining_bytes(cursor: &Cursor<&[u8]>) -> usize {
    cursor
        .get_ref()
        .len()
        .saturating_sub(cursor.position() as usize)
}
#[derive(Debug)]
struct LimitedVecWriter {
    inner: Vec<u8>,
    max_len: usize,
}
impl LimitedVecWriter {
    fn with_limit(max_len: usize) -> Self {
        Self {
            inner: Vec::new(),
            max_len,
        }
    }
    fn into_inner(self) -> Vec<u8> {
        self.inner
    }
}
impl Write for LimitedVecWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Stop decompression before an attacker-controlled LZMA stream can grow without bound.
        if self.inner.len().saturating_add(buf.len()) > self.max_len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("decompressed payload exceeds {} bytes", self.max_len),
            ));
        }
        self.inner.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn decompress_lzma(data: &[u8], max_output_bytes: usize) -> Result<Vec<u8>> {
    let mut output = LimitedVecWriter::with_limit(max_output_bytes);
    let mut reader = Cursor::new(data);
    lzma_rs::lzma_decompress(&mut reader, &mut output)
        .map_err(|e| anyhow!("lzma decompress failed: {}", e))?;
    Ok(output.into_inner())
}
fn parse_lazer_score_info_blob(data: &[u8], legacy_bits: u32) -> Option<ParsedLazerScoreInfo> {
    let decompressed = decompress_lzma(data, MAX_LAZER_SCORE_INFO_BYTES).ok()?;
    let raw: RawLazerScoreInfo = serde_json::from_slice(&decompressed).ok()?;
    let api_mods: Vec<ApiModEntry> = raw
        .mods
        .into_iter()
        .map(|entry| ApiModEntry {
            acronym: entry.acronym,
            settings: entry.settings,
        })
        .collect();
    let has_classic = api_mods
        .iter()
        .any(|entry| entry.acronym.eq_ignore_ascii_case("CL"));
    let has_score_v2 = api_mods
        .iter()
        .any(|entry| entry.acronym.eq_ignore_ascii_case("SV2"));
    Some(ParsedLazerScoreInfo {
        mod_info: ReplayModInfo {
            legacy_bits,
            has_classic,
            has_score_v2,
            api_mods,
            display_mods: None,
        },
        score_info: ReplayScoreInfo {
            statistics: raw.statistics,
            maximum_statistics: raw.maximum_statistics,
            client_version: raw.client_version.and_then(empty_string_to_none),
            solo_score_online_id: raw.online_id.filter(|value| *value >= 0),
            pauses: raw.pauses,
        },
    })
}
fn empty_string_to_none(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
fn parse_replay_frames(data: &str) -> Result<Vec<ReplayFrame>> {
    if data.len() > MAX_REPLAY_FRAME_DATA_BYTES {
        return Err(anyhow!(
            "replay frame data exceeds max size: {} bytes > {} bytes",
            data.len(),
            MAX_REPLAY_FRAME_DATA_BYTES
        ));
    }
    let mut frames = Vec::new();
    let mut cumulative_time: i32 = 0;
    for segment in data.split(',') {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        let parts: Vec<&str> = segment.split('|').collect();
        if parts.len() < 4 {
            continue;
        }
        let delta = parse_legacy_frame_delta(parts[0]).unwrap_or(0);
        if parts[0] == "-12345" {
            // -12345 marks the trailing RNG seed segment, not an input frame.
            break;
        }
        let x: f32 = parts[1].parse().unwrap_or(0.0);
        let y: f32 = parts[2].parse().unwrap_or(0.0);
        let button_state: u32 = parts[3].parse().unwrap_or(0);
        cumulative_time = cumulative_time
            .checked_add(delta)
            .ok_or_else(|| anyhow!("replay frame time overflow"))?;
        if !(-MAX_REPLAY_ABSOLUTE_TIME_MS..=MAX_REPLAY_ABSOLUTE_TIME_MS).contains(&cumulative_time)
        {
            return Err(anyhow!(
                "replay frame timeline exceeds max absolute time: {} ms",
                cumulative_time
            ));
        }
        frames.push(ReplayFrame {
            time: cumulative_time,
            x,
            y,
            // Mania stores key state in x when x is inside the packed-key range.
            keys: decode_mania_keys(x, button_state),
        });
        if frames.len() > MAX_REPLAY_FRAMES {
            return Err(anyhow!(
                "replay frame count exceeds max allowed frames: {}",
                MAX_REPLAY_FRAMES
            ));
        }
    }
    Ok(sanitise_mania_replay_frames(frames))
}
fn parse_legacy_frame_delta(raw: &str) -> Option<i32> {
    raw.parse::<i32>()
        .ok()
        .or_else(|| raw.parse::<f32>().ok().map(|value| value.round() as i32))
}
fn decode_mania_keys(mouse_x: f32, button_state: u32) -> u32 {
    const MANIA_MOUSE_X_LIMIT: f32 = ((1 << 20) - 1) as f32;
    if (0.0..=MANIA_MOUSE_X_LIMIT).contains(&mouse_x) {
        mouse_x as u32
    } else {
        button_state
    }
}
fn sanitise_mania_replay_frames(mut frames: Vec<ReplayFrame>) -> Vec<ReplayFrame> {
    let mut had_dummy_lead = false;
    // Stable can include dummy lead frames around time zero; normalize them before deriving key actions.
    if frames.len() >= 2 && frames[1].time < frames[0].time {
        frames[1].time = frames[0].time;
        frames[0].time = 0;
    }
    if frames.len() >= 3 && frames[0].time > frames[2].time {
        frames[0].time = frames[2].time;
        frames[1].time = frames[2].time;
    }
    if frames.len() >= 2 && is_legacy_dummy_frame(&frames[1]) {
        had_dummy_lead = true;
        frames.remove(1);
    }
    if !frames.is_empty() && is_legacy_dummy_frame(&frames[0]) {
        had_dummy_lead = true;
        frames.remove(0);
    }
    if had_dummy_lead {
        let marker_idx = frames
            .iter()
            .position(|frame| frame.time >= 0)
            .unwrap_or(frames.len());
        frames.insert(
            marker_idx,
            ReplayFrame {
                time: 0,
                x: 256.0,
                y: -500.0,
                keys: 0,
            },
        );
    }
    frames
}
fn is_legacy_dummy_frame(frame: &ReplayFrame) -> bool {
    (frame.x - 256.0).abs() < f32::EPSILON && (frame.y + 500.0).abs() < f32::EPSILON
}
fn detect_key_count(mods: u32) -> u8 {
    // Mania key-count mods occupy non-contiguous legacy bit positions.
    if mods & (1 << 15) != 0 {
        return 4;
    }
    if mods & (1 << 16) != 0 {
        return 5;
    }
    if mods & (1 << 17) != 0 {
        return 6;
    }
    if mods & (1 << 18) != 0 {
        return 7;
    }
    if mods & (1 << 19) != 0 {
        return 8;
    }
    if mods & (1 << 24) != 0 {
        return 9;
    }
    if mods & (1 << 26) != 0 {
        return 1;
    }
    if mods & (1 << 28) != 0 {
        return 2;
    }
    if mods & (1 << 27) != 0 {
        return 3;
    }
    4
}

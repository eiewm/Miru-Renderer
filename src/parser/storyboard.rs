use crate::types::{
    AnimationLoopType, Storyboard, StoryboardCommand, StoryboardCommandData, StoryboardCommandKind,
    StoryboardLayer, StoryboardLoop, StoryboardObject, StoryboardObjectKind, StoryboardOrigin,
    StoryboardParamFlags, StoryboardTrigger, StoryboardTriggerKind,
};
use anyhow::Result;
pub(crate) type StoryboardVariables = Vec<(String, String)>;
pub fn parse_storyboard_content(content: &str) -> Result<Storyboard> {
    let mut section = StoryboardContentSection::None;
    let mut variables = StoryboardVariables::new();
    let mut lines: Vec<String> = Vec::new();
    for raw in content.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            section = StoryboardContentSection::from_header(trimmed);
            continue;
        }
        match section {
            StoryboardContentSection::Events => {
                let line = decode_storyboard_variables(raw, &variables);
                lines.push(line);
            }
            StoryboardContentSection::Variables => {
                parse_storyboard_variable_line(raw, &mut variables);
            }
            StoryboardContentSection::None => {}
        }
    }
    parse_storyboard_lines(&lines)
}
pub fn parse_storyboard_lines(lines: &[String]) -> Result<Storyboard> {
    let mut objects = Vec::new();
    let mut i = 0usize;
    while i < lines.len() {
        let raw = &lines[i];
        let trimmed = trim_storyboard_indent(raw).trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            i += 1;
            continue;
        }
        if let Some(mut obj) = parse_object_line(trimmed)? {
            let next = i + 1;
            // Storyboard commands belong to the preceding object by indentation, not by braces.
            let base_indent = find_next_indent(lines, next);
            if base_indent > 0 {
                let (cmds, end_idx) = parse_command_block(lines, next, base_indent)?;
                obj.commands = cmds;
                i = end_idx;
            } else {
                i += 1;
            }
            objects.push(obj);
            continue;
        }
        i += 1;
    }
    Ok(Storyboard { objects })
}
fn parse_object_line(line: &str) -> Result<Option<StoryboardObject>> {
    let parts = split_csv(line);
    if parts.is_empty() {
        return Ok(None);
    }
    let obj_type = parts[0].trim().to_lowercase();
    if obj_type != "sprite" && obj_type != "animation" {
        return Ok(None);
    }
    let Some(layer) = parts.get(1).and_then(|s| StoryboardLayer::from_str(s)) else {
        return Ok(None);
    };
    let origin = parts
        .get(2)
        .and_then(|s| StoryboardOrigin::from_str(s))
        .unwrap_or(StoryboardOrigin::Centre);
    let Some(filepath) = parts.get(3).map(|s| s.to_string()) else {
        return Ok(None);
    };
    let x = parts
        .get(4)
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.0);
    let y = parts
        .get(5)
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.0);
    let kind = if obj_type == "animation" {
        let frame_count = parts
            .get(6)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1);
        let frame_delay = parts
            .get(7)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        let loop_type = parts
            .get(8)
            .map(|s| AnimationLoopType::from_str(s))
            .unwrap_or(AnimationLoopType::LoopForever);
        StoryboardObjectKind::Animation {
            filepath,
            x,
            y,
            frame_count,
            frame_delay,
            loop_type,
        }
    } else {
        StoryboardObjectKind::Sprite { filepath, x, y }
    };
    Ok(Some(StoryboardObject {
        layer,
        origin,
        kind,
        commands: Vec::new(),
    }))
}
fn parse_command_block(
    lines: &[String],
    mut i: usize,
    base_indent: usize,
) -> Result<(Vec<StoryboardCommand>, usize)> {
    let mut cmds = Vec::new();
    while i < lines.len() {
        let raw = &lines[i];
        let trimmed = trim_storyboard_indent(raw).trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            i += 1;
            continue;
        }
        if trimmed.starts_with("Sprite,")
            || trimmed.starts_with("Animation,")
            || trimmed.starts_with('[')
        {
            break;
        }
        let indent = count_indent(raw);
        if indent < base_indent {
            break;
        }
        if indent > base_indent {
            // Deeper lines are parsed only through loop or trigger recursion.
            i += 1;
            continue;
        }
        if let Some(mut cmd) = parse_command_line(trimmed)? {
            match &mut cmd {
                StoryboardCommand::Loop(loop_cmd) => {
                    let child_indent = find_next_indent(lines, i + 1);
                    if child_indent > indent {
                        let (children, next) = parse_command_block(lines, i + 1, child_indent)?;
                        loop_cmd.commands = children;
                        i = next;
                    } else {
                        i += 1;
                    }
                }
                StoryboardCommand::Trigger(trigger_cmd) => {
                    let child_indent = find_next_indent(lines, i + 1);
                    if child_indent > indent {
                        let (children, next) = parse_command_block(lines, i + 1, child_indent)?;
                        trigger_cmd.commands = children;
                        i = next;
                    } else {
                        i += 1;
                    }
                }
                _ => {
                    i += 1;
                }
            }
            cmds.push(cmd);
        } else {
            i += 1;
        }
    }
    Ok((cmds, i))
}
fn parse_command_line(line: &str) -> Result<Option<StoryboardCommand>> {
    let parts = split_csv(line);
    if parts.is_empty() {
        return Ok(None);
    }
    let cmd = parts[0].trim().to_uppercase();
    let parsed = match cmd.as_str() {
        "L" => {
            let start_time = parts
                .get(1)
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(0);
            let loop_count = parts
                .get(2)
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(0);
            Some(StoryboardCommand::Loop(StoryboardLoop {
                start_time,
                loop_count,
                commands: Vec::new(),
            }))
        }
        "T" => {
            let trigger_name = parts.get(1).map(|s| s.as_str()).unwrap_or("");
            let Some(trigger) = StoryboardTriggerKind::from_name(trigger_name) else {
                return Ok(None);
            };
            let start_time = parts
                .get(2)
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(0);
            let end_time = parts
                .get(3)
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(start_time);
            Some(StoryboardCommand::Trigger(StoryboardTrigger {
                trigger,
                start_time,
                end_time,
                commands: Vec::new(),
            }))
        }
        _ => parse_simple_command(&cmd, &parts).map(StoryboardCommand::Command),
    };
    Ok(parsed)
}
fn parse_simple_command(cmd: &str, parts: &[String]) -> Option<StoryboardCommandData> {
    let easing = parts
        .get(1)
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    let start_time = parts
        .get(2)
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    let end_time = parts
        .get(3)
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(start_time);
    let values: Vec<f32> = parts
        .iter()
        .skip(4)
        .filter_map(|v| v.parse::<f32>().ok())
        .collect();
    let (kind, start_values, end_values, params) = match cmd {
        "F" => {
            // osu! allows shortened commands; missing end values repeat the start value.
            let (s, e) = match values.len() {
                0 => (0.0, 0.0),
                1 => (values[0], values[0]),
                _ => (values[0], values[1]),
            };
            (
                StoryboardCommandKind::Fade,
                vec![s],
                vec![e],
                StoryboardParamFlags::default(),
            )
        }
        "M" => {
            let (sx, sy, ex, ey) = match values.len() {
                0 => (0.0, 0.0, 0.0, 0.0),
                1 => (values[0], 0.0, values[0], 0.0),
                2 => (values[0], values[1], values[0], values[1]),
                3 => (values[0], values[1], values[2], values[1]),
                _ => (values[0], values[1], values[2], values[3]),
            };
            (
                StoryboardCommandKind::Move,
                vec![sx, sy],
                vec![ex, ey],
                StoryboardParamFlags::default(),
            )
        }
        "MX" => {
            let (s, e) = match values.len() {
                0 => (0.0, 0.0),
                1 => (values[0], values[0]),
                _ => (values[0], values[1]),
            };
            (
                StoryboardCommandKind::MoveX,
                vec![s],
                vec![e],
                StoryboardParamFlags::default(),
            )
        }
        "MY" => {
            let (s, e) = match values.len() {
                0 => (0.0, 0.0),
                1 => (values[0], values[0]),
                _ => (values[0], values[1]),
            };
            (
                StoryboardCommandKind::MoveY,
                vec![s],
                vec![e],
                StoryboardParamFlags::default(),
            )
        }
        "S" => {
            let (s, e) = match values.len() {
                0 => (1.0, 1.0),
                1 => (values[0], values[0]),
                _ => (values[0], values[1]),
            };
            (
                StoryboardCommandKind::Scale,
                vec![s],
                vec![e],
                StoryboardParamFlags::default(),
            )
        }
        "V" => {
            let (sx, sy, ex, ey) = match values.len() {
                0 => (1.0, 1.0, 1.0, 1.0),
                1 => (values[0], values[0], values[0], values[0]),
                2 => (values[0], values[1], values[0], values[1]),
                3 => (values[0], values[1], values[2], values[1]),
                _ => (values[0], values[1], values[2], values[3]),
            };
            (
                StoryboardCommandKind::VectorScale,
                vec![sx, sy],
                vec![ex, ey],
                StoryboardParamFlags::default(),
            )
        }
        "R" => {
            let (s, e) = match values.len() {
                0 => (0.0, 0.0),
                1 => (values[0], values[0]),
                _ => (values[0], values[1]),
            };
            (
                StoryboardCommandKind::Rotate,
                vec![s],
                vec![e],
                StoryboardParamFlags::default(),
            )
        }
        "C" => {
            let (sr, sg, sb, er, eg, eb) = match values.len() {
                0 => (255.0, 255.0, 255.0, 255.0, 255.0, 255.0),
                3 => (
                    values[0], values[1], values[2], values[0], values[1], values[2],
                ),
                6 => (
                    values[0], values[1], values[2], values[3], values[4], values[5],
                ),
                _ => {
                    let r = values.first().copied().unwrap_or(255.0);
                    let g = values.get(1).copied().unwrap_or(255.0);
                    let b = values.get(2).copied().unwrap_or(255.0);
                    (r, g, b, r, g, b)
                }
            };
            (
                StoryboardCommandKind::Color,
                vec![sr, sg, sb],
                vec![er, eg, eb],
                StoryboardParamFlags::default(),
            )
        }
        "P" => {
            let param_raw = parts
                .iter()
                .skip(4)
                .find_map(|p| {
                    if p.trim().is_empty() {
                        None
                    } else {
                        Some(p.trim())
                    }
                })
                .unwrap_or("");
            let params = StoryboardParamFlags::from_str(param_raw);
            (StoryboardCommandKind::Param, Vec::new(), Vec::new(), params)
        }
        _ => return None,
    };
    Some(StoryboardCommandData {
        kind,
        easing,
        start_time,
        end_time,
        start_values,
        end_values,
        params,
    })
}
fn split_csv(line: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    // Storyboard file paths may contain commas when wrapped in quotes.
    for ch in line.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
            }
            ',' if !in_quotes => {
                parts.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    parts.push(current.trim().to_string());
    parts
}
fn count_indent(line: &str) -> usize {
    line.chars()
        .take_while(|&c| is_storyboard_indent(c))
        .count()
}
fn trim_storyboard_indent(line: &str) -> &str {
    line.trim_start_matches(is_storyboard_indent)
}
fn is_storyboard_indent(ch: char) -> bool {
    // osu! storyboard indentation may use spaces, tabs, or underscores.
    ch == ' ' || ch == '_' || ch == '\t'
}
fn find_next_indent(lines: &[String], mut i: usize) -> usize {
    while i < lines.len() {
        let raw = &lines[i];
        let trimmed = trim_storyboard_indent(raw).trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            i += 1;
            continue;
        }
        return count_indent(raw);
    }
    0
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoryboardContentSection {
    None,
    Events,
    Variables,
}
impl StoryboardContentSection {
    fn from_header(header: &str) -> Self {
        if header.eq_ignore_ascii_case("[Events]") {
            Self::Events
        } else if header.eq_ignore_ascii_case("[Variables]") {
            Self::Variables
        } else {
            Self::None
        }
    }
}
pub(crate) fn parse_storyboard_variable_line(line: &str, variables: &mut StoryboardVariables) {
    let Some((key, value)) = line.split_once('=') else {
        return;
    };
    variables.push((key.to_string(), value.to_string()));
}
pub(crate) fn decode_storyboard_variables(line: &str, variables: &StoryboardVariables) -> String {
    if variables.is_empty() || !line.contains('$') {
        return line.to_string();
    }
    let mut decoded = line.to_string();
    let max_passes = variables.len().saturating_mul(4).max(1);
    for _ in 0..max_passes {
        // Multiple passes allow nested variables while bounding cyclic substitutions.
        let original = decoded.clone();
        for (key, value) in variables {
            decoded = decoded.replace(key, value);
        }
        if decoded == original || !decoded.contains('$') {
            break;
        }
    }
    decoded
}

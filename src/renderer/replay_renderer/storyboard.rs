use super::sprites::{z_order, SpriteCommand, SpritePlanner};
use super::ReplayRenderer;
use crate::renderer::gpu::SpriteBlendMode;
use crate::types::{
    AnimationLoopType, Beatmap, HitObject, SampleSet, StoryboardCommand, StoryboardCommandKind,
    StoryboardLayer, StoryboardObject, StoryboardObjectKind, StoryboardOrigin,
    StoryboardParamFlags, StoryboardTriggerKind, TimingPoint,
};
use std::collections::HashMap;
use std::path::PathBuf;
#[derive(Debug, Clone)]
struct TextureInfo {
    id: String,
    w: f32,
    h: f32,
}
#[derive(Debug, Clone)]
enum SpriteKind {
    Sprite {
        tex: TextureInfo,
    },
    Animation {
        frames: Vec<TextureInfo>,
        frame_delay: u32,
        loop_type: AnimationLoopType,
    },
}
#[derive(Debug, Clone)]
struct CommandInstance {
    kind: StoryboardCommandKind,
    easing: i32,
    start_time: i32,
    end_time: i32,
    start_values: Vec<f32>,
    end_values: Vec<f32>,
    params: StoryboardParamFlags,
    trigger_origin: Option<i32>,
}
#[derive(Debug, Clone, Default)]
struct StoryboardTracks {
    fade: Vec<CommandInstance>,
    move_x: Vec<CommandInstance>,
    move_y: Vec<CommandInstance>,
    scale: Vec<CommandInstance>,
    vscale: Vec<CommandInstance>,
    rotate: Vec<CommandInstance>,
    color: Vec<CommandInstance>,
    param_h: Vec<CommandInstance>,
    param_v: Vec<CommandInstance>,
    param_a: Vec<CommandInstance>,
}
#[derive(Debug, Clone)]
struct StoryboardSprite {
    layer: StoryboardLayer,
    origin: StoryboardOrigin,
    kind: SpriteKind,
    base_pos: [f32; 2],
    tracks: StoryboardTracks,
    start_time: i32,
    end_time: i32,
}
#[derive(Debug, Clone, Copy)]
struct HitSoundEvent {
    time: i32,
    normal_set: u8,
    addition_set: u8,
    hit_sound: u8,
}
#[derive(Debug)]
pub struct StoryboardPlayer {
    objects: Vec<StoryboardSprite>,
    scale: f32,
    offset_x: f32,
    offset_y: f32,
    has_overlay_layer: bool,
}
impl StoryboardPlayer {
    pub fn from_beatmap<F>(
        beatmap: &Beatmap,
        mut resolver: F,
        renderer: &mut ReplayRenderer,
    ) -> Result<Option<Self>, String>
    where
        F: FnMut(&str) -> Option<PathBuf>,
    {
        if beatmap.storyboard.is_empty() {
            return Ok(None);
        }
        let hit_events = collect_hitsound_events(beatmap);
        let mut cache = TextureCache::default();
        // osu! storyboards are authored in a 640x480 playfield space centered in the output frame.
        let scale = renderer.cfg.height as f32 / 480.0;
        let offset_x = (renderer.cfg.width as f32 - 640.0 * scale) / 2.0;
        let offset_y = 0.0;
        let mut objects = Vec::new();
        for obj in &beatmap.storyboard.objects {
            if let Some(sprite) =
                build_sprite(obj, &hit_events, &mut resolver, renderer, &mut cache)?
            {
                objects.push(sprite);
            }
        }
        if objects.is_empty() {
            Ok(None)
        } else {
            let has_overlay_layer = objects
                .iter()
                .any(|object| object.layer == StoryboardLayer::Overlay);
            Ok(Some(Self {
                objects,
                scale,
                offset_x,
                offset_y,
                has_overlay_layer,
            }))
        }
    }
    pub fn has_overlay_layer(&self) -> bool {
        self.has_overlay_layer
    }
    pub fn plan_layer(&self, planner: &mut SpritePlanner, time: i32, layer: StoryboardLayer) {
        if layer == StoryboardLayer::Fail {
            // Replay rendering has no live pass/fail branch switch, so fail-only storyboard sprites stay hidden.
            return;
        }
        for obj in &self.objects {
            if obj.layer != layer {
                continue;
            }
            if time < obj.start_time || time > obj.end_time {
                continue;
            }
            if let Some(cmd) = self.build_sprite_command(obj, time) {
                planner.add_sprite(cmd);
            }
        }
    }
    pub fn object_count(&self) -> usize {
        self.objects.len()
    }
    fn build_sprite_command(&self, obj: &StoryboardSprite, time: i32) -> Option<SpriteCommand> {
        let (tex, base_w, base_h) = match &obj.kind {
            SpriteKind::Sprite { tex } => (tex, tex.w, tex.h),
            SpriteKind::Animation {
                frames,
                frame_delay,
                loop_type,
            } => {
                if frames.is_empty() {
                    return None;
                }
                let idx = animation_frame_index(
                    time,
                    obj.start_time,
                    *frame_delay,
                    frames.len() as u32,
                    *loop_type,
                );
                let frame = &frames[idx as usize];
                (frame, frame.w, frame.h)
            }
        };
        let base_x = obj.base_pos[0];
        let base_y = obj.base_pos[1];
        let (x, _) = eval_f32(&obj.tracks.move_x, time, base_x);
        let (y, _) = eval_f32(&obj.tracks.move_y, time, base_y);
        let (scale_uni, has_scale) = eval_f32(&obj.tracks.scale, time, 1.0);
        let (scale_vec, has_vscale) = eval_vec2(&obj.tracks.vscale, time, [1.0, 1.0]);
        let scale_factor = if has_scale { scale_uni } else { 1.0 };
        let vector_scale = if has_vscale { scale_vec } else { [1.0, 1.0] };
        let mut sx = scale_factor * vector_scale[0];
        let mut sy = scale_factor * vector_scale[1];
        let (rotation, _) = eval_f32(&obj.tracks.rotate, time, 0.0);
        let (color, _) = eval_vec3(&obj.tracks.color, time, [255.0, 255.0, 255.0]);
        let (mut alpha, _) = eval_fade(&obj.tracks.fade, time);
        if alpha > 1.0 {
            alpha %= 1.0;
        }
        if alpha <= 0.001 {
            return None;
        }
        let mut flip_h = eval_param(&obj.tracks.param_h, time);
        let mut flip_v = eval_param(&obj.tracks.param_v, time);
        let blend_mode = if eval_param(&obj.tracks.param_a, time) {
            SpriteBlendMode::Additive
        } else {
            SpriteBlendMode::Alpha
        };
        if sx < 0.0 {
            sx = -sx;
            flip_h = !flip_h;
        }
        if sy < 0.0 {
            sy = -sy;
            flip_v = !flip_v;
        }
        let w = (base_w * self.scale * sx).abs();
        let h = (base_h * self.scale * sy).abs();
        if w < 0.5 || h < 0.5 {
            return None;
        }
        let origin_offset = origin_offset(obj.origin, w, h);
        let screen_x = x * self.scale + self.offset_x;
        let screen_y = y * self.scale + self.offset_y;
        let left = screen_x - origin_offset[0];
        let top = screen_y - origin_offset[1];
        let mut uv = [0.0, 0.0, 1.0, 1.0];
        if flip_h {
            uv.swap(0, 2);
        }
        if flip_v {
            uv.swap(1, 3);
        }
        let tint = [
            (color[0] / 255.0).clamp(0.0, 1.0),
            (color[1] / 255.0).clamp(0.0, 1.0),
            (color[2] / 255.0).clamp(0.0, 1.0),
            alpha.clamp(0.0, 1.0),
        ];
        let z = match obj.layer {
            StoryboardLayer::Background => z_order::STORYBOARD_BACKGROUND,
            StoryboardLayer::Fail => z_order::STORYBOARD_FAIL,
            StoryboardLayer::Pass => z_order::STORYBOARD_PASS,
            StoryboardLayer::Foreground => z_order::STORYBOARD_FOREGROUND,
            StoryboardLayer::Overlay => z_order::STORYBOARD_OVERLAY,
        };
        Some(SpriteCommand {
            texture_id: tex.id.clone(),
            x: left.round() as i32,
            y: top.round() as i32,
            width: w.round().max(1.0) as u32,
            height: h.round().max(1.0) as u32,
            tint,
            uv_rect: uv,
            origin: origin_offset,
            rotation,
            z_order: z,
            blend_mode,
            ..Default::default()
        })
    }
}
#[derive(Default)]
struct TextureCache {
    map: HashMap<String, TextureInfo>,
}
impl TextureCache {
    fn get_or_load<F>(
        &mut self,
        path: &str,
        resolver: &mut F,
        renderer: &mut ReplayRenderer,
    ) -> Result<Option<TextureInfo>, String>
    where
        F: FnMut(&str) -> Option<PathBuf>,
    {
        let key = normalize_path(path);
        if let Some(tex) = self.map.get(&key) {
            return Ok(Some(tex.clone()));
        }
        // Normalize paths before caching so repeated storyboard references share one GPU upload.
        let Some(file_path) = resolver(path) else {
            return Ok(None);
        };
        let data = match std::fs::read(&file_path) {
            Ok(data) => data,
            Err(_) => return Ok(None),
        };
        let Some(img) = crate::utils::image_proc::load_rgba(&data) else {
            return Ok(None);
        };
        let (w, h) = img.dimensions();
        let tex_id = format!("sb:{}", key);
        if !renderer.load_texture_rgba(&tex_id, img.as_raw(), w, h) {
            return Ok(None);
        }
        let tex = TextureInfo {
            id: tex_id,
            w: w as f32,
            h: h as f32,
        };
        self.map.insert(key, tex.clone());
        Ok(Some(tex))
    }
}
fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}
fn build_sprite<F>(
    obj: &StoryboardObject,
    hit_events: &[HitSoundEvent],
    resolver: &mut F,
    renderer: &mut ReplayRenderer,
    cache: &mut TextureCache,
) -> Result<Option<StoryboardSprite>, String>
where
    F: FnMut(&str) -> Option<PathBuf>,
{
    let (kind, base_pos) = match &obj.kind {
        StoryboardObjectKind::Sprite { filepath, x, y } => {
            let Some(tex) = cache.get_or_load(filepath, resolver, renderer)? else {
                return Ok(None);
            };
            (SpriteKind::Sprite { tex }, [*x, *y])
        }
        StoryboardObjectKind::Animation {
            filepath,
            x,
            y,
            frame_count,
            frame_delay,
            loop_type,
        } => {
            let frames = build_animation_frames(filepath, *frame_count, resolver, renderer, cache)?;
            if frames.is_empty() {
                return Ok(None);
            }
            (
                SpriteKind::Animation {
                    frames,
                    frame_delay: *frame_delay,
                    loop_type: *loop_type,
                },
                [*x, *y],
            )
        }
    };
    let instances = flatten_commands(&obj.commands, hit_events, 0)?;
    if instances.is_empty() {
        return Ok(None);
    }
    let mut start_time = i32::MAX;
    let mut end_time = i32::MIN;
    for inst in &instances {
        if inst.start_time < start_time {
            start_time = inst.start_time;
        }
        if inst.end_time > end_time {
            end_time = inst.end_time;
        }
    }
    let tracks = build_tracks(instances);
    Ok(Some(StoryboardSprite {
        layer: obj.layer,
        origin: obj.origin,
        kind,
        base_pos,
        tracks,
        start_time,
        end_time,
    }))
}
fn build_animation_frames<F>(
    filepath: &str,
    frame_count: u32,
    resolver: &mut F,
    renderer: &mut ReplayRenderer,
    cache: &mut TextureCache,
) -> Result<Vec<TextureInfo>, String>
where
    F: FnMut(&str) -> Option<PathBuf>,
{
    let mut frames = Vec::new();
    for idx in 0..frame_count {
        let frame_path = animation_frame_path(filepath, idx);
        if let Some(tex) = cache.get_or_load(&frame_path, resolver, renderer)? {
            frames.push(tex);
        }
    }
    Ok(frames)
}
fn animation_frame_path(base: &str, idx: u32) -> String {
    // osu! animation frames insert the frame number before the extension: sprite0.png.
    base.replace('.', &format!("{idx}."))
}
fn animation_frame_index(
    time: i32,
    start_time: i32,
    delay: u32,
    frame_count: u32,
    loop_type: AnimationLoopType,
) -> u32 {
    if frame_count == 0 {
        return 0;
    }
    if delay == 0 {
        return 0;
    }
    let rel = (time - start_time).max(0) as u32;
    let frame = rel / delay;
    match loop_type {
        AnimationLoopType::LoopForever => frame % frame_count,
        AnimationLoopType::LoopOnce => frame.min(frame_count.saturating_sub(1)),
    }
}
fn build_tracks(instances: Vec<CommandInstance>) -> StoryboardTracks {
    let mut tracks = StoryboardTracks::default();
    for inst in instances {
        match inst.kind {
            StoryboardCommandKind::Fade => tracks.fade.push(inst),
            StoryboardCommandKind::Move => {
                if let Some(command) = component_command(&inst, StoryboardCommandKind::MoveX, 0) {
                    tracks.move_x.push(command);
                }
                if let Some(command) = component_command(&inst, StoryboardCommandKind::MoveY, 1) {
                    tracks.move_y.push(command);
                }
            }
            StoryboardCommandKind::MoveX => tracks.move_x.push(inst),
            StoryboardCommandKind::MoveY => tracks.move_y.push(inst),
            StoryboardCommandKind::Scale => tracks.scale.push(inst),
            StoryboardCommandKind::VectorScale => tracks.vscale.push(inst),
            StoryboardCommandKind::Rotate => tracks.rotate.push(inst),
            StoryboardCommandKind::Color => tracks.color.push(inst),
            StoryboardCommandKind::Param => {
                if inst.params.h {
                    tracks.param_h.push(inst.clone());
                }
                if inst.params.v {
                    tracks.param_v.push(inst.clone());
                }
                if inst.params.a {
                    tracks.param_a.push(inst.clone());
                }
            }
        }
    }
    sort_tracks(&mut tracks);
    tracks
}
fn component_command(
    command: &CommandInstance,
    kind: StoryboardCommandKind,
    index: usize,
) -> Option<CommandInstance> {
    Some(CommandInstance {
        kind,
        easing: command.easing,
        start_time: command.start_time,
        end_time: command.end_time,
        start_values: vec![*command.start_values.get(index)?],
        end_values: vec![*command.end_values.get(index)?],
        params: command.params,
        trigger_origin: command.trigger_origin,
    })
}
fn sort_tracks(tracks: &mut StoryboardTracks) {
    let sort = |v: &mut Vec<CommandInstance>| v.sort_by_key(|c| (c.start_time, c.end_time));
    sort(&mut tracks.fade);
    sort(&mut tracks.move_x);
    sort(&mut tracks.move_y);
    sort(&mut tracks.scale);
    sort(&mut tracks.vscale);
    sort(&mut tracks.rotate);
    sort(&mut tracks.color);
    sort(&mut tracks.param_h);
    sort(&mut tracks.param_v);
    sort(&mut tracks.param_a);
}
fn flatten_commands(
    commands: &[StoryboardCommand],
    hit_events: &[HitSoundEvent],
    base_offset: i64,
) -> Result<Vec<CommandInstance>, String> {
    flatten_commands_with_origin(commands, hit_events, base_offset, None)
}
fn flatten_commands_with_origin(
    commands: &[StoryboardCommand],
    hit_events: &[HitSoundEvent],
    base_offset: i64,
    trigger_origin: Option<i32>,
) -> Result<Vec<CommandInstance>, String> {
    let mut out = Vec::new();
    for cmd in commands {
        match cmd {
            StoryboardCommand::Command(data) => {
                let start_time = checked_storyboard_time(base_offset, data.start_time as i64)?;
                let end_time =
                    checked_storyboard_time(base_offset, data.end_time as i64)?.max(start_time);
                out.push(CommandInstance {
                    kind: data.kind,
                    easing: data.easing,
                    start_time,
                    end_time,
                    start_values: data.start_values.clone(),
                    end_values: data.end_values.clone(),
                    params: data.params,
                    trigger_origin,
                });
            }
            StoryboardCommand::Loop(loop_cmd) => {
                let Some((loop_start, loop_end)) = command_time_bounds(&loop_cmd.commands) else {
                    continue;
                };
                let span = (loop_end - loop_start).max(0);
                let count = loop_total_iterations(loop_cmd.loop_count);
                // Loops are flattened into absolute command times so runtime evaluation can stay track-based.
                let loop_base = (base_offset)
                    .checked_add(loop_cmd.start_time as i64)
                    .ok_or_else(|| "storyboard loop time overflowed".to_string())?;
                for i in 0..count {
                    let loop_offset = loop_base
                        .checked_add((i as i64).saturating_mul(span))
                        .ok_or_else(|| "storyboard loop expansion overflowed".to_string())?;
                    let nested = flatten_commands_with_origin(
                        &loop_cmd.commands,
                        hit_events,
                        loop_offset,
                        trigger_origin,
                    )?;
                    out.extend(nested);
                }
            }
            StoryboardCommand::Trigger(trigger) => {
                // Triggers duplicate their child commands for every matching hitsound event in the trigger window.
                for ev in hit_events.iter().filter(|ev| {
                    ev.time >= trigger.start_time
                        && ev.time <= trigger.end_time
                        && trigger_matches(trigger.trigger, ev)
                }) {
                    let nested = flatten_commands_with_origin(
                        &trigger.commands,
                        hit_events,
                        base_offset
                            .checked_add(ev.time as i64)
                            .ok_or_else(|| "storyboard trigger time overflowed".to_string())?,
                        Some(ev.time),
                    )?;
                    out.extend(nested);
                }
            }
        }
    }
    Ok(out)
}
fn loop_total_iterations(loop_count: i32) -> usize {
    loop_count.max(1) as usize
}
fn command_time_bounds(commands: &[StoryboardCommand]) -> Option<(i64, i64)> {
    let mut min_start = i64::MAX;
    let mut max_end = i64::MIN;
    for cmd in commands {
        let bounds = match cmd {
            StoryboardCommand::Command(data) => {
                let start = data.start_time as i64;
                let end = data.end_time.max(data.start_time) as i64;
                Some((start, end))
            }
            StoryboardCommand::Loop(loop_cmd) => {
                command_time_bounds(&loop_cmd.commands).map(|(inner_start, inner_end)| {
                    let period = (inner_end - inner_start).max(0);
                    let iterations = loop_total_iterations(loop_cmd.loop_count) as i64;
                    let start = (loop_cmd.start_time as i64).saturating_add(inner_start);
                    let end = (loop_cmd.start_time as i64)
                        .saturating_add(inner_end)
                        .saturating_add(period.saturating_mul(iterations.saturating_sub(1)));
                    (start, end)
                })
            }
            StoryboardCommand::Trigger(trigger) => {
                command_time_bounds(&trigger.commands).map(|(inner_start, inner_end)| {
                    (
                        (trigger.start_time as i64).saturating_add(inner_start),
                        (trigger.end_time as i64).saturating_add(inner_end),
                    )
                })
            }
        };
        if let Some((start, end)) = bounds {
            min_start = min_start.min(start);
            max_end = max_end.max(end);
        }
    }
    if min_start == i64::MAX {
        None
    } else {
        Some((min_start, max_end))
    }
}
fn checked_storyboard_time(base_offset: i64, relative_time: i64) -> Result<i32, String> {
    let absolute = base_offset
        .checked_add(relative_time)
        .ok_or_else(|| "storyboard command time overflowed".to_string())?;
    i32::try_from(absolute).map_err(|_| "storyboard command time does not fit in i32".to_string())
}
fn origin_offset(origin: StoryboardOrigin, w: f32, h: f32) -> [f32; 2] {
    match origin {
        StoryboardOrigin::TopLeft => [0.0, 0.0],
        StoryboardOrigin::TopCentre => [w / 2.0, 0.0],
        StoryboardOrigin::TopRight => [w, 0.0],
        StoryboardOrigin::CentreLeft => [0.0, h / 2.0],
        StoryboardOrigin::Centre => [w / 2.0, h / 2.0],
        StoryboardOrigin::CentreRight => [w, h / 2.0],
        StoryboardOrigin::BottomLeft => [0.0, h],
        StoryboardOrigin::BottomCentre => [w / 2.0, h],
        StoryboardOrigin::BottomRight => [w, h],
    }
}
fn eval_f32(cmds: &[CommandInstance], time: i32, default: f32) -> (f32, bool) {
    if cmds.is_empty() {
        return (default, false);
    }
    let idx = cmds.partition_point(|c| c.start_time <= time);
    if idx == 0 {
        let cmd = &cmds[0];
        if !cmd.start_values.is_empty() {
            return (cmd.start_values[0], true);
        }
        return (default, false);
    }
    let cmd = &cmds[idx - 1];
    let val = eval_command(cmd, time, default);
    (val, true)
}
fn eval_fade(cmds: &[CommandInstance], time: i32) -> (f32, bool) {
    if cmds.is_empty() {
        return (1.0, false);
    }
    if let Some(first) = cmds.first() {
        if first.trigger_origin.is_some() && time < first.start_time {
            return (0.0, false);
        }
    }
    let idx = cmds.partition_point(|c| c.start_time <= time);
    if idx == 0 {
        let cmd = &cmds[0];
        if !cmd.start_values.is_empty() {
            return (cmd.start_values[0], true);
        }
        return (1.0, false);
    }
    let cmd = select_effective_fade_command(&cmds[..idx]);
    let val = eval_command(cmd, time, 1.0);
    (val, true)
}
fn select_effective_fade_command(cmds: &[CommandInstance]) -> &CommandInstance {
    let latest_trigger_origin = cmds.iter().filter_map(|cmd| cmd.trigger_origin).max();
    if let Some(origin) = latest_trigger_origin {
        // Normal fades after a trigger origin resume control over trigger-created fades.
        if let Some(normal) = cmds
            .iter()
            .rev()
            .find(|cmd| cmd.trigger_origin.is_none() && cmd.start_time >= origin)
        {
            return normal;
        }
        if let Some(trigger_cmd) = cmds
            .iter()
            .rev()
            .find(|cmd| cmd.trigger_origin == Some(origin))
        {
            return trigger_cmd;
        }
    }
    cmds.last().expect("fade command slice should not be empty")
}
fn eval_vec2(cmds: &[CommandInstance], time: i32, default: [f32; 2]) -> ([f32; 2], bool) {
    if cmds.is_empty() {
        return (default, false);
    }
    let idx = cmds.partition_point(|c| c.start_time <= time);
    if idx == 0 {
        let cmd = &cmds[0];
        if cmd.start_values.len() >= 2 {
            return ([cmd.start_values[0], cmd.start_values[1]], true);
        }
        return (default, false);
    }
    let cmd = &cmds[idx - 1];
    let val = eval_command_vec2(cmd, time, default);
    (val, true)
}
fn eval_vec3(cmds: &[CommandInstance], time: i32, default: [f32; 3]) -> ([f32; 3], bool) {
    if cmds.is_empty() {
        return (default, false);
    }
    let idx = cmds.partition_point(|c| c.start_time <= time);
    if idx == 0 {
        let cmd = &cmds[0];
        if cmd.start_values.len() >= 3 {
            return (
                [
                    cmd.start_values[0],
                    cmd.start_values[1],
                    cmd.start_values[2],
                ],
                true,
            );
        }
        return (default, false);
    }
    let cmd = &cmds[idx - 1];
    let val = eval_command_vec3(cmd, time, default);
    (val, true)
}
fn eval_param(cmds: &[CommandInstance], time: i32) -> bool {
    if cmds.is_empty() {
        return false;
    }
    let idx = cmds.partition_point(|c| c.start_time <= time);
    if idx == 0 {
        return false;
    }
    let cmd = &cmds[idx - 1];
    if cmd.end_time == cmd.start_time {
        time >= cmd.start_time
    } else {
        time >= cmd.start_time && time <= cmd.end_time
    }
}
fn eval_command(cmd: &CommandInstance, time: i32, default: f32) -> f32 {
    if cmd.start_values.is_empty() || cmd.end_values.is_empty() {
        return default;
    }
    if time <= cmd.start_time {
        return cmd.start_values[0];
    }
    if time >= cmd.end_time {
        return cmd.end_values[0];
    }
    let duration = (cmd.end_time - cmd.start_time).max(1) as f32;
    let t = (time - cmd.start_time) as f32 / duration;
    let eased = apply_easing(cmd.easing, t);
    lerp(cmd.start_values[0], cmd.end_values[0], eased)
}
fn eval_command_vec2(cmd: &CommandInstance, time: i32, default: [f32; 2]) -> [f32; 2] {
    if cmd.start_values.len() < 2 || cmd.end_values.len() < 2 {
        return default;
    }
    if time <= cmd.start_time {
        return [cmd.start_values[0], cmd.start_values[1]];
    }
    if time >= cmd.end_time {
        return [cmd.end_values[0], cmd.end_values[1]];
    }
    let duration = (cmd.end_time - cmd.start_time).max(1) as f32;
    let t = (time - cmd.start_time) as f32 / duration;
    let eased = apply_easing(cmd.easing, t);
    [
        lerp(cmd.start_values[0], cmd.end_values[0], eased),
        lerp(cmd.start_values[1], cmd.end_values[1], eased),
    ]
}
fn eval_command_vec3(cmd: &CommandInstance, time: i32, default: [f32; 3]) -> [f32; 3] {
    if cmd.start_values.len() < 3 || cmd.end_values.len() < 3 {
        return default;
    }
    if time <= cmd.start_time {
        return [
            cmd.start_values[0],
            cmd.start_values[1],
            cmd.start_values[2],
        ];
    }
    if time >= cmd.end_time {
        return [cmd.end_values[0], cmd.end_values[1], cmd.end_values[2]];
    }
    let duration = (cmd.end_time - cmd.start_time).max(1) as f32;
    let t = (time - cmd.start_time) as f32 / duration;
    let eased = apply_easing(cmd.easing, t);
    [
        lerp(cmd.start_values[0], cmd.end_values[0], eased),
        lerp(cmd.start_values[1], cmd.end_values[1], eased),
        lerp(cmd.start_values[2], cmd.end_values[2], eased),
    ]
}
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
fn apply_easing(easing: i32, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    // Numeric easing ids are storyboard format values, not local enum discriminants.
    match easing {
        0 => t,
        1 => ease_out_quad(t),
        2 => ease_in_quad(t),
        3 => ease_in_quad(t),
        4 => ease_out_quad(t),
        5 => ease_in_out_quad(t),
        6 => ease_in_cubic(t),
        7 => ease_out_cubic(t),
        8 => ease_in_out_cubic(t),
        9 => ease_in_quart(t),
        10 => ease_out_quart(t),
        11 => ease_in_out_quart(t),
        12 => ease_in_quint(t),
        13 => ease_out_quint(t),
        14 => ease_in_out_quint(t),
        15 => ease_in_sine(t),
        16 => ease_out_sine(t),
        17 => ease_in_out_sine(t),
        18 => ease_in_expo(t),
        19 => ease_out_expo(t),
        20 => ease_in_out_expo(t),
        21 => ease_in_circ(t),
        22 => ease_out_circ(t),
        23 => ease_in_out_circ(t),
        24 => ease_in_elastic(t),
        25 => ease_out_elastic(t),
        26 => ease_out_elastic_half(t),
        27 => ease_out_elastic_quarter(t),
        28 => ease_in_out_elastic(t),
        29 => ease_in_back(t),
        30 => ease_out_back(t),
        31 => ease_in_out_back(t),
        32 => ease_in_bounce(t),
        33 => ease_out_bounce(t),
        34 => ease_in_out_bounce(t),
        _ => t,
    }
}
fn ease_in_quad(t: f32) -> f32 {
    t * t
}
fn ease_out_quad(t: f32) -> f32 {
    1.0 - (1.0 - t) * (1.0 - t)
}
fn ease_in_out_quad(t: f32) -> f32 {
    if t < 0.5 {
        2.0 * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
    }
}
fn ease_in_cubic(t: f32) -> f32 {
    t * t * t
}
fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}
fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}
fn ease_in_quart(t: f32) -> f32 {
    t.powi(4)
}
fn ease_out_quart(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(4)
}
fn ease_in_out_quart(t: f32) -> f32 {
    if t < 0.5 {
        8.0 * t.powi(4)
    } else {
        1.0 - (-2.0 * t + 2.0).powi(4) / 2.0
    }
}
fn ease_in_quint(t: f32) -> f32 {
    t.powi(5)
}
fn ease_out_quint(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(5)
}
fn ease_in_out_quint(t: f32) -> f32 {
    if t < 0.5 {
        16.0 * t.powi(5)
    } else {
        1.0 - (-2.0 * t + 2.0).powi(5) / 2.0
    }
}
fn ease_in_sine(t: f32) -> f32 {
    1.0 - (t * std::f32::consts::FRAC_PI_2).cos()
}
fn ease_out_sine(t: f32) -> f32 {
    (t * std::f32::consts::FRAC_PI_2).sin()
}
fn ease_in_out_sine(t: f32) -> f32 {
    -((std::f32::consts::PI * t).cos() - 1.0) / 2.0
}
fn ease_in_expo(t: f32) -> f32 {
    if t == 0.0 {
        0.0
    } else {
        2.0_f32.powf(10.0 * t - 10.0)
    }
}
fn ease_out_expo(t: f32) -> f32 {
    if t == 1.0 {
        1.0
    } else {
        1.0 - 2.0_f32.powf(-10.0 * t)
    }
}
fn ease_in_out_expo(t: f32) -> f32 {
    if t == 0.0 {
        return 0.0;
    }
    if t == 1.0 {
        return 1.0;
    }
    if t < 0.5 {
        2.0_f32.powf(20.0 * t - 10.0) / 2.0
    } else {
        (2.0 - 2.0_f32.powf(-20.0 * t + 10.0)) / 2.0
    }
}
fn ease_in_circ(t: f32) -> f32 {
    1.0 - (1.0 - t * t).sqrt()
}
fn ease_out_circ(t: f32) -> f32 {
    (1.0 - (t - 1.0).powi(2)).sqrt()
}
fn ease_in_out_circ(t: f32) -> f32 {
    if t < 0.5 {
        (1.0 - (1.0 - (2.0 * t).powi(2)).sqrt()) / 2.0
    } else {
        ((1.0 - (-2.0 * t + 2.0).powi(2)).sqrt() + 1.0) / 2.0
    }
}
fn ease_in_elastic(t: f32) -> f32 {
    if t == 0.0 {
        return 0.0;
    }
    if t == 1.0 {
        return 1.0;
    }
    let c4 = (2.0 * std::f32::consts::PI) / 3.0;
    -(2.0_f32.powf(10.0 * t - 10.0)) * ((t * 10.0 - 10.75) * c4).sin()
}
fn ease_out_elastic(t: f32) -> f32 {
    if t == 0.0 {
        return 0.0;
    }
    if t == 1.0 {
        return 1.0;
    }
    let c4 = (2.0 * std::f32::consts::PI) / 3.0;
    2.0_f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
}
fn ease_out_elastic_half(t: f32) -> f32 {
    if t == 0.0 {
        return 0.0;
    }
    if t == 1.0 {
        return 1.0;
    }
    let c4 = (2.0 * std::f32::consts::PI) / 2.0;
    2.0_f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
}
fn ease_out_elastic_quarter(t: f32) -> f32 {
    if t == 0.0 {
        return 0.0;
    }
    if t == 1.0 {
        return 1.0;
    }
    let c4 = (2.0 * std::f32::consts::PI) / 1.0;
    2.0_f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
}
fn ease_in_out_elastic(t: f32) -> f32 {
    if t == 0.0 {
        return 0.0;
    }
    if t == 1.0 {
        return 1.0;
    }
    let c5 = (2.0 * std::f32::consts::PI) / 4.5;
    if t < 0.5 {
        -(2.0_f32.powf(20.0 * t - 10.0)) * ((20.0 * t - 11.125) * c5).sin() / 2.0
    } else {
        2.0_f32.powf(-20.0 * t + 10.0) * ((20.0 * t - 11.125) * c5).sin() / 2.0 + 1.0
    }
}
fn ease_in_back(t: f32) -> f32 {
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    c3 * t * t * t - c1 * t * t
}
fn ease_out_back(t: f32) -> f32 {
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}
fn ease_in_out_back(t: f32) -> f32 {
    let c1 = 1.70158;
    let c2 = c1 * 1.525;
    if t < 0.5 {
        ((2.0 * t).powi(2) * ((c2 + 1.0) * 2.0 * t - c2)) / 2.0
    } else {
        ((2.0 * t - 2.0).powi(2) * ((c2 + 1.0) * (t * 2.0 - 2.0) + c2) + 2.0) / 2.0
    }
}
fn ease_out_bounce(t: f32) -> f32 {
    let n1 = 7.5625;
    let d1 = 2.75;
    if t < 1.0 / d1 {
        n1 * t * t
    } else if t < 2.0 / d1 {
        let t = t - 1.5 / d1;
        n1 * t * t + 0.75
    } else if t < 2.5 / d1 {
        let t = t - 2.25 / d1;
        n1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / d1;
        n1 * t * t + 0.984375
    }
}
fn ease_in_bounce(t: f32) -> f32 {
    1.0 - ease_out_bounce(1.0 - t)
}
fn ease_in_out_bounce(t: f32) -> f32 {
    if t < 0.5 {
        (1.0 - ease_out_bounce(1.0 - 2.0 * t)) / 2.0
    } else {
        (1.0 + ease_out_bounce(2.0 * t - 1.0)) / 2.0
    }
}
fn trigger_matches(trigger: StoryboardTriggerKind, ev: &HitSoundEvent) -> bool {
    if trigger.any {
        return true;
    }
    if let Some(set) = trigger.sample_set {
        let id = match set {
            SampleSet::Normal => 1,
            SampleSet::Soft => 2,
            SampleSet::Drum => 3,
        };
        if trigger.hit_sound.is_some() {
            if ev.addition_set != id {
                return false;
            }
        } else if ev.normal_set != id && ev.addition_set != id {
            return false;
        }
    }
    if let Some(hit) = trigger.hit_sound {
        // Storyboard hitsound triggers use osu! bit flags: whistle=2, finish=4, clap=8.
        let flag = match hit {
            crate::types::HitSoundType::Whistle => 2,
            crate::types::HitSoundType::Finish => 4,
            crate::types::HitSoundType::Clap => 8,
        };
        if (ev.hit_sound & flag) == 0 {
            return false;
        }
    }
    true
}
fn collect_hitsound_events(beatmap: &Beatmap) -> Vec<HitSoundEvent> {
    let mut events = Vec::with_capacity(beatmap.hit_objects.len());
    for ho in &beatmap.hit_objects {
        let (normal_set, addition_set) = resolve_sample_sets(beatmap, ho);
        let mut hit_sound = ho.hit_sound;
        if hit_sound == 0 {
            hit_sound = 1;
        }
        events.push(HitSoundEvent {
            time: ho.time,
            normal_set,
            addition_set,
            hit_sound,
        });
    }
    events.sort_by_key(|e| e.time);
    events
}
fn resolve_sample_sets(beatmap: &Beatmap, ho: &HitObject) -> (u8, u8) {
    let tp = timing_point_at(beatmap, ho.time);
    let timing_set = tp.map(|t| t.sample_set).unwrap_or(1);
    let normal_set = if ho.hit_sample.normal_set > 0 {
        ho.hit_sample.normal_set
    } else if timing_set > 0 {
        timing_set
    } else {
        1
    };
    let addition_set = if ho.hit_sample.addition_set > 0 {
        ho.hit_sample.addition_set
    } else {
        normal_set
    };
    (normal_set, addition_set)
}
fn timing_point_at(beatmap: &Beatmap, time: i32) -> Option<&TimingPoint> {
    let mut chosen: Option<&TimingPoint> = None;
    for tp in &beatmap.timing_points {
        if tp.time as i32 <= time {
            chosen = Some(tp);
        } else {
            break;
        }
    }
    chosen
}

use super::*;
const MAX_ALLOWED_STAR_RATING: f64 = 80.0;
const STABLE_REPLAY_NEAR_FAIL_LIFE: f32 = 0.025;
fn seconds_to_millis(seconds: f32, arg_name: &str) -> Result<i64, ConvertError> {
    if !seconds.is_finite() {
        return Err(ConvertError::Resolve(format!("{arg_name} must be finite")));
    }
    if seconds < 0.0 {
        return Err(ConvertError::Resolve(format!("{arg_name} must be >= 0")));
    }
    Ok((f64::from(seconds) * 1000.0).round() as i64)
}
pub(crate) fn apply_fail_policy_to_health_timeline(
    mut health_timeline: crate::renderer::HealthTimeline,
    replay: &ReplayData,
) -> crate::renderer::HealthTimeline {
    // No Fail keeps the life graph but suppresses the render fail animation.
    if crate::utils::mods::has_no_fail_mod(replay.mods)
        || crate::utils::mods::replay_has_api_mod(replay, "NF")
    {
        if let Some(fail_time_ms) = health_timeline.fail_time_ms.take() {
            println!(
                "   [health] NF active, suppressing fail trigger at {}ms",
                fail_time_ms
            );
        }
    }
    health_timeline
}
pub(crate) fn finalize_health_timeline_for_replay(
    health_timeline: crate::renderer::HealthTimeline,
    replay: &ReplayData,
    score_judgments: &[judgment::ScoreJudgmentEvent],
    fail_conditions: ReplayFailConditions,
) -> crate::renderer::HealthTimeline {
    let health_timeline =
        apply_replay_life_bar_fail_state(health_timeline, replay, score_judgments);
    // Replay metadata can be more authoritative than simulated HP for fail timing.
    let health_timeline = apply_replay_fail_condition_mod_to_health_timeline(
        health_timeline,
        replay,
        score_judgments,
        fail_conditions,
    );
    apply_fail_policy_to_health_timeline(health_timeline, replay)
}
fn apply_replay_fail_condition_mod_to_health_timeline(
    mut health_timeline: crate::renderer::HealthTimeline,
    replay: &ReplayData,
    score_judgments: &[judgment::ScoreJudgmentEvent],
    fail_conditions: ReplayFailConditions,
) -> crate::renderer::HealthTimeline {
    let Some(fail_time_ms) =
        first_fail_time_from_score_judgments(replay, score_judgments, fail_conditions)
    else {
        return health_timeline;
    };
    match health_timeline.fail_time_ms {
        Some(existing_fail_time_ms) if fail_time_ms < existing_fail_time_ms => {
            println!(
                "   [health] replay fail mod overrides fail trigger {}ms -> {}ms",
                existing_fail_time_ms, fail_time_ms
            );
            health_timeline.fail_time_ms = Some(fail_time_ms);
        }
        None => {
            println!(
                "   [health] replay fail mod supplies fail trigger at {}ms",
                fail_time_ms
            );
            health_timeline.fail_time_ms = Some(fail_time_ms);
        }
        _ => {}
    }
    health_timeline
}
fn first_fail_time_from_score_judgments(
    replay: &ReplayData,
    score_judgments: &[judgment::ScoreJudgmentEvent],
    fail_conditions: ReplayFailConditions,
) -> Option<i32> {
    if replay.origin != crate::types::ReplayOrigin::LazerExport || fail_conditions.is_empty() {
        return None;
    }
    if score_judgments.is_empty() {
        return None;
    }
    let score_constants = crate::renderer::ScoreConstants::for_mode(judgment::ScoreMode::Lazer);
    let total_judgments = score_judgments.len() as u32;
    let max_per_hit = score_constants.acc_max_per_hit as f64;
    let total_max_hits = total_judgments as f64 * max_per_hit;
    let mut acc_hits = 0u32;
    for (idx, judgment) in score_judgments.iter().enumerate() {
        acc_hits += score_constants.acc_weight(judgment.kind);
        let judged_count = idx as f64 + 1.0;
        let standard_accuracy = if judged_count > 0.0 {
            acc_hits as f64 / (judged_count * max_per_hit)
        } else {
            1.0
        };
        let remaining_judgments = total_judgments.saturating_sub(idx as u32 + 1);
        let maximum_achievable_accuracy = if total_max_hits > 0.0 {
            (acc_hits as f64 + remaining_judgments as f64 * max_per_hit) / total_max_hits
        } else {
            1.0
        };
        let sudden_death_failed =
            fail_conditions.sudden_death && judgment.kind == crate::types::JudgmentKind::Miss;
        let perfect_failed = fail_conditions
            .perfect
            .is_some_and(|perfect| perfect_fail_triggers_on_judgment(perfect, judgment.kind));
        let accuracy_challenge_failed = fail_conditions.accuracy_challenge.is_some_and(|ac| {
            let measured_accuracy = match ac.mode {
                // MaximumAchievable fails when even perfect remaining hits cannot save the run.
                ReplayAccuracyChallengeMode::MaximumAchievable => maximum_achievable_accuracy,
                ReplayAccuracyChallengeMode::Standard => standard_accuracy,
            };
            measured_accuracy < ac.minimum_accuracy
        });
        if sudden_death_failed || perfect_failed || accuracy_challenge_failed {
            return Some(judgment.event_time);
        }
    }
    None
}
fn perfect_fail_triggers_on_judgment(
    perfect: ReplayPerfectFailCondition,
    kind: crate::types::JudgmentKind,
) -> bool {
    if perfect.require_perfect_hits {
        kind != crate::types::JudgmentKind::Max
    } else {
        !matches!(
            kind,
            crate::types::JudgmentKind::Max | crate::types::JudgmentKind::Hit300
        )
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
struct ReplayLifeBarSample {
    time_ms: i32,
    life: f32,
}
fn apply_replay_life_bar_fail_state(
    mut health_timeline: crate::renderer::HealthTimeline,
    replay: &ReplayData,
    score_judgments: &[judgment::ScoreJudgmentEvent],
) -> crate::renderer::HealthTimeline {
    // The replay lifebar records the player's actual fail state, including recovery behavior.
    let replay_samples = parse_replay_life_bar_samples(&replay.life_bar);
    if replay_samples.is_empty() {
        return health_timeline;
    }
    let replay_fail_time_ms = replay_samples
        .iter()
        .find(|sample| sample.life <= 0.0)
        .map(|sample| sample.time_ms);
    let near_fail_time_ms = replay_samples
        .iter()
        .find(|sample| sample.life <= STABLE_REPLAY_NEAR_FAIL_LIFE)
        .map(|sample| sample.time_ms);
    let stable_summary_indicates_fail =
        stable_replay_summary_indicates_fail(replay, score_judgments)
            && near_fail_time_ms.is_some();
    match (health_timeline.fail_time_ms, replay_fail_time_ms) {
        (Some(simulated_fail_time_ms), None) => {
            if stable_summary_indicates_fail {
                println!(
                    "   [health] replay lifebar stayed above zero but low HP and truncated stable summary keep fail trigger at {}ms",
                    simulated_fail_time_ms
                );
                return health_timeline;
            }
            println!(
                "   [health] replay lifebar suppresses synthetic fail trigger at {}ms",
                simulated_fail_time_ms
            );
            health_timeline.fail_time_ms = None;
        }
        (Some(simulated_fail_time_ms), Some(replay_fail_time_ms))
            if simulated_fail_time_ms != replay_fail_time_ms =>
        {
            println!(
                "   [health] replay lifebar overrides fail trigger {}ms -> {}ms",
                simulated_fail_time_ms, replay_fail_time_ms
            );
            health_timeline.fail_time_ms = Some(replay_fail_time_ms);
        }
        (None, Some(replay_fail_time_ms)) => {
            println!(
                "   [health] replay lifebar supplies fail trigger at {}ms",
                replay_fail_time_ms
            );
            health_timeline.fail_time_ms = Some(replay_fail_time_ms);
        }
        (None, None) if stable_summary_indicates_fail => {
            let fail_time_ms = near_fail_time_ms.unwrap_or_default();
            println!(
                "   [health] low HP and truncated stable summary supply fail trigger at {}ms",
                fail_time_ms
            );
            health_timeline.fail_time_ms = Some(fail_time_ms);
        }
        _ => {}
    }
    health_timeline
}
fn stable_replay_summary_indicates_fail(
    replay: &ReplayData,
    score_judgments: &[judgment::ScoreJudgmentEvent],
) -> bool {
    if replay.origin != crate::types::ReplayOrigin::StableLegacy {
        return false;
    }
    let replay_total = replay.basic_statistics().total() as usize;
    replay_total > 0 && replay_total < score_judgments.len()
}
fn parse_replay_life_bar_samples(raw: &str) -> Vec<ReplayLifeBarSample> {
    let mut samples: Vec<_> = raw
        .split(',')
        .filter_map(|segment| {
            let segment = segment.trim();
            let (time_raw, life_raw) = segment.split_once('|')?;
            let time_ms = time_raw.parse::<i32>().ok()?;
            let life = life_raw.parse::<f32>().ok()?;
            life.is_finite().then_some(ReplayLifeBarSample {
                time_ms,
                life: life.clamp(0.0, 1.0),
            })
        })
        .collect();
    samples.sort_by_key(|sample| sample.time_ms);
    samples
}

#[derive(Clone)]
pub(crate) struct RenderPlan {
    pub(crate) timeline_start: i32,
    pub(crate) timeline_end: i32,
    pub(crate) playback_profile: crate::utils::mods::PlaybackRateProfile,
    pub(crate) total_frames: u64,
}
fn resolve_allowlisted_asset_path(dir: &Path, relative_name: &str) -> Option<PathBuf> {
    let path = crate::utils::safe_archive_entry_path(dir, relative_name)?;
    path.is_file().then_some(path)
}
impl ManiaVideoConverter {
    pub(crate) fn calculate_star_rating(&self, beatmap_path: &Path) -> Result<f64, ConvertError> {
        let map = rosu_pp::Beatmap::from_path(beatmap_path).map_err(|e| {
            ConvertError::Parse(format!(
                "failed to parse beatmap for star rating ({}): {e}",
                beatmap_path.display()
            ))
        })?;
        if let Err(suspicion) = map.check_suspicion() {
            return Err(ConvertError::Resolve(format!(
                "beatmap rejected by suspicion check: {:?} ({})",
                suspicion,
                beatmap_path.display()
            )));
        }
        let stars = rosu_pp::Difficulty::new().calculate(&map).stars();
        if !stars.is_finite() {
            return Err(ConvertError::Resolve(format!(
                "invalid star rating for beatmap {}",
                beatmap_path.display()
            )));
        }
        Ok(stars)
    }
    pub(crate) fn enforce_star_limit(&self, beatmap_path: &Path) -> Result<f64, ConvertError> {
        let stars = self.calculate_star_rating(beatmap_path)?;
        if stars > MAX_ALLOWED_STAR_RATING {
            return Err(ConvertError::Resolve(format!(
                "beatmap exceeds star limit: {:.2}* > {:.2}* ({})",
                stars,
                MAX_ALLOWED_STAR_RATING,
                beatmap_path.display()
            )));
        }
        Ok(stars)
    }
    pub(crate) fn resolve_audio_local_only(
        &self,
        beatmap_path: &Path,
        filename: &str,
    ) -> Result<Option<PathBuf>, ConvertError> {
        let set_dir = beatmap_path.parent().unwrap_or(Path::new("."));
        Ok(beatmaps::resolve_audio(set_dir, Some(filename)))
    }
    pub(crate) fn list_dir_files_recursive(&self, dir: &Path) -> Vec<String> {
        let mut result = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(current) = stack.pop() {
            if let Ok(entries) = fs::read_dir(&current) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        stack.push(path);
                    } else if let Ok(rel) = path.strip_prefix(dir) {
                        if let Some(rel_str) = rel.to_str() {
                            result.push(rel_str.replace('\\', "/"));
                        }
                    }
                }
            }
        }
        result
    }
    pub(crate) fn build_autoplay_intro_user_data(
        &self,
        override_data: Option<crate::utils::IntroUserData>,
    ) -> crate::utils::IntroUserData {
        let primary = PathBuf::from("assets/miru-avatar.png");
        if !primary.exists() {
            println!(
                "   warn: autoplay avatar not found on disk (assets/miru-avatar.png), using embedded"
            );
        }
        let mut data = override_data.unwrap_or_default();
        data.avatar_path = Some(primary);
        if data.country_code.is_none() {
            data.country_code = Some("EC".to_string());
        }
        data.team_badge_path = None;
        data
    }
    pub(crate) fn resolve_beatmap(
        &self,
        replay: &ManiaReplayData,
        opts: &ResolveOpts,
    ) -> Result<(PathBuf, PathBuf, Vec<String>), ConvertError> {
        let resolve_opts = ResolveOptions {
            osu: opts.osu.clone(),
        };
        let result = beatmaps::resolve_beatmap_from_replay(replay, &resolve_opts)
            .map_err(|e| ConvertError::Resolve(e.to_string()))?;
        Ok((result.osu_path, result.set_dir, result.set_files))
    }
    pub(crate) fn resolve_audio(
        &self,
        beatmap_path: &Path,
        filename: &str,
    ) -> Result<Option<PathBuf>, ConvertError> {
        let set_dir = beatmap_path.parent().unwrap_or(Path::new("."));
        Ok(beatmaps::resolve_audio(set_dir, Some(filename)))
    }
    pub(crate) fn pick_background(
        &self,
        beatmap: &Beatmap,
        _beatmap_path: &Path,
        set_dir: &Path,
        set_files: &[String],
        _opts: &ResolveOpts,
    ) -> Option<BackgroundSource> {
        if self.settings.background_dim >= 1.0 {
            return None;
        }
        let mut events: Vec<(usize, &BackgroundEvent)> =
            beatmap.events.backgrounds.iter().enumerate().collect();
        events.sort_by_key(|(idx, ev)| {
            let (kind_rank, start_time) = match ev {
                BackgroundEvent::Video { start_time, .. } => (0, *start_time),
                BackgroundEvent::Image { .. } => (1, 0),
            };
            (kind_rank, start_time, *idx)
        });
        for (_, bg) in events {
            let (filename, start_time_ms, kind) = match bg {
                BackgroundEvent::Image { filename, .. } => (filename, 0, BackgroundKind::Image),
                BackgroundEvent::Video {
                    filename,
                    start_time,
                    ..
                } => {
                    if !self.settings.background_video_enabled {
                        continue;
                    }
                    (filename, *start_time, BackgroundKind::Video)
                }
            };
            if let Some(path) = self.resolve_asset(set_dir, filename, set_files) {
                return Some(BackgroundSource {
                    kind,
                    path,
                    start_time_ms,
                    dim: self.settings.background_dim,
                });
            }
        }
        None
    }
    pub(crate) fn pick_intro_background(
        &self,
        beatmap: &Beatmap,
        _beatmap_path: &Path,
        set_dir: &Path,
        set_files: &[String],
        bg: Option<&BackgroundSource>,
        _opts: &ResolveOpts,
    ) -> Option<PathBuf> {
        if let Some(bg) = bg {
            if matches!(bg.kind, BackgroundKind::Image) {
                return Some(bg.path.clone());
            }
        }
        let filename = beatmap.events.backgrounds.iter().find_map(|ev| {
            if let BackgroundEvent::Image { filename, .. } = ev {
                Some(filename.as_str())
            } else {
                None
            }
        })?;
        if let Some(path) = self.resolve_asset(set_dir, filename, set_files) {
            return Some(path);
        }
        None
    }
    pub(crate) fn resolve_asset(
        &self,
        dir: &Path,
        filename: &str,
        files: &[String],
    ) -> Option<PathBuf> {
        let targets = asset_lookup_candidates(filename)?;
        for target in &targets {
            let target_normalized = target.to_ascii_lowercase();
            for file in files {
                // Resolve only files discovered in the mapset listing, then revalidate the path.
                let Some(safe_file) = crate::utils::sanitize_archive_entry_name(file) else {
                    continue;
                };
                if safe_file.to_ascii_lowercase() == target_normalized {
                    return resolve_allowlisted_asset_path(dir, &safe_file);
                }
            }
        }
        for target in &targets {
            let target_base = Path::new(target)
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_ascii_lowercase())?;
            for file in files {
                let Some(safe_file) = crate::utils::sanitize_archive_entry_name(file) else {
                    continue;
                };
                let base = Path::new(&safe_file)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.to_ascii_lowercase())
                    .unwrap_or_default();
                if base == target_base {
                    return resolve_allowlisted_asset_path(dir, &safe_file);
                }
            }
        }
        None
    }
    pub(crate) fn find_preview(&self, beatmap: &Beatmap) -> u32 {
        if beatmap.metadata.preview_time > 0 {
            return beatmap.metadata.preview_time as u32;
        }
        beatmap
            .hit_objects
            .first()
            .map(|h| (h.time as f32 * 0.33) as u32)
            .unwrap_or(0)
    }
    pub(crate) fn find_logo_path(&self) -> Option<PathBuf> {
        let candidates = ["assets/logo1.png"];
        for c in &candidates {
            let p = PathBuf::from(c);
            if p.exists() {
                return Some(p);
            }
        }
        None
    }
    pub(crate) fn make_temp_overlay_path_in(&self, dir: &Path, stem: &str, ext: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        dir.join(format!("miru_{stem}_{nanos}.{ext}"))
    }
    pub(crate) fn compute_plan(
        &self,
        beatmap: &Beatmap,
        opts: &ResolveOpts,
        playback_profile: crate::utils::mods::PlaybackRateProfile,
    ) -> Result<RenderPlan, ConvertError> {
        let last = beatmap.hit_objects.last();
        let end = last
            .map(|object| i64::from(object.end_time.unwrap_or(object.time)))
            .unwrap_or(60_000);
        let start_ms = opts
            .start_seconds
            .map(|seconds| seconds_to_millis(seconds, "start_seconds"))
            .transpose()?
            .unwrap_or(-i64::from(self.settings.lead_in_ms));
        let end_ms = opts
            .end_seconds
            .map(|seconds| seconds_to_millis(seconds, "end_seconds"))
            .transpose()?
            .unwrap_or(end)
            .max(start_ms + 1000_i64);
        let timeline_start = i32::try_from(start_ms).map_err(|_| {
            ConvertError::Resolve(format!(
                "render timeline start is out of i32 range: {start_ms} ms"
            ))
        })?;
        let timeline_end = i32::try_from(end_ms).map_err(|_| {
            ConvertError::Resolve(format!(
                "render timeline end is out of i32 range: {end_ms} ms"
            ))
        })?;
        let playback_clock = PlaybackClock::new(timeline_start, playback_profile.clone());
        // Rate-changing mods alter output duration, so frame count uses playback-clock time.
        let output_duration_ms = playback_clock
            .output_elapsed_ms_for_beatmap_time(end_ms as f64)
            .max(1000.0);
        let frames = (output_duration_ms / 1000.0 * self.settings.fps as f64).ceil() as u64;
        Ok(RenderPlan {
            timeline_start,
            timeline_end,
            playback_profile,
            total_frames: frames,
        })
    }
    pub(crate) fn estimate_frames(
        &self,
        beatmap: &Beatmap,
        opts: &ResolveOpts,
        playback_profile: crate::utils::mods::PlaybackRateProfile,
    ) -> Result<u64, ConvertError> {
        self.compute_plan(beatmap, opts, playback_profile)
            .map(|plan| plan.total_frames)
    }
    pub(crate) fn build_compose_opts(
        &self,
        output: &Path,
        audio: Option<&Path>,
        timeline_start: i32,
        timeline_end: i32,
        preview_ms: i32,
        bg: Option<&BackgroundSource>,
        playback: PlaybackModSettings,
        audio_playback_profile: Option<&crate::utils::mods::PlaybackRateProfile>,
        fail_time_ms: Option<i32>,
        main_audio_gain: Option<&Path>,
        music_overlays: &[PathBuf],
        effects_overlays: &[PathBuf],
    ) -> ComposeOpts {
        let background = bg.map(|b| {
            let dim = if b.dim.is_finite() {
                b.dim.clamp(0.0, 1.0)
            } else {
                1.0
            };
            BackgroundInput {
                kind: match b.kind {
                    BackgroundKind::Image => VideoBackgroundKind::Image,
                    BackgroundKind::Video => VideoBackgroundKind::Video,
                },
                path: b.path.to_string_lossy().into_owned(),
                start_time_ms: b.start_time_ms,
                dim,
            }
        });
        ComposeOpts {
            width: self.settings.width,
            height: self.settings.height,
            fps: self.settings.fps,
            audio_path: audio.map(|p| p.to_string_lossy().into_owned()),
            output_path: output.to_string_lossy().into_owned(),
            preset: self.settings.preset.clone(),
            crf: self.settings.crf,
            motion_blur_percent: self.settings.motion_blur_percent,
            timeline_start_ms: timeline_start,
            timeline_end_ms: timeline_end,
            intro_duration_ms: if self.settings.intro_enabled {
                INTRO_DURATION_MS
            } else {
                0
            },
            preview_time_ms: preview_ms as u32,
            background,
            background_blur_percent: self.settings.background_blur_percent.unwrap_or(0),
            bg_compose_mode: self.settings.bg_compose_mode,
            encoder: self.settings.encoder,
            ffmpeg_threads: self.settings.ffmpeg_threads,
            playback_profile: playback.profile.clone(),
            audio_playback_profile: audio_playback_profile.cloned(),
            audio_mode: playback.audio_mode,
            audio_frequency_rate: playback.audio_frequency_rate,
            audio_tempo_rate: playback.audio_tempo_rate,
            fail_time_ms,
            music_volume_percent: self.settings.music_volume_percent,
            hitsound_volume_percent: self.settings.hitsound_volume_percent,
            main_audio_gain_path: main_audio_gain.map(|path| path.to_string_lossy().into_owned()),
            music_overlay_paths: music_overlays
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
            effects_overlay_paths: effects_overlays
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
        }
    }
}
fn asset_lookup_candidates(filename: &str) -> Option<Vec<String>> {
    let target = crate::utils::sanitize_archive_entry_name(filename)?;
    let mut candidates = vec![target.clone()];
    if Path::new(&target).extension().is_none() {
        // Storyboards and HUD configs may omit common image extensions.
        for ext in ["png", "jpg", "jpeg", "bmp", "gif", "webp"] {
            candidates.push(format!("{target}.{ext}"));
        }
    }
    Some(candidates)
}
impl ManiaVideoConverter {
    pub(crate) fn normalized_autoplay_mods(
        &self,
        opts: &ResolveOpts,
    ) -> Result<Option<crate::utils::NormalizedAutoplayMods>, ConvertError> {
        opts.autoplay_mods
            .as_ref()
            .map(|config| {
                crate::utils::normalize_autoplay_mods_config(config).map_err(ConvertError::Parse)
            })
            .transpose()
    }
    pub(crate) fn build_autoplay_replay_data(
        &self,
        autoplay_mods: Option<&crate::utils::NormalizedAutoplayMods>,
        key_actions: Vec<KeyAction>,
    ) -> ManiaReplayData {
        let default_mods = 1 << 11;
        let (mods, origin, mod_info) = if let Some(autoplay_mods) = autoplay_mods {
            (
                autoplay_mods.legacy_bits,
                autoplay_mods.origin,
                autoplay_mods.replay_mod_info(),
            )
        } else {
            (
                default_mods,
                crate::types::ReplayOrigin::StableLegacy,
                ReplayModInfo {
                    legacy_bits: default_mods,
                    display_mods: Some(vec!["AT".to_string()]),
                    ..Default::default()
                },
            )
        };
        ManiaReplayData {
            replay: ReplayData {
                game_mode: 3,
                player_name: "Miru".into(),
                mods,
                origin,
                mod_info,
                ..Default::default()
            },
            frames: Vec::new(),
            key_actions,
            beatmap_file: None,
        }
    }
    pub(crate) fn ensure_rd_not_enabled(&self, replay: &ReplayData) -> Result<(), ConvertError> {
        let has_rd = replay.mods & (1 << 21) != 0 || self.replay_has_api_mod(replay, "RD");
        if has_rd {
            return Err(ConvertError::Parse("Mod RD aún no soportado".to_string()));
        }
        Ok(())
    }
    #[inline]
    pub(crate) fn replay_has_api_mod(&self, replay: &ReplayData, acronym: &str) -> bool {
        replay
            .mod_info
            .api_mods
            .iter()
            .any(|mod_entry| mod_entry.acronym.eq_ignore_ascii_case(acronym))
    }
    #[inline]
    pub(crate) fn replay_api_mod_entry<'a>(
        &self,
        replay: &'a ReplayData,
        acronym: &str,
    ) -> Option<&'a crate::types::ApiModEntry> {
        replay
            .mod_info
            .api_mods
            .iter()
            .find(|mod_entry| mod_entry.acronym.eq_ignore_ascii_case(acronym))
    }
    fn parse_cover_direction(&self, raw: &serde_json::Value) -> Option<PlayfieldCoverDirection> {
        match raw {
            serde_json::Value::String(value) => {
                let canonical = value
                    .chars()
                    .filter(|ch| ch.is_ascii_alphanumeric())
                    .flat_map(|ch| ch.to_lowercase())
                    .collect::<String>();
                match canonical.as_str() {
                    "alongscroll" | "downwards" => Some(PlayfieldCoverDirection::Downwards),
                    "againstscroll" | "upwards" => Some(PlayfieldCoverDirection::Upwards),
                    _ => None,
                }
            }
            serde_json::Value::Number(value) => match value.as_i64() {
                Some(0) => Some(PlayfieldCoverDirection::Downwards),
                Some(1) => Some(PlayfieldCoverDirection::Upwards),
                _ => None,
            },
            _ => None,
        }
    }
    fn parse_cover_coverage_ratio(&self, raw: &serde_json::Value) -> Option<f32> {
        let parsed = match raw {
            serde_json::Value::Number(value) => value.as_f64().map(|value| value as f32),
            serde_json::Value::String(value) => value.parse::<f32>().ok(),
            _ => None,
        }?;
        Some(parsed.clamp(0.2, 0.8))
    }
    fn parse_flashlight_size_multiplier(&self, raw: &serde_json::Value) -> Option<f32> {
        let parsed = match raw {
            serde_json::Value::Number(value) => value.as_f64().map(|value| value as f32),
            serde_json::Value::String(value) => value.parse::<f32>().ok(),
            _ => None,
        }?;
        parsed.is_finite().then_some(parsed.clamp(0.5, 3.0))
    }
    fn parse_positive_f64(&self, raw: &serde_json::Value) -> Option<f64> {
        let parsed = match raw {
            serde_json::Value::Number(value) => value.as_f64(),
            serde_json::Value::String(value) => value.parse::<f64>().ok(),
            _ => None,
        }?;
        (parsed.is_finite() && parsed > 0.0).then_some(parsed)
    }
    fn parse_bool_like(&self, raw: &serde_json::Value) -> Option<bool> {
        match raw {
            serde_json::Value::Bool(value) => Some(*value),
            serde_json::Value::Number(value) => value.as_i64().and_then(|value| match value {
                0 => Some(false),
                1 => Some(true),
                _ => None,
            }),
            serde_json::Value::String(value) => {
                let normalized = value.trim().to_ascii_lowercase();
                match normalized.as_str() {
                    "true" | "1" => Some(true),
                    "false" | "0" => Some(false),
                    _ => None,
                }
            }
            _ => None,
        }
    }
    fn parse_u32_like(&self, raw: &serde_json::Value) -> Option<u32> {
        match raw {
            serde_json::Value::Number(value) => value
                .as_u64()
                .and_then(|value| (value <= u32::MAX as u64).then_some(value as u32))
                .or_else(|| {
                    value.as_f64().and_then(|value| {
                        (value.is_finite()
                            && value >= 0.0
                            && value <= u32::MAX as f64
                            && (value.fract().abs() < f64::EPSILON))
                            .then_some(value as u32)
                    })
                }),
            serde_json::Value::String(value) => value.parse::<u32>().ok().or_else(|| {
                value.parse::<f64>().ok().and_then(|value| {
                    (value.is_finite()
                        && value >= 0.0
                        && value <= u32::MAX as f64
                        && (value.fract().abs() < f64::EPSILON))
                        .then_some(value as u32)
                })
            }),
            _ => None,
        }
    }
    fn replay_api_setting<T, F>(
        &self,
        replay: &ReplayData,
        acronym: &str,
        keys: &[&str],
        parse: F,
    ) -> Option<T>
    where
        F: Fn(&serde_json::Value) -> Option<T>,
    {
        let mod_entry = self.replay_api_mod_entry(replay, acronym)?;
        // Lazer exports have used snake_case, camelCase, and PascalCase setting names.
        keys.iter()
            .find_map(|key| mod_entry.settings.get(*key).and_then(&parse))
    }
    fn resolve_intro_summary_priority(&self, acronym: &str) -> u16 {
        match acronym.trim().to_ascii_uppercase().as_str() {
            "DT" => 200,
            "NC" => 190,
            "HT" => 180,
            "DC" => 170,
            "AS" => 160,
            "WU" => 150,
            "WD" => 140,
            "FL" => 70,
            "CO" => 65,
            "MU" => 35,
            "AC" => 30,
            _ => 0,
        }
    }
    fn resolve_intro_mod_summary(&self, replay: &ReplayData, acronym: &str) -> Option<String> {
        let normalized = acronym.trim().to_ascii_uppercase();
        match normalized.as_str() {
            "DT" => {
                let speed_change = self
                    .replay_api_setting(
                        replay,
                        "DT",
                        &["speed_change", "speedChange", "SpeedChange"],
                        |raw| self.parse_positive_f64(raw),
                    )
                    .unwrap_or(1.5)
                    .clamp(1.01, 2.0);
                let adjust_pitch = self
                    .replay_api_setting(
                        replay,
                        "DT",
                        &["adjust_pitch", "adjustPitch", "AdjustPitch"],
                        |raw| self.parse_bool_like(raw),
                    )
                    .unwrap_or(false);
                if !approx_eq_f64(speed_change, 1.5) {
                    Some(format_rate_summary(speed_change))
                } else if adjust_pitch {
                    Some("Pitch".to_string())
                } else {
                    None
                }
            }
            "NC" => {
                let speed_change = self
                    .replay_api_setting(
                        replay,
                        "NC",
                        &["speed_change", "speedChange", "SpeedChange"],
                        |raw| self.parse_positive_f64(raw),
                    )
                    .unwrap_or(1.5)
                    .clamp(1.01, 2.0);
                (!approx_eq_f64(speed_change, 1.5)).then(|| format_rate_summary(speed_change))
            }
            "HT" => {
                let speed_change = self
                    .replay_api_setting(
                        replay,
                        "HT",
                        &["speed_change", "speedChange", "SpeedChange"],
                        |raw| self.parse_positive_f64(raw),
                    )
                    .unwrap_or(0.75)
                    .clamp(0.5, 0.99);
                let adjust_pitch = self
                    .replay_api_setting(
                        replay,
                        "HT",
                        &["adjust_pitch", "adjustPitch", "AdjustPitch"],
                        |raw| self.parse_bool_like(raw),
                    )
                    .unwrap_or(false);
                if !approx_eq_f64(speed_change, 0.75) {
                    Some(format_rate_summary(speed_change))
                } else if adjust_pitch {
                    Some("Pitch".to_string())
                } else {
                    None
                }
            }
            "DC" => {
                let speed_change = self
                    .replay_api_setting(
                        replay,
                        "DC",
                        &["speed_change", "speedChange", "SpeedChange"],
                        |raw| self.parse_positive_f64(raw),
                    )
                    .unwrap_or(0.75)
                    .clamp(0.5, 0.99);
                (!approx_eq_f64(speed_change, 0.75)).then(|| format_rate_summary(speed_change))
            }
            "AS" => {
                let initial_rate = self
                    .replay_api_setting(
                        replay,
                        "AS",
                        &["initial_rate", "initialRate", "InitialRate"],
                        |raw| self.parse_positive_f64(raw),
                    )
                    .unwrap_or(1.0)
                    .clamp(0.5, 2.0);
                let adjust_pitch = self
                    .replay_api_setting(
                        replay,
                        "AS",
                        &["adjust_pitch", "adjustPitch", "AdjustPitch"],
                        |raw| self.parse_bool_like(raw),
                    )
                    .unwrap_or(true);
                if !approx_eq_f64(initial_rate, 1.0) {
                    Some(format_rate_summary(initial_rate))
                } else if !adjust_pitch {
                    Some("No Pitch".to_string())
                } else {
                    None
                }
            }
            "WU" => {
                let initial_rate = self
                    .replay_api_setting(
                        replay,
                        "WU",
                        &["initial_rate", "initialRate", "InitialRate"],
                        |raw| self.parse_positive_f64(raw),
                    )
                    .unwrap_or(1.0)
                    .clamp(0.5, 2.0);
                let final_rate = self
                    .replay_api_setting(
                        replay,
                        "WU",
                        &["final_rate", "finalRate", "FinalRate"],
                        |raw| self.parse_positive_f64(raw),
                    )
                    .unwrap_or(1.5)
                    .clamp(0.5, 2.0);
                let adjust_pitch = self
                    .replay_api_setting(
                        replay,
                        "WU",
                        &["adjust_pitch", "adjustPitch", "AdjustPitch"],
                        |raw| self.parse_bool_like(raw),
                    )
                    .unwrap_or(true);
                if !approx_eq_f64(initial_rate, 1.0) || !approx_eq_f64(final_rate, 1.5) {
                    Some(format_rate_ramp_summary(initial_rate, final_rate))
                } else if !adjust_pitch {
                    Some("No Pitch".to_string())
                } else {
                    None
                }
            }
            "WD" => {
                let initial_rate = self
                    .replay_api_setting(
                        replay,
                        "WD",
                        &["initial_rate", "initialRate", "InitialRate"],
                        |raw| self.parse_positive_f64(raw),
                    )
                    .unwrap_or(1.0)
                    .clamp(0.5, 2.0);
                let final_rate = self
                    .replay_api_setting(
                        replay,
                        "WD",
                        &["final_rate", "finalRate", "FinalRate"],
                        |raw| self.parse_positive_f64(raw),
                    )
                    .unwrap_or(0.75)
                    .clamp(0.5, 2.0);
                let adjust_pitch = self
                    .replay_api_setting(
                        replay,
                        "WD",
                        &["adjust_pitch", "adjustPitch", "AdjustPitch"],
                        |raw| self.parse_bool_like(raw),
                    )
                    .unwrap_or(true);
                if !approx_eq_f64(initial_rate, 1.0) || !approx_eq_f64(final_rate, 0.75) {
                    Some(format_rate_ramp_summary(initial_rate, final_rate))
                } else if !adjust_pitch {
                    Some("No Pitch".to_string())
                } else {
                    None
                }
            }
            "FL" => self
                .resolve_playfield_cover_config(replay)
                .ok()
                .and_then(|config| {
                    (config.mode == PlayfieldCoverMode::Flashlight).then_some(config)
                })
                .and_then(|config| {
                    if !approx_eq_f32(config.flashlight_size_multiplier, 1.0) {
                        Some(format_rate_summary(f64::from(
                            config.flashlight_size_multiplier,
                        )))
                    } else if config.flashlight_combo_based_size {
                        Some("Combo".to_string())
                    } else {
                        None
                    }
                }),
            "CO" => self
                .resolve_playfield_cover_config(replay)
                .ok()
                .and_then(|config| (config.mode == PlayfieldCoverMode::Cover).then_some(config))
                .and_then(|config| {
                    if !approx_eq_f32(config.coverage_ratio, 0.5) {
                        Some(format_percentage_summary(f64::from(config.coverage_ratio)))
                    } else if config.direction != PlayfieldCoverDirection::Downwards {
                        Some(
                            match config.direction {
                                PlayfieldCoverDirection::Downwards => "Down",
                                PlayfieldCoverDirection::Upwards => "Up",
                            }
                            .to_string(),
                        )
                    } else {
                        None
                    }
                }),
            "MU" => self.resolve_replay_muted_config(replay).and_then(|config| {
                if config.mute_combo_count != 100 {
                    Some(config.mute_combo_count.to_string())
                } else if config.inverse_muting {
                    Some("Inv".to_string())
                } else if !config.enable_metronome {
                    Some("Metro Off".to_string())
                } else if !config.affects_hit_sounds {
                    Some("Hits Off".to_string())
                } else {
                    None
                }
            }),
            "AC" => self
                .resolve_replay_fail_condition_mod(replay)
                .ok()
                .and_then(|conditions| conditions.accuracy_challenge)
                .and_then(|config| {
                    if !approx_eq_f64(config.minimum_accuracy, 0.90) {
                        Some(format_percentage_summary(config.minimum_accuracy))
                    } else if config.mode != ReplayAccuracyChallengeMode::MaximumAchievable {
                        Some("Std".to_string())
                    } else {
                        None
                    }
                }),
            _ => None,
        }
    }
    fn parse_require_perfect_hits(&self, replay: &ReplayData) -> bool {
        let Some(mod_entry) = self.replay_api_mod_entry(replay, "PF") else {
            return false;
        };
        [
            "RequirePerfectHits",
            "require_perfect_hits",
            "requirePerfectHits",
        ]
        .iter()
        .find_map(|key| {
            mod_entry
                .settings
                .get(*key)
                .and_then(|raw| self.parse_bool_like(raw))
        })
        .unwrap_or(false)
    }
    fn parse_minimum_accuracy(&self, raw: &serde_json::Value) -> Option<f64> {
        let parsed = match raw {
            serde_json::Value::Number(value) => value.as_f64(),
            serde_json::Value::String(value) => value.parse::<f64>().ok(),
            _ => None,
        }?;
        parsed.is_finite().then_some(parsed.clamp(0.60, 0.999))
    }
    fn parse_accuracy_challenge_mode(
        &self,
        raw: &serde_json::Value,
    ) -> Option<ReplayAccuracyChallengeMode> {
        match raw {
            serde_json::Value::Number(value) => match value.as_i64() {
                Some(0) => Some(ReplayAccuracyChallengeMode::MaximumAchievable),
                Some(1) => Some(ReplayAccuracyChallengeMode::Standard),
                _ => None,
            },
            serde_json::Value::String(value) => {
                let normalized = value
                    .chars()
                    .filter(|ch| ch.is_ascii_alphanumeric())
                    .flat_map(|ch| ch.to_lowercase())
                    .collect::<String>();
                match normalized.as_str() {
                    "0" | "maximumachievable" => {
                        Some(ReplayAccuracyChallengeMode::MaximumAchievable)
                    }
                    "1" | "standard" => Some(ReplayAccuracyChallengeMode::Standard),
                    _ => None,
                }
            }
            _ => None,
        }
    }
    pub(crate) fn resolve_intro_display_mods(&self, replay: &ReplayData) -> Vec<String> {
        if let Some(display_mods) = replay.mod_info.display_mods.as_ref() {
            return self.normalize_visible_replay_display_mods(replay, display_mods.clone());
        }
        let mut mods = crate::intro::display_mods(replay.mods)
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        if replay.origin == crate::types::ReplayOrigin::LazerExport {
            // Legacy bitfields miss API-only lazer mods, so merge them for intro badges.
            for api_only_mod in [
                "AC", "AS", "CL", "CO", "CS", "DC", "HO", "IN", "LZ", "MU", "NR", "SV2", "WU", "WD",
            ] {
                if self.replay_has_api_mod(replay, api_only_mod)
                    && !mods.iter().any(|mod_abbr| mod_abbr == api_only_mod)
                {
                    mods.push(api_only_mod.to_string());
                }
            }
        }
        self.normalize_visible_replay_display_mods(replay, mods)
    }
    fn normalize_visible_replay_display_mods<I>(&self, replay: &ReplayData, mods: I) -> Vec<String>
    where
        I: IntoIterator<Item = String>,
    {
        let mut out = Vec::new();
        let has_autoplay = replay.mods & (1 << 11) != 0 || self.replay_has_api_mod(replay, "AT");
        if has_autoplay {
            out.push("AT".to_string());
        }
        for mod_abbr in mods {
            let normalized = mod_abbr.trim().to_ascii_uppercase();
            if normalized.is_empty() || normalized == "SV1" {
                continue;
            }
            if !out.iter().any(|existing| existing == &normalized) {
                out.push(normalized);
            }
        }
        out
    }
    pub(crate) fn resolve_intro_mod_badges(&self, replay: &ReplayData) -> Vec<IntroModBadgeSpec> {
        self.resolve_intro_display_mods(replay)
            .into_iter()
            .map(|acronym| IntroModBadgeSpec {
                summary: self.resolve_intro_mod_summary(replay, &acronym),
                summary_priority: self.resolve_intro_summary_priority(&acronym),
                acronym,
            })
            .collect()
    }
    #[inline]
    pub(crate) fn resolve_replay_beatmap_conversion_mods(
        &self,
        replay: &ReplayData,
    ) -> Result<ReplayBeatmapConversionMods, ConvertError> {
        // These lazer mods change the playable beatmap before judgment calculation.
        let invert = replay.origin == crate::types::ReplayOrigin::LazerExport
            && self.replay_has_api_mod(replay, "IN");
        let hold_off = replay.origin == crate::types::ReplayOrigin::LazerExport
            && self.replay_has_api_mod(replay, "HO");
        let no_release = replay.origin == crate::types::ReplayOrigin::LazerExport
            && self.replay_has_api_mod(replay, "NR");
        if (invert && hold_off) || (hold_off && no_release) {
            return Err(ConvertError::Parse(
                "Mods de replay incompatibles".to_string(),
            ));
        }
        Ok(ReplayBeatmapConversionMods {
            mirror: crate::utils::mods::has_mirror_mod(replay.mods),
            invert,
            hold_off,
        })
    }
    #[inline]
    pub(crate) fn resolve_replay_scroll_visualisation_mods(
        &self,
        replay: &ReplayData,
    ) -> ReplayScrollVisualisationMods {
        ReplayScrollVisualisationMods {
            constant_speed: replay.origin == crate::types::ReplayOrigin::LazerExport
                && self.replay_has_api_mod(replay, "CS"),
        }
    }
    #[inline]
    pub(crate) fn resolve_replay_muted_config(
        &self,
        replay: &ReplayData,
    ) -> Option<ReplayMutedConfig> {
        if replay.origin != crate::types::ReplayOrigin::LazerExport {
            return None;
        }
        let mod_entry = self.replay_api_mod_entry(replay, "MU")?;
        let inverse_muting = ["inverse_muting", "inverseMuting", "InverseMuting"]
            .iter()
            .find_map(|key| {
                mod_entry
                    .settings
                    .get(*key)
                    .and_then(|raw| self.parse_bool_like(raw))
            })
            .unwrap_or(false);
        let enable_metronome = ["enable_metronome", "enableMetronome", "EnableMetronome"]
            .iter()
            .find_map(|key| {
                mod_entry
                    .settings
                    .get(*key)
                    .and_then(|raw| self.parse_bool_like(raw))
            })
            .unwrap_or(true);
        let affects_hit_sounds = ["affects_hit_sounds", "affectsHitSounds", "AffectsHitSounds"]
            .iter()
            .find_map(|key| {
                mod_entry
                    .settings
                    .get(*key)
                    .and_then(|raw| self.parse_bool_like(raw))
            })
            .unwrap_or(true);
        let mut mute_combo_count = ["mute_combo_count", "muteComboCount", "MuteComboCount"]
            .iter()
            .find_map(|key| {
                mod_entry
                    .settings
                    .get(*key)
                    .and_then(|raw| self.parse_u32_like(raw))
            })
            .unwrap_or(100)
            .clamp(0, 500);
        if inverse_muting {
            mute_combo_count = mute_combo_count.max(1);
        }
        Some(ReplayMutedConfig {
            inverse_muting,
            enable_metronome,
            mute_combo_count,
            affects_hit_sounds,
        })
    }
    pub(crate) fn resolve_replay_scroll_model(
        &self,
        replay: &ReplayData,
        timing_points: &[TimingPoint],
        last_object_time_ms: Option<i32>,
        playback_clock: &crate::video::playback::PlaybackClock,
        pps_base: f64,
    ) -> Option<timing::StableScrollModel> {
        let scroll_visualisation_mods = self.resolve_replay_scroll_visualisation_mods(replay);
        if !self.settings.sv_enabled || scroll_visualisation_mods.constant_speed {
            return None;
        }
        // Rate-changing mods move visual time; remap timing points into output time for SV.
        let remapped_timing_points = timing_points
            .iter()
            .map(|point| {
                let mut remapped = *point;
                remapped.time = playback_clock.output_elapsed_ms_for_beatmap_time(point.time);
                remapped
            })
            .collect::<Vec<_>>();
        let remapped_last_object_time_ms = last_object_time_ms.map(|time_ms| {
            playback_clock
                .output_elapsed_ms_for_beatmap_time(time_ms as f64)
                .round() as i32
        });
        Some(timing::build_stable_scroll_model(
            &remapped_timing_points,
            remapped_last_object_time_ms,
            pps_base,
        ))
    }
    pub(crate) fn apply_replay_beatmap_conversion_mods(
        &self,
        beatmap: &mut Beatmap,
        replay: &ReplayData,
    ) -> Result<(), ConvertError> {
        let mods = self.resolve_replay_beatmap_conversion_mods(replay)?;
        if mods.is_empty() {
            return Ok(());
        }
        let key_count = beatmap.key_count();
        if mods.mirror {
            for hit_object in &mut beatmap.hit_objects {
                hit_object.column = crate::utils::mods::mirror_column(hit_object.column, key_count);
            }
            println!("   [mod] mirror applied");
        }
        if mods.invert {
            apply_invert_mod_to_beatmap(beatmap);
            println!("   [mod] invert applied");
        }
        if mods.hold_off {
            apply_hold_off_mod_to_beatmap(beatmap);
            println!("   [mod] hold off applied");
        }
        Ok(())
    }
    #[inline]
    pub(crate) fn resolve_replay_playback_settings(
        &self,
        replay: &ReplayData,
        beatmap: &Beatmap,
    ) -> Result<PlaybackModSettings, ConvertError> {
        if replay.origin == crate::types::ReplayOrigin::LazerExport {
            if self.replay_has_api_mod(replay, "AS") {
                return self.resolve_adaptive_speed_playback_settings(replay);
            }
            let has_wind_up = self.replay_has_api_mod(replay, "WU");
            let has_wind_down = self.replay_has_api_mod(replay, "WD");
            if has_wind_up && has_wind_down {
                return Err(ConvertError::Parse(
                    "Mods de replay incompatibles".to_string(),
                ));
            }
            if has_wind_up {
                return self.resolve_time_ramp_playback_settings(replay, beatmap, "WU", 1.0, 1.5);
            }
            if has_wind_down {
                return self.resolve_time_ramp_playback_settings(replay, beatmap, "WD", 1.0, 0.75);
            }
        }
        if let Some(settings) =
            resolve_lazer_playback_mod_settings(replay).map_err(ConvertError::Parse)?
        {
            return Ok(settings);
        }
        Ok(resolve_playback_mod_settings(replay.mods))
    }
    fn resolve_adaptive_speed_playback_settings(
        &self,
        replay: &ReplayData,
    ) -> Result<PlaybackModSettings, ConvertError> {
        let static_rate_mod_active = resolve_lazer_playback_mod_settings(replay)
            .map_err(ConvertError::Parse)?
            .is_some()
            || resolve_playback_mod_settings(replay.mods).clock_rate != 1.0;
        let conflicting_fun_mod = self.replay_has_api_mod(replay, "WU")
            || self.replay_has_api_mod(replay, "WD")
            || self.replay_has_api_mod(replay, "AT");
        if static_rate_mod_active || conflicting_fun_mod {
            return Err(ConvertError::Parse(
                "Mods de replay incompatibles".to_string(),
            ));
        }
        let mod_entry = self
            .replay_api_mod_entry(replay, "AS")
            .ok_or_else(|| ConvertError::Parse("AS no encontrado en replay".to_string()))?;
        let initial_rate = ["initial_rate", "initialRate", "InitialRate"]
            .iter()
            .find_map(|key| {
                mod_entry
                    .settings
                    .get(*key)
                    .and_then(|raw| self.parse_positive_f64(raw))
            })
            .unwrap_or(1.0)
            .clamp(0.5, 2.0);
        let adjust_pitch = ["adjust_pitch", "adjustPitch", "AdjustPitch"]
            .iter()
            .find_map(|key| {
                mod_entry
                    .settings
                    .get(*key)
                    .and_then(|raw| self.parse_bool_like(raw))
            })
            .unwrap_or(true);
        eprintln!(
            "   [mod] adaptive speed resolved: initial_rate={:.3} adjust_pitch={}",
            initial_rate, adjust_pitch
        );
        Ok(PlaybackModSettings {
            clock_rate: initial_rate,
            profile: PlaybackRateProfile::constant(initial_rate),
            audio_mode: PlaybackAudioMode::RateDriven { adjust_pitch },
            audio_frequency_rate: 1.0,
            audio_tempo_rate: 1.0,
            nightcore: false,
            adaptive_speed: Some(AdaptivePlaybackConfig {
                initial_rate,
                adjust_pitch,
            }),
        })
    }
    fn resolve_time_ramp_playback_settings(
        &self,
        replay: &ReplayData,
        beatmap: &Beatmap,
        acronym: &str,
        default_initial_rate: f64,
        default_final_rate: f64,
    ) -> Result<PlaybackModSettings, ConvertError> {
        let static_rate_mod_active = resolve_lazer_playback_mod_settings(replay)
            .map_err(ConvertError::Parse)?
            .is_some()
            || resolve_playback_mod_settings(replay.mods).clock_rate != 1.0;
        let conflicting_fun_mod = self.replay_has_api_mod(replay, "AS")
            || match acronym {
                "WU" => self.replay_has_api_mod(replay, "WD"),
                "WD" => self.replay_has_api_mod(replay, "WU"),
                _ => false,
            };
        if static_rate_mod_active || conflicting_fun_mod {
            return Err(ConvertError::Parse(
                "Mods de replay incompatibles".to_string(),
            ));
        }
        let mod_entry = self
            .replay_api_mod_entry(replay, acronym)
            .ok_or_else(|| ConvertError::Parse(format!("{acronym} no encontrado en replay")))?;
        let initial_rate = ["initial_rate", "initialRate", "InitialRate"]
            .iter()
            .find_map(|key| {
                mod_entry
                    .settings
                    .get(*key)
                    .and_then(|raw| self.parse_positive_f64(raw))
            })
            .unwrap_or(default_initial_rate);
        let final_rate = ["final_rate", "finalRate", "FinalRate"]
            .iter()
            .find_map(|key| {
                mod_entry
                    .settings
                    .get(*key)
                    .and_then(|raw| self.parse_positive_f64(raw))
            })
            .unwrap_or(default_final_rate);
        let adjust_pitch = ["adjust_pitch", "adjustPitch", "AdjustPitch"]
            .iter()
            .find_map(|key| {
                mod_entry
                    .settings
                    .get(*key)
                    .and_then(|raw| self.parse_bool_like(raw))
            })
            .unwrap_or(true);
        let first_object_start = beatmap
            .hit_objects
            .first()
            .map(|hit_object| hit_object.time);
        let last_object_end = beatmap
            .hit_objects
            .iter()
            .map(|hit_object| hit_object.end_time.unwrap_or(hit_object.time))
            .max();
        let (Some(begin_ms), Some(last_object_end)) = (first_object_start, last_object_end) else {
            return Ok(PlaybackModSettings::normal());
        };
        let end_ms =
            begin_ms + (((last_object_end - begin_ms).max(0) as f64) * 0.75).round() as i32;
        Ok(PlaybackModSettings {
            clock_rate: initial_rate,
            profile: PlaybackRateProfile::LinearRamp {
                initial_rate,
                final_rate,
                begin_ms,
                end_ms: end_ms.max(begin_ms + 1),
            },
            audio_mode: PlaybackAudioMode::RateDriven { adjust_pitch },
            audio_frequency_rate: 1.0,
            audio_tempo_rate: 1.0,
            nightcore: false,
            adaptive_speed: None,
        })
    }
    #[inline]
    pub(crate) fn resolve_playfield_cover_config(
        &self,
        replay: &ReplayData,
    ) -> Result<PlayfieldCoverConfig, ConvertError> {
        let use_api_visual_mods = replay.origin == crate::types::ReplayOrigin::LazerExport
            && !replay.mod_info.api_mods.is_empty();
        let has_fi = self.replay_has_api_mod(replay, "FI")
            || (!use_api_visual_mods && has_fade_in_mod(replay.mods));
        let has_hd = self.replay_has_api_mod(replay, "HD")
            || (!use_api_visual_mods && has_hidden_mod(replay.mods));
        let has_fl = self.replay_has_api_mod(replay, "FL")
            || (!use_api_visual_mods && has_flashlight_mod(replay.mods));
        let cover_mod = self.replay_api_mod_entry(replay, "CO");
        let has_cover = cover_mod.is_some();
        let active_visual_mods = [has_fi, has_hd, has_fl, has_cover]
            .into_iter()
            .filter(|v| *v)
            .count();
        if active_visual_mods > 1 {
            return Err(ConvertError::Parse(
                "Mods de replay incompatibles".to_string(),
            ));
        }
        Ok(if let Some(mod_entry) = cover_mod {
            let coverage_ratio = mod_entry
                .settings
                .get("coverage")
                .and_then(|raw| self.parse_cover_coverage_ratio(raw))
                .unwrap_or(0.5);
            let direction = mod_entry
                .settings
                .get("direction")
                .and_then(|raw| self.parse_cover_direction(raw))
                .unwrap_or(PlayfieldCoverDirection::Downwards);
            PlayfieldCoverConfig {
                mode: PlayfieldCoverMode::Cover,
                direction,
                coverage_ratio,
                ..Default::default()
            }
        } else if has_fi {
            PlayfieldCoverConfig {
                mode: PlayfieldCoverMode::FadeIn,
                direction: PlayfieldCoverDirection::Downwards,
                ..Default::default()
            }
        } else if has_hd {
            PlayfieldCoverConfig {
                mode: PlayfieldCoverMode::Hidden,
                direction: PlayfieldCoverDirection::Upwards,
                ..Default::default()
            }
        } else if has_fl {
            let flashlight_size_multiplier = self
                .replay_api_mod_entry(replay, "FL")
                .and_then(|mod_entry| {
                    ["size_multiplier", "sizeMultiplier", "SizeMultiplier"]
                        .iter()
                        .find_map(|key| {
                            mod_entry
                                .settings
                                .get(*key)
                                .and_then(|raw| self.parse_flashlight_size_multiplier(raw))
                        })
                })
                .unwrap_or(1.0);
            let flashlight_combo_based_size = self
                .replay_api_mod_entry(replay, "FL")
                .and_then(|mod_entry| {
                    ["combo_based_size", "comboBasedSize", "ComboBasedSize"]
                        .iter()
                        .find_map(|key| {
                            mod_entry
                                .settings
                                .get(*key)
                                .and_then(|raw| self.parse_bool_like(raw))
                        })
                })
                .unwrap_or(false);
            PlayfieldCoverConfig {
                mode: PlayfieldCoverMode::Flashlight,
                direction: PlayfieldCoverDirection::Downwards,
                flashlight_size_multiplier,
                flashlight_combo_based_size,
                ..Default::default()
            }
        } else {
            PlayfieldCoverConfig::default()
        })
    }
    #[inline]
    pub(crate) fn resolve_replay_fail_condition_mod(
        &self,
        replay: &ReplayData,
    ) -> Result<ReplayFailConditions, ConvertError> {
        if replay.origin != crate::types::ReplayOrigin::LazerExport {
            return Ok(ReplayFailConditions::default());
        }
        let has_nf = self.replay_has_api_mod(replay, "NF") || has_no_fail_mod(replay.mods);
        let has_pf = self.replay_has_api_mod(replay, "PF") || has_perfect_mod(replay.mods);
        let has_sd =
            !has_pf && (self.replay_has_api_mod(replay, "SD") || has_sudden_death_mod(replay.mods));
        let ac_entry = self.replay_api_mod_entry(replay, "AC");
        let has_ac = ac_entry.is_some();
        if has_nf && (has_sd || has_pf || has_ac) {
            return Err(ConvertError::Parse(
                "Mods de replay incompatibles".to_string(),
            ));
        }
        if has_pf && has_ac {
            return Err(ConvertError::Parse(
                "Mods de replay incompatibles".to_string(),
            ));
        }
        let accuracy_challenge = ac_entry.map(|mod_entry| ReplayAccuracyChallengeFailCondition {
            minimum_accuracy: mod_entry
                .settings
                .get("minimum_accuracy")
                .and_then(|raw| self.parse_minimum_accuracy(raw))
                .unwrap_or(0.90),
            mode: mod_entry
                .settings
                .get("accuracy_judge_mode")
                .and_then(|raw| self.parse_accuracy_challenge_mode(raw))
                .unwrap_or(ReplayAccuracyChallengeMode::MaximumAchievable),
        });
        Ok(ReplayFailConditions {
            sudden_death: has_sd,
            perfect: has_pf.then(|| ReplayPerfectFailCondition {
                require_perfect_hits: self.parse_require_perfect_hits(replay),
            }),
            accuracy_challenge,
        })
    }
}

fn approx_eq_f64(left: f64, right: f64) -> bool {
    (left - right).abs() <= 0.0001
}
fn approx_eq_f32(left: f32, right: f32) -> bool {
    (left - right).abs() <= 0.0001
}
fn format_rate_summary(rate: f64) -> String {
    format!("{rate:.2}x")
}
fn format_rate_ramp_summary(initial_rate: f64, final_rate: f64) -> String {
    format!("{initial_rate:.2}->{final_rate:.2}x")
}
fn format_percentage_summary(value: f64) -> String {
    format!("{}%", (value * 100.0).round() as i32)
}
fn apply_invert_mod_to_beatmap(beatmap: &mut Beatmap) {
    let key_count = beatmap.key_count();
    if key_count == 0 {
        beatmap.hit_objects.clear();
        beatmap.events.breaks.clear();
        return;
    }
    let mut by_column = vec![Vec::new(); usize::from(key_count)];
    for hit_object in &beatmap.hit_objects {
        let column = usize::from(hit_object.column);
        if column < by_column.len() {
            by_column[column].push(hit_object.clone());
        }
    }
    let mut inverted_objects = Vec::new();
    for (column_idx, column_objects) in by_column.iter_mut().enumerate() {
        column_objects.sort_by_key(|hit_object| hit_object.time);
        // Invert turns gaps between adjacent notes in a column into playable holds.
        for pair in column_objects.windows(2) {
            let current = &pair[0];
            let next = &pair[1];
            let delta = f64::from(next.time.saturating_sub(current.time));
            let beat_length = beat_length_at(&beatmap.timing_points, next.time);
            let duration = (delta / 2.0).max(delta - beat_length / 4.0);
            let rounded_duration = duration.round() as i32;
            let end_time = current
                .time
                .max(current.time.saturating_add(rounded_duration));
            let column = column_idx as u8;
            let mut hold = current.clone();
            hold.x = mania_column_center_x(column, key_count);
            hold.time = current.time;
            hold.obj_type = 128;
            hold.end_time = Some(end_time);
            hold.column = column;
            inverted_objects.push(hold);
        }
    }
    inverted_objects.sort_by_key(|hit_object| hit_object.time);
    beatmap.hit_objects = inverted_objects;
    beatmap.events.breaks.clear();
}
fn apply_hold_off_mod_to_beatmap(beatmap: &mut Beatmap) {
    let beat_length_before = most_common_beat_length_for_beatmap(beatmap);
    let mut converted_objects = Vec::with_capacity(beatmap.hit_objects.len());
    for hit_object in &beatmap.hit_objects {
        if hit_object.is_long_note() {
            let mut note = hit_object.clone();
            note.obj_type = 1;
            note.end_time = None;
            converted_objects.push(note);
        } else {
            converted_objects.push(hit_object.clone());
        }
    }
    converted_objects.sort_by_key(|hit_object| hit_object.time);
    beatmap.hit_objects = converted_objects;
    let beat_length_after = most_common_beat_length_for_beatmap(beatmap);
    // Removing holds can change the dominant BPM window; rescale inherited SV to compensate.
    rescale_inherited_timing_points_for_hold_off(beatmap, beat_length_before, beat_length_after);
}
fn beat_length_at(timing_points: &[TimingPoint], target_time: i32) -> f64 {
    let mut first_valid = None;
    let mut best = None;
    for timing_point in timing_points {
        if !timing_point.uninherited
            || !timing_point.beat_length.is_finite()
            || timing_point.beat_length <= 0.0
        {
            continue;
        }
        first_valid.get_or_insert(timing_point.beat_length);
        if timing_point.time <= f64::from(target_time) {
            best = Some(timing_point.beat_length);
        }
    }
    best.or(first_valid).unwrap_or(500.0)
}
fn most_common_beat_length_for_beatmap(beatmap: &Beatmap) -> f64 {
    let last_object_time_ms = beatmap
        .hit_objects
        .iter()
        .map(|hit_object| hit_object.end_time.unwrap_or(hit_object.time))
        .max();
    timing::most_common_beat_length(&beatmap.timing_points, last_object_time_ms)
}
fn rescale_inherited_timing_points_for_hold_off(
    beatmap: &mut Beatmap,
    beat_length_before: f64,
    beat_length_after: f64,
) {
    if !beat_length_before.is_finite()
        || !beat_length_after.is_finite()
        || beat_length_before <= 0.0
        || beat_length_after <= 0.0
        || (beat_length_before - beat_length_after).abs() <= f64::EPSILON
    {
        return;
    }
    let ratio = beat_length_before / beat_length_after;
    if !ratio.is_finite() || ratio <= 0.0 {
        return;
    }
    for timing_point in &mut beatmap.timing_points {
        if timing_point.uninherited || !timing_point.beat_length.is_finite() {
            continue;
        }
        timing_point.beat_length /= ratio;
    }
}
fn mania_column_center_x(column: u8, key_count: u8) -> i32 {
    (((f32::from(column) + 0.5) / f32::from(key_count)) * 512.0).round() as i32
}

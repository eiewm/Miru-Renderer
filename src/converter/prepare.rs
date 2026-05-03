use super::*;
use crate::renderer::ReplayModDisplay;
#[derive(Debug, Clone)]
pub(crate) struct ComboComputation {
    pub(crate) ln_ticks: Vec<crate::renderer::replay_renderer::LnComboTick>,
    pub(crate) ln_breaks: Vec<crate::renderer::replay_renderer::LnComboBreak>,
    pub(crate) score_events: Vec<crate::renderer::replay_renderer::ComboEvent>,
    pub(crate) computed_max_combo: u32,
    pub(crate) combo_before_breaks: Vec<u32>,
}
pub(crate) struct PreparedReplayRender {
    pub(crate) beatmap: Beatmap,
    pub(crate) cover_config: crate::renderer::PlayfieldCoverConfig,
    pub(crate) cover_metrics: crate::renderer::PlayfieldCoverMetrics,
    pub(crate) set_dir: PathBuf,
    pub(crate) set_files: Vec<String>,
    pub(crate) bg: Option<BackgroundSource>,
    pub(crate) results_background_path: Option<PathBuf>,
    pub(crate) skin: SkinAssets,
    pub(crate) replay_mod_display: ReplayModDisplay,
    pub(crate) compose_template: ComposeOpts,
    pub(crate) resolved_hud_config: Option<HudConfig>,
    pub(crate) layout: ManiaLayoutInfo,
    pub(crate) render_judgments: Vec<RenderJudgment>,
    pub(crate) judgments_by_idx: Vec<Option<RenderJudgment>>,
    pub(crate) ln_release_by_idx: Vec<Option<LnReleaseInfo>>,
    pub(crate) score_judgments: Vec<judgment::ScoreJudgmentEvent>,
    pub(crate) score_mode: judgment::ScoreMode,
    pub(crate) combo_data: ComboComputation,
    pub(crate) health_timeline: crate::renderer::HealthTimeline,
    pub(crate) score_state_end_time_ms: Option<i32>,
    pub(crate) render_windows: Windows,
    pub(crate) visual_pps: f32,
    pub(crate) scroll_model: Option<timing::StableScrollModel>,
    pub(crate) sorted_indices: Vec<usize>,
    pub(crate) note_distances: Vec<f64>,
    pub(crate) effective_end_distances: Vec<f64>,
    pub(crate) playback_clock: PlaybackClock,
    pub(crate) end_sequence: EndSequencePlan,
    pub(crate) main_scene_total_frames: u64,
    pub(crate) barlines: Vec<i32>,
    pub(crate) key_timeline: Vec<KeyMaskEvent>,
    pub(crate) intro_frames: u64,
    pub(crate) intro_config: Option<IntroConfig>,
    pub(crate) results_data: Option<ResultsScreenData>,
    pub(crate) replay_integrity: Option<ReplayIntegrityReport>,
    pub(crate) score_scale: f64,
    pub(crate) hud_pp_timeline: Vec<(i32, f32)>,
    pub(crate) hud_pp_final: Option<f32>,
    pub(crate) _audio_temp_files: Vec<TempFileGuard>,
}
#[derive(Debug, Clone)]
pub(crate) struct EffectiveReplayOutcome {
    pub(crate) score_judgments: Vec<judgment::ScoreJudgmentEvent>,
    pub(crate) combo_data: ComboComputation,
    pub(crate) computed_stats: ReplayBasicStatistics,
    pub(crate) computed_accuracy: f32,
    pub(crate) computed_score: u32,
    pub(crate) final_combo: u32,
    pub(crate) score_state_end_time_ms: Option<i32>,
}
#[derive(Debug, Clone, Copy, Default)]
struct EffectiveScoreCutoff {
    time_ms: Option<i32>,
    score_judgment_count: Option<usize>,
}
pub(crate) fn resolve_effective_playback_profile(
    playback: &crate::utils::mods::PlaybackModSettings,
    adaptive_audio_profile: Option<&crate::utils::mods::PlaybackRateProfile>,
) -> crate::utils::mods::PlaybackRateProfile {
    adaptive_audio_profile
        .cloned()
        .unwrap_or_else(|| playback.profile.clone())
}
const LATE_HOLD_TIMEOUT_MS: i32 = 200;
pub(crate) fn effective_hit_object_end_time(
    hit_object: &crate::types::HitObject,
    release_time: Option<i32>,
) -> i32 {
    let end_time = hit_object
        .end_time
        .filter(|&end| end > hit_object.time + 2)
        .unwrap_or(hit_object.time);
    if hit_object.is_long_note() {
        if let Some(release_time) = release_time {
            if release_time > end_time {
                // Late releases keep held notes visible briefly without stretching them forever.
                return release_time.min(end_time + LATE_HOLD_TIMEOUT_MS);
            }
        }
        end_time
    } else {
        hit_object.time
    }
}

fn add_score_judgment_to_stats(
    stats: &mut ReplayBasicStatistics,
    judgment: judgment::ScoreJudgmentEvent,
) {
    match judgment.kind {
        crate::types::JudgmentKind::Max => stats.max += 1,
        crate::types::JudgmentKind::Hit300 => stats.hit300 += 1,
        crate::types::JudgmentKind::Hit200 => stats.hit200 += 1,
        crate::types::JudgmentKind::Hit100 => stats.hit100 += 1,
        crate::types::JudgmentKind::Hit50 => stats.hit50 += 1,
        crate::types::JudgmentKind::Miss => stats.miss += 1,
    }
}
impl ManiaVideoConverter {
    pub(crate) fn resolve_intro_possible_max_combo(
        &self,
        beatmap: &Beatmap,
        score_judgments: &[judgment::ScoreJudgmentEvent],
        score_mode: judgment::ScoreMode,
    ) -> u32 {
        if score_mode.uses_prgrss_ln_ticks() {
            beatmap.max_combo()
        } else {
            score_judgments.len().min(u32::MAX as usize) as u32
        }
    }
    pub(crate) fn resolve_playable_mania_beatmap(
        &self,
        replay: &ReplayData,
        beatmap: Beatmap,
        beatmap_path: &Path,
    ) -> Result<Beatmap, ConvertError> {
        if beatmap.metadata.mode == 3 {
            return Ok(beatmap);
        }
        if replay.origin == crate::types::ReplayOrigin::LazerExport
            && replay.game_mode == 3
            && beatmap.metadata.mode == 0
        {
            // Lazer mania replays can reference standard maps that were converted at play time.
            return crate::modes::mania::convert::convert_standard_beatmap_to_mania(
                &beatmap,
                beatmap_path,
            )
            .map_err(|error| ConvertError::Parse(error.to_string()));
        }
        Err(ConvertError::Parse(format!(
            "beatmap is not osu!mania (Mode={})",
            beatmap.metadata.mode
        )))
    }
    pub(crate) fn build_end_sequence_plan(
        &self,
        opts: &ResolveOpts,
        health_timeline: &crate::renderer::HealthTimeline,
        playback_clock: &PlaybackClock,
        frame_time_ms: f64,
        default_timeline_end_ms: i32,
        effective_gameplay_end_ms: i32,
    ) -> EndSequencePlan {
        let fail_end_ms = health_timeline.fail_time_ms.map(|fail_time| {
            (fail_time + anim::FAIL_ANIM_MS + anim::FAIL_HOLD_MS).min(default_timeline_end_ms)
        });
        let manual_end_active = opts.end_seconds.is_some();
        let results_enabled = !manual_end_active;
        if results_enabled {
            let gameplay_end_ms = fail_end_ms
                .unwrap_or_else(|| effective_gameplay_end_ms.max(default_timeline_end_ms));
            let fade_out_end_ms = playback_clock
                .profile()
                .beatmap_time_after_output_elapsed_ms(
                    gameplay_end_ms as f64,
                    crate::results::RESULTS_FADE_MS as f64,
                )
                .round() as i32;
            let results_start_ms = playback_clock
                .profile()
                .beatmap_time_after_output_elapsed_ms(
                    gameplay_end_ms as f64,
                    crate::results::RESULTS_TRANSITION_MS as f64,
                )
                .round() as i32;
            let results_end_ms = playback_clock
                .profile()
                .beatmap_time_after_output_elapsed_ms(
                    results_start_ms as f64,
                    crate::results::RESULTS_DURATION_MS as f64,
                )
                .round() as i32;
            let main_scene_output_duration_ms = playback_clock
                .output_elapsed_ms_for_beatmap_time(results_start_ms as f64)
                .max(frame_time_ms);
            let main_scene_frames = (main_scene_output_duration_ms / frame_time_ms)
                .ceil()
                .max(1.0) as u64;
            let results_frames = (crate::results::RESULTS_DURATION_MS as f64 / frame_time_ms)
                .ceil()
                .max(1.0) as u64;
            EndSequencePlan {
                gameplay_end_ms,
                hud_hide_start_ms: gameplay_end_ms,
                fade_out_end_ms,
                results_start_ms,
                results_end_ms,
                main_scene_frames,
                results_frames,
            }
        } else {
            // A manual end time is an explicit clip, so skip the results sequence.
            let main_scene_end_ms = fail_end_ms.unwrap_or(default_timeline_end_ms);
            let main_scene_output_duration_ms = playback_clock
                .output_elapsed_ms_for_beatmap_time(main_scene_end_ms as f64)
                .max(frame_time_ms);
            let main_scene_frames = (main_scene_output_duration_ms / frame_time_ms)
                .ceil()
                .max(1.0) as u64;
            EndSequencePlan {
                gameplay_end_ms: fail_end_ms.unwrap_or(effective_gameplay_end_ms),
                hud_hide_start_ms: effective_gameplay_end_ms,
                fade_out_end_ms: effective_gameplay_end_ms,
                results_start_ms: main_scene_end_ms,
                results_end_ms: main_scene_end_ms,
                main_scene_frames,
                results_frames: 0,
            }
        }
    }
    fn build_hud_pp_timeline(
        &self,
        beatmap_path: &Path,
        replay: &ManiaReplayData,
        render_judgments: &[RenderJudgment],
    ) -> (Vec<(i32, f32)>, Option<f32>) {
        let map = match rosu_pp::Beatmap::from_path(beatmap_path) {
            Ok(map) => map,
            Err(err) => {
                eprintln!("   hud pp unavailable: failed to parse beatmap for pp ({err})");
                return (Vec::new(), None);
            }
        };
        if let Err(suspicion) = map.check_suspicion() {
            eprintln!("   hud pp unavailable: beatmap rejected by suspicion check ({suspicion:?})");
            return (Vec::new(), None);
        }
        let difficulty = rosu_pp::Difficulty::new().mods(replay.replay.mods);
        let mut gradual = match rosu_pp::mania::ManiaGradualPerformance::new(difficulty, &map) {
            Ok(gradual) => gradual,
            Err(err) => {
                eprintln!("   hud pp unavailable: gradual mania pp setup failed ({err:?})");
                return (Vec::new(), None);
            }
        };
        let mut sorted = render_judgments.to_vec();
        sorted.sort_by_key(|judgment| judgment.idx);
        let mut state = rosu_pp::mania::ManiaScoreState::new();
        let mut timeline = Vec::with_capacity(sorted.len());
        for judgment in sorted {
            match judgment.kind {
                crate::types::JudgmentKind::Max => state.n320 += 1,
                crate::types::JudgmentKind::Hit300 => state.n300 += 1,
                crate::types::JudgmentKind::Hit200 => state.n200 += 1,
                crate::types::JudgmentKind::Hit100 => state.n100 += 1,
                crate::types::JudgmentKind::Hit50 => state.n50 += 1,
                crate::types::JudgmentKind::Miss => state.misses += 1,
            }
            let Some(attrs) = gradual.next(state.clone()) else {
                break;
            };
            if attrs.pp.is_finite() {
                timeline.push((judgment.judgment_time(), attrs.pp as f32));
            }
        }
        let final_pp = timeline.last().map(|(_, pp)| *pp);
        (timeline, final_pp)
    }
    pub(crate) fn prepare_replay_render(
        &self,
        replay: &mut ManiaReplayData,
        mut beatmap: Beatmap,
        beatmap_path: &Path,
        set_dir: &Path,
        set_files: &[String],
        output_path: &Path,
        skin_path: Option<&Path>,
        opts: &ResolveOpts,
        intro_user_data: Option<&crate::utils::IntroUserData>,
        autoplay: bool,
    ) -> Result<PreparedReplayRender, ConvertError> {
        let key_count = self.effective_key_count(&beatmap);
        self.ensure_rd_not_enabled(&replay.replay)?;
        let fail_conditions = self.resolve_replay_fail_condition_mod(&replay.replay)?;
        let muted_config = self.resolve_replay_muted_config(&replay.replay);
        let cover_config = self.resolve_playfield_cover_config(&replay.replay)?;
        self.apply_replay_beatmap_conversion_mods(&mut beatmap, &replay.replay)?;
        let playback = self.resolve_replay_playback_settings(&replay.replay, &beatmap)?;
        // Compute the plan once before loading expensive assets so invalid time ranges fail early.
        let _preliminary_plan = self.compute_plan(&beatmap, opts, playback.profile.clone())?;
        println!(
            "   {} - {} [{}]",
            beatmap.metadata.artist, beatmap.metadata.title, beatmap.metadata.version
        );
        let star_rating = self.enforce_star_limit(beatmap_path)?;
        println!("   stars: {:.2}", star_rating);
        let bg = self.pick_background(&beatmap, beatmap_path, set_dir, set_files, opts);
        if let Some(ref bg) = bg {
            println!("   bg ({:?}): {}", bg.kind, bg.path.display());
        }
        let results_background_path = self.pick_intro_background(
            &beatmap,
            beatmap_path,
            set_dir,
            set_files,
            bg.as_ref(),
            opts,
        );
        self.progress(10, "Loading skin...");
        let mut skin = self.load_skin(skin_path, set_dir, key_count)?;
        if !self.settings.combo_images_enabled {
            skin.config.combo_burst.clear();
        }
        println!("   skin: {} images loaded", skin.image_count());
        self.progress(15, "Resolving audio...");
        let audio_local_only = autoplay || opts.audio_local_only;
        let audio_path = if audio_local_only {
            self.resolve_audio_local_only(beatmap_path, &beatmap.metadata.audio_filename)?
        } else {
            self.resolve_audio(beatmap_path, &beatmap.metadata.audio_filename)?
        };
        if let Some(ref path) = audio_path {
            println!("   audio: {}", path.display());
        } else if audio_local_only {
            println!(
                "   warn: audio not found: {}",
                beatmap.metadata.audio_filename
            );
        } else {
            println!("   warn: no audio");
        }
        self.progress(20, "Computing judgments...");
        let (windows, score_mode_ctx) =
            judgment::res_sco_mod_for_repl(beatmap.difficulty.od, &replay.replay);
        println!(
            "   [judgment] mode: {:?} (requested: {:?})",
            score_mode_ctx.effective_mode, score_mode_ctx.requested_mode
        );
        let (judgments, ln_releases, ln_debug, score_judgments, debug_result) =
            if self.settings.note_debug || self.settings.all_presses {
                let dr = judgment::calc_jdbg_mode_for_replay(
                    &replay.replay,
                    &beatmap.hit_objects,
                    &replay.key_actions,
                    &windows,
                    score_mode_ctx,
                );
                let judgments: Vec<_> = dr.judgments.iter().map(judgment::Judgment::from).collect();
                (
                    judgments,
                    dr.ln_releases.clone(),
                    dr.ln_debug.clone(),
                    dr.score_judgments.clone(),
                    Some(dr),
                )
            } else {
                let result = judgment::calc_judg_mode_for_replay(
                    &replay.replay,
                    &beatmap.hit_objects,
                    &replay.key_actions,
                    &windows,
                    score_mode_ctx,
                );
                (
                    result.judgments,
                    result.ln_releases,
                    result.ln_debug,
                    result.score_judgments,
                    None,
                )
            };
        println!("   {} judgments", judgments.len());
        if let Some(ref dr) = debug_result {
            self.print_judgment_summary(&windows, &judgments, beatmap.difficulty.od);
            if self.settings.note_debug {
                self.print_note_debug_detailed(
                    &beatmap,
                    &dr.judgments,
                    &dr.ln_releases,
                    &windows,
                    Some(&dr.press_status),
                    score_mode_ctx.effective_mode,
                );
            }
            if self.settings.all_presses {
                self.print_all_presses(&dr.press_status, &beatmap);
            }
        }
        let resolved_hud_config = self.settings.hud_config.as_ref().map(|cfg| {
            resolve_hud_config(cfg, self.settings.width as f32, self.settings.height as f32)
        });
        let layout = self.build_layout(key_count, &skin, resolved_hud_config.as_ref());
        let cover_metrics = crate::renderer::resolve_playfield_cover_metrics(
            cover_config.mode,
            &layout,
            &skin,
            self.settings.height,
        );
        println!(
            "   layout: {}k, stage {}x{} at ({}, {})",
            key_count, layout.stage.width, layout.stage.height, layout.stage.x, layout.stage.y
        );
        let render_judgments =
            self.build_render_judgments(&beatmap.hit_objects, &judgments, &ln_releases);
        let mut judgments_by_idx = vec![None; beatmap.hit_objects.len()];
        for judgment in &render_judgments {
            if judgment.idx < judgments_by_idx.len() {
                judgments_by_idx[judgment.idx] = Some(*judgment);
            }
        }
        let mut ln_release_by_idx = vec![None; beatmap.hit_objects.len()];
        for (&idx, info) in &ln_releases {
            if idx < ln_release_by_idx.len() {
                ln_release_by_idx[idx] = Some(LnReleaseInfo::from(info));
            }
        }
        let combo_renderer = ReplayRenderer::new();
        let combo_data = self.compute_combo_computation(
            &combo_renderer,
            &beatmap.hit_objects,
            &render_judgments,
            &score_judgments,
            &ln_releases,
            &ln_debug,
            &windows,
            score_mode_ctx.effective_mode,
            replay.replay.mods,
        );
        let health_timeline = finalize_health_timeline_for_replay(
            combo_renderer.precompute_health_timeline(
                &beatmap.hit_objects,
                &beatmap.events.breaks,
                &judgments_by_idx,
                &ln_release_by_idx,
                &combo_data.ln_ticks,
                &combo_data.ln_breaks,
                beatmap.difficulty.hp,
            ),
            &replay.replay,
            &score_judgments,
            fail_conditions,
        );
        if let Some(fail_time_ms) = health_timeline.fail_time_ms {
            println!(
                "   [health] fail_time={}ms hp_multiplier={:.3}",
                fail_time_ms, health_timeline.hp_multiplier_normal
            );
        }
        let effective_outcome = self.build_effective_replay_outcome_for_replay(
            &score_judgments,
            &combo_data,
            &health_timeline,
            score_mode_ctx.effective_mode,
            &replay.replay,
        );
        let intro_possible_max_combo = self.resolve_intro_possible_max_combo(
            &beatmap,
            &score_judgments,
            score_mode_ctx.effective_mode,
        );
        if autoplay {
            self.apply_autoplay_stats(
                replay,
                &effective_outcome.score_judgments,
                score_mode_ctx.effective_mode,
                effective_outcome.combo_data.computed_max_combo,
            );
        }
        let adaptive_audio_profile = playback.adaptive_speed.map(|adaptive_speed| {
            crate::video::build_adaptive_playback_profile(
                adaptive_speed.initial_rate,
                &beatmap.hit_objects,
                &effective_outcome.score_judgments,
            )
        });
        let effective_playback_profile =
            resolve_effective_playback_profile(&playback, adaptive_audio_profile.as_ref());
        let plan = self.compute_plan(&beatmap, opts, effective_playback_profile.clone())?;
        let est_mb = plan.total_frames as f32
            * self.settings.width as f32
            * self.settings.height as f32
            * 4.0
            / (1024.0 * 1024.0);
        println!("   {} frames ({:.1} MB)", plan.total_frames, est_mb);
        let intro_ms = if self.settings.intro_enabled {
            INTRO_DURATION_MS
        } else {
            0
        };
        let preview_ms = self.find_preview(&beatmap);
        let intro_bpm = get_bpm_at_time(&beatmap.timing_points, preview_ms as f64);
        let playback_clock = PlaybackClock::new(plan.timeline_start, plan.playback_profile.clone());
        let normal_count = render_judgments
            .iter()
            .filter(|judgment| !judgment.is_ln)
            .count();
        let ln_count = render_judgments
            .iter()
            .filter(|judgment| judgment.is_ln)
            .count();
        let normal_hits = render_judgments
            .iter()
            .filter(|judgment| !judgment.is_ln && !judgment.kind.breaks_combo())
            .count();
        let ln_hits = render_judgments
            .iter()
            .filter(|judgment| judgment.is_ln && !judgment.kind.breaks_combo())
            .count();
        let with_press = render_judgments
            .iter()
            .filter(|judgment| judgment.press_time.is_some())
            .count();
        println!(
            "   [debug] normal: {} ({} hits), LN: {} ({} hits), with_press_time: {}",
            normal_count, normal_hits, ln_count, ln_hits, with_press
        );
        let mut max_count = 0;
        let mut h300_count = 0;
        let mut h200_count = 0;
        let mut h100_count = 0;
        let mut h50_count = 0;
        let mut miss_count = 0;
        for judgment in &render_judgments {
            match judgment.kind {
                crate::types::JudgmentKind::Max => max_count += 1,
                crate::types::JudgmentKind::Hit300 => h300_count += 1,
                crate::types::JudgmentKind::Hit200 => h200_count += 1,
                crate::types::JudgmentKind::Hit100 => h100_count += 1,
                crate::types::JudgmentKind::Hit50 => h50_count += 1,
                crate::types::JudgmentKind::Miss => miss_count += 1,
            }
        }
        println!(
            "   [debug] judgments: MAX={}, 300={}, 200={}, 100={}, 50={}, Miss={}",
            max_count, h300_count, h200_count, h100_count, h50_count, miss_count
        );
        let replay_stats = &replay.replay;
        println!(
            "   [replay] osr stats: MAX(geki)={}, 300={}, 200(katu)={}, 100={}, 50={}, Miss={}",
            replay_stats.count_geki,
            replay_stats.count_300,
            replay_stats.count_katu,
            replay_stats.count_100,
            replay_stats.count_50,
            replay_stats.count_miss
        );
        let render_windows = Windows {
            max: windows.max,
            hit300: windows.hit300,
            hit200: windows.hit200,
            hit100: windows.hit100,
            hit50: windows.hit50,
            miss: windows.hit50 + 100,
        };
        // Stable scroll speed is calibrated against osu!'s legacy 768px playfield space.
        const STABLE_MAX_TIME_RANGE_MS: f64 = 11_485.0;
        const LEGACY_HIT_POSITION_SPACE: f64 = 768.0;
        const LEGACY_DEFAULT_HIT_POSITION: f64 = 402.0;
        let ss = (self.settings.scroll_speed as f64).max(1.0);
        let travel_dist = (layout.stage.hit_y - layout.stage.top_y).max(1) as f64;
        let scale_y = (layout.scale_y as f64).max(1e-6);
        let hit_position = layout.stage.hit_y as f64 / scale_y;
        let default_length = (LEGACY_HIT_POSITION_SPACE - LEGACY_DEFAULT_HIT_POSITION).max(1.0);
        let hit_length = (LEGACY_HIT_POSITION_SPACE - hit_position).max(1.0);
        let hit_position_scale = (hit_length / default_length).clamp(0.25, 4.0);
        let time_range_ms = (STABLE_MAX_TIME_RANGE_MS / ss) * hit_position_scale;
        let pps_base = (travel_dist * 1000.0 / time_range_ms.max(1e-3)) as f32;
        let visual_pps = pps_base;
        eprintln!(
            "   [scroll] ss={} pps_base={:.2} visual_pps={:.2} travel_dist={}",
            self.settings.scroll_speed,
            pps_base,
            visual_pps,
            layout.stage.hit_y - layout.stage.top_y
        );
        let last_object_time = beatmap
            .hit_objects
            .iter()
            .map(|hit_object| hit_object.end_time.unwrap_or(hit_object.time))
            .max();
        let scroll_visualisation_mods =
            self.resolve_replay_scroll_visualisation_mods(&replay.replay);
        if scroll_visualisation_mods.constant_speed {
            eprintln!("   [scroll] visualisation=constant");
        }
        let scroll_model = self.resolve_replay_scroll_model(
            &replay.replay,
            &beatmap.timing_points,
            last_object_time,
            &playback_clock,
            pps_base as f64,
        );
        let sorted_indices = ReplayRenderer::prepare_sorted_notes(&beatmap.hit_objects, key_count);
        let distance_at_ms = |time_ms: i32| -> f64 {
            let display_time_ms = playback_clock
                .output_elapsed_ms_for_beatmap_time(time_ms as f64)
                .round() as i32;
            if let Some(model) = scroll_model.as_ref() {
                model.object_distance_at_ms(display_time_ms)
            } else {
                (pps_base as f64 * display_time_ms as f64) / 1000.0
            }
        };
        let note_distances: Vec<f64> = sorted_indices
            .iter()
            .map(|&idx| distance_at_ms(beatmap.hit_objects[idx].time))
            .collect();
        let effective_note_end_times: Vec<i32> = sorted_indices
            .iter()
            .map(|&idx| {
                // Visibility uses judgment release data so held notes survive late releases.
                effective_hit_object_end_time(
                    &beatmap.hit_objects[idx],
                    ln_releases.get(&idx).and_then(|info| info.time),
                )
            })
            .collect();
        let effective_end_distances: Vec<f64> = sorted_indices
            .iter()
            .zip(effective_note_end_times.iter())
            .map(|(_, &effective_end_time)| distance_at_ms(effective_end_time))
            .collect();
        let frame_time_ms = 1000.0 / self.settings.fps as f64;
        let last_object_end_ms = beatmap
            .hit_objects
            .iter()
            .map(|hit_object| hit_object.end_time.unwrap_or(hit_object.time))
            .max()
            .unwrap_or(plan.timeline_end);
        let last_effective_note_end_ms = effective_note_end_times
            .iter()
            .copied()
            .max()
            .unwrap_or(last_object_end_ms);
        let last_score_event_time_ms = effective_outcome
            .combo_data
            .score_events
            .last()
            .map(|event| event.time)
            .unwrap_or(last_effective_note_end_ms);
        let effective_gameplay_end_ms = last_object_end_ms
            .max(last_effective_note_end_ms)
            .max(last_score_event_time_ms);
        let end_sequence = self.build_end_sequence_plan(
            opts,
            &health_timeline,
            &playback_clock,
            frame_time_ms,
            plan.timeline_end,
            effective_gameplay_end_ms,
        );
        let output_timeline_end_ms = end_sequence.results_end_ms;
        let barlines =
            self.compute_barlines(&beatmap, plan.timeline_start, end_sequence.results_start_ms);
        let key_timeline = self.build_key_mask_timeline(&replay.key_actions);
        let intro_frames = if self.settings.intro_enabled {
            (intro_ms as f32 / 1000.0 * self.settings.fps as f32) as u64
        } else {
            0
        };
        let _total_output_frames =
            intro_frames + end_sequence.main_scene_frames + end_sequence.results_frames;
        let _total_output_duration_ms = intro_ms as f64
            + playback_clock
                .output_elapsed_ms_for_beatmap_time(f64::from(output_timeline_end_ms))
                .max(0.0);
        println!(
            "   [combo] computed_max={} ln_ticks={} ln_breaks={}",
            effective_outcome.combo_data.computed_max_combo,
            effective_outcome.combo_data.ln_ticks.len(),
            effective_outcome.combo_data.ln_breaks.len()
        );
        println!(
            "   [end] gameplay={}ms hud_hide={}ms results_start={}ms results_end={}ms",
            end_sequence.gameplay_end_ms,
            end_sequence.hud_hide_start_ms,
            end_sequence.results_start_ms,
            end_sequence.results_end_ms
        );
        if replay.replay.max_combo as u32 != effective_outcome.combo_data.computed_max_combo {
            println!(
                "   [combo] mismatch: osr_max={} computed_max={}",
                replay.replay.max_combo, effective_outcome.combo_data.computed_max_combo
            );
        }
        let replay_final_score = replay.replay.total_score as u64;
        let computed_final_score = effective_outcome.computed_score;
        let computed_stats = effective_outcome.computed_stats;
        let computed_accuracy = effective_outcome.computed_accuracy;
        let replay_integrity = if autoplay {
            None
        } else {
            Some(ReplayIntegrityReport {
                has_summary_mismatch: self
                    .has_replay_summary_mismatch(&replay.replay, computed_stats),
            })
        };
        let target_score = replay_final_score.min(1_000_000);
        let score_scale = if computed_final_score > 0 && target_score > 0 {
            let diff = (target_score as i64 - computed_final_score as i64).abs();
            if diff > 1 {
                // Keep HUD score interpolation aligned with the trusted replay total.
                target_score as f64 / computed_final_score as f64
            } else {
                1.0
            }
        } else {
            1.0
        };
        println!(
            "   score: replay={}, computed={}, scale={:.4}",
            replay_final_score, computed_final_score, score_scale
        );
        let replay_mod_display = ReplayModDisplay {
            origin: replay.replay.origin,
            acronyms: self.resolve_intro_display_mods(&replay.replay),
        };
        let display_mods = self.resolve_intro_mod_badges(&replay.replay);
        let silver_grade = crate::results::silver_grade_from_mods(replay.replay.mods)
            || self.replay_has_api_mod(&replay.replay, "HD")
            || self.replay_has_api_mod(&replay.replay, "FL")
            || self.replay_has_api_mod(&replay.replay, "FI");
        let perfect_combo = crate::results::compute_perfect_combo(
            computed_stats,
            effective_outcome.combo_data.combo_before_breaks.len(),
            effective_outcome.final_combo,
            effective_outcome.combo_data.computed_max_combo,
        );
        let graph_start_ms = plan.timeline_start.min(0);
        let graph_points = crate::results::build_results_graph_points(
            &health_timeline,
            graph_start_ms,
            end_sequence.gameplay_end_ms.max(graph_start_ms + 1),
            crate::results::GRAPH_SAMPLE_COUNT,
            &effective_outcome.score_judgments,
        );
        let timing_summary = self.build_results_timing_summary(
            &beatmap.hit_objects,
            &render_judgments,
            &ln_release_by_idx,
            &effective_outcome.score_judgments,
            playback_clock.profile(),
        );
        let use_computed_results_summary = autoplay
            || replay_integrity
                .as_ref()
                .is_some_and(|report| report.has_summary_mismatch);
        let results_display_score = if use_computed_results_summary {
            computed_final_score
        } else {
            target_score as u32
        };
        let results_data = end_sequence.has_results().then(|| ResultsScreenData {
            player_name: replay.replay.player_name.clone(),
            artist: beatmap.metadata.artist.clone(),
            title: beatmap.metadata.title.clone(),
            difficulty: beatmap.metadata.version.clone(),
            creator: beatmap.metadata.creator.clone(),
            mod_badges: display_mods.clone(),
            mod_origin: replay.replay.origin,
            replay_timestamp: (replay.replay.timestamp > 0).then_some(replay.replay.timestamp),
            score: results_display_score,
            accuracy: computed_accuracy,
            max_combo: effective_outcome.combo_data.computed_max_combo,
            final_combo: effective_outcome.final_combo,
            statistics: computed_stats,
            grade: crate::results::grade_for_accuracy(computed_accuracy, silver_grade),
            perfect_combo,
            graph_points,
            timing_summary,
        });
        let compose_bg = bg
            .as_ref()
            .filter(|source| matches!(source.kind, BackgroundKind::Video));
        let muted_automation = muted_config.map(|config| {
            crate::video::build_muted_automation(&effective_outcome.combo_data.score_events, config)
        });
        let mut audio_temp_files = Vec::new();
        let music_overlay_paths = Vec::new();
        let mut effects_overlay_paths = Vec::new();
        let mut main_audio_gain_path: Option<PathBuf> = None;
        let effective_audio_path = audio_path.clone();
        let temp_artifact_dir = output_path.parent().unwrap_or_else(|| Path::new("."));
        let mut compose_playback = playback.clone();
        compose_playback.profile = effective_playback_profile;
        let compose_audio_playback_profile = adaptive_audio_profile.as_ref();
        if effective_audio_path.is_some() {
            let (audio_sources, _skin_audio_temp_dirs) = {
                let mut sources = vec![crate::video::AudioSearchSource::new(
                    set_dir.to_path_buf(),
                    set_files.to_vec(),
                )];
                let (skin_sources, guards) = self.resolve_skin_audio_sources(skin_path);
                sources.extend(skin_sources);
                (sources, guards)
            };
            if playback.nightcore {
                // Nightcore adds a generated clap track instead of modifying the source audio.
                let overlay_path =
                    self.make_temp_overlay_path_in(temp_artifact_dir, "nightcore_overlay", "wav");
                crate::video::generate_nightcore_overlay(
                    &beatmap,
                    plan.timeline_start,
                    output_timeline_end_ms,
                    intro_ms,
                    playback.clock_rate,
                    &overlay_path,
                )
                .map_err(|err| {
                    ConvertError::Compose(ComposeError::Io(std::io::Error::other(err.to_string())))
                })?;
                effects_overlay_paths.push(overlay_path.clone());
                audio_temp_files.push(TempFileGuard::new(overlay_path));
            }
            let hitsound_overlay_path =
                self.make_temp_overlay_path_in(temp_artifact_dir, "hitsound_overlay", "wav");
            // Hitsounds and gameplay effects stay as overlays so ffmpeg can mix them with music.
            crate::video::generate_hitsound_overlay(
                &beatmap,
                &audio_sources,
                &effective_outcome.score_judgments,
                &playback_clock,
                adaptive_audio_profile.as_ref(),
                intro_ms,
                output_timeline_end_ms,
                muted_automation.as_ref(),
                &hitsound_overlay_path,
            )
            .map_err(|err| {
                ConvertError::Compose(ComposeError::Io(std::io::Error::other(err.to_string())))
            })?;
            effects_overlay_paths.push(hitsound_overlay_path.clone());
            audio_temp_files.push(TempFileGuard::new(hitsound_overlay_path));
            let gameplay_effects_path =
                self.make_temp_overlay_path_in(temp_artifact_dir, "gameplay_effects", "wav");
            crate::video::generate_gameplay_effect_overlay(
                &audio_sources,
                &effective_outcome.combo_data.score_events,
                health_timeline.fail_time_ms,
                &playback_clock,
                adaptive_audio_profile.as_ref(),
                intro_ms,
                output_timeline_end_ms,
                &gameplay_effects_path,
            )
            .map_err(|err| {
                ConvertError::Compose(ComposeError::Io(std::io::Error::other(err.to_string())))
            })?;
            effects_overlay_paths.push(gameplay_effects_path.clone());
            audio_temp_files.push(TempFileGuard::new(gameplay_effects_path));
            if let Some(muted_config) = muted_config {
                let automation = muted_automation
                    .as_ref()
                    .expect("muted automation should exist when config exists");
                let gain_path =
                    self.make_temp_overlay_path_in(temp_artifact_dir, "main_audio_gain", "wav");
                crate::video::generate_main_audio_gain_track(
                    &playback_clock,
                    intro_ms,
                    output_timeline_end_ms,
                    automation,
                    &gain_path,
                )
                .map_err(|err| {
                    ConvertError::Compose(ComposeError::Io(std::io::Error::other(err.to_string())))
                })?;
                main_audio_gain_path = Some(gain_path.clone());
                audio_temp_files.push(TempFileGuard::new(gain_path));
                if muted_config.enable_metronome {
                    let metronome_path =
                        self.make_temp_overlay_path_in(temp_artifact_dir, "muted_metronome", "wav");
                    crate::video::generate_metronome_overlay(
                        &beatmap,
                        &playback_clock,
                        adaptive_audio_profile.as_ref(),
                        intro_ms,
                        output_timeline_end_ms,
                        automation,
                        &metronome_path,
                    )
                    .map_err(|err| {
                        ConvertError::Compose(ComposeError::Io(std::io::Error::other(
                            err.to_string(),
                        )))
                    })?;
                    effects_overlay_paths.push(metronome_path.clone());
                    audio_temp_files.push(TempFileGuard::new(metronome_path));
                }
            }
        }
        let compose_template = self.build_compose_opts(
            output_path,
            effective_audio_path.as_deref(),
            plan.timeline_start,
            output_timeline_end_ms,
            preview_ms as i32,
            compose_bg,
            compose_playback,
            compose_audio_playback_profile,
            health_timeline.fail_time_ms,
            main_audio_gain_path.as_deref(),
            &music_overlay_paths,
            &effects_overlay_paths,
        );
        let intro_config = if intro_frames > 0 {
            let intro_bg_path = self.pick_intro_background(
                &beatmap,
                beatmap_path,
                set_dir,
                set_files,
                bg.as_ref(),
                opts,
            );
            Some(IntroConfig {
                duration_ms: intro_ms,
                logo_path: self.find_logo_path().unwrap_or_default(),
                background_path: intro_bg_path,
                background_blur_percent: self.settings.background_blur_percent,
                preview_time_ms: preview_ms as i32,
                bpm: intro_bpm,
                width: self.settings.width,
                height: self.settings.height,
                fps: self.settings.fps,
                player_name: Some(replay.replay.player_name.clone()),
                avatar_path: intro_user_data.and_then(|data| data.avatar_path.clone()),
                country_code: intro_user_data.and_then(|data| data.country_code.clone()),
                flag_path: intro_user_data.and_then(|data| data.flag_path.clone()),
                team_badge_path: intro_user_data.and_then(|data| data.team_badge_path.clone()),
                map_title: Some(beatmap.metadata.title.clone()),
                map_artist: Some(beatmap.metadata.artist.clone()),
                map_difficulty: Some(beatmap.metadata.version.clone()),
                map_creator: Some(beatmap.metadata.creator.clone()),
                key_count,
                mods: replay.replay.mods,
                display_mods: Some(display_mods.clone()),
                star_rating: Some(star_rating as f32),
                accuracy: computed_accuracy,
                best_combo: effective_outcome.combo_data.computed_max_combo,
                final_combo: effective_outcome.final_combo,
                max_combo: intro_possible_max_combo,
                glow_enabled: false,
            })
        } else {
            None
        };
        let (hud_pp_timeline, hud_pp_final) =
            self.build_hud_pp_timeline(beatmap_path, replay, &render_judgments);
        let score_state_end_time_ms = effective_outcome.score_state_end_time_ms;
        Ok(PreparedReplayRender {
            beatmap,
            cover_config,
            cover_metrics,
            set_dir: set_dir.to_path_buf(),
            set_files: set_files.to_vec(),
            bg,
            results_background_path,
            skin,
            replay_mod_display,
            compose_template,
            resolved_hud_config,
            layout,
            render_judgments,
            judgments_by_idx,
            ln_release_by_idx,
            score_judgments: effective_outcome.score_judgments,
            score_mode: score_mode_ctx.effective_mode,
            combo_data: effective_outcome.combo_data,
            health_timeline,
            score_state_end_time_ms,
            render_windows,
            visual_pps,
            scroll_model,
            sorted_indices,
            note_distances,
            effective_end_distances,
            playback_clock,
            end_sequence,
            main_scene_total_frames: end_sequence.main_scene_frames,
            barlines,
            key_timeline,
            intro_frames,
            intro_config,
            results_data,
            replay_integrity,
            score_scale,
            hud_pp_timeline,
            hud_pp_final,
            _audio_temp_files: audio_temp_files,
        })
    }
    pub(crate) fn effective_key_count(&self, beatmap: &Beatmap) -> u8 {
        let mut key_count = beatmap.key_count();
        if key_count == 0 {
            if let Some(max_col) = beatmap.hit_objects.iter().map(|h| h.column).max() {
                key_count = max_col.saturating_add(1);
            }
        }
        key_count.max(1)
    }
    pub(crate) fn build_render_judgments(
        &self,
        hit_objects: &[crate::types::HitObject],
        judgments: &[judgment::Judgment],
        ln_releases: &HashMap<usize, judgment::LnReleaseInfo>,
    ) -> Vec<RenderJudgment> {
        judgments
            .iter()
            .map(|j| {
                let is_ln = hit_objects
                    .get(j.index)
                    .map(|h| h.is_long_note())
                    .unwrap_or(false);
                let rel_time = if is_ln {
                    ln_releases.get(&j.index).and_then(|r| r.time)
                } else {
                    None
                };
                RenderJudgment {
                    idx: j.index,
                    column: hit_objects.get(j.index).map(|h| h.column).unwrap_or(0),
                    time: j
                        .press_time
                        .unwrap_or(hit_objects.get(j.index).map(|h| h.time).unwrap_or(0)),
                    kind: crate::types::JudgmentKind::from_str(&j.kind),
                    press_time: j.press_time,
                    rel_time,
                    is_ln,
                }
            })
            .collect()
    }
    pub(crate) fn judgment_kind_to_rel_kind(
        kind: crate::types::JudgmentKind,
    ) -> judgment::ReleaseKind {
        match kind {
            crate::types::JudgmentKind::Max => judgment::ReleaseKind::Max,
            crate::types::JudgmentKind::Hit300 => judgment::ReleaseKind::Hit300,
            crate::types::JudgmentKind::Hit200 => judgment::ReleaseKind::Hit200,
            crate::types::JudgmentKind::Hit100 => judgment::ReleaseKind::Hit100,
            crate::types::JudgmentKind::Hit50 => judgment::ReleaseKind::Hit50,
            crate::types::JudgmentKind::Miss => judgment::ReleaseKind::Miss,
        }
    }
    pub(crate) fn compute_combo_computation(
        &self,
        renderer: &ReplayRenderer,
        hit_objects: &[crate::types::HitObject],
        render_judgments: &[RenderJudgment],
        score_judgments: &[judgment::ScoreJudgmentEvent],
        ln_releases: &HashMap<usize, judgment::LnReleaseInfo>,
        ln_debug: &HashMap<usize, judgment::LnDebugInfo>,
        windows: &HitWindows,
        score_mode: judgment::ScoreMode,
        _mods: u32,
    ) -> ComboComputation {
        let mut head_judgments: HashMap<usize, (judgment::ReleaseKind, Option<i32>)> =
            HashMap::with_capacity(render_judgments.len());
        for j in render_judgments {
            let rel_kind = Self::judgment_kind_to_rel_kind(j.kind);
            head_judgments.insert(j.idx, (rel_kind, j.press_time));
        }
        let (ln_ticks_raw, ln_breaks_raw) = judgment::gen_ln_combo(
            hit_objects,
            &head_judgments,
            ln_releases,
            ln_debug,
            windows,
            score_mode,
        );
        let ln_ticks: Vec<crate::renderer::replay_renderer::LnComboTick> = ln_ticks_raw
            .into_iter()
            .map(|t| crate::renderer::replay_renderer::LnComboTick {
                time: t.time,
                ln_idx: t.ln_index,
            })
            .collect();
        let ln_breaks: Vec<crate::renderer::replay_renderer::LnComboBreak> = ln_breaks_raw
            .into_iter()
            .map(|b| crate::renderer::replay_renderer::LnComboBreak {
                time: b.time,
                ln_idx: b.ln_index,
            })
            .collect();
        let score_events = if score_mode.use_ext_ln_comb_evnt() {
            // This score mode already emits LN combo events; avoid adding synthetic ticks.
            renderer.precompute_score_events(score_judgments, &[], &[], score_mode)
        } else {
            renderer.precompute_score_events(score_judgments, &ln_ticks, &ln_breaks, score_mode)
        };
        let computed_max_combo = score_events
            .iter()
            .map(|e| e.combo_after)
            .max()
            .unwrap_or(0);
        let combo_before_breaks = score_events
            .iter()
            .filter_map(|e| e.combo_break_start)
            .collect();
        ComboComputation {
            ln_ticks,
            ln_breaks,
            score_events,
            computed_max_combo,
            combo_before_breaks,
        }
    }
    #[expect(
        dead_code,
        reason = "unit tests exercise the generic fail cutoff without replay metadata"
    )]
    pub(crate) fn build_effective_replay_outcome(
        &self,
        score_judgments: &[judgment::ScoreJudgmentEvent],
        combo_data: &ComboComputation,
        health_timeline: &crate::renderer::HealthTimeline,
        score_mode: judgment::ScoreMode,
    ) -> EffectiveReplayOutcome {
        self.build_effective_replay_outcome_inner(
            score_judgments,
            combo_data,
            health_timeline,
            score_mode,
            None,
        )
    }
    pub(crate) fn build_effective_replay_outcome_for_replay(
        &self,
        score_judgments: &[judgment::ScoreJudgmentEvent],
        combo_data: &ComboComputation,
        health_timeline: &crate::renderer::HealthTimeline,
        score_mode: judgment::ScoreMode,
        replay: &ReplayData,
    ) -> EffectiveReplayOutcome {
        self.build_effective_replay_outcome_inner(
            score_judgments,
            combo_data,
            health_timeline,
            score_mode,
            Some(replay),
        )
    }
    fn build_effective_replay_outcome_inner(
        &self,
        score_judgments: &[judgment::ScoreJudgmentEvent],
        combo_data: &ComboComputation,
        health_timeline: &crate::renderer::HealthTimeline,
        score_mode: judgment::ScoreMode,
        replay: Option<&ReplayData>,
    ) -> EffectiveReplayOutcome {
        let cutoff = self.resolve_effective_score_cutoff(score_judgments, health_timeline, replay);
        let effective_score_judgments = match cutoff.score_judgment_count {
            Some(count) => score_judgments.iter().take(count).copied().collect(),
            None => match cutoff.time_ms {
                Some(cutoff_time_ms) => score_judgments
                    .iter()
                    .copied()
                    .filter(|judgment| judgment.event_time <= cutoff_time_ms)
                    .collect(),
                None => score_judgments.to_vec(),
            },
        };
        let effective_combo_data = match cutoff.time_ms {
            Some(cutoff_time_ms) => {
                // Results after fail use the trusted replay scoring cutoff, not the full map tail.
                let score_events = combo_data
                    .score_events
                    .iter()
                    .filter(|event| {
                        if let Some(count) = cutoff.score_judgment_count {
                            if let Some(judgment_idx) = event.score_judgment_idx {
                                return judgment_idx < count;
                            }
                        }
                        event.time <= cutoff_time_ms
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let computed_max_combo = score_events
                    .iter()
                    .map(|event| event.combo_after)
                    .max()
                    .unwrap_or(0);
                let combo_before_breaks = score_events
                    .iter()
                    .filter_map(|event| event.combo_break_start)
                    .collect::<Vec<_>>();
                ComboComputation {
                    ln_ticks: combo_data
                        .ln_ticks
                        .iter()
                        .copied()
                        .filter(|tick| tick.time <= cutoff_time_ms)
                        .collect(),
                    ln_breaks: combo_data
                        .ln_breaks
                        .iter()
                        .copied()
                        .filter(|combo_break| combo_break.time <= cutoff_time_ms)
                        .collect(),
                    score_events,
                    computed_max_combo,
                    combo_before_breaks,
                }
            }
            None => combo_data.clone(),
        };
        let computed_stats = self.tally_score_judgments(&effective_score_judgments);
        let computed_accuracy = self.calculate_accuracy_from_stats(computed_stats, score_mode);
        let computed_score = effective_combo_data
            .score_events
            .last()
            .map(|event| ReplayRenderer::score_from_float(event.cumulative_score))
            .unwrap_or(0);
        let final_combo = effective_combo_data
            .score_events
            .last()
            .map(|event| event.combo_after)
            .unwrap_or(0);
        let score_state_end_time_ms = health_timeline.fail_time_ms.and_then(|fail_time_ms| {
            effective_combo_data
                .score_events
                .last()
                .map(|event| event.time)
                .filter(|last_score_time| *last_score_time > fail_time_ms)
        });
        EffectiveReplayOutcome {
            score_judgments: effective_score_judgments,
            combo_data: effective_combo_data,
            computed_stats,
            computed_accuracy,
            computed_score,
            final_combo,
            score_state_end_time_ms,
        }
    }
    fn resolve_effective_score_cutoff(
        &self,
        score_judgments: &[judgment::ScoreJudgmentEvent],
        health_timeline: &crate::renderer::HealthTimeline,
        replay: Option<&ReplayData>,
    ) -> EffectiveScoreCutoff {
        let Some(fail_time_ms) = health_timeline.fail_time_ms else {
            return EffectiveScoreCutoff::default();
        };
        let fallback = EffectiveScoreCutoff {
            time_ms: Some(fail_time_ms),
            score_judgment_count: None,
        };
        let Some(replay) = replay else {
            return fallback;
        };
        if replay.origin != crate::types::ReplayOrigin::StableLegacy {
            return fallback;
        }
        let target_stats = replay.basic_statistics();
        let target_total = target_stats.total() as usize;
        if target_total == 0 {
            return fallback;
        }
        let mut fail_stats = ReplayBasicStatistics::default();
        for judgment in score_judgments {
            if judgment.event_time > fail_time_ms {
                break;
            }
            add_score_judgment_to_stats(&mut fail_stats, *judgment);
        }
        let fail_count = fail_stats.total() as usize;
        if fail_count >= target_total {
            return fallback;
        }
        let Some(cutoff_judgment) = score_judgments.get(target_total.saturating_sub(1)) else {
            return fallback;
        };
        if cutoff_judgment.event_time <= fail_time_ms {
            return fallback;
        }
        let cutoff_stats = self.tally_score_judgments(&score_judgments[..target_total]);
        if cutoff_stats == target_stats {
            println!(
                "   [health] replay summary extends scoring cutoff {}ms -> {}ms ({} -> {} judgments)",
                fail_time_ms,
                cutoff_judgment.event_time,
                fail_count,
                target_total
            );
        } else {
            println!(
                "   [health] replay total extends scoring cutoff {}ms -> {}ms ({} -> {} judgments; summary differs)",
                fail_time_ms,
                cutoff_judgment.event_time,
                fail_count,
                target_total
            );
        }
        EffectiveScoreCutoff {
            time_ms: Some(cutoff_judgment.event_time),
            score_judgment_count: Some(target_total),
        }
    }
    pub(crate) fn tally_score_judgments(
        &self,
        score_judgments: &[judgment::ScoreJudgmentEvent],
    ) -> ReplayBasicStatistics {
        let mut stats = ReplayBasicStatistics::default();
        for judgment in score_judgments {
            add_score_judgment_to_stats(&mut stats, *judgment);
        }
        stats
    }
    pub(crate) fn calculate_accuracy_from_stats(
        &self,
        stats: ReplayBasicStatistics,
        score_mode: judgment::ScoreMode,
    ) -> f32 {
        let total = stats.total();
        if total == 0 {
            return 100.0;
        }
        let max_acc_weight = score_mode.accuracy_max_per_hit();
        let weighted_hits = stats.weighted_hits(max_acc_weight);
        weighted_hits as f32 / (total * max_acc_weight) as f32 * 100.0
    }
    pub(crate) fn build_results_timing_summary(
        &self,
        hit_objects: &[crate::types::HitObject],
        render_judgments: &[RenderJudgment],
        ln_release_by_idx: &[Option<LnReleaseInfo>],
        score_judgments: &[judgment::ScoreJudgmentEvent],
        rate_profile: &crate::utils::mods::PlaybackRateProfile,
    ) -> crate::results::ResultsTimingSummary {
        crate::results::summarize_timing_from_render_data(
            hit_objects,
            render_judgments,
            ln_release_by_idx,
            score_judgments,
            rate_profile,
        )
    }
    pub(crate) fn has_replay_summary_mismatch(
        &self,
        replay: &ReplayData,
        computed_stats: ReplayBasicStatistics,
    ) -> bool {
        replay.basic_statistics() != computed_stats
    }
    pub(crate) fn generate_autoplay_key_actions_for_replay(
        &self,
        beatmap: &Beatmap,
        replay: &ReplayData,
    ) -> Result<Vec<KeyAction>, ConvertError> {
        let mut autoplay_input_beatmap = beatmap.clone();
        self.apply_replay_beatmap_conversion_mods(&mut autoplay_input_beatmap, replay)?;
        Ok(self.generate_autoplay_key_actions(&autoplay_input_beatmap))
    }
    pub(crate) fn generate_autoplay_key_actions(&self, beatmap: &Beatmap) -> Vec<KeyAction> {
        const TAP_MS: i32 = 20;
        let key_count = self.effective_key_count(beatmap);
        let mut by_col: Vec<Vec<&crate::types::HitObject>> = vec![Vec::new(); key_count as usize];
        for ho in &beatmap.hit_objects {
            let col = ho.column as usize;
            if col < by_col.len() {
                by_col[col].push(ho);
            }
        }
        for notes in &mut by_col {
            notes.sort_by_key(|h| h.time);
        }
        let mut actions: Vec<KeyAction> = Vec::with_capacity(beatmap.hit_objects.len() * 2);
        for (col, notes) in by_col.iter().enumerate() {
            let mut group_start = 0;
            while group_start < notes.len() {
                let press_time = notes[group_start].time;
                let group_end = notes[group_start..]
                    .iter()
                    .take_while(|candidate| candidate.time == press_time)
                    .count()
                    + group_start;
                let next_distinct_time = notes.get(group_end).map(|n| n.time);
                let simultaneous_hold_anchor = notes[group_start..group_end]
                    .iter()
                    .filter(|candidate| candidate.is_long_note())
                    .filter_map(|candidate| candidate.end_time)
                    .max();
                for ho in &notes[group_start..group_end] {
                    let mut rel_time = if ho.is_long_note() {
                        ho.end_time.unwrap_or(ho.time)
                    } else {
                        ho.time + TAP_MS
                    };
                    if !ho.is_long_note() {
                        if let Some(nt) = next_distinct_time {
                            if rel_time >= nt {
                                rel_time = nt - 1;
                            }
                        }
                        if let Some(anchor) = simultaneous_hold_anchor {
                            // Simultaneous taps should not release keys before a same-time hold ends.
                            rel_time = rel_time.max(anchor);
                        }
                        if rel_time < press_time {
                            rel_time = press_time;
                        }
                    } else if let Some(nt) = next_distinct_time {
                        if nt == rel_time {
                            rel_time = (rel_time - 1).max(press_time);
                        }
                    }
                    actions.push(KeyAction {
                        time: press_time,
                        column: col as u8,
                        pressed: true,
                        keys_mask: 0,
                    });
                    if rel_time >= press_time {
                        actions.push(KeyAction {
                            time: rel_time,
                            column: col as u8,
                            pressed: false,
                            keys_mask: 0,
                        });
                    }
                }
                group_start = group_end;
            }
        }
        actions.sort_by(|a, b| {
            a.time
                .cmp(&b.time)
                .then_with(|| a.column.cmp(&b.column))
                .then_with(|| b.pressed.cmp(&a.pressed))
        });
        let mut mask: u32 = 0;
        for action in &mut actions {
            let bit = 1u32 << action.column;
            if action.pressed {
                mask |= bit;
            } else {
                mask &= !bit;
            }
            action.keys_mask = mask;
        }
        actions
    }
    pub(crate) fn apply_autoplay_stats(
        &self,
        replay: &mut ManiaReplayData,
        score_judgments: &[judgment::ScoreJudgmentEvent],
        score_mode: judgment::ScoreMode,
        computed_max_combo: u32,
    ) {
        let mut max_count: u32 = 0;
        let mut h300_count: u32 = 0;
        let mut h200_count: u32 = 0;
        let mut h100_count: u32 = 0;
        let mut h50_count: u32 = 0;
        let mut miss_count: u32 = 0;
        for j in score_judgments {
            match j.kind {
                crate::types::JudgmentKind::Max => max_count += 1,
                crate::types::JudgmentKind::Hit300 => h300_count += 1,
                crate::types::JudgmentKind::Hit200 => h200_count += 1,
                crate::types::JudgmentKind::Hit100 => h100_count += 1,
                crate::types::JudgmentKind::Hit50 => h50_count += 1,
                crate::types::JudgmentKind::Miss => miss_count += 1,
            }
        }
        let to_u16 = |v: u32| -> u16 { v.min(u16::MAX as u32) as u16 };
        replay.replay.count_geki = to_u16(max_count);
        replay.replay.count_300 = to_u16(h300_count);
        replay.replay.count_katu = to_u16(h200_count);
        replay.replay.count_100 = to_u16(h100_count);
        replay.replay.count_50 = to_u16(h50_count);
        replay.replay.count_miss = to_u16(miss_count);
        replay.replay.total_score =
            ReplayRenderer::calculate_final_score(score_judgments, score_mode);
        replay.replay.max_combo = to_u16(computed_max_combo);
        replay.replay.perfect_combo = miss_count == 0;
    }
}

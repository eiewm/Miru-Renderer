use super::*;
impl ManiaVideoConverter {
    pub fn analyze_judgments_only(
        &self,
        osr_path: &Path,
        opts: &ResolveOpts,
    ) -> Result<Option<ReplayIntegrityReport>, ConvertError> {
        let start = std::time::Instant::now();
        println!("-> dry-run: analyzing judgments");
        let mut replay =
            parser::parse_osr_file(osr_path).map_err(|e| ConvertError::Parse(e.to_string()))?;
        println!(
            "   player: {}, actions: {}",
            replay.replay.player_name,
            replay.key_actions.len()
        );
        self.ensure_rd_not_enabled(&replay.replay)?;
        let fail_conditions = self.resolve_replay_fail_condition_mod(&replay.replay)?;
        let (beatmap_path, _, _) = self.resolve_beatmap(&replay, opts)?;
        let mut beatmap = parser::parse_osu_file(&beatmap_path)
            .map_err(|e| ConvertError::Parse(e.to_string()))?;
        beatmap = self.resolve_playable_mania_beatmap(&replay.replay, beatmap, &beatmap_path)?;
        let key_count = self.effective_key_count(&beatmap);
        replay.key_actions = ManiaReplayData::derive_key_actions(&replay.frames, key_count);
        self.apply_replay_beatmap_conversion_mods(&mut beatmap, &replay.replay)?;
        let playback = self.resolve_replay_playback_settings(&replay.replay, &beatmap)?;
        println!(
            "   {} - {} [{}]",
            beatmap.metadata.artist, beatmap.metadata.title, beatmap.metadata.version
        );
        let star_rating = self.enforce_star_limit(&beatmap_path)?;
        println!("   stars: {:.2}", star_rating);
        let (windows, score_mode_ctx) =
            judgment::res_sco_mod_for_repl(beatmap.difficulty.od, &replay.replay);
        println!(
            "   [judgment] mode: {:?} (requested: {:?})",
            score_mode_ctx.effective_mode, score_mode_ctx.requested_mode
        );
        let (
            judgments_for_combo,
            ln_releases_for_combo,
            ln_debug_for_combo,
            score_judgments_for_combo,
        ) = if self.settings.note_debug || self.settings.all_presses {
            let debug_result = judgment::calc_jdbg_mode_for_replay(
                &replay.replay,
                &beatmap.hit_objects,
                &replay.key_actions,
                &windows,
                score_mode_ctx,
            );
            let judgments: Vec<_> = debug_result
                .judgments
                .iter()
                .map(judgment::Judgment::from)
                .collect();
            self.print_hit_windows(&windows, beatmap.difficulty.od);
            if self.settings.note_debug {
                self.print_note_debug_detailed(
                    &beatmap,
                    &debug_result.judgments,
                    &debug_result.ln_releases,
                    &windows,
                    Some(&debug_result.press_status),
                    score_mode_ctx.effective_mode,
                );
            }
            if self.settings.all_presses {
                self.print_all_presses(&debug_result.press_status, &beatmap);
            }
            (
                judgments,
                debug_result.ln_releases,
                debug_result.ln_debug,
                debug_result.score_judgments,
            )
        } else {
            let result = judgment::calc_judg_mode_for_replay(
                &replay.replay,
                &beatmap.hit_objects,
                &replay.key_actions,
                &windows,
                score_mode_ctx,
            );
            self.print_hit_windows(&windows, beatmap.difficulty.od);
            if self.settings.ln_debug {
                self.print_ln_details(&beatmap, &result.judgments, &windows);
            }
            (
                result.judgments,
                result.ln_releases,
                result.ln_debug,
                result.score_judgments,
            )
        };
        let render_judgments = self.build_render_judgments(
            &beatmap.hit_objects,
            &judgments_for_combo,
            &ln_releases_for_combo,
        );
        let mut judgments_by_idx = vec![None; beatmap.hit_objects.len()];
        for judgment in &render_judgments {
            if judgment.idx < judgments_by_idx.len() {
                judgments_by_idx[judgment.idx] = Some(*judgment);
            }
        }
        let mut ln_release_by_idx = vec![None; beatmap.hit_objects.len()];
        for (&idx, info) in &ln_releases_for_combo {
            if idx < ln_release_by_idx.len() {
                ln_release_by_idx[idx] = Some(LnReleaseInfo::from(info));
            }
        }
        let combo_renderer = ReplayRenderer::new();
        let combo_data = self.compute_combo_computation(
            &combo_renderer,
            &beatmap.hit_objects,
            &render_judgments,
            &score_judgments_for_combo,
            &ln_releases_for_combo,
            &ln_debug_for_combo,
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
            &score_judgments_for_combo,
            fail_conditions,
        );
        if let Some(fail_time_ms) = health_timeline.fail_time_ms {
            println!(
                "   [health] fail_time={}ms hp_multiplier={:.3}",
                fail_time_ms, health_timeline.hp_multiplier_normal
            );
        }
        let effective_outcome = self.build_effective_replay_outcome_for_replay(
            &score_judgments_for_combo,
            &combo_data,
            &health_timeline,
            score_mode_ctx.effective_mode,
            &replay.replay,
        );
        let timing_summary = self.build_results_timing_summary(
            &beatmap.hit_objects,
            &render_judgments,
            &ln_release_by_idx,
            &effective_outcome.score_judgments,
            &playback.profile,
        );
        // Dry-run output keeps multiple UR definitions for comparing replay tools and score modes.
        let timing_summary_with_misses = {
            let mut deltas = Vec::new();
            for judgment in &render_judgments {
                let Some(hit_object) = beatmap.hit_objects.get(judgment.idx) else {
                    continue;
                };
                if let Some(press_time) = judgment.press_time {
                    let rate = playback
                        .profile
                        .rate_at_beatmap_time_ms(press_time as f64)
                        .abs()
                        .max(f64::EPSILON);
                    deltas.push(f64::from(press_time - hit_object.time) / rate);
                }
                if !judgment.is_ln {
                    continue;
                }
                let Some(release) = ln_release_by_idx.get(judgment.idx).copied().flatten() else {
                    continue;
                };
                let Some(release_time) = release.time else {
                    continue;
                };
                let rate = playback
                    .profile
                    .rate_at_beatmap_time_ms(release_time as f64)
                    .abs()
                    .max(f64::EPSILON);
                deltas.push(
                    f64::from(release_time - hit_object.end_time.unwrap_or(hit_object.time)) / rate,
                );
            }
            if deltas.is_empty() {
                crate::results::ResultsTimingSummary::default()
            } else {
                let mut early_sum = 0.0f64;
                let mut early_count = 0usize;
                let mut late_sum = 0.0f64;
                let mut late_count = 0usize;
                for &delta in &deltas {
                    if delta < 0.0 {
                        early_sum += delta;
                        early_count += 1;
                    } else {
                        late_sum += delta;
                        late_count += 1;
                    }
                }
                let mean = deltas.iter().sum::<f64>() / deltas.len() as f64;
                let variance = deltas
                    .iter()
                    .map(|delta| {
                        let centered = *delta - mean;
                        centered * centered
                    })
                    .sum::<f64>()
                    / deltas.len() as f64;
                crate::results::ResultsTimingSummary {
                    sample_count: deltas.len(),
                    avg_early_ms: if early_count > 0 {
                        (early_sum / early_count as f64) as f32
                    } else {
                        0.0
                    },
                    avg_late_ms: if late_count > 0 {
                        (late_sum / late_count as f64) as f32
                    } else {
                        0.0
                    },
                    unstable_rate: (variance.sqrt() * 10.0) as f32,
                }
            }
        };
        let summarize_variant = |include_heads: bool,
                                 include_tails: bool,
                                 include_misses: bool|
         -> crate::results::ResultsTimingSummary {
            let mut deltas = Vec::new();
            for judgment in &render_judgments {
                let Some(hit_object) = beatmap.hit_objects.get(judgment.idx) else {
                    continue;
                };
                if include_heads {
                    let head_ok = include_misses || judgment.kind.score_value() > 0;
                    if head_ok {
                        if let Some(press_time) = judgment.press_time {
                            let rate = playback
                                .profile
                                .rate_at_beatmap_time_ms(press_time as f64)
                                .abs()
                                .max(f64::EPSILON);
                            deltas.push(f64::from(press_time - hit_object.time) / rate);
                        }
                    }
                }
                if include_tails && judgment.is_ln {
                    let Some(release) = ln_release_by_idx.get(judgment.idx).copied().flatten()
                    else {
                        continue;
                    };
                    let Some(release_time) = release.time else {
                        continue;
                    };
                    let tail_ok = include_misses
                        || !matches!(
                            release.kind,
                            crate::renderer::ReleaseKind::Miss | crate::renderer::ReleaseKind::None
                        );
                    if tail_ok {
                        let rate = playback
                            .profile
                            .rate_at_beatmap_time_ms(release_time as f64)
                            .abs()
                            .max(f64::EPSILON);
                        deltas.push(
                            f64::from(
                                release_time - hit_object.end_time.unwrap_or(hit_object.time),
                            ) / rate,
                        );
                    }
                }
            }
            if deltas.is_empty() {
                crate::results::ResultsTimingSummary::default()
            } else {
                let mut early_sum = 0.0f64;
                let mut early_count = 0usize;
                let mut late_sum = 0.0f64;
                let mut late_count = 0usize;
                for &delta in &deltas {
                    if delta < 0.0 {
                        early_sum += delta;
                        early_count += 1;
                    } else {
                        late_sum += delta;
                        late_count += 1;
                    }
                }
                let mean = deltas.iter().sum::<f64>() / deltas.len() as f64;
                let variance = deltas
                    .iter()
                    .map(|delta| {
                        let centered = *delta - mean;
                        centered * centered
                    })
                    .sum::<f64>()
                    / deltas.len() as f64;
                crate::results::ResultsTimingSummary {
                    sample_count: deltas.len(),
                    avg_early_ms: if early_count > 0 {
                        (early_sum / early_count as f64) as f32
                    } else {
                        0.0
                    },
                    avg_late_ms: if late_count > 0 {
                        (late_sum / late_count as f64) as f32
                    } else {
                        0.0
                    },
                    unstable_rate: (variance.sqrt() * 10.0) as f32,
                }
            }
        };
        let heads_only = summarize_variant(true, false, false);
        let heads_only_with_misses = summarize_variant(true, false, true);
        let tails_only = summarize_variant(false, true, false);
        let taps_plus_tails = {
            let mut deltas = Vec::new();
            for judgment in &render_judgments {
                let Some(hit_object) = beatmap.hit_objects.get(judgment.idx) else {
                    continue;
                };
                if !judgment.is_ln {
                    if judgment.kind.score_value() > 0 {
                        if let Some(press_time) = judgment.press_time {
                            let rate = playback
                                .profile
                                .rate_at_beatmap_time_ms(press_time as f64)
                                .abs()
                                .max(f64::EPSILON);
                            deltas.push(f64::from(press_time - hit_object.time) / rate);
                        }
                    }
                    continue;
                }
                let Some(release) = ln_release_by_idx.get(judgment.idx).copied().flatten() else {
                    continue;
                };
                let Some(release_time) = release.time else {
                    continue;
                };
                if matches!(
                    release.kind,
                    crate::renderer::ReleaseKind::Miss | crate::renderer::ReleaseKind::None
                ) {
                    continue;
                }
                let rate = playback
                    .profile
                    .rate_at_beatmap_time_ms(release_time as f64)
                    .abs()
                    .max(f64::EPSILON);
                deltas.push(
                    f64::from(release_time - hit_object.end_time.unwrap_or(hit_object.time)) / rate,
                );
            }
            if deltas.is_empty() {
                crate::results::ResultsTimingSummary::default()
            } else {
                let mut early_sum = 0.0f64;
                let mut early_count = 0usize;
                let mut late_sum = 0.0f64;
                let mut late_count = 0usize;
                for &delta in &deltas {
                    if delta < 0.0 {
                        early_sum += delta;
                        early_count += 1;
                    } else {
                        late_sum += delta;
                        late_count += 1;
                    }
                }
                let mean = deltas.iter().sum::<f64>() / deltas.len() as f64;
                let variance = deltas
                    .iter()
                    .map(|delta| {
                        let centered = *delta - mean;
                        centered * centered
                    })
                    .sum::<f64>()
                    / deltas.len() as f64;
                crate::results::ResultsTimingSummary {
                    sample_count: deltas.len(),
                    avg_early_ms: if early_count > 0 {
                        (early_sum / early_count as f64) as f32
                    } else {
                        0.0
                    },
                    avg_late_ms: if late_count > 0 {
                        (late_sum / late_count as f64) as f32
                    } else {
                        0.0
                    },
                    unstable_rate: (variance.sqrt() * 10.0) as f32,
                }
            }
        };
        let taps_plus_long_tails = |min_duration_ms: i32| -> crate::results::ResultsTimingSummary {
            let mut deltas = Vec::new();
            for judgment in &render_judgments {
                let Some(hit_object) = beatmap.hit_objects.get(judgment.idx) else {
                    continue;
                };
                if !judgment.is_ln {
                    if judgment.kind.score_value() > 0 {
                        if let Some(press_time) = judgment.press_time {
                            let rate = playback
                                .profile
                                .rate_at_beatmap_time_ms(press_time as f64)
                                .abs()
                                .max(f64::EPSILON);
                            deltas.push(f64::from(press_time - hit_object.time) / rate);
                        }
                    }
                    continue;
                }
                let duration = hit_object.end_time.unwrap_or(hit_object.time) - hit_object.time;
                if duration < min_duration_ms {
                    continue;
                }
                let Some(release) = ln_release_by_idx.get(judgment.idx).copied().flatten() else {
                    continue;
                };
                let Some(release_time) = release.time else {
                    continue;
                };
                if matches!(
                    release.kind,
                    crate::renderer::ReleaseKind::Miss | crate::renderer::ReleaseKind::None
                ) {
                    continue;
                }
                let rate = playback
                    .profile
                    .rate_at_beatmap_time_ms(release_time as f64)
                    .abs()
                    .max(f64::EPSILON);
                deltas.push(
                    f64::from(release_time - hit_object.end_time.unwrap_or(hit_object.time)) / rate,
                );
            }
            if deltas.is_empty() {
                crate::results::ResultsTimingSummary::default()
            } else {
                let mut early_sum = 0.0f64;
                let mut early_count = 0usize;
                let mut late_sum = 0.0f64;
                let mut late_count = 0usize;
                for &delta in &deltas {
                    if delta < 0.0 {
                        early_sum += delta;
                        early_count += 1;
                    } else {
                        late_sum += delta;
                        late_count += 1;
                    }
                }
                let mean = deltas.iter().sum::<f64>() / deltas.len() as f64;
                let variance = deltas
                    .iter()
                    .map(|delta| {
                        let centered = *delta - mean;
                        centered * centered
                    })
                    .sum::<f64>()
                    / deltas.len() as f64;
                crate::results::ResultsTimingSummary {
                    sample_count: deltas.len(),
                    avg_early_ms: if early_count > 0 {
                        (early_sum / early_count as f64) as f32
                    } else {
                        0.0
                    },
                    avg_late_ms: if late_count > 0 {
                        (late_sum / late_count as f64) as f32
                    } else {
                        0.0
                    },
                    unstable_rate: (variance.sqrt() * 10.0) as f32,
                }
            }
        };
        let taps_plus_tails_127 = taps_plus_long_tails(windows.hit50);
        let taps_plus_tails_254 = taps_plus_long_tails(windows.hit50 * 2);
        let head_samples = render_judgments
            .iter()
            .filter(|judgment| judgment.kind.score_value() > 0 && judgment.press_time.is_some())
            .count();
        let tail_samples = render_judgments
            .iter()
            .filter(|judgment| {
                judgment.is_ln
                    && judgment
                        .idx
                        .checked_sub(0)
                        .and_then(|idx| ln_release_by_idx.get(idx))
                        .copied()
                        .flatten()
                        .is_some_and(|release| {
                            release.time.is_some()
                                && !matches!(
                                    release.kind,
                                    crate::renderer::ReleaseKind::Miss
                                        | crate::renderer::ReleaseKind::None
                                )
                        })
            })
            .count();
        println!("\n[Timing Summary]");
        println!("  head_samples: {}", head_samples);
        println!("  tail_samples: {}", tail_samples);
        println!("  total_samples: {}", timing_summary.sample_count);
        println!(
            "  error: {:.2}ms - {:.2}ms avg",
            timing_summary.avg_early_ms, timing_summary.avg_late_ms
        );
        println!("  unstable_rate: {:.2}", timing_summary.unstable_rate);
        println!(
            "  with_misses: {:.2}ms - {:.2}ms avg / {:.2} UR ({} samples)",
            timing_summary_with_misses.avg_early_ms,
            timing_summary_with_misses.avg_late_ms,
            timing_summary_with_misses.unstable_rate,
            timing_summary_with_misses.sample_count
        );
        println!(
            "  heads_only: {:.2}ms - {:.2}ms avg / {:.2} UR ({} samples)",
            heads_only.avg_early_ms,
            heads_only.avg_late_ms,
            heads_only.unstable_rate,
            heads_only.sample_count
        );
        println!(
            "  heads_only_with_misses: {:.2}ms - {:.2}ms avg / {:.2} UR ({} samples)",
            heads_only_with_misses.avg_early_ms,
            heads_only_with_misses.avg_late_ms,
            heads_only_with_misses.unstable_rate,
            heads_only_with_misses.sample_count
        );
        println!(
            "  tails_only: {:.2}ms - {:.2}ms avg / {:.2} UR ({} samples)",
            tails_only.avg_early_ms,
            tails_only.avg_late_ms,
            tails_only.unstable_rate,
            tails_only.sample_count
        );
        println!(
            "  taps_plus_tails: {:.2}ms - {:.2}ms avg / {:.2} UR ({} samples)",
            taps_plus_tails.avg_early_ms,
            taps_plus_tails.avg_late_ms,
            taps_plus_tails.unstable_rate,
            taps_plus_tails.sample_count
        );
        println!(
            "  taps_plus_tails_127: {:.2}ms - {:.2}ms avg / {:.2} UR ({} samples)",
            taps_plus_tails_127.avg_early_ms,
            taps_plus_tails_127.avg_late_ms,
            taps_plus_tails_127.unstable_rate,
            taps_plus_tails_127.sample_count
        );
        println!(
            "  taps_plus_tails_254: {:.2}ms - {:.2}ms avg / {:.2} UR ({} samples)",
            taps_plus_tails_254.avg_early_ms,
            taps_plus_tails_254.avg_late_ms,
            taps_plus_tails_254.unstable_rate,
            taps_plus_tails_254.sample_count
        );
        self.print_score_judgment_summary(&effective_outcome.score_judgments, Some(&replay.replay));
        self.print_combo_parseable(&effective_outcome.combo_data);
        let computed_stats = effective_outcome.computed_stats;
        let replay_integrity = Some(ReplayIntegrityReport {
            has_summary_mismatch: self.has_replay_summary_mismatch(&replay.replay, computed_stats),
        });
        println!("\ncompleted in {}ms", start.elapsed().as_millis());
        Ok(replay_integrity)
    }
    pub fn analyze_autoplay_judgments_only(
        &self,
        beatmap_path: &Path,
        opts: &ResolveOpts,
    ) -> Result<Option<ReplayIntegrityReport>, ConvertError> {
        let start = std::time::Instant::now();
        println!("-> dry-run: analyzing autoplay judgments");
        let beatmap =
            parser::parse_osu_file(beatmap_path).map_err(|e| ConvertError::Parse(e.to_string()))?;
        if beatmap.metadata.mode != 3 {
            return Err(ConvertError::Parse(format!(
                "beatmap is not osu!mania (Mode={})",
                beatmap.metadata.mode
            )));
        }
        println!(
            "   {} - {} [{}]",
            beatmap.metadata.artist, beatmap.metadata.title, beatmap.metadata.version
        );
        let star_rating = self.enforce_star_limit(beatmap_path)?;
        println!("   stars: {:.2}", star_rating);
        let autoplay_mods = self.normalized_autoplay_mods(opts)?;
        let mut replay = self.build_autoplay_replay_data(autoplay_mods.as_ref(), Vec::new());
        replay.key_actions =
            self.generate_autoplay_key_actions_for_replay(&beatmap, &replay.replay)?;
        println!("   actions: {}", replay.key_actions.len());
        let (windows, score_mode_ctx) =
            judgment::res_sco_mod_for_repl(beatmap.difficulty.od, &replay.replay);
        if self.settings.note_debug || self.settings.all_presses {
            let debug_result = judgment::calc_jdbg_mode_for_replay(
                &replay.replay,
                &beatmap.hit_objects,
                &replay.key_actions,
                &windows,
                score_mode_ctx,
            );
            let judgments: Vec<_> = debug_result
                .judgments
                .iter()
                .map(judgment::Judgment::from)
                .collect();
            self.print_judgment_summary(&windows, &judgments, beatmap.difficulty.od);
            if self.settings.note_debug {
                self.print_note_debug_detailed(
                    &beatmap,
                    &debug_result.judgments,
                    &debug_result.ln_releases,
                    &windows,
                    Some(&debug_result.press_status),
                    score_mode_ctx.effective_mode,
                );
            }
            if self.settings.all_presses {
                self.print_all_presses(&debug_result.press_status, &beatmap);
            }
        } else {
            let result = judgment::calc_judg_mode_for_replay(
                &replay.replay,
                &beatmap.hit_objects,
                &replay.key_actions,
                &windows,
                score_mode_ctx,
            );
            self.print_judgment_summary(&windows, &result.judgments, beatmap.difficulty.od);
            if self.settings.ln_debug {
                self.print_ln_details(&beatmap, &result.judgments, &windows);
            }
        }
        println!("\ncompleted in {}ms", start.elapsed().as_millis());
        Ok(None)
    }
    pub(crate) fn print_combo_parseable(&self, combo: &ComboComputation) {
        println!("[combo] max={}", combo.computed_max_combo);
        println!("[combo] break_count={}", combo.combo_before_breaks.len());
        let break_csv = combo
            .combo_before_breaks
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(",");
        println!("[combo] break_combos={}", break_csv);
    }
    pub(crate) fn print_score_judgment_summary(
        &self,
        score_judgments: &[judgment::ScoreJudgmentEvent],
        replay: Option<&ReplayData>,
    ) {
        let computed = self.tally_score_judgments(score_judgments);
        println!("\n[Score Summary]");
        println!("  MAX:  {}", computed.max);
        println!("  300:  {}", computed.hit300);
        println!("  200:  {}", computed.hit200);
        println!("  100:  {}", computed.hit100);
        println!("  50:   {}", computed.hit50);
        println!("  MISS: {}", computed.miss);
        println!("  Total: {}", computed.total());
        if let Some(replay) = replay {
            let replay_stats = replay.basic_statistics();
            println!(
                "  [replay] MAX={} 300={} 200={} 100={} 50={} MISS={} Total={}",
                replay_stats.max,
                replay_stats.hit300,
                replay_stats.hit200,
                replay_stats.hit100,
                replay_stats.hit50,
                replay_stats.miss,
                replay_stats.total()
            );
            if computed != replay_stats {
                println!(
                    "  [delta] MAX={:+} 300={:+} 200={:+} 100={:+} 50={:+} MISS={:+}",
                    computed.max as i64 - replay_stats.max as i64,
                    computed.hit300 as i64 - replay_stats.hit300 as i64,
                    computed.hit200 as i64 - replay_stats.hit200 as i64,
                    computed.hit100 as i64 - replay_stats.hit100 as i64,
                    computed.hit50 as i64 - replay_stats.hit50 as i64,
                    computed.miss as i64 - replay_stats.miss as i64
                );
            } else {
                println!("  [delta] matches replay statistics");
            }
        }
    }
    pub(crate) fn print_hit_windows(&self, w: &HitWindows, od: f32) {
        println!("\n{}", "=".repeat(60));
        println!("[Hit Windows] OD={:.1}", od);
        println!("  MAX:  ±{}ms", w.max);
        println!("  300:  ±{}ms", w.hit300);
        println!("  200:  ±{}ms", w.hit200);
        println!("  100:  ±{}ms", w.hit100);
        println!("  50:   ±{}ms", w.hit50);
        println!("{}", "=".repeat(60));
    }
    pub(crate) fn print_judgment_summary(&self, w: &HitWindows, judgments: &[Judgment], od: f32) {
        println!("\n{}", "=".repeat(60));
        println!("[Hit Windows] OD={:.1}", od);
        println!("  MAX:  ±{}ms", w.max);
        println!("  300:  ±{}ms", w.hit300);
        println!("  200:  ±{}ms", w.hit200);
        println!("  100:  ±{}ms", w.hit100);
        println!("  50:   ±{}ms", w.hit50);
        println!("{}", "=".repeat(60));
        let mut counts = [0u32; 6];
        for j in judgments {
            match j.kind.as_str() {
                "Max" => counts[0] += 1,
                "Hit300" => counts[1] += 1,
                "Hit200" => counts[2] += 1,
                "Hit100" => counts[3] += 1,
                "Hit50" => counts[4] += 1,
                _ => counts[5] += 1,
            }
        }
        println!("\n[Summary]");
        println!("  MAX:  {}", counts[0]);
        println!("  300:  {}", counts[1]);
        println!("  200:  {}", counts[2]);
        println!("  100:  {}", counts[3]);
        println!("  50:   {}", counts[4]);
        println!("  MISS: {}", counts[5]);
        println!("  Total: {}", judgments.len());
    }
    pub(crate) fn print_note_debug_detailed(
        &self,
        beatmap: &Beatmap,
        judgments: &[judgment::InternalJudgment],
        ln_releases: &std::collections::HashMap<usize, judgment::LnReleaseInfo>,
        windows: &HitWindows,
        press_status: Option<&[judgment::PressStatus]>,
        score_mode: judgment::ScoreMode,
    ) {
        let print_note = |idx: usize,
                          ho: &crate::types::HitObject,
                          judgments: &[judgment::InternalJudgment],
                          ln_releases: &std::collections::HashMap<
            usize,
            judgment::LnReleaseInfo,
        >,
                          windows: &HitWindows,
                          press_status: Option<&[judgment::PressStatus]>| {
            let is_ln = ho.end_time.map(|et| et > ho.time + 2).unwrap_or(false);
            let jj = judgments.iter().find(|j| j.index == idx);
            if is_ln {
                let end_time = ho.end_time.unwrap_or(ho.time);
                let duration = end_time - ho.time;
                let rel = ln_releases.get(&idx);
                let split_ln = score_mode.uses_split_ln_events();
                let tail_kind = rel
                    .and_then(|info| info.kind.as_judgment_kind())
                    .map(score_judgment_label)
                    .unwrap_or("MISS");
                println!("\n[LN #{}] col {}", idx, ho.column);
                println!("  head:     {}ms", ho.time);
                println!("  end:      {}ms", end_time);
                println!("  duration: {}ms", duration);
                if let Some(jj) = jj {
                    if let Some(pt) = jj.press_time {
                        let delta = pt - ho.time;
                        let kind = classify_delta(delta.abs(), windows);
                        println!("  press:    {}ms ({:+}ms, {})", pt, delta, kind);
                    } else {
                        println!("  press:    NONE");
                    }
                    println!("  assigned: {:?}", jj.kind);
                } else {
                    println!("  press:    NONE");
                    println!("  assigned: NOT FOUND");
                }
                let raw_release = jj.and_then(|head| head.press_time).and_then(|pt| {
                    press_status.and_then(|events| {
                        events
                            .iter()
                            .filter(|ev| {
                                ev.column == ho.column
                                    && ev.time > pt
                                    && ev.status.starts_with("RELEASE")
                            })
                            .map(|ev| ev.time)
                            .min()
                    })
                });
                let miss_tail_rel = rel.and_then(|info| info.time.filter(|&time| time <= end_time));
                let display_release_time = if split_ln {
                    // Split-LN misses report at the tail boundary, not at the next raw release.
                    match rel {
                        Some(info) if info.kind == judgment::ReleaseKind::Miss => miss_tail_rel,
                        Some(info) => info.time,
                        None => None,
                    }
                } else {
                    rel.and_then(|info| info.time).or(raw_release)
                };
                if let Some(rt) = display_release_time {
                    let delta = rt - end_time;
                    let kind = rel
                        .and_then(|info| info.kind.as_judgment_kind())
                        .map(score_judgment_label)
                        .unwrap_or_else(|| classify_delta(delta.abs(), windows));
                    println!("  release:  {}ms ({:+}ms, {})", rt, delta, kind);
                } else {
                    println!("  release:  NONE");
                }
                if rel.map(|info| info.rescued).unwrap_or(false) {
                    println!("  rescued:  true");
                }
                if split_ln {
                    let head_judgment = jj.map(|j| score_judgment_label(j.kind)).unwrap_or("MISS");
                    let tail_judgment = tail_kind;
                    let head_press_time = jj.and_then(|j| j.press_time);
                    let tail_rel_time = match rel {
                        Some(info) if info.kind == judgment::ReleaseKind::Miss => miss_tail_rel,
                        Some(info) => info.time,
                        None => None,
                    };
                    println!("  head_judgment: {}", head_judgment);
                    println!("  tail_judgment: {}", tail_judgment);
                    println!(
                        "  head_press_time_ms: {}",
                        head_press_time
                            .map(|time| time.to_string())
                            .unwrap_or_else(|| "NONE".to_string())
                    );
                    println!(
                        "  tail_release_time_ms: {}",
                        tail_rel_time
                            .map(|time| time.to_string())
                            .unwrap_or_else(|| "NONE".to_string())
                    );
                }
            } else {
                println!("\n[Note #{}] col {}", idx, ho.column);
                println!("  time:     {}ms", ho.time);
                if let Some(jj) = jj {
                    if let Some(pt) = jj.press_time {
                        let delta = pt - ho.time;
                        let kind = classify_delta(delta.abs(), windows);
                        println!("  press:    {}ms ({:+}ms, {})", pt, delta, kind);
                    } else {
                        println!("  press:    NONE");
                    }
                    println!("  assigned: {:?}", jj.kind);
                } else {
                    println!("  press:    NONE");
                    println!("  assigned: NOT FOUND");
                }
            }
        };
        println!("\n{}", "=".repeat(80));
        println!("[NOTE DEBUG] All Notes (sorted by column, then by time)");
        println!("{}", "=".repeat(80));
        let max_col = beatmap
            .hit_objects
            .iter()
            .map(|ho| ho.column)
            .max()
            .unwrap_or(3);
        for col in 0..=max_col {
            let col_notes: Vec<_> = beatmap
                .hit_objects
                .iter()
                .enumerate()
                .filter(|(_, ho)| ho.column == col)
                .collect();
            if col_notes.is_empty() {
                continue;
            }
            println!("\n{}", "-".repeat(60));
            println!("COLUMN {} ({} notes)", col, col_notes.len());
            println!("{}", "-".repeat(60));
            let mut sorted_col = col_notes.clone();
            sorted_col.sort_by_key(|(_, ho)| ho.time);
            for (idx, ho) in sorted_col {
                print_note(idx, ho, judgments, ln_releases, windows, press_status);
            }
        }
        println!("\n{}", "=".repeat(80));
    }
    pub(crate) fn print_all_presses(
        &self,
        press_status: &[judgment::PressStatus],
        beatmap: &Beatmap,
    ) {
        let presses: Vec<_> = press_status
            .iter()
            .filter(|p| p.status.starts_with("PRESS"))
            .collect();
        let releases: Vec<_> = press_status
            .iter()
            .filter(|p| p.status.starts_with("RELEASE"))
            .collect();
        let key_count = self.effective_key_count(beatmap);
        println!("\n{}", "=".repeat(80));
        println!(
            "[ALL PRESSES & RELEASES] {} presses, {} releases",
            presses.len(),
            releases.len()
        );
        println!("{}", "=".repeat(80));
        for col in 0..key_count {
            let col_events: Vec<_> = press_status.iter().filter(|p| p.column == col).collect();
            if col_events.is_empty() {
                continue;
            }
            let mut sorted = col_events.clone();
            sorted.sort_by_key(|p| p.time);
            for p in &sorted {
                let delta_str = if p.status.starts_with("PRESS") {
                    let note_time = p
                        .assigned_to
                        .and_then(|idx| beatmap.hit_objects.get(idx))
                        .map(|ho| ho.time);
                    if let (Some(_), Some(nt)) = (p.assigned_to, note_time) {
                        format!(" (delta {:+}ms)", p.time - nt)
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                println!(
                    "[col {}] {}ms{} -> {}",
                    p.column, p.time, delta_str, p.status
                );
            }
        }
        println!("\n[Summary by column]");
        for col in 0..key_count {
            let col_events: Vec<_> = press_status.iter().filter(|p| p.column == col).collect();
            if col_events.is_empty() {
                continue;
            }
            let press_assigned = col_events
                .iter()
                .filter(|p| p.status.contains("PRESS ASSIGNED"))
                .count();
            let press_ghost = col_events
                .iter()
                .filter(|p| p.status.contains("PRESS GHOST"))
                .count();
            let press_extra = col_events
                .iter()
                .filter(|p| p.status == "PRESS EXTRA")
                .count();
            let release_count = col_events
                .iter()
                .filter(|p| p.status.starts_with("RELEASE"))
                .count();
            println!(
                "  col {}: {} presses ({} assigned, {} ghost, {} extra), {} releases",
                col,
                press_assigned + press_ghost + press_extra,
                press_assigned,
                press_ghost,
                press_extra,
                release_count
            );
        }
        let press_assigned = presses
            .iter()
            .filter(|p| p.status.contains("ASSIGNED"))
            .count();
        let press_ghost = presses
            .iter()
            .filter(|p| p.status.contains("GHOST"))
            .count();
        let press_extra = presses
            .iter()
            .filter(|p| p.status.contains("EXTRA"))
            .count();
        let release_assigned = releases
            .iter()
            .filter(|p| p.status.contains("ASSIGNED"))
            .count();
        println!("\n[Total]");
        println!(
            "  presses:  {} ({} assigned, {} ghost, {} extra)",
            presses.len(),
            press_assigned,
            press_ghost,
            press_extra
        );
        println!(
            "  releases: {} ({} assigned to LN)",
            releases.len(),
            release_assigned
        );
        println!("{}", "=".repeat(80));
    }
    pub(crate) fn print_ln_details(
        &self,
        beatmap: &Beatmap,
        judgments: &[Judgment],
        windows: &HitWindows,
    ) {
        println!("\n[LN Details]");
        println!("{}", "=".repeat(60));
        println!("[Hit Windows] OD={:.1}", beatmap.difficulty.od);
        println!("  MAX:  ±{}ms", windows.max);
        println!("  300:  ±{}ms", windows.hit300);
        println!("  200:  ±{}ms", windows.hit200);
        println!("  100:  ±{}ms", windows.hit100);
        println!("  50:   ±{}ms", windows.hit50);
        println!("{}", "=".repeat(60));
        for (idx, ho) in beatmap.hit_objects.iter().enumerate() {
            let end = ho.end_time.unwrap_or(ho.time);
            if end <= ho.time + 2 {
                continue;
            }
            let j = judgments.iter().find(|j| j.index == idx);
            let dur = end - ho.time;
            println!("\n[LN #{}] col {}", idx, ho.column);
            println!("  head: {}ms, end: {}ms, dur: {}ms", ho.time, end, dur);
            let mut actual_head_judgment = "MISS";
            if let Some(j) = j {
                if let Some(pt) = j.press_time {
                    let head_delta = pt - ho.time;
                    let abs_delta = head_delta.abs();
                    if abs_delta <= windows.max {
                        actual_head_judgment = "MAX";
                    } else if abs_delta <= windows.hit300 {
                        actual_head_judgment = "300";
                    } else if abs_delta <= windows.hit200 {
                        actual_head_judgment = "200";
                    } else if abs_delta <= windows.hit100 {
                        actual_head_judgment = "100";
                    } else if abs_delta <= windows.hit50 {
                        actual_head_judgment = "50";
                    }
                    println!(
                        "  press: {}ms, delta: {:+}ms ({})",
                        pt, head_delta, actual_head_judgment
                    );
                } else {
                    println!("  press: MISS");
                }
                println!("  final: {}", j.kind);
            }
        }
        println!("\n{}[End LN Details]\n", "=".repeat(60));
    }
}
pub(crate) fn classify_delta(delta: i32, w: &HitWindows) -> &'static str {
    if delta <= w.max {
        "MAX"
    } else if delta <= w.hit300 {
        "300"
    } else if delta <= w.hit200 {
        "200"
    } else if delta <= w.hit100 {
        "100"
    } else if delta <= w.hit50 {
        "50"
    } else {
        "MISS"
    }
}
pub(crate) fn score_judgment_label(kind: crate::types::JudgmentKind) -> &'static str {
    match kind {
        crate::types::JudgmentKind::Max => "MAX",
        crate::types::JudgmentKind::Hit300 => "300",
        crate::types::JudgmentKind::Hit200 => "200",
        crate::types::JudgmentKind::Hit100 => "100",
        crate::types::JudgmentKind::Hit50 => "50",
        crate::types::JudgmentKind::Miss => "MISS",
    }
}

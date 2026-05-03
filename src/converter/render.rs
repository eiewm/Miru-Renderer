use super::*;
fn build_hud_beatmap_metadata(
    beatmap: &Beatmap,
    key_count: u8,
    max_combo: u32,
    duration_ms: i32,
) -> HudBeatmapMetadataState {
    let fallback_duration = beatmap
        .hit_objects
        .iter()
        .map(|object| object.end_time.unwrap_or(object.time))
        .max()
        .unwrap_or(0);
    let preview_bpm = get_bpm_at_time(
        &beatmap.timing_points,
        beatmap.metadata.preview_time.max(0) as f64,
    );
    HudBeatmapMetadataState {
        title: if beatmap.metadata.title_unicode.trim().is_empty() {
            beatmap.metadata.title.clone()
        } else {
            beatmap.metadata.title_unicode.clone()
        },
        title_romanized: beatmap.metadata.title.clone(),
        artist: if beatmap.metadata.artist_unicode.trim().is_empty() {
            beatmap.metadata.artist.clone()
        } else {
            beatmap.metadata.artist_unicode.clone()
        },
        artist_romanized: beatmap.metadata.artist.clone(),
        difficulty: beatmap.metadata.version.clone(),
        mapper: beatmap.metadata.creator.clone(),
        source: beatmap.metadata.source.clone(),
        tags: beatmap.metadata.tags.clone(),
        beatmap_id: beatmap.metadata.beatmap_id,
        beatmapset_id: beatmap.metadata.beatmapset_id,
        key_count,
        cs: beatmap.difficulty.cs,
        od: beatmap.difficulty.od,
        hp: beatmap.difficulty.hp,
        bpm: preview_bpm,
        bpm_text: format_hud_bpm(&beatmap.timing_points)
            .unwrap_or_else(|| format_bpm_value(preview_bpm)),
        note_count: beatmap.hit_objects.len() as u32,
        max_combo,
        duration_ms: duration_ms.max(fallback_duration).max(0),
    }
}
fn format_bpm_value(value: f32) -> String {
    if !value.is_finite() || value <= 0.0 {
        return "0".to_string();
    }
    let rounded = (value * 100.0).round() / 100.0;
    if (rounded - rounded.round()).abs() < 0.01 {
        format!("{:.0}", rounded)
    } else {
        format!("{:.2}", rounded)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}
fn format_hud_bpm(timing_points: &[TimingPoint]) -> Option<String> {
    let mut values = timing_points
        .iter()
        .filter_map(|point| point.bpm().map(|value| value as f32))
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| (value * 100.0).round() / 100.0)
        .collect::<Vec<_>>();
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.total_cmp(b));
    let min = *values.first()?;
    let max = *values.last()?;
    if (max - min).abs() < 0.01 {
        Some(format_bpm_value(min))
    } else {
        Some(format!(
            "{}-{}",
            format_bpm_value(min),
            format_bpm_value(max)
        ))
    }
}
fn static_hud_editor_preview_state(key_count: u8) -> HudFrameState {
    let beatmap_metadata = HudBeatmapMetadataState {
        title: "HUD Preview".to_string(),
        title_romanized: "HUD Preview".to_string(),
        artist: "Miru".to_string(),
        artist_romanized: "Miru".to_string(),
        difficulty: format!("{key_count}K"),
        mapper: "Miru".to_string(),
        key_count,
        cs: key_count as f32,
        od: 8.0,
        hp: 5.0,
        bpm: 180.0,
        bpm_text: "180".to_string(),
        note_count: 0,
        max_combo: 999,
        duration_ms: 120_000,
        ..Default::default()
    };

    HudFrameState {
        hud_visible: true,
        score: 1_000_000,
        accuracy: 1.0,
        combo: 999,
        judgment_counts: [0, 0, 0, 0, 0, 1],
        progress: 0.5,
        song_elapsed_ms: 60_000,
        song_duration_ms: 120_000,
        beatmap: beatmap_metadata,
        life: 1.0,
        key_down_mask: 0,
        total_kps: 0.0,
        is_break_time: false,
        has_failed: false,
        last_judgment: Some(LastJudgment {
            kind: JudgmentKind::Miss,
            age_ms: 0,
            column: key_count.saturating_sub(1) / 2,
            hit_offset_ms: None,
        }),
        hit_error_judgments: vec![
            crate::renderer::HitErrorJudgment {
                kind: JudgmentKind::Hit300,
                offset_ms: -24,
                age_ms: 650,
            },
            crate::renderer::HitErrorJudgment {
                kind: JudgmentKind::Max,
                offset_ms: 4,
                age_ms: 240,
            },
            crate::renderer::HitErrorJudgment {
                kind: JudgmentKind::Hit200,
                offset_ms: 39,
                age_ms: 1200,
            },
        ],
        hit_error_moving_avg_ms: Some(2.4),
        ..Default::default()
    }
}
impl ManiaVideoConverter {
    fn hud_editor_preview_key_count_from_nodes(nodes: &[crate::hud::HudLayerConfig]) -> Option<u8> {
        for node in nodes {
            if node.layer_type == "component.keyCounter" {
                if let Some(value) = node
                    .props
                    .get("keyCount")
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|value| u8::try_from(value).ok())
                    .filter(|value| (1..=18).contains(value))
                {
                    return Some(value);
                }
            }
            if let Some(value) = Self::hud_editor_preview_key_count_from_nodes(&node.children) {
                return Some(value);
            }
        }
        None
    }
    fn hud_editor_preview_key_count(&self) -> u8 {
        let Some(config) = self.settings.hud_config.as_ref() else {
            return 4;
        };
        // Static HUD previews do not have a beatmap, so infer columns from HUD metadata.
        if let Some(value) = config
            .metadata
            .get("keyCount")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .filter(|value| (1..=18).contains(value))
        {
            return value;
        }
        Self::hud_editor_preview_key_count_from_nodes(&config.nodes)
            .or_else(|| Self::hud_editor_preview_key_count_from_nodes(&config.layers))
            .unwrap_or(4)
    }
    pub fn render_static_hud_editor_preview_frame(
        &self,
        output_path: &Path,
        skin_path: Option<&Path>,
        layout_only: bool,
    ) -> Result<PathBuf, ConvertError> {
        println!("-> static HUD editor preview frame");
        self.progress(0, "Preparing static preview...");
        let key_count = self.hud_editor_preview_key_count();
        let set_dir = Path::new(".");
        self.progress(10, "Loading skin...");
        let mut skin = self.load_skin(skin_path, set_dir, key_count)?;
        println!("   skin: {} images loaded", skin.image_count());
        let resolved_hud_config = self.settings.hud_config.as_ref().map(|cfg| {
            resolve_hud_config(cfg, self.settings.width as f32, self.settings.height as f32)
        });
        let layout = self.build_layout(key_count, &skin, resolved_hud_config.as_ref());
        println!(
            "   layout: {}k, stage {}x{} at ({}, {})",
            key_count, layout.stage.width, layout.stage.height, layout.stage.x, layout.stage.y
        );
        let hud_state = static_hud_editor_preview_state(key_count);
        let mut layout_renderer = ReplayRenderer::new();
        layout_renderer.set_canvas_size(self.settings.width, self.settings.height);
        layout_renderer.set_fps(self.settings.fps);
        layout_renderer.set_scroll_speed(self.settings.scroll_speed);
        layout_renderer.set_lead_in_ms(self.settings.lead_in_ms);
        layout_renderer.set_hud_enabled(self.settings.enable_hud);
        layout_renderer.set_editor_preview_base_only(!self.settings.enable_hud);
        layout_renderer.set_lighting_enabled(self.settings.enable_lighting);
        layout_renderer.set_barlines_enabled(self.settings.enable_barlines);
        layout_renderer.set_ln_debug(self.settings.ln_debug);
        layout_renderer.set_sv_enabled(self.settings.sv_enabled);
        layout_renderer.set_skin_animations_enabled(self.settings.skin_animations_enabled);
        layout_renderer.set_hud_config(resolved_hud_config.clone());
        layout_renderer.set_storyboard_enabled(false);
        layout_renderer.set_storyboard(None);
        layout_renderer.set_stage_opaque_bg(true);
        layout_renderer.set_hud_beatmap_metadata(hud_state.beatmap.clone());
        let metadata_count = layout_renderer.load_skin_texture_metadata_for_layout(&skin);
        println!("   loaded {} skin texture metadata records", metadata_count);
        layout_renderer.prescale_hud_digits(&skin);
        for (name, rect) in
            layout_renderer.measure_hud_editor_preview_components(&layout, &skin, &hud_state)
        {
            println!(
                "   [hud-preview] component={} x={} y={} width={} height={}",
                name, rect.x, rect.y, rect.width, rect.height
            );
        }
        if layout_only {
            self.progress(100, "HUD layout ready");
            return Ok(output_path.to_path_buf());
        }
        self.progress(25, "Initializing static preview GPU...");
        let mut renderer = ReplayRenderer::new();
        renderer.set_canvas_size(self.settings.width, self.settings.height);
        renderer.set_fps(self.settings.fps);
        renderer.set_scroll_speed(self.settings.scroll_speed);
        renderer.set_lead_in_ms(self.settings.lead_in_ms);
        renderer.set_hud_enabled(self.settings.enable_hud);
        renderer.set_editor_preview_base_only(!self.settings.enable_hud);
        renderer.set_lighting_enabled(self.settings.enable_lighting);
        renderer.set_barlines_enabled(self.settings.enable_barlines);
        renderer.set_ln_debug(self.settings.ln_debug);
        renderer.set_sv_enabled(self.settings.sv_enabled);
        renderer.set_skin_animations_enabled(self.settings.skin_animations_enabled);
        renderer.set_hud_config(resolved_hud_config.clone());
        renderer.set_storyboard_enabled(false);
        renderer.set_storyboard(None);
        renderer.set_stage_opaque_bg(true);
        renderer.set_hud_beatmap_metadata(HudBeatmapMetadataState {
            title: "HUD Preview".to_string(),
            title_romanized: "HUD Preview".to_string(),
            artist: "Miru".to_string(),
            artist_romanized: "Miru".to_string(),
            difficulty: format!("{key_count}K"),
            mapper: "Miru".to_string(),
            key_count,
            cs: key_count as f32,
            od: 8.0,
            hp: 5.0,
            bpm: 180.0,
            bpm_text: "180".to_string(),
            note_count: 0,
            max_combo: 999,
            duration_ms: 120_000,
            ..Default::default()
        });
        let gpu_info = pollster::block_on(renderer.init_gpu(self.settings.gpu_preference, None))
            .map_err(|e| ConvertError::Render(format!("GPU init failed: {}", e)))?;
        println!("   gpu: {}", gpu_info);
        renderer.create_common_textures();
        match renderer.load_skin_textures(&skin) {
            Ok(count) => println!("   loaded {} skin textures", count),
            Err(err) => println!("   warn: skin textures: {}", err),
        }
        let column_widths: Vec<u32> = layout.columns.iter().map(|column| column.width).collect();
        renderer.precompute_ln_body_atlases(&skin, &column_widths);
        renderer.precompute_note_atlases(&skin, &column_widths);
        renderer.prescale_hud_digits(&skin);
        // The GPU now owns the needed textures; drop CPU image data before rendering.
        skin.images.clear();
        skin.images.shrink_to_fit();
        for (name, rect) in
            renderer.measure_hud_editor_preview_components(&layout, &skin, &hud_state)
        {
            println!(
                "   [hud-preview] component={} x={} y={} width={} height={}",
                name, rect.x, rect.y, rect.width, rect.height
            );
        }
        let notes: Vec<crate::types::HitObject> = Vec::new();
        let active_indices: Vec<usize> = Vec::new();
        let judgments_by_idx: Vec<Option<RenderJudgment>> = Vec::new();
        let ln_releases_by_idx: Vec<Option<LnReleaseInfo>> = Vec::new();
        let render_windows = Windows {
            max: 16,
            hit300: 64,
            hit200: 97,
            hit100: 127,
            hit50: 151,
            miss: 251,
        };
        let submitted = renderer.submit_frame(
            0,
            0.0,
            &layout,
            &skin,
            &notes,
            &active_indices,
            &judgments_by_idx,
            &ln_releases_by_idx,
            &hud_state,
            0,
            1000.0,
            Some(&render_windows),
            &[],
            None,
            None,
        );
        if !submitted {
            return Err(ConvertError::Render(
                "GPU render submit failed during static HUD preview".into(),
            ));
        }
        self.progress(80, "Reading preview frame...");
        let frame = renderer
            .drain_ready_frame_blocking()
            .ok_or_else(|| ConvertError::Render("GPU preview frame was not produced".into()))?
            .to_vec();
        self.progress(90, "Saving preview frame...");
        let image = image::RgbaImage::from_raw(self.settings.width, self.settings.height, frame)
            .ok_or_else(|| {
                ConvertError::Render("failed to build preview image buffer".to_string())
            })?;
        image
            .save(output_path)
            .map_err(|e| ConvertError::Render(format!("failed to save preview png: {e}")))?;
        renderer.dispose();
        self.progress(100, "Preview done");
        println!("   saved: {}", output_path.display());
        Ok(output_path.to_path_buf())
    }
    pub fn convert_beatmap_to_video(
        &self,
        beatmap_path: &Path,
        audio_path: &Path,
        output_path: &Path,
        _skin_path: Option<&Path>,
        opts: &ResolveOpts,
    ) -> Result<ConvertResult, ConvertError> {
        let start = std::time::Instant::now();
        println!("-> beatmap conversion");
        println!("   beatmap: {}", beatmap_path.display());
        println!("   audio: {}", audio_path.display());
        self.progress(0, "Parsing beatmap...");
        let beatmap = parser::parse_osu_file_with_options(
            beatmap_path,
            parser::ParseBeatmapOptions {
                storyboard_enabled: self.settings.storyboard_enabled,
            },
        )
        .map_err(|e| ConvertError::Parse(e.to_string()))?;
        println!(
            "   {} - {} [{}]",
            beatmap.metadata.artist, beatmap.metadata.title, beatmap.metadata.version
        );
        let star_rating = self.enforce_star_limit(beatmap_path)?;
        println!("   stars: {:.2}", star_rating);
        if !audio_path.exists() {
            return Err(ConvertError::Resolve(format!(
                "audio not found: {}",
                audio_path.display()
            )));
        }
        self.progress(20, "Rendering...");
        let frame_count =
            self.estimate_frames(&beatmap, opts, PlaybackRateProfile::constant(1.0))?;
        self.progress(80, "Composing...");
        let compose_opts = self.build_compose_opts(
            output_path,
            Some(audio_path),
            0,
            0,
            0,
            None,
            PlaybackModSettings::normal(),
            None,
            None,
            None,
            &[],
            &[],
        );
        let mut composer = FrameComposer::spawn(&compose_opts)?;
        let frame_size = (self.settings.width * self.settings.height * 4) as usize;
        let black = vec![0u8; frame_size];
        for _ in 0..frame_count.min(120) {
            composer.push_frame(&black)?;
        }
        composer.finish()?;
        self.progress(100, "Done");
        let elapsed = start.elapsed();
        Ok(ConvertResult {
            output_path: output_path.to_path_buf(),
            elapsed_ms: elapsed.as_millis() as u64,
            frames_rendered: frame_count.min(120),
            replay_integrity: None,
        })
    }
    pub fn convert_replay_to_video(
        &self,
        osr_path: &Path,
        output_path: &Path,
        skin_path: Option<&Path>,
        opts: &ResolveOpts,
    ) -> Result<ConvertResult, ConvertError> {
        println!("-> replay conversion");
        println!("   replay: {}", osr_path.display());
        println!("   output: {}", output_path.display());
        self.progress(0, "Parsing replay...");
        let mut replay =
            parser::parse_osr_file(osr_path).map_err(|e| ConvertError::Parse(e.to_string()))?;
        println!(
            "   player: {}, actions: {}",
            replay.replay.player_name,
            replay.key_actions.len()
        );
        self.ensure_rd_not_enabled(&replay.replay)?;
        self.progress(5, "Resolving beatmap...");
        let (beatmap_path, set_dir, set_files) = self.resolve_beatmap(&replay, opts)?;
        println!("   beatmap: {}", beatmap_path.display());
        let beatmap = parser::parse_osu_file_with_options(
            &beatmap_path,
            parser::ParseBeatmapOptions {
                storyboard_enabled: self.settings.storyboard_enabled,
            },
        )
        .map_err(|e| ConvertError::Parse(e.to_string()))?;
        let beatmap =
            self.resolve_playable_mania_beatmap(&replay.replay, beatmap, &beatmap_path)?;
        let key_count = self.effective_key_count(&beatmap);
        replay.key_actions = ManiaReplayData::derive_key_actions(&replay.frames, key_count);
        self.render_with_replay_context(
            &mut replay,
            beatmap,
            &beatmap_path,
            &set_dir,
            &set_files,
            Some(osr_path),
            output_path,
            skin_path,
            opts,
            opts.intro_user_data.clone(),
            false,
        )
    }
    pub fn convert_autoplay_to_video(
        &self,
        beatmap_path: &Path,
        output_path: &Path,
        skin_path: Option<&Path>,
        opts: &ResolveOpts,
    ) -> Result<ConvertResult, ConvertError> {
        println!("-> autoplay conversion");
        println!("   beatmap: {}", beatmap_path.display());
        println!("   output: {}", output_path.display());
        self.progress(0, "Parsing beatmap...");
        let beatmap = parser::parse_osu_file_with_options(
            beatmap_path,
            parser::ParseBeatmapOptions {
                storyboard_enabled: self.settings.storyboard_enabled,
            },
        )
        .map_err(|e| ConvertError::Parse(e.to_string()))?;
        if beatmap.metadata.mode != 3 {
            return Err(ConvertError::Parse(format!(
                "beatmap is not osu!mania (Mode={})",
                beatmap.metadata.mode
            )));
        }
        let autoplay_mods = self.normalized_autoplay_mods(opts)?;
        let mut replay = self.build_autoplay_replay_data(autoplay_mods.as_ref(), Vec::new());
        replay.key_actions =
            self.generate_autoplay_key_actions_for_replay(&beatmap, &replay.replay)?;
        let set_dir = beatmap_path
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();
        let set_files = self.list_dir_files_recursive(&set_dir);
        self.render_with_replay_context(
            &mut replay,
            beatmap,
            beatmap_path,
            &set_dir,
            &set_files,
            None,
            output_path,
            skin_path,
            opts,
            if self.settings.intro_enabled {
                Some(self.build_autoplay_intro_user_data(opts.intro_user_data.clone()))
            } else {
                None
            },
            true,
        )
    }
    pub(crate) fn render_with_replay_context(
        &self,
        replay: &mut ManiaReplayData,
        beatmap: Beatmap,
        beatmap_path: &Path,
        set_dir: &Path,
        set_files: &[String],
        _replay_path: Option<&Path>,
        output_path: &Path,
        skin_path: Option<&Path>,
        opts: &ResolveOpts,
        intro_user_override: Option<crate::utils::IntroUserData>,
        autoplay: bool,
    ) -> Result<ConvertResult, ConvertError> {
        let start = std::time::Instant::now();
        let intro_user_data = if self.settings.intro_enabled {
            intro_user_override
                .map(IntroUserDataGuard::persistent)
                .unwrap_or_default()
        } else {
            IntroUserDataGuard::default()
        };
        let prepared = self.prepare_replay_render(
            replay,
            beatmap.clone(),
            beatmap_path,
            set_dir,
            set_files,
            output_path,
            skin_path,
            opts,
            intro_user_data.as_ref(),
            autoplay,
        )?;
        let mut attempt_index = 0usize;
        let mut requested_encoder = self.settings.encoder;
        let result = loop {
            match self.render_prepared_replay_attempt(
                replay,
                &prepared,
                output_path,
                requested_encoder,
                attempt_index,
                start,
            ) {
                Ok(result) => break Ok(result),
                Err(ConvertError::Compose(ComposeError::Ffmpeg(failure))) => {
                    if let Some(retry_encoder) = retry_encoder_for_failure(&failure, attempt_index)
                    {
                        eprintln!(
                            "   warn: ffmpeg {} failed with {} after {} frames; retrying full render with {}",
                            failure.stage.as_str(),
                            failure.resolved_encoder.as_str(),
                            failure.frames_written,
                            retry_encoder.as_str()
                        );
                        attempt_index += 1;
                        requested_encoder = retry_encoder;
                        continue;
                    }
                    break Err(ConvertError::Compose(ComposeError::Ffmpeg(failure)));
                }
                Err(err) => break Err(err),
            }
        };
        result
    }
    pub(crate) fn render_prepared_replay_attempt(
        &self,
        replay: &ManiaReplayData,
        prepared: &PreparedReplayRender,
        output_path: &Path,
        requested_encoder: VideoEncoder,
        attempt_index: usize,
        start: std::time::Instant,
    ) -> Result<ConvertResult, ConvertError> {
        self.progress(25, "Initializing GPU...");
        let temp_output = AttemptOutputFile::new(output_path, attempt_index);
        let mut compose_opts = prepared.compose_template.clone();
        compose_opts.output_path = temp_output.temp_path().to_string_lossy().into_owned();
        compose_opts.encoder = requested_encoder;
        let mut composer = FrameComposer::spawn(&compose_opts)?;
        let mut renderer = ReplayRenderer::new();
        renderer.set_canvas_size(self.settings.width, self.settings.height);
        renderer.set_fps(self.settings.fps);
        renderer.set_scroll_speed(self.settings.scroll_speed);
        renderer.set_lead_in_ms(self.settings.lead_in_ms);
        renderer.set_hud_enabled(self.settings.enable_hud);
        renderer.set_lighting_enabled(self.settings.enable_lighting);
        renderer.set_barlines_enabled(self.settings.enable_barlines);
        renderer.set_ln_debug(self.settings.ln_debug);
        renderer.set_sv_enabled(self.settings.sv_enabled);
        renderer.set_skin_animations_enabled(self.settings.skin_animations_enabled);
        renderer.set_scroll_playback_clock(Some(prepared.playback_clock.clone()));
        renderer.set_hud_config(prepared.resolved_hud_config.clone());
        renderer.set_hud_pp_timeline(prepared.hud_pp_timeline.clone(), prepared.hud_pp_final);
        renderer.set_hud_unstable_rate(
            prepared
                .results_data
                .as_ref()
                .map(|data| data.timing_summary.unstable_rate),
        );
        renderer.set_hud_beatmap_metadata(build_hud_beatmap_metadata(
            &prepared.beatmap,
            prepared.layout.num_columns() as u8,
            prepared.beatmap.max_combo(),
            prepared.end_sequence.gameplay_end_ms,
        ));
        renderer.set_replay_mod_display(Some(prepared.replay_mod_display.clone()));
        renderer.set_stage_opaque_bg(prepared.bg.is_none());
        renderer
            .set_first_note_time_ms(prepared.beatmap.hit_objects.iter().map(|ho| ho.time).min());
        let mut seed = 1_469_598_103_934_665_603u64;
        // Seed visual randomness from replay and map identity so retries render the same video.
        for byte in format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
            replay.replay.player_name,
            prepared.beatmap.metadata.artist,
            prepared.beatmap.metadata.title,
            prepared.beatmap.metadata.version,
            prepared.beatmap.hit_objects.len()
        )
        .bytes()
        {
            seed ^= u64::from(byte);
            seed = seed.wrapping_mul(1_099_511_628_211);
        }
        renderer.set_random_seed(seed);
        let gpu_info = pollster::block_on(renderer.init_gpu(self.settings.gpu_preference, None))
            .map_err(|e| ConvertError::Render(format!("GPU init failed: {}", e)))?;
        println!("   gpu: {}", gpu_info);
        renderer.create_common_textures();
        if let Some(bg) = prepared.bg.as_ref() {
            if matches!(bg.kind, BackgroundKind::Image) {
                match renderer.set_background_image(
                    &bg.path,
                    bg.dim,
                    self.settings.background_blur_percent.unwrap_or(0),
                    self.settings.background_offset_x,
                    self.settings.background_offset_y,
                ) {
                    Ok(()) => renderer.set_stage_opaque_bg(false),
                    Err(err) => {
                        println!("   warn: background image load failed: {}", err);
                        renderer.set_stage_opaque_bg(true);
                    }
                }
            }
        }
        self.progress(28, "Loading skin textures...");
        match renderer.load_skin_textures(&prepared.skin) {
            Ok(count) => println!("   loaded {} skin textures", count),
            Err(err) => println!("   warn: skin textures: {}", err),
        }
        renderer.prepare_replay_mod_textures(&prepared.skin);
        renderer.set_storyboard_enabled(self.settings.storyboard_enabled);
        if self.settings.storyboard_enabled {
            match crate::renderer::StoryboardPlayer::from_beatmap(
                &prepared.beatmap,
                |path| self.resolve_asset(&prepared.set_dir, path, &prepared.set_files),
                &mut renderer,
            ) {
                Ok(Some(sb)) => {
                    println!("   storyboard: {} objects", sb.object_count());
                    renderer.set_storyboard(Some(sb));
                }
                Ok(None) => renderer.set_storyboard(None),
                Err(err) => {
                    println!("   warn: storyboard disabled: {}", err);
                    renderer.set_storyboard(None);
                }
            }
        } else {
            renderer.set_storyboard(None);
        }
        let column_widths: Vec<u32> = prepared
            .layout
            .columns
            .iter()
            .map(|column| column.width)
            .collect();
        renderer.precompute_ln_body_atlases(&prepared.skin, &column_widths);
        renderer.precompute_note_atlases(&prepared.skin, &column_widths);
        renderer.prescale_hud_digits(&prepared.skin);
        if self.settings.ln_debug {
            let avg_col_width =
                column_widths.iter().sum::<u32>() / column_widths.len().max(1) as u32;
            renderer.preload_debug_textures(&prepared.render_judgments, avg_col_width);
            println!(
                "   [debug] preloaded debug text textures for {} judgments",
                prepared.render_judgments.len()
            );
        }
        renderer.set_scroll_timeline(prepared.scroll_model.clone());
        let results_background_frame = if prepared.end_sequence.has_results() {
            // If no explicit results background exists, capture the gameplay backdrop at transition.
            load_results_background_frame(
                prepared.results_background_path.as_deref(),
                self.settings.width,
                self.settings.height,
            )
            .or_else(|| {
                if renderer.submit_results_backdrop_frame(prepared.end_sequence.results_start_ms) {
                    renderer
                        .drain_ready_frame_blocking()
                        .map(|frame| frame.to_vec())
                } else {
                    None
                }
            })
        } else {
            None
        };
        let render_start = std::time::Instant::now();
        let mut frames_in_batch = 0u64;
        let mut last_fps_print = std::time::Instant::now();
        let mut gpu_fps = 0.0f64;
        let mut note_window = NoteWindow::default();
        const EXTRA_VISIBLE_PX: f64 = 12.0;
        let look_behind = 500;
        let visible_dist_px =
            (prepared.layout.stage.hit_y - prepared.layout.stage.top_y).max(1) as f64;
        let replay_final_score = replay.replay.total_score as u64;
        let total_judgments = prepared.score_judgments.len();
        let mut judgment_idx = 0usize;
        let score_events = prepared.combo_data.score_events.clone();
        let frame_time_ms = 1000.0 / self.settings.fps as f64;
        let total_output_frames = prepared.intro_frames
            + prepared.main_scene_total_frames
            + prepared.end_sequence.results_frames;
        let gameplay_progress_output_ms = prepared
            .playback_clock
            .output_elapsed_ms_for_beatmap_time(prepared.end_sequence.gameplay_end_ms as f64)
            .max(frame_time_ms);
        let gameplay_progress_frames = (gameplay_progress_output_ms / frame_time_ms)
            .ceil()
            .max(1.0) as usize;
        let distance_at_ms = |time_ms: i32| -> f64 {
            let display_time_ms = prepared
                .playback_clock
                .output_elapsed_ms_for_beatmap_time(time_ms as f64)
                .round() as i32;
            if let Some(model) = prepared.scroll_model.as_ref() {
                model.object_distance_at_ms(display_time_ms)
            } else {
                (prepared.visual_pps as f64 * display_time_ms as f64) / 1000.0
            }
        };
        if prepared.intro_frames > 0 {
            println!("   rendering {} intro frames...", prepared.intro_frames);
            let intro_cfg = prepared
                .intro_config
                .as_ref()
                .ok_or_else(|| ConvertError::Render("missing intro config".into()))?;
            let ctx = renderer
                .gpu_context()
                .ok_or_else(|| ConvertError::Render("GPU not initialized".into()))?;
            let mut gpu_intro = GpuIntroRenderer::new(ctx, intro_cfg)
                .ok_or_else(|| ConvertError::Render("Failed to create intro renderer".into()))?;
            let total_frames_f32 = total_output_frames as f32;
            gpu_intro.render_frames_into(
                ctx,
                intro_cfg,
                |idx, _time_ms, frame_data| -> Result<(), ConvertError> {
                    composer.push_frame(frame_data)?;
                    frames_in_batch += 1;
                    if idx % 60 == 0 {
                        let pct = 25 + ((idx as f32 / total_frames_f32) * 70.0) as u32;
                        self.progress(pct, "Rendering intro...");
                    }
                    Ok(())
                },
            )?;
        }
        renderer.compact_runtime_memory();
        println!(
            "   rendering {} main-scene frames with GPU...",
            prepared.main_scene_total_frames
        );
        let mut rendered = 0u64;
        let mut rendered_results = 0u64;
        let mut fail_anim_state: Option<FailAnimationState> = None;
        let mut fail_visual_time_ms = 0.0f64;
        let mut fail_last_real_time_ms: Option<i32> = None;
        let mut last_frame_time_ms = prepared
            .playback_clock
            .beatmap_time_ms_for_output_elapsed(0.0);
        for i in 0..prepared.main_scene_total_frames {
            let output_elapsed_ms = i as f64 * frame_time_ms;
            let animation_time_ms = prepared
                .playback_clock
                .beatmap_time_for_output_elapsed_ms(output_elapsed_ms + frame_time_ms);
            let frame_time = prepared
                .playback_clock
                .beatmap_time_ms_for_output_elapsed(output_elapsed_ms);
            last_frame_time_ms = frame_time;
            let current_dist = distance_at_ms(frame_time);
            let behind_dist = if look_behind > 0 {
                (current_dist - distance_at_ms(frame_time - look_behind)).abs()
            } else {
                0.0
            };
            let start_dist = current_dist - behind_dist;
            let end_dist = current_dist + visible_dist_px + EXTRA_VISIBLE_PX;
            // Cull in scroll-distance space so SV and playback-rate changes stay aligned.
            while note_window.start < prepared.effective_end_distances.len()
                && prepared.effective_end_distances[note_window.start] < start_dist
            {
                note_window.start += 1;
            }
            note_window.end = prepared
                .note_distances
                .partition_point(|dist| *dist <= end_dist);
            if note_window.end < note_window.start {
                note_window.end = note_window.start;
            }
            let active_indices = &prepared.sorted_indices[note_window.range()];
            while judgment_idx < prepared.render_judgments.len() {
                let judgment = &prepared.render_judgments[judgment_idx];
                let judgment_time = judgment.press_time.unwrap_or(judgment.time);
                if judgment_time > frame_time {
                    break;
                }
                judgment_idx += 1;
            }
            let _current_score = if total_judgments > 0 {
                (replay_final_score * judgment_idx as u64) / total_judgments as u64
            } else {
                0
            };
            let replay_stats = replay.replay.basic_statistics();
            let max_acc_weight = prepared.score_mode.accuracy_max_per_hit();
            let weighted_hits = replay_stats.weighted_hits(max_acc_weight);
            let total_notes_osr = replay_stats.total();
            let final_accuracy = if total_notes_osr > 0 {
                weighted_hits as f64 / (total_notes_osr as f64 * max_acc_weight as f64)
            } else {
                1.0
            };
            let _accuracy = if total_judgments > 0 && judgment_idx > 0 {
                let progress_ratio = judgment_idx as f64 / total_judgments as f64;
                1.0 - (1.0 - final_accuracy) * progress_ratio
            } else {
                1.0
            };
            let key_mask = self.get_key_mask_at(frame_time, &prepared.key_timeline);
            if let Some(fail_time) = prepared.health_timeline.fail_time_ms {
                if frame_time >= fail_time {
                    let elapsed = (frame_time - fail_time).max(0);
                    let progress = (elapsed as f32 / anim::FAIL_ANIM_MS as f32).clamp(0.0, 1.0);
                    if fail_anim_state.is_none() {
                        // Freeze the visible notes and key state at fail start for the fail animation.
                        fail_visual_time_ms = fail_time as f64;
                        fail_last_real_time_ms = Some(frame_time);
                        fail_anim_state = Some(FailAnimationState {
                            active: true,
                            fail_started_at: fail_time,
                            visual_time_ms: fail_time,
                            progress,
                            active_note_indices: active_indices.to_vec(),
                            frozen_key_mask: key_mask,
                        });
                    } else if let Some(prev_real_time) = fail_last_real_time_ms {
                        if elapsed < anim::FAIL_ANIM_MS {
                            let delta_real_ms = (frame_time - prev_real_time).max(0) as f64;
                            let rate = 1.0 - 0.85 * ease_out_cubic(progress) as f64;
                            fail_visual_time_ms += delta_real_ms * rate;
                        }
                        fail_last_real_time_ms = Some(frame_time);
                    }
                    if let Some(state) = fail_anim_state.as_mut() {
                        state.progress = progress;
                        state.visual_time_ms = fail_visual_time_ms.round() as i32;
                    }
                }
            }
            let mut hud_state = renderer.compute_hud_state(
                frame_time,
                &score_events,
                &prepared.score_judgments,
                prepared.score_scale,
                (i as usize).min(gameplay_progress_frames.saturating_sub(1)),
                gameplay_progress_frames,
                prepared.score_state_end_time_ms,
                Some(&prepared.health_timeline),
                Some(&prepared.render_windows),
            );
            hud_state.hud_visible = prepared.end_sequence.hud_visible_at(frame_time);
            hud_state.is_break_time = prepared.beatmap.events.breaks.iter().any(|break_period| {
                frame_time >= break_period.start && frame_time < break_period.end
            });
            let playfield_cover = renderer.resolve_smoothed_playfield_cover(
                frame_time,
                prepared.cover_config,
                prepared.cover_metrics,
                &hud_state,
            );
            let submitted = {
                let _scope = crate::utils::perf::scoped("render_frame");
                renderer.submit_frame(
                    frame_time,
                    animation_time_ms,
                    &prepared.layout,
                    &prepared.skin,
                    &prepared.beatmap.hit_objects,
                    active_indices,
                    &prepared.judgments_by_idx,
                    &prepared.ln_release_by_idx,
                    &hud_state,
                    key_mask,
                    prepared.visual_pps,
                    Some(&prepared.render_windows),
                    &prepared.barlines,
                    fail_anim_state.as_ref(),
                    playfield_cover.as_ref(),
                )
            };
            if !submitted {
                return Err(ConvertError::Render(
                    "GPU render submit failed during gameplay".into(),
                ));
            }
            while let Some(frame_data) = renderer.poll_ready_frame() {
                if let Some(results_background_frame) = results_background_frame.as_ref() {
                    let gameplay_alpha = prepared.end_sequence.gameplay_alpha_at(frame_time);
                    if gameplay_alpha <= 0.0 {
                        composer.push_frame(results_background_frame)?;
                    } else if gameplay_alpha >= 1.0 {
                        composer.push_frame(frame_data)?;
                    } else {
                        let blended =
                            blend_frames(results_background_frame, frame_data, gameplay_alpha);
                        composer.push_frame(&blended)?;
                    }
                } else {
                    composer.push_frame(frame_data)?;
                }
                rendered += 1;
                frames_in_batch += 1;
            }
            if i > 0 && i % 300 == 0 {
                renderer.compact_runtime_memory();
            }
            if i > 0 && i % 600 == 0 {
                let (bind_groups_cache, gpu_textures) = renderer.runtime_memory_stats();
                println!(
                    "   [mem] bind_groups_cache={} gpu_textures={}",
                    bind_groups_cache, gpu_textures
                );
            }
            let now = std::time::Instant::now();
            let elapsed_since_fps = now.duration_since(last_fps_print).as_secs_f64();
            if elapsed_since_fps >= 1.0 {
                gpu_fps = frames_in_batch as f64 / elapsed_since_fps;
                last_fps_print = now;
                frames_in_batch = 0;
            }
            if i % 60 == 0 {
                let pct = 25
                    + (((prepared.intro_frames + i) as f32 / total_output_frames as f32) * 70.0)
                        as u32;
                let msg = format!(
                    "GPU: {:.1} fps | Frame {}/{}",
                    gpu_fps, i, prepared.main_scene_total_frames
                );
                self.progress(pct.min(95), &msg);
                if gpu_fps > 0.0 {
                    print!(
                        "\r   GPU: {:.1} fps | {:.1}% | Frame {}/{}    ",
                        gpu_fps, pct as f32, i, prepared.main_scene_total_frames
                    );
                    use std::io::Write;
                    let _ = std::io::stdout().flush();
                }
            }
        }
        while let Some(frame_data) = renderer.drain_ready_frame_blocking() {
            if let Some(results_background_frame) = results_background_frame.as_ref() {
                let gameplay_alpha = prepared.end_sequence.gameplay_alpha_at(last_frame_time_ms);
                if gameplay_alpha <= 0.0 {
                    composer.push_frame(results_background_frame)?;
                } else if gameplay_alpha >= 1.0 {
                    composer.push_frame(frame_data)?;
                } else {
                    let blended =
                        blend_frames(results_background_frame, frame_data, gameplay_alpha);
                    composer.push_frame(&blended)?;
                }
            } else {
                composer.push_frame(frame_data)?;
            }
            rendered += 1;
        }
        println!();
        if rendered != prepared.main_scene_total_frames {
            return Err(ConvertError::Render(format!(
                "frame count mismatch: rendered {} main-scene frames, expected {}",
                rendered, prepared.main_scene_total_frames
            )));
        }
        if let Some(results_data) = prepared.results_data.as_ref() {
            if prepared.end_sequence.results_frames > 0 {
                println!(
                    "   rendering {} results frames on CPU...",
                    prepared.end_sequence.results_frames
                );
                let background_frame = results_background_frame.clone().unwrap_or_else(|| {
                    vec![0u8; (self.settings.width * self.settings.height * 4) as usize]
                });
                let results_renderer = crate::results::ResultsSceneRenderer::new(
                    &background_frame,
                    self.settings.width,
                    self.settings.height,
                    &prepared.skin,
                    results_data,
                );
                for i in 0..prepared.end_sequence.results_frames {
                    let results_frame =
                        results_renderer.render_frame(i, prepared.end_sequence.results_frames);
                    composer.push_frame(&results_frame)?;
                    rendered_results += 1;
                    if i % 60 == 0 {
                        let finished_frames = prepared.intro_frames + rendered + rendered_results;
                        let pct = 25
                            + ((finished_frames as f32 / total_output_frames as f32) * 70.0) as u32;
                        self.progress(pct.min(95), "Rendering results...");
                    }
                }
            }
        }
        let render_elapsed = render_start.elapsed();
        let avg_fps = (prepared.intro_frames + rendered + rendered_results) as f64
            / render_elapsed.as_secs_f64();
        println!(
            "   render done: {} frames in {:.2}s ({:.1} fps avg)",
            prepared.intro_frames + rendered + rendered_results,
            render_elapsed.as_secs_f64(),
            avg_fps
        );
        self.progress(96, "Finalizing video...");
        composer.finish()?;
        crate::utils::perf::print_summary();
        renderer.dispose();
        temp_output.commit()?;
        self.progress(100, "Done");
        let elapsed = start.elapsed();
        println!("ok: completed in {}", fmt_time(elapsed.as_secs_f32()));
        Ok(ConvertResult {
            output_path: output_path.to_path_buf(),
            elapsed_ms: elapsed.as_millis() as u64,
            frames_rendered: prepared.intro_frames + rendered + rendered_results,
            replay_integrity: prepared.replay_integrity.clone(),
        })
    }
    pub fn render_preview_frame(
        &self,
        osr_path: &Path,
        output_path: &Path,
        skin_path: Option<&Path>,
        opts: &ResolveOpts,
        preview_time_ms: Option<i32>,
        hud_editor_preview: bool,
    ) -> Result<PathBuf, ConvertError> {
        println!("-> preview frame");
        self.progress(0, "Parsing replay...");
        let mut replay =
            parser::parse_osr_file(osr_path).map_err(|e| ConvertError::Parse(e.to_string()))?;
        self.ensure_rd_not_enabled(&replay.replay)?;
        self.progress(5, "Resolving beatmap...");
        let (beatmap_path, _, _) = self.resolve_beatmap(&replay, opts)?;
        self.progress(8, "Parsing beatmap...");
        let beatmap = parser::parse_osu_file_with_options(
            &beatmap_path,
            parser::ParseBeatmapOptions {
                storyboard_enabled: self.settings.storyboard_enabled,
            },
        )
        .map_err(|e| ConvertError::Parse(e.to_string()))?;
        let beatmap =
            self.resolve_playable_mania_beatmap(&replay.replay, beatmap, &beatmap_path)?;
        let key_count = self.effective_key_count(&beatmap);
        replay.key_actions = ManiaReplayData::derive_key_actions(&replay.frames, key_count);
        let set_dir = beatmap_path
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();
        let set_files = self.list_dir_files_recursive(&set_dir);
        self.progress(12, "Preparing preview frame...");
        let prepared = self.prepare_replay_render(
            &mut replay,
            beatmap,
            &beatmap_path,
            &set_dir,
            &set_files,
            output_path,
            skin_path,
            opts,
            None,
            false,
        )?;
        self.progress(25, "Initializing preview GPU...");
        let mut renderer = ReplayRenderer::new();
        renderer.set_canvas_size(self.settings.width, self.settings.height);
        renderer.set_fps(self.settings.fps);
        renderer.set_scroll_speed(self.settings.scroll_speed);
        renderer.set_lead_in_ms(self.settings.lead_in_ms);
        renderer.set_hud_enabled(self.settings.enable_hud);
        renderer.set_editor_preview_base_only(hud_editor_preview && !self.settings.enable_hud);
        renderer.set_lighting_enabled(self.settings.enable_lighting);
        renderer.set_barlines_enabled(self.settings.enable_barlines);
        renderer.set_ln_debug(self.settings.ln_debug);
        renderer.set_sv_enabled(self.settings.sv_enabled);
        renderer.set_skin_animations_enabled(self.settings.skin_animations_enabled);
        renderer.set_scroll_playback_clock(Some(prepared.playback_clock.clone()));
        renderer.set_hud_config(prepared.resolved_hud_config.clone());
        renderer.set_hud_pp_timeline(prepared.hud_pp_timeline.clone(), prepared.hud_pp_final);
        renderer.set_hud_unstable_rate(
            prepared
                .results_data
                .as_ref()
                .map(|data| data.timing_summary.unstable_rate),
        );
        renderer.set_hud_beatmap_metadata(build_hud_beatmap_metadata(
            &prepared.beatmap,
            prepared.layout.num_columns() as u8,
            prepared.beatmap.max_combo(),
            prepared.end_sequence.gameplay_end_ms,
        ));
        renderer.set_replay_mod_display(Some(prepared.replay_mod_display.clone()));
        renderer.set_stage_opaque_bg(prepared.bg.is_none());
        renderer
            .set_first_note_time_ms(prepared.beatmap.hit_objects.iter().map(|ho| ho.time).min());
        let gpu_info = pollster::block_on(renderer.init_gpu(self.settings.gpu_preference, None))
            .map_err(|e| ConvertError::Render(format!("GPU init failed: {}", e)))?;
        println!("   gpu: {}", gpu_info);
        renderer.create_common_textures();
        if let Some(bg) = prepared.bg.as_ref() {
            if matches!(bg.kind, BackgroundKind::Image) {
                match renderer.set_background_image(
                    &bg.path,
                    bg.dim,
                    self.settings.background_blur_percent.unwrap_or(0),
                    self.settings.background_offset_x,
                    self.settings.background_offset_y,
                ) {
                    Ok(()) => renderer.set_stage_opaque_bg(false),
                    Err(err) => {
                        println!("   warn: background image load failed: {}", err);
                        renderer.set_stage_opaque_bg(true);
                    }
                }
            }
        }
        match renderer.load_skin_textures(&prepared.skin) {
            Ok(count) => println!("   loaded {} skin textures", count),
            Err(err) => println!("   warn: skin textures: {}", err),
        }
        renderer.prepare_replay_mod_textures(&prepared.skin);
        renderer.set_storyboard_enabled(self.settings.storyboard_enabled);
        if self.settings.storyboard_enabled {
            match crate::renderer::StoryboardPlayer::from_beatmap(
                &prepared.beatmap,
                |path| self.resolve_asset(&prepared.set_dir, path, &prepared.set_files),
                &mut renderer,
            ) {
                Ok(Some(sb)) => renderer.set_storyboard(Some(sb)),
                Ok(None) => renderer.set_storyboard(None),
                Err(err) => {
                    println!("   warn: storyboard disabled: {}", err);
                    renderer.set_storyboard(None);
                }
            }
        } else {
            renderer.set_storyboard(None);
        }
        let column_widths: Vec<u32> = prepared
            .layout
            .columns
            .iter()
            .map(|column| column.width)
            .collect();
        renderer.precompute_ln_body_atlases(&prepared.skin, &column_widths);
        renderer.precompute_note_atlases(&prepared.skin, &column_widths);
        renderer.prescale_hud_digits(&prepared.skin);
        if self.settings.ln_debug {
            let avg_col_width =
                column_widths.iter().sum::<u32>() / column_widths.len().max(1) as u32;
            renderer.preload_debug_textures(&prepared.render_judgments, avg_col_width);
        }
        renderer.set_scroll_timeline(prepared.scroll_model.clone());
        let frame_time_ms = 1000.0 / self.settings.fps as f64;
        let first_note_time = prepared
            .beatmap
            .hit_objects
            .iter()
            .map(|hit_object| hit_object.time)
            .min();
        let last_note_time = prepared
            .beatmap
            .hit_objects
            .iter()
            .map(|hit_object| hit_object.time)
            .max();
        let default_preview_time = if prepared.beatmap.metadata.preview_time > 0 {
            prepared.beatmap.metadata.preview_time
        } else {
            let first = first_note_time.unwrap_or(0);
            let last = last_note_time.unwrap_or(first);
            (first + 5_000).min(last).max(first)
        };
        let requested_beatmap_time = preview_time_ms.unwrap_or(default_preview_time);
        let max_output_elapsed_ms =
            prepared.main_scene_total_frames.saturating_sub(1) as f64 * frame_time_ms;
        let output_elapsed_ms = prepared
            .playback_clock
            .output_elapsed_ms_for_beatmap_time(requested_beatmap_time as f64)
            .clamp(0.0, max_output_elapsed_ms.max(0.0));
        // Preview requests are beatmap-time based, but frame selection must respect output time.
        let frame_time = prepared
            .playback_clock
            .beatmap_time_ms_for_output_elapsed(output_elapsed_ms);
        let animation_time_ms = prepared
            .playback_clock
            .beatmap_time_for_output_elapsed_ms(output_elapsed_ms + frame_time_ms);
        let distance_at_ms = |time_ms: i32| -> f64 {
            let display_time_ms = prepared
                .playback_clock
                .output_elapsed_ms_for_beatmap_time(time_ms as f64)
                .round() as i32;
            if let Some(model) = prepared.scroll_model.as_ref() {
                model.object_distance_at_ms(display_time_ms)
            } else {
                (prepared.visual_pps as f64 * display_time_ms as f64) / 1000.0
            }
        };
        let mut note_window = NoteWindow::default();
        const EXTRA_VISIBLE_PX: f64 = 12.0;
        let look_behind = 500;
        let visible_dist_px =
            (prepared.layout.stage.hit_y - prepared.layout.stage.top_y).max(1) as f64;
        let current_dist = distance_at_ms(frame_time);
        let behind_dist = if look_behind > 0 {
            (current_dist - distance_at_ms(frame_time - look_behind)).abs()
        } else {
            0.0
        };
        let start_dist = current_dist - behind_dist;
        let end_dist = current_dist + visible_dist_px + EXTRA_VISIBLE_PX;
        while note_window.start < prepared.effective_end_distances.len()
            && prepared.effective_end_distances[note_window.start] < start_dist
        {
            note_window.start += 1;
        }
        note_window.end = prepared
            .note_distances
            .partition_point(|dist| *dist <= end_dist);
        if note_window.end < note_window.start {
            note_window.end = note_window.start;
        }
        let active_indices = &prepared.sorted_indices[note_window.range()];
        let submitted_note_indices: &[usize] = if hud_editor_preview {
            &[]
        } else {
            active_indices
        };
        let key_mask = if hud_editor_preview {
            0
        } else {
            self.get_key_mask_at(frame_time, &prepared.key_timeline)
        };
        let gameplay_progress_output_ms = prepared
            .playback_clock
            .output_elapsed_ms_for_beatmap_time(prepared.end_sequence.gameplay_end_ms as f64)
            .max(frame_time_ms);
        let gameplay_progress_frames = (gameplay_progress_output_ms / frame_time_ms)
            .ceil()
            .max(1.0) as usize;
        let frame_index = (output_elapsed_ms / frame_time_ms).floor().max(0.0) as usize;
        let score_events = prepared.combo_data.score_events.clone();
        let mut hud_state = renderer.compute_hud_state(
            frame_time,
            &score_events,
            &prepared.score_judgments,
            prepared.score_scale,
            frame_index.min(gameplay_progress_frames.saturating_sub(1)),
            gameplay_progress_frames,
            prepared.score_state_end_time_ms,
            Some(&prepared.health_timeline),
            Some(&prepared.render_windows),
        );
        hud_state.hud_visible = prepared.end_sequence.hud_visible_at(frame_time);
        hud_state.is_break_time =
            prepared.beatmap.events.breaks.iter().any(|break_period| {
                frame_time >= break_period.start && frame_time < break_period.end
            });
        if hud_editor_preview {
            hud_state.hud_visible = true;
            hud_state.score = 1_000_000;
            hud_state.accuracy = 1.0;
            hud_state.combo = 999;
            hud_state.judgment_counts = [0, 0, 0, 0, 0, 1];
            hud_state.progress = 0.5;
            hud_state.life = 1.0;
            hud_state.key_down_mask = 0;
            hud_state.key_kps = [0.0; 32];
            hud_state.key_press_duration_ms = [0; 32];
            hud_state.total_kps = 0.0;
            hud_state.unstable_rate = prepared
                .results_data
                .as_ref()
                .map(|data| data.timing_summary.unstable_rate);
            hud_state.is_break_time = false;
            hud_state.has_failed = false;
            hud_state.fail_started_at = None;
            hud_state.last_judgment = Some(LastJudgment {
                kind: JudgmentKind::Miss,
                age_ms: 0,
                column: key_count.saturating_sub(1) / 2,
                hit_offset_ms: None,
            });
            hud_state.hit_error_judgments = vec![
                crate::renderer::HitErrorJudgment {
                    kind: JudgmentKind::Hit300,
                    offset_ms: -24,
                    age_ms: 650,
                },
                crate::renderer::HitErrorJudgment {
                    kind: JudgmentKind::Max,
                    offset_ms: 4,
                    age_ms: 240,
                },
                crate::renderer::HitErrorJudgment {
                    kind: JudgmentKind::Hit200,
                    offset_ms: 39,
                    age_ms: 1200,
                },
            ];
            hud_state.hit_error_moving_avg_ms = Some(2.4);
            hud_state.combo_break_anim = None;
            hud_state.combo_inc_anim = None;
            hud_state.combo_burst_anim = None;
        }
        let fail_anim_state = if hud_editor_preview {
            None
        } else {
            prepared.health_timeline.fail_time_ms.and_then(|fail_time| {
                if frame_time < fail_time {
                    return None;
                }
                let elapsed = (frame_time - fail_time).max(0);
                let progress = (elapsed as f32 / anim::FAIL_ANIM_MS as f32).clamp(0.0, 1.0);
                Some(FailAnimationState {
                    active: true,
                    fail_started_at: fail_time,
                    visual_time_ms: frame_time,
                    progress,
                    active_note_indices: active_indices.to_vec(),
                    frozen_key_mask: key_mask,
                })
            })
        };
        let playfield_cover = if hud_editor_preview {
            None
        } else {
            renderer.resolve_smoothed_playfield_cover(
                frame_time,
                prepared.cover_config,
                prepared.cover_metrics,
                &hud_state,
            )
        };
        if hud_editor_preview {
            let replay_mods = renderer.resolved_replay_mod_display_acronyms();
            if !replay_mods.is_empty() {
                if let Some(origin) = renderer.replay_mod_display_origin() {
                    let origin_label = match origin {
                        crate::types::ReplayOrigin::StableLegacy => "stable",
                        crate::types::ReplayOrigin::LazerExport => "lazer",
                    };
                    println!("   [hud-preview] replayModOrigin={origin_label}");
                }
                println!("   [hud-preview] replayMods={}", replay_mods.join(","));
            }
            for (name, rect) in renderer.measure_hud_editor_preview_components(
                &prepared.layout,
                &prepared.skin,
                &hud_state,
            ) {
                println!(
                    "   [hud-preview] component={} x={} y={} width={} height={}",
                    name, rect.x, rect.y, rect.width, rect.height
                );
            }
        }
        self.progress(70, "Submitting preview frame...");
        let submitted = renderer.submit_frame(
            frame_time,
            animation_time_ms,
            &prepared.layout,
            &prepared.skin,
            &prepared.beatmap.hit_objects,
            submitted_note_indices,
            &prepared.judgments_by_idx,
            &prepared.ln_release_by_idx,
            &hud_state,
            key_mask,
            prepared.visual_pps,
            Some(&prepared.render_windows),
            &prepared.barlines,
            fail_anim_state.as_ref(),
            playfield_cover.as_ref(),
        );
        if !submitted {
            return Err(ConvertError::Render(
                "GPU render submit failed during preview".into(),
            ));
        }
        self.progress(80, "Reading preview frame...");
        let frame = renderer
            .drain_ready_frame_blocking()
            .ok_or_else(|| ConvertError::Render("GPU preview frame was not produced".into()))?
            .to_vec();
        self.progress(90, "Saving preview frame...");
        let image = image::RgbaImage::from_raw(self.settings.width, self.settings.height, frame)
            .ok_or_else(|| {
                ConvertError::Render("failed to build preview image buffer".to_string())
            })?;
        image
            .save(output_path)
            .map_err(|e| ConvertError::Render(format!("failed to save preview png: {e}")))?;
        renderer.dispose();
        self.progress(100, "Preview done");
        println!("   saved: {}", output_path.display());
        Ok(output_path.to_path_buf())
    }
}
fn fmt_time(secs: f32) -> String {
    let t = secs as u32;
    format!("{:02}:{:02}:{:02}", t / 3600, (t % 3600) / 60, t % 60)
}
fn ease_out_cubic(progress: f32) -> f32 {
    let clamped = progress.clamp(0.0, 1.0);
    1.0 - (1.0 - clamped).powi(3)
}
const RESULTS_BACKGROUND_IMAGE_OPACITY: f32 = 0.40;
fn load_results_background_frame(
    background_path: Option<&Path>,
    width: u32,
    height: u32,
) -> Option<Vec<u8>> {
    let background_path = background_path?;
    let bytes = std::fs::read(background_path).ok()?;
    let image = crate::utils::image_proc::load_rgba(&bytes)?;
    let image = crate::utils::image_proc::resize_cover(&image, width.max(1), height.max(1));
    let mut canvas =
        image::RgbaImage::from_pixel(width.max(1), height.max(1), image::Rgba([0, 0, 0, 255]));
    for (dst, src) in canvas.pixels_mut().zip(image.pixels()) {
        *dst = blend_rgba_pixel(*dst, *src, RESULTS_BACKGROUND_IMAGE_OPACITY);
    }
    Some(canvas.into_raw())
}
fn blend_frames(background: &[u8], foreground: &[u8], foreground_alpha: f32) -> Vec<u8> {
    let foreground_alpha = foreground_alpha.clamp(0.0, 1.0);
    if foreground_alpha <= 0.0 {
        return background.to_vec();
    }
    if foreground_alpha >= 1.0 {
        return foreground.to_vec();
    }
    let len = background.len().min(foreground.len());
    let mut output = Vec::with_capacity(len);
    for (bg, fg) in background[..len]
        .chunks_exact(4)
        .zip(foreground[..len].chunks_exact(4))
    {
        let mixed = blend_rgba_pixel(
            image::Rgba([bg[0], bg[1], bg[2], bg[3]]),
            image::Rgba([fg[0], fg[1], fg[2], fg[3]]),
            foreground_alpha,
        );
        output.extend_from_slice(&mixed.0);
    }
    output
}
fn blend_rgba_pixel(dst: image::Rgba<u8>, src: image::Rgba<u8>, opacity: f32) -> image::Rgba<u8> {
    let src_alpha = (src[3] as f32 / 255.0) * opacity.clamp(0.0, 1.0);
    if src_alpha <= 0.0 {
        return dst;
    }
    let inv_alpha = 1.0 - src_alpha;
    image::Rgba([
        (src[0] as f32 * src_alpha + dst[0] as f32 * inv_alpha).round() as u8,
        (src[1] as f32 * src_alpha + dst[1] as f32 * inv_alpha).round() as u8,
        (src[2] as f32 * src_alpha + dst[2] as f32 * inv_alpha).round() as u8,
        255,
    ])
}

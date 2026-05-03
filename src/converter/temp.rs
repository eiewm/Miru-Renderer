use super::*;
pub(crate) struct AttemptOutputFile {
    final_path: PathBuf,
    temp_path: Option<PathBuf>,
}
impl AttemptOutputFile {
    pub(crate) fn new(final_path: &Path, attempt_index: usize) -> Self {
        let token = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        Self {
            final_path: final_path.to_path_buf(),
            temp_path: Some(build_attempt_output_path(final_path, attempt_index, token)),
        }
    }
    pub(crate) fn temp_path(&self) -> &Path {
        self.temp_path
            .as_deref()
            .expect("temporary output path should exist until commit")
    }
    pub(crate) fn commit(mut self) -> std::io::Result<()> {
        let Some(temp_path) = self.temp_path.as_ref().cloned() else {
            return Ok(());
        };
        // Only replace the final output after ffmpeg has produced a complete attempt file.
        if self.final_path.exists() {
            std::fs::remove_file(&self.final_path)?;
        }
        std::fs::rename(&temp_path, &self.final_path)?;
        self.temp_path = None;
        Ok(())
    }
}
impl Drop for AttemptOutputFile {
    fn drop(&mut self) {
        if let Some(path) = self.temp_path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}
#[derive(Default)]
pub(crate) struct TempFileGuard {
    path: Option<PathBuf>,
}
impl TempFileGuard {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }
}
impl Drop for TempFileGuard {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}
#[derive(Default)]
pub(crate) struct TempDirGuard {
    path: Option<PathBuf>,
}
impl TempDirGuard {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }
}
impl Drop for TempDirGuard {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}
#[derive(Default)]
pub(crate) struct IntroUserDataGuard {
    data: Option<crate::utils::IntroUserData>,
}
impl IntroUserDataGuard {
    pub(crate) fn persistent(data: crate::utils::IntroUserData) -> Self {
        Self { data: Some(data) }
    }
    pub(crate) fn as_ref(&self) -> Option<&crate::utils::IntroUserData> {
        self.data.as_ref()
    }
}
pub(crate) fn retry_encoder_for_failure(
    failure: &ComposeFailure,
    attempt_index: usize,
) -> Option<VideoEncoder> {
    // Hardware encoders can fail late; retry once with software x264 before giving up.
    if attempt_index == 0 && failure.is_hardware_failure() {
        Some(VideoEncoder::X264)
    } else {
        None
    }
}
pub(crate) fn build_attempt_output_path(
    final_path: &Path,
    attempt_index: usize,
    token: u128,
) -> PathBuf {
    let parent = final_path.parent().unwrap_or(Path::new("."));
    let stem = final_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("output");
    let ext = final_path
        .extension()
        .and_then(|ext| ext.to_str())
        .filter(|ext| !ext.is_empty());
    let file_name = if let Some(ext) = ext {
        format!("{stem}.miru-attempt{attempt_index}.{token}.partial.{ext}")
    } else {
        format!("{stem}.miru-attempt{attempt_index}.{token}.partial")
    };
    parent.join(file_name)
}

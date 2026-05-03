use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntroUserData {
    pub avatar_path: Option<PathBuf>,
    pub country_code: Option<String>,
    pub flag_path: Option<PathBuf>,
    pub team_badge_path: Option<PathBuf>,
}
impl IntroUserData {
    pub fn resolve_relative_to(&mut self, base_dir: &Path) {
        // User-data JSON paths are authored next to that JSON file, not necessarily the process CWD.
        self.avatar_path = resolve_optional_path(self.avatar_path.take(), base_dir);
        self.flag_path = resolve_optional_path(self.flag_path.take(), base_dir);
        self.team_badge_path = resolve_optional_path(self.team_badge_path.take(), base_dir);
    }
}
fn resolve_optional_path(path: Option<PathBuf>, base_dir: &Path) -> Option<PathBuf> {
    path.map(|path| {
        if path.is_relative() {
            base_dir.join(path)
        } else {
            path
        }
    })
}

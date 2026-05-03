use std::path::{Component, Path, PathBuf};
pub fn sanitize_archive_entry_name(entry_name: &str) -> Option<String> {
    // Archive entries are untrusted; normalize separators before rejecting absolute or escaping paths.
    let normalized = entry_name.replace('\\', "/");
    let mut safe_components = Vec::new();
    for component in Path::new(&normalized).components() {
        match component {
            Component::Normal(value) => {
                let value = value.to_str()?;
                if value.is_empty() || value.contains('\0') || value.contains(':') {
                    return None;
                }
                safe_components.push(value.to_owned());
            }
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => return None,
        }
    }
    if safe_components.is_empty() {
        return None;
    }
    Some(safe_components.join("/"))
}
pub fn safe_archive_entry_path(dest_dir: &Path, entry_name: &str) -> Option<PathBuf> {
    sanitize_archive_entry_name(entry_name).map(|relative| dest_dir.join(relative))
}

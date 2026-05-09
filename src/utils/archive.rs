use std::path::{Path, PathBuf};

pub fn sanitize_archive_entry_name(entry_name: &str) -> Option<String> {
    // Archive entries are untrusted; normalize separators before rejecting absolute or escaping paths.
    let normalized = entry_name.replace('\\', "/");
    if normalized.starts_with('/') {
        return None;
    }

    let mut safe_components = Vec::new();
    for raw_component in normalized.split('/') {
        if raw_component.is_empty() || raw_component == "." {
            continue;
        }
        if raw_component == ".." {
            return None;
        }
        if safe_components.is_empty() && is_windows_drive_root(raw_component) {
            return None;
        }
        safe_components.push(sanitize_cross_platform_component(raw_component)?);
    }
    if safe_components.is_empty() {
        return None;
    }
    Some(safe_components.join("/"))
}

fn is_windows_drive_root(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn sanitize_cross_platform_component(value: &str) -> Option<String> {
    if value.is_empty() {
        return None;
    }

    let mut sanitized = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*' | '/' | '\\') {
            sanitized.push('_');
        } else {
            sanitized.push(ch);
        }
    }

    while sanitized.ends_with([' ', '.']) {
        sanitized.pop();
    }
    if sanitized.is_empty() {
        sanitized.push('_');
    }

    let stem = sanitized
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    ) {
        sanitized.insert(0, '_');
    }

    Some(sanitized)
}

pub fn safe_archive_entry_path(dest_dir: &Path, entry_name: &str) -> Option<PathBuf> {
    sanitize_archive_entry_name(entry_name).map(|relative| dest_dir.join(relative))
}

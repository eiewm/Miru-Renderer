use crate::utils::safe_archive_entry_path;
use std::fs::{self, File};
use std::io::{BufReader, Read, Seek, Write};
use std::path::Path;
#[derive(Debug, Clone, Copy)]
pub(crate) struct ArchiveExtractionLimits {
    pub max_entries: usize,
    pub max_files: usize,
    pub max_osu_files: usize,
    pub max_total_bytes: u64,
    pub max_entry_bytes: u64,
    pub max_path_depth: usize,
    pub max_relative_path_bytes: usize,
}
pub(crate) const DEFAULT_ARCHIVE_EXTRACTION_LIMITS: ArchiveExtractionLimits =
    ArchiveExtractionLimits {
        // These caps reject zip bombs while still allowing large osu! mapsets.
        max_entries: 4096,
        max_files: 4096,
        max_osu_files: 512,
        max_total_bytes: 512 * 1024 * 1024,
        max_entry_bytes: 256 * 1024 * 1024,
        max_path_depth: 32,
        max_relative_path_bytes: 240,
    };
pub(crate) fn extract_archive_to_dir(
    zip_path: &Path,
    dest_dir: &Path,
    limits: ArchiveExtractionLimits,
) -> Result<(), String> {
    let file = File::open(zip_path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| format!("invalid zip: {e}"))?;
    extract_archive(&mut archive, dest_dir, limits)
}
fn extract_archive<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    dest_dir: &Path,
    limits: ArchiveExtractionLimits,
) -> Result<(), String> {
    if archive.len() > limits.max_entries {
        return Err(format!(
            "zip archive has too many entries: {} > {}",
            archive.len(),
            limits.max_entries
        ));
    }
    remove_path_if_exists(dest_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;
    let extraction_result = (|| {
        let mut file_count = 0usize;
        let mut osu_file_count = 0usize;
        let mut total_bytes = 0u64;
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("zip entry error: {e}"))?;
            let name = entry.name().to_string();
            // Validate the path before any write so archive entries stay inside dest_dir.
            let Some(out_path) = safe_archive_entry_path(dest_dir, &name) else {
                return Err(format!("unsafe zip entry path rejected: {name}"));
            };
            let relative_path = out_path
                .strip_prefix(dest_dir)
                .map_err(|_| format!("entry escaped extraction root: {name}"))?;
            let relative_path_str = relative_path.to_string_lossy();
            if relative_path_str.len() > limits.max_relative_path_bytes {
                return Err(format!(
                    "zip entry path too long: {} bytes > {} bytes",
                    relative_path_str.len(),
                    limits.max_relative_path_bytes
                ));
            }
            if relative_path.components().count() > limits.max_path_depth {
                return Err(format!(
                    "zip entry path too deep: {} components > {} components",
                    relative_path.components().count(),
                    limits.max_path_depth
                ));
            }
            if entry.is_dir() {
                fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
                continue;
            }
            file_count += 1;
            if file_count > limits.max_files {
                return Err(format!(
                    "zip archive has too many files: {} > {}",
                    file_count, limits.max_files
                ));
            }
            let header_size = entry.size();
            if header_size > limits.max_entry_bytes {
                return Err(format!(
                    "zip entry exceeds max per-file bytes: {} > {}",
                    header_size, limits.max_entry_bytes
                ));
            }
            let is_osu_file = out_path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("osu"))
                .unwrap_or(false);
            if is_osu_file {
                osu_file_count += 1;
                if osu_file_count > limits.max_osu_files {
                    return Err(format!(
                        "zip archive has too many .osu files: {} > {}",
                        osu_file_count, limits.max_osu_files
                    ));
                }
            }
            let remaining_total_bytes = limits
                .max_total_bytes
                .checked_sub(total_bytes)
                .ok_or_else(|| "zip archive extracted size overflow".to_string())?;
            if header_size > remaining_total_bytes {
                return Err(format!(
                    "zip archive exceeds max total extracted bytes: {} + {} > {}",
                    total_bytes, header_size, limits.max_total_bytes
                ));
            }
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let entry_limit = remaining_total_bytes.min(limits.max_entry_bytes);
            let written = copy_reader_to_file_with_limit(&mut entry, &out_path, entry_limit)?;
            total_bytes = total_bytes
                .checked_add(written)
                .ok_or_else(|| "zip archive extracted size overflow".to_string())?;
        }
        Ok(())
    })();
    if extraction_result.is_err() {
        // Failed extractions must not leave a partial mapset that later loads as valid.
        let _ = remove_path_if_exists(dest_dir);
    }
    extraction_result
}
fn copy_reader_to_file_with_limit<R: Read>(
    mut reader: R,
    output_path: &Path,
    max_bytes: u64,
) -> Result<u64, String> {
    let mut output = File::create(output_path).map_err(|e| e.to_string())?;
    let mut buffer = [0u8; 64 * 1024];
    let mut written = 0u64;
    loop {
        let read = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        written = written
            .checked_add(read as u64)
            .ok_or_else(|| "zip entry size overflow".to_string())?;
        if written > max_bytes {
            // Zip headers can underreport sizes, so enforce the limit while streaming.
            drop(output);
            let _ = fs::remove_file(output_path);
            return Err(format!(
                "zip entry exceeds extraction limit while streaming: {} > {}",
                written, max_bytes
            ));
        }
        output
            .write_all(&buffer[..read])
            .map_err(|e| e.to_string())?;
    }
    Ok(written)
}
fn remove_path_if_exists(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ArchiveSecurityError {
    #[error("Güvenlik İhlali: Path traversal (Zip Slip) tespit edildi: '{0}'")]
    PathTraversal(String),

    #[error("Güvenlik İhlali: Sıkıştırma oranı limiti aşıldı (Zip Bomb şüphesi). Oran: {ratio:.1}:1")]
    DecompressionRatioExceeded { ratio: f64 },

    #[error("Güvenlik İhlali: Maksimum arşiv boyutu aşıldı: {current_bytes} > {max_bytes} bayt")]
    MaxExtractedBytesExceeded { current_bytes: u64, max_bytes: u64 },

    #[error("Güvenlik İhlali: Maksimum dosya sayısı aşıldı: {current_files} > {max_files}")]
    MaxFileCountExceeded { current_files: usize, max_files: usize },

    #[error("Güvenlik İhlali: Yasaklı çalıştırılabilir dosya türü tespit edildi: '{0}'")]
    ForbiddenExecutableExtension(String),

    #[error("Arşiv Hatası: {0}")]
    ArchiveError(String),

    #[error("G/Ç Hatası: {0}")]
    Io(String),
}

#[derive(Debug, Clone)]
pub struct ArchiveSecurityLimits {
    pub max_file_count: usize,
    pub max_total_decompressed_bytes: u64,
    pub max_decompression_ratio: f64,
}

impl Default for ArchiveSecurityLimits {
    fn default() -> Self {
        Self {
            max_file_count: 10_000,
            max_total_decompressed_bytes: 20 * 1024 * 1024 * 1024, // 20 GB
            max_decompression_ratio: 100.0,
        }
    }
}

const FORBIDDEN_EXTENSIONS: &[&str] = &[
    "exe", "bat", "cmd", "ps1", "vbs", "vbe", "jse", "wsf", "wsh", "msc", "dll", "scr", "com",
];

/// Verilen dosya yolunun hedef dizin sınırları içinde kalıp kalmadığını doğrular (Zip-Slip önleme)
pub fn validate_archive_path(relative_path: &str, target_dir: &Path) -> Result<PathBuf, ArchiveSecurityError> {
    let clean_path = relative_path.replace('\\', "/");
    let path = Path::new(&clean_path);

    for component in path.components() {
        match component {
            Component::ParentDir => {
                return Err(ArchiveSecurityError::PathTraversal(relative_path.to_string()));
            }
            Component::Prefix(_) | Component::RootDir => {
                return Err(ArchiveSecurityError::PathTraversal(relative_path.to_string()));
            }
            Component::Normal(_) | Component::CurDir => {}
        }
    }

    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let lower_ext = ext.to_lowercase();
        if FORBIDDEN_EXTENSIONS.contains(&lower_ext.as_str()) {
            return Err(ArchiveSecurityError::ForbiddenExecutableExtension(relative_path.to_string()));
        }
    }

    let full_path = target_dir.join(path);
    Ok(full_path)
}

/// Güvenli arşiv çıkarma işlemi
pub fn extract_archive_safely<R: Read + std::io::Seek>(
    reader: R,
    target_dir: &Path,
    limits: &ArchiveSecurityLimits,
) -> Result<usize, ArchiveSecurityError> {
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| ArchiveSecurityError::ArchiveError(e.to_string()))?;

    if archive.len() > limits.max_file_count {
        return Err(ArchiveSecurityError::MaxFileCountExceeded {
            current_files: archive.len(),
            max_files: limits.max_file_count,
        });
    }

    let mut total_decompressed_bytes: u64 = 0;
    let mut total_compressed_bytes: u64 = 0;
    let mut extracted_count = 0;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)
            .map_err(|e| ArchiveSecurityError::ArchiveError(e.to_string()))?;

        let raw_name = file.name().to_string();
        let destination = validate_archive_path(&raw_name, target_dir)?;

        total_compressed_bytes += file.compressed_size();

        if file.is_dir() {
            std::fs::create_dir_all(&destination)
                .map_err(|e| ArchiveSecurityError::Io(e.to_string()))?;
            continue;
        }

        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ArchiveSecurityError::Io(e.to_string()))?;
        }

        let mut output_file = File::create(&destination)
            .map_err(|e| ArchiveSecurityError::Io(e.to_string()))?;

        let mut buffer = [0u8; 8192];
        loop {
            let bytes_read = file.read(&mut buffer)
                .map_err(|e| ArchiveSecurityError::Io(e.to_string()))?;
            if bytes_read == 0 {
                break;
            }

            total_decompressed_bytes += bytes_read as u64;

            if total_decompressed_bytes > limits.max_total_decompressed_bytes {
                return Err(ArchiveSecurityError::MaxExtractedBytesExceeded {
                    current_bytes: total_decompressed_bytes,
                    max_bytes: limits.max_total_decompressed_bytes,
                });
            }

            if total_compressed_bytes > 0 {
                let ratio = total_decompressed_bytes as f64 / total_compressed_bytes as f64;
                if ratio > limits.max_decompression_ratio && total_decompressed_bytes > 10 * 1024 * 1024 {
                    return Err(ArchiveSecurityError::DecompressionRatioExceeded { ratio });
                }
            }

            output_file.write_all(&buffer[..bytes_read])
                .map_err(|e| ArchiveSecurityError::Io(e.to_string()))?;
        }

        extracted_count += 1;
    }

    Ok(extracted_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;

    #[test]
    fn test_zip_slip_rejection() {
        let target = Path::new("C:/games/instance");
        
        let bad_paths = [
            "../evil.jar",
            "../../windows/system32/cmd.exe",
            "foo/../../bar",
            "/absolute/path/mod.jar",
            "C:\\windows\\evil.dll",
        ];

        for path in bad_paths {
            assert!(
                validate_archive_path(path, target).is_err(),
                "Yol güvenliği ihlalini yakalayamadı: {}",
                path
            );
        }
    }

    #[test]
    fn test_valid_path_allowed() {
        let target = Path::new("C:/games/instance");
        let valid_paths = [
            "mods/jei.jar",
            "config/jei-client.ini",
            "resourcepacks/faithful.zip",
        ];

        for path in valid_paths {
            let res = validate_archive_path(path, target);
            assert!(res.is_ok(), "Geçerli yol reddedildi: {}", path);
        }
    }

    #[test]
    fn test_forbidden_extension_rejection() {
        let target = Path::new("C:/games/instance");
        let bad_ext = "mods/virus.exe";
        let res = validate_archive_path(bad_ext, target);
        assert_eq!(
            res.unwrap_err(),
            ArchiveSecurityError::ForbiddenExecutableExtension("mods/virus.exe".to_string())
        );
    }

    #[test]
    fn test_safe_extraction_success() {
        let dir = tempdir().unwrap();
        let mut buf = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(Cursor::new(&mut buf));
            writer.start_file("mods/sample.txt", SimpleFileOptions::default()).unwrap();
            writer.write_all(b"Hello safe archive").unwrap();
            writer.finish().unwrap();
        }

        let limits = ArchiveSecurityLimits::default();
        let count = extract_archive_safely(Cursor::new(&buf), dir.path(), &limits).unwrap();
        assert_eq!(count, 1);
        assert!(dir.path().join("mods/sample.txt").exists());
    }
}


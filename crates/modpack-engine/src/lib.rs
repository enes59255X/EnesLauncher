use archive_security::validate_archive_path;
use download_engine::{DownloadEngine, DownloadTask};
use instance_manager::InstanceManager;
use launcher_core::{paths, LauncherError, LauncherResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use uuid::Uuid;
use zip::write::SimpleFileOptions;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthIndex {
    pub format_version: u32,
    pub game: String,
    pub version_id: String,
    pub name: String,
    pub summary: Option<String>,
    pub files: Vec<MrpackFile>,
    pub dependencies: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MrpackFile {
    pub path: String,
    pub hashes: HashMap<String, String>,
    pub env: Option<MrpackEnv>,
    pub downloads: Vec<String>,
    pub file_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MrpackEnv {
    pub client: Option<String>, // "required", "optional", "unsupported"
    pub server: Option<String>,
}

pub struct ModpackEngine;

impl ModpackEngine {
    /// .mrpack arşivini doğrular ve manifesti ayrıştırır
    pub fn read_manifest<P: AsRef<Path>>(archive_path: P) -> LauncherResult<ModrinthIndex> {
        let file = File::open(archive_path)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| LauncherError::Validation(format!("Geçersiz .mrpack zip arşivi: {}", e)))?;

        let mut index_file = archive
            .by_name("modrinth.index.json")
            .map_err(|_| LauncherError::Validation(".mrpack içinde 'modrinth.index.json' bulunamadı".to_string()))?;

        let mut content = String::new();
        index_file.read_to_string(&mut content)?;

        let manifest: ModrinthIndex = serde_json::from_str(&content)
            .map_err(|e| LauncherError::Validation(format!("Manifest JSON şema hatası: {}", e)))?;

        if manifest.format_version != 1 {
            return Err(LauncherError::Validation(format!(
                "Desteklenmeyen .mrpack format sürümü: {}",
                manifest.format_version
            )));
        }

        if manifest.game != "minecraft" {
            return Err(LauncherError::Validation(format!(
                "Desteklenmeyen oyun türü: {}",
                manifest.game
            )));
        }

        Ok(manifest)
    }

    /// .mrpack mod paketini staging alanı, yedekleme ve rollback korumasıyla instance'a kurar
    pub async fn import_modpack(
        archive_path: &Path,
        instance_dir: &Path,
        download_engine: &DownloadEngine,
    ) -> LauncherResult<ModrinthIndex> {
        let manifest = Self::read_manifest(archive_path)?;

        let task_id = Uuid::new_v4().to_string();
        let staging_dir = paths::get_staging_dir().join(&task_id);
        let backups_dir = paths::get_backups_dir().join(&task_id);
        tokio::fs::create_dir_all(&staging_dir).await?;

        // 1. İndirme görevlerini derle
        let mut download_tasks = Vec::new();

        for file in &manifest.files {
            // İstemcide desteklenmeyen modları atla
            if let Some(ref env) = file.env {
                if let Some(ref client_env) = env.client {
                    if client_env == "unsupported" {
                        continue;
                    }
                }
            }

            // Güvenlik doğrulaması: Staging dışına çıkan yollar anında reddedilir
            let clean_dest = validate_archive_path(&file.path, &staging_dir)
                .map_err(|e| LauncherError::Security(e.to_string()))?;

            if let Some(url) = file.downloads.first() {
                download_tasks.push(DownloadTask {
                    id: format!("mrpack-file-{}", file.path),
                    url: url.clone(),
                    destination: clean_dest,
                    expected_sha1: file.hashes.get("sha1").cloned(),
                    expected_size: Some(file.file_size),
                });
            }
        }

        // 2. Mod dosyalarını staging alanına indir
        if let Err(err) = download_engine.download_all(download_tasks).await {
            let _ = tokio::fs::remove_dir_all(&staging_dir).await;
            return Err(LauncherError::Network(format!("Modpack dosyaları indirilemedi: {}", err)));
        }

        // 3. Arşiv içindeki overrides/ dizinini güvenle staging alanına çıkart (Zip-Slip denetimiyle)
        let archive_file = File::open(archive_path)?;
        let mut archive = zip::ZipArchive::new(archive_file)
            .map_err(|e| LauncherError::Validation(e.to_string()))?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)
                .map_err(|e| LauncherError::Validation(e.to_string()))?;
            let name = file.name().to_string();

            if let Some(rel_override) = name.strip_prefix("overrides/") {
                if rel_override.is_empty() {
                    continue;
                }

                let target_path = validate_archive_path(rel_override, &staging_dir)
                    .map_err(|e| LauncherError::Security(e.to_string()))?;

                if file.is_dir() {
                    std::fs::create_dir_all(&target_path)?;
                    continue;
                }

                if let Some(parent) = target_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                let mut out_file = File::create(&target_path)?;
                std::io::copy(&mut file, &mut out_file)?;
            }
        }

        // 4. Mevcut dosyaların yedeğini al (Rollback desteği)
        if instance_dir.exists() {
            tokio::fs::create_dir_all(&backups_dir).await?;
            let _ = copy_directory_recursive(instance_dir, &backups_dir).await;
        }

        // 5. Staging alanından instance dizinine atomik transfer
        InstanceManager::ensure_instance_layout(instance_dir).await?;
        if let Err(copy_err) = copy_directory_recursive(&staging_dir, instance_dir).await {
            // Hata oluştu! Rollback: Yedeği geri yükle
            tracing::error!("Modpack transferi başarısız oldu, geri alınıyor: {}", copy_err);
            if backups_dir.exists() {
                let _ = copy_directory_recursive(&backups_dir, instance_dir).await;
            }
            let _ = tokio::fs::remove_dir_all(&staging_dir).await;
            return Err(LauncherError::Io(copy_err));
        }

        // 6. Başarılı olduysa staging ve geçici yedeği temizle
        let _ = tokio::fs::remove_dir_all(&staging_dir).await;
        let _ = tokio::fs::remove_dir_all(&backups_dir).await;

        Ok(manifest)
    }

    /// Bir instance'ı standart .mrpack formatında dışa aktarır
    pub fn export_instance_to_mrpack(
        instance_dir: &Path,
        export_path: &Path,
        pack_name: &str,
        mc_version: &str,
        loader_name: &str,
        loader_version: &str,
    ) -> LauncherResult<()> {
        if let Some(parent) = export_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let zip_file = File::create(export_path)?;
        let mut zip_writer = zip::ZipWriter::new(zip_file);
        let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        // 1. modrinth.index.json oluştur
        let mut dependencies = HashMap::new();
        dependencies.insert("minecraft".to_string(), mc_version.to_string());
        dependencies.insert(loader_name.to_lowercase(), loader_version.to_string());

        let index = ModrinthIndex {
            format_version: 1,
            game: "minecraft".to_string(),
            version_id: format!("{}-1.0.0", sanitize_name(pack_name)),
            name: pack_name.to_string(),
            summary: Some("EnesLauncher tarafından dışa aktarıldı.".to_string()),
            files: Vec::new(),
            dependencies,
        };

        let index_json = serde_json::to_string_pretty(&index)?;
        zip_writer.start_file("modrinth.index.json", options)
            .map_err(|e| LauncherError::Validation(e.to_string()))?;
        zip_writer.write_all(index_json.as_bytes())?;

        // 2. overrides/ dizinine config ve options.txt dosyalarını ekle
        let config_dir = instance_dir.join("config");
        if config_dir.is_dir() {
            for entry in walkdir::WalkDir::new(&config_dir).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    let rel_path = entry.path().strip_prefix(instance_dir).unwrap_or(entry.path());
                    let zip_path = format!("overrides/{}", rel_path.to_string_lossy().replace('\\', "/"));

                    zip_writer.start_file(&zip_path, options)
                        .map_err(|e| LauncherError::Validation(e.to_string()))?;
                    let mut src_file = File::open(entry.path())?;
                    std::io::copy(&mut src_file, &mut zip_writer)?;
                }
            }
        }

        zip_writer.finish().map_err(|e| LauncherError::Validation(e.to_string()))?;
        Ok(())
    }
}

async fn copy_directory_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    tokio::fs::create_dir_all(dst).await?;
    let mut entries = tokio::fs::read_dir(src).await?;

    while let Some(entry) = entries.next_entry().await? {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if entry.file_type().await?.is_dir() {
            Box::pin(copy_directory_recursive(&src_path, &dst_path)).await?;
        } else {
            tokio::fs::copy(&src_path, &dst_path).await?;
        }
    }

    Ok(())
}

fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_mrpack_read_and_export_lifecycle() {
        let dir = tempdir().unwrap();
        let inst_dir = dir.path().join("instance");
        let export_zip = dir.path().join("exported.mrpack");
        std::fs::create_dir_all(inst_dir.join("config")).unwrap();
        std::fs::write(inst_dir.join("config").join("test.cfg"), b"setting=true").unwrap();

        // 1. Dışa aktarma
        ModpackEngine::export_instance_to_mrpack(
            &inst_dir,
            &export_zip,
            "Odyssey Pack",
            "1.21.1",
            "fabric",
            "0.16.5",
        ).unwrap();

        assert!(export_zip.is_file());

        // 2. İçe aktarma manifest doğrulaması
        let manifest = ModpackEngine::read_manifest(&export_zip).unwrap();
        assert_eq!(manifest.name, "Odyssey Pack");
        assert_eq!(manifest.format_version, 1);
        assert_eq!(manifest.dependencies.get("minecraft").unwrap(), "1.21.1");
        assert_eq!(manifest.dependencies.get("fabric").unwrap(), "0.16.5");
    }

    #[test]
    fn test_bad_mrpack_rejection() {
        let dir = tempdir().unwrap();
        let bad_zip = dir.path().join("corrupted.mrpack");
        std::fs::write(&bad_zip, b"not a zip file").unwrap();

        assert!(ModpackEngine::read_manifest(&bad_zip).is_err());
    }
}


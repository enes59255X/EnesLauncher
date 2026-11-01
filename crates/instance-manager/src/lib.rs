use launcher_core::{LauncherError, LauncherResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledModInfo {
    pub file_name: String,
    pub is_enabled: bool,
    pub file_size: u64,
    pub sha512_hash: Option<String>,
}

pub struct InstanceManager;

impl InstanceManager {
    /// Her instance için tamamen izole dizin yapısını garanti eder
    pub async fn ensure_instance_layout(instance_dir: &Path) -> LauncherResult<()> {
        let subdirectories = [
            "mods",
            "saves",
            "config",
            "resourcepacks",
            "shaderpacks",
            "logs",
            "natives",
        ];

        for sub in subdirectories {
            let dir = instance_dir.join(sub);
            fs::create_dir_all(&dir).await?;
        }

        Ok(())
    }

    /// Bir instance içindeki kurulu modları listeler
    pub async fn list_installed_mods(instance_dir: &Path) -> LauncherResult<Vec<InstalledModInfo>> {
        let mods_dir = instance_dir.join("mods");
        if !mods_dir.is_dir() {
            return Ok(Vec::new());
        }

        let mut mods = Vec::new();
        let mut entries = fs::read_dir(&mods_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let file_name = entry.file_name().to_string_lossy().to_string();
            let is_jar = file_name.ends_with(".jar");
            let is_disabled = file_name.ends_with(".jar.disabled");

            if !is_jar && !is_disabled {
                continue;
            }

            let metadata = entry.metadata().await?;
            let sha512 = calculate_file_sha512(&path).await.ok();

            mods.push(InstalledModInfo {
                file_name,
                is_enabled: is_jar,
                file_size: metadata.len(),
                sha512_hash: sha512,
            });
        }

        mods.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        Ok(mods)
    }

    /// Bir modu etkinleştirir veya devre dışı bırakır (.jar <-> .jar.disabled)
    pub async fn toggle_mod(instance_dir: &Path, file_name: &str) -> LauncherResult<String> {
        let mods_dir = instance_dir.join("mods");
        let current_path = mods_dir.join(file_name);

        if !current_path.is_file() {
            return Err(LauncherError::NotFound(format!("Mod bulunamadı: {}", file_name)));
        }

        let new_file_name = if file_name.ends_with(".jar") {
            format!("{}.disabled", file_name)
        } else if file_name.ends_with(".jar.disabled") {
            file_name.trim_end_matches(".disabled").to_string()
        } else {
            return Err(LauncherError::Validation("Geçersiz mod uzantısı".to_string()));
        };

        let new_path = mods_dir.join(&new_file_name);
        fs::rename(&current_path, &new_path).await?;

        Ok(new_file_name)
    }

    /// Bir modu fiziksel olarak siler
    pub async fn delete_mod(instance_dir: &Path, file_name: &str) -> LauncherResult<()> {
        let mods_dir = instance_dir.join("mods");
        let target_file = mods_dir.join(file_name);

        if target_file.is_file() {
            fs::remove_file(&target_file).await?;
        }

        Ok(())
    }

    /// Bir mod dosyasını instance'a güvenle yazar
    pub async fn write_mod_file(
        instance_dir: &Path,
        file_name: &str,
        data: &[u8],
    ) -> LauncherResult<PathBuf> {
        let mods_dir = instance_dir.join("mods");
        fs::create_dir_all(&mods_dir).await?;

        // Güvenlik: Path traversal engelleme
        if file_name.contains('/') || file_name.contains('\\') || file_name.contains("..") {
            return Err(LauncherError::Security("Mod dosya adı geçersiz yol karakterleri içeriyor".to_string()));
        }

        let target_path = mods_dir.join(file_name);
        fs::write(&target_path, data).await?;

        Ok(target_path)
    }
}

pub async fn calculate_file_sha512(path: &Path) -> LauncherResult<String> {
    use tokio::io::AsyncReadExt;

    let mut file = fs::File::open(path).await?;
    let mut hasher = Sha512::new();
    let mut buffer = [0u8; 8192];

    loop {
        let count = file.read(&mut buffer).await?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_instance_layout_and_mod_lifecycle() {
        let dir = tempdir().unwrap();
        let inst_dir = dir.path();

        // 1. Dizin izolasyonu testi
        InstanceManager::ensure_instance_layout(inst_dir).await.unwrap();
        assert!(inst_dir.join("mods").is_dir());
        assert!(inst_dir.join("saves").is_dir());
        assert!(inst_dir.join("config").is_dir());

        // 2. Mod yazma ve listeleme
        let mod_content = b"fake jar payload";
        InstanceManager::write_mod_file(inst_dir, "sodium-fabric.jar", mod_content).await.unwrap();

        let mods = InstanceManager::list_installed_mods(inst_dir).await.unwrap();
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].file_name, "sodium-fabric.jar");
        assert!(mods[0].is_enabled);

        // 3. Modu devre dışı bırakma (.jar -> .jar.disabled)
        let toggled = InstanceManager::toggle_mod(inst_dir, "sodium-fabric.jar").await.unwrap();
        assert_eq!(toggled, "sodium-fabric.jar.disabled");

        let mods_after_toggle = InstanceManager::list_installed_mods(inst_dir).await.unwrap();
        assert_eq!(mods_after_toggle.len(), 1);
        assert!(!mods_after_toggle[0].is_enabled);

        // 4. Modu silme
        InstanceManager::delete_mod(inst_dir, "sodium-fabric.jar.disabled").await.unwrap();
        let mods_empty = InstanceManager::list_installed_mods(inst_dir).await.unwrap();
        assert_eq!(mods_empty.len(), 0);
    }
}


use crate::models::{AssetIndex, VersionManifest, VersionPackage};
use launcher_core::{LauncherError, LauncherResult};
use std::path::Path;

pub const PISTON_MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Clone)]
pub struct MojangClient {
    client: reqwest::Client,
}

impl Default for MojangClient {
    fn default() -> Self {
        Self::new()
    }
}

impl MojangClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("EnesLauncher/0.1.0")
            .build()
            .unwrap_or_default();
        Self { client }
    }

    pub async fn fetch_manifest(&self) -> LauncherResult<VersionManifest> {
        let resp = self
            .client
            .get(PISTON_MANIFEST_URL)
            .send()
            .await
            .map_err(|e| LauncherError::Network(format!("Manifest alınamadı: {}", e)))?;

        if !resp.status().is_success() {
            return Err(LauncherError::Network(format!(
                "Mojang sunucusu hata kodu döndürdü: {}",
                resp.status()
            )));
        }

        let manifest = resp
            .json::<VersionManifest>()
            .await
            .map_err(|e| LauncherError::Network(format!("Manifest JSON hatası: {}", e)))?;

        Ok(manifest)
    }

    pub async fn fetch_version_package(&self, url: &str) -> LauncherResult<VersionPackage> {
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| LauncherError::Network(format!("Sürüm detayları alınamadı: {}", e)))?;

        if !resp.status().is_success() {
            return Err(LauncherError::Network(format!(
                "Sürüm metaveri isteği başarısız: {}",
                resp.status()
            )));
        }

        let pkg = resp
            .json::<VersionPackage>()
            .await
            .map_err(|e| LauncherError::Network(format!("Sürüm paketi JSON hatası: {}", e)))?;

        Ok(pkg)
    }

    pub async fn fetch_asset_index(&self, url: &str) -> LauncherResult<AssetIndex> {
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| LauncherError::Network(format!("Asset indeksi alınamadı: {}", e)))?;

        let index = resp
            .json::<AssetIndex>()
            .await
            .map_err(|e| LauncherError::Network(format!("Asset indeksi JSON hatası: {}", e)))?;

        Ok(index)
    }

    pub async fn cache_manifest_to_disk(&self, manifest: &VersionManifest, path: &Path) -> LauncherResult<()> {
        let json = serde_json::to_string_pretty(manifest)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(path, json).await?;
        Ok(())
    }
}


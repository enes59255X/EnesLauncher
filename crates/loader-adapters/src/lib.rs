use launcher_core::{LauncherError, LauncherResult};
use minecraft_metadata::{DownloadArtifact, Library, LibraryDownloads, VersionPackage};
use serde::{Deserialize, Serialize};
use std::io::Read;

pub const FABRIC_META_BASE: &str = "https://meta.fabricmc.net/v2";

pub struct FabricAdapter {
    client: reqwest::Client,
}

impl Default for FabricAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl FabricAdapter {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("EnesLauncher/0.1.0")
            .build()
            .unwrap_or_default();
        Self { client }
    }

    /// Fabric sürüm metaverisini alır ve temel Vanilla VersionPackage üzerine kütüphaneleri ve mainClass'ı enjekte eder
    pub async fn patch_version_package(
        &self,
        base_pkg: &mut VersionPackage,
        loader_version: &str,
    ) -> LauncherResult<()> {
        let url = format!(
            "{}/versions/loader/{}/{}",
            FABRIC_META_BASE, base_pkg.id, loader_version
        );

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| LauncherError::Network(format!("Fabric metaveri isteği başarısız: {}", e)))?;

        if !resp.status().is_success() {
            return Err(LauncherError::Network(format!(
                "Fabric metaveri sunucusu hata döndürdü: {}",
                resp.status()
            )));
        }

        let fabric_profile = resp
            .json::<FabricProfileResponse>()
            .await
            .map_err(|e| LauncherError::Network(format!("Fabric JSON ayrıştırma hatası: {}", e)))?;

        // 1. KnotClient Main Class değiştir
        base_pkg.main_class = fabric_profile
            .launcher_meta
            .main_class
            .client
            .unwrap_or_else(|| "net.fabricmc.loader.impl.launch.knot.KnotClient".to_string());

        // 2. Fabric Kütüphanelerini ekle
        if let Some(common_libs) = fabric_profile.launcher_meta.libraries.common {
            for lib_entry in common_libs {
                base_pkg.libraries.push(convert_fabric_library(lib_entry));
            }
        }

        if let Some(client_libs) = fabric_profile.launcher_meta.libraries.client {
            for lib_entry in client_libs {
                base_pkg.libraries.push(convert_fabric_library(lib_entry));
            }
        }

        Ok(())
    }
}

pub struct ForgeAdapter {
    client: reqwest::Client,
}

impl Default for ForgeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ForgeAdapter {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) EnesLauncher/0.1.0")
            .build()
            .unwrap_or_default();
        Self { client }
    }

    /// Forge sürüm metaverisini temel Vanilla VersionPackage üzerine enjekte eder.
    /// Installer JAR'ı indirip içindeki version.json dosyasını okuyarak Forge kütüphanelerini ve argümanlarını tamamlar.
    pub async fn patch_version_package(
        &self,
        base_pkg: &mut VersionPackage,
        loader_version: &str,
    ) -> LauncherResult<()> {
        let mc_version = &base_pkg.id;
        let forge_artifact = format!("{}-{}", mc_version, loader_version);

        // Önce yerel shared/versions/ dizininde kurulmuş version.json var mı kontrol et
        let shared_dir = launcher_core::paths::get_shared_cache_dir();
        let possible_names = [
            format!("{}-forge-{}", mc_version, loader_version),
            format!("{}-{}", mc_version, loader_version),
            loader_version.to_string(),
        ];

        let mut local_vj = None;
        for name in &possible_names {
            let p = shared_dir
                .join("versions")
                .join(name)
                .join(format!("{}.json", name));
            if p.is_file() {
                if let Ok(c) = std::fs::read_to_string(&p) {
                    local_vj = Some(c);
                    break;
                }
            }
        }

        // Eğer tam sürüm bulunamadıysa, shared/versions altındaki aynı MC sürümüne ait kurulu herhangi bir Forge sürümünü bul (Örn: 1.20.1-forge-47.4.22)
        if local_vj.is_none() {
            let versions_dir = shared_dir.join("versions");
            if let Ok(entries) = std::fs::read_dir(&versions_dir) {
                let prefix = format!("{}-forge-", mc_version);
                for entry in entries.filter_map(|e| e.ok()) {
                    let dname = entry.file_name().to_string_lossy().to_string();
                    if dname.starts_with(&prefix) {
                        let candidate_json = entry.path().join(format!("{}.json", dname));
                        if candidate_json.is_file() {
                            if let Ok(c) = std::fs::read_to_string(&candidate_json) {
                                tracing::info!("İstenen Forge sürümü ({}) yerine kurulu uyumlu Forge ({}) kullanılıyor.", loader_version, dname);
                                local_vj = Some(c);
                                break;
                            }
                        }
                    }
                }
            }
        }

        let raw_json = if let Some(content) = local_vj {
            Some(content)
        } else {
            let installer_url = format!(
                "https://maven.minecraftforge.net/net/minecraftforge/forge/{}/forge-{}-installer.jar",
                forge_artifact, forge_artifact
            );

            if let Ok(resp) = self.client.get(&installer_url).send().await {
                if resp.status().is_success() {
                    if let Ok(bytes) = resp.bytes().await {
                        if let Ok(mut archive) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) {
                            // Maven altındaki jar dosyalarını shared/libraries/ dizinine çıkar
                            let lib_root = shared_dir.join("libraries");
                            for i in 0..archive.len() {
                                if let Ok(mut f) = archive.by_index(i) {
                                    let fname = f.name().to_string();
                                    if fname.starts_with("maven/") && fname.ends_with(".jar") {
                                        let rel = &fname["maven/".len()..];
                                        let target = lib_root.join(rel);
                                        if let Some(p) = target.parent() {
                                            let _ = std::fs::create_dir_all(p);
                                        }
                                        if let Ok(mut out) = std::fs::File::create(&target) {
                                            let _ = std::io::copy(&mut f, &mut out);
                                        }
                                    }
                                }
                            }

                            if let Ok(mut vj_file) = archive.by_name("version.json") {
                                let mut s = String::new();
                                if vj_file.read_to_string(&mut s).is_ok() {
                                    Some(s)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(vj_str) = raw_json {
            if let Ok(forge_vj) = serde_json::from_str::<VersionPackage>(&vj_str) {
                base_pkg.inherits_from = Some(base_pkg.id.clone());
                // 1. Main Class güncelle
                base_pkg.main_class = forge_vj.main_class;

                // 2. Kütüphaneleri ekle
                for lib in forge_vj.libraries {
                    if !base_pkg.libraries.iter().any(|l| l.name == lib.name) {
                        base_pkg.libraries.push(lib);
                    }
                }

                // 3. JVM ve Game argümanlarını birleştir
                if let Some(forge_args) = forge_vj.arguments {
                    if let Some(ref mut base_args) = base_pkg.arguments {
                        for jvm in forge_args.jvm {
                            base_args.jvm.push(jvm);
                        }
                        for game in forge_args.game {
                            base_args.game.push(game);
                        }
                    } else {
                        base_pkg.arguments = Some(forge_args);
                    }
                }

                return Ok(());
            }
        }

        // Fallback: Installer indirilemediyse temel ayarları uygula
        if mc_version.starts_with("1.12") || mc_version.starts_with("1.7") {
            base_pkg.main_class = "net.minecraft.launchwrapper.Launch".to_string();
        } else {
            base_pkg.main_class = "cpw.mods.bootstraplauncher.BootstrapLauncher".to_string();
        }

        let forge_lib = Library {
            name: format!("net.minecraftforge:forge:{}", forge_artifact),
            downloads: Some(LibraryDownloads {
                artifact: Some(DownloadArtifact {
                    path: Some(format!(
                        "net/minecraftforge/forge/{}/forge-{}.jar",
                        forge_artifact, forge_artifact
                    )),
                    sha1: String::new(),
                    size: 0,
                    url: format!(
                        "https://maven.minecraftforge.net/net/minecraftforge/forge/{}/forge-{}.jar",
                        forge_artifact, forge_artifact
                    ),
                }),
                classifiers: None,
            }),
            rules: None,
            natives: None,
        };
        base_pkg.libraries.push(forge_lib);

        Ok(())
    }
}

pub struct NeoForgeAdapter {
    _client: reqwest::Client,
}

impl Default for NeoForgeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl NeoForgeAdapter {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("EnesLauncher/0.1.0")
            .build()
            .unwrap_or_default();
        Self { _client: client }
    }

    /// NeoForge sürüm metaverisini temel Vanilla VersionPackage üzerine enjekte eder
    pub async fn patch_version_package(
        &self,
        base_pkg: &mut VersionPackage,
        loader_version: &str,
    ) -> LauncherResult<()> {
        base_pkg.main_class = "cpw.mods.bootstraplauncher.BootstrapLauncher".to_string();

        let neoforge_lib = Library {
            name: format!("net.neoforged:neoforge:{}", loader_version),
            downloads: Some(LibraryDownloads {
                artifact: Some(DownloadArtifact {
                    path: Some(format!(
                        "net/neoforged/neoforge/{}/neoforge-{}.jar",
                        loader_version, loader_version
                    )),
                    sha1: String::new(),
                    size: 0,
                    url: format!(
                        "https://maven.neoforged.net/releases/net/neoforged/neoforge/{}/neoforge-{}.jar",
                        loader_version, loader_version
                    ),
                }),
                classifiers: None,
            }),
            rules: None,
            natives: None,
        };
        base_pkg.libraries.push(neoforge_lib);

        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct FabricProfileResponse {
    #[serde(rename = "launcherMeta")]
    launcher_meta: FabricLauncherMeta,
}

#[derive(Debug, Deserialize, Serialize)]
struct FabricLauncherMeta {
    #[serde(rename = "mainClass")]
    main_class: FabricMainClass,
    libraries: FabricLibraries,
}

#[derive(Debug, Deserialize, Serialize)]
struct FabricMainClass {
    client: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct FabricLibraries {
    client: Option<Vec<FabricLibraryEntry>>,
    common: Option<Vec<FabricLibraryEntry>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct FabricLibraryEntry {
    name: String,
    url: Option<String>,
}

fn convert_fabric_library(entry: FabricLibraryEntry) -> Library {
    let maven_repo = entry
        .url
        .unwrap_or_else(|| "https://maven.fabricmc.net/".to_string());
    let path = maven_to_path(&entry.name);
    let full_url = format!("{}{}", maven_repo.trim_end_matches('/'), path);

    Library {
        name: entry.name,
        downloads: Some(LibraryDownloads {
            artifact: Some(DownloadArtifact {
                path: Some(path.trim_start_matches('/').to_string()),
                sha1: String::new(),
                size: 0,
                url: full_url,
            }),
            classifiers: None,
        }),
        rules: None,
        natives: None,
    }
}

pub fn maven_to_path(maven_id: &str) -> String {
    let parts: Vec<&str> = maven_id.split(':').collect();
    if parts.len() < 3 {
        return maven_id.to_string();
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version = parts[2];
    format!("/{}/{}/{}/{}-{}.jar", group, artifact, version, artifact, version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maven_to_path() {
        let maven = "net.fabricmc:fabric-loader:0.16.5";
        let path = maven_to_path(maven);
        assert_eq!(path, "/net/fabricmc/fabric-loader/0.16.5/fabric-loader-0.16.5.jar");
    }
}


use async_trait::async_trait;
use launcher_core::{LauncherError, LauncherResult};
use mod_provider_api::{
    DependencyType, ModDependency, ModFile, ModProvider, ModSearchResult, ModVersionInfo,
    SearchFilterQuery, SearchPageResult,
};
use serde::{Deserialize, Serialize};

pub const MODRINTH_API_BASE: &str = "https://api.modrinth.com/v2";

pub struct ModrinthProvider {
    client: reqwest::Client,
}

impl Default for ModrinthProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ModrinthProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("EnesLauncher/0.1.0 (https://github.com/eneslauncher)")
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .tcp_nodelay(true)
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_default();

        Self { client }
    }
}

// Modrinth v2 API Yanıt Modelleri
#[derive(Debug, Deserialize)]
struct LabrinthSearchResponse {
    hits: Vec<LabrinthSearchHit>,
    #[allow(dead_code)]
    total_hits: u64,
}

#[derive(Debug, Deserialize)]
struct LabrinthSearchHit {
    project_id: String,
    slug: String,
    title: String,
    description: String,
    author: String,
    icon_url: Option<String>,
    downloads: u64,
    follows: u64,
    categories: Option<Vec<String>>,
    versions: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct LabrinthFile {
    hashes: LabrinthHashes,
    url: String,
    filename: String,
    primary: bool,
    size: u64,
}

#[derive(Debug, Deserialize, Serialize)]
struct LabrinthHashes {
    sha512: Option<String>,
    sha1: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct LabrinthDependency {
    version_id: Option<String>,
    project_id: Option<String>,
    dependency_type: String, // "required", "optional", "incompatible", "embedded"
}

#[derive(Debug, Deserialize, Serialize)]
struct LabrinthVersion {
    id: String,
    project_id: String,
    name: String,
    version_number: String,
    game_versions: Vec<String>,
    loaders: Vec<String>,
    files: Vec<LabrinthFile>,
    dependencies: Vec<LabrinthDependency>,
    date_published: String,
}

#[async_trait]
impl ModProvider for ModrinthProvider {
    fn provider_name(&self) -> &'static str {
        "modrinth"
    }

    async fn search_advanced(
        &self,
        filter: &SearchFilterQuery,
    ) -> LauncherResult<SearchPageResult> {
        let p_type = filter.project_type.as_deref().unwrap_or("mod");
        let mut facets = vec![format!("[\"project_type:{}\"]", p_type)];

        if let Some(ref v) = filter.game_version {
            if !v.is_empty() && v != "all" {
                facets.push(format!("[\"versions:{}\"]", v));
            }
        }

        if let Some(ref l) = filter.loader {
            if !l.is_empty() && l != "all" {
                facets.push(format!("[\"categories:{}\"]", l.to_lowercase()));
            }
        }

        if let Some(ref c) = filter.category {
            if !c.is_empty() && c != "all" {
                facets.push(format!("[\"categories:{}\"]", c.to_lowercase()));
            }
        }

        let index_param = match filter.sort_by.as_deref().unwrap_or("relevance") {
            "downloads" => "downloads",
            "follows" => "follows",
            "newest" => "newest",
            "updated" => "updated",
            _ => "relevance",
        };

        let facets_param = format!("[{}]", facets.join(","));
        let url = format!(
            "{}/search?query={}&facets={}&index={}&offset={}&limit={}",
            MODRINTH_API_BASE,
            urlencoding::encode(&filter.query),
            urlencoding::encode(&facets_param),
            index_param,
            filter.offset,
            filter.limit.clamp(1, 100)
        );

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| LauncherError::Network(format!("Modrinth arama hatası: {}", e)))?;

        if !resp.status().is_success() {
            return Err(LauncherError::Network(format!(
                "Modrinth API hatası: {}",
                resp.status()
            )));
        }

        let data = resp
            .json::<LabrinthSearchResponse>()
            .await
            .map_err(|e| LauncherError::Network(format!("Modrinth JSON ayrıştırma hatası: {}", e)))?;

        let hits = data
            .hits
            .into_iter()
            .map(|hit| ModSearchResult {
                project_id: hit.project_id,
                slug: hit.slug,
                title: hit.title,
                description: hit.description,
                author: hit.author,
                icon_url: hit.icon_url,
                downloads: hit.downloads,
                follows: hit.follows,
                categories: hit.categories.unwrap_or_default(),
                supported_versions: hit.versions.unwrap_or_default(),
                supported_loaders: Vec::new(),
            })
            .collect();

        Ok(SearchPageResult {
            hits,
            total_hits: data.total_hits,
            offset: filter.offset,
            limit: filter.limit,
        })
    }

    async fn search_mods(
        &self,
        query: &str,
        game_version: Option<&str>,
        loader: Option<&str>,
        limit: usize,
    ) -> LauncherResult<Vec<ModSearchResult>> {
        let mut facets = vec!["[\"project_type:mod\"]".to_string()];

        if let Some(v) = game_version {
            facets.push(format!("[\"versions:{}\"]", v));
        }

        if let Some(l) = loader {
            facets.push(format!("[\"categories:{}\"]", l.to_lowercase()));
        }

        let facets_param = format!("[{}]", facets.join(","));
        let url = format!(
            "{}/search?query={}&facets={}&limit={}",
            MODRINTH_API_BASE,
            urlencoding::encode(query),
            urlencoding::encode(&facets_param),
            limit
        );

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| LauncherError::Network(format!("Modrinth arama hatası: {}", e)))?;

        if !resp.status().is_success() {
            return Err(LauncherError::Network(format!(
                "Modrinth API hatası: {}",
                resp.status()
            )));
        }

        let data = resp
            .json::<LabrinthSearchResponse>()
            .await
            .map_err(|e| LauncherError::Network(format!("Modrinth JSON ayrıştırma hatası: {}", e)))?;

        let results = data
            .hits
            .into_iter()
            .map(|hit| ModSearchResult {
                project_id: hit.project_id,
                slug: hit.slug,
                title: hit.title,
                description: hit.description,
                author: hit.author,
                icon_url: hit.icon_url,
                downloads: hit.downloads,
                follows: hit.follows,
                categories: hit.categories.unwrap_or_default(),
                supported_versions: hit.versions.unwrap_or_default(),
                supported_loaders: Vec::new(),
            })
            .collect();

        Ok(results)
    }

    async fn search_modpacks(
        &self,
        query: &str,
        game_version: Option<&str>,
        loader: Option<&str>,
        limit: usize,
    ) -> LauncherResult<Vec<ModSearchResult>> {
        let mut facets = vec!["[\"project_type:modpack\"]".to_string()];

        if let Some(v) = game_version {
            facets.push(format!("[\"versions:{}\"]", v));
        }

        if let Some(l) = loader {
            facets.push(format!("[\"categories:{}\"]", l.to_lowercase()));
        }

        let facets_param = format!("[{}]", facets.join(","));
        let url = format!(
            "{}/search?query={}&facets={}&limit={}",
            MODRINTH_API_BASE,
            urlencoding::encode(query),
            urlencoding::encode(&facets_param),
            limit
        );

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| LauncherError::Network(format!("Modrinth modpack arama hatası: {}", e)))?;

        if !resp.status().is_success() {
            return Err(LauncherError::Network(format!(
                "Modrinth API hatası: {}",
                resp.status()
            )));
        }

        let data = resp
            .json::<LabrinthSearchResponse>()
            .await
            .map_err(|e| LauncherError::Network(format!("Modrinth JSON ayrıştırma hatası: {}", e)))?;

        let results = data
            .hits
            .into_iter()
            .map(|hit| ModSearchResult {
                project_id: hit.project_id,
                slug: hit.slug,
                title: hit.title,
                description: hit.description,
                author: hit.author,
                icon_url: hit.icon_url,
                downloads: hit.downloads,
                follows: hit.follows,
                categories: hit.categories.unwrap_or_default(),
                supported_versions: hit.versions.unwrap_or_default(),
                supported_loaders: Vec::new(),
            })
            .collect();

        Ok(results)
    }

    async fn get_project_versions(
        &self,
        project_id: &str,
        game_version: Option<&str>,
        loader: Option<&str>,
    ) -> LauncherResult<Vec<ModVersionInfo>> {
        let mut query_params = Vec::new();

        if let Some(v) = game_version {
            query_params.push(format!("game_versions=[\"{}\"]", urlencoding::encode(v)));
        }

        if let Some(l) = loader {
            query_params.push(format!("loaders=[\"{}\"]", urlencoding::encode(&l.to_lowercase())));
        }

        let query_str = if query_params.is_empty() {
            String::new()
        } else {
            format!("?{}", query_params.join("&"))
        };

        let url = format!("{}/project/{}/version{}", MODRINTH_API_BASE, project_id, query_str);

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| LauncherError::Network(format!("Modrinth sürüm listesi hatası: {}", e)))?;

        if !resp.status().is_success() {
            return Err(LauncherError::Network(format!(
                "Modrinth sürümleri alınamadı: {}",
                resp.status()
            )));
        }

        let versions = resp
            .json::<Vec<LabrinthVersion>>()
            .await
            .map_err(|e| LauncherError::Network(format!("Sürüm JSON ayrıştırma hatası: {}", e)))?;

        Ok(versions.into_iter().map(convert_labrinth_version).collect())
    }

    async fn get_version_details(&self, version_id: &str) -> LauncherResult<ModVersionInfo> {
        let url = format!("{}/version/{}", MODRINTH_API_BASE, version_id);

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| LauncherError::Network(format!("Modrinth sürüm detay hatası: {}", e)))?;

        let ver = resp
            .json::<LabrinthVersion>()
            .await
            .map_err(|e| LauncherError::Network(format!("Sürüm detayı JSON hatası: {}", e)))?;

        Ok(convert_labrinth_version(ver))
    }
}

fn convert_labrinth_version(ver: LabrinthVersion) -> ModVersionInfo {
    ModVersionInfo {
        id: ver.id,
        project_id: ver.project_id,
        name: ver.name,
        version_number: ver.version_number,
        game_versions: ver.game_versions,
        loaders: ver.loaders,
        files: ver
            .files
            .into_iter()
            .map(|f| ModFile {
                filename: f.filename,
                url: f.url,
                size: f.size,
                sha512: f.hashes.sha512,
                sha1: f.hashes.sha1,
                primary: f.primary,
            })
            .collect(),
        dependencies: ver
            .dependencies
            .into_iter()
            .map(|d| ModDependency {
                project_id: d.project_id,
                version_id: d.version_id,
                dependency_type: match d.dependency_type.as_str() {
                    "required" => DependencyType::Required,
                    "optional" => DependencyType::Optional,
                    "incompatible" => DependencyType::Incompatible,
                    _ => DependencyType::Embedded,
                },
            })
            .collect(),
        date_published: ver.date_published,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_labrinth_version() {
        let raw = LabrinthVersion {
            id: "ver-123".to_string(),
            project_id: "sodium".to_string(),
            name: "Sodium 0.5.8".to_string(),
            version_number: "0.5.8".to_string(),
            game_versions: vec!["1.20.4".to_string()],
            loaders: vec!["fabric".to_string()],
            files: vec![LabrinthFile {
                hashes: LabrinthHashes {
                    sha512: Some("abcd512".to_string()),
                    sha1: Some("abcd1".to_string()),
                },
                url: "https://cdn.modrinth.com/sodium.jar".to_string(),
                filename: "sodium-fabric-0.5.8.jar".to_string(),
                primary: true,
                size: 1024500,
            }],
            dependencies: vec![LabrinthDependency {
                version_id: None,
                project_id: Some("fabric-api".to_string()),
                dependency_type: "required".to_string(),
            }],
            date_published: "2024-01-01T00:00:00Z".to_string(),
        };

        let converted = convert_labrinth_version(raw);
        assert_eq!(converted.id, "ver-123");
        assert_eq!(converted.dependencies.len(), 1);
        assert_eq!(converted.dependencies[0].dependency_type, DependencyType::Required);
        assert_eq!(converted.files[0].sha512.as_deref(), Some("abcd512"));
    }
}


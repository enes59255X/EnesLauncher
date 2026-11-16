use async_trait::async_trait;
use launcher_core::LauncherResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DependencyType {
    Required,
    Optional,
    Incompatible,
    Embedded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModDependency {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub dependency_type: DependencyType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModFile {
    pub filename: String,
    pub url: String,
    pub size: u64,
    pub sha512: Option<String>,
    pub sha1: Option<String>,
    pub primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModVersionInfo {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub files: Vec<ModFile>,
    pub dependencies: Vec<ModDependency>,
    pub date_published: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModSearchResult {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub follows: u64,
    pub categories: Vec<String>,
    pub supported_versions: Vec<String>,
    pub supported_loaders: Vec<String>,
    /// Proje galerisi (kapak görselleri). Sağlayıcı desteklemiyorsa boş.
    #[serde(default)]
    pub gallery: Vec<String>,
    /// Sağlayıcının önerdiği vurgu rengi (0xRRGGBB).
    #[serde(default)]
    pub color: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchFilterQuery {
    pub query: String,
    pub game_version: Option<String>,
    pub loader: Option<String>,
    pub category: Option<String>,
    pub sort_by: Option<String>, // "relevance" | "downloads" | "follows" | "newest" | "updated"
    pub offset: usize,
    pub limit: usize,
    pub project_type: Option<String>, // "mod" | "modpack"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchPageResult {
    pub hits: Vec<ModSearchResult>,
    pub total_hits: u64,
    pub offset: usize,
    pub limit: usize,
}

#[async_trait]
pub trait ModProvider: Send + Sync {
    fn provider_name(&self) -> &'static str;

    async fn search_advanced(
        &self,
        filter: &SearchFilterQuery,
    ) -> LauncherResult<SearchPageResult>;

    async fn search_mods(
        &self,
        query: &str,
        game_version: Option<&str>,
        loader: Option<&str>,
        limit: usize,
    ) -> LauncherResult<Vec<ModSearchResult>>;

    async fn search_modpacks(
        &self,
        query: &str,
        game_version: Option<&str>,
        loader: Option<&str>,
        limit: usize,
    ) -> LauncherResult<Vec<ModSearchResult>>;

    async fn get_project_versions(
        &self,
        project_id: &str,
        game_version: Option<&str>,
        loader: Option<&str>,
    ) -> LauncherResult<Vec<ModVersionInfo>>;

    async fn get_version_details(&self, version_id: &str) -> LauncherResult<ModVersionInfo>;
}


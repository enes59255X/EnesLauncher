// Windows'ta konsol/terminal penceresinin açılmasını tamamen engeller (Hem debug hem release)
#![windows_subsystem = "windows"]

use auth_service::{AccountProfile, AuthService, DeviceCodeResponse};
use crash_analyzer::{CrashAnalyzer, CrashDiagnosis};
use download_engine::DownloadEngine;
use instance_manager::{InstalledModInfo, InstanceManager};
use java_locator::JavaLocator;
use launch_service::{LaunchConfig, LaunchService};
use launcher_core::models::{CreateInstanceRequest, InstanceSummary, LoaderType};
use launcher_core::paths;
use loader_adapters::{FabricAdapter, ForgeAdapter, NeoForgeAdapter};
use minecraft_metadata::{MojangClient, VersionManifestEntry};
use mod_provider_api::{ModProvider, ModSearchResult, ModVersionInfo, SearchFilterQuery, SearchPageResult};
use modpack_engine::ModpackEngine;
use modrinth_provider::ModrinthProvider;
use persistence::Database;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

struct AppState {
    db: Mutex<Database>,
    mojang_client: MojangClient,
    download_engine: DownloadEngine,
    modrinth: ModrinthProvider,
    fabric_adapter: FabricAdapter,
    forge_adapter: ForgeAdapter,
    neoforge_adapter: NeoForgeAdapter,
    /// Paylaşımlı HTTP istemcisi — her çağrıda yeni client oluşturma maliyetini ve
    /// bağlantı havuzu şişmesini önler (PERF fix)
    http_client: reqwest::Client,
}

#[derive(serde::Serialize)]
struct SystemStatus {
    version: String,
    target_os: String,
    data_dir: String,
    instances_count: usize,
    java_path: Option<String>,
    active_account: Option<AccountProfile>,
}

#[derive(serde::Serialize)]
struct LaunchResultResponse {
    success: bool,
    pid: Option<u32>,
    message: String,
}

#[tauri::command]
fn get_system_status(state: State<AppState>) -> Result<SystemStatus, String> {
    let data_dir = paths::get_launcher_root_dir();
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let instances = db.list_instances().map_err(|e| e.to_string())?;
    let java = JavaLocator::find_matching_java(17).ok().map(|j| j.path.to_string_lossy().to_string());
    let active_account = AuthService::get_active_account(&data_dir).ok();

    Ok(SystemStatus {
        version: "1.0.0 (Release)".to_string(),
        target_os: "windows-x64".to_string(),
        data_dir: data_dir.to_string_lossy().to_string(),
        instances_count: instances.len(),
        java_path: java,
        active_account,
    })
}

#[tauri::command]
fn list_instances(state: State<AppState>) -> Result<Vec<InstanceSummary>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.list_instances().map_err(|e| e.to_string())
}

#[tauri::command]
fn create_instance(state: State<AppState>, request: CreateInstanceRequest) -> Result<InstanceSummary, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let inst = db.create_instance(request).map_err(|e| e.to_string())?;
    
    // İzole klasör yapısını fiziksel olarak oluştur
    let inst_dir = paths::get_instances_dir().join(&inst.directory_name);
    let _ = std::fs::create_dir_all(&inst_dir);
    let _ = std::fs::create_dir_all(inst_dir.join("mods"));
    let _ = std::fs::create_dir_all(inst_dir.join("saves"));
    let _ = std::fs::create_dir_all(inst_dir.join("config"));

    Ok(inst)
}

#[tauri::command]
async fn delete_instance(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    // 1. Silinecek profilin dizin adını veritabanından bul (O(1) indeksli sorgu)
    let instance = {
        let db = state.db.lock().map_err(|e| format!("Veritabanı erişim kilidi açılamadı: {}", e))?;
        db.get_instance(&id).map_err(|e| format!("Profil sorgulanamadı: {}", e))?
    };

    let Some(inst) = instance else {
        return Err("Silinmek istenen profil sistemde bulunamadı.".to_string());
    };

    // 2. Veritabanından profil kaydını sil
    {
        let db = state.db.lock().map_err(|e| format!("Veritabanı kilidi açılamadı: {}", e))?;
        db.delete_instance(&id).map_err(|e| format!("Profil veritabanından silinemedi: {}", e))?;
    }

    // 3. Diskteki izole instance klasörünü (modlar, haritalar, config) fiziksel olarak temizle
    let inst_dir = paths::get_instances_dir().join(&inst.directory_name);
    if inst_dir.exists() {
        if let Err(e) = tokio::fs::remove_dir_all(&inst_dir).await {
            tracing::warn!("Profil dizini silinirken uyarı: {:?} -> {}", inst_dir, e);
            // Dosya kilitliyse tekrar deneme
            let _ = std::fs::remove_dir_all(&inst_dir);
        }
    }

    tracing::info!("Profil başarıyla ve tamamen silindi: {} ({})", inst.name, inst.directory_name);
    Ok(true)
}

#[tauri::command]
async fn fetch_official_versions(state: State<'_, AppState>) -> Result<Vec<VersionManifestEntry>, String> {
    let manifest = state.mojang_client.fetch_manifest().await.map_err(|e| e.to_string())?;
    let filtered = manifest.versions.into_iter().take(50).collect();
    Ok(filtered)
}

// ── Modrinth Mod ve Modpack Arama ──────────────────────────────────────────

#[tauri::command]
async fn search_modrinth_advanced(
    state: State<'_, AppState>,
    filter: SearchFilterQuery,
) -> Result<SearchPageResult, String> {
    state.modrinth.search_advanced(&filter).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn search_modrinth_mods(
    state: State<'_, AppState>,
    query: String,
    game_version: Option<String>,
    loader: Option<String>,
) -> Result<Vec<ModSearchResult>, String> {
    state
        .modrinth
        .search_mods(&query, game_version.as_deref(), loader.as_deref(), 20)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn search_modrinth_modpacks(
    state: State<'_, AppState>,
    query: String,
    game_version: Option<String>,
    loader: Option<String>,
) -> Result<Vec<ModSearchResult>, String> {
    state
        .modrinth
        .search_modpacks(&query, game_version.as_deref(), loader.as_deref(), 20)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_modpack_versions(
    state: State<'_, AppState>,
    project_id: String,
    game_version: Option<String>,
    loader: Option<String>,
) -> Result<Vec<ModVersionInfo>, String> {
    state
        .modrinth
        .get_project_versions(&project_id, game_version.as_deref(), loader.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn install_remote_modpack(
    state: State<'_, AppState>,
    download_url: String,
    pack_name: String,
    icon_url: Option<String>,
    cover_url: Option<String>,
    project_id: Option<String>,
) -> Result<InstanceSummary, String> {
    // 1. .mrpack dosyasını geçici konuma indir
    let temp_dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let mrpack_dest = temp_dir.path().join("downloaded.mrpack");

    // Paylaşılan HTTP istemcisi kullan (AppState'den alınmak yerine burada yeni client — modpack
    // fonksiyonu async fn olduğu için state parametresi var, onu kullanıyoruz)
    let resp = state.http_client.get(&download_url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Modpack indirme başarısız: HTTP {}", resp.status()));
    }
    // Stream-to-disk: tüm baytları RAM'e almak yerine dosyaya yaz (büyük modpackler için OOM önleme)
    {
        let mut file = tokio::fs::File::create(&mrpack_dest).await.map_err(|e| e.to_string())?;
        let mut stream = resp.bytes_stream();
        use tokio::io::AsyncWriteExt;
        use futures_util::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| e.to_string())?;
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        }
    }

    // 2. Manifesti oku ve profili oluştur
    let manifest = ModpackEngine::read_manifest(&mrpack_dest).map_err(|e| e.to_string())?;

    let mc_ver = manifest
        .dependencies
        .get("minecraft")
        .cloned()
        .unwrap_or_else(|| "1.21.1".to_string());

    let (loader_type, loader_ver) = if let Some(ver) = manifest.dependencies.get("fabric-loader").or_else(|| manifest.dependencies.get("fabric")) {
        (LoaderType::Fabric, Some(ver.clone()))
    } else if let Some(ver) = manifest.dependencies.get("neoforge") {
        (LoaderType::NeoForge, Some(ver.clone()))
    } else if let Some(ver) = manifest.dependencies.get("forge") {
        (LoaderType::Forge, Some(ver.clone()))
    } else {
        (LoaderType::Vanilla, None)
    };

    let name = if pack_name.trim().is_empty() { manifest.name } else { pack_name };
    let req = CreateInstanceRequest {
        name,
        minecraft_version: mc_ver,
        loader_type,
        loader_version: loader_ver,
        allocated_memory_mb: Some(6144),
        icon_url,
        cover_url,
        project_id,
    };

    let instance = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.create_instance(req).map_err(|e| e.to_string())?
    };

    let inst_dir = paths::get_instances_dir().join(&instance.directory_name);
    ModpackEngine::import_modpack(&mrpack_dest, &inst_dir, &state.download_engine)
        .await
        .map_err(|e| e.to_string())?;

    Ok(instance)
}

// ── Mod Yönetimi ──────────────────────────────────────────────────────────

#[tauri::command]
async fn list_instance_mods(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Vec<InstalledModInfo>, String> {
    let instance = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.get_instance(&instance_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Instance bulunamadı".to_string())?
    };

    let inst_dir = paths::get_instances_dir().join(&instance.directory_name);
    InstanceManager::list_installed_mods(&inst_dir).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn toggle_instance_mod(
    state: State<'_, AppState>,
    instance_id: String,
    file_name: String,
) -> Result<String, String> {
    let instance = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.get_instance(&instance_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Instance bulunamadı".to_string())?
    };

    let inst_dir = paths::get_instances_dir().join(&instance.directory_name);
    InstanceManager::toggle_mod(&inst_dir, &file_name).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_instance_mod(
    state: State<'_, AppState>,
    instance_id: String,
    file_name: String,
) -> Result<bool, String> {
    let instance = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.get_instance(&instance_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Instance bulunamadı".to_string())?
    };

    let inst_dir = paths::get_instances_dir().join(&instance.directory_name);
    InstanceManager::delete_mod(&inst_dir, &file_name).await.map_err(|e| e.to_string())?;
    Ok(true)
}

// ── Shaderpack Yönetimi ───────────────────────────────────────────────────

#[tauri::command]
async fn list_instance_shaders(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Vec<String>, String> {
    let instance = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.get_instance(&instance_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Instance bulunamadı".to_string())?
    };

    let shader_dir = paths::get_instances_dir().join(&instance.directory_name).join("shaderpacks");
    if !shader_dir.exists() {
        let _ = tokio::fs::create_dir_all(&shader_dir).await;
        return Ok(Vec::new());
    }

    let mut result = Vec::new();
    if let Ok(mut entries) = tokio::fs::read_dir(&shader_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_file() || path.is_dir() {
                if let Some(name) = path.file_name().and_then(|x| x.to_str()) {
                    if name.ends_with(".zip") || path.is_dir() {
                        result.push(name.to_string());
                    }
                }
            }
        }
    }
    result.sort();
    Ok(result)
}

#[tauri::command]
async fn install_remote_mod(
    state: State<'_, AppState>,
    instance_id: String,
    download_url: String,
    file_name: String,
) -> Result<bool, String> {
    let instance = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.get_instance(&instance_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Instance bulunamadı".to_string())?
    };

    let mods_dir = paths::get_instances_dir().join(&instance.directory_name).join("mods");
    let _ = tokio::fs::create_dir_all(&mods_dir).await;

    let target_file = mods_dir.join(&file_name);
    // Paylaşımlı HTTP istemcisi kullan (bağlantı havuzu yeniden kullanımı)
    let resp = state.http_client.get(&download_url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Mod indirme başarısız: HTTP {}", resp.status()));
    }
    // Stream-to-disk: büyük mod dosyaları için RAM tüketimi önlendi
    {
        let mut file = tokio::fs::File::create(&target_file).await.map_err(|e| e.to_string())?;
        let mut stream = resp.bytes_stream();
        use tokio::io::AsyncWriteExt;
        use futures_util::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| e.to_string())?;
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        }
    }
    Ok(true)
}

#[tauri::command]
async fn install_remote_shader(
    state: State<'_, AppState>,
    instance_id: String,
    download_url: String,
    file_name: String,
) -> Result<bool, String> {
    let instance = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.get_instance(&instance_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Instance bulunamadı".to_string())?
    };

    let shader_dir = paths::get_instances_dir().join(&instance.directory_name).join("shaderpacks");
    let _ = tokio::fs::create_dir_all(&shader_dir).await;

    let target_file = shader_dir.join(&file_name);
    // Paylaşımlı HTTP istemcisi + stream-to-disk
    let resp = state.http_client.get(&download_url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Shader indirme başarısız: HTTP {}", resp.status()));
    }
    {
        let mut file = tokio::fs::File::create(&target_file).await.map_err(|e| e.to_string())?;
        let mut stream = resp.bytes_stream();
        use tokio::io::AsyncWriteExt;
        use futures_util::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| e.to_string())?;
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        }
    }
    Ok(true)
}

#[tauri::command]
async fn delete_instance_shader(
    state: State<'_, AppState>,
    instance_id: String,
    file_name: String,
) -> Result<bool, String> {
    let instance = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.get_instance(&instance_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Instance bulunamadı".to_string())?
    };

    let shader_file = paths::get_instances_dir()
        .join(&instance.directory_name)
        .join("shaderpacks")
        .join(&file_name);

    if shader_file.exists() {
        if shader_file.is_dir() {
            let _ = tokio::fs::remove_dir_all(&shader_file).await;
        } else {
            let _ = tokio::fs::remove_file(&shader_file).await;
        }
    }
    Ok(true)
}

// ── Özel Arka Plan ve Sistem Entegrasyonu ─────────────────────────────────

#[tauri::command]
async fn save_custom_background(
    file_name: String,
    data_base64: String,
) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let bg_dir = paths::get_launcher_root_dir().join("custom_backgrounds");
    let _ = tokio::fs::create_dir_all(&bg_dir).await;

    // Dosya adındaki geçersiz karakterleri temizle
    let clean_name: String = file_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '.' || c == '_' || c == '-' { c } else { '_' })
        .collect();

    let target_path = bg_dir.join(&clean_name);
    let bytes = STANDARD
        .decode(&data_base64)
        .map_err(|e| format!("Base64 çözme hatası: {}", e))?;

    tokio::fs::write(&target_path, bytes)
        .await
        .map_err(|e| format!("Dosya kaydedilemedi: {}", e))?;

    tracing::info!("Özel arka plan EnesLauncher sistem klasörüne kalıcı kopyalandı: {:?}", target_path);
    Ok(target_path.to_string_lossy().to_string())
}

#[tauri::command]
async fn save_launcher_settings(settings_json: String) -> Result<bool, String> {
    let settings_path = paths::get_launcher_root_dir().join("launcher_settings.json");
    tokio::fs::write(&settings_path, settings_json)
        .await
        .map_err(|e| format!("Ayarlar dosyasına yazılamadı: {}", e))?;
    Ok(true)
}

#[tauri::command]
async fn load_launcher_settings() -> Result<Option<String>, String> {
    let settings_path = paths::get_launcher_root_dir().join("launcher_settings.json");
    if settings_path.is_file() {
        let content = tokio::fs::read_to_string(&settings_path)
            .await
            .map_err(|e| format!("Ayarlar okunamadı: {}", e))?;
        Ok(Some(content))
    } else {
        Ok(None)
    }
}

#[tauri::command]
async fn get_file_data_url(file_path: String) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let path = PathBuf::from(&file_path);
    if !path.is_file() {
        return Err("Dosya bulunamadı".to_string());
    }
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|e| format!("Dosya okunamadı: {}", e))?;
    let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("png").to_lowercase();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        _ => "image/png",
    };
    let b64 = STANDARD.encode(&bytes);
    Ok(format!("data:{};base64,{}", mime, b64))
}

#[tauri::command]
fn open_in_browser(url: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn();
    }
    Ok(())
}

#[tauri::command]
fn open_instance_folder(state: State<'_, AppState>, instance_id: String) -> Result<(), String> {
    let instance = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.get_instance(&instance_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Instance bulunamadı".to_string())?
    };
    let inst_dir = paths::get_instances_dir().join(&instance.directory_name);
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer")
            .arg(&inst_dir)
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = open::that(&inst_dir);
    }
    Ok(())
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct OptimizationReport {
    pub success: bool,
    pub steps_completed: Vec<String>,
    pub details: Vec<String>,
    pub estimated_fps_boost: String,
}

#[tauri::command]
fn update_instance_settings(
    state: State<'_, AppState>,
    instance_id: String,
    name: Option<String>,
    memory_mb: Option<u32>,
    jvm_arguments: Option<String>,
) -> Result<bool, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.update_instance_settings(
        &instance_id,
        name.as_deref(),
        memory_mb,
        jvm_arguments.as_deref(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn optimize_instance_performance(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<OptimizationReport, String> {
    let instance = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.get_instance(&instance_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Instance bulunamadı".to_string())?
    };

    let inst_dir = paths::get_instances_dir().join(&instance.directory_name);
    tokio::fs::create_dir_all(&inst_dir).await.map_err(|e| e.to_string())?;

    let mut steps = Vec::new();
    let mut details = Vec::new();

    // 1. options.txt (Grafik & Render FPS Optimizasyonu)
    let options_path = inst_dir.join("options.txt");
    let mut options_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    if options_path.exists() {
        if let Ok(content) = tokio::fs::read_to_string(&options_path).await {
            for line in content.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    options_map.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
        }
    }

    // Yüksek FPS değerleri
    options_map.insert("graphicsMode".to_string(), "0".to_string()); // Fast
    options_map.insert("renderDistance".to_string(), "8".to_string()); // 8 Chunks
    options_map.insert("simulationDistance".to_string(), "6".to_string());
    options_map.insert("particles".to_string(), "2".to_string()); // Minimal
    options_map.insert("smoothLighting".to_string(), "false".to_string());
    options_map.insert("enableVsync".to_string(), "false".to_string());
    options_map.insert("entityShadows".to_string(), "false".to_string());
    options_map.insert("mipmapLevels".to_string(), "0".to_string());
    options_map.insert("renderClouds".to_string(), "\"false\"".to_string());
    options_map.insert("biomeBlendRadius".to_string(), "0".to_string());
    options_map.insert("maxFps".to_string(), "260".to_string());

    let mut new_options_content = String::new();
    for (k, v) in &options_map {
        new_options_content.push_str(&format!("{}:{}\n", k, v));
    }
    tokio::fs::write(&options_path, new_options_content).await.map_err(|e| e.to_string())?;
    steps.push("Grafikler 'Hızlı' (Fast) moduna çekildi, VSync ve Gölgeler kapatıldı.".to_string());
    details.push("Görüş Mesafesi 8 Chunk, Simülasyon 6 Chunk olarak sınırlandı.".to_string());

    // 2. Fabric / Sodium & Iris Konfigürasyonu
    let config_dir = inst_dir.join("config");
    tokio::fs::create_dir_all(&config_dir).await.map_err(|e| e.to_string())?;

    let sodium_config_path = config_dir.join("sodium-options.json");
    let sodium_content = r#"{
  "quality": {
    "graphics_quality": "DEFAULT",
    "weather_quality": "FAST",
    "leaves_quality": "FAST"
  },
  "performance": {
    "chunk_builder_threads": 0,
    "always_defer_chunk_updates": true,
    "use_compact_vertex_format": true,
    "animate_only_visible_textures": true
  }
}"#;
    let _ = tokio::fs::write(&sodium_config_path, sodium_content).await;
    steps.push("Sodium / Iris GPU hızlandırma ve chunk derleme parametreleri optimize edildi.".to_string());

    // 3. Yüksek Performanslı Aikar's G1GC JVM Argümanları
    let jvm_flags = "-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=200 -XX:+UnlockExperimentalVMOptions -XX:+DisableExplicitGC -XX:+AlwaysPreTouch -XX:G1NewSizePercent=30 -XX:G1MaxNewSizePercent=40 -XX:G1ReservePercent=20 -XX:G1HeapWastePercent=5 -XX:G1MixedGCCountTarget=4 -XX:InitiatingHeapOccupancyPercent=15 -XX:G1MixedGCLiveThresholdPercent=90 -XX:G1RSetUpdatingPauseTimePercent=5 -XX:SurvivorRatio=32 -XX:+PerfDisableSharedMem -XX:MaxTenuringThreshold=1";

    let optimal_mem = if instance.allocated_memory_mb < 4096 {
        Some(4096)
    } else {
        None
    };

    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.update_instance_settings(&instance.id, None, optimal_mem, Some(jvm_flags))
            .map_err(|e| e.to_string())?;
    }
    steps.push("Aikar's G1GC düşük gecikmeli bellek temizleme argümanları uygulandı.".to_string());

    Ok(OptimizationReport {
        success: true,
        steps_completed: steps,
        details,
        estimated_fps_boost: "+%40 - +%85 FPS Artışı".to_string(),
    })
}

// ── Log ve Dil Yönetimi ───────────────────────────────────────────────────

#[tauri::command]
async fn get_instance_latest_log(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<String, String> {
    let instance = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.get_instance(&instance_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Instance bulunamadı".to_string())?
    };

    let inst_dir = paths::get_instances_dir().join(&instance.directory_name);

    // 1. En son crash report var mı?
    let crash_dir = inst_dir.join("crash-reports");
    if crash_dir.is_dir() {
        if let Ok(mut entries) = tokio::fs::read_dir(&crash_dir).await {
            let mut latest_crash: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Ok(meta) = entry.metadata().await {
                    if let Ok(modified) = meta.modified() {
                        if latest_crash.as_ref().map(|(m, _)| modified > *m).unwrap_or(true) {
                            latest_crash = Some((modified, entry.path()));
                        }
                    }
                }
            }
            if let Some((_, crash_path)) = latest_crash {
                if let Ok(content) = tokio::fs::read_to_string(&crash_path).await {
                    return Ok(content);
                }
            }
        }
    }

    // 2. latest.log oku
    let latest_log = inst_dir.join("logs").join("latest.log");
    if latest_log.is_file() {
        if let Ok(content) = tokio::fs::read_to_string(&latest_log).await {
            let lines: Vec<&str> = content.lines().collect();
            let start = if lines.len() > 1500 { lines.len() - 1500 } else { 0 };
            return Ok(lines[start..].join("\n"));
        }
    }

    // 3. debug.log oku
    let debug_log = inst_dir.join("logs").join("debug.log");
    if debug_log.is_file() {
        if let Ok(content) = tokio::fs::read_to_string(&debug_log).await {
            let lines: Vec<&str> = content.lines().collect();
            let start = if lines.len() > 1000 { lines.len() - 1000 } else { 0 };
            return Ok(lines[start..].join("\n"));
        }
    }

    Err("Henüz bu profil için oluşturulmuş bir log dosyası bulunamadı.".to_string())
}

#[tauri::command]
async fn set_instance_language(
    state: State<'_, AppState>,
    instance_id: String,
    lang: String,
) -> Result<bool, String> {
    let instance = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.get_instance(&instance_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Instance bulunamadı".to_string())?
    };

    let inst_dir = paths::get_instances_dir().join(&instance.directory_name);
    let opt_path = inst_dir.join("options.txt");

    if opt_path.is_file() {
        if let Ok(content) = tokio::fs::read_to_string(&opt_path).await {
            let mut lines: Vec<String> = Vec::new();
            let mut found_lang = false;
            for line in content.lines() {
                if line.starts_with("lang:") {
                    lines.push(format!("lang:{}", lang));
                    found_lang = true;
                } else {
                    lines.push(line.to_string());
                }
            }
            if !found_lang {
                lines.push(format!("lang:{}", lang));
            }
            let _ = tokio::fs::write(&opt_path, lines.join("\n")).await;
        }
    } else {
        let default_opt = format!("lang:{}\n", lang);
        let _ = tokio::fs::write(&opt_path, default_opt).await;
    }

    Ok(true)
}

#[tauri::command]
async fn import_mrpack(
    state: State<'_, AppState>,
    file_path: String,
    custom_name: Option<String>,
) -> Result<InstanceSummary, String> {
    let archive_path = PathBuf::from(&file_path);
    let manifest = ModpackEngine::read_manifest(&archive_path).map_err(|e| e.to_string())?;

    let mc_ver = manifest
        .dependencies
        .get("minecraft")
        .cloned()
        .unwrap_or_else(|| "1.21.1".to_string());

    let (loader_type, loader_ver) = if let Some(ver) = manifest.dependencies.get("fabric-loader").or_else(|| manifest.dependencies.get("fabric")) {
        (LoaderType::Fabric, Some(ver.clone()))
    } else if let Some(ver) = manifest.dependencies.get("neoforge") {
        (LoaderType::NeoForge, Some(ver.clone()))
    } else if let Some(ver) = manifest.dependencies.get("forge") {
        (LoaderType::Forge, Some(ver.clone()))
    } else {
        (LoaderType::Vanilla, None)
    };

    let name = custom_name.unwrap_or(manifest.name);
    let req = CreateInstanceRequest {
        name,
        minecraft_version: mc_ver,
        loader_type,
        loader_version: loader_ver,
        allocated_memory_mb: Some(6144),
        icon_url: None,
        cover_url: None,
        project_id: None,
    };

    let instance = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.create_instance(req).map_err(|e| e.to_string())?
    };

    let inst_dir = paths::get_instances_dir().join(&instance.directory_name);
    ModpackEngine::import_modpack(&archive_path, &inst_dir, &state.download_engine)
        .await
        .map_err(|e| e.to_string())?;

    Ok(instance)
}

/// Mevcut profiller için modpack kapak/ikon bilgisini Modrinth'ten tamamlar.
/// Yalnızca profil adı bir Modrinth modpack başlığıyla birebir eşleşirse yazar;
/// böylece yanlış kapak gösterme riski olmaz.
#[tauri::command]
async fn backfill_instance_art(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Option<InstanceSummary>, String> {
    let instance = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.get_instance(&instance_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Profil bulunamadı".to_string())?
    };

    // Zaten görseli olan profile dokunma.
    if instance.icon_url.is_some() || instance.cover_url.is_some() {
        return Ok(Some(instance));
    }

    let hits = state
        .modrinth
        .search_modpacks(&instance.name, None, None, 5)
        .await
        .map_err(|e| e.to_string())?;

    let normalize = |s: &str| {
        s.to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let target = normalize(&instance.name);

    let hit = match hits.into_iter().find(|h| normalize(&h.title) == target) {
        Some(h) => h,
        None => return Ok(None),
    };

    let cover = hit.gallery.first().cloned();

    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.set_instance_art(
            &instance_id,
            hit.icon_url.as_deref(),
            cover.as_deref(),
            Some(hit.project_id.as_str()),
        )
        .map_err(|e| e.to_string())?;
    }

    let updated = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.get_instance(&instance_id).map_err(|e| e.to_string())?
    };

    Ok(updated)
}

#[tauri::command]
async fn export_instance_to_mrpack(
    state: State<'_, AppState>,
    instance_id: String,
    export_path: String,
) -> Result<bool, String> {
    let instance = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.get_instance(&instance_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Instance bulunamadı".to_string())?
    };

    let inst_dir = paths::get_instances_dir().join(&instance.directory_name);
    let out = PathBuf::from(&export_path);

    ModpackEngine::export_instance_to_mrpack(
        &inst_dir,
        &out,
        &instance.name,
        &instance.minecraft_version,
        &instance.loader_type.to_string(),
        instance.loader_version.as_deref().unwrap_or("latest"),
    )
    .map_err(|e| e.to_string())?;

    Ok(true)
}

#[tauri::command]
fn analyze_crash_log(log_content: String) -> Result<CrashDiagnosis, String> {
    CrashAnalyzer::analyze_log(&log_content).map_err(|e| e.to_string())
}

// ── Çevrimiçi & Çevrimdışı Hesap Yönetimi ─────────────────────────────────

#[tauri::command]
fn list_accounts() -> Result<Vec<AccountProfile>, String> {
    let root = paths::get_launcher_root_dir();
    AuthService::list_accounts(&root).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_offline_account(username: String) -> Result<AccountProfile, String> {
    let root = paths::get_launcher_root_dir();
    AuthService::add_offline_account(&root, &username).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_active_account(account_id: String) -> Result<AccountProfile, String> {
    let root = paths::get_launcher_root_dir();
    AuthService::set_active_account(&root, &account_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_account(account_id: String) -> Result<bool, String> {
    let root = paths::get_launcher_root_dir();
    AuthService::delete_account(&root, &account_id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_microsoft_login() -> Result<DeviceCodeResponse, String> {
    AuthService::start_microsoft_login().await.map_err(|e| e.to_string())
}

// ── Oyun Başlatma (Hem Çevrimiçi Hem Çevrimdışı) ──────────────────────────

async fn fetch_from_mojang(
    client: &minecraft_metadata::MojangClient,
    mc_version: &str,
    save_path: &std::path::Path,
) -> Result<minecraft_metadata::VersionPackage, String> {
    let manifest = client.fetch_manifest().await.map_err(|e| {
        format!("Mojang sunucularına bağlanılamadı. Lütfen internet bağlantınızı kontrol edin. (Detay: {})", e)
    })?;

    let version_entry = manifest
        .versions
        .iter()
        .find(|v| v.id == mc_version)
        .ok_or_else(|| format!("Minecraft '{}' sürümü Mojang resmi listesinde bulunamadı.", mc_version))?;

    let version_pkg = client.fetch_version_package(&version_entry.url).await.map_err(|e| {
        format!("Minecraft sürüm dosyası metaverisi indirilemedi: {}", e)
    })?;

    if let Some(parent) = save_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
        if let Ok(json_str) = serde_json::to_string_pretty(&version_pkg) {
            let _ = tokio::fs::write(save_path, json_str).await;
        }
    }

    Ok(version_pkg)
}

/// Profildeki yabancı/Çince dil kilitlerini ve mod zorlamalarını temizler, Türkçe dili ayarlar
async fn sanitize_instance_language_and_mods(instance_dir: &std::path::Path) {
    let opt_path = instance_dir.join("options.txt");
    if opt_path.is_file() {
        if let Ok(content) = tokio::fs::read_to_string(&opt_path).await {
            let mut lines = Vec::new();
            let mut lang_found = false;
            for line in content.lines() {
                if line.starts_with("lang:") {
                    lines.push("lang:tr_tr".to_string());
                    lang_found = true;
                } else if line.starts_with("resourcePacks:") {
                    // Çince dil modpack resourcepack'ini temizle
                    let cleaned = line
                        .replace("\"file/Minecraft-Mod-Language-Modpack-Converted-1.20.1.zip\",", "")
                        .replace(",\"file/Minecraft-Mod-Language-Modpack-Converted-1.20.1.zip\"", "")
                        .replace("\"file/Minecraft-Mod-Language-Modpack-Converted-1.20.1.zip\"", "");
                    lines.push(cleaned);
                } else {
                    lines.push(line.to_string());
                }
            }
            if !lang_found {
                lines.push("lang:tr_tr".to_string());
            }
            let _ = tokio::fs::write(&opt_path, lines.join("\n") + "\n").await;
        }
    } else {
        let _ = tokio::fs::write(&opt_path, "lang:tr_tr\n").await;
    }

    // Çince zoraki dil kilitleme modlarını (I18nUpdateMod vb.) devre dışı bırak
    let mods_dir = instance_dir.join("mods");
    if mods_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&mods_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if (name.contains("i18nupdatemod") || name.contains("chinese-") || name.contains("languagepack")) && name.ends_with(".jar") {
                    let disabled_path = entry.path().with_extension("jar.disabled");
                    let _ = std::fs::rename(entry.path(), disabled_path);
                    tracing::info!("Zoraki yabancı dil kilitleme modu devre dışı bırakıldı: {}", name);
                }
            }
        }
    }
}

#[tauri::command]
async fn launch_instance(
    state: State<'_, AppState>,
    instance_id: String,
    username: Option<String>,
) -> Result<LaunchResultResponse, String> {
    // 1. Hedef profili veritabanından al (O(1) indeksli arama)
    let instance = {
        let db = state.db.lock().map_err(|e| format!("Veritabanı erişilemedi: {}", e))?;
        db.get_instance(&instance_id)
            .map_err(|e| format!("Profil sorgulanamadı: {}", e))?
            .ok_or_else(|| "Başlatılacak profil veritabanında bulunamadı.".to_string())?
    };

    // 2. Aktif hesabı belirle (Offline veya Microsoft Online)
    let root = paths::get_launcher_root_dir();
    let active_acc = AuthService::get_active_account(&root).ok();

    let final_username = username
        .filter(|u| !u.trim().is_empty())
        .or_else(|| active_acc.as_ref().map(|a| a.username.clone()))
        .unwrap_or_else(|| "Oyuncu".to_string());

    let final_uuid = active_acc
        .as_ref()
        .map(|a| a.uuid.clone())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let access_token = if let Some(ref acc) = active_acc {
        if acc.account_type == "microsoft" {
            AuthService::get_secure_token(&acc.id).unwrap_or_else(|_| "offline_token".to_string())
        } else {
            "offline_token".to_string()
        }
    } else {
        "offline_token".to_string()
    };

    // 3. Sürüm Metaverisini Al (Önce Yerel Önbelleğe Bak, Yoksa Mojang'a Git)
    let shared_dir = paths::get_shared_cache_dir();
    let local_version_json = shared_dir
        .join("versions")
        .join(&instance.minecraft_version)
        .join(format!("{}.json", instance.minecraft_version));

    let mut version_pkg = if local_version_json.is_file() {
        match tokio::fs::read_to_string(&local_version_json).await {
            Ok(content) => match serde_json::from_str::<minecraft_metadata::VersionPackage>(&content) {
                Ok(pkg) => pkg,
                Err(_) => fetch_from_mojang(&state.mojang_client, &instance.minecraft_version, &local_version_json).await?,
            },
            Err(_) => fetch_from_mojang(&state.mojang_client, &instance.minecraft_version, &local_version_json).await?,
        }
    } else {
        fetch_from_mojang(&state.mojang_client, &instance.minecraft_version, &local_version_json).await?
    };

    let instance_dir = paths::get_instances_dir().join(&instance.directory_name);
    InstanceManager::ensure_instance_layout(&instance_dir).await.map_err(|e| {
        format!("Profil klasör düzeni oluşturulamadı: {}", e)
    })?;

    // 4. Otomatik Türkçe Dil Kontrolü & Çince/Yabancı Dil Zorlamalarını Temizleme
    sanitize_instance_language_and_mods(&instance_dir).await;

    // 5. Mod Yükleyici (Loader) Entegrasyonu & Mod Kontrolü
    let mut effective_loader = instance.loader_type.clone();
    let mods_dir = instance_dir.join("mods");
    let has_mods = if mods_dir.is_dir() {
        std::fs::read_dir(&mods_dir)
            .map(|entries| entries.filter_map(|e| e.ok()).any(|e| e.path().extension().and_then(|x| x.to_str()) == Some("jar")))
            .unwrap_or(false)
    } else {
        false
    };

    if effective_loader == LoaderType::Vanilla && has_mods {
        tracing::info!("Profilde mod dosyaları tespit edildi, mod yükleyici otomatik olarak Fabric olarak ayarlandı.");
        effective_loader = LoaderType::Fabric;
    }

    match effective_loader {
        LoaderType::Fabric => {
            let loader_ver = instance.loader_version.as_deref().unwrap_or("0.16.5");
            if let Err(e) = state.fabric_adapter.patch_version_package(&mut version_pkg, loader_ver).await {
                tracing::warn!("Fabric yama uyarısı: {}", e);
            }
        }
        LoaderType::Forge => {
            let loader_ver = instance.loader_version.as_deref().unwrap_or("47.4.22");
            if let Err(e) = state.forge_adapter.patch_version_package(&mut version_pkg, loader_ver).await {
                tracing::warn!("Forge yama uyarısı: {}", e);
            }
        }
        LoaderType::NeoForge => {
            let loader_ver = instance.loader_version.as_deref().unwrap_or("latest");
            if let Err(e) = state.neoforge_adapter.patch_version_package(&mut version_pkg, loader_ver).await {
                tracing::warn!("NeoForge yama uyarısı: {}", e);
            }
        }
        _ => {}
    }

    // 5. Gerekli Kütüphaneleri ve Client JAR'ı İndir (İndirilmişse çevrimdışı başlat)
    let base_mc_version = version_pkg
        .inherits_from
        .as_deref()
        .unwrap_or(&instance.minecraft_version);

    let client_jar_path = shared_dir
        .join("versions")
        .join(base_mc_version)
        .join(format!("{}.jar", base_mc_version));

    let tasks = LaunchService::prepare_download_tasks(&version_pkg, &shared_dir);
    if let Err(e) = state.download_engine.download_all(tasks).await {
        let client_jar_exists = client_jar_path.is_file();
        if !client_jar_exists {
            return Err(format!("Minecraft temel oyun dosyası indirilemedi. İnternet bağlantınızı kontrol edin: {}", e));
        } else {
            tracing::warn!("Bazı kütüphaneler güncellenemedi ancak temel oyun dosyası mevcut, çevrimdışı başlatılıyor: {}", e);
        }
    }

    // 6. Başlatma Yapılandırması ve Argüman Derleme
    let launch_config = LaunchConfig {
        instance_dir: instance_dir.clone(),
        version_package: version_pkg,
        username: final_username.clone(),
        uuid: final_uuid,
        access_token,
        memory_mb: instance.allocated_memory_mb,
        custom_java_path: None,
        jvm_arguments: instance.jvm_arguments,
    };

    let (java_path, args) = LaunchService::build_launch_arguments(&launch_config).map_err(|e| {
        format!("Oyun argümanları oluşturulamadı (Java tespiti veya classpath hatası): {}", e)
    })?;

    // 7. Alt Süreç Olarak Minecraft'ı Başlat
    let handle = LaunchService::spawn_game(&java_path, &args, &instance_dir).map_err(|e| {
        format!("Minecraft Java süreci başlatılamadı! Lütfen Java Runtime (JDK 17/21) kurulu olduğundan emin olun. (Hata: {})", e)
    })?;

    let loader_text = match effective_loader {
        LoaderType::Fabric => "Fabric Modlu",
        LoaderType::Forge => "Forge Modlu",
        LoaderType::NeoForge => "NeoForge Modlu",
        LoaderType::Quilt => "Quilt Modlu",
        LoaderType::Vanilla => "Vanilla (Saf)",
    };

    Ok(LaunchResultResponse {
        success: true,
        pid: handle.pid,
        message: format!("Minecraft {} ({}) başarıyla başlatıldı! Oyuncu: {} (PID: {:?})", instance.minecraft_version, loader_text, final_username, handle.pid),
    })
}

/// Tek log dosyası 512 KB'ı aşarsa sıfırdan başlar; böylece log klasörü asla şişmez.
const LOG_MAX_BYTES: u64 = 512 * 1024;

/// `logs/launcher.log` dosyasına zaman damgalı satır ekler.
fn write_log(path: &std::path::Path, msg: &str) {
    use std::io::Write;

    let oversized = std::fs::metadata(path)
        .map(|m| m.len() > LOG_MAX_BYTES)
        .unwrap_or(false);

    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).write(true);
    if oversized {
        opts.truncate(true);
    } else {
        opts.append(true);
    }

    if let Ok(mut file) = opts.open(path) {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(file, "[{}] {}", ts, msg);
    }
}

/// `logs/` içinde 7 günden eski dosyaları siler (eski sürümlerin bıraktığı loglar dahil).
fn prune_old_logs(dir: &std::path::Path) {
    let max_age = std::time::Duration::from_secs(7 * 24 * 60 * 60);
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !meta.is_file() {
            continue;
        }
        let expired = meta
            .modified()
            .ok()
            .and_then(|t| t.elapsed().ok())
            .map(|age| age > max_age)
            .unwrap_or(false);
        if expired {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn main() {
    // Tek log dosyası: <root>/logs/launcher.log
    // (eskiden launcher_panic / launcher_boot / launcher_setup / launcher_tauri_error
    //  olarak 4 ayrı dosyaya yazılıyordu)
    let log_dir = paths::get_launcher_root_dir().join("logs");
    let _ = std::fs::create_dir_all(&log_dir);
    prune_old_logs(&log_dir);
    let panic_log_path = log_dir.join("launcher.log");
    let tauri_err_path = panic_log_path.clone();

    // Panik yakala — tüm kritik hataları tek log dosyasına yaz
    let panic_log = panic_log_path.clone();
    std::panic::set_hook(Box::new(move |info| {
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            format!("{:?}", info)
        };
        write_log(&panic_log, &format!("PANIC: {}\nLocation: {:?}", msg, info.location()));
    }));

    let root_dir = paths::get_launcher_root_dir();
    let _ = std::fs::create_dir_all(&root_dir);
    let db_path = root_dir.join("launcher.sqlite");

    let db = match Database::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            let err_msg = format!("Veritabanı açılamadı: {:?}\n", e);
            write_log(&panic_log_path, &err_msg);
            panic!("{}", err_msg);
        }
    };

    // Paylaşımlı HTTP istemcisi: bağlantı havuzu yeniden kullanımı, timeout ayarı
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .user_agent("EnesLauncher/1.0")
        .build()
        .unwrap_or_default();

    tauri::Builder::default()
        .setup(move |_app| {
            Ok(())
        })
        .manage(AppState {
            db: Mutex::new(db),
            mojang_client: MojangClient::new(),
            download_engine: DownloadEngine::new(8),
            modrinth: ModrinthProvider::new(),
            fabric_adapter: FabricAdapter::new(),
            forge_adapter: ForgeAdapter::new(),
            neoforge_adapter: NeoForgeAdapter::new(),
            http_client,
        })
        .invoke_handler(tauri::generate_handler![
            get_system_status,
            list_instances,
            create_instance,
            delete_instance,
            fetch_official_versions,
            search_modrinth_advanced,
            search_modrinth_mods,
            search_modrinth_modpacks,
            get_modpack_versions,
            install_remote_modpack,
            list_instance_mods,
            install_remote_mod,
            toggle_instance_mod,
            delete_instance_mod,
            list_instance_shaders,
            install_remote_shader,
            delete_instance_shader,
            save_custom_background,
            save_launcher_settings,
            load_launcher_settings,
            get_file_data_url,
            open_in_browser,
            open_instance_folder,
            update_instance_settings,
            optimize_instance_performance,
            get_instance_latest_log,
            set_instance_language,
            import_mrpack,
            export_instance_to_mrpack,
            backfill_instance_art,
            analyze_crash_log,
            list_accounts,
            add_offline_account,
            set_active_account,
            delete_account,
            start_microsoft_login,
            launch_instance
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            let msg = format!("Tauri run hatasi: {:?}\n", e);
            write_log(&tauri_err_path, &msg);
            eprintln!("{}", msg);
            std::process::exit(1);
        });
}

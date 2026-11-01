use download_engine::DownloadTask;
use java_locator::JavaLocator;
use launcher_core::{paths, LauncherError, LauncherResult};
use minecraft_metadata::{is_rule_allowed, ArgumentValue, VersionPackage};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};

pub struct LaunchService;

#[derive(Debug, Clone)]
pub struct LaunchConfig {
    pub instance_dir: PathBuf,
    pub version_package: VersionPackage,
    pub username: String,
    pub uuid: String,
    pub access_token: String,
    pub memory_mb: u32,
    pub custom_java_path: Option<PathBuf>,
    pub jvm_arguments: Option<String>,
}

pub struct LaunchHandle {
    pub child: Child,
    pub pid: Option<u32>,
}

impl LaunchService {
    /// Minecraft başlatma komut satırı argümanlarını deterministik olarak derler
    pub fn build_launch_arguments(config: &LaunchConfig) -> LauncherResult<(PathBuf, Vec<String>)> {
        let shared_dir = paths::get_shared_cache_dir();
        let libraries_dir = shared_dir.join("libraries");

        // Temel Minecraft sürümü (Örn: "1.20.1")
        // Forge/Fabric durumunda inherits_from varsa onu kullan, yoksa id
        let base_mc_version = config
            .version_package
            .inherits_from
            .as_deref()
            .unwrap_or(&config.version_package.id);

        let client_jar = shared_dir
            .join("versions")
            .join(base_mc_version)
            .join(format!("{}.jar", base_mc_version));

        // 1. Classpath oluşturma
        let mut classpath_entries = Vec::new();

        for lib in &config.version_package.libraries {
            let allowed = if let Some(ref rules) = lib.rules {
                is_rule_allowed(rules, "windows", "x64")
            } else {
                true
            };

            if !allowed {
                continue;
            }

            if let Some(ref downloads) = lib.downloads {
                if let Some(ref artifact) = downloads.artifact {
                    if let Some(ref rel_path) = artifact.path {
                        let full_lib_path = libraries_dir.join(rel_path);
                        if !classpath_entries.contains(&full_lib_path) {
                            classpath_entries.push(full_lib_path);
                        }
                    }
                }
            }
        }

        // Client JAR dosyasını classpath'e ekle
        if !classpath_entries.contains(&client_jar) {
            classpath_entries.push(client_jar);
        }

        // Windows üzerinde noktalı virgül (;) ile birleştir
        let classpath_str = classpath_entries
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(";");

        // 2. Placeholder Değişkenleri
        let natives_dir = config.instance_dir.join("natives");
        let assets_dir = shared_dir.join("assets");
        let asset_index_id = config
            .version_package
            .asset_index
            .as_ref()
            .map(|a| a.id.as_str())
            .unwrap_or("legacy");

        let mut vars = HashMap::new();
        vars.insert("auth_player_name", config.username.as_str());
        // ÖNEMLİ: Forge -DignoreList=${version_name}.jar kuralının vanilla client jar'ı
        // (1.20.1.jar) doğru eşleştirebilmesi için version_name temel sürüm olmalıdır.
        vars.insert("version_name", base_mc_version);
        vars.insert("game_directory", config.instance_dir.to_str().unwrap_or("."));
        vars.insert("assets_root", assets_dir.to_str().unwrap_or("."));
        vars.insert("assets_index_name", asset_index_id);
        vars.insert("auth_uuid", config.uuid.as_str());
        vars.insert("auth_access_token", config.access_token.as_str());
        vars.insert("user_type", "msa");
        vars.insert("version_type", config.version_package.version_type.as_str());
        vars.insert("natives_directory", natives_dir.to_str().unwrap_or("."));
        vars.insert("launcher_name", "EnesLauncher");
        vars.insert("launcher_version", "0.1.0");
        vars.insert("classpath", classpath_str.as_str());
        let lib_dir_str = libraries_dir.to_string_lossy().to_string();
        vars.insert("library_directory", lib_dir_str.as_str());
        vars.insert("classpath_separator", ";");
        vars.insert("clientid", "EnesLauncherClient");
        vars.insert("auth_xuid", "0");

        // 3. JVM Argümanları
        let mut final_args = Vec::new();

        // Temel JVM Bellek Parametreleri
        final_args.push("-Xms512M".to_string());
        final_args.push(format!("-Xmx{}M", config.memory_mb));
        final_args.push(format!("-Djava.library.path={}", natives_dir.to_string_lossy()));
        final_args.push(format!("-Dminecraft.applet.TargetDirectory={}", config.instance_dir.to_string_lossy()));

        // Kullanıcı tanımlı ek JVM parametreleri (Örn: Aikar G1GC optimizasyonları)
        if let Some(ref custom_jvm) = config.jvm_arguments {
            for token in custom_jvm.split_whitespace() {
                if !token.is_empty() {
                    final_args.push(token.to_string());
                }
            }
        }

        if let Some(ref arguments) = config.version_package.arguments {
            for jvm_arg in &arguments.jvm {
                match jvm_arg {
                    ArgumentValue::Simple(val) => {
                        final_args.push(substitute_placeholders(val, &vars));
                    }
                    ArgumentValue::Conditional { rules, value } => {
                        if is_rule_allowed(rules, "windows", "x64") {
                            match value {
                                minecraft_metadata::ArgumentInnerValue::Single(s) => {
                                    final_args.push(substitute_placeholders(s, &vars));
                                }
                                minecraft_metadata::ArgumentInnerValue::Multiple(list) => {
                                    for s in list {
                                        final_args.push(substitute_placeholders(s, &vars));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // Klasik fallback JVM argümanları
            final_args.push("-cp".to_string());
            final_args.push(classpath_str.clone());
        }

        // Main Class
        final_args.push(config.version_package.main_class.clone());

        // 4. Oyun Argümanları
        if let Some(ref arguments) = config.version_package.arguments {
            for game_arg in &arguments.game {
                match game_arg {
                    ArgumentValue::Simple(val) => {
                        final_args.push(substitute_placeholders(val, &vars));
                    }
                    ArgumentValue::Conditional { rules, value } => {
                        if is_rule_allowed(rules, "windows", "x64") {
                            match value {
                                minecraft_metadata::ArgumentInnerValue::Single(s) => {
                                    final_args.push(substitute_placeholders(s, &vars));
                                }
                                minecraft_metadata::ArgumentInnerValue::Multiple(list) => {
                                    for s in list {
                                        final_args.push(substitute_placeholders(s, &vars));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else if let Some(ref legacy_args) = config.version_package.minecraft_arguments {
            for token in legacy_args.split_whitespace() {
                final_args.push(substitute_placeholders(token, &vars));
            }
        }

        // Modpack ve Instance İzolasyon Garantisi: --gameDir parametresi yoksa instance dizinini bağla
        if !final_args.iter().any(|arg| arg == "--gameDir") {
            final_args.push("--gameDir".to_string());
            final_args.push(config.instance_dir.to_string_lossy().to_string());
        }

        // 5. Java Çalıştırılabilir Dosyasını Belirle
        let java_path = if let Some(ref custom) = config.custom_java_path {
            custom.clone()
        } else {
            let required_major = config
                .version_package
                .java_version
                .as_ref()
                .map(|j| j.major_version)
                .unwrap_or(17);
            let runtime = JavaLocator::find_matching_java(required_major)?;
            runtime.path
        };

        Ok((java_path, final_args))
    }

    /// Oyunu ayrı bir alt süreç (subprocess) olarak başlatır
    pub fn spawn_game(
        java_path: &Path,
        args: &[String],
        working_dir: &Path,
    ) -> LauncherResult<LaunchHandle> {
        // tokio::process yerine std::process::Command kullanıyoruz —
        // tokio child pipe blokunu önlemek ve DETACHED_PROCESS desteği için.
        // Windows'ta konsol penceresi açılmaması için javaw.exe tercih et
        let effective_java = if java_path.ends_with("java.exe") {
            let javaw = java_path.with_file_name("javaw.exe");
            if javaw.is_file() {
                javaw
            } else {
                java_path.to_path_buf()
            }
        } else {
            java_path.to_path_buf()
        };

        let mut cmd = std::process::Command::new(effective_java);
        cmd.args(args);
        cmd.current_dir(working_dir);
        // Stdout ve Stderr'ı instance dizinindeki log dosyalarına yönlendir (Erken crash teşhisi ve buffer tıkanmasını önleme)
        let stdout_file = std::fs::File::create(working_dir.join("game_stdout.log")).ok();
        let stderr_file = std::fs::File::create(working_dir.join("game_stderr.log")).ok();

        if let Some(f) = stdout_file {
            cmd.stdout(f);
        } else {
            cmd.stdout(Stdio::null());
        }

        if let Some(f) = stderr_file {
            cmd.stderr(f);
        } else {
            cmd.stderr(Stdio::null());
        }
        cmd.stdin(Stdio::null());

        // Windows: CREATE_NEW_PROCESS_GROUP — Launcher kapansa bile bağımsız çalışır (DETACHED_PROCESS kullanılmamalıdır, process handle'ı koparır)
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
            cmd.creation_flags(CREATE_NEW_PROCESS_GROUP);
        }

        let child = cmd.spawn().map_err(|e| {
            LauncherError::Unknown(format!("Oyun süreci başlatılamadı: {}", e))
        })?;

        let pid = child.id();
        tracing::info!("Minecraft başlatıldı! PID: {:?}", pid);

        Ok(LaunchHandle { child, pid: Some(pid) })
    }

    /// Bir sürümün indirme görevlerini derler
    pub fn prepare_download_tasks(
        version_pkg: &VersionPackage,
        shared_dir: &Path,
    ) -> Vec<DownloadTask> {
        let mut tasks = Vec::new();

        let base_id = version_pkg
            .inherits_from
            .as_deref()
            .unwrap_or(&version_pkg.id);

        // 1. Client JAR
        if let Some(ref downloads) = version_pkg.downloads {
            let client_dest = shared_dir
                .join("versions")
                .join(base_id)
                .join(format!("{}.jar", base_id));

            tasks.push(DownloadTask {
                id: format!("client-jar-{}", base_id),
                url: downloads.client.url.clone(),
                destination: client_dest,
                expected_sha1: Some(downloads.client.sha1.clone()),
                expected_size: Some(downloads.client.size),
            });
        }

        // 2. Kütüphaneler (Windows x64 için izin verilenler)
        let libraries_dir = shared_dir.join("libraries");
        for lib in &version_pkg.libraries {
            let allowed = if let Some(ref rules) = lib.rules {
                is_rule_allowed(rules, "windows", "x64")
            } else {
                true
            };

            if !allowed {
                continue;
            }

            if let Some(ref downloads) = lib.downloads {
                if let Some(ref artifact) = downloads.artifact {
                    if let Some(ref rel_path) = artifact.path {
                        tasks.push(DownloadTask {
                            id: format!("lib-{}", lib.name),
                            url: artifact.url.clone(),
                            destination: libraries_dir.join(rel_path),
                            expected_sha1: Some(artifact.sha1.clone()),
                            expected_size: Some(artifact.size),
                        });
                    }
                }
            }
        }

        // 3. Asset Index Dosyası (assets/indexes/<id>.json - Minecraft'ın açılabilmesi için şarttır)
        if let Some(ref asset_index) = version_pkg.asset_index {
            let asset_index_dest = shared_dir
                .join("assets")
                .join("indexes")
                .join(format!("{}.json", asset_index.id));

            tasks.push(DownloadTask {
                id: format!("asset-index-{}", asset_index.id),
                url: asset_index.url.clone(),
                destination: asset_index_dest,
                expected_sha1: Some(asset_index.sha1.clone()),
                expected_size: Some(asset_index.size),
            });
        }

        tasks
    }
}

fn substitute_placeholders(template: &str, vars: &HashMap<&str, &str>) -> String {
    let mut result = template.to_string();
    for (k, v) in vars {
        let pattern = format!("${{{}}}", k);
        result = result.replace(&pattern, v);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_substitute_placeholders() {
        let mut vars = HashMap::new();
        vars.insert("auth_player_name", "Enes");
        vars.insert("version_name", "1.21.1");

        let input = "--username ${auth_player_name} --version ${version_name}";
        let res = substitute_placeholders(input, &vars);
        assert_eq!(res, "--username Enes --version 1.21.1");
    }
}


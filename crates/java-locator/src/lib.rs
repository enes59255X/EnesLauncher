use launcher_core::error::LauncherError;
use launcher_core::LauncherResult;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JavaRuntime {
    pub path: PathBuf,
    pub major_version: u32,
    pub full_version: String,
    pub is_64bit: bool,
}

pub struct JavaLocator;

impl JavaLocator {
    pub fn locate_all() -> Vec<JavaRuntime> {
        let mut runtimes = Vec::new();
        let mut searched_paths = Vec::new();

        // 1. JAVA_HOME kontrolü
        if let Ok(java_home) = std::env::var("JAVA_HOME") {
            let p = PathBuf::from(java_home).join("bin").join("java.exe");
            searched_paths.push(p);
        }

        // 2. PATH içindeki java
        if let Ok(path_var) = std::env::var("PATH") {
            for entry in std::env::split_paths(&path_var) {
                let p = entry.join("java.exe");
                if p.is_file() {
                    searched_paths.push(p);
                }
            }
        }

        // 3. Standart Program Files dizinleri
        let standard_roots = [
            "C:\\Program Files\\Microsoft",
            "C:\\Program Files\\Java",
            "C:\\Program Files\\Eclipse Adoptium",
            "C:\\Program Files\\BellSoft",
            "C:\\Program Files\\Zulu",
        ];

        for root in standard_roots {
            let root_path = Path::new(root);
            if root_path.is_dir() {
                if let Ok(entries) = std::fs::read_dir(root_path) {
                    for entry in entries.flatten() {
                        let java_exe = entry.path().join("bin").join("java.exe");
                        if java_exe.is_file() {
                            searched_paths.push(java_exe);
                        }
                    }
                }
            }
        }

        // 4. Kullanıcı kapsamındaki kurulumlar:
        //    %LOCALAPPDATA%\Programs\<vendor>\<jdk>\bin\java.exe
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let programs = PathBuf::from(&local_app_data).join("Programs");
            for vendor in [
                "Microsoft",
                "Java",
                "Eclipse Adoptium",
                "BellSoft",
                "Zulu",
                "Amazon Corretto",
            ] {
                let vendor_root = programs.join(vendor);
                if let Ok(entries) = std::fs::read_dir(&vendor_root) {
                    for entry in entries.flatten() {
                        let java_exe = entry.path().join("bin").join("java.exe");
                        if java_exe.is_file() {
                            searched_paths.push(java_exe);
                        }
                    }
                }
            }

            // 5. Minecraft resmi launcher'ın indirdiği çalışma zamanları
            let mc_runtime_root = PathBuf::from(&local_app_data)
                .join("Packages")
                .join("Microsoft.4297127D64EC6_8wekyb3d8bbwe")
                .join("LocalCache")
                .join("Local")
                .join("runtime");
            collect_java_exes(&mc_runtime_root, 5, &mut searched_paths);
        }

        // 6. Eski Minecraft launcher'ın runtime klasörü: %APPDATA%\.minecraft\runtime
        if let Ok(app_data) = std::env::var("APPDATA") {
            let legacy_runtime = PathBuf::from(app_data).join(".minecraft").join("runtime");
            collect_java_exes(&legacy_runtime, 5, &mut searched_paths);
        }

        // Tekrar edenleri temizle ve metadata oku
        searched_paths.sort();
        searched_paths.dedup();

        for path in searched_paths {
            if let Some(runtime) = Self::inspect_java(&path) {
                runtimes.push(runtime);
            }
        }

        runtimes
    }

    pub fn find_matching_java(required_major: u32) -> LauncherResult<JavaRuntime> {
        let all = Self::locate_all();

        // Tam eşleşen 64-bit Java
        if let Some(exact) = all.iter().find(|j| j.major_version == required_major && j.is_64bit) {
            return Ok(exact.clone());
        }

        // Geriye dönük uyumlu daha yeni 64-bit Java (örn: Minecraft 17 istiyor ama 21 var)
        if let Some(newer) = all.iter().find(|j| j.major_version >= required_major && j.is_64bit) {
            return Ok(newer.clone());
        }

        // Herhangi bir 64-bit
        if let Some(any_64) = all.iter().find(|j| j.is_64bit) {
            return Ok(any_64.clone());
        }

        // Herhangi bir Java
        all.into_iter().next().ok_or_else(|| {
            LauncherError::NotFound(format!(
                "Sistemde Java {} bulunamadı. Lütfen 64-bit Java kurun.",
                required_major
            ))
        })
    }

    pub fn inspect_java(java_exe: &Path) -> Option<JavaRuntime> {
        if !java_exe.is_file() {
            return None;
        }

        // JDK/JRE kök dizinindeki 'release' dosyasını okumayı dene
        if let Some(bin_dir) = java_exe.parent() {
            if let Some(jdk_root) = bin_dir.parent() {
                let release_file = jdk_root.join("release");
                if release_file.is_file() {
                    if let Ok(file) = File::open(release_file) {
                        let reader = BufReader::new(file);
                        let mut java_version = String::new();
                        let mut os_arch = String::new();

                        for line in reader.lines().flatten() {
                            if let Some(val) = line.strip_prefix("JAVA_VERSION=") {
                                java_version = val.trim_matches('"').to_string();
                            } else if let Some(val) = line.strip_prefix("OS_ARCH=") {
                                os_arch = val.trim_matches('"').to_string();
                            }
                        }

                        if !java_version.is_empty() {
                            let major = parse_major_version(&java_version);
                            let is_64bit = os_arch.contains("x86_64") || os_arch.contains("amd64") || os_arch.contains("x64");
                            return Some(JavaRuntime {
                                path: java_exe.to_path_buf(),
                                major_version: major,
                                full_version: java_version,
                                is_64bit,
                            });
                        }
                    }
                }
            }
        }

        // 'release' dosyası yoksa varsayılan fallback
        Some(JavaRuntime {
            path: java_exe.to_path_buf(),
            major_version: 17,
            full_version: "17.0.0".to_string(),
            is_64bit: true,
        })
    }
}

/// Verilen klasörün altında (en fazla `depth` seviye) `bin\java.exe` dosyalarını toplar.
fn collect_java_exes(root: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth == 0 || !root.is_dir() {
        return;
    }

    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let java_exe = path.join("bin").join("java.exe");
            if java_exe.is_file() {
                out.push(java_exe);
            }
            collect_java_exes(&path, depth - 1, out);
        }
    }
}

fn parse_major_version(v: &str) -> u32 {
    let clean = v.trim_start_matches("1.");
    if let Some(first_dot) = clean.find('.') {
        clean[..first_dot].parse().unwrap_or(17)
    } else {
        clean.parse().unwrap_or(17)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_major_version() {
        assert_eq!(parse_major_version("1.8.0_312"), 8);
        assert_eq!(parse_major_version("17.0.2"), 17);
        assert_eq!(parse_major_version("21.0.1"), 21);
    }

    #[test]
    fn test_locate_does_not_panic() {
        // Makineye bağımlı değil: Java kurulu olmayan bir PC'de veya CI'da da geçer.
        for runtime in JavaLocator::locate_all() {
            assert!(
                runtime.major_version > 0,
                "Geçersiz Java sürümü: {}",
                runtime.full_version
            );
            assert!(
                runtime.path.is_file(),
                "Java yolu mevcut değil: {:?}",
                runtime.path
            );
        }
    }
}


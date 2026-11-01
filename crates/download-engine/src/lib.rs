use futures_util::StreamExt;
use launcher_core::{LauncherError, LauncherResult};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTask {
    pub id: String,
    pub url: String,
    pub destination: PathBuf,
    pub expected_sha1: Option<String>,
    pub expected_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub task_id: String,
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub is_completed: bool,
}

pub struct DownloadEngine {
    client: reqwest::Client,
    semaphore: Arc<Semaphore>,
}

impl DownloadEngine {
    pub fn new(max_concurrent: usize) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("EnesLauncher-DownloadEngine/0.1.0")
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(30))
            .pool_max_idle_per_host(10)
            .build()
            .unwrap_or_default();

        Self {
            client,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    /// Tek bir dosyayı hash doğrulamasıyla indirir
    pub async fn download_file(&self, task: &DownloadTask) -> LauncherResult<()> {
        let _permit = self.semaphore.acquire().await.map_err(|e| {
            LauncherError::Unknown(format!("Semaphore izni alınamadı: {}", e))
        })?;

        // Dosya zaten varsa ve boyutu / SHA1'i tutuyorsa tekrar indirme!
        if task.destination.is_file() {
            // Hızlı Boyut Doğrulaması (Fast-Path):
            // Disk üzerindeki 100+ JAR dosyasını her oyun başlatmada SHA1 ile tekrar tekrar hashlemek
            // başlatma süresini 20-30 saniye geciktirir. Beklenen boyut eşleşiyorsa anında geç.
            if let Some(expected_size) = task.expected_size {
                if expected_size > 0 {
                    if let Ok(meta) = tokio::fs::metadata(&task.destination).await {
                        if meta.len() == expected_size {
                            return Ok(());
                        }
                    }
                }
            }

            if let Some(ref expected_sha1) = task.expected_sha1 {
                if !expected_sha1.trim().is_empty() {
                    if let Ok(actual_sha1) = calculate_file_sha1(&task.destination).await {
                        if actual_sha1.eq_ignore_ascii_case(expected_sha1) {
                            return Ok(());
                        }
                    }
                } else {
                    return Ok(());
                }
            } else {
                return Ok(());
            }
        }

        let temp_destination = task.destination.with_extension("part");
        if let Some(parent) = task.destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut retries = 3;
        let mut success = false;

        while retries > 0 && !success {
            match self.execute_single_download(&task.url, &temp_destination).await {
                Ok(_) => {
                    // Hash Doğrulaması (Yalnızca sha1 boş değilse)
                    if let Some(ref expected_sha1) = task.expected_sha1 {
                        if !expected_sha1.trim().is_empty() {
                            let actual_sha1 = calculate_file_sha1(&temp_destination).await?;
                            if !actual_sha1.eq_ignore_ascii_case(expected_sha1) {
                                let _ = tokio::fs::remove_file(&temp_destination).await;
                                retries -= 1;
                                tracing::warn!(
                                    "SHA1 uyuşmazlığı (Beklenen: {}, Alınan: {}). Kalan deneme: {}",
                                    expected_sha1, actual_sha1, retries
                                );
                                continue;
                            }
                        }
                    }

                    // Boyut Doğrulaması (Yalnızca 0'dan büyükse)
                    if let Some(expected_size) = task.expected_size {
                        if expected_size > 0 {
                            let metadata = tokio::fs::metadata(&temp_destination).await?;
                            if metadata.len() != expected_size {
                                let _ = tokio::fs::remove_file(&temp_destination).await;
                                retries -= 1;
                                continue;
                            }
                        }
                    }

                    // Geçici dosyayı nihai konuma taşı
                    tokio::fs::rename(&temp_destination, &task.destination).await?;
                    success = true;
                }
                Err(err) => {
                    retries -= 1;
                    if retries == 0 {
                        let _ = tokio::fs::remove_file(&temp_destination).await;
                        return Err(err);
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
            }
        }

        if !success {
            return Err(LauncherError::Network(format!(
                "Dosya indirme başarısız oldu: {}",
                task.url
            )));
        }

        Ok(())
    }

    async fn execute_single_download(&self, url: &str, temp_path: &Path) -> LauncherResult<()> {
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| LauncherError::Network(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(LauncherError::Network(format!(
                "HTTP sunucu hatası: {}",
                resp.status()
            )));
        }

        let mut file = File::create(temp_path).await?;
        let mut stream = resp.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| LauncherError::Network(e.to_string()))?;
            file.write_all(&chunk).await?;
        }

        file.flush().await?;
        Ok(())
    }

    /// Birden fazla görevi eşzamanlı olarak indirir (Hayati olmayan eksik kütüphaneler oyunu kilitlemez)
    pub async fn download_all(&self, tasks: Vec<DownloadTask>) -> LauncherResult<()> {
        let mut handles = Vec::new();

        for task in tasks {
            let engine = self.clone_instance();
            handles.push(tokio::spawn(async move {
                let is_critical = task.id.starts_with("client-jar-");
                let res = engine.download_file(&task).await;
                (task.id, is_critical, res)
            }));
        }

        for handle in handles {
            if let Ok((task_id, is_critical, res)) = handle.await {
                if let Err(e) = res {
                    if is_critical {
                        // Yalnızca ana client.jar gibi hayati dosyalarda durdur
                        return Err(e);
                    } else {
                        tracing::warn!("Hayati olmayan kütüphane atlandı ({:?}): {}", task_id, e);
                    }
                }
            }
        }

        Ok(())
    }

    fn clone_instance(&self) -> Self {
        Self {
            client: self.client.clone(),
            semaphore: Arc::clone(&self.semaphore),
        }
    }
}

pub async fn calculate_file_sha1(path: &Path) -> LauncherResult<String> {
    let mut file = File::open(path).await?;
    let mut hasher = Sha1::new();
    let mut buffer = [0u8; 8192];

    use tokio::io::AsyncReadExt;
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
    async fn test_calculate_sha1() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        tokio::fs::write(&file_path, b"hello world").await.unwrap();

        let sha1 = calculate_file_sha1(&file_path).await.unwrap();
        // "hello world" sha1: 2aae6c35c94fcfb415dbe95f408b9ce91ee846ed
        assert_eq!(sha1, "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed");
    }
}


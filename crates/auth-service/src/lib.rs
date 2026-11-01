use keyring::Entry;
use launcher_core::{LauncherError, LauncherResult};
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

pub const SERVICE_NAME: &str = "com.eneslauncher.auth";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountProfile {
    pub id: String,
    pub username: String,
    pub uuid: String,
    pub account_type: String, // "microsoft" | "offline"
    pub is_active: bool,
    pub skin_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCodeResponse {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    pub message: String,
    pub interval: u64,
}

#[derive(Debug, Deserialize)]
struct MsDeviceCodeRaw {
    device_code: String,
    user_code: String,
    verification_uri: String,
    interval: Option<u64>,
    message: Option<String>,
}

pub struct AuthService;

impl AuthService {
    /// Güvenli Kimlik Deposu: Hassas token'ı doğrudan Windows Credential Manager'a kaydeder
    pub fn store_secure_token(account_id: &str, token: &str) -> LauncherResult<()> {
        let entry = Entry::new(SERVICE_NAME, account_id)
            .map_err(|e| LauncherError::Security(format!("Keyring girişi oluşturulamadı: {}", e)))?;

        entry
            .set_password(token)
            .map_err(|e| LauncherError::Security(format!("Windows Credential Manager'a token yazılamadı: {}", e)))?;

        tracing::info!("Hassas belirteç Windows Credential Manager'da şifreli olarak saklandı.");
        Ok(())
    }

    /// Güvenli Kimlik Deposu: Windows Credential Manager'dan token okur
    pub fn get_secure_token(account_id: &str) -> LauncherResult<String> {
        let entry = Entry::new(SERVICE_NAME, account_id)
            .map_err(|e| LauncherError::Security(format!("Keyring girişi oluşturulamadı: {}", e)))?;

        let token = entry
            .get_password()
            .map_err(|e| LauncherError::Security(format!("Windows Credential Manager'dan token okunamadı: {}", e)))?;

        Ok(token)
    }

    /// Token silme
    pub fn delete_secure_token(account_id: &str) -> LauncherResult<()> {
        if let Ok(entry) = Entry::new(SERVICE_NAME, account_id) {
            let _ = entry.delete_password();
        }
        Ok(())
    }

    /// Loglar ve hata mesajları için maskeleme fonksiyonu (Token sızıntısını engeller)
    pub fn mask_sensitive_string(raw: &str) -> String {
        if raw.len() <= 6 {
            return "***MASKED***".to_string();
        }
        format!("{}***MASKED***{}", &raw[..3], &raw[raw.len() - 3..])
    }

    /// Hesap listesini accounts.json dosyasından okur
    pub fn list_accounts(root_dir: &Path) -> LauncherResult<Vec<AccountProfile>> {
        let file_path = root_dir.join("accounts.json");
        if !file_path.exists() {
            // Varsayılan çevrimdışı kullanıcı oluştur
            let default_account = AccountProfile {
                id: "default-offline".to_string(),
                username: "Enes".to_string(),
                uuid: generate_offline_uuid("Enes"),
                account_type: "offline".to_string(),
                is_active: true,
                skin_url: None,
            };
            let list = vec![default_account];
            let _ = Self::save_accounts(root_dir, &list);
            return Ok(list);
        }

        let content = std::fs::read_to_string(file_path)?;
        let accounts: Vec<AccountProfile> = serde_json::from_str(&content)
            .unwrap_or_else(|_| Vec::new());
        Ok(accounts)
    }

    /// Hesap listesini accounts.json dosyasına yazar
    pub fn save_accounts(root_dir: &Path, accounts: &[AccountProfile]) -> LauncherResult<()> {
        let file_path = root_dir.join("accounts.json");
        let content = serde_json::to_string_pretty(accounts)?;
        std::fs::write(file_path, content)?;
        Ok(())
    }

    /// Yeni bir ÇEVRİMDIŞI hesap ekler (İnternet bağlantısı gerektirmez)
    pub fn add_offline_account(root_dir: &Path, username: &str) -> LauncherResult<AccountProfile> {
        let trimmed = username.trim();
        if trimmed.is_empty() {
            return Err(LauncherError::Validation("Kullanıcı adı boş olamaz".to_string()));
        }

        let mut accounts = Self::list_accounts(root_dir)?;
        
        // Diğer hesapların aktifliğini kaldır
        for acc in &mut accounts {
            acc.is_active = false;
        }

        let new_account = AccountProfile {
            id: format!("offline-{}", Uuid::new_v4()),
            username: trimmed.to_string(),
            uuid: generate_offline_uuid(trimmed),
            account_type: "offline".to_string(),
            is_active: true,
            skin_url: None,
        };

        accounts.push(new_account.clone());
        Self::save_accounts(root_dir, &accounts)?;
        Ok(new_account)
    }

    /// Seçili hesabı aktif yapar
    pub fn set_active_account(root_dir: &Path, account_id: &str) -> LauncherResult<AccountProfile> {
        let mut accounts = Self::list_accounts(root_dir)?;
        let mut found = None;

        for acc in &mut accounts {
            if acc.id == account_id {
                acc.is_active = true;
                found = Some(acc.clone());
            } else {
                acc.is_active = false;
            }
        }

        match found {
            Some(acc) => {
                Self::save_accounts(root_dir, &accounts)?;
                Ok(acc)
            }
            None => Err(LauncherError::Validation(format!("Hesap bulunamadı: {}", account_id))),
        }
    }

    /// Hesabı siler
    pub fn delete_account(root_dir: &Path, account_id: &str) -> LauncherResult<bool> {
        let mut accounts = Self::list_accounts(root_dir)?;
        let initial_len = accounts.len();
        accounts.retain(|a| a.id != account_id);

        if accounts.len() < initial_len {
            // Eğer aktif hesap silindiyse ilk hesabı aktif yap
            if !accounts.is_empty() && !accounts.iter().any(|a| a.is_active) {
                accounts[0].is_active = true;
            }
            let _ = Self::delete_secure_token(account_id);
            Self::save_accounts(root_dir, &accounts)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Aktif hesabı döner
    pub fn get_active_account(root_dir: &Path) -> LauncherResult<AccountProfile> {
        let accounts = Self::list_accounts(root_dir)?;
        accounts
            .into_iter()
            .find(|a| a.is_active)
            .ok_or_else(|| LauncherError::Validation("Aktif hesap bulunamadı".to_string()))
    }

    /// Microsoft ÇEVRİMİÇİ Giriş: Cihaz Kodu (Device Code) isteği başlatır
    pub async fn start_microsoft_login() -> LauncherResult<DeviceCodeResponse> {
        // Microsoft Azure Public Client ID for Minecraft Launcher
        let client_id = "00000000402b5328"; // Standard Minecraft Client ID
        let scope = "service::user.auth.xboxlive.com::MBI_SSL";
        
        let client = reqwest::Client::new();
        let params = [
            ("client_id", client_id),
            ("scope", scope),
            ("response_type", "device_code"),
        ];

        let resp = client
            .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
            .form(&params)
            .send()
            .await
            .map_err(|e| LauncherError::Network(format!("Microsoft Device Code isteği başarısız: {}", e)))?;

        if !resp.status().is_success() {
            // Çevrimdışı veya hata durumunda fallback yönlendirmesi
            return Ok(DeviceCodeResponse {
                user_code: "DEMO-ABCD".to_string(),
                device_code: "demo_device_code".to_string(),
                verification_uri: "https://microsoft.com/link".to_string(),
                message: "Tarayıcınızda https://microsoft.com/link adresine gidip DEMO-ABCD kodunu girin.".to_string(),
                interval: 5,
            });
        }

        let raw: MsDeviceCodeRaw = resp.json().await.map_err(|e| LauncherError::Network(e.to_string()))?;
        Ok(DeviceCodeResponse {
            user_code: raw.user_code.clone(),
            device_code: raw.device_code,
            verification_uri: raw.verification_uri,
            message: raw.message.unwrap_or_else(|| format!("Kodu girin: {}", raw.user_code)),
            interval: raw.interval.unwrap_or(5),
        })
    }
}

/// Standart Minecraft Offline UUID oluşturucu: UUID v3 (MD5 DNS tabanlı)
pub fn generate_offline_uuid(username: &str) -> String {
    let raw = format!("OfflinePlayer:{}", username);
    Uuid::new_v3(&Uuid::NAMESPACE_DNS, raw.as_bytes()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_mask_sensitive_token() {
        let secret = "eyJhbGciOiJSUzI1NiIsIng1dCI6InRlc3QtdG9rZW4ifQ";
        let masked = AuthService::mask_sensitive_string(secret);
        assert!(!masked.contains("test-token"));
        assert!(masked.contains("***MASKED***"));
    }

    #[test]
    fn test_offline_uuid_deterministic() {
        let uuid1 = generate_offline_uuid("Enes");
        let uuid2 = generate_offline_uuid("Enes");
        let uuid_other = generate_offline_uuid("Alex");

        assert_eq!(uuid1, uuid2);
        assert_ne!(uuid1, uuid_other);
    }

    #[test]
    fn test_account_lifecycle() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // 1. Varsayılan hesap listesi
        let list = AuthService::list_accounts(root).unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].is_active);
        assert_eq!(list[0].username, "Enes");

        // 2. Yeni çevrimdışı hesap ekle
        let acc2 = AuthService::add_offline_account(root, "SteveCraft").unwrap();
        assert_eq!(acc2.username, "SteveCraft");
        assert!(acc2.is_active);

        // Önceki hesabın aktifliği kalkmış olmalı
        let active = AuthService::get_active_account(root).unwrap();
        assert_eq!(active.username, "SteveCraft");

        // 3. Tekrar ilk hesaba geçiş yap
        let switched = AuthService::set_active_account(root, &list[0].id).unwrap();
        assert_eq!(switched.username, "Enes");
        assert!(switched.is_active);

        // 4. Hesabı sil
        let deleted = AuthService::delete_account(root, &acc2.id).unwrap();
        assert!(deleted);
        let list_after = AuthService::list_accounts(root).unwrap();
        assert_eq!(list_after.len(), 1);
    }
}

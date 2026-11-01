use launcher_core::models::{CreateInstanceRequest, InstanceSummary, LoaderType};
use launcher_core::{LauncherError, LauncherResult};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open<P: AsRef<Path>>(path: P) -> LauncherResult<Self> {
        let conn = Connection::open(path).map_err(|e| LauncherError::Database(e.to_string()))?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.run_migrations()?;
        Ok(db)
    }

    pub fn open_in_memory() -> LauncherResult<Self> {
        let conn = Connection::open_in_memory().map_err(|e| LauncherError::Database(e.to_string()))?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.run_migrations()?;
        Ok(db)
    }

    fn run_migrations(&self) -> LauncherResult<()> {
        let conn = self.conn.lock().unwrap();

        // GÜVENLİK & PERFORMANS: WAL modu ve eşzamanlılık PRAGMA'ları
        // WAL (Write-Ahead Logging) ani kapanmalarda veritabanı bozulmasını (corruption) engeller,
        // okuma/yazma kilit çakışmalarını ortadan kaldırır. busy_timeout kilitlenmeyi önler.
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;
            PRAGMA busy_timeout = 5000;
            "
        ).map_err(|e| LauncherError::Database(e.to_string()))?;

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS instances (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                directory_name TEXT NOT NULL UNIQUE,
                minecraft_version TEXT NOT NULL,
                loader_type TEXT NOT NULL,
                loader_version TEXT,
                java_path TEXT,
                allocated_memory_mb INTEGER NOT NULL DEFAULT 4096,
                jvm_arguments TEXT,
                game_resolution_width INTEGER DEFAULT 854,
                game_resolution_height INTEGER DEFAULT 480,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                last_played_at DATETIME
            );

            CREATE TABLE IF NOT EXISTS installed_mods (
                id TEXT PRIMARY KEY,
                instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
                provider TEXT NOT NULL,
                project_id TEXT,
                version_id TEXT,
                file_name TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                sha512_hash TEXT,
                is_enabled INTEGER NOT NULL DEFAULT 1,
                is_pinned INTEGER NOT NULL DEFAULT 0,
                installed_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(instance_id, file_name)
            );

            CREATE TABLE IF NOT EXISTS downloads (
                id TEXT PRIMARY KEY,
                instance_id TEXT REFERENCES instances(id) ON DELETE CASCADE,
                destination_path TEXT NOT NULL,
                url TEXT NOT NULL,
                expected_size INTEGER NOT NULL,
                downloaded_size INTEGER NOT NULL DEFAULT 0,
                expected_hash TEXT,
                hash_algorithm TEXT,
                status TEXT NOT NULL,
                retry_count INTEGER NOT NULL DEFAULT 0,
                updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS operation_journal (
                id TEXT PRIMARY KEY,
                instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
                operation_type TEXT NOT NULL,
                stage TEXT NOT NULL,
                backup_path TEXT,
                details_json TEXT,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            ",
        )
        .map_err(|e| LauncherError::Database(e.to_string()))?;

        Ok(())
    }

    pub fn create_instance(&self, req: CreateInstanceRequest) -> LauncherResult<InstanceSummary> {
        let conn = self.conn.lock().unwrap();
        let id = Uuid::new_v4().to_string();
        let dir_name = format!("{}-{}", sanitize_dir_name(&req.name), &id[..8]);
        let memory = req.allocated_memory_mb.unwrap_or(4096);
        let loader_str = req.loader_type.to_string();

        conn.execute(
            "INSERT INTO instances (id, name, directory_name, minecraft_version, loader_type, loader_version, allocated_memory_mb)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                req.name,
                dir_name,
                req.minecraft_version,
                loader_str,
                req.loader_version,
                memory
            ],
        )
        .map_err(|e| LauncherError::Database(e.to_string()))?;

        Ok(InstanceSummary {
            id,
            name: req.name,
            directory_name: dir_name,
            minecraft_version: req.minecraft_version,
            loader_type: req.loader_type,
            loader_version: req.loader_version,
            allocated_memory_mb: memory,
            jvm_arguments: None,
            created_at: chrono_like_now(),
            last_played_at: None,
        })
    }

    pub fn list_instances(&self) -> LauncherResult<Vec<InstanceSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, directory_name, minecraft_version, loader_type, loader_version, allocated_memory_mb, created_at, last_played_at, jvm_arguments
                 FROM instances ORDER BY created_at DESC",
            )
            .map_err(|e| LauncherError::Database(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                let loader_str: String = row.get(4)?;
                let loader_type = loader_str.parse::<LoaderType>().unwrap_or(LoaderType::Vanilla);

                Ok(InstanceSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    directory_name: row.get(2)?,
                    minecraft_version: row.get(3)?,
                    loader_type,
                    loader_version: row.get(5)?,
                    allocated_memory_mb: row.get(6)?,
                    created_at: row.get(7)?,
                    last_played_at: row.get(8)?,
                    jvm_arguments: row.get(9)?,
                })
            })
            .map_err(|e| LauncherError::Database(e.to_string()))?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r.map_err(|e| LauncherError::Database(e.to_string()))?);
        }

        Ok(list)
    }

    /// PERFORMANS İYİLEŞTİRMESİ: O(1) doğrudan ID ile indeksli profil sorgusu.
    /// Tüm profilleri deserialize edip belleğe alma maliyetini ortadan kaldırır.
    pub fn get_instance(&self, id: &str) -> LauncherResult<Option<InstanceSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, directory_name, minecraft_version, loader_type, loader_version, allocated_memory_mb, created_at, last_played_at, jvm_arguments
                 FROM instances WHERE id = ?1 LIMIT 1",
            )
            .map_err(|e| LauncherError::Database(e.to_string()))?;

        let mut rows = stmt
            .query_map(params![id], |row| {
                let loader_str: String = row.get(4)?;
                let loader_type = loader_str.parse::<LoaderType>().unwrap_or(LoaderType::Vanilla);

                Ok(InstanceSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    directory_name: row.get(2)?,
                    minecraft_version: row.get(3)?,
                    loader_type,
                    loader_version: row.get(5)?,
                    allocated_memory_mb: row.get(6)?,
                    created_at: row.get(7)?,
                    last_played_at: row.get(8)?,
                    jvm_arguments: row.get(9)?,
                })
            })
            .map_err(|e| LauncherError::Database(e.to_string()))?;

        if let Some(res) = rows.next() {
            Ok(Some(res.map_err(|e| LauncherError::Database(e.to_string()))?))
        } else {
            Ok(None)
        }
    }

    pub fn delete_instance(&self, id: &str) -> LauncherResult<bool> {
        let conn = self.conn.lock().unwrap();
        let affected = conn
            .execute("DELETE FROM instances WHERE id = ?1", params![id])
            .map_err(|e| LauncherError::Database(e.to_string()))?;
        Ok(affected > 0)
    }

    pub fn update_instance_settings(
        &self,
        id: &str,
        name: Option<&str>,
        memory_mb: Option<u32>,
        jvm_arguments: Option<&str>,
    ) -> LauncherResult<bool> {
        let conn = self.conn.lock().unwrap();
        if let Some(n) = name {
            conn.execute("UPDATE instances SET name = ?1 WHERE id = ?2", params![n, id])
                .map_err(|e| LauncherError::Database(e.to_string()))?;
        }
        if let Some(m) = memory_mb {
            conn.execute("UPDATE instances SET allocated_memory_mb = ?1 WHERE id = ?2", params![m, id])
                .map_err(|e| LauncherError::Database(e.to_string()))?;
        }
        if let Some(jvm) = jvm_arguments {
            conn.execute("UPDATE instances SET jvm_arguments = ?1 WHERE id = ?2", params![jvm, id])
                .map_err(|e| LauncherError::Database(e.to_string()))?;
        }
        Ok(true)
    }
}

fn sanitize_dir_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

fn chrono_like_now() -> String {
    "2026-10-28 23:30:00".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrations_and_instance_crud() {
        let db = Database::open_in_memory().unwrap();

        let instances = db.list_instances().unwrap();
        assert_eq!(instances.len(), 0);

        let created = db
            .create_instance(CreateInstanceRequest {
                name: "Survival 1.21".to_string(),
                minecraft_version: "1.21.1".to_string(),
                loader_type: LoaderType::Fabric,
                loader_version: Some("0.16.5".to_string()),
                allocated_memory_mb: Some(6144),
            })
            .unwrap();

        assert_eq!(created.name, "Survival 1.21");
        assert_eq!(created.minecraft_version, "1.21.1");
        assert_eq!(created.allocated_memory_mb, 6144);

        let list = db.list_instances().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, created.id);

        let deleted = db.delete_instance(&created.id).unwrap();
        assert!(deleted);

        let list_after = db.list_instances().unwrap();
        assert_eq!(list_after.len(), 0);
    }
}


use crate::database::Pool;
use rusqlite::params;

pub struct UploadCacheModel {
    pool: Pool,
}

#[derive(Debug, Clone)]
pub struct UploadCache {
    pub id: String,
    pub user_id: String,
    pub file_path: String,
    pub temp_file_path: String,
}

impl UploadCacheModel {
    pub fn new(pool: &Pool) -> Self {
        UploadCacheModel { pool: pool.clone() }
    }

    pub fn create(&self, id: &str, user_id: &str, file_path: &str, temp_file_path: &str) -> Result<(), String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        conn.execute(
            "INSERT INTO upload_cache (id, user_id, file_path, temp_file_path) VALUES (?1, ?2, ?3, ?4)",
            params![id, user_id, file_path, temp_file_path],
        )
        .map_err(|e| {
            if e.to_string().contains("UNIQUE constraint failed") {
                "FILE_ALREADY_EXISTS".to_string()
            } else {
                e.to_string()
            }
        })?;

        Ok(())
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<UploadCache>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let mut stmt = conn
            .prepare("SELECT id, user_id, file_path, temp_file_path FROM upload_cache WHERE id = ?1")
            .map_err(|e| e.to_string())?;

        let result = stmt.query_row(params![id], |row| {
            Ok(UploadCache {
                id: row.get(0)?,
                user_id: row.get(1)?,
                file_path: row.get(2)?,
                temp_file_path: row.get(3)?,
            })
        });

        match result {
            Ok(cache) => Ok(Some(cache)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn update_last_updated(&self, id: &str) -> Result<(), String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        conn.execute(
            "UPDATE upload_cache SET last_updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
            params![id],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        conn.execute("DELETE FROM upload_cache WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn file_path_exists(&self, user_id: &str, file_path: &str) -> Result<bool, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM upload_cache WHERE user_id = ?1 AND file_path = ?2",
                params![user_id, file_path],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        Ok(count > 0)
    }

    pub fn get_expired_uploads(&self, timeout_seconds: i64) -> Result<Vec<UploadCache>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let mut stmt = conn.prepare(
            "SELECT id, user_id, file_path, temp_file_path FROM upload_cache WHERE datetime(last_updated_at) < datetime('now', ?1 || ' seconds')"
        ).map_err(|e| e.to_string())?;

        let caches = stmt
            .query_map(params![format!("-{}", timeout_seconds)], |row| {
                Ok(UploadCache {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    file_path: row.get(2)?,
                    temp_file_path: row.get(3)?,
                })
            })
            .map_err(|e| e.to_string())?;

        let mut result = Vec::new();
        for cache in caches {
            result.push(cache.map_err(|e| e.to_string())?);
        }

        Ok(result)
    }

}

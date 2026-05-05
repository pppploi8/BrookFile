use crate::database::Pool;
use rusqlite::params;
use uuid::Uuid;

pub struct ShareInfo {
    pub id: String,
    pub user_id: String,
    pub file_path: String,
    pub file_name: String,
    pub is_directory: bool,
    pub share_code: String,
    pub expire_type: String,
    pub expire_at: Option<String>,
    pub max_downloads: Option<i64>,
    pub download_count: i64,
    pub share_mode: String,
    pub password: Option<String>,
    pub created_at: String,
}

fn row_to_share_info(row: &rusqlite::Row) -> rusqlite::Result<ShareInfo> {
    Ok(ShareInfo {
        id: row.get(0)?,
        user_id: row.get(1)?,
        file_path: row.get(2)?,
        file_name: row.get(3)?,
        is_directory: row.get::<_, i64>(4)? != 0,
        share_code: row.get(5)?,
        expire_type: row.get(6)?,
        expire_at: row.get(7)?,
        max_downloads: row.get(8)?,
        download_count: row.get(9)?,
        share_mode: row.get(10)?,
        password: row.get(11)?,
        created_at: row.get(12)?,
    })
}

const SELECT_COLUMNS: &str = "id, user_id, file_path, file_name, is_directory, share_code, expire_type, expire_at, max_downloads, download_count, share_mode, password, created_at";

pub struct ShareModel {
    pool: Pool,
}

impl ShareModel {
    pub fn new(pool: &Pool) -> Self {
        ShareModel { pool: pool.clone() }
    }

    pub fn create(
        &self,
        user_id: &str,
        file_path: &str,
        file_name: &str,
        is_directory: bool,
        share_code: &str,
        expire_type: &str,
        expire_at: Option<&str>,
        max_downloads: Option<i64>,
        share_mode: &str,
        password: Option<&str>,
    ) -> Result<String, String> {
        let id = Uuid::new_v4().to_string();
        let is_dir_int = if is_directory { 1i64 } else { 0i64 };

        let conn = self.pool.get().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO shares (id, user_id, file_path, file_name, is_directory, share_code, expire_type, expire_at, max_downloads, share_mode, password) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![id, user_id, file_path, file_name, is_dir_int, share_code, expire_type, expire_at, max_downloads, share_mode, password],
        ).map_err(|e| e.to_string())?;

        Ok(id)
    }

    pub fn get_by_code(&self, share_code: &str) -> Result<Option<ShareInfo>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let result: Result<ShareInfo, _> = conn.query_row(
            &format!(
                "SELECT {} FROM shares WHERE share_code = ?1",
                SELECT_COLUMNS
            ),
            params![share_code],
            row_to_share_info,
        );

        match result {
            Ok(info) => Ok(Some(info)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn get_by_path(&self, user_id: &str, file_path: &str) -> Result<Option<ShareInfo>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let result: Result<ShareInfo, _> = conn.query_row(
            &format!(
                "SELECT {} FROM shares WHERE user_id = ?1 AND file_path = ?2 ORDER BY created_at DESC LIMIT 1",
                SELECT_COLUMNS
            ),
            params![user_id, file_path],
            row_to_share_info,
        );

        match result {
            Ok(info) => Ok(Some(info)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn list_by_user(&self, user_id: &str) -> Result<Vec<ShareInfo>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} FROM shares WHERE user_id = ?1 ORDER BY created_at DESC",
                SELECT_COLUMNS
            ))
            .map_err(|e| e.to_string())?;

        let shares = stmt
            .query_map(params![user_id], row_to_share_info)
            .map_err(|e| e.to_string())?;

        shares
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    pub fn delete_by_ids(&self, user_id: &str, ids: &[String]) -> Result<(), String> {
        if ids.is_empty() {
            return Ok(());
        }

        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let placeholders: Vec<String> = ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 2))
            .collect();
        let sql = format!(
            "DELETE FROM shares WHERE user_id = ?1 AND id IN ({})",
            placeholders.join(", ")
        );
        let mut param_values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        param_values.push(Box::new(user_id.to_string()));
        for id in ids {
            param_values.push(Box::new(id.clone()));
        }
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            param_values.iter().map(|v| v.as_ref()).collect();

        conn.execute(&sql, params_refs.as_slice())
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn increment_download_count(&self, share_code: &str) -> Result<bool, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let rows = conn.execute(
            "UPDATE shares SET download_count = download_count + 1 WHERE share_code = ?1 AND (max_downloads IS NULL OR download_count < max_downloads)",
            params![share_code],
        ).map_err(|e| e.to_string())?;
        Ok(rows > 0)
    }

    pub fn cleanup_expired(&self, older_than_days: i64) -> Result<u64, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let affected = conn
            .execute(
                "DELETE FROM shares WHERE (
                    (expire_type = 'time' AND datetime(expire_at) < datetime('now'))
                    OR (expire_type = 'count' AND max_downloads IS NOT NULL AND download_count >= max_downloads)
                ) AND datetime(updated_at) < datetime('now', ?1 || ' days')",
                params![format!("-{}", older_than_days)],
            )
            .map_err(|e| e.to_string())?;
        Ok(affected as u64)
    }

    pub fn has_shares_by_user(&self, user_id: &str) -> Result<bool, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM shares WHERE user_id = ?1",
                params![user_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        Ok(count > 0)
    }
}

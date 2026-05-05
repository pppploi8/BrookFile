use crate::database::Pool;
use rusqlite::params;

pub struct RecycleBinItem {
    pub id: String,
    #[allow(dead_code)]
    pub user_id: String,
    pub original_path: String,
    pub original_name: String,
    pub is_directory: bool,
    pub file_size: i64,
    pub deleted_at: String,
}

pub struct RecycleBinModel {
    pool: Pool,
}

impl RecycleBinModel {
    pub fn new(pool: &Pool) -> Self {
        RecycleBinModel { pool: pool.clone() }
    }

    pub fn insert(
        &self,
        id: &str,
        user_id: &str,
        original_path: &str,
        original_name: &str,
        is_directory: bool,
        file_size: i64,
    ) -> Result<(), String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let is_dir_int = if is_directory { 1 } else { 0 };
        conn.execute(
            "INSERT INTO recycle_bin (id, user_id, original_path, original_name, is_directory, file_size) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, user_id, original_path, original_name, is_dir_int, file_size],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn list(
        &self,
        user_id: &str,
        page: i64,
        page_size: i64,
    ) -> Result<(Vec<RecycleBinItem>, i64), String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let total: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM recycle_bin WHERE user_id = ?1",
                params![user_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        let offset = page.saturating_sub(1) * page_size;
        let mut stmt = conn.prepare(
            "SELECT id, user_id, original_path, original_name, is_directory, file_size, deleted_at FROM recycle_bin WHERE user_id = ?1 ORDER BY deleted_at DESC LIMIT ?2 OFFSET ?3"
        ).map_err(|e| e.to_string())?;

        let items = stmt
            .query_map(params![user_id, page_size, offset], |row| {
                Ok(RecycleBinItem {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    original_path: row.get(2)?,
                    original_name: row.get(3)?,
                    is_directory: row.get::<_, i64>(4)? != 0,
                    file_size: row.get(5)?,
                    deleted_at: row.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?;

        let items: Vec<RecycleBinItem> = items
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok((items, total))
    }

    pub fn get_by_id(&self, id: &str, user_id: &str) -> Result<Option<RecycleBinItem>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare(
            "SELECT id, user_id, original_path, original_name, is_directory, file_size, deleted_at FROM recycle_bin WHERE id = ?1 AND user_id = ?2"
        ).map_err(|e| e.to_string())?;

        let result: Result<RecycleBinItem, _> = stmt.query_row(params![id, user_id], |row| {
            Ok(RecycleBinItem {
                id: row.get(0)?,
                user_id: row.get(1)?,
                original_path: row.get(2)?,
                original_name: row.get(3)?,
                is_directory: row.get::<_, i64>(4)? != 0,
                file_size: row.get(5)?,
                deleted_at: row.get(6)?,
            })
        });

        match result {
            Ok(item) => Ok(Some(item)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn get_by_ids(&self, ids: &[String], user_id: &str) -> Result<Vec<RecycleBinItem>, String> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let placeholders: Vec<String> = ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 2))
            .collect();
        let sql = format!(
            "SELECT id, user_id, original_path, original_name, is_directory, file_size, deleted_at FROM recycle_bin WHERE user_id = ?1 AND id IN ({})",
            placeholders.join(", ")
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let mut param_values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        param_values.push(Box::new(user_id.to_string()));
        for id in ids {
            param_values.push(Box::new(id.clone()));
        }
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            param_values.iter().map(|v| v.as_ref()).collect();

        let items = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok(RecycleBinItem {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    original_path: row.get(2)?,
                    original_name: row.get(3)?,
                    is_directory: row.get::<_, i64>(4)? != 0,
                    file_size: row.get(5)?,
                    deleted_at: row.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?;

        let items: Vec<RecycleBinItem> = items
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(items)
    }

    pub fn delete_by_id(&self, id: &str, user_id: &str) -> Result<(), String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM recycle_bin WHERE id = ?1 AND user_id = ?2",
            params![id, user_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}

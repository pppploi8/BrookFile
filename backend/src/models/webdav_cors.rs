use crate::database::Pool;
use rusqlite::params;
use std::collections::HashSet;
use uuid::Uuid;

pub struct WebDavCorsModel {
    pool: Pool,
}

impl WebDavCorsModel {
    pub fn new(pool: &Pool) -> Self {
        WebDavCorsModel { pool: pool.clone() }
    }

    pub fn list_by_user(&self, user_id: &str) -> Result<Vec<String>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT origin FROM webdav_cors WHERE user_id = ?1 ORDER BY created_at")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![user_id], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        let mut origins = Vec::new();
        for row in rows {
            origins.push(row.map_err(|e| e.to_string())?);
        }
        Ok(origins)
    }

    pub fn save(&self, user_id: &str, origins: &[String]) -> Result<(), String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM webdav_cors WHERE user_id = ?1", params![user_id])
            .map_err(|e| e.to_string())?;
        for origin in origins {
            let id = Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO webdav_cors (id, user_id, origin) VALUES (?1, ?2, ?3)",
                params![id, user_id, origin],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn all_origins(&self) -> Result<HashSet<String>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT DISTINCT origin FROM webdav_cors")
            .map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |row| row.get(0)).map_err(|e| e.to_string())?;
        let mut origins = HashSet::new();
        for row in rows {
            origins.insert(row.map_err(|e| e.to_string())?);
        }
        Ok(origins)
    }
}

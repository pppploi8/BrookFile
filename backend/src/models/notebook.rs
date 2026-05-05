use crate::database::Pool;
use rusqlite::params;
use uuid::Uuid;

pub struct NotebookModel {
    pub pool: Pool,
}

pub struct NotebookInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub path: String,
    pub encrypted: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl NotebookModel {
    pub fn new(pool: &Pool) -> Self {
        NotebookModel { pool: pool.clone() }
    }

    pub fn list_by_user(&self, user_id: &str) -> Result<Vec<NotebookInfo>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let mut stmt = conn
            .prepare(
                "SELECT id, name, description, path, encrypted, created_at, updated_at \
             FROM notebooks WHERE user_id = ?1 ORDER BY name ASC",
            )
            .map_err(|e| e.to_string())?;

        let notebooks = stmt
            .query_map(params![user_id], |row| {
                Ok(NotebookInfo {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    path: row.get(3)?,
                    encrypted: row.get::<_, i64>(4)? != 0,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?;

        notebooks
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    pub fn get_by_id_and_user(
        &self,
        notebook_id: &str,
        user_id: &str,
    ) -> Result<Option<NotebookInfo>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let result: Result<NotebookInfo, _> = conn.query_row(
            "SELECT id, name, description, path, encrypted, created_at, updated_at \
             FROM notebooks WHERE id = ?1 AND user_id = ?2",
            params![notebook_id, user_id],
            |row| {
                Ok(NotebookInfo {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    path: row.get(3)?,
                    encrypted: row.get::<_, i64>(4)? != 0,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        );

        match result {
            Ok(notebook) => Ok(Some(notebook)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn get_by_user_and_path(
        &self,
        user_id: &str,
        path: &str,
    ) -> Result<Option<NotebookInfo>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let result: Result<NotebookInfo, _> = conn.query_row(
            "SELECT id, name, description, path, encrypted, created_at, updated_at \
             FROM notebooks WHERE user_id = ?1 AND path = ?2",
            params![user_id, path],
            |row| {
                Ok(NotebookInfo {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    path: row.get(3)?,
                    encrypted: row.get::<_, i64>(4)? != 0,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        );

        match result {
            Ok(notebook) => Ok(Some(notebook)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn create(
        &self,
        user_id: &str,
        name: &str,
        description: &str,
        path: &str,
        encrypted: bool,
    ) -> Result<String, String> {
        let notebook_id = Uuid::new_v4().to_string();
        let encrypted_int: i64 = if encrypted { 1 } else { 0 };

        let conn = self.pool.get().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO notebooks (id, user_id, name, description, path, encrypted) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![notebook_id, user_id, name, description, path, encrypted_int],
        )
        .map_err(|e| e.to_string())?;

        Ok(notebook_id)
    }

    pub fn update(
        &self,
        notebook_id: &str,
        user_id: &str,
        name: &str,
        description: &str,
    ) -> Result<bool, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let exists = match conn
            .query_row(
                "SELECT 1 FROM notebooks WHERE id = ?1 AND user_id = ?2",
                params![notebook_id, user_id],
                |_| Ok(true),
            ) {
                Ok(true) => true,
                Err(rusqlite::Error::QueryReturnedNoRows) => false,
                Err(e) => return Err(e.to_string()),
                Ok(_) => false,
            };

        if !exists {
            return Ok(false);
        }

        conn.execute(
            "UPDATE notebooks SET name = ?1, description = ?2, updated_at = datetime('now') \
             WHERE id = ?3 AND user_id = ?4",
            params![name, description, notebook_id, user_id],
        )
        .map_err(|e| e.to_string())?;

        Ok(true)
    }

    pub fn delete(&self, notebook_id: &str, user_id: &str) -> Result<bool, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let exists = match conn
            .query_row(
                "SELECT 1 FROM notebooks WHERE id = ?1 AND user_id = ?2",
                params![notebook_id, user_id],
                |_| Ok(true),
            ) {
                Ok(true) => true,
                Err(rusqlite::Error::QueryReturnedNoRows) => false,
                Err(e) => return Err(e.to_string()),
                Ok(_) => false,
            };

        if !exists {
            return Ok(false);
        }

        conn.execute(
            "DELETE FROM notebooks WHERE id = ?1 AND user_id = ?2",
            params![notebook_id, user_id],
        )
        .map_err(|e| e.to_string())?;

        Ok(true)
    }

    pub fn list_encrypted_by_user(&self, user_id: &str) -> Result<Vec<NotebookInfo>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let mut stmt = conn
            .prepare(
                "SELECT id, name, description, path, encrypted, created_at, updated_at \
             FROM notebooks WHERE user_id = ?1 AND encrypted = 1",
            )
            .map_err(|e| e.to_string())?;

        let notebooks = stmt
            .query_map(params![user_id], |row| {
                Ok(NotebookInfo {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    path: row.get(3)?,
                    encrypted: row.get::<_, i64>(4)? != 0,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?;

        notebooks
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    pub fn list_all_non_encrypted(&self) -> Result<Vec<(String, NotebookInfo)>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let mut stmt = conn
            .prepare(
                "SELECT user_id, id, name, description, path, encrypted, created_at, updated_at \
             FROM notebooks WHERE encrypted = 0",
            )
            .map_err(|e| e.to_string())?;

        let notebooks = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    NotebookInfo {
                        id: row.get(1)?,
                        name: row.get(2)?,
                        description: row.get(3)?,
                        path: row.get(4)?,
                        encrypted: row.get::<_, i64>(5)? != 0,
                        created_at: row.get(6)?,
                        updated_at: row.get(7)?,
                    },
                ))
            })
            .map_err(|e| e.to_string())?;

        notebooks
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
}

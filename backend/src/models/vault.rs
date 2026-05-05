use crate::database::Pool;
use rusqlite::params;
use uuid::Uuid;

pub struct VaultModel {
    pool: Pool,
}

pub struct VaultInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub path: String,
    pub filename: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

pub struct CreateVaultData {
    pub name: String,
    pub description: String,
    pub path: String,
    pub filename: String,
}

pub struct ImportVaultData {
    pub name: String,
    pub description: String,
    pub path: String,
    pub filename: String,
}

impl VaultModel {
    pub fn new(pool: &Pool) -> Self {
        VaultModel { pool: pool.clone() }
    }

    pub fn list_by_user(&self, user_id: &str) -> Result<Vec<VaultInfo>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let mut stmt = conn
            .prepare(
                "SELECT id, name, description, path, filename, created_at, updated_at 
             FROM vaults WHERE user_id = ?1 ORDER BY created_at",
            )
            .map_err(|e| e.to_string())?;

        let vaults = stmt
            .query_map(params![user_id], |row| {
                Ok(VaultInfo {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    path: row.get(3)?,
                    filename: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?;

        vaults
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    pub fn get_by_id(&self, vault_id: &str, user_id: &str) -> Result<Option<VaultInfo>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let result: Result<VaultInfo, _> = conn.query_row(
            "SELECT id, name, description, path, filename, created_at, updated_at 
             FROM vaults WHERE id = ?1 AND user_id = ?2",
            params![vault_id, user_id],
            |row| {
                Ok(VaultInfo {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    path: row.get(3)?,
                    filename: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        );

        match result {
            Ok(vault) => Ok(Some(vault)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn create(&self, user_id: &str, data: &CreateVaultData) -> Result<String, String> {
        let vault_id = Uuid::new_v4().to_string();

        let conn = self.pool.get().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO vaults (id, user_id, name, description, path, filename) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                vault_id,
                user_id,
                data.name,
                data.description,
                data.path,
                data.filename
            ],
        )
        .map_err(|e| e.to_string())?;

        Ok(vault_id)
    }

    pub fn update(
        &self,
        vault_id: &str,
        user_id: &str,
        name: &Option<String>,
        description: &Option<String>,
    ) -> Result<bool, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let result = conn.query_row(
            "SELECT 1 FROM vaults WHERE id = ?1 AND user_id = ?2",
            params![vault_id, user_id],
            |_| Ok(true),
        );

        match result {
            Ok(true) => {}
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(false),
            Err(e) => return Err(e.to_string()),
            _ => return Ok(false),
        }

        match (name, description) {
            (Some(n), Some(d)) => {
                conn.execute(
                    "UPDATE vaults SET name = ?1, description = ?2, updated_at = datetime('now') 
                     WHERE id = ?3 AND user_id = ?4",
                    params![n, d, vault_id, user_id],
                )
                .map_err(|e| e.to_string())?;
            }
            (Some(n), None) => {
                conn.execute(
                    "UPDATE vaults SET name = ?1, updated_at = datetime('now') 
                     WHERE id = ?2 AND user_id = ?3",
                    params![n, vault_id, user_id],
                )
                .map_err(|e| e.to_string())?;
            }
            (None, Some(d)) => {
                conn.execute(
                    "UPDATE vaults SET description = ?1, updated_at = datetime('now') 
                     WHERE id = ?2 AND user_id = ?3",
                    params![d, vault_id, user_id],
                )
                .map_err(|e| e.to_string())?;
            }
            (None, None) => {
                conn.execute(
                    "UPDATE vaults SET updated_at = datetime('now') 
                     WHERE id = ?1 AND user_id = ?2",
                    params![vault_id, user_id],
                )
                .map_err(|e| e.to_string())?;
            }
        }

        Ok(true)
    }

    pub fn delete(&self, vault_id: &str, user_id: &str) -> Result<bool, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let result = conn.query_row(
            "SELECT 1 FROM vaults WHERE id = ?1 AND user_id = ?2",
            params![vault_id, user_id],
            |_| Ok(true),
        );

        match result {
            Ok(true) => {}
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(false),
            Err(e) => return Err(e.to_string()),
            _ => return Ok(false),
        }

        conn.execute(
            "DELETE FROM vaults WHERE id = ?1 AND user_id = ?2",
            params![vault_id, user_id],
        )
        .map_err(|e| e.to_string())?;

        Ok(true)
    }

    pub fn is_file_imported(
        &self,
        user_id: &str,
        path: &str,
        filename: &str,
    ) -> Result<bool, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let result = conn.query_row(
            "SELECT 1 FROM vaults WHERE user_id = ?1 AND path = ?2 AND filename = ?3",
            params![user_id, path, filename],
            |_| Ok(true),
        );

        match result {
            Ok(true) => Ok(true),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(e.to_string()),
            _ => Ok(false),
        }
    }

    pub fn import(&self, user_id: &str, data: &ImportVaultData) -> Result<String, String> {
        let vault_id = Uuid::new_v4().to_string();

        let conn = self.pool.get().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO vaults (id, user_id, name, description, path, filename) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                vault_id,
                user_id,
                data.name,
                data.description,
                data.path,
                data.filename
            ],
        )
        .map_err(|e| e.to_string())?;

        Ok(vault_id)
    }

}

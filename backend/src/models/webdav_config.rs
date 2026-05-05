use crate::database::Pool;
use hmac::{Hmac, Mac};
use md5::Md5;
use rand::Rng;
use rusqlite::params;
use sha2::{Digest, Sha256};
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

pub struct WebDavConfigInfo {
    pub id: String,
    #[allow(dead_code)]
    pub user_id: String,
    pub dav_path: String,
    pub access_path: String,
    pub password: String,
    pub password_salt: String,
    pub permission: String,
    pub global_access: bool,
    pub created_at: String,
    pub updated_at: String,
    pub digest_ha1: Option<String>,
}

fn row_to_config(row: &rusqlite::Row) -> rusqlite::Result<WebDavConfigInfo> {
    Ok(WebDavConfigInfo {
        id: row.get(0)?,
        user_id: row.get(1)?,
        dav_path: row.get(2)?,
        access_path: row.get(3)?,
        password: row.get(4)?,
        password_salt: row.get(5)?,
        permission: row.get(6)?,
        global_access: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        digest_ha1: row.get(10)?,
    })
}

const SELECT_COLUMNS: &str = "id, user_id, dav_path, access_path, password, password_salt, permission, global_access, created_at, updated_at, digest_ha1";

fn compute_digest_ha1(username: &str, realm: &str, password: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(format!("{}:{}:{}", username, realm, password).as_bytes());
    hex::encode(hasher.finalize())
}

pub struct WebDavConfigModel {
    pool: Pool,
}

impl WebDavConfigModel {
    pub fn new(pool: &Pool) -> Self {
        WebDavConfigModel { pool: pool.clone() }
    }

    fn get_username_by_user_id(&self, user_id: &str) -> Result<String, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT username FROM users WHERE id = ?1",
            params![user_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())
    }

    pub fn list_by_user(&self, user_id: &str) -> Result<Vec<WebDavConfigInfo>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} FROM webdav_configs WHERE user_id = ?1 ORDER BY created_at ASC",
                SELECT_COLUMNS
            ))
            .map_err(|e| e.to_string())?;
        let configs = stmt
            .query_map(params![user_id], row_to_config)
            .map_err(|e| e.to_string())?;
        configs
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    pub fn get_by_id(&self, user_id: &str, id: &str) -> Result<Option<WebDavConfigInfo>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let result: Result<WebDavConfigInfo, _> = conn.query_row(
            &format!(
                "SELECT {} FROM webdav_configs WHERE id = ?1 AND user_id = ?2",
                SELECT_COLUMNS
            ),
            params![id, user_id],
            row_to_config,
        );
        match result {
            Ok(info) => Ok(Some(info)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn get_by_dav_path(
        &self,
        user_id: &str,
        dav_path: &str,
    ) -> Result<Option<WebDavConfigInfo>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let result: Result<WebDavConfigInfo, _> = conn.query_row(
            &format!(
                "SELECT {} FROM webdav_configs WHERE user_id = ?1 AND dav_path = ?2",
                SELECT_COLUMNS
            ),
            params![user_id, dav_path],
            row_to_config,
        );
        match result {
            Ok(info) => Ok(Some(info)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn create(
        &self,
        user_id: &str,
        dav_path: &str,
        access_path: &str,
        password: &str,
        permission: &str,
        global_access: bool,
    ) -> Result<String, String> {
        let id = Uuid::new_v4().to_string();
        let salt: String = rand::thread_rng()
            .sample_iter(rand::distributions::Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();
        let mut mac = HmacSha256::new_from_slice(salt.as_bytes()).map_err(|e| e.to_string())?;
        mac.update(password.as_bytes());
        let hash = hex::encode(mac.finalize().into_bytes());

        let username = self.get_username_by_user_id(user_id)?;
        let digest_ha1 = compute_digest_ha1(&username, "WebDAV", password);

        let conn = self.pool.get().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO webdav_configs (id, user_id, dav_path, access_path, password, password_salt, permission, global_access, digest_ha1) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![id, user_id, dav_path, access_path, hash, salt, permission, global_access, digest_ha1],
        ).map_err(|e| {
            if e.to_string().contains("UNIQUE constraint failed") {
                "DAV_PATH_DUPLICATE".to_string()
            } else {
                e.to_string()
            }
        })?;

        Ok(id)
    }

    pub fn update(
        &self,
        user_id: &str,
        id: &str,
        dav_path: &str,
        access_path: &str,
        password: Option<&str>,
        permission: &str,
        global_access: bool,
    ) -> Result<(), String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        if let Some(pwd) = password {
            if !pwd.is_empty() {
                let salt: String = rand::thread_rng()
                    .sample_iter(rand::distributions::Alphanumeric)
                    .take(32)
                    .map(char::from)
                    .collect();
                let mut mac =
                    HmacSha256::new_from_slice(salt.as_bytes()).map_err(|e| e.to_string())?;
                mac.update(pwd.as_bytes());
                let hash = hex::encode(mac.finalize().into_bytes());

                let username = self.get_username_by_user_id(user_id)?;
                let digest_ha1 = compute_digest_ha1(&username, "WebDAV", pwd);

                conn.execute(
                    "UPDATE webdav_configs SET dav_path = ?1, access_path = ?2, password = ?3, password_salt = ?4, permission = ?5, global_access = ?6, digest_ha1 = ?7, updated_at = datetime('now') WHERE id = ?8 AND user_id = ?9",
                    params![dav_path, access_path, hash, salt, permission, global_access, digest_ha1, id, user_id],
                ).map_err(|e| {
                    if e.to_string().contains("UNIQUE constraint failed") {
                        "DAV_PATH_DUPLICATE".to_string()
                    } else {
                        e.to_string()
                    }
                })?;
                return Ok(());
            }
        }

        conn.execute(
            "UPDATE webdav_configs SET dav_path = ?1, access_path = ?2, permission = ?3, global_access = ?4, updated_at = datetime('now') WHERE id = ?5 AND user_id = ?6",
            params![dav_path, access_path, permission, global_access, id, user_id],
        ).map_err(|e| {
            if e.to_string().contains("UNIQUE constraint failed") {
                "DAV_PATH_DUPLICATE".to_string()
            } else {
                e.to_string()
            }
        })?;

        Ok(())
    }

    pub fn has_global_config(
        &self,
        user_id: &str,
        exclude_id: Option<&str>,
    ) -> Result<bool, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let count: i64 = match exclude_id {
            Some(eid) => conn
                .query_row(
                    "SELECT COUNT(*) FROM webdav_configs WHERE user_id = ?1 AND global_access = 1 AND id != ?2",
                    params![user_id, eid],
                    |row| row.get(0),
                ),
            None => conn
                .query_row(
                    "SELECT COUNT(*) FROM webdav_configs WHERE user_id = ?1 AND global_access = 1",
                    params![user_id],
                    |row| row.get(0),
                ),
        }
        .map_err(|e| e.to_string())?;
        Ok(count > 0)
    }

    pub fn count_by_user(&self, user_id: &str, exclude_id: Option<&str>) -> Result<i64, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let count: i64 = match exclude_id {
            Some(eid) => conn.query_row(
                "SELECT COUNT(*) FROM webdav_configs WHERE user_id = ?1 AND id != ?2",
                params![user_id, eid],
                |row| row.get(0),
            ),
            None => conn.query_row(
                "SELECT COUNT(*) FROM webdav_configs WHERE user_id = ?1",
                params![user_id],
                |row| row.get(0),
            ),
        }
        .map_err(|e| e.to_string())?;
        Ok(count)
    }

    pub fn delete(&self, user_id: &str, id: &str) -> Result<bool, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let affected = conn
            .execute(
                "DELETE FROM webdav_configs WHERE id = ?1 AND user_id = ?2",
                params![id, user_id],
            )
            .map_err(|e| e.to_string())?;
        Ok(affected > 0)
    }

    #[allow(dead_code)]
    pub fn delete_by_user(&self, user_id: &str) -> Result<(), String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM webdav_configs WHERE user_id = ?1",
            params![user_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn verify_password(&self, config: &WebDavConfigInfo, password: &str) -> bool {
        let mut mac = match HmacSha256::new_from_slice(config.password_salt.as_bytes()) {
            Ok(m) => m,
            Err(_) => return false,
        };
        mac.update(password.as_bytes());
        let result = mac.finalize();
        let expected = match hex::decode(&config.password) {
            Ok(b) => b,
            Err(_) => return false,
        };
        let result_bytes = result.into_bytes();
        if result_bytes.len() != expected.len() {
            return false;
        }
        let mut diff = 0u8;
        for (a, b) in result_bytes.iter().zip(expected.iter()) {
            diff |= a ^ b;
        }
        diff == 0
    }
}

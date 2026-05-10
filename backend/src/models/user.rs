use crate::database::Pool;
use crate::models::{BackupRuleModel, NotebookModel, RecycleBinModel, VaultModel, WebDavConfigModel};
use crate::session_manager::SessionManager;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use argon2::{PasswordHash, PasswordHasher, PasswordVerifier};
use rusqlite::params;
use std::path::Path;
use uuid::Uuid;

pub struct UserModel {
    pool: Pool,
}

pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub root_path: Option<String>,
    pub recycle_bin_path: Option<String>,
    pub is_admin: bool,
    pub expire_at: Option<String>,
    pub remark: Option<String>,
    pub feature_order: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl UserModel {
    pub fn new(pool: &Pool) -> Self {
        UserModel { pool: pool.clone() }
    }

    pub fn create_user(
        &self,
        username: &str,
        password: &str,
        root_path: &str,
        recycle_bin_path: Option<&str>,
        is_admin: bool,
    ) -> Result<(), String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let password_hash = self.hash_password(password)?;
        let is_admin_int = if is_admin { 1 } else { 0 };
        let user_id = Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO users (id, username, password_hash, password_salt, root_path, recycle_bin_path, is_admin) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![user_id, username, password_hash, "", root_path, recycle_bin_path, is_admin_int],
        ).map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn create_user_full(
        &self,
        username: &str,
        password: &str,
        root_path: &str,
        recycle_bin_path: Option<&str>,
        is_admin: bool,
        expire_at: Option<&str>,
        remark: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let password_hash = self.hash_password(password)?;
        let is_admin_int = if is_admin { 1 } else { 0 };
        let user_id = Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO users (id, username, password_hash, password_salt, root_path, recycle_bin_path, is_admin, expire_at, remark) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![user_id, username, password_hash, "", root_path, recycle_bin_path, is_admin_int, expire_at, remark],
        ).map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn verify_password(&self, username: &str, password: &str) -> Result<bool, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let result: Result<String, _> = conn
            .query_row(
                "SELECT password_hash FROM users WHERE username = ?1",
                params![username],
                |row| row.get(0),
            );

        match result {
            Ok(stored_hash) => self.verify_password_hash(password, &stored_hash),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                let _ = self.verify_password_hash(password, "$argon2id$v=19$m=19456,t=2,p=1$dummysalt$dummyhashdummyhashdummyhashdummyhashdummy");
                Ok(false)
            }
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn get_user_by_username(&self, username: &str) -> Result<Option<UserInfo>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let mut stmt = conn.prepare(
            "SELECT id, username, root_path, recycle_bin_path, is_admin, expire_at, remark, feature_order, created_at, updated_at FROM users WHERE username = ?1"
        ).map_err(|e| e.to_string())?;

        let result: Result<UserInfo, _> = stmt.query_row(params![username], |row| {
            Ok(UserInfo {
                id: row.get(0)?,
                username: row.get(1)?,
                root_path: row.get(2)?,
                recycle_bin_path: row.get(3)?,
                is_admin: row.get::<_, i64>(4)? != 0,
                expire_at: row.get(5)?,
                remark: row.get(6)?,
                feature_order: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        });

        match result {
            Ok(user) => Ok(Some(user)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn hash_password(&self, password: &str) -> Result<String, String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = crate::config::Config::global().create_argon2();
        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| e.to_string())?;
        Ok(hash.to_string())
    }

    fn verify_password_hash(&self, password: &str, hash: &str) -> Result<bool, String> {
        let parsed = PasswordHash::new(hash).map_err(|e| e.to_string())?;
        Ok(crate::config::Config::global().create_argon2()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    }

    pub fn get_user_full(&self, user_id: &str) -> Result<Option<UserInfo>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let mut stmt = conn.prepare(
            "SELECT id, username, root_path, recycle_bin_path, is_admin, expire_at, remark, feature_order, created_at, updated_at FROM users WHERE id = ?1"
        ).map_err(|e| e.to_string())?;

        let result: Result<UserInfo, _> = stmt.query_row(params![user_id], |row| {
            Ok(UserInfo {
                id: row.get(0)?,
                username: row.get(1)?,
                root_path: row.get(2)?,
                recycle_bin_path: row.get(3)?,
                is_admin: row.get::<_, i64>(4)? != 0,
                expire_at: row.get(5)?,
                remark: row.get(6)?,
                feature_order: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        });

        match result {
            Ok(user) => Ok(Some(user)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn get_recycle_bin_path(&self, user_id: &str) -> Result<Option<String>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let result: Result<Option<String>, _> = conn.query_row(
            "SELECT recycle_bin_path FROM users WHERE id = ?1",
            params![user_id],
            |row| row.get(0),
        );
        match result {
            Ok(path) => Ok(path.filter(|s| !s.is_empty())),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn update_user(
        &self,
        user_id: &str,
        password: Option<&str>,
        root_path: Option<&str>,
        recycle_bin_path: Option<Option<&str>>,
        is_admin: Option<bool>,
        expire_at: Option<&str>,
        remark: Option<&str>,
    ) -> Result<(bool, bool, bool, bool), String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        let password_updated = password.is_some() && !password.unwrap().is_empty();

        let old_user = tx.query_row(
            "SELECT root_path, is_admin, recycle_bin_path FROM users WHERE id = ?1",
            params![user_id],
            |row| {
                let root_path: Option<String> = row.get(0)?;
                let is_admin: bool = row.get(1)?;
                let recycle_bin_path: Option<String> = row.get(2)?;
                Ok((root_path, is_admin, recycle_bin_path))
            },
        ).map_err(|e| e.to_string())?;

        let root_path_changed = root_path.is_some() && {
            let new_val = if root_path.unwrap().is_empty() { None } else { Some(root_path.unwrap().to_string()) };
            new_val != old_user.0
        };
        let admin_changed = is_admin.is_some() && is_admin.unwrap() != old_user.1;

        let recycle_bin_toggled = recycle_bin_path.is_some() && {
            let old_empty = old_user.2.is_none() || old_user.2.as_deref().unwrap_or("").is_empty();
            let new_val = recycle_bin_path.unwrap();
            let new_empty = new_val.is_none() || new_val.unwrap_or("").is_empty();
            old_empty != new_empty
        };

        if let Some(pwd) = password {
            if !pwd.is_empty() {
                let password_hash = self.hash_password(pwd)?;

                tx.execute(
                    "UPDATE users SET password_hash = ?1, password_salt = ?2, updated_at = datetime('now') WHERE id = ?3",
                    params![password_hash, "", user_id],
                ).map_err(|e| e.to_string())?;
            }
        }

        {
            let mut updates: Vec<String> = Vec::new();
            let mut values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

            if let Some(rp) = root_path {
                updates.push("root_path = ?".to_string());
                if rp.is_empty() {
                    values.push(Box::new(Option::<String>::None));
                } else {
                    values.push(Box::new(rp.to_string()));
                }
            }
            if let Some(rbp) = recycle_bin_path {
                updates.push("recycle_bin_path = ?".to_string());
                match rbp {
                    Some(path) => values.push(Box::new(path.to_string())),
                    None => values.push(Box::new(Option::<String>::None)),
                }
            }
            if let Some(ia) = is_admin {
                updates.push("is_admin = ?".to_string());
                values.push(Box::new(if ia { 1i64 } else { 0i64 }));
            }
            if let Some(ea) = expire_at {
                updates.push("expire_at = ?".to_string());
                if ea.is_empty() {
                    values.push(Box::new(Option::<String>::None));
                } else {
                    values.push(Box::new(ea.to_string()));
                }
            }
            if let Some(rm) = remark {
                updates.push("remark = ?".to_string());
                if rm.is_empty() {
                    values.push(Box::new(Option::<String>::None));
                } else {
                    values.push(Box::new(rm.to_string()));
                }
            }

            if !updates.is_empty() {
                updates.push("updated_at = datetime('now')".to_string());

                let sql = format!("UPDATE users SET {} WHERE id = ?", updates.join(", "));
                values.push(Box::new(user_id.to_string()));

                let params: Vec<&dyn rusqlite::ToSql> = values.iter().map(|v| v.as_ref()).collect();
                tx.execute(&sql, params.as_slice())
                    .map_err(|e| e.to_string())?;
            }
        }

        tx.commit().map_err(|e| e.to_string())?;

        Ok((password_updated, root_path_changed, admin_changed, recycle_bin_toggled))
    }

    pub fn delete_user_by_username(&self, username: &str) -> Result<(), String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM users WHERE username = ?1", params![username])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn delete_user(
        &self,
        user_id: &str,
        _backup_rule_model: &BackupRuleModel,
        recycle_bin_model: &RecycleBinModel,
        _vault_model: &VaultModel,
        _webdav_config_model: &WebDavConfigModel,
        _notebook_model: &NotebookModel,
        session_manager: &SessionManager,
    ) -> Result<(), String> {
        if let Some(rb_path) = self.get_recycle_bin_path(user_id)? {
            let page_size: i64 = 10000;
            let mut page: i64 = 1;
            loop {
                let (items, total) = recycle_bin_model.list(user_id, page, page_size)?;
                for item in &items {
                    let record_dir = Path::new(&rb_path).join(&item.id);
                    if let Err(e) = std::fs::remove_dir_all(&record_dir) {
                        crate::error_logger::log_error("delete_user", &format!("Failed to remove recycle bin dir {}: {}", record_dir.display(), e));
                    }
                }
                if items.len() < page_size as usize || page * page_size >= total {
                    break;
                }
                page += 1;
            }
        }

        let mut conn = self.pool.get().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        tx.execute(
            "DELETE FROM backup_logs WHERE backup_rule_id IN (SELECT id FROM backup_rules WHERE user_id = ?1)",
            params![user_id],
        ).map_err(|e| e.to_string())?;

        tx.execute("DELETE FROM backup_rules WHERE user_id = ?1", params![user_id])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM vaults WHERE user_id = ?1", params![user_id])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM webdav_configs WHERE user_id = ?1", params![user_id])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM notebooks WHERE user_id = ?1", params![user_id])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM shares WHERE user_id = ?1", params![user_id])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM upload_cache WHERE user_id = ?1", params![user_id])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM recycle_bin WHERE user_id = ?1", params![user_id])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM users WHERE id = ?1", params![user_id])
            .map_err(|e| e.to_string())?;

        tx.commit().map_err(|e| e.to_string())?;

        session_manager.invalidate_user_sessions(user_id);

        Ok(())
    }

    pub fn list_users(&self) -> Result<Vec<UserInfo>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let mut stmt = conn.prepare(
            "SELECT id, username, root_path, recycle_bin_path, is_admin, expire_at, remark, feature_order, created_at, updated_at FROM users ORDER BY created_at DESC"
        ).map_err(|e| e.to_string())?;

        let users = stmt
            .query_map([], |row| {
                Ok(UserInfo {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    root_path: row.get(2)?,
                    recycle_bin_path: row.get(3)?,
                    is_admin: row.get::<_, i64>(4)? != 0,
                    expire_at: row.get(5)?,
                    remark: row.get(6)?,
                    feature_order: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            })
            .map_err(|e| e.to_string())?;

        users
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    pub fn verify_password_by_id(&self, user_id: &str, password: &str) -> Result<bool, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let result: Result<String, _> = conn
            .query_row(
                "SELECT password_hash FROM users WHERE id = ?1",
                params![user_id],
                |row| row.get(0),
            );

        match result {
            Ok(stored_hash) => self.verify_password_hash(password, &stored_hash),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn change_password(
        &self,
        user_id: &str,
        new_password: &str,
        session_manager: &SessionManager,
    ) -> Result<(), String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let password_hash = self.hash_password(new_password)?;

        conn.execute(
            "UPDATE users SET password_hash = ?1, password_salt = ?2, updated_at = datetime('now') WHERE id = ?3",
            params![password_hash, "", user_id],
        ).map_err(|e| e.to_string())?;

        session_manager.invalidate_user_sessions(user_id);

        Ok(())
    }

    pub fn update_feature_order(&self, user_id: &str, feature_order: &str) -> Result<(), String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        conn.execute(
            "UPDATE users SET feature_order = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![feature_order, user_id],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }
}

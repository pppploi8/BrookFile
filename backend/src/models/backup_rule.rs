use crate::database::Pool;
use rusqlite::params;
use uuid::Uuid;

const BACKUP_SALT: &[u8] = b"_brookfile_backup_salt_v1_";

#[derive(serde::Serialize)]
pub struct BackupLogItem {
    pub id: String,
    pub rule_id: String,
    pub mode: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub backup_success_count: u64,
    pub backup_fail_count: u64,
    pub cleanup_deleted_count: u64,
    pub fail_reason: Option<String>,
}

pub struct BackupRuleModel {
    pub pool: Pool,
}

pub struct BackupRuleListItem {
    pub id: String,
    pub name: String,
    pub storage_type: String,
    pub local_path: String,
    pub cycle: String,
    pub backup_time: serde_json::Value,
    pub status: String,
    pub next_backup_time: Option<String>,
    pub last_backup_time: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Clone)]
pub struct BackupRuleDetail {
    pub user_id: String,
    pub id: String,
    pub name: String,
    pub storage_type: String,
    pub storage_config: serde_json::Value,
    pub local_path: String,
    pub encrypted: bool,
    pub backup_password: Option<String>,
    pub cycle: String,
    pub backup_time: serde_json::Value,
    pub status: String,
    pub last_backup_time: Option<String>,
    pub created_at: Option<String>,
}

pub struct CreateBackupRuleData {
    pub name: String,
    pub storage_type: String,
    pub storage_config: serde_json::Value,
    pub local_path: String,
    pub encrypted: bool,
    pub backup_password: Option<String>,
    pub cycle: String,
    pub backup_time: serde_json::Value,
}

pub struct UpdateBackupRuleData {
    pub id: String,
    pub name: String,
    pub storage_type: String,
    pub storage_config: serde_json::Value,
    pub local_path: String,
    pub encrypted: bool,
    pub backup_password: Option<String>,
    pub cycle: String,
    pub backup_time: serde_json::Value,
}

fn encrypt_password(password: &str) -> Result<String, String> {
    let argon2 = crate::config::Config::global().create_argon2();
    let mut output = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), BACKUP_SALT, &mut output)
        .map_err(|e| e.to_string())?;
    Ok(hex::encode(output))
}

pub fn encrypt_password_string(password: &str) -> Result<String, String> {
    encrypt_password(password)
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct BackupKdfInfo {
    pub algorithm: String,
    pub version: String,
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32,
    pub output_len: usize,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct BackupInfo {
    pub version: u32,
    pub kdf: BackupKdfInfo,
}

pub fn current_backup_info() -> BackupInfo {
    let config = crate::config::Config::global();
    BackupInfo {
        version: 1,
        kdf: BackupKdfInfo {
            algorithm: "argon2id".to_string(),
            version: "0x13".to_string(),
            m_cost: config.argon2.m_cost,
            t_cost: config.argon2.t_cost,
            p_cost: config.argon2.p_cost,
            output_len: 32,
        },
    }
}

pub fn encrypt_password_with_params(
    password: &str,
    info: &BackupKdfInfo,
) -> Result<String, String> {
    let version = match info.version.as_str() {
        "0x13" => argon2::Version::V0x13,
        _ => return Err("Unsupported Argon2 version".to_string()),
    };
    let params = argon2::Params::new(info.m_cost, info.t_cost, info.p_cost, Some(info.output_len))
        .map_err(|e| e.to_string())?;
    let argon2 = argon2::Argon2::new(argon2::Algorithm::Argon2id, version, params);
    let mut output = vec![0u8; info.output_len];
    argon2
        .hash_password_into(password.as_bytes(), BACKUP_SALT, &mut output)
        .map_err(|e| e.to_string())?;
    Ok(hex::encode(output))
}

impl BackupRuleModel {
    pub fn new(pool: &Pool) -> Self {
        BackupRuleModel { pool: pool.clone() }
    }

    pub fn list_by_user(&self, user_id: &str) -> Result<Vec<BackupRuleListItem>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let mut stmt = conn.prepare(
            "SELECT id, name, storage_type, local_path, cycle, backup_time, last_backup_time, created_at 
             FROM backup_rules WHERE user_id = ?1 ORDER BY created_at"
        ).map_err(|e| e.to_string())?;

        let rules = stmt
            .query_map(params![user_id], |row| {
                let backup_time_str: String = row.get(5)?;
                let backup_time: serde_json::Value =
                    serde_json::from_str(&backup_time_str).unwrap_or(serde_json::json!({}));

                Ok(BackupRuleListItem {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    storage_type: row.get(2)?,
                    local_path: row.get(3)?,
                    cycle: row.get(4)?,
                    backup_time,
                    status: String::new(),
                    next_backup_time: None,
                    last_backup_time: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })
            .map_err(|e| e.to_string())?;

        rules
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    pub fn get_by_id(&self, rule_id: &str) -> Result<Option<BackupRuleDetail>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let mut stmt = conn.prepare(
            "SELECT id, user_id, name, storage_type, storage_config, local_path, encrypted, backup_password, cycle, backup_time, last_backup_time, created_at 
             FROM backup_rules WHERE id = ?1"
        ).map_err(|e| e.to_string())?;

        let result: Result<BackupRuleDetail, _> = stmt.query_row(params![rule_id], |row| {
            let storage_config_str: String = row.get(4)?;
            let backup_time_str: String = row.get(9)?;

            let storage_config: serde_json::Value =
                serde_json::from_str(&storage_config_str).unwrap_or(serde_json::json!({}));

            let backup_time: serde_json::Value =
                serde_json::from_str(&backup_time_str).unwrap_or(serde_json::json!({}));

            Ok(BackupRuleDetail {
                id: row.get(0)?,
                user_id: row.get(1)?,
                name: row.get(2)?,
                storage_type: row.get(3)?,
                storage_config,
                local_path: row.get(5)?,
                encrypted: row.get::<_, i64>(6)? != 0,
                backup_password: row.get(7)?,
                cycle: row.get(8)?,
                backup_time,
                status: String::new(),
                last_backup_time: row.get(10)?,
                created_at: row.get(11)?,
            })
        });

        match result {
            Ok(rule) => Ok(Some(rule)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn create(&self, user_id: &str, req: &CreateBackupRuleData) -> Result<String, String> {
        if req.name.is_empty() {
            return Err("NAME_EMPTY".to_string());
        }
        if req.local_path.is_empty() {
            return Err("LOCAL_PATH_EMPTY".to_string());
        }
        if req.storage_type != "webdav" {
            return Err("INVALID_STORAGE_TYPE".to_string());
        }
        if !["daily", "weekly", "monthly", "yearly"].contains(&req.cycle.as_str()) {
            return Err("INVALID_CYCLE".to_string());
        }

        let storage_config = req.storage_config.clone();

        let backup_password_encrypted = if req.encrypted {
            match &req.backup_password {
                Some(pwd) if !pwd.is_empty() => Some(encrypt_password(pwd)?),
                _ => return Err("INVALID_PARAM".to_string()),
            }
        } else {
            None
        };

        let rule_id = Uuid::new_v4().to_string();
        let storage_config_str =
            serde_json::to_string(&storage_config).map_err(|e| e.to_string())?;
        let backup_time_str = serde_json::to_string(&req.backup_time).map_err(|e| e.to_string())?;
        let encrypted_int = if req.encrypted { 1i64 } else { 0i64 };

        let conn = self.pool.get().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO backup_rules (id, user_id, name, storage_type, storage_config, local_path, encrypted, backup_password, cycle, backup_time) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                rule_id,
                user_id,
                req.name,
                req.storage_type,
                storage_config_str,
                req.local_path,
                encrypted_int,
                backup_password_encrypted,
                req.cycle,
                backup_time_str
            ],
        ).map_err(|e| e.to_string())?;

        Ok(rule_id)
    }

    pub fn update(&self, user_id: &str, req: &UpdateBackupRuleData) -> Result<bool, String> {
        if req.name.is_empty() {
            return Err("NAME_EMPTY".to_string());
        }
        if req.local_path.is_empty() {
            return Err("LOCAL_PATH_EMPTY".to_string());
        }
        if req.storage_type != "webdav" {
            return Err("INVALID_STORAGE_TYPE".to_string());
        }
        if !["daily", "weekly", "monthly", "yearly"].contains(&req.cycle.as_str()) {
            return Err("INVALID_CYCLE".to_string());
        }

        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let exists = match conn.query_row(
            "SELECT 1 FROM backup_rules WHERE id = ?1 AND user_id = ?2",
            params![req.id, user_id],
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

        let mut storage_config = req.storage_config.clone();

        if let Some(obj) = storage_config.as_object_mut() {
            if let Some(password) = obj.get("password").and_then(|p| p.as_str()) {
                if password.is_empty() {
                    let old_config: Option<String> = conn
                        .query_row(
                            "SELECT storage_config FROM backup_rules WHERE id = ?1",
                            params![req.id],
                            |row| row.get(0),
                        )
                        .ok();

                    if let Some(old) = old_config {
                        if let Ok(old_obj) = serde_json::from_str::<serde_json::Value>(&old) {
                            if let Some(old_password) =
                                old_obj.get("password").and_then(|p| p.as_str())
                            {
                                obj.insert("password".to_string(), old_password.to_string().into());
                            }
                        }
                    }
                }
            }
        }

        let storage_config_str =
            serde_json::to_string(&storage_config).map_err(|e| e.to_string())?;

        let backup_password_encrypted = if req.encrypted {
            match &req.backup_password {
                Some(pwd) if !pwd.is_empty() => Some(encrypt_password(pwd)?),
                None => {
                    if let Ok(old_encrypted) = conn.query_row(
                        "SELECT encrypted FROM backup_rules WHERE id = ?1 AND user_id = ?2",
                        params![req.id, user_id],
                        |row| row.get::<_, i64>(0),
                    ) {
                        if old_encrypted == 0 {
                            return Err("INVALID_PARAM".to_string());
                        }
                    }
                    None
                }
                Some(_) => {
                    if let Ok(old_encrypted) = conn.query_row(
                        "SELECT encrypted FROM backup_rules WHERE id = ?1 AND user_id = ?2",
                        params![req.id, user_id],
                        |row| row.get::<_, i64>(0),
                    ) {
                        if old_encrypted == 0 {
                            return Err("INVALID_PARAM".to_string());
                        }
                    }
                    None
                }
            }
        } else {
            None
        };

        let encrypted_int = if req.encrypted { 1i64 } else { 0i64 };
        let backup_time_str = serde_json::to_string(&req.backup_time).map_err(|e| e.to_string())?;

        if backup_password_encrypted.is_some() {
            conn.execute(
                "UPDATE backup_rules SET name = ?1, storage_type = ?2, storage_config = ?3, local_path = ?4, 
                 encrypted = ?5, backup_password = ?6, cycle = ?7, backup_time = ?8, updated_at = datetime('now') 
                 WHERE id = ?9 AND user_id = ?10",
                params![
                    req.name,
                    req.storage_type,
                    storage_config_str,
                    req.local_path,
                    encrypted_int,
                    backup_password_encrypted,
                    req.cycle,
                    backup_time_str,
                    req.id,
                    user_id
                ],
            ).map_err(|e| e.to_string())?;
        } else {
            conn.execute(
                "UPDATE backup_rules SET name = ?1, storage_type = ?2, storage_config = ?3, local_path = ?4, 
                 encrypted = ?5, cycle = ?6, backup_time = ?7, updated_at = datetime('now') 
                 WHERE id = ?8 AND user_id = ?9",
                params![
                    req.name,
                    req.storage_type,
                    storage_config_str,
                    req.local_path,
                    encrypted_int,
                    req.cycle,
                    backup_time_str,
                    req.id,
                    user_id
                ],
            ).map_err(|e| e.to_string())?;
        }

        Ok(true)
    }

    pub fn delete(&self, rule_id: &str, user_id: &str) -> Result<bool, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let exists = match conn.query_row(
            "SELECT 1 FROM backup_rules WHERE id = ?1 AND user_id = ?2",
            params![rule_id, user_id],
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
            "DELETE FROM backup_logs WHERE backup_rule_id = ?1",
            params![rule_id],
        )
        .map_err(|e| e.to_string())?;

        conn.execute(
            "DELETE FROM backup_rules WHERE id = ?1 AND user_id = ?2",
            params![rule_id, user_id],
        )
        .map_err(|e| e.to_string())?;

        Ok(true)
    }

    pub fn create_backup_log(&self, log_id: &str, rule_id: &str, mode: &str) -> Result<(), String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        conn.execute(
            "INSERT INTO backup_logs (id, backup_rule_id, status, mode, started_at) VALUES (?1, ?2, 'running', ?3, ?4)",
            params![log_id, rule_id, mode, now],
        ).map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn finish_backup_log(
        &self,
        log_id: &str,
        status: &str,
        backup_success_count: u64,
        backup_fail_count: u64,
        cleanup_deleted_count: u64,
        fail_reason: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        conn.execute(
            "UPDATE backup_logs SET status = ?1, finished_at = ?2, backup_success_count = ?3, backup_fail_count = ?4, cleanup_deleted_count = ?5, fail_reason = ?6 WHERE id = ?7",
            params![status, now, backup_success_count as i64, backup_fail_count as i64, cleanup_deleted_count as i64, fail_reason, log_id],
        ).map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn get_user_root_path(&self, user_id: &str) -> Result<Option<String>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let result: Result<Option<String>, _> = conn.query_row(
            "SELECT root_path FROM users WHERE id = ?1",
            params![user_id],
            |row| row.get(0),
        );

        match result {
            Ok(path) => Ok(path),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn update_rule_last_backup_time(&self, rule_id: &str) -> Result<(), String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        conn.execute(
            "UPDATE backup_rules SET last_backup_time = ?1 WHERE id = ?2",
            params![now, rule_id],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn interrupt_running_logs(&self) -> Result<(), String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        conn.execute(
            "UPDATE backup_logs SET status = 'interrupted', finished_at = ?1 
             WHERE finished_at IS NULL",
            params![now],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn list_logs_by_rule(
        &self,
        rule_id: &str,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<BackupLogItem>, u32), String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let total: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM backup_logs WHERE backup_rule_id = ?1",
                params![rule_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let offset = (page.saturating_sub(1)) * page_size;

        let mut stmt = conn.prepare(
            "SELECT id, backup_rule_id, mode, status, started_at, finished_at, backup_success_count, backup_fail_count, cleanup_deleted_count, fail_reason 
             FROM backup_logs WHERE backup_rule_id = ?1 ORDER BY started_at DESC LIMIT ?2 OFFSET ?3"
        ).map_err(|e| e.to_string())?;

        let items: Vec<BackupLogItem> = stmt
            .query_map(params![rule_id, page_size, offset], |row| {
                Ok(BackupLogItem {
                    id: row.get(0)?,
                    rule_id: row.get(1)?,
                    mode: row.get(2)?,
                    status: row.get(3)?,
                    started_at: row.get(4)?,
                    finished_at: row.get(5)?,
                    backup_success_count: row.get::<_, i64>(6)? as u64,
                    backup_fail_count: row.get::<_, i64>(7)? as u64,
                    cleanup_deleted_count: row.get::<_, i64>(8)? as u64,
                    fail_reason: row.get(9)?,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok((items, total))
    }

    pub fn get_last_log_status(&self, rule_id: &str) -> Option<String> {
        let conn = self.pool.get().ok()?;
        conn.query_row(
            "SELECT status FROM backup_logs WHERE backup_rule_id = ?1 ORDER BY started_at DESC LIMIT 1",
            params![rule_id],
            |row| row.get(0),
        ).ok()
    }
}

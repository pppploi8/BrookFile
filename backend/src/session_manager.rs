use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::database::Pool;
use rusqlite::params;

struct MemorySession {
    data: HashMap<String, String>,
    last_access_time: Instant,
    last_db_sync_time: Instant,
}

pub struct SessionManager {
    pool: Pool,
    sessions: RwLock<HashMap<String, MemorySession>>,
    timeout_days: RwLock<u64>,
    max_devices: RwLock<u64>,
    regenerated: RwLock<HashMap<String, String>>,
    login_attempts: RwLock<HashMap<String, (u32, Instant)>>,
    global_fail_count: RwLock<(u32, Instant)>,
}

const DB_SYNC_INTERVAL_SECS: u64 = 6 * 3600;
const DEFAULT_TIMEOUT_DAYS: u64 = 7;
const DEFAULT_MAX_DEVICES: u64 = 3;

const MAX_GLOBAL_FAILURES_PER_MINUTE: u32 = 30;
const GLOBAL_DELAY_SECS: u64 = 2;
const BASE_DELAY_SECS: u64 = 1;
const MAX_DELAY_SECS: u64 = 30;

impl SessionManager {
    pub fn new(pool: Pool) -> Arc<Self> {
        let timeout_days = read_config(&pool, "session_timeout_days", DEFAULT_TIMEOUT_DAYS);
        let max_devices = read_config(&pool, "max_login_devices", DEFAULT_MAX_DEVICES);

        let manager = Arc::new(SessionManager {
            pool,
            sessions: RwLock::new(HashMap::new()),
            timeout_days: RwLock::new(timeout_days),
            max_devices: RwLock::new(max_devices),
            regenerated: RwLock::new(HashMap::new()),
            login_attempts: RwLock::new(HashMap::new()),
            global_fail_count: RwLock::new((0, Instant::now())),
        });

        manager.load_sessions_from_db();

        let manager_clone = Arc::clone(&manager);
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_secs(60));
                manager_clone.cleanup_expired();
            }
        });

        manager
    }

    fn load_sessions_from_db(&self) {
        let timeout_days = *self.timeout_days.read().unwrap_or_else(|e| e.into_inner());
        let now_secs = now_secs();
        let cutoff = now_secs.saturating_sub(timeout_days * 86400);

        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return,
        };

        let mut stmt = match conn.prepare(
            "SELECT s.id, s.user_id, u.username, u.is_admin, u.root_path \
             FROM sessions s JOIN users u ON s.user_id = u.id \
             WHERE s.last_access_time > ?1"
        ) {
            Ok(s) => s,
            Err(_) => return,
        };

        let rows = match stmt.query_map(params![cutoff], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        }) {
            Ok(r) => r,
            Err(_) => return,
        };

        let mut sessions = self.sessions.write().unwrap_or_else(|e| e.into_inner());
        for row in rows {
            if let Ok((id, user_id, username, is_admin, root_path)) = row {
                let mut data = HashMap::new();
                data.insert("user_id".to_string(), user_id);
                data.insert("username".to_string(), username);
                data.insert("is_admin".to_string(), if is_admin != 0 { "true" } else { "false" }.to_string());
                if let Some(rp) = root_path {
                    data.insert("root_path".to_string(), rp);
                }
                sessions.insert(id, MemorySession {
                    data,
                    last_access_time: Instant::now(),
                    last_db_sync_time: Instant::now(),
                });
            }
        }
    }

    pub fn get_session_timeout_days(&self) -> u64 {
        *self.timeout_days.read().unwrap_or_else(|e| e.into_inner())
    }

    pub fn get_max_login_devices(&self) -> u64 {
        *self.max_devices.read().unwrap_or_else(|e| e.into_inner())
    }

    pub fn update_config(&self, timeout_days: u64, max_devices: u64) {
        *self.timeout_days.write().unwrap_or_else(|e| e.into_inner()) = timeout_days;
        *self.max_devices.write().unwrap_or_else(|e| e.into_inner()) = max_devices;
    }

    fn write_sessions(&self) -> std::sync::RwLockWriteGuard<'_, HashMap<String, MemorySession>> {
        self.sessions.write().unwrap_or_else(|e| e.into_inner())
    }

    fn read_sessions(&self) -> std::sync::RwLockReadGuard<'_, HashMap<String, MemorySession>> {
        self.sessions.read().unwrap_or_else(|e| e.into_inner())
    }

    fn timeout_secs(&self) -> u64 {
        self.get_session_timeout_days() * 86400
    }

    pub fn validate_session(&self, session_id: &str) -> bool {
        let timeout = self.timeout_secs();
        let should_sync;
        {
            let sessions = self.read_sessions();
            if let Some(session_data) = sessions.get(session_id) {
                if session_data.last_access_time.elapsed().as_secs() < timeout {
                    should_sync = session_data.last_db_sync_time.elapsed().as_secs() >= DB_SYNC_INTERVAL_SECS;
                    drop(sessions);
                    let mut sessions_mut = self.write_sessions();
                    if let Some(sd) = sessions_mut.get_mut(session_id) {
                        sd.last_access_time = Instant::now();
                        if should_sync {
                            sd.last_db_sync_time = Instant::now();
                        }
                    }
                    if should_sync {
                        self.sync_access_time_to_db(session_id);
                    }
                    return true;
                }
            }
        }
        false
    }

    fn sync_access_time_to_db(&self, session_id: &str) {
        if let Ok(conn) = self.pool.get() {
            let _ = conn.execute(
                "UPDATE sessions SET last_access_time = ?1 WHERE id = ?2",
                params![now_secs(), session_id],
            );
        }
    }

    pub fn create_session(&self) -> String {
        let session_id = Uuid::new_v4().to_string();
        let session_data = MemorySession {
            data: HashMap::new(),
            last_access_time: Instant::now(),
            last_db_sync_time: Instant::now(),
        };

        self.write_sessions().insert(session_id.clone(), session_data);

        session_id
    }

    pub fn get(&self, session_id: &str, key: &str) -> Option<String> {
        let timeout = self.timeout_secs();
        let sessions = self.read_sessions();

        if let Some(session_data) = sessions.get(session_id) {
            if session_data.last_access_time.elapsed().as_secs() < timeout {
                return session_data.data.get(key).cloned();
            }
        }
        None
    }

    pub fn set(&self, session_id: &str, key: &str, value: &str) {
        let timeout = self.timeout_secs();
        let mut sessions = self.write_sessions();

        if let Some(session_data) = sessions.get_mut(session_id) {
            if session_data.last_access_time.elapsed().as_secs() < timeout {
                session_data.last_access_time = Instant::now();
                session_data.data.insert(key.to_string(), value.to_string());
            }
        }
    }

    pub fn invalidate(&self, session_id: &str) {
        self.write_sessions().remove(session_id);
        if let Ok(conn) = self.pool.get() {
            let _ = conn.execute("DELETE FROM sessions WHERE id = ?1", params![session_id]);
        }
    }

    pub fn invalidate_user_sessions(&self, user_id: &str) {
        let mut sessions = self.write_sessions();
        let to_remove: Vec<String> = sessions
            .iter()
            .filter(|(_, sd)| sd.data.get("user_id").map(|u| u == user_id).unwrap_or(false))
            .map(|(id, _)| id.clone())
            .collect();
        for id in &to_remove {
            sessions.remove(id);
        }
        drop(sessions);
        if let Ok(conn) = self.pool.get() {
            let _ = conn.execute("DELETE FROM sessions WHERE user_id = ?1", params![user_id]);
        }
    }

    fn cleanup_expired(&self) {
        let timeout = self.timeout_secs();
        let now = Instant::now();

        let expired_ids: Vec<String> = {
            let sessions = self.read_sessions();
            sessions
                .iter()
                .filter(|(_, sd)| now.duration_since(sd.last_access_time).as_secs() >= timeout)
                .map(|(id, _)| id.clone())
                .collect()
        };

        if !expired_ids.is_empty() {
            self.write_sessions().retain(|_, sd| now.duration_since(sd.last_access_time).as_secs() < timeout);
            if let Ok(conn) = self.pool.get() {
                for id in &expired_ids {
                    let _ = conn.execute("DELETE FROM sessions WHERE id = ?1", params![id]);
                }
            }
        }

        let mut attempts = self.login_attempts.write().unwrap_or_else(|e| e.into_inner());
        attempts.retain(|_, (_, locked_at)| locked_at.elapsed().as_secs() < MAX_DELAY_SECS * 2);
        drop(attempts);
        self.regenerated.write().unwrap_or_else(|e| e.into_inner()).clear();
    }

    pub fn get_all_session_ids(&self) -> Vec<String> {
        self.read_sessions().keys().cloned().collect()
    }

    pub fn get_user_id(&self, session_id: &str) -> Option<String> {
        self.get(session_id, "user_id")
    }

    pub fn regenerate(&self, old_session_id: &str) -> Option<String> {
        let new_session_id = Uuid::new_v4().to_string();
        let mut sessions = self.write_sessions();
        let data = sessions.remove(old_session_id)?;
        sessions.insert(new_session_id.clone(), data);
        drop(sessions);

        if let Ok(conn) = self.pool.get() {
            let _ = conn.execute("DELETE FROM sessions WHERE id = ?1", params![old_session_id]);
        }

        let mut regen = self.regenerated.write().unwrap_or_else(|e| e.into_inner());
        regen.insert(old_session_id.to_string(), new_session_id.clone());
        Some(new_session_id)
    }

    pub fn persist_session(&self, session_id: &str) {
        let user_id = self.get(session_id, "user_id");
        if let Some(user_id) = user_id {
            if let Ok(conn) = self.pool.get() {
                let _ = conn.execute(
                    "INSERT OR REPLACE INTO sessions (id, user_id, created_at, last_access_time) VALUES (?1, ?2, ?3, ?4)",
                    params![session_id, user_id, now_secs(), now_secs()],
                );
            }
        }
    }

    pub fn enforce_max_devices(&self, user_id: &str) {
        let max_devices = self.get_max_login_devices();
        if max_devices == 0 {
            return;
        }

        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return,
        };

        let ids: Vec<String> = match conn.prepare(
            "SELECT id FROM sessions WHERE user_id = ?1 ORDER BY created_at ASC"
        ) {
            Ok(mut stmt) => match stmt.query_map(params![user_id], |row| row.get::<_, String>(0)) {
                Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
                Err(_) => return,
            },
            Err(_) => return,
        };

        if ids.len() >= max_devices as usize {
            let to_remove_count = ids.len() - max_devices as usize + 1;
            for id in ids.iter().take(to_remove_count) {
                self.invalidate(id);
            }
        }
    }

    pub fn take_regenerated(&self, old_session_id: &str) -> Option<String> {
        let mut regen = self.regenerated.write().unwrap_or_else(|e| e.into_inner());
        regen.remove(old_session_id)
    }

    pub fn get_login_delay(&self, username: &str) -> u64 {
        let mut should_global_delay = false;
        {
            let global = self.global_fail_count.read().unwrap_or_else(|e| e.into_inner());
            if global.1.elapsed().as_secs() < 60 && global.0 >= MAX_GLOBAL_FAILURES_PER_MINUTE {
                should_global_delay = true;
            }
        }

        let attempts = self.login_attempts.read().unwrap_or_else(|e| e.into_inner());
        if let Some((count, _)) = attempts.get(username) {
            if *count > 0 {
                let exp_delay = BASE_DELAY_SECS * 2u64.pow(count.saturating_sub(1).min(4));
                let delay = exp_delay.min(MAX_DELAY_SECS);
                return if should_global_delay { delay.max(GLOBAL_DELAY_SECS) } else { delay };
            }
        }

        if should_global_delay { GLOBAL_DELAY_SECS } else { 0 }
    }

    pub fn record_login_failure(&self, username: &str) {
        {
            let mut attempts = self.login_attempts.write().unwrap_or_else(|e| e.into_inner());
            let (count, _) = attempts.get(username).copied().unwrap_or((0, Instant::now()));
            attempts.insert(username.to_string(), (count + 1, Instant::now()));
        }
        {
            let mut global = self.global_fail_count.write().unwrap_or_else(|e| e.into_inner());
            if global.1.elapsed().as_secs() >= 60 {
                *global = (1, Instant::now());
            } else {
                global.0 += 1;
            }
        }
    }

    pub fn clear_login_failures(&self, username: &str) {
        let mut attempts = self.login_attempts.write().unwrap_or_else(|e| e.into_inner());
        attempts.remove(username);
    }
}

fn read_config(pool: &Pool, key: &str, default: u64) -> u64 {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return default,
    };
    conn.query_row(
        "SELECT value FROM system_config WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(default)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

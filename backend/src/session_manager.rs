use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

pub struct SessionData {
    data: HashMap<String, String>,
    last_access_time: Instant,
}

pub struct SessionManager {
    sessions: RwLock<HashMap<String, SessionData>>,
    session_timeout_secs: u64,
    regenerated: RwLock<HashMap<String, String>>,
    login_attempts: RwLock<HashMap<String, (u32, Instant)>>,
    global_fail_count: RwLock<(u32, Instant)>,
}

const MAX_LOGIN_ATTEMPTS: u32 = 10;
const MAX_GLOBAL_FAILURES_PER_MINUTE: u32 = 30;
const GLOBAL_DELAY_SECS: u64 = 2;
const BASE_DELAY_SECS: u64 = 1;
const MAX_DELAY_SECS: u64 = 30;

impl SessionManager {
    pub fn new(session_timeout_secs: u64) -> Arc<Self> {
        let manager = Arc::new(SessionManager {
            sessions: RwLock::new(HashMap::new()),
            session_timeout_secs,
            regenerated: RwLock::new(HashMap::new()),
            login_attempts: RwLock::new(HashMap::new()),
            global_fail_count: RwLock::new((0, Instant::now())),
        });

        let manager_clone = Arc::clone(&manager);
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_secs(60));
                manager_clone.cleanup_expired();
            }
        });

        manager
    }

    fn write_sessions(&self) -> std::sync::RwLockWriteGuard<'_, HashMap<String, SessionData>> {
        self.sessions.write().unwrap_or_else(|e| e.into_inner())
    }

    fn read_sessions(&self) -> std::sync::RwLockReadGuard<'_, HashMap<String, SessionData>> {
        self.sessions.read().unwrap_or_else(|e| e.into_inner())
    }

    pub fn validate_session(&self, session_id: &str) -> bool {
        {
            let sessions = self.read_sessions();
            if let Some(session_data) = sessions.get(session_id) {
                if session_data.last_access_time.elapsed().as_secs() < self.session_timeout_secs {
                    drop(sessions);
                    let mut sessions_mut = self.write_sessions();
                    if let Some(session_data) = sessions_mut.get_mut(session_id) {
                        session_data.last_access_time = Instant::now();
                    }
                    return true;
                }
            }
        }
        false
    }

    pub fn create_session(&self) -> String {
        let session_id = Uuid::new_v4().to_string();
        let session_data = SessionData {
            data: HashMap::new(),
            last_access_time: Instant::now(),
        };

        self.write_sessions().insert(session_id.clone(), session_data);

        session_id
    }

    pub fn get(&self, session_id: &str, key: &str) -> Option<String> {
        let sessions = self.read_sessions();

        if let Some(session_data) = sessions.get(session_id) {
            if session_data.last_access_time.elapsed().as_secs() < self.session_timeout_secs {
                return session_data.data.get(key).cloned();
            }
        }
        None
    }

    pub fn set(&self, session_id: &str, key: &str, value: &str) {
        let mut sessions = self.write_sessions();

        if let Some(session_data) = sessions.get_mut(session_id) {
            if session_data.last_access_time.elapsed().as_secs() < self.session_timeout_secs {
                session_data.last_access_time = Instant::now();
                session_data.data.insert(key.to_string(), value.to_string());
            }
        }
    }

    pub fn invalidate(&self, session_id: &str) {
        self.write_sessions().remove(session_id);
    }

    fn cleanup_expired(&self) {
        self.write_sessions().retain(|_, session_data| {
            session_data.last_access_time.elapsed().as_secs() < self.session_timeout_secs
        });
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
        let mut regen = self.regenerated.write().unwrap_or_else(|e| e.into_inner());
        regen.insert(old_session_id.to_string(), new_session_id.clone());
        Some(new_session_id)
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

    #[allow(dead_code)]
    pub fn get_lockout_remaining_secs(&self, username: &str) -> u64 {
        let attempts = self.login_attempts.read().unwrap_or_else(|e| e.into_inner());
        if let Some((count, locked_at)) = attempts.get(username) {
            if *count >= MAX_LOGIN_ATTEMPTS {
                let exp_delay = BASE_DELAY_SECS * 2u64.pow(count.saturating_sub(1).min(4));
                let delay = exp_delay.min(MAX_DELAY_SECS);
                let elapsed = locked_at.elapsed().as_secs();
                if elapsed < delay {
                    return delay - elapsed;
                }
            }
        }
        0
    }
}

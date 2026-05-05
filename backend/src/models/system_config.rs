use crate::database::Pool;
use rusqlite::params;

pub struct SystemConfigModel {
    pool: Pool,
    initialized: std::sync::Mutex<bool>,
}

impl SystemConfigModel {
    pub fn new(pool: &Pool) -> Self {
        let initialized = Self::load_initialized_from_db(pool);
        SystemConfigModel {
            pool: pool.clone(),
            initialized: std::sync::Mutex::new(initialized),
        }
    }

    fn load_initialized_from_db(pool: &Pool) -> bool {
        let conn = match pool.get() {
            Ok(c) => c,
            Err(_) => return false,
        };

        let result: Result<String, _> = conn.query_row(
            "SELECT value FROM system_config WHERE key = 'initialized'",
            [],
            |row| row.get(0),
        );

        match result {
            Ok(value) => value == "true",
            Err(_) => false,
        }
    }

    pub fn is_initialized(&self) -> Result<bool, String> {
        self.initialized
            .lock()
            .map(|guard| *guard)
            .map_err(|e| e.to_string())
    }

    pub fn set_initialized(&self, value: bool) -> Result<(), String> {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(e) => return Err(e.to_string()),
        };

        let result = conn.execute(
            "INSERT OR REPLACE INTO system_config (key, value) VALUES ('initialized', ?1)",
            params![if value { "true" } else { "false" }],
        );

        match result {
            Ok(_) => {
                let mut guard = self.initialized.lock().map_err(|e| e.to_string())?;
                *guard = value;
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn get_config(&self, key: &str) -> Result<Option<String>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        let result: Result<String, _> = conn.query_row(
            "SELECT value FROM system_config WHERE key = ?1",
            params![key],
            |row| row.get(0),
        );

        match result {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn set_config(&self, key: &str, value: &str) -> Result<(), String> {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(e) => return Err(e.to_string()),
        };

        let result = conn.execute(
            "INSERT OR REPLACE INTO system_config (key, value) VALUES (?1, ?2)",
            params![key, value],
        );

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn delete_config(&self, key: &str) -> Result<(), String> {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(e) => return Err(e.to_string()),
        };

        let result = conn.execute("DELETE FROM system_config WHERE key = ?1", params![key]);

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

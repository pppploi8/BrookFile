use r2d2_sqlite::SqliteConnectionManager;

pub type Pool = r2d2::Pool<SqliteConnectionManager>;

pub struct Database {
    pub pool: Pool,
}

impl Database {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let manager = SqliteConnectionManager::file("database.db")
            .with_init(|conn| {
                conn.execute_batch("PRAGMA busy_timeout = 5000; PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")
            });
        let pool = r2d2::Pool::builder().build(manager)?;

        let db = Database { pool };
        db.init_tables()?;

        Ok(db)
    }

    fn init_tables(&self) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS system_config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                username TEXT UNIQUE NOT NULL,
                password_hash TEXT NOT NULL,
                password_salt TEXT NOT NULL,
                root_path TEXT,
                recycle_bin_path TEXT,
                is_admin INTEGER DEFAULT 0,
                expire_at TIMESTAMP,
                remark TEXT,
                feature_order TEXT DEFAULT 'file,note,password',
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS upload_cache (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL REFERENCES users(id),
                file_path TEXT NOT NULL,
                temp_file_path TEXT NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                last_updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS backup_rules (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL REFERENCES users(id),
                name TEXT NOT NULL,
                storage_type TEXT NOT NULL,
                storage_config TEXT NOT NULL,
                local_path TEXT NOT NULL,
                encrypted INTEGER DEFAULT 0,
                backup_password TEXT,
                cycle TEXT NOT NULL,
                backup_time TEXT NOT NULL,
                last_backup_time TIMESTAMP,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS backup_logs (
                id TEXT PRIMARY KEY,
                backup_rule_id TEXT NOT NULL REFERENCES backup_rules(id),
                mode TEXT DEFAULT 'full',
                status TEXT NOT NULL,
                backup_success_count INTEGER DEFAULT 0,
                backup_fail_count INTEGER DEFAULT 0,
                cleanup_deleted_count INTEGER DEFAULT 0,
                fail_reason TEXT,
                started_at TIMESTAMP NOT NULL,
                finished_at TIMESTAMP
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_backup_logs_rule_started ON backup_logs(backup_rule_id, started_at)",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS recycle_bin (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL REFERENCES users(id),
                original_path TEXT NOT NULL,
                original_name TEXT NOT NULL,
                is_directory INTEGER NOT NULL DEFAULT 0,
                file_size INTEGER NOT NULL DEFAULT 0,
                deleted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_recycle_bin_user_deleted ON recycle_bin(user_id, deleted_at)",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS vaults (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL REFERENCES users(id),
                name TEXT NOT NULL,
                description TEXT DEFAULT '',
                path TEXT NOT NULL,
                filename TEXT NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_vaults_user_id ON vaults(user_id)",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS notebooks (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL REFERENCES users(id),
                name TEXT NOT NULL,
                description TEXT DEFAULT '',
                path TEXT NOT NULL,
                encrypted INTEGER DEFAULT 0,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_notebooks_user_id ON notebooks(user_id)",
            [],
        )?;

        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_notebooks_user_path ON notebooks(user_id, path)",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS shares (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL REFERENCES users(id),
                file_path TEXT NOT NULL,
                file_name TEXT NOT NULL,
                is_directory INTEGER NOT NULL DEFAULT 0,
                share_code TEXT NOT NULL UNIQUE,
                expire_type TEXT NOT NULL DEFAULT 'permanent',
                expire_at TIMESTAMP NULL,
                max_downloads INTEGER NULL,
                download_count INTEGER NOT NULL DEFAULT 0,
                share_mode TEXT NOT NULL DEFAULT 'page',
                password TEXT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_shares_user_id ON shares(user_id)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_shares_user_path ON shares(user_id, file_path)",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS webdav_configs (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL REFERENCES users(id),
                dav_path TEXT NOT NULL DEFAULT '',
                access_path TEXT NOT NULL,
                password TEXT NOT NULL,
                password_salt TEXT NOT NULL,
                permission TEXT NOT NULL DEFAULT 'full_control',
                global_access INTEGER NOT NULL DEFAULT 0,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_webdav_configs_user_id ON webdav_configs(user_id)",
            [],
        )?;

        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_webdav_configs_user_dav_path ON webdav_configs(user_id, dav_path)",
            [],
        )?;

        let _ = conn.execute(
            "ALTER TABLE webdav_configs ADD COLUMN global_access INTEGER NOT NULL DEFAULT 0",
            [],
        );

        let _ = conn.execute(
            "ALTER TABLE upload_cache ADD COLUMN user_id TEXT NOT NULL DEFAULT ''",
            [],
        );

        let _ = conn.execute(
            "ALTER TABLE webdav_configs ADD COLUMN digest_ha1 TEXT",
            [],
        );

        let _ = conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_upload_cache_user_path ON upload_cache(user_id, file_path)",
            [],
        );

        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                last_access_time INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sessions_last_access ON sessions(last_access_time)",
            [],
        )?;

        conn.execute(
            "INSERT OR IGNORE INTO system_config (key, value) VALUES ('system_name', 'BrookFile')",
            [],
        )?;

        Ok(())
    }
}

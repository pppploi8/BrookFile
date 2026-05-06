use crate::models::NotebookInfo;
use jieba_rs::{Jieba, TokenizeMode};
use rusqlite::params;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct SearchManager {
    enabled: bool,
    jieba: Option<Arc<Jieba>>,
    conn_cache: Mutex<HashMap<String, rusqlite::Connection>>,
    notebook_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

#[derive(Serialize, Clone)]
pub struct SearchMatch {
    pub line_number: i64,
    pub content: String,
}

#[derive(Serialize)]
pub struct SearchResult {
    pub notebook_id: String,
    pub notebook_name: String,
    pub note_path: String,
    pub title: String,
    pub title_matched: bool,
    pub matches: Vec<SearchMatch>,
    pub match_count: i64,
    pub modified: Option<String>,
}

pub struct SearchOutput {
    pub results: Vec<SearchResult>,
    pub failed_notebooks: usize,
    pub total_notebooks: usize,
}

impl SearchManager {
    pub fn new(enabled: bool) -> Self {
        let jieba = if enabled {
            Some(Arc::new(Jieba::new()))
        } else {
            None
        };
        SearchManager {
            enabled,
            jieba,
            conn_cache: Mutex::new(HashMap::new()),
            notebook_locks: Mutex::new(HashMap::new()),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn get_notebook_lock(&self, notebook_id: &str) -> Arc<Mutex<()>> {
        let mut locks = self.notebook_locks.lock().unwrap();
        locks
            .entry(notebook_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    fn get_connection(&self, notebook_id: &str) -> Result<rusqlite::Connection, String> {
        {
            let mut cache = self.conn_cache.lock().map_err(|e| e.to_string())?;
            if let Some(conn) = cache.remove(notebook_id) {
                return Ok(conn);
            }
        }
        Self::create_connection(notebook_id)
    }

    fn return_connection(&self, notebook_id: &str, conn: rusqlite::Connection) {
        if let Ok(mut cache) = self.conn_cache.lock() {
            cache.entry(notebook_id.to_string()).or_insert(conn);
        }
    }

    fn create_connection(notebook_id: &str) -> Result<rusqlite::Connection, String> {
        let dir = Self::get_notebook_search_dir()?;
        let db_path = dir.join(format!("{}.db", notebook_id));
        let conn = rusqlite::Connection::open(&db_path).map_err(|e| e.to_string())?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
            .map_err(|e| e.to_string())?;
        Ok(conn)
    }

    pub fn init_search_db(&self, notebook_id: &str) -> Result<(), String> {
        let lock = self.get_notebook_lock(notebook_id);
        let _guard = lock.lock().map_err(|e| e.to_string())?;
        let conn = self.get_connection(notebook_id)?;
        let result = conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS note_index USING fts5(content, tokenize='unicode61', content='note_index_content');
             CREATE TABLE IF NOT EXISTS note_index_content (id INTEGER PRIMARY KEY, note_path TEXT NOT NULL, line_number INTEGER NOT NULL, content TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS index_meta (note_path TEXT NOT NULL, file_modified TEXT, indexed_at TEXT, PRIMARY KEY (note_path));"
        ).map_err(|e| e.to_string());
        self.return_connection(notebook_id, conn);
        result
    }

    pub fn index_note(
        &self,
        notebook_id: &str,
        note_path: &str,
        content: &str,
        notebook_root: &str,
    ) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        self.init_search_db(notebook_id)?;
        let lock = self.get_notebook_lock(notebook_id);
        let _guard = lock.lock().map_err(|e| e.to_string())?;
        let conn = self.get_connection(notebook_id)?;

        let result = {
            let retry = |op: &dyn Fn(&rusqlite::Connection) -> Result<(), String>| {
                for attempt in 0..3 {
                    match op(&conn) {
                        Ok(()) => return Ok(()),
                        Err(e) if e.contains("SQLITE_BUSY") && attempt < 2 => {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            continue;
                        }
                        Err(e) => return Err(e),
                    }
                }
                Err("retry exhausted".to_string())
            };

            retry(&|conn: &rusqlite::Connection| {
                let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

                tx.execute("DELETE FROM note_index WHERE rowid IN (SELECT id FROM note_index_content WHERE note_path = ?1)", params![note_path]).map_err(|e| e.to_string())?;
                tx.execute(
                    "DELETE FROM note_index_content WHERE note_path = ?1",
                    params![note_path],
                )
                .map_err(|e| e.to_string())?;
                tx.execute(
                    "DELETE FROM index_meta WHERE note_path = ?1",
                    params![note_path],
                )
                .map_err(|e| e.to_string())?;

                for (line_num, line) in content.lines().enumerate() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    let tokens = self.jieba.as_ref().unwrap().tokenize(trimmed, TokenizeMode::Search, true);
                    let tokenized: String = tokens.iter().map(|t| t.word).collect::<Vec<_>>().join(" ");

                    tx.execute(
                        "INSERT INTO note_index_content (note_path, line_number, content) VALUES (?1, ?2, ?3)",
                        params![note_path, line_num as i64, line],
                    ).map_err(|e| e.to_string())?;

                    let rowid: i64 = tx
                        .query_row("SELECT last_insert_rowid()", [], |row| row.get(0))
                        .map_err(|e| e.to_string())?;

                    tx.execute(
                        "INSERT INTO note_index (rowid, content) VALUES (?1, ?2)",
                        params![rowid, tokenized],
                    )
                    .map_err(|e| e.to_string())?;
                }

                let file_modified = Self::get_file_mtime(notebook_root, note_path)?;
                let now = chrono::Utc::now().to_rfc3339();
                tx.execute(
                    "INSERT INTO index_meta (note_path, file_modified, indexed_at) VALUES (?1, ?2, ?3)",
                    params![note_path, file_modified, now],
                )
                .map_err(|e| e.to_string())?;

                tx.commit().map_err(|e| e.to_string())?;
                Ok(())
            })
        };

        self.return_connection(notebook_id, conn);
        result
    }

    pub fn remove_note_index(&self, notebook_id: &str, note_path: &str) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        let lock = self.get_notebook_lock(notebook_id);
        let _guard = lock.lock().map_err(|e| e.to_string())?;
        let conn = self.get_connection(notebook_id)?;
        let result = Self::retry_on_busy(|| {
            let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM note_index WHERE rowid IN (SELECT id FROM note_index_content WHERE note_path = ?1)", params![note_path]).map_err(|e| e.to_string())?;
            tx.execute(
                "DELETE FROM note_index_content WHERE note_path = ?1",
                params![note_path],
            )
            .map_err(|e| e.to_string())?;
            tx.execute(
                "DELETE FROM index_meta WHERE note_path = ?1",
                params![note_path],
            )
            .map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
            Ok(())
        });
        self.return_connection(notebook_id, conn);
        result
    }

    pub fn remove_note_index_with_children(
        &self,
        notebook_id: &str,
        note_path: &str,
    ) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        let lock = self.get_notebook_lock(notebook_id);
        let _guard = lock.lock().map_err(|e| e.to_string())?;
        let conn = self.get_connection(notebook_id)?;
        let escaped = note_path.replace('%', "\\%").replace('_', "\\_");
        let prefix = format!("{}/%", escaped);
        let result = Self::retry_on_busy(|| {
            let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM note_index WHERE rowid IN (SELECT id FROM note_index_content WHERE note_path = ?1 OR note_path LIKE ?2 ESCAPE '\\')", params![note_path, prefix]).map_err(|e| e.to_string())?;
            tx.execute(
                "DELETE FROM note_index_content WHERE note_path = ?1 OR note_path LIKE ?2 ESCAPE '\\'",
                params![note_path, prefix],
            )
            .map_err(|e| e.to_string())?;
            tx.execute(
                "DELETE FROM index_meta WHERE note_path = ?1 OR note_path LIKE ?2 ESCAPE '\\'",
                params![note_path, prefix],
            )
            .map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
            Ok(())
        });
        self.return_connection(notebook_id, conn);
        result
    }

    pub fn rename_note_index(
        &self,
        notebook_id: &str,
        old_path: &str,
        new_path: &str,
        notebook_root: &str,
    ) -> Result<(), String> {
        self.remove_note_index_with_children(notebook_id, old_path)?;

        let new_full = PathBuf::from(notebook_root).join(new_path);
        if new_full.is_dir() {
            let root = PathBuf::from(notebook_root);
            let md_files = Self::walk_md_files(&new_full, &root)?;
            for relative_path in &md_files {
                let full_path = root.join(relative_path);
                if let Ok(content) = std::fs::read_to_string(&full_path) {
                    let _ = self.index_note(
                        notebook_id,
                        &relative_path.replace('\\', "/"),
                        &content,
                        notebook_root,
                    );
                }
            }
        } else if new_full.extension().and_then(|e| e.to_str()) == Some("md") {
            let content = std::fs::read_to_string(&new_full).map_err(|e| e.to_string())?;
            self.index_note(notebook_id, new_path, &content, notebook_root)?;
        }
        Ok(())
    }

    pub fn delete_search_db(&self, notebook_id: &str) -> Result<(), String> {
        {
            let lock = self.get_notebook_lock(notebook_id);
            let _guard = lock.lock().map_err(|e| e.to_string())?;
            let mut cache = self.conn_cache.lock().map_err(|e| e.to_string())?;
            cache.remove(notebook_id);
        }
        let dir = Self::get_notebook_search_dir()?;
        let db_path = dir.join(format!("{}.db", notebook_id));
        if db_path.exists() {
            std::fs::remove_file(&db_path).map_err(|e| e.to_string())?;
        }
        let wal_path = dir.join(format!("{}.db-wal", notebook_id));
        if wal_path.exists() {
            let _ = std::fs::remove_file(&wal_path);
        }
        let shm_path = dir.join(format!("{}.db-shm", notebook_id));
        if shm_path.exists() {
            let _ = std::fs::remove_file(&shm_path);
        }
        Ok(())
    }

    pub fn search(
        arc_self: &Arc<Self>,
        keyword: &str,
        notebook_id: Option<&str>,
        notebooks: Vec<NotebookInfo>,
        root_path: &str,
    ) -> Result<SearchOutput, String> {
        if !arc_self.enabled {
            return Self::title_only_search(keyword, notebook_id, notebooks, root_path);
        }

        let target_notebooks: Vec<(String, String, String)> = notebooks
            .into_iter()
            .filter(|nb| !nb.encrypted)
            .map(|nb| (nb.id, nb.name, nb.path))
            .collect();

        let keyword_tokens = arc_self.jieba.as_ref().unwrap().tokenize(keyword, TokenizeMode::Search, true);
        let search_query: String = keyword_tokens
            .iter()
            .map(|t| t.word)
            .collect::<Vec<_>>()
            .join(" ");

        if search_query.trim().is_empty() {
            return Ok(SearchOutput { results: vec![], failed_notebooks: 0, total_notebooks: 0 });
        }

        let target_notebooks: Vec<(String, String, String)> = if let Some(nid) = notebook_id {
            target_notebooks
                .into_iter()
                .filter(|(id, _, _)| id == nid)
                .collect()
        } else {
            target_notebooks
        };

        let keyword_owned = keyword.to_string();
        let handles: Vec<std::thread::JoinHandle<Option<Vec<SearchResult>>>> = target_notebooks
            .into_iter()
            .map(|(nb_id, nb_name, _nb_path)| {
                let mgr = Arc::clone(arc_self);
                let sq = search_query.clone();
                let kw = keyword_owned.clone();
                std::thread::spawn(move || {
                    let lock = mgr.get_notebook_lock(&nb_id);
                    let _guard = match lock.lock() {
                        Ok(g) => g,
                        Err(_) => return None,
                    };
                    let conn = match mgr.get_connection(&nb_id) {
                        Ok(c) => c,
                        Err(_) => return None,
                    };
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        Self::search_single_notebook_with_conn(&conn, &nb_id, &nb_name, &sq, &kw)
                    }));
                    mgr.return_connection(&nb_id, conn);
                    result.unwrap_or(None)
                })
            })
            .collect();

        let mut results = Vec::new();
        let mut fail_count = 0usize;
        let total = handles.len();
        for handle in handles {
            match handle.join() {
                Ok(Some(r)) => results.extend(r),
                _ => fail_count += 1,
            }
        }

        if fail_count > 0 && results.is_empty() && fail_count == total {
            return Err(format!("all {} search threads failed", fail_count));
        }

        results.sort_by(|a, b| match a.notebook_name.cmp(&b.notebook_name) {
            std::cmp::Ordering::Equal => b.match_count.cmp(&a.match_count),
            other => other,
        });
        Ok(SearchOutput { results, failed_notebooks: fail_count, total_notebooks: total })
    }

    fn title_only_search(
        keyword: &str,
        notebook_id: Option<&str>,
        notebooks: Vec<NotebookInfo>,
        root_path: &str,
    ) -> Result<SearchOutput, String> {
        let keyword_lower = keyword.to_lowercase();
        let target: Vec<&NotebookInfo> = notebooks
            .iter()
            .filter(|nb| !nb.encrypted)
            .filter(|nb| notebook_id.map_or(true, |id| nb.id == id))
            .collect();

        let total = target.len();
        let mut results = Vec::new();
        let mut fail_count = 0usize;

        for nb in &target {
            let nb_dir = PathBuf::from(root_path).join(&nb.path);
            if !nb_dir.exists() {
                fail_count += 1;
                continue;
            }
            match Self::walk_md_files(&nb_dir, &nb_dir) {
                Ok(files) => {
                    for rel_path in &files {
                        let title = PathBuf::from(rel_path)
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| rel_path.clone());
                        if title.to_lowercase().contains(&keyword_lower) {
                            results.push(SearchResult {
                                notebook_id: nb.id.clone(),
                                notebook_name: nb.name.clone(),
                                note_path: rel_path.replace('\\', "/"),
                                title,
                                title_matched: true,
                                matches: vec![],
                                match_count: 0,
                                modified: None,
                            });
                        }
                    }
                }
                Err(_) => fail_count += 1,
            }
        }

        results.sort_by(|a, b| match a.notebook_name.cmp(&b.notebook_name) {
            std::cmp::Ordering::Equal => a.note_path.cmp(&b.note_path),
            other => other,
        });
        Ok(SearchOutput { results, failed_notebooks: fail_count, total_notebooks: total })
    }

    fn search_single_notebook_with_conn(
        search_conn: &rusqlite::Connection,
        nb_id: &str,
        nb_name: &str,
        search_query: &str,
        keyword: &str,
    ) -> Option<Vec<SearchResult>> {
        let fts_query = "SELECT cic.note_path, cic.line_number, cic.content FROM note_index ni JOIN note_index_content cic ON ni.rowid = cic.id WHERE note_index MATCH ?1 ORDER BY cic.note_path, cic.line_number";

        let mut stmt = search_conn.prepare(fts_query).ok()?;
        let rows = stmt
            .query_map(params![search_query], |row| {
                let note_path: String = row.get(0)?;
                let line_number: i64 = row.get(1)?;
                let content: String = row.get(2)?;
                Ok((note_path, line_number, content))
            })
            .ok()?;

        let mut note_matches: std::collections::HashMap<String, Vec<SearchMatch>> =
            std::collections::HashMap::new();
        let mut note_match_counts: std::collections::HashMap<String, i64> =
            std::collections::HashMap::new();

        for row in rows.flatten() {
            let (note_path, line_number, content) = row;
            if let Some(m) = Self::extract_match(&content, keyword, line_number) {
                note_matches.entry(note_path.clone()).or_default().push(m);
            }
            *note_match_counts.entry(note_path).or_insert(0) += 1;
        }

        let meta_map: std::collections::HashMap<String, Option<String>> = {
            let mut s = search_conn
                .prepare("SELECT note_path, file_modified FROM index_meta")
                .ok()?;
            let r = s
                .query_map([], |row| {
                    let path: String = row.get(0)?;
                    let modified: Option<String> = row.get(1)?;
                    Ok((path, modified))
                })
                .ok()?;
            r.collect::<Result<std::collections::HashMap<_, _>, _>>()
                .unwrap_or_default()
        };

        let keyword_lower = keyword.to_lowercase();

        for note_path in meta_map.keys() {
            let title = PathBuf::from(note_path)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| note_path.clone());
            if title.to_lowercase().contains(&keyword_lower) && !note_matches.contains_key(note_path) {
                note_matches.entry(note_path.clone()).or_default();
            }
        }

        let mut results = Vec::new();

        for (note_path, mut matches_list) in note_matches {
            let match_count = *note_match_counts.get(&note_path).unwrap_or(&0);
            let title = PathBuf::from(&note_path)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| note_path.clone());

            let title_matched = title.to_lowercase().contains(&keyword_lower);
            matches_list.truncate(10);

            let modified = meta_map.get(&note_path).and_then(|m| m.clone());

            results.push(SearchResult {
                notebook_id: nb_id.to_string(),
                notebook_name: nb_name.to_string(),
                note_path,
                title,
                title_matched,
                matches: matches_list,
                match_count,
                modified,
            });
        }

        Some(results)
    }

    pub fn rebuild_notebook_index(
        &self,
        notebook_id: &str,
        notebook_root: &str,
    ) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        self.init_search_db(notebook_id)?;

        let root = PathBuf::from(notebook_root);
        if !root.exists() {
            return Ok(());
        }

        let md_files = Self::walk_md_files(&root, &root)?;
        for relative_path in &md_files {
            let full_path = root.join(relative_path);
            let content = match std::fs::read_to_string(&full_path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let _ = self.index_note(
                notebook_id,
                &relative_path.replace('\\', "/"),
                &content,
                notebook_root,
            );
        }

        Ok(())
    }

    pub fn needs_rebuild(&self, notebook_id: &str) -> bool {
        if !self.enabled {
            return false;
        }
        let dir = match Self::get_notebook_search_dir() {
            Ok(d) => d,
            Err(_) => return true,
        };
        let db_path = dir.join(format!("{}.db", notebook_id));
        if !db_path.exists() {
            return true;
        }

        let lock = self.get_notebook_lock(notebook_id);
        let _guard = match lock.lock() {
            Ok(g) => g,
            Err(_) => return true,
        };
        let conn = match self.get_connection(notebook_id) {
            Ok(c) => c,
            Err(_) => return true,
        };

        let max_indexed: Option<String> =
            match conn.query_row("SELECT MAX(indexed_at) FROM index_meta", [], |row| {
                row.get(0)
            }) {
                Ok(v) => v,
                Err(_) => {
                    self.return_connection(notebook_id, conn);
                    return true;
                }
            };

        self.return_connection(notebook_id, conn);

        match max_indexed {
            None => true,
            Some(s) => {
                let indexed_time = match chrono::DateTime::parse_from_rfc3339(&s) {
                    Ok(dt) => dt.with_timezone(&chrono::Utc),
                    Err(_) => return true,
                };
                let now = chrono::Utc::now();
                (now - indexed_time) > chrono::Duration::hours(24)
            }
        }
    }

    pub fn incremental_rebuild(
        &self,
        notebook_id: &str,
        notebook_root: &str,
    ) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        let lock = self.get_notebook_lock(notebook_id);
        let _guard = lock.lock().map_err(|e| e.to_string())?;
        let conn = self.get_connection(notebook_id)?;

        let indexed_files: std::collections::HashMap<String, Option<String>> = {
            let mut stmt = conn
                .prepare("SELECT note_path, file_modified FROM index_meta")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |row| {
                    let path: String = row.get(0)?;
                    let modified: Option<String> = row.get(1)?;
                    Ok((path, modified))
                })
                .map_err(|e| e.to_string())?;
            rows.collect::<Result<std::collections::HashMap<_, _>, _>>()
                .map_err(|e| e.to_string())?
        };

        self.return_connection(notebook_id, conn);

        let root = PathBuf::from(notebook_root);
        if !root.exists() {
            return Ok(());
        }

        let md_files = Self::walk_md_files(&root, &root)?;

        for relative_path in &md_files {
            let normalized = relative_path.replace('\\', "/");
            let current_mtime =
                Self::get_file_mtime(notebook_root, &normalized).unwrap_or_default();
            let needs_index = match indexed_files.get(&normalized) {
                None => true,
                Some(stored_mtime) => match stored_mtime {
                    Some(stored) => stored != &current_mtime,
                    None => true,
                },
            };

            if needs_index {
                let full_path = root.join(relative_path);
                let content = match std::fs::read_to_string(&full_path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let _ = self.index_note(notebook_id, &normalized, &content, notebook_root);
            }
        }

        for indexed_path in indexed_files.keys() {
            let full_path = root.join(indexed_path);
            if !full_path.exists() {
                let _ = self.remove_note_index(notebook_id, indexed_path);
            }
        }

        Ok(())
    }

    fn retry_on_busy<F, T>(op: F) -> Result<T, String>
    where
        F: Fn() -> Result<T, String>,
    {
        for attempt in 0..3 {
            match op() {
                Ok(result) => return Ok(result),
                Err(e) if e.contains("SQLITE_BUSY") && attempt < 2 => {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        Err("retry exhausted".to_string())
    }

    fn get_notebook_search_dir() -> Result<PathBuf, String> {
        let exe_dir = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe_parent = exe_dir.parent().unwrap_or(std::path::Path::new("."));
        let search_dir = exe_parent.join("notebook_search");
        if !search_dir.exists() {
            std::fs::create_dir_all(&search_dir).map_err(|e| e.to_string())?;
        }
        Ok(search_dir)
    }

    fn get_file_mtime(notebook_root: &str, note_path: &str) -> Result<String, String> {
        let full_path = PathBuf::from(notebook_root).join(note_path);
        let metadata = std::fs::metadata(&full_path).map_err(|e| e.to_string())?;
        let modified = metadata.modified().map_err(|e| e.to_string())?;
        let datetime: chrono::DateTime<chrono::Utc> = modified.into();
        Ok(datetime.to_rfc3339())
    }

    fn walk_md_files(dir: &PathBuf, base: &PathBuf) -> Result<Vec<String>, String> {
        let mut result = Vec::new();
        let entries = std::fs::read_dir(dir).map_err(|e| e.to_string())?;

        for entry in entries.flatten() {
            let path = entry.path();
            let filename = entry.file_name().to_string_lossy().to_string();

            if filename.starts_with('.') {
                continue;
            }

            if filename == "attachment" {
                continue;
            }

            if path.is_dir() {
                let mut sub = Self::walk_md_files(&path, base)?;
                result.append(&mut sub);
            } else if filename.ends_with(".md") {
                let relative = path.strip_prefix(base).unwrap_or(&path);
                result.push(relative.to_string_lossy().to_string());
            }
        }

        Ok(result)
    }

    fn extract_match(content: &str, keyword: &str, line_number: i64) -> Option<SearchMatch> {
        let content_lower = content.to_lowercase();
        let keyword_lower = keyword.to_lowercase();

        if !content_lower.contains(&keyword_lower) {
            return None;
        }

        let content_chars: Vec<char> = content.chars().collect();
        let lower_chars: Vec<char> = content_lower.chars().collect();
        let keyword_chars: Vec<char> = keyword_lower.chars().collect();
        let keyword_char_len = keyword_chars.len();

        let char_to_byte: Vec<usize> = {
            let mut v = Vec::with_capacity(content_chars.len() + 1);
            let mut byte_pos = 0;
            v.push(0);
            for ch in &content_chars {
                byte_pos += ch.len_utf8();
                v.push(byte_pos);
            }
            v
        };

        let mut result = String::new();
        let mut last_char_end = 0;

        let mut search_from = 0;
        while search_from + keyword_char_len <= lower_chars.len() {
            let matched = lower_chars[search_from..].starts_with(&keyword_chars);
            if !matched {
                search_from += 1;
                continue;
            }

            let match_char_start = search_from;
            let match_char_end = search_from + keyword_char_len;

            let byte_start = char_to_byte[match_char_start];
            let byte_end = char_to_byte[match_char_end];
            let last_byte = char_to_byte[last_char_end];

            result.push_str(&content[last_byte..byte_start]);
            result.push_str("<match>");
            result.push_str(&content[byte_start..byte_end]);
            result.push_str("</match>");

            last_char_end = match_char_end;
            search_from = match_char_end;
        }

        let last_byte = char_to_byte[last_char_end];
        result.push_str(&content[last_byte..]);

        Some(SearchMatch {
            line_number,
            content: result,
        })
    }
}

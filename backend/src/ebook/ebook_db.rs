use std::path::{Path, PathBuf};
use rusqlite::params;
use uuid::Uuid;

const META_DIR: &str = ".brookfile";
const META_DB: &str = "metadata.db";
const SCHEMA_VERSION: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EbookDbStatus {
    Ok,
    Missing,
    Corrupt,
    SchemaMismatch,
}

pub fn ebook_meta_dir(root_path: &Path, relative_ebook_path: &str) -> PathBuf {
    root_path.join(relative_ebook_path).join(META_DIR)
}

pub fn ebook_meta_db_path(root_path: &Path, relative_ebook_path: &str) -> PathBuf {
    ebook_meta_dir(root_path, relative_ebook_path).join(META_DB)
}

pub fn init_ebook_metadata_db(root_path: &Path, relative_ebook_path: &str) -> Result<(), String> {
    let meta_dir = ebook_meta_dir(root_path, relative_ebook_path);
    std::fs::create_dir_all(&meta_dir)
        .map_err(|e| format!("create ebook metadata dir: {}", e))?;

    let db_path = ebook_meta_db_path(root_path, relative_ebook_path);
    let conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| format!("open ebook metadata db: {}", e))?;

    conn.execute_batch(
        "PRAGMA journal_mode=WAL; \
         CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT); \
         CREATE TABLE IF NOT EXISTS books (\
             id TEXT PRIMARY KEY, \
             path TEXT UNIQUE, \
             title TEXT, \
             author TEXT, \
             format TEXT, \
             cover_path TEXT, \
             file_size INTEGER, \
             added_at TEXT, \
             updated_at TEXT\
         ); \
         CREATE TABLE IF NOT EXISTS reading_progress (\
             book_id TEXT PRIMARY KEY, \
             position REAL, \
             page INTEGER, \
             cfi TEXT, \
             updated_at TEXT\
         ); \
         CREATE TABLE IF NOT EXISTS category (\
             id TEXT PRIMARY KEY, \
             name TEXT NOT NULL, \
             sort_order INTEGER DEFAULT 0\
         ); \
         CREATE TABLE IF NOT EXISTS chapter_index (\
             book_id TEXT NOT NULL, \
             chapter_no INTEGER NOT NULL, \
             title TEXT, \
             location TEXT NOT NULL, \
             PRIMARY KEY(book_id, chapter_no)\
         ); \
         CREATE TABLE IF NOT EXISTS bookmark (\
             id TEXT PRIMARY KEY, \
             book_id TEXT NOT NULL, \
             bookmark_type TEXT NOT NULL, \
             content_coord TEXT NOT NULL, \
             summary TEXT, \
             created_at TEXT NOT NULL, \
             updated_at TEXT NOT NULL\
         );",
    )
    .map_err(|e| format!("create ebook metadata tables: {}", e))?;

    let ver: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;

    if ver == 0 {
        conn.execute_batch(&format!("PRAGMA user_version = {}", SCHEMA_VERSION))
            .map_err(|e| e.to_string())?;
    } else if ver < SCHEMA_VERSION {
        migrate_incremental(&conn, ver)?;
        conn.execute_batch(&format!("PRAGMA user_version = {}", SCHEMA_VERSION))
            .map_err(|e| e.to_string())?;
    }

    add_missing_columns(&conn)?;
    Ok(())
}

fn migrate_incremental(conn: &rusqlite::Connection, from: i32) -> Result<(), String> {
    if from < 2 {
        let _ = conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS category (\
                 id TEXT PRIMARY KEY, \
                 name TEXT NOT NULL, \
                 sort_order INTEGER DEFAULT 0\
             ); \
             CREATE TABLE IF NOT EXISTS chapter_index (\
                 book_id TEXT NOT NULL, \
                 chapter_no INTEGER NOT NULL, \
                 title TEXT, \
                 location TEXT NOT NULL, \
                 PRIMARY KEY(book_id, chapter_no)\
             ); \
             CREATE TABLE IF NOT EXISTS bookmark (\
                 id TEXT PRIMARY KEY, \
                 book_id TEXT NOT NULL, \
                 bookmark_type TEXT NOT NULL, \
                 content_coord TEXT NOT NULL, \
                 summary TEXT, \
                 created_at TEXT NOT NULL, \
                 updated_at TEXT NOT NULL\
             );",
        );
    }
    Ok(())
}

fn add_missing_columns(conn: &rusqlite::Connection) -> Result<(), String> {
    let columns = [
        ("sha256", "TEXT"),
        ("category_id", "TEXT"),
        ("scan_status", "TEXT DEFAULT 'pending'"),
        ("preview_filename", "TEXT"),
        ("txt_encoding", "TEXT"),
        ("txt_cache_filename", "TEXT"),
    ];

    for (col, col_type) in &columns {
        let check_sql = format!(
            "SELECT COUNT(*) FROM pragma_table_info('books') WHERE name='{}'",
            col
        );
        let count: i64 = conn
            .query_row(&check_sql, [], |r| r.get(0))
            .unwrap_or(0);
        if count == 0 {
            let alter_sql = format!("ALTER TABLE books ADD COLUMN {} {}", col, col_type);
            let _ = conn.execute(&alter_sql, []);
        }
    }
    Ok(())
}

pub fn check_ebook_metadata_db(
    root_path: &Path,
    relative_ebook_path: &str,
) -> Result<EbookDbStatus, String> {
    let db_path = ebook_meta_db_path(root_path, relative_ebook_path);
    if !db_path.exists() {
        return Ok(EbookDbStatus::Missing);
    }

    let conn = match rusqlite::Connection::open(&db_path) {
        Ok(c) => c,
        Err(_) => return Ok(EbookDbStatus::Corrupt),
    };

    let ver: Result<i32, _> = conn.query_row("PRAGMA user_version", [], |r| r.get(0));
    match ver {
        Ok(v) if v == SCHEMA_VERSION => {
            let table_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('meta','books','reading_progress','category','chapter_index','bookmark')",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            if table_count >= 6 {
                Ok(EbookDbStatus::Ok)
            } else {
                Ok(EbookDbStatus::SchemaMismatch)
            }
        }
        Ok(_) => Ok(EbookDbStatus::SchemaMismatch),
        Err(_) => Ok(EbookDbStatus::Corrupt),
    }
}

pub struct EbookDb {
    conn: rusqlite::Connection,
}

#[derive(Debug, Clone)]
pub struct BookRecord {
    pub id: String,
    pub source_path: String,
    pub format: String,
    pub title: String,
    pub file_size: i64,
    pub sha256: String,
    pub category_id: Option<String>,
    pub added_at: String,
    pub scan_status: String,
    pub preview_filename: Option<String>,
    pub txt_cache_filename: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CategoryRecord {
    pub id: String,
    pub name: String,
    pub sort_order: i64,
}

#[derive(Debug, Clone)]
pub struct ChapterRecord {
    pub chapter_no: i64,
    pub title: Option<String>,
    pub location: String,
}

#[derive(Debug, Clone)]
pub struct BookmarkRecord {
    pub id: String,
    pub content_coord: String,
    pub summary: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct ProgressRecord {
    pub slot: i64,
    pub content_coord: String,
    pub summary: Option<String>,
    pub updated_at: String,
}

impl EbookDb {
    pub fn open(root_path: &Path, ebook_path: &str) -> Result<Self, String> {
        let db_path = ebook_meta_db_path(root_path, ebook_path);
        let conn = rusqlite::Connection::open(&db_path)
            .map_err(|e| format!("open ebook db: {}", e))?;
        Ok(EbookDb { conn })
    }

    pub fn list_books(&self) -> Result<Vec<BookRecord>, String> {
        let mut stmt = self.conn
            .prepare(
                "SELECT id, path, format, title, file_size, \
                 COALESCE(sha256, ''), category_id, added_at, \
                 COALESCE(scan_status, 'pending'), preview_filename, \
                 txt_cache_filename \
                 FROM books ORDER BY added_at DESC",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                Ok(BookRecord {
                    id: row.get(0)?,
                    source_path: row.get(1)?,
                    format: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    title: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    file_size: row.get(4).unwrap_or(0),
                    sha256: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                    category_id: row.get(6)?,
                    added_at: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
                    scan_status: row.get::<_, Option<String>>(8)?.unwrap_or("pending".to_string()),
                    preview_filename: row.get(9)?,
                    txt_cache_filename: row.get(10)?,
                })
            })
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn get_book(&self, id: &str) -> Result<Option<BookRecord>, String> {
        let mut stmt = self.conn
            .prepare(
                "SELECT id, path, format, title, file_size, \
                 COALESCE(sha256, ''), category_id, added_at, \
                 COALESCE(scan_status, 'pending'), preview_filename, \
                 txt_cache_filename \
                 FROM books WHERE id = ?1",
            )
            .map_err(|e| e.to_string())?;

        let result = stmt
            .query_row(params![id], |row| {
                Ok(BookRecord {
                    id: row.get(0)?,
                    source_path: row.get(1)?,
                    format: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    title: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    file_size: row.get(4).unwrap_or(0),
                    sha256: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                    category_id: row.get(6)?,
                    added_at: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
                    scan_status: row.get::<_, Option<String>>(8)?.unwrap_or("pending".to_string()),
                    preview_filename: row.get(9)?,
                    txt_cache_filename: row.get(10)?,
                })
            });

        match result {
            Ok(book) => Ok(Some(book)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn add_book(
        &self,
        source_path: &str,
        format: &str,
        title: &str,
        file_size: i64,
        sha256: &str,
    ) -> Result<String, String> {
        let id = Uuid::new_v4().to_string();
        self.conn
            .execute(
                "INSERT INTO books (id, path, format, title, file_size, sha256, added_at, scan_status) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'), 'pending')",
                params![id, source_path, format, title, file_size, sha256],
            )
            .map_err(|e| e.to_string())?;
        Ok(id)
    }

    pub fn remove_book(&self, id: &str) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM chapter_index WHERE book_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        self.conn
            .execute("DELETE FROM bookmark WHERE book_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        self.conn
            .execute("DELETE FROM reading_progress WHERE book_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        self.conn
            .execute("DELETE FROM books WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn find_duplicate(
        &self,
        source_path: &str,
        file_size: i64,
        sha256: &str,
    ) -> Result<Option<BookRecord>, String> {
        let mut stmt = self.conn
            .prepare(
                "SELECT id, path, format, title, file_size, \
                 COALESCE(sha256, ''), category_id, added_at, \
                 COALESCE(scan_status, 'pending'), preview_filename, \
                 txt_cache_filename \
                 FROM books WHERE path = ?1 AND file_size = ?2 AND sha256 = ?3",
            )
            .map_err(|e| e.to_string())?;

        let result = stmt
            .query_row(params![source_path, file_size, sha256], |row| {
                Ok(BookRecord {
                    id: row.get(0)?,
                    source_path: row.get(1)?,
                    format: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    title: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    file_size: row.get(4).unwrap_or(0),
                    sha256: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                    category_id: row.get(6)?,
                    added_at: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
                    scan_status: row.get::<_, Option<String>>(8)?.unwrap_or("pending".to_string()),
                    preview_filename: row.get(9)?,
                    txt_cache_filename: row.get(10)?,
                })
            });

        match result {
            Ok(book) => Ok(Some(book)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn update_book_scan_status(&self, id: &str, status: &str) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE books SET scan_status = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![status, id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn update_book_after_scan(
        &self,
        id: &str,
        title: &str,
        file_size: i64,
        sha256: &str,
        scan_status: &str,
        preview_filename: Option<&str>,
        txt_encoding: Option<&str>,
        txt_cache_filename: Option<&str>,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE books SET title = ?1, file_size = ?2, sha256 = ?3, \
                 scan_status = ?4, preview_filename = ?5, txt_encoding = ?6, \
                 txt_cache_filename = ?7, updated_at = datetime('now') \
                 WHERE id = ?8",
                params![title, file_size, sha256, scan_status, preview_filename, txt_encoding, txt_cache_filename, id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Set the cover preview filename for a book (used when the frontend uploads a
    /// rendered cover image). Pass an empty string to clear.
    pub fn update_book_cover(&self, id: &str, preview_filename: &str) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE books SET preview_filename = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![preview_filename, id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn update_book_path(&self, id: &str, new_path: &str) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE books SET path = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![new_path, id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn list_categories(&self) -> Result<Vec<CategoryRecord>, String> {
        let mut stmt = self.conn
            .prepare("SELECT id, name, sort_order FROM category ORDER BY sort_order ASC, name ASC")
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                Ok(CategoryRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    sort_order: row.get(2).unwrap_or(0),
                })
            })
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn add_category(&self, name: &str) -> Result<String, String> {
        let id = Uuid::new_v4().to_string();
        let max_order: i64 = self.conn
            .query_row("SELECT COALESCE(MAX(sort_order), 0) FROM category", [], |r| r.get(0))
            .unwrap_or(0);
        self.conn
            .execute(
                "INSERT INTO category (id, name, sort_order) VALUES (?1, ?2, ?3)",
                params![id, name, max_order + 1],
            )
            .map_err(|e| e.to_string())?;
        Ok(id)
    }

    pub fn rename_category(&self, id: &str, name: &str) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE category SET name = ?1 WHERE id = ?2",
                params![name, id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn remove_category(&self, id: &str) -> Result<(), String> {
        self.conn
            .execute("UPDATE books SET category_id = NULL WHERE category_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        self.conn
            .execute("DELETE FROM category WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn move_book(&self, book_id: &str, category_id: Option<&str>) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE books SET category_id = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![category_id, book_id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn list_chapters(&self, book_id: &str) -> Result<Vec<ChapterRecord>, String> {
        let mut stmt = self.conn
            .prepare(
                "SELECT chapter_no, title, location \
                 FROM chapter_index WHERE book_id = ?1 ORDER BY chapter_no ASC",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![book_id], |row| {
                Ok(ChapterRecord {
                    chapter_no: row.get(0)?,
                    title: row.get(1)?,
                    location: row.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn replace_chapters(&self, book_id: &str, chapters: &[(Option<String>, String)]) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM chapter_index WHERE book_id = ?1", params![book_id])
            .map_err(|e| e.to_string())?;

        for (i, (title, location)) in chapters.iter().enumerate() {
            self.conn
                .execute(
                    "INSERT INTO chapter_index (book_id, chapter_no, title, location) VALUES (?1, ?2, ?3, ?4)",
                    params![book_id, i as i64 + 1, title, location],
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn list_bookmarks(&self, book_id: &str) -> Result<Vec<BookmarkRecord>, String> {
        let mut stmt = self.conn
            .prepare(
                "SELECT id, content_coord, summary, created_at \
                 FROM bookmark WHERE book_id = ?1 AND bookmark_type = 'free' ORDER BY created_at ASC",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![book_id], |row| {
                Ok(BookmarkRecord {
                    id: row.get(0)?,
                    content_coord: row.get(1)?,
                    summary: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn add_bookmark(
        &self,
        book_id: &str,
        content_coord: &str,
        summary: Option<&str>,
    ) -> Result<String, String> {
        let id = Uuid::new_v4().to_string();
        self.conn
            .execute(
                "INSERT INTO bookmark (id, book_id, bookmark_type, content_coord, summary, created_at, updated_at) \
                 VALUES (?1, ?2, 'free', ?3, ?4, datetime('now'), datetime('now'))",
                params![id, book_id, content_coord, summary],
            )
            .map_err(|e| e.to_string())?;
        Ok(id)
    }

    pub fn remove_bookmark(&self, id: &str) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM bookmark WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn save_progress(
        &self,
        book_id: &str,
        slot: i32,
        content_coord: &str,
        summary: Option<&str>,
    ) -> Result<(), String> {
        let bookmark_type = format!("progress_{}", slot);
        let existing: Option<String> = self.conn
            .query_row(
                "SELECT id FROM bookmark WHERE book_id = ?1 AND bookmark_type = ?2",
                params![book_id, bookmark_type],
                |row| row.get(0),
            )
            .ok();

        match existing {
            Some(id) => {
                self.conn
                    .execute(
                        "UPDATE bookmark SET content_coord = ?1, summary = ?2, updated_at = datetime('now') WHERE id = ?3",
                        params![content_coord, summary, id],
                    )
                    .map_err(|e| e.to_string())?;
            }
            None => {
                let id = Uuid::new_v4().to_string();
                self.conn
                    .execute(
                        "INSERT INTO bookmark (id, book_id, bookmark_type, content_coord, summary, created_at, updated_at) \
                         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'), datetime('now'))",
                        params![id, book_id, bookmark_type, content_coord, summary],
                    )
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    pub fn get_progress(&self, book_id: &str) -> Result<Vec<ProgressRecord>, String> {
        let mut stmt = self.conn
            .prepare(
                "SELECT bookmark_type, content_coord, summary, updated_at \
                 FROM bookmark WHERE book_id = ?1 AND bookmark_type LIKE 'progress_%' \
                 ORDER BY updated_at DESC",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![book_id], |row| {
                let bt: String = row.get(0)?;
                let slot: i64 = bt
                    .strip_prefix("progress_")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                Ok(ProgressRecord {
                    slot,
                    content_coord: row.get(1)?,
                    summary: row.get(2)?,
                    updated_at: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                })
            })
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }
}

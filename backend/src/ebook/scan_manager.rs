use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::io::Read;

use crate::ebook::ebook_db::EbookDb;
use crate::ebook::txt_parser;
use crate::ebook::epub_parser;
use crate::ebook::pdf_parser;

pub struct ScanTask {
    pub total: AtomicU64,
    pub completed: AtomicU64,
    pub failed: AtomicU64,
    pub current_book: RwLock<Option<String>>,
    pub is_running: AtomicBool,
}

pub struct ScanProgressResponse {
    pub is_running: bool,
    pub total: u64,
    pub completed: u64,
    pub failed: u64,
    pub current_book: Option<String>,
}

pub struct EbookScanManager {
    running_tasks: Arc<RwLock<HashMap<String, Arc<ScanTask>>>>,
}

impl EbookScanManager {
    pub fn new() -> Self {
        EbookScanManager {
            running_tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn start_metadata_scan(
        &self,
        batch_id: &str,
        books: Vec<(String, String, String)>,
        root_path: String,
        ebook_path: String,
        data_dir: PathBuf,
    ) {
        let task = Arc::new(ScanTask {
            total: AtomicU64::new(books.len() as u64),
            completed: AtomicU64::new(0),
            failed: AtomicU64::new(0),
            current_book: RwLock::new(None),
            is_running: AtomicBool::new(true),
        });

        {
            let mut tasks = self.running_tasks.write().unwrap();
            tasks.insert(batch_id.to_string(), Arc::clone(&task));
        }

        let task_clone = Arc::clone(&task);

        tokio::spawn(async move {
            for (book_id, source_path_abs, format) in &books {
                {
                    let mut current = task_clone.current_book.write().unwrap();
                    *current = Some(source_path_abs.clone());
                }

                let result = process_single_book(
                    book_id,
                    source_path_abs,
                    format,
                    &root_path,
                    &ebook_path,
                    &data_dir,
                );

                match result {
                    Ok(_) => {
                        task_clone.completed.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(_) => {
                        task_clone.failed.fetch_add(1, Ordering::SeqCst);
                        if let Ok(db) = EbookDb::open(Path::new(&root_path), &ebook_path) {
                            let _ = db.update_book_scan_status(book_id, "failed");
                        }
                    }
                }
            }

            task_clone.is_running.store(false, Ordering::SeqCst);
        });
    }

    pub fn get_progress(&self, batch_id: &str) -> Option<ScanProgressResponse> {
        let tasks = self.running_tasks.read().unwrap();
        tasks.get(batch_id).map(|task| ScanProgressResponse {
            is_running: task.is_running.load(Ordering::SeqCst),
            total: task.total.load(Ordering::SeqCst),
            completed: task.completed.load(Ordering::SeqCst),
            failed: task.failed.load(Ordering::SeqCst),
            current_book: task.current_book.read().unwrap().clone(),
        })
    }
}

fn process_single_book(
    book_id: &str,
    source_path_abs: &str,
    format: &str,
    root_path: &str,
    ebook_path: &str,
    data_dir: &Path,
) -> Result<(), String> {
    let file_path = Path::new(source_path_abs);
    if !file_path.exists() {
        return Err("file not found".to_string());
    }

    let db = EbookDb::open(Path::new(root_path), ebook_path)?;
    db.update_book_scan_status(book_id, "scanning")?;

    let (sha256, file_size) = compute_file_hash_and_size(file_path)?;

    let previews_dir = data_dir.join("previews");
    let cache_dir = data_dir.join("cache");
    std::fs::create_dir_all(&previews_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;

    match format {
        "txt" => {
            let encoding = txt_parser::detect_encoding(file_path)?;
            let cache_filename = format!("{}.txt", book_id);
            let cache_path = cache_dir.join(&cache_filename);
            let total_chars = txt_parser::transcode_to_utf8(file_path, &encoding, &cache_path)?;

            let utf8_content = std::fs::read_to_string(&cache_path).map_err(|e| e.to_string())?;
            let chapters = txt_parser::split_chapters(&utf8_content);
            let chapter_data: Vec<(Option<String>, String)> = chapters
                .iter()
                .map(|(title, offset)| (title.clone(), offset.to_string()))
                .collect();

            // TXT 天然无图，不生成占位封面；has_preview=false，
            // 前端以「背景色 + 书名」模式显示（与无预览书籍一致）。
            let title = file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string();

            db.update_book_after_scan(
                book_id,
                &title,
                file_size,
                &sha256,
                "ready",
                None,
                Some(&encoding),
                Some(&cache_filename),
            )?;
            db.replace_chapters(book_id, &chapter_data)?;

            let _ = total_chars;
        }
        "epub" => {
            let result = epub_parser::parse_epub(file_path)?;
            let chapter_data: Vec<(Option<String>, String)> = result.chapters
                .iter()
                .map(|(title, spine_id)| (title.clone(), spine_id.clone()))
                .collect();

            let preview_filename = format!("{}.png", book_id);
            let preview_path = previews_dir.join(&preview_filename);
            if let Some(cover_data) = &result.cover_image {
                std::fs::write(&preview_path, cover_data).map_err(|e| e.to_string())?;
            } else {
                generate_placeholder_png(&file_path, &preview_path)?;
            }

            db.update_book_after_scan(
                book_id,
                &result.title,
                file_size,
                &sha256,
                "ready",
                Some(&preview_filename),
                None,
                None,
            )?;
            db.replace_chapters(book_id, &chapter_data)?;
        }
        "pdf" => {
            let result = pdf_parser::parse_pdf(file_path)?;
            let chapter_data: Vec<(Option<String>, String)> = result.chapters
                .iter()
                .map(|(title, page)| (title.clone(), page.clone()))
                .collect();

            // PDF 封面由前端渲染首页后上传（/api/ebook/shelf/cover），
            // 后端不生成预览图，cover_path 置空，has_preview=false。
            db.update_book_after_scan(
                book_id,
                &result.title,
                file_size,
                &sha256,
                "ready",
                None,
                None,
                None,
            )?;
            db.replace_chapters(book_id, &chapter_data)?;
        }
        _ => {
            db.update_book_scan_status(book_id, "failed")?;
            return Err(format!("unsupported format: {}", format));
        }
    }

    Ok(())
}

fn compute_file_hash_and_size(file_path: &Path) -> Result<(String, i64), String> {
    use sha2::{Sha256, Digest};
    use std::io::BufReader;

    let file = std::fs::File::open(file_path).map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    let file_size = metadata.len() as i64;

    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    let hash = hasher.finalize();
    Ok((hex::encode(hash), file_size))
}

fn generate_placeholder_png(file_path: &Path, output_path: &Path) -> Result<(), String> {
    use image::{ImageBuffer, Rgb, ImageEncoder};

    let title = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown");

    let width = 120u32;
    let height = 160u32;

    let hash = title.bytes().fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    let r = ((hash >> 16) & 0xFF) as u8;
    let g = ((hash >> 8) & 0xFF) as u8;
    let b = (hash & 0xFF) as u8;

    let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(width, height, |_, _| {
        Rgb([r, g, b])
    });

    let mut buf = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut buf);
    encoder.write_image(img.as_raw(), width, height, image::ExtendedColorType::Rgb8)
        .map_err(|e| e.to_string())?;

    std::fs::write(output_path, &buf).map_err(|e| e.to_string())?;
    Ok(())
}

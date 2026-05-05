use actix_web::{web, HttpRequest, HttpResponse, Responder};
use actix_web::http::header::{ContentDisposition, DispositionParam, DispositionType, ExtendedValue, Charset};
use async_zip::{Compression, ZipEntryBuilder};
use futures_lite::io::AsyncWriteExt;
use serde::Deserialize;
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::AsyncReadExt;
use walkdir::WalkDir;

use crate::app_state::AppState;
use crate::handlers::{
    get_user_root_path, is_path_under_root, is_safe_path, ApiResponse,
};

const CHUNK_SIZE: usize = 64 * 1024;

#[derive(Debug, Deserialize)]
pub struct DownloadFolderRequest {
    pub path: String,
}

fn attachment_content_disposition(filename: &str) -> ContentDisposition {
    ContentDisposition {
        disposition: DispositionType::Attachment,
        parameters: vec![
            DispositionParam::Filename(filename.to_string()),
            DispositionParam::FilenameExt(ExtendedValue {
                charset: Charset::Ext("UTF-8".to_string()),
                language_tag: None,
                value: filename.as_bytes().to_vec(),
            }),
        ],
    }
}

struct AsyncChannelWriter {
    tx: tokio::sync::mpsc::Sender<Result<bytes::Bytes, String>>,
    buf: Vec<u8>,
}

impl AsyncChannelWriter {
    fn new(tx: tokio::sync::mpsc::Sender<Result<bytes::Bytes, String>>) -> Self {
        Self {
            tx,
            buf: Vec::with_capacity(CHUNK_SIZE),
        }
    }

    fn flush_buf(&mut self, cx: &mut Context<'_>) -> std::io::Result<Poll<()>> {
        if self.buf.is_empty() {
            return Ok(Poll::Ready(()));
        }
        let data: Vec<u8> = self.buf.iter().copied().collect();
        match self.tx.try_send(Ok(bytes::Bytes::from(data))) {
            Ok(()) => {
                self.buf.clear();
                Ok(Poll::Ready(()))
            }
            Err(tokio::sync::mpsc::error::TrySendError::Full(Ok(bytes))) => {
                self.buf.clear();
                self.buf.extend_from_slice(&bytes);
                cx.waker().wake_by_ref();
                Ok(Poll::Pending)
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "channel closed"))
            }
            Err(tokio::sync::mpsc::error::TrySendError::Full(Err(_))) => unreachable!(),
        }
    }
}

impl tokio::io::AsyncWrite for AsyncChannelWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        if this.buf.len() >= CHUNK_SIZE {
            match this.flush_buf(cx)? {
                Poll::Ready(()) => {}
                Poll::Pending => return Poll::Pending,
            }
        }
        this.buf.extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        match this.flush_buf(cx)? {
            Poll::Ready(()) => Poll::Ready(Ok(())),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        self.poll_flush(cx)
    }
}

pub async fn download_folder(
    body: web::Json<DownloadFolderRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };
    let root_path_obj = Path::new(&root_path);
    let target_path = root_path_obj.join(&body.path);

    if body.path.is_empty() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    if !is_safe_path(&body.path) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    if !target_path.exists() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PATH_NOT_FOUND".to_string()),
        });
    }

    if !is_path_under_root(&target_path, root_path_obj) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PATH_NOT_FOUND".to_string()),
        });
    }

    if !target_path.is_dir() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("NOT_A_DIRECTORY".to_string()),
        });
    }

    let folder_name = target_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("folder")
        .to_string();

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<bytes::Bytes, String>>(8);
    let target_path_clone = target_path.clone();

    tokio::spawn(async move {
        if let Err(e) = compress_folder(&target_path_clone, tx.clone()).await {
            let _ = tx.send(Err(e)).await;
        }
    });

    let filename = format!("{}.zip", folder_name);
    let stream = async_stream::stream! {
        let mut rx = rx;
        while let Some(item) = rx.recv().await {
            match item {
                Ok(bytes) => yield Ok::<bytes::Bytes, actix_web::Error>(bytes),
                Err(e) => {
                    yield Err(actix_web::error::ErrorInternalServerError(e));
                    break;
                }
            }
        }
    };

    HttpResponse::Ok()
        .content_type("application/zip")
        .insert_header(attachment_content_disposition(&filename))
        .streaming(stream)
}

pub async fn compress_folder(
    source_path: &Path,
    tx: tokio::sync::mpsc::Sender<Result<bytes::Bytes, String>>,
) -> Result<(), String> {
    let writer = AsyncChannelWriter::new(tx);
    let mut zip_writer = async_zip::tokio::write::ZipFileWriter::with_tokio(writer);

    let file_entries: Vec<_> = WalkDir::new(source_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .collect();

    for entry in &file_entries {
        let path = entry.path();
        let name = path
            .strip_prefix(source_path)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");

        if entry.file_type().is_dir() {
            if !name.is_empty() {
                let dir_name = format!("{}/", name);
                let builder = ZipEntryBuilder::new(dir_name.into(), Compression::Stored);
                zip_writer.write_entry_whole(builder, &[])
                    .await
                    .map_err(|e: async_zip::error::ZipError| e.to_string())?;
            }
        } else if entry.file_type().is_file() {
            let builder = ZipEntryBuilder::new(name.into(), Compression::Deflate);
            let mut entry_writer = zip_writer.write_entry_stream(builder)
                .await
                .map_err(|e: async_zip::error::ZipError| e.to_string())?;

            let mut file = tokio::fs::File::open(path)
                .await
                .map_err(|e| format!("Failed to open file: {}", e))?;
            let mut buf = vec![0u8; CHUNK_SIZE];
            loop {
                let bytes_read: usize = file.read(&mut buf)
                    .await
                    .map_err(|e| format!("Failed to read file: {}", e))?;
                if bytes_read == 0 {
                    break;
                }
                entry_writer.write_all(&buf[..bytes_read])
                    .await
                    .map_err(|e: std::io::Error| e.to_string())?;
            }

            entry_writer.close()
                .await
                .map_err(|e: async_zip::error::ZipError| e.to_string())?;
        }
    }

    let mut inner = zip_writer.close()
        .await
        .map_err(|e: async_zip::error::ZipError| e.to_string())?;

    inner.flush().await.map_err(|e: std::io::Error| e.to_string())?;

    Ok(())
}

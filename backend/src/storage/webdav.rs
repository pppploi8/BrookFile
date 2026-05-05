use crate::storage::{FileInfo, StorageBackend, StorageError};
use async_trait::async_trait;
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

pub struct WebDAVBackend {
    base_url: String,
    username: String,
    password: String,
    base_path: String,
    client: reqwest::Client,
}

impl WebDAVBackend {
    pub fn new(base_url: String, username: String, password: String, base_path: String) -> Self {
        let base_url = base_url.trim_end_matches('/').to_string();
        let base_path = base_path.trim_end_matches('/').to_string();
        WebDAVBackend {
            base_url,
            username,
            password,
            base_path,
            client: reqwest::Client::new(),
        }
    }

    fn build_url(&self, path: &str) -> String {
        let path = path.trim_start_matches('/');
        if self.base_path.is_empty() {
            format!("{}/{}", self.base_url, path)
        } else {
            format!("{}/{}/{}", self.base_url, self.base_path.trim_start_matches('/'), path)
        }
    }

    fn check_status(status: reqwest::StatusCode, context: &str) -> Result<(), StorageError> {
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(StorageError::AuthError(format!("{} failed: {}", context, status)));
        }
        if status.is_client_error() || status.is_server_error() {
            return Err(StorageError::Other(format!("{} failed: {}", context, status)));
        }
        Ok(())
    }

    async fn ensure_parent_dirs(&self, remote_path: &str) -> Result<(), StorageError> {
        let parts: Vec<&str> = remote_path.split('/').filter(|s| !s.is_empty()).collect();
        if parts.len() < 2 {
            return Ok(());
        }
        for i in 0..parts.len() - 1 {
            let dir_path = parts[..=i].join("/");
            let url = self.build_url(&dir_path);
            let resp = self.client
                .request(reqwest::Method::from_bytes(b"MKCOL").unwrap(), &url)
                .basic_auth(&self.username, Some(&self.password))
                .send()
                .await
                .map_err(|e| StorageError::ConnectionError(e.to_string()))?;
            let status = resp.status();
            if status == reqwest::StatusCode::CREATED
                || status == reqwest::StatusCode::METHOD_NOT_ALLOWED
                || status == reqwest::StatusCode::OK
                || status.as_u16() == 423
            {
                continue;
            }
            Self::check_status(status, "MKCOL")?;
        }
        Ok(())
    }
}

#[async_trait]
impl StorageBackend for WebDAVBackend {
    async fn list_files(&self, path: &str) -> Result<Vec<FileInfo>, StorageError> {
        let url = self.build_url(path);
        let depth_header = "1";

        let body = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:">
  <d:prop>
    <d:resourcetype/>
  </d:prop>
</d:propfind>"#;

        let resp = self.client
            .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &url)
            .header("Depth", depth_header)
            .header("Content-Type", "application/xml; charset=utf-8")
            .basic_auth(&self.username, Some(&self.password))
            .body(body)
            .send()
            .await
            .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        Self::check_status(resp.status(), "PROPFIND")?;

        let response_text = resp
            .text()
            .await
            .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        let mut reader = quick_xml::Reader::from_str(&response_text);
        reader.config_mut().trim_text(true);

        let mut files = Vec::new();
        let mut in_response = false;
        let mut in_href = false;
        let mut in_resourcetype = false;
        let mut is_collection = false;
        let mut current_href = String::new();

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Start(ref e)) | Ok(quick_xml::events::Event::Empty(ref e)) => {
                    let local = e.local_name();
                    let local_str = local.as_ref();
                    match local_str {
                        b"response" => {
                            in_response = true;
                            current_href.clear();
                            is_collection = false;
                        }
                        b"href" if in_response => in_href = true,
                        b"resourcetype" if in_response => in_resourcetype = true,
                        b"collection" if in_resourcetype => is_collection = true,
                        _ => {}
                    }
                }
                Ok(quick_xml::events::Event::End(ref e)) => {
                    let local = e.local_name();
                    let local_str = local.as_ref();
                    match local_str {
                        b"response" => {
                            in_response = false;
                            in_resourcetype = false;
                            if !current_href.is_empty() {
                                let href_path = current_href.trim_end_matches('/');
                                let requested_path = url.trim_end_matches('/');
                                let decoded_requested = urlencoding::decode(requested_path)
                                    .unwrap_or_else(|_| requested_path.into())
                                    .into_owned();
                                let decoded_href = urlencoding::decode(href_path)
                                    .unwrap_or_else(|_| href_path.into())
                                    .into_owned();
                                let decoded_href_trimmed = decoded_href.trim_end_matches('/');
                                let decoded_requested_trimmed = decoded_requested.trim_end_matches('/');
                                if decoded_href_trimmed != decoded_requested_trimmed {
                                    let name = decoded_href_trimmed
                                        .rsplit('/')
                                        .next()
                                        .unwrap_or("")
                                        .to_string();
                                    if !name.is_empty() {
                                        files.push(FileInfo {
                                            path: name,
                                            is_dir: is_collection,
                                        });
                                    }
                                }
                            }
                        }
                        b"href" => in_href = false,
                        b"resourcetype" => in_resourcetype = false,
                        _ => {}
                    }
                }
                Ok(quick_xml::events::Event::Text(ref e)) => {
                    if in_href {
                        current_href = e.unescape().map(|s| s.to_string()).unwrap_or_default();
                    }
                }
                Ok(quick_xml::events::Event::Eof) => break,
                Err(e) => return Err(StorageError::Other(format!("XML parse error: {}", e))),
                _ => {}
            }
            buf.clear();
        }

        Ok(files)
    }

    async fn upload_stream(
        &self,
        remote_path: &str,
        total_size: u64,
        mut data_receiver: tokio::sync::mpsc::Receiver<Vec<u8>>,
        progress: crate::storage::ProgressCallback,
    ) -> Result<(), StorageError> {
        let temp_id = Uuid::new_v4().to_string();
        let temp_path = std::env::temp_dir().join(format!("webdav_upload_{}.tmp", temp_id));

        {
            let mut file = tokio::fs::File::create(&temp_path).await
                .map_err(|e| StorageError::Other(e.to_string()))?;
            let mut total_written: u64 = 0;
            while let Some(chunk) = data_receiver.recv().await {
                total_written += chunk.len() as u64;
                progress(total_written, total_size);
                AsyncWriteExt::write_all(&mut file, &chunk).await
                    .map_err(|e| StorageError::Other(e.to_string()))?;
            }
            AsyncWriteExt::flush(&mut file).await
                .map_err(|e| StorageError::Other(e.to_string()))?;
        }

        let file_size = tokio::fs::metadata(&temp_path).await
            .map(|m| m.len())
            .unwrap_or(0);

        self.ensure_parent_dirs(remote_path).await?;
        let url = self.build_url(remote_path);

        let file = tokio::fs::File::open(&temp_path).await
            .map_err(|e| StorageError::Other(e.to_string()))?;
        let body = reqwest::Body::wrap_stream(tokio_util::io::ReaderStream::new(file));

        let resp = self.client
            .put(&url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Content-Length", file_size)
            .body(body)
            .send()
            .await
            .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        let _ = tokio::fs::remove_file(&temp_path).await;

        Self::check_status(resp.status(), "PUT")?;
        Ok(())
    }

    async fn download_file(&self, remote_path: &str) -> Result<Vec<u8>, StorageError> {
        let url = self.build_url(remote_path);

        let resp = self.client
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        Self::check_status(resp.status(), "GET")?;

        let data = resp
            .bytes()
            .await
            .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        Ok(data.to_vec())
    }

    async fn download_stream(
        &self,
        remote_path: &str,
        data_sender: tokio::sync::mpsc::Sender<Vec<u8>>,
        progress: crate::storage::ProgressCallback,
    ) -> Result<u64, StorageError> {
        let url = self.build_url(remote_path);

        let resp = self.client
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        Self::check_status(resp.status(), "GET")?;

        let total_size: u64 = resp
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let mut downloaded: u64 = 0;
        let mut byte_stream = resp.bytes_stream();

        while let Some(chunk_result) = byte_stream.next().await {
            let chunk = chunk_result
                .map_err(|e| StorageError::ConnectionError(e.to_string()))?;
            let chunk_len = chunk.len() as u64;
            downloaded += chunk_len;
            progress(downloaded, total_size);

            if data_sender.send(chunk.to_vec()).await.is_err() {
                break;
            }
        }

        Ok(downloaded)
    }

    async fn delete_file(&self, remote_path: &str) -> Result<(), StorageError> {
        let url = self.build_url(remote_path);

        let resp = self.client
            .delete(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(());
        }

        Self::check_status(resp.status(), "DELETE")?;
        Ok(())
    }

    async fn move_file(&self, from: &str, to: &str) -> Result<(), StorageError> {
        self.ensure_parent_dirs(to).await?;

        let from_url = self.build_url(from);
        let to_url = self.build_url(to);

        let resp = self.client
            .request(reqwest::Method::from_bytes(b"MOVE").unwrap(), &from_url)
            .header("Destination", &to_url)
            .header("Overwrite", "T")
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        let status = resp.status();
        if status == reqwest::StatusCode::CREATED || status == reqwest::StatusCode::NO_CONTENT || status == reqwest::StatusCode::OK {
            return Ok(());
        }

        Self::check_status(status, "MOVE")?;
        Ok(())
    }

    async fn mkdir(&self, path: &str) -> Result<(), StorageError> {
        let url = self.build_url(path);

        let resp = self.client
            .request(reqwest::Method::from_bytes(b"MKCOL").unwrap(), &url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .map_err(|e| StorageError::ConnectionError(e.to_string()))?;

        let status = resp.status();
        if status == reqwest::StatusCode::CREATED
            || status == reqwest::StatusCode::METHOD_NOT_ALLOWED
            || status == reqwest::StatusCode::OK
            || status.as_u16() == 423
        {
            return Ok(());
        }

        Self::check_status(status, "MKCOL")?;
        Ok(())
    }
}

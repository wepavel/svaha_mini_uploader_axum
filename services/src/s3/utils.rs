use bytes::{Bytes, BytesMut};
use tokio::io::{AsyncRead, AsyncReadExt};
use super::errors::Result;
use std::path::Path;

/// Максимальный размер буфера для чтения (5MB)
pub const DEFAULT_CHUNK_SIZE: usize = 5 * 1024 * 1024;

/// Создает уникальное имя файла с заданным префиксом и расширением
pub fn generate_unique_filename(prefix: &str, extension: &str) -> String {
    let timestamp = chrono::Utc::now().timestamp();
    let random = rand::random::<u16>();
    format!("{prefix}_{timestamp}_{random}{extension}")
}

/// Преобразует AsyncRead в вектор частей заданного размера
pub async fn stream_to_chunks<R>(
    mut reader: R,
    chunk_size: usize
) -> Result<Vec<Bytes>>
where
    R: AsyncRead + Unpin
{
    let mut chunks = Vec::new();
    let mut buffer = BytesMut::with_capacity(chunk_size);

    loop {
        let bytes_read = reader.read_buf(&mut buffer).await?;
        if bytes_read == 0 {
            break;
        }

        if buffer.len() >= chunk_size {
            chunks.push(buffer.split_to(chunk_size).freeze());
        }
    }

    // Остаток
    if !buffer.is_empty() {
        chunks.push(buffer.freeze());
    }

    Ok(chunks)
}

/// Извлекает расширение файла из имени
pub fn get_file_extension(filename: &str) -> Option<&str> {
    Path::new(filename)
        .extension()
        .and_then(|ext| ext.to_str())
}

/// Определяет MIME-тип по расширению файла
pub fn get_mime_type(filename: &str) -> String {
    if let Some(ext) = get_file_extension(filename) {
        match ext.to_lowercase().as_str() {
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "mp3" => "audio/mpeg",
            "wav" => "audio/wav",
            "ogg" => "audio/ogg",
            "flac" => "audio/flac",
            "m4a" => "audio/m4a",
            "mp4" => "video/mp4",
            "webm" => "video/webm",
            "txt" => "text/plain",
            "pdf" => "application/pdf",
            "json" => "application/json",
            _ => "application/octet-stream",
        }
            .to_string()
    } else {
        "application/octet-stream".to_string()
    }
}

use serde::{Deserialize, Serialize};
use serde_json::json;

use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_swagger_ui::SwaggerUi;

use axum::response::IntoResponse;
use axum::{
    http::{StatusCode},
    body::Bytes,
    extract::{Multipart, State},
    handler::Handler
};
use tokio_util::io::ReaderStream;
use std::time::Duration;
use axum::Json;

use tower_http::{classify::ServerErrorsFailureClass, trace::TraceLayer};
use tracing::{info_span, Span};
use bytes::{Bytes as BBytes, BytesMut};
use crate::custom_exceptions::{JsonResponse, ErrorCode, BadResponseObject, PlainTextResponse};
use once_cell::sync::Lazy;
use crate::try_json;

use services::{AppState, s3::S3Manager};
use std::sync::Arc;

const TAG: &str = "Upload";
pub fn get_router(app_state: Arc<AppState>) -> OpenApiRouter {
    OpenApiRouter::new()
        .routes(routes!(upload_tracks))
        .with_state(app_state)
}

const CHUNK_SIZE: usize = 1024 * 1024 * 5; // 5 MB chunks, adjust as needed
static ALLOWED_EXTENSIONS: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![".ogg", ".mp3", ".wav", ".flac", ".m4a", "",]
});

/// Just a schema for axum native multipart
#[derive(Deserialize, ToSchema)]
#[allow(unused)]
struct UploadTracksForm {
    #[schema(format = Binary, content_media_type = "application/octet-stream")]
    vocal: String,
    #[schema(format = Binary, content_media_type = "application/octet-stream")]
    instrumental: String,
    // name: String,
}

#[derive(Deserialize, Serialize, ToSchema, Default)]
#[schema(example = json!({
    "vocal_name": "vocal.mp3",
    "vocal_size": 1024,
    "instrumental_name": "instrumental.mp3",
    "instrumental_size": 1024
}))]
struct FilesRespForm {
    vocal_name: String,
    vocal_size: u64,
    instrumental_name: String,
    instrumental_size: u64,
}

/// Результат загрузки файла
struct FileUploadResult {
    name: String,
    size: u64,
}


#[utoipa::path(
    post,
    path = "/upload-tracks",
    tag = TAG,
    description = "Endpoint for uploading two files: vocal and instrumental",
    request_body(content = UploadTracksForm, content_type = "multipart/form-data", description = "Hello guys!"),
    responses(
        (status = 200, body = FilesRespForm, description = "Tracks uploaded successfully!"),
        (status = 400, description = "Bad request", body = BadResponseObject, example = json!(BadResponseObject::default_400())),
        (status = 500, description = "Internal server error", body = BadResponseObject, example = json!(BadResponseObject::default_500())),
    ),
)]
pub async fn upload_tracks(
    State(app_state): State<Arc<AppState>>,
    mut multipart: Multipart
) -> JsonResponse {
    let s3 = &app_state.s3;
    let bucket = "svaha-mini-input";

    let mut vocal_result: Option<FileUploadResult> = None;
    let mut instrumental_result: Option<FileUploadResult> = None;



    // Обрабатываем каждую часть формы
    while let Some(field) = try_json!(multipart.next_field().await.map_err(|_| {
        ErrorCode::CoreFileUploadingError.details()
    })) {
        let name = field.name().unwrap_or_default();

        match name {
            "vocal" => {
                vocal_result = Some(try_json!(upload_file(s3, bucket, field).await));
            }
            "instrumental" => {
                instrumental_result = Some(try_json!(upload_file(s3, bucket, field).await));
            }
            _ => {
                return ErrorCode::CoreFileUploadingError.into();
            }
        }
    }

    // Проверяем, что оба файла загружены
    if vocal_result.is_none() || instrumental_result.is_none() {
        return ErrorCode::CoreFileUploadingError.into();
    }

    // Формируем ответ
    let vocal = vocal_result.unwrap();
    let instrumental = instrumental_result.unwrap();

    let response = FilesRespForm {
        vocal_name: vocal.name,
        vocal_size: vocal.size,
        instrumental_name: instrumental.name,
        instrumental_size: instrumental.size,
    };

    JsonResponse::Ok(json!(response))
}

/// Функция для загрузки файла в S3
async fn upload_file(
    s3: &S3Manager,
    bucket: &str,
    mut field: axum::extract::multipart::Field<'_>,
) -> Result<FileUploadResult, ErrorCode> {
    // Получаем имя файла из поля
    let file_name = field.file_name()
        .map(ToString::to_string)
        .ok_or_else(|| {
            tracing::error!("Missing file name");
            ErrorCode::CoreFileUploadingError
        })?;

    // Формируем путь в S3
    let path = format!("test/{file_name}");

    // Создаем контекст для многочастной загрузки
    let mut upload_context = s3.create_multipart_upload_context(bucket, &path, None).await
        .map_err(|err| {
            tracing::error!("Failed to create multipart upload context: {}", err);
            ErrorCode::CoreFileUploadingError
        })?;

    // Буфер для чтения данных
    let mut buffer = BytesMut::new();
    let mut part_number = 1;
    let mut total_size = 0u64;

    // Читаем чанки данных из поля формы
    while let Some(chunk) = field.chunk().await.map_err(|err| {
        tracing::error!("Error reading chunk: {}", err);
        ErrorCode::CoreFileUploadingError
    })? {
        // Добавляем данные в буфер
        buffer.extend_from_slice(&chunk);
        total_size += chunk.len() as u64;

        // Если накопили достаточно данных, отправляем часть
        while buffer.len() >= CHUNK_SIZE {
            let chunk_data = buffer.split_to(CHUNK_SIZE).freeze();
            upload_context.upload_part(part_number, chunk_data).await
                .map_err(|err| {
                    tracing::error!("Failed to upload part {}: {}", part_number, err);
                    ErrorCode::CoreFileUploadingError
                })?;
            part_number += 1;
        }
    }

    // Отправляем оставшиеся данные, если они есть
    if !buffer.is_empty() {
        let chunk_data = buffer.freeze();
        upload_context.upload_part(part_number, chunk_data).await
            .map_err(|err| {
                tracing::error!("Failed to upload final part {}: {}", part_number, err);
                ErrorCode::CoreFileUploadingError
            })?;
    }

    // Завершаем многочастную загрузку
    upload_context.complete().await
        .map_err(|err| {
            tracing::error!("Failed to complete multipart upload: {}", err);
            ErrorCode::CoreFileUploadingError
        })?;

    // Возвращаем информацию о загруженном файле
    Ok(FileUploadResult {
        name: file_name,
        size: total_size,
    })
}

// Пример функции обработки чанка (замените на вашу логику)
fn process_chunk(chunk: &[u8]) {
    // Выполните здесь нужную обработку чанка
    println!("Processing chunk of size: {}", chunk.len());
}

fn process_fixed_size_chunk(chunk: &Bytes) {
    tracing::info!("Processing fixed-size chunk of size: {}", chunk.len());
    // Здесь вы можете выполнять любую нужную обработку чанка
}
//

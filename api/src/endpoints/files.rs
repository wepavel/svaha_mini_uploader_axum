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
use crate::exceptions::{JsonResponse, ErrorCode, BadResponseObject, PlainTextResponse};
use once_cell::sync::Lazy;


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
    )
)]
pub async fn upload_tracks(
    State(app_state): State<Arc<AppState>>,
    mut multipart: Multipart
) -> JsonResponse {
    let s3 = &app_state.s3;
    let mut vocal_info: Option<(String, u64)> = None;
    let mut instrumental_info: Option<(String, u64)> = None;

    while let Some(field) = multipart.next_field().await.map_err(|err| {
        tracing::error!("Error processing multipart field: {}", err);
        (StatusCode::BAD_REQUEST, Json(json!({"error": err.to_string()})))
    }).unwrap() {
        match field.name() {
            Some("vocal") => {
                vocal_info = Some(process_file_upload(s3, "svaha-mini-input", field).await.unwrap());
            }
            Some("instrumental") => {
                instrumental_info = Some(process_file_upload(s3, "svaha-mini-input", field).await.unwrap());
            }
            _ => {
                tracing::warn!("Unexpected field in multipart form");
            }
        }
    }

    let response = FilesRespForm {
        vocal_name: vocal_info.as_ref().map(|(name, _)| name.clone()).unwrap_or_default(),
        vocal_size: vocal_info.map(|(_, size)| size).unwrap_or_default(),
        instrumental_name: instrumental_info.as_ref().map(|(name, _)| name.clone()).unwrap_or_default(),
        instrumental_size: instrumental_info.map(|(_, size)| size).unwrap_or_default(),
    };

    JsonResponse::Ok(json!(response))
}
// pub async fn upload_tracks(State(app_state): State<Arc<AppState>>, mut multipart: Multipart) -> PlainTextResponse {
//     let s3 = &app_state.s3;
//     let mut name: Option<String> = None;
//     let mut content_type: Option<String> = None;
//     let mut total_size: usize = 0;
//     let mut file_name: Option<String> = None;
//     let mut buffer = BytesMut::new();
//
//     tracing::info!("Sus from endpoint");
//
//     while let Some(mut field) = multipart
//         .next_field()
//         .await
//         .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))
//         .unwrap()
//     {
//         match field.name() {
//             Some("name") => {
//                 name = Some(field.text().await.unwrap_or_default());
//             }
//             Some("vocal") => {
//                 file_name = field.file_name().map(ToString::to_string);
//                 content_type = field.content_type().map(ToString::to_string);
//
//                 while let Some(chunk) = field
//                     .chunk()
//                     .await
//                     .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))
//                     .unwrap()
//                 {
//                     let mut s3_context =
//                         s3.create_multipart_upload_context(
//                             "svaha-mini-input",
//                             &file_name.unwrap_or_default("Sus")
//                         ).await.unwrap();
//
//                     let mut part_number = 1;
//
//                     buffer.extend_from_slice(&chunk);
//                     total_size += chunk.len();
//                     while buffer.len() >= CHUNK_SIZE {
//                         let chunk = buffer.split_to(CHUNK_SIZE).freeze();
//
//                         // process_fixed_size_chunk(&chunk);
//                     }
//                 }
//
//                 // Process any remaining data
//                 if !buffer.is_empty() {
//                     let chunk = buffer.split().freeze();
//                     // process_fixed_size_chunk(&chunk);
//                 }
//             }
//             Some("instrumental") => {
//                 file_name = field.file_name().map(ToString::to_string);
//                 content_type = field.content_type().map(ToString::to_string);
//
//                 while let Some(chunk) = field
//                     .chunk()
//                     .await
//                     .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))
//                     .unwrap()
//                 {
//                     buffer.extend_from_slice(&chunk);
//                     total_size += chunk.len();
//
//                     while buffer.len() >= CHUNK_SIZE {
//                         let chunk = buffer.split_to(CHUNK_SIZE).freeze();
//                         // process_fixed_size_chunk(&chunk);
//                     }
//                 }
//
//                 // Process any remaining data
//                 if !buffer.is_empty() {
//                     let chunk = buffer.split().freeze();
//                     // process_fixed_size_chunk(&chunk);
//                 }
//             }
//             _ => (),
//         }
//     }
//
//     PlainTextResponse::Ok(format!(
//         "name: {}, content_type: {}, total_size: {}, file_name: {}",
//         name.unwrap_or_default(),
//         content_type.unwrap_or_default(),
//         total_size,
//         file_name.unwrap_or_default()
//     ))
// }

async fn process_file_upload(
    s3: &S3Manager,
    bucket: &str,
    mut field: axum::extract::multipart::Field<'_>,
) -> Result<(String, u64), (StatusCode, Json<serde_json::Value>)> {
    let file_name = field.file_name().map(ToString::to_string).ok_or_else(|| {
        tracing::error!("Missing file name");
        (StatusCode::BAD_REQUEST, Json(json!({"error": "Missing file name"})))
    })?;

    let mut upload_context = s3.create_multipart_upload_context(bucket, &file_name).await.map_err(|err| {
        tracing::error!("Failed to create multipart upload context: {}", err);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": err.to_string()})))
    })?;

    let mut buffer = BytesMut::new();
    let mut part_number = 1;
    let mut total_size = 0u64;

    while let Some(chunk) = field.chunk().await.map_err(|err| {
        tracing::error!("Error reading chunk: {}", err);
        (StatusCode::BAD_REQUEST, Json(json!({"error": err.to_string()})))
    })? {
        buffer.extend_from_slice(&chunk);
        total_size += chunk.len() as u64;

        while buffer.len() >= CHUNK_SIZE {
            let chunk = buffer.split_to(CHUNK_SIZE).freeze();
            upload_context.upload_part(part_number, chunk).await.map_err(|err| {
                tracing::error!("Failed to upload part {}: {}", part_number, err);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": err.to_string()})))
            })?;
            part_number += 1;
        }
    }

    if !buffer.is_empty() {
        let chunk = buffer.freeze();
        upload_context.upload_part(part_number, chunk).await.map_err(|err| {
            tracing::error!("Failed to upload final part {}: {}", part_number, err);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": err.to_string()})))
        })?;
    }

    upload_context.complete().await.map_err(|err| {
        tracing::error!("Failed to complete multipart upload: {}", err);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": err.to_string()})))
    })?;

    Ok((file_name, total_size))
}

// Пример функции обработки чанка (замените на вашу логику)
// fn process_chunk(chunk: &[u8]) {
//     // Выполните здесь нужную обработку чанка
//     println!("Processing chunk of size: {}", chunk.len());
// }
//
// fn process_fixed_size_chunk(chunk: &Bytes) {
//     tracing::info!("Processing fixed-size chunk of size: {}", chunk.len());
//     // Здесь вы можете выполнять любую нужную обработку чанка
// }
//

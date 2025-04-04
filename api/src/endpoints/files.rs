use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

use utoipa_swagger_ui::SwaggerUi;

use axum::{
    http::{StatusCode},
    body::Bytes,
    extract::Multipart,
};

use std::time::Duration;
use tokio::net::TcpListener;
use tower_http::{classify::ServerErrorsFailureClass, trace::TraceLayer};
use tracing::{info_span, Span};
use bytes::{Bytes as BBytes, BytesMut};
use crate::exceptions::{JsonResponse, ErrorCode, BadResponseObject};
use once_cell::sync::Lazy;
use serde_json::json;

const TAG: &str = "Upload";
pub fn get_router() -> OpenApiRouter {
    OpenApiRouter::new().routes(routes!(upload_tracks))
}

const CHUNK_SIZE: usize = 1024 * 1024 * 5; // 5 MB chunks, adjust as needed
static ALLOWED_EXTENSIONS: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![
        ".ogg",
        ".mp3",
        ".wav",
        ".flac",
        ".m4a",
        "",
    ]
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
async fn upload_tracks(mut multipart: Multipart) -> String {
    let mut name: Option<String> = None;
    let mut content_type: Option<String> = None;
    let mut total_size: usize = 0;
    let mut file_name: Option<String> = None;
    let mut buffer = BytesMut::new();

    tracing::info!("Sus from endpoint");

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))
        .unwrap()
    {
        match field.name() {
            Some("name") => {
                name = Some(field.text().await.unwrap_or_default());
            }
            Some("vocal") => {
                file_name = field.file_name().map(ToString::to_string);
                content_type = field.content_type().map(ToString::to_string);

                while let Some(chunk) = field
                    .chunk()
                    .await
                    .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))
                    .unwrap()
                {
                    buffer.extend_from_slice(&chunk);
                    total_size += chunk.len();

                    while buffer.len() >= CHUNK_SIZE {
                        let chunk = buffer.split_to(CHUNK_SIZE).freeze();
                        // process_fixed_size_chunk(&chunk);
                    }
                }

                // Process any remaining data
                if !buffer.is_empty() {
                    let chunk = buffer.split().freeze();
                    // process_fixed_size_chunk(&chunk);
                }
            }
            _ => (),
        }
    }

    format!(
        "name: {}, content_type: {}, total_size: {}, file_name: {}",
        name.unwrap_or_default(),
        content_type.unwrap_or_default(),
        total_size,
        file_name.unwrap_or_default()
    )
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


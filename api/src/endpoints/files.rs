
use serde::{Deserialize, Serialize};
use serde_json::json;

use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

use axum::{
    body::Bytes,
    extract::{Multipart, State},

};

use tower_http::{classify::ServerErrorsFailureClass, trace::TraceLayer};
use tracing::{info_span, Span};
use bytes::{Bytes as BBytes, BytesMut};
use crate::custom_exceptions::{JsonResponse, ErrorCode, BadResponseObject};
use once_cell::sync::Lazy;
use crate::{json_err, json_opt};

use services::{AppState, s3::S3Manager};
use std::sync::Arc;


const TAG: &str = "Upload";
pub fn get_router(app_state: Arc<AppState>) -> OpenApiRouter {
    OpenApiRouter::new()
        .routes(routes!(upload_tracks))
        .routes(routes!(upload_track_single))
        .with_state(app_state)
}

const CHUNK_SIZE: usize = 1024 * 1024 * 20; // 5 MB chunks, adjust as needed
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

#[derive(Deserialize, ToSchema)]
#[allow(unused)]
struct UploadTrackForm {
    path: String,
    #[schema(format = Binary, content_media_type = "application/octet-stream")]
    track: String,
}

#[derive(Deserialize, Serialize, ToSchema, Default)]
#[schema(example = json!({
    "vocal_name": "vocal.mp3",
    "vocal_size": 1024,
    "instrumental_name": "instrumental.mp3",
    "instrumental_size": 1024
}))]
struct FilesUploadResult {
    vocal_name: String,
    vocal_size: u64,
    instrumental_name: String,
    instrumental_size: u64,
}

/// Результат загрузки файла
#[derive(Deserialize, Serialize, ToSchema, Default)]
#[schema(example = json!({
    "name": "track.mp3",
    "size": 1024,
}))]
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
        (status = 200, body = FilesUploadResult, description = "Tracks uploaded successfully!"),
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
    while let Some(field) = json_err!(multipart.next_field().await.map_err(|_| {
        ErrorCode::CoreFileUploadingError.details()
    })) {
        let name = field.name().unwrap_or_default();
        let file_name = json_err!(field.file_name()
            .map(ToString::to_string)
            .ok_or_else(|| {
                tracing::error!("Missing file name");
                ErrorCode::ValidationError.details()
            }));

        let path = "test2".to_string();

        match name {
            "vocal" => {
                vocal_result = Some(json_err!(upload_file(s3, bucket, &file_name, &path, field).await));
                // Читаем чанки данных из поля формы
                // json_err!(process_chunk(field).await);

            }
            "instrumental" => {
                instrumental_result = Some(json_err!(upload_file(s3, bucket, &file_name, &path, field).await));
                // json_err!(process_chunk(field).await);
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

    let response = FilesUploadResult {
        vocal_name: vocal.name,
        vocal_size: vocal.size,
        instrumental_name: instrumental.name,
        instrumental_size: instrumental.size,
    };

    JsonResponse::Ok(json!(response))
}


#[utoipa::path(
    post,
    path = "/upload-track-single",
    tag = TAG,
    description = "Endpoint for uploading one file",
    request_body(content = UploadTrackForm, content_type = "multipart/form-data", description = "Upload file body"),
    responses(
        (status = 200, body = FileUploadResult, description = "Track uploaded successfully!"),
        (status = 400, description = "Bad request", body = BadResponseObject, example = json!(BadResponseObject::default_400())),
        (status = 500, description = "Internal server error", body = BadResponseObject, example = json!(BadResponseObject::default_500())),
    ),
)]
pub async fn upload_track_single(
    State(app_state): State<Arc<AppState>>,
    mut multipart: Multipart
) -> JsonResponse {
    let s3 = &app_state.s3;
    let bucket = "svaha-mini-input";

    let mut result: FileUploadResult = FileUploadResult::default();
    let mut path: String = "test".to_string(); // Значение по умолчанию
    let mut filename: Option<String> = None;

    // Обрабатываем каждую часть формы
    while let Some(field) = json_err!(
        multipart.next_field().await, 
        ErrorCode::CoreFileUploadingError.details()
            .with("reason", "Failed to get form field")
    ) {
        let name = field.name().unwrap_or_default();

        match name {
            "track" => {
                // Получаем имя файла из поля
                filename = field.file_name().map(ToString::to_string);
                let file_name = json_opt!(
                    filename.clone(), 
                    ErrorCode::CoreFileUploadingError.details()
                        .with("reason", "Missing file name in the uploaded file")
                );

                // Сохраняем field для последующей обработки после того, как определим все параметры
                // let track_field = field;

                // Если уже собрали все необходимые данные, загружаем файл
                // if let Some(file_name) = &filename {
                //     result = Some(json_err!(upload_file(s3, bucket, file_name, &path, field).await));
                // }
                // return .into();

                result = json_err!(upload_file(s3, bucket, &file_name, &path, field).await);
                // json_err!(process_chunk(field).await);

            }
            "path" => {
                // Читаем значение path как текст
                path = json_err!(
                    field.text().await, 
                    ErrorCode::CoreFileUploadingError.details()
                        .with("reason", "Failed to read path field as text")
                );

                // Проверяем, что путь не пустой
                if path.trim().is_empty() {
                    path = "test".to_string(); // Используем значение по умолчанию
                }
            }
            _ => {
                return ErrorCode::CoreFileUploadingError.details()
                    .with("reason", "Unknown field in multipart form")
                    .with("field_name", name)
                    .into();
            }
        }
    }

    // Возвращаем результат
    JsonResponse::Ok(json!(result))
}


/// Функция для загрузки файла в S3
async fn upload_file(
    s3: &S3Manager,
    bucket: &str,
    filename: &str,
    path: &str,
    mut field: axum::extract::multipart::Field<'_>,
) -> Result<FileUploadResult, BadResponseObject> {
    // Формируем путь в S3sdg
    let path = format!("{path}/{filename}");



    // Создаем контекст для многочастной загрузки
    let mut upload_context = s3.create_multipart_upload_context(bucket, &path, None).await
        .map_err(|err| {
            tracing::error!("Failed to create multipart upload context: {}", err);
            ErrorCode::CoreFileUploadingError.details()
        })?;

    // Буфер для чтения данных
    let mut buffer = BytesMut::new();
    let mut part_number = 1;
    let mut total_size = 0u64;

    // Читаем чанки данных из поля формы
    while let Some(chunk) = field.chunk().await.map_err(|err| {
        tracing::error!("Error reading chunk: {}", err);
        ErrorCode::CoreFileUploadingError.details()
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
                    ErrorCode::CoreFileUploadingError.details()
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
                ErrorCode::CoreFileUploadingError.details()
            })?;
    }

    // Завершаем многочастную загрузку
    upload_context.complete().await
        .map_err(|err| {
            tracing::error!("Failed to complete multipart upload: {}", err);
            ErrorCode::CoreFileUploadingError.details()
        })?;

    // Возвращаем информацию о загруженном файле
    Ok(FileUploadResult {
        name: filename.to_string(),
        size: total_size,
    })
}

/// Функция для неблокирующей загрузки файла в S3
// async fn upload_file(
//     s3: &S3Manager,
//     bucket: &str,
//     filename: &str,
//     path: &str,
//     mut field: axum::extract::multipart::Field<'_>,
// ) -> Result<FileUploadResult, BadResponseObject> {
//     // Формируем путь в S3
//     let path = format!("{path}/{filename}");
// 
//     // Создаем контекст для многочастной загрузки
//     let mut upload_context = s3.create_multipart_upload_context(bucket, &path, None).await
//         .map_err(|err| {
//             tracing::error!("Failed to create multipart upload context: {}", err);
//             ErrorCode::CoreFileUploadingError.details()
//         })?;
// 
//     // Создаем канал для передачи чанков между получением и отправкой
//     let (tx, mut rx) = tokio::sync::mpsc::channel::<(i32, Bytes)>(5); // Буфер на 5 частей
// 
//     // Запускаем отдельную задачу для обработки данных от S3
//     let upload_task = tokio::spawn(async move {
//         while let Some((part_num, data)) = rx.recv().await {
//             // Отправляем часть в S3
//             upload_context.upload_part(part_num, data).await
//                 .map_err(|err| {
//                     tracing::error!("Failed to upload part {}: {}", part_num, err);
//                     ErrorCode::CoreFileUploadingError.details()
//                 })?;
//         }
// 
//         // Все чанки отправлены, завершаем загрузку
//         upload_context.complete().await
//             .map_err(|err| {
//                 tracing::error!("Failed to complete multipart upload: {}", err);
//                 ErrorCode::CoreFileUploadingError.details()
//             })?;
// 
//         Ok::<(), BadResponseObject>(())
//     });
// 
//     let mut part_number = 1;
//     let mut total_size = 0u64;
//     let mut buffer = BytesMut::with_capacity(CHUNK_SIZE);
// 
//     // Читаем чанки данных из поля формы в основном потоке
//     while let Some(chunk) = field.chunk().await.map_err(|err| {
//         tracing::error!("Error reading chunk: {}", err);
//         ErrorCode::CoreFileUploadingError.details()
//     })? {
//         // Добавляем данные в буфер
//         buffer.extend_from_slice(&chunk);
//         total_size += chunk.len() as u64;
// 
//         // Отправляем полные чанки по каналу
//         while buffer.len() >= CHUNK_SIZE {
//             let chunk_data = buffer.split_to(CHUNK_SIZE).freeze();
// 
//             if tx.send((part_number, chunk_data)).await.is_err() {
//                 // Канал закрыт, обработчик завершился с ошибкой
//                 return Err(ErrorCode::CoreFileUploadingError.details());
//             }
// 
//             part_number += 1;
//         }
//     }
// 
//     // Отправляем оставшиеся данные, если они есть
//     if !buffer.is_empty() {
//         let chunk_data = buffer.freeze();
// 
//         if tx.send((part_number, chunk_data)).await.is_err() {
//             return Err(ErrorCode::CoreFileUploadingError.details());
//         }
//     }
// 
//     // Закрываем канал, чтобы upload_task знал, что больше данных не будет
//     drop(tx);
// 
//     // Ждем завершения отправки
//     upload_task.await
//         .map_err(|_| {
//             tracing::error!("Uploader task panicked");
//             ErrorCode::CoreFileUploadingError.details()
//         })??;
// 
//     // Возвращаем информацию о загруженном файле
//     Ok(FileUploadResult {
//         name: filename.to_string(),
//         size: total_size,
//     })
// }

// Пример функции обработки чанка (замените на вашу логику)
async fn process_chunk(mut field: axum::extract::multipart::Field<'_>,) -> Result<(), BadResponseObject>{
    while let Some(chunk) = field.chunk().await.map_err(|err| {
        tracing::error!("Error reading chunk: {}", err);
        ErrorCode::CoreFileUploadingError.details()
    })? {
        // Добавляем данные в буфер
        println!("Processing chunk of size: {}", chunk.len());
    }
    // Выполните здесь нужную обработку чанка
    Ok(())
}

fn process_fixed_size_chunk(chunk: &Bytes) {
    tracing::info!("Processing fixed-size chunk of size: {}", chunk.len());
    // Здесь вы можете выполнять любую нужную обработку чанка
}
//
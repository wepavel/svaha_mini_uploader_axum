use axum::extract::Multipart;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use utoipa_swagger_ui::SwaggerUi;
use axum::middleware::{self, Next};


use axum::{http::{HeaderValue, Method}, body::Bytes, extract::MatchedPath, extract::DefaultBodyLimit, http::{HeaderMap, Request}, response::{Html, Response}, routing::get, Router, http::StatusCode, response::IntoResponse, routing::post, ServiceExt};
use std::time::Duration;
use tokio::net::TcpListener;
use tower_http::{classify::ServerErrorsFailureClass, trace::TraceLayer};
use tracing::{info_span, Span};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use bytes::{Bytes as BBytes, BytesMut};

use api::custom_tracing;
use tower_http::cors::{Any, CorsLayer};
use utoipa::openapi::Info;
use std::net::SocketAddr;

use core::exceptions::{ErrorCode, global_error_handler};

const CHUNK_SIZE: usize = 1024 * 1024 * 5; // 1 MB chunks, adjust as needed


/// Just a schema for axum native multipart
#[derive(Deserialize, ToSchema)]
#[allow(unused)]
struct HelloForm {
    #[schema(format = Binary, content_media_type = "application/octet-stream")]
    vocal: String,
    #[schema(format = Binary, content_media_type = "application/octet-stream")]
    instrumental: String,
    name: String,
}



#[derive(Deserialize, Serialize, ToSchema)]
struct Sas {
    sas: String
}


#[utoipa::path(
    post,
    path = "/hello",
    tag = "Upload",
    description = "SAS",
    request_body(content = HelloForm, content_type = "multipart/form-data", description = "Hello guys!"),
    responses(
        (status = 200, body = Sas, description = "Pet stored successfully",
            examples(
                ("Demo" = (summary = "This is summary", description = "Long description",
                            value = json!(Sas{sas: "Demo".to_string()}))),
                ("John" = (summary = "Another user", value = json!({"name": "John"})))
            )
        ),
        (status = 400, body = Sas, description = "Pet stored successfully",
            examples(
                // ("Demo" = (summary = "This is summary", description = "Long description",
                //             value = json!(Sas{sas: "Demo".to_string()}))),
                ("John" = (summary = "Another user", value = json!({"name": "John"})))
            )
        )
    )
)]
async fn hello_form(mut multipart: Multipart) -> String {
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

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                // axum logs rejections from built-in extractors with the `axum::rejection`
                // target, at `TRACE` level. `axum::rejection=trace` enables showing those events
                format!(
                    "{}=debug,tower_http=debug,axum::rejection=trace,api=debug",
                    env!("CARGO_CRATE_NAME")
                )
                    .into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting server");

    let cors = CorsLayer::new()
        // .allow_credentials(true)
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let (router, mut api) = OpenApiRouter::new()
        .routes(routes!(hello_form))
        .split_for_parts();

    api.info = Info::new("svaha-mini-uploader", "1.0.0");
    api.info.description = Some("Щас по ебалу получишь блять!".to_string());

    // let sas = router.into_make_service_with_connect_info();

    let router = router
        .merge(SwaggerUi::new("/docs").url("/api/v1/openapi.json", api))
        .layer(cors)
        .layer(DefaultBodyLimit::max(1024 * 1024 * 1024 * 10 ))
        .layer(custom_tracing::create_tracing_layer())
        .fallback(core::exceptions::handler_404)
        .layer(middleware::from_fn(global_error_handler));
    //     .layer(
    //     TraceLayer::new_for_http()
    //         .make_span_with(|request: &Request<_>| {
    //             // Log the matched route's path (with placeholders not filled in).
    //             // Use request.uri() or OriginalUri if you want the real path.
    //             let matched_path = request
    //                 .extensions()
    //                 .get::<MatchedPath>()
    //                 .map(MatchedPath::as_str);
    //
    //             tracing::info!("SUS");
    //
    //             info_span!(
    //                     "http_request",
    //                     method = ?request.method(),
    //                     matched_path,
    //                     some_other_field = tracing::field::Empty,
    //                 )
    //         })                .on_request(|_request: &Request<_>, _span: &Span| {
    //         // You can use `_span.record("some_other_field", value)` in one of these
    //         // closures to attach a value to the initially empty field in the info_span
    //         // created above.
    //     })
    //         .on_response(|_response: &Response, _latency: Duration, _span: &Span| {
    //             // ...
    //         })
    //         .on_body_chunk(|_chunk: &Bytes, _latency: Duration, _span: &Span| {
    //             // ...
    //         })
    //         .on_eos(
    //             |_trailers: Option<&HeaderMap>, _stream_duration: Duration, _span: &Span| {
    //                 // ...
    //             },
    //         )
    //         .on_failure(
    //             |_error: ServerErrorsFailureClass, _latency: Duration, _span: &Span| {
    //                 // ...
    //             },
    //         ),
    // );

    let app = router.into_make_service_with_connect_info::<SocketAddr>();
    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(listener, app).await
}
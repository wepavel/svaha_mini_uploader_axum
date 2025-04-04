use utoipa_swagger_ui::SwaggerUi;
use axum::middleware::{self };

use axum::extract::DefaultBodyLimit;
use tokio::net::TcpListener;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tower_http::{compression::CompressionLayer, decompression::RequestDecompressionLayer, services::ServeDir};
use axum::routing::get_service;
use api::custom_tracing;
use tower_http::cors::{Any, CorsLayer};

use std::net::SocketAddr;

use api::exceptions::{ErrorCode, global_error_handler};
use api::get_api;

use core::config::CONFIG;

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                // axum logs rejections from built-in extractors with the `axum::rejection`
                // target, at `TRACE` level. `axum::rejection=trace` enables showing those events
                format!(
                    "{}=debug,\
                    tower_http=debug,\
                    axum::rejection=trace,\
                    api=info,\
                    http_response=info,\
                    // http_request=info",
                    env!("CARGO_CRATE_NAME")
                ).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let addr = format!("{}:{}", CONFIG.host, CONFIG.port);
    tracing::info!("Starting server on http://{addr}");

    let cors = CorsLayer::new()
        // .allow_credentials(true)
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // let sas = router.into_make_service_with_connect_info();

    let (router, api) = get_api();
    let router = router
        .merge(SwaggerUi::new("/docs").url("/api/v1/openapi.json", api))
        .layer(cors)
        .layer(RequestDecompressionLayer::new())  // Сначала разжимаем входящие запросы
        .layer(CompressionLayer::new())  // Затем сжимаем исходящие ответы
        // .fallback_service(get_service(ServeDir::new("static")))
        // .layer(DefaultBodyLimit::max(1024 * 1024 * 1024 * 10 ))
        .layer(custom_tracing::create_tracing_layer())
        .layer(middleware::from_fn(global_error_handler))
    ;



    let app = router
        .into_make_service_with_connect_info::<SocketAddr>();
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}
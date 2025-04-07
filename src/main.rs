use utoipa_swagger_ui::SwaggerUi;
use axum::middleware::{self};
use axum::Router;
use axum::extract::DefaultBodyLimit;
use tokio::net::TcpListener;
use tokio::signal;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tower_http::{compression::CompressionLayer, decompression::RequestDecompressionLayer, services::ServeDir};
use axum::routing::get_service;
use api::custom_tracing;
use axum::routing::get;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tower_http::timeout::TimeoutLayer;
use std::time::Duration;
use axum::extract::State;
use std::net::SocketAddr;
use std::sync::Arc;

use api::exceptions::{ErrorCode, global_error_handler};
use api::get_api;

use core::config::CONFIG;

use services::AppState;


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
                    http_response=info",
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
    let app_state = Arc::new(AppState::new().expect("Failed to create AppState"));
    let mut router = get_api(app_state);



    router = router
        .layer(cors)
        .layer(RequestDecompressionLayer::new())  // Сначала разжимаем входящие запросы
        .layer(CompressionLayer::new())  // Затем сжимаем исходящие ответы
        // .layer(DefaultBodyLimit::max(1024 * 1024 * 1024 * 10 ))
        .layer(custom_tracing::create_tracing_layer())
        .layer(middleware::from_fn(global_error_handler))

        .layer((
            TraceLayer::new_for_http(),
            // Graceful shutdown will wait for outstanding requests to complete. Add a timeout so
            // requests don't hang forever.
            TimeoutLayer::new(Duration::from_secs(10)),
        ));


    let app = router
        .into_make_service_with_connect_info::<SocketAddr>();
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

use axum::middleware::{self};
use tokio::net::TcpListener;
use tokio::signal;
use tower_http::{compression::CompressionLayer, decompression::RequestDecompressionLayer};

use api::custom_tracing;

use tower_http::cors::{Any, CorsLayer};

use tower_http::timeout::TimeoutLayer;
use std::time::Duration;

use std::net::SocketAddr;
use std::sync::Arc;
use axum::extract::DefaultBodyLimit;
use api::custom_exceptions::{ErrorCode, global_error_handler};
use api::exceptions;

use api::get_api;
use core::logging::init_logger;
use tower_http::limit::RequestBodyLimitLayer;
use core::config::CONFIG;
use tracing_subscriber;
use tower_http::trace::TraceLayer;
use services::AppState;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};


#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    init_logger("svaha_mini_uploader_axum");
    // tracing_subscriber::registry()
    //     .with(
    //         tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
    //             // axum logs rejections from built-in extractors with the `axum::rejection`
    //             // target, at `TRACE` level. `axum::rejection=trace` enables showing those events
    //             format!(
    //                 "{}=debug,\
    //                 tower_http=debug,\
    //                 axum::rejection=trace,\
    //                 api=info,\
    //                 custom_exceptions=info,\
    //                 http_response=info,\
    //                 http_failure=info",
    //                 env!("CARGO_CRATE_NAME")
    //             ).into()
    //         }),
    //     )
    //     .with(tracing_subscriber::fmt::layer())
    //     .init();

    let addr = format!("{}:{}", CONFIG.host, CONFIG.port);
    tracing::info!("Starting server on http://{addr}");

    let cors = CorsLayer::new()
        // .allow_credentials(true)
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // let sas = router.into_make_service_with_connect_info();
    let app_state = Arc::new(AppState::new().await.expect("Failed to create AppState"));
    let mut router = get_api(app_state);



    router = router
        .layer(cors)
        // .layer(tower::limit::ConcurrencyLimitLayer::new(500))
        .layer(RequestDecompressionLayer::new())  // Сначала разжимаем входящие запросы
        .layer(CompressionLayer::new())  // Затем сжимаем исходящие ответы

        // 
        .layer(DefaultBodyLimit::disable())
        // .layer(RequestBodyLimitLayer::new(CONFIG.body_size_limit))
        .layer(custom_tracing::create_tracing_layer())
        // .layer(middleware::from_fn(custom_tracing::request_data_middleware))
        .layer(middleware::from_fn(global_error_handler))
        // .layer(middleware::from_fn(exceptions::global_error_handler))
        // .layer((
        //     // TraceLayer::new_for_http(),
        //     // Graceful shutdown will wait for outstanding requests to complete. Add a timeout so
        //     // requests don't hang forever.
        //     TimeoutLayer::new(Duration::from_secs(60)),
        // ))
    ;


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

    tracing::info!("Shutdown signal received, starting graceful shutdown");
    tokio::time::sleep(Duration::from_secs(10)).await;
    tracing::info!("Graceful shutdown period ended, forcing shutdown");
}

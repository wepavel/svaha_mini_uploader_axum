use utoipa::openapi::{Info, OpenApi};
pub mod custom_tracing;
mod endpoints;
pub mod exceptions;

use my_core::config::CONFIG;
use utoipa_axum::router::OpenApiRouter;
use axum::Router;
use axum::routing::get_service;
use tower_http::services::ServeDir;
// use endpoints::files::__path_hello_form;
use endpoints::{
    files, tests, webui
};


pub fn get_api() -> (Router, OpenApi) {

    let (router, mut api) = OpenApiRouter::new()
        .nest(&format!("{}upload", CONFIG.api_v1_str.as_str()), files::get_router())
        .nest(&format!("{}test", CONFIG.api_v1_str.as_str()), tests::get_router())
        .nest(&format!("{}upload-ui", CONFIG.api_v1_str.as_str()), webui::get_router())
        .split_for_parts();

    api.info = Info::new("svaha-mini-uploader", "1.0.0");
    api.info.description = Some("Upload best files!".to_string());

    (router, api)
}


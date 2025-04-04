use utoipa::openapi::{Info, OpenApi};
pub mod custom_tracing;
mod endpoints;
pub mod exceptions;

use my_core::config::CONFIG;
use utoipa_axum::router::OpenApiRouter;
use axum::Router;
use utoipa_swagger_ui::SwaggerUi;

use endpoints::{
    files, tests, webui
};


pub fn get_api() -> Router {

    let (mut router, mut api) = OpenApiRouter::new()
        .nest(&format!("{}upload", CONFIG.api_v1_str.as_str()), files::get_router())
        .nest(&format!("{}test", CONFIG.api_v1_str.as_str()), tests::get_router())
        .nest(&format!("{}upload-ui", CONFIG.api_v1_str.as_str()), webui::get_router())
        .split_for_parts();

    api.info = Info::new("Svaha-Mini Uploader", "1.0.0");
    api.info.description = Some("This is world best uploader, writed on RUST!".to_string());

    if !CONFIG.production {
        router = router.merge(SwaggerUi::new("/docs").url(format!("{}openapi.json", CONFIG.api_v1_str.as_str()), api));
    }

    router
}


use std::collections::HashMap;
use axum::extract::Multipart;

// use core::exceptions::{ErrorCode, global_error_handler};
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};
use crate::custom_exceptions::{JsonResponse, ErrorCode, BadResponseObject};

use serde_json::Value;
use axum::{Json, extract::Path};
use serde::Serialize;
use tracing;
use serde_json::json;
use services::AppState;
use std::sync::Arc;

const TAG: &str = "Test";
pub fn get_router(app_state: Arc<AppState>) -> OpenApiRouter {
    OpenApiRouter::new().routes(routes!(test_endpoint))
}

#[derive(Serialize)]
struct SAS {
    pisun: String,
    zalupa: String,
}

#[derive(Serialize)]
struct PUK {
    kal: String,
    mocha: i32,
    hehe: SAS
}

impl PUK {
    fn new(kal: &str, mocha: i32) -> Self {
        let sas = SAS{pisun: "piska".to_string(), zalupa: "golova chlena".to_string()};
        PUK{kal: kal.to_string(), mocha, hehe: sas}
    }
}

#[utoipa::path(
    get,
    tag=TAG,
    path = "/return-number/{number}",
    params(
        ("number" = i32, Path, description = "The number to return")
    ),
    responses(
        (status = 200, description = "Number returned successfully", body = i32),
        (status = 400, description = "Bad request", body = BadResponseObject, example = json!(BadResponseObject::default_400())),
        (status = 500, description = "Internal server error", body = BadResponseObject, example = json!(BadResponseObject::default_500())),
    )
)]
async fn test_endpoint(Path(number): Path<i32>) -> JsonResponse {
    if number == 2 {
        // return JsonResponse::from(ErrorCode::AuthorizeError.details()
        //     .with_detail("reason", "You have already taken access to this endpoint."));
        return ErrorCode::BrideError.details()
            .with("reason", "You have already taken access to this endpoint.")
            .into();
    }

    // tracing::info!("Hello from tracing!");

    let x = PUK::new("KAKASHECHKA", number as i32);


    JsonResponse::Ok(json!(x))
}


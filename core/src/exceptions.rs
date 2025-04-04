// use axum::{
//     extract::{FromRequest, FromRequestParts, Request},
//     http::{StatusCode, request::Parts},
//     response::{IntoResponse, Response},
//     Json as AxumJson,
//     RequestPartsExt,
//     middleware::Next,
//     body::{Body, Bytes},
// };
// use serde::{de::DeserializeOwned, Serialize};
// use std::{collections::HashMap, error::Error as StdError};
//
// use strum_macros::{EnumIter, AsRefStr};
//
// use axum::extract::{MatchedPath};
//
// use serde_json::json;
// use std::fmt;
//
// use utoipa::ToSchema;
//
// // Определение BadResponseObject
// #[derive(Debug, Clone, Serialize, Default, ToSchema)]
// #[schema(example = json!({
//     "code": 5000,
//     "msg": "Internal Server Error",
//     "details": {},
//     "redirect": false,
//     "notification": false
// }))]
// pub struct BadResponseObject {
//     code: u16,
//     msg: String,
//     #[serde(default)]
//     details: HashMap<String, serde_json::Value>,
//     #[serde(default)]
//     redirect: bool,
//     #[serde(default)]
//     notification: bool,
// }
//
//
// impl fmt::Display for BadResponseObject {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         write!(f, "Error {}: {}", self.code, self.msg)
//     }
// }
// impl StdError for BadResponseObject {}
//
// impl BadResponseObject {
//     pub fn with_detail(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
//         self.details.insert(key.into(), serde_json::json!(value));
//         self
//     }
//
//     pub fn with_detail_option(mut self, key: impl Into<String>, value: Option<impl Serialize>) -> Self {
//         if let Some(v) = value {
//             self.details.insert(key.into(), serde_json::json!(v));
//         }
//         self
//     }
//
//     pub fn with_detail_if(self, condition: bool, key: impl Into<String>, value: impl Serialize) -> Self {
//         if condition { self.with_detail(key, value) } else { self }
//     }
//
//     pub fn add_redirect(mut self) -> Self { self.redirect = true; self }
//     pub fn add_notification(mut self) -> Self { self.notification = true; self }
// }
//
// // Макрос для определения кодов ошибок
// macro_rules! define_error_codes {
//     ($($variant:ident => BadResponseObject{ code: $code:expr, msg: $msg:expr },)*) => {
//         #[derive(Debug, EnumIter, AsRefStr, PartialEq, Eq)]
//         pub enum ErrorCode { $($variant,)* }
//
//         impl ErrorCode {
//             pub fn details(&self) -> BadResponseObject {
//                 match self {
//                     $(ErrorCode::$variant => BadResponseObject {
//                         code: $code,
//                         msg: $msg.to_string(),
//                         ..Default::default()
//                     },)*
//                 }
//             }
//         }
//     };
// }
//
// impl IntoResponse for BadResponseObject {
//     fn into_response(self) -> Response {
//         let status = if (4000..5000).contains(&self.code) {
//             StatusCode::BAD_REQUEST
//         } else {
//             StatusCode::INTERNAL_SERVER_ERROR
//         };
//         (status, AxumJson(self)).into_response()
//     }
// }
//
// // Определение кодов ошибок (пример, вы можете расширить его в соответствии с вашими потребностями)
// define_error_codes! {
//     // 4000: Bad Request
//     BadRequest => BadResponseObject{ code: 4000, msg: "Bad Request" },
//     // 4021 - 4040: User Management Errors
//     CouldNotValidateUserCreds => BadResponseObject{ code: 4021, msg: "Could not validate credentials: ValidationError" },
//     UserExpiredSignatureError => BadResponseObject{ code: 4022, msg: "Could not validate credentials: ExpiredSignatureError" },
//     IncorrUserCreds => BadResponseObject{ code: 4023, msg: "Incorrect login or password" },
//     NotAuthenticated => BadResponseObject{ code: 4030, msg: "Not authenticated" },
//     InactiveUser => BadResponseObject{ code: 4032, msg: "Inactive user" },
//     UserRegistrationForbidden => BadResponseObject{ code: 4033, msg: "Open user registration is forbidden on this server" },
//     UserNotExists => BadResponseObject{ code: 4035, msg: "The user with this username does not exist in the system" },
//     UserExists => BadResponseObject{ code: 4036, msg: "The user already exists in the system" },
//     // 4041 - 4060: Project Management Errors
//     ProjectLocked => BadResponseObject{ code: 4041, msg: "Project locked" },
//     AvailableProjectsLimitExceeded => BadResponseObject{ code: 4042, msg: "Available projects limit exceeded" },
//     AvailableEditsLimitExceeded => BadResponseObject{ code: 4043, msg: "Available edits limit exceeded" },
//     NameAlreadyExists => BadResponseObject{ code: 4044, msg: "This name already exists" },
//     InstrumentalTrackExists => BadResponseObject{ code: 4045, msg: "Instrumental track already exists" },
//     // 4061 - 4081: Task Management Errors
//     TaskNotFound => BadResponseObject{ code: 4061, msg: "Task not found" },
//     TaskAlreadyExists => BadResponseObject{ code: 4062, msg: "Task already exists" },
//     SessionNotFound => BadResponseObject{ code: 4071, msg: "Session not found" },
//     SessionAlreadyExists => BadResponseObject{ code: 4072, msg: "Session already exists" },
//     // 4301 - 4320: Resource and Limit Errors
//     TooManyRequestsError => BadResponseObject{ code: 4301, msg: "Too Many Requests" },
//     // 4400: Validation Error
//     ValidationError => BadResponseObject{ code: 4400, msg: "Validation error" },
//     // 4401-4500: General Validation Errors
//     WrongFormat => BadResponseObject{ code: 4411, msg: "Wrong format" },
//     // 4501 - 4508: API and Request Errors
//     Unauthorized => BadResponseObject{ code: 4501, msg: "Sorry, you are not allowed to access this service: UnauthorizedRequest" },
//     AuthorizeError => BadResponseObject{ code: 4502, msg: "Authorization error" },
//     ForbiddenError => BadResponseObject{ code: 4503, msg: "Forbidden" },
//     NotFoundError => BadResponseObject{ code: 4504, msg: "Not Found" },
//     ResponseProcessingError => BadResponseObject{ code: 4505, msg: "Response Processing Error" },
//     YookassaApiError => BadResponseObject{ code: 4511, msg: "Yookassa Api Error" },
//     // 5000: Internal Server Error
//     InternalError => BadResponseObject{ code: 5000, msg: "Internal Server Error" },
//     BrideError => BadResponseObject{ code: 5010, msg: "Bride in prison" }, //
//     CoreOffline => BadResponseObject{ code: 5021, msg: "Core is offline" },
//     CoreFileUploadingError => BadResponseObject{ code: 5022, msg: "Core file uploading error" },
//     // 5041-5060: Database Errors
//     DbError => BadResponseObject{ code: 5041, msg: "Bad Gateway" },
//     // 5061 - 5999: System and Server Errors
//     UnknownError => BadResponseObject{ code: 5999, msg: "Internal Server Error" },
// }
//
// impl From<ErrorCode> for BadResponseObject {
//     fn from(error_code: ErrorCode) -> Self {
//         error_code.details()
//     }
// }
//
//
// // pub async fn global_error_handler(
// //     request: Request,
// //     next: Next,
// // ) -> Result<impl IntoResponse, Response> {
// //     let (parts, body) = request.into_parts();
// //     let bytes = match body.collect().await {
// //         Ok(collected) => collected.to_bytes(),
// //         Err(err) => {
// //             let error_response = BadResponseObject {
// //                 code: 5000,
// //                 msg: format!("Failed to read request body: {}", err),
// //                 ..Default::default()
// //             };
// //             // return (StatusCode::INTERNAL_SERVER_ERROR, AxumJson(error_response)).into_response();
// //             return Err(StatusCode::INTERNAL_SERVER_ERROR, AxumJson(error_response)).into_response())
// //         }
// //     };
// //
// //     let request = Request::from_parts(parts, Body::from(bytes));
// //     let response = next.run(request).await;
// //
// //     if let Some(error) = response.extensions().get::<Box<dyn StdError + Send + Sync>>() {
// //         let error_response = if let Some(bad_response) = error.downcast_ref::<BadResponseObject>() {
// //             bad_response.clone()
// //         } else {
// //             BadResponseObject {
// //                 code: 5999,
// //                 msg: format!("Internal Server Error: {}", error),
// //                 ..Default::default()
// //             }
// //         };
// //
// //         let status = if (4000..5000).contains(&error_response.code) {
// //             StatusCode::BAD_REQUEST
// //         } else {
// //             StatusCode::INTERNAL_SERVER_ERROR
// //         };
// //
// //         (status, AxumJson(error_response)).into_response()
// //     } else if response.status().is_server_error() {
// //         let error_response = BadResponseObject {
// //             code: 5000,
// //             msg: "Unexpected Server Error".to_string(),
// //             ..Default::default()
// //         };
// //         (StatusCode::INTERNAL_SERVER_ERROR, AxumJson(error_response)).into_response()
// //     } else {
// //         response
// //     }
// // }
//
// pub async fn global_error_handler(
//     request: Request,
//     next: Next,
// ) -> Result<impl IntoResponse, Response> {
//     let response = next.run(request).await;
//
//     if response.status().is_server_error() {
//         let error_response = BadResponseObject {
//             code: 5000,
//             msg: "Unexpected Server Error".to_string(),
//             ..Default::default()
//         };
//         Ok((StatusCode::INTERNAL_SERVER_ERROR, AxumJson(error_response)))
//     } else if let Some(error) = response.extensions().get::<Box<dyn StdError + Send + Sync>>() {
//         let error_response = if let Some(bad_response) = error.downcast_ref::<BadResponseObject>() {
//             bad_response.clone()
//         } else {
//             BadResponseObject {
//                 code: 5999,
//                 msg: format!("Internal Server Error: {}", error),
//                 ..Default::default()
//             }
//         };
//
//         let status = if (4000..5000).contains(&error_response.code) {
//             StatusCode::BAD_REQUEST
//         } else {
//             StatusCode::INTERNAL_SERVER_ERROR
//         };
//
//         Ok((status, AxumJson(error_response)))
//     } else {
//         Err(response)
//     }
// }
//
// // Пользовательский JSON экстрактор
// pub struct Json<T>(pub T);
//
// // impl<T> Deref for Json<T> {
// //     type Target = T;
// //
// //     fn deref(&self) -> &Self::Target {
// //         &self.0
// //     }
// // }
//
// impl<S, T> FromRequest<S> for Json<T>
// where
//     T: DeserializeOwned,
//     S: Send + Sync,
//     AxumJson<T>: FromRequest<S, Rejection = axum::extract::rejection::JsonRejection>,
// {
//     type Rejection = (StatusCode, AxumJson<serde_json::Value>);
//
//     async fn from_request(req: Request<axum::body::Body>, state: &S) -> Result<Self, Self::Rejection> {
//         let (mut parts, body) = req.into_parts();
//
//         let path = parts
//             .extract::<MatchedPath>()
//             .await
//             .map(|path| path.as_str().to_owned())
//             .ok();
//
//         let req = Request::from_parts(parts, body);
//
//         match AxumJson::<T>::from_request(req, state).await {
//             Ok(json) => Ok(Json(json.0)),
//             Err(rejection) => {
//                 let payload = serde_json::json!({
//                     "message": rejection.body_text(),
//                     "origin": "custom_extractor",
//                     "path": path,
//                 });
//
//                 Err((rejection.status(), AxumJson(payload)))
//             }
//         }
//     }
// }
//
//
//
// // Макрос для определения пользовательских типов ответов
// macro_rules! define_response {
//     ($name:ident $(<$($gen:ident $(: $bound:tt $(+ $bounds:tt)*)?),+>)?, $ok_type:ty) => {
//         pub enum $name $(<$($gen $(: $bound $(+ $bounds)*)?),+>)? {
//             Ok($ok_type),
//             Err(BadResponseObject),
//         }
//
//         impl $(<$($gen $(: $bound $(+ $bounds)*)?),+>)? IntoResponse for $name $(<$($gen),+>)? {
//             fn into_response(self) -> axum::response::Response {
//                 match self {
//                     Self::Ok(data) => (StatusCode::OK, AxumJson(data)).into_response(),
//                     Self::Err(err) => {
//                         let status = if (4000..5000).contains(&err.code) {
//                             StatusCode::BAD_REQUEST
//                         } else {
//                             StatusCode::INTERNAL_SERVER_ERROR
//                         };
//                         (status, AxumJson(err)).into_response()
//                     }
//                 }
//             }
//         }
//
//         impl $(<$($gen $(: $bound $(+ $bounds)*)?),+>)? From<BadResponseObject> for $name $(<$($gen),+>)? {
//             fn from(obj: BadResponseObject) -> Self {
//                 Self::Err(obj)
//             }
//         }
//
//         impl $(<$($gen $(: $bound $(+ $bounds)*)?),+>)? From<ErrorCode> for $name $(<$($gen),+>)? {
//             fn from(error_code: ErrorCode) -> Self {
//                 Self::Err(error_code.into())
//             }
//         }
//     };
// }
//
// // Определение пользовательских типов ответов
// define_response!(JsonResponse<T: Serialize>, T);
// define_response!(GetFileResponse, Vec<u8>);
// define_response!(PlainTextResponse, String);
// define_response!(HtmlResponse, String);
//
// // Пример использования в обработчике
// pub async fn example_handler(Json(payload): Json<serde_json::Value>) -> impl IntoResponse {
//     // Ваша логика обработки
//     if payload["some_field"].is_null() {
//         return JsonResponse::Err(ErrorCode::ValidationError.details().with_detail("field", "some_field is required"));
//     }
//     JsonResponse::Ok(payload)
// }
//
// // Для использования в main.rs или где вы настраиваете маршруты:
// // use axum::{Router, routing::post};
// // use tower_http::trace::TraceLayer;
// //
// // let app = Router::new()
// //     .route("/example", post(example_handler))
// //     .layer(TraceLayer::new_for_http())
// //     .layer(axum::error_handling::HandleErrorLayer::new(global_error_handler));
//
// pub async fn handler_404() -> impl IntoResponse {
//     ErrorCode::NotFoundError.details()
//     // (StatusCode::NOT_FOUND, "nothing to see here")
// }

use axum::{
    extract::{Request},
    http::{StatusCode},
    response::{IntoResponse, Response, Html as AxumHtml},
    Json as AxumJson,
    middleware::Next,
    body::{to_bytes},
};
use serde::{Serialize, Deserialize};
use std::{collections::HashMap, error::Error as StdError, fmt};
use strum_macros::{EnumIter, AsRefStr};
use utoipa::ToSchema;
use serde_json::json;
use lazy_regex::regex;


#[macro_export]
macro_rules! try_json {
    ($expr:expr) => {
        match $expr {
            Ok(val) => val,
            Err(err) => {
                return JsonResponse::Err(err.into());
            }
        }
    };
}

// Базовая структура для ошибок API
#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
#[schema(example = json!({
    "code": 5000,
    "msg": "Internal Server Error",
    "details": {},
    "redirect": false,
    "notification": false
}))]
pub struct BadResponseObject {
    pub code: u16,
    pub msg: String,
    // #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub details: HashMap<String, serde_json::Value>,
    // #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub redirect: bool,
    // #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub notification: bool,
}

impl fmt::Display for BadResponseObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error {}: {}", self.code, self.msg)
    }
}

impl StdError for BadResponseObject {}

impl IntoResponse for BadResponseObject {
    fn into_response(self) -> Response {
        let status = if (4000..5000).contains(&self.code) {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        (status, AxumJson(self)).into_response()
    }
}

impl BadResponseObject {
    // Универсальный метод для добавления данных в details
    pub fn with<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Serialize,
    {
        self.details.insert(key.into(), json!(value));
        self
    }

    // Метод для добавления с условием
    pub fn with_if<K, V>(self, condition: bool, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Serialize,
    {
        if condition { self.with(key, value) } else { self }
    }

    // Метод для добавления опционального значения
    pub fn with_opt<K, V>(self, key: K, value: Option<V>) -> Self
    where
        K: Into<String>,
        V: Serialize,
    {
        if let Some(v) = value {
            self.with(key, v)
        } else {
            self
        }
    }

    // Методы для установки флагов
    pub fn redirect(mut self) -> Self { self.redirect = true; self }
    pub fn notify(mut self) -> Self { self.notification = true; self }

    // Стандартные ошибки
    pub fn default_400() -> Self {
        Self { code: 4000, msg: "Bad Request".to_string(), ..Default::default() }
    }

    pub fn default_500() -> Self {
        Self { code: 5000, msg: "Internal Server Error".to_string(), ..Default::default() }
    }
}

// Улучшенный макрос для определения кодов ошибок
macro_rules! define_error_codes {
    ($(
        $variant:ident => $code:expr, $msg:expr
        $(, $extra:ident = $value:expr)*
    );* $(;)?) => {
        #[derive(Debug, EnumIter, AsRefStr, PartialEq, Eq)]
        pub enum ErrorCode {
            $($variant,)*
        }

        impl ErrorCode {
            pub fn details(&self) -> BadResponseObject {
                match self {
                    $(ErrorCode::$variant => {
                        let mut response = BadResponseObject {
                            code: $code,
                            msg: $msg.to_string(),
                            ..Default::default()
                        };
                        $(
                            match stringify!($extra) {
                                "redirect" => response.redirect = $value,
                                "notification" => response.notification = $value,
                                _ => { response = response.with(stringify!($extra), $value); }
                            }
                        )*
                        response
                    },)*
                }
            }
        }
    };
}

impl From<ErrorCode> for BadResponseObject {
    fn from(error_code: ErrorCode) -> Self {
        error_code.details()
    }
}

// Упрощенная функция очистки сообщения об ошибке
fn clean_error_message(message: &str) -> String {
    let re_unreadable = regex!(r"\(/\x00X.\x01\x00");
    let re_unwanted = regex!(r"[^a-zA-Zа-яА-Я0-9\s:\-,.'`]");

    re_unwanted.replace_all(&re_unreadable.replace_all(message, ""), "")
        .trim().to_string()
}

// Упрощенный глобальный обработчик ошибок
pub async fn global_error_handler(
    request: Request,
    next: Next,
) -> Result<Response, Response> {
    let path = request.uri().path().to_string();
    let response = next.run(request).await;

    // Если ответ успешный - просто возвращаем
    if response.status().is_success() {
        return Ok(response);
    }

    let (parts, body) = response.into_parts();
    let status = parts.status;

    // Читаем тело ответа
    let bytes = match to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(_) => return Ok(
            ErrorCode::InternalError.details()
                .with("endpoint", &path)
                .into_response()
        ),
    };

    // Основная логика обработки ошибок
    let error_response = if let Ok(bad_response) = serde_json::from_slice::<BadResponseObject>(&bytes) {
        // Уже сформированная ошибка - просто добавляем endpoint
        bad_response.with("endpoint", &path)
    } else {
        match status {
            StatusCode::BAD_REQUEST => {
                // Ошибка валидации
                let message = String::from_utf8_lossy(&bytes);
                ErrorCode::ValidationError.details()
                    .with("reason", clean_error_message(&message))
                    .with("endpoint", &path)
            },
            StatusCode::NOT_FOUND => {
                // Ресурс не найден
                ErrorCode::NotFoundError.details()
                    .with("endpoint", &path)
            },
            _ if status.is_server_error() => {
                // Внутренняя ошибка сервера
                // BadResponseObject::default_500()
                ErrorCode::InternalError.details()
                    .with("endpoint", &path)
            },
            _ => {
                // Неизвестная ошибка
                ErrorCode::UnknownError.details()
                    .with("status", status.as_u16())
                    .with("endpoint", &path)
            }
        }
    };

    Ok(error_response.into_response())
}

// Стандартные типы для ответов API
pub trait IntoCustomResponse {
    fn into_custom_response(self) -> Response;
}

impl IntoCustomResponse for String {
    fn into_custom_response(self) -> Response {
        AxumHtml(self).into_response()
    }
}

impl IntoCustomResponse for Vec<u8> {
    fn into_custom_response(self) -> Response {
        (StatusCode::OK, self).into_response()
    }
}

impl IntoCustomResponse for serde_json::Value {
    fn into_custom_response(self) -> Response {
        AxumJson(self).into_response()
    }
}

// Значительно упрощенный макрос для определения типов ответов
macro_rules! define_responses {
    ($($name:ident => $ok_type:ty),* $(,)?) => {
        $(
            pub enum $name {
                Ok($ok_type),
                Err(BadResponseObject),
            }

            impl IntoResponse for $name {
                fn into_response(self) -> Response {
                    match self {
                        Self::Ok(data) => data.into_custom_response(),
                        Self::Err(err) => err.into_response(),
                    }
                }
            }

            impl From<BadResponseObject> for $name {
                fn from(obj: BadResponseObject) -> Self {
                    Self::Err(obj)
                }
            }

            impl From<ErrorCode> for $name {
                fn from(error_code: ErrorCode) -> Self {
                    Self::Err(error_code.into())
                }
            }
        )*
    };
}

// Определяем все типы ответов одним вызовом макроса
define_responses! {
    JsonResponse => serde_json::Value,
    GetFileResponse => Vec<u8>,
    PlainTextResponse => String,
    HtmlResponse => String,
}

// Определяем все коды ошибок одним вызовом макроса
define_error_codes! {
    // 4000: Bad Request
    BadRequest => 4000, "Bad Request";

    // 4021 - 4040: User Management Errors
    CouldNotValidateUserCreds => 4021, "Could not validate credentials: ValidationError";
    UserExpiredSignatureError => 4022, "Could not validate credentials: ExpiredSignatureError";
    IncorrUserCreds => 4023, "Incorrect login or password";
    NotAuthenticated => 4030, "Not authenticated";
    InactiveUser => 4032, "Inactive user";
    UserRegistrationForbidden => 4033, "Open user registration is forbidden on this server";
    UserNotExists => 4035, "The user with this username does not exist in the system";
    UserExists => 4036, "The user already exists in the system";

    // 4041 - 4060: Project Management Errors
    ProjectLocked => 4041, "Project locked";
    AvailableProjectsLimitExceeded => 4042, "Available projects limit exceeded";
    AvailableEditsLimitExceeded => 4043, "Available edits limit exceeded";
    NameAlreadyExists => 4044, "This name already exists";
    InstrumentalTrackExists => 4045, "Instrumental track already exists";

    // 4061 - 4081: Task Management Errors
    TaskNotFound => 4061, "Task not found";
    TaskAlreadyExists => 4062, "Task already exists";
    SessionNotFound => 4071, "Session not found";
    SessionAlreadyExists => 4072, "Session already exists";

    // 4301 - 4320: Resource and Limit Errors
    TooManyRequestsError => 4301, "Too Many Requests";

    // 4400: Validation Error
    ValidationError => 4400, "Validation error";

    // 4401-4500: General Validation Errors
    WrongFormat => 4411, "Wrong format";

    // 4501 - 4508: API and Request Errors
    Unauthorized => 4501, "Sorry, you are not allowed to access this service: UnauthorizedRequest";
    AuthorizeError => 4502, "Authorization error";
    ForbiddenError => 4503, "Forbidden";
    NotFoundError => 4504, "Not Found";
    ResponseProcessingError => 4505, "Response Processing Error";
    YookassaApiError => 4511, "Yookassa Api Error";

    // 5000: Internal Server Error
    InternalError => 5000, "Internal Server Error";
    BrideError => 5010, "Bride in prison";
    CoreOffline => 5021, "Core is offline";
    CoreFileUploadingError => 5022, "Core file uploading error";

    // 5041-5060: Database Errors
    DbError => 5041, "Bad Gateway";

    // 5061 - 5999: System and Server Errors
    UnknownError => 5999, "Internal Server Error";
}

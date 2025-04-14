use axum::http::{Request, StatusCode};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fmt;
use std::io::{self, Write};
use std::time::Instant;
use tracing::{Event, Level, Subscriber, field::{Field, Visit}};
use tracing_subscriber::{
    fmt::{format::Writer, FmtContext, FormatEvent, FormatFields},
    layer::SubscriberExt,
    registry::LookupSpan,
    EnvFilter, Registry,
};
use ulid::Ulid;

/// Основная структура для хранения данных лога
#[derive(Serialize, Debug, Default)]
struct LogRecord {
    #[serde(rename = "@timestamp")]
    timestamp: String,
    level: String,
    message: String,
    service: String,

    // HTTP данные
    #[serde(skip_serializing_if = "Option::is_none")]
    host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_ms: Option<f64>,

    // Идентификация и трассировка
    trace_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    span_id: Option<String>,

    // Дополнительные поля
    #[serde(flatten)]
    additional_fields: HashMap<String, Value>,
}

/// Структура для форматирования логов в JSON
pub struct JsonFormatter {
    service_name: String,
}

impl JsonFormatter {
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
        }
    }
}

/// Посетитель для сбора полей события
struct FieldVisitor(HashMap<String, Value>);

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.0.insert(field.name().to_string(), json!(format!("{:?}", value)));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().to_string(), json!(value));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.0.insert(field.name().to_string(), json!(value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.0.insert(field.name().to_string(), json!(value));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.0.insert(field.name().to_string(), json!(value));
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.0.insert(field.name().to_string(), json!({
            "error": value.to_string(),
            "type": std::any::type_name::<dyn std::error::Error>(),
        }));
    }
}

impl<S, N> FormatEvent<S, N> for JsonFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        // Собираем базовые метаданные
        let metadata = event.metadata();
        let now: DateTime<Utc> = Utc::now();

        // Создаем новую запись лога
        let mut log_record = LogRecord {
            timestamp: now.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string(),
            level: metadata.level().to_string().to_lowercase(),
            message: String::new(), // Заполним позже
            service: self.service_name.clone(),
            trace_id: Ulid::new().to_string(), // Используем ULID вместо UUID
            ..Default::default()
        };

        // Собираем поля события
        let mut visitor = FieldVisitor(HashMap::new());
        event.record(&mut visitor);
        let fields = visitor.0;

        // Извлекаем сообщение
        if let Some(msg) = fields.get("message") {
            if let Some(msg_str) = msg.as_str() {
                log_record.message = msg_str.to_string();
            } else {
                log_record.message = msg.to_string();
            }
        } else {
            log_record.message = format!("{:?}", event);
        }

        // Обрабатываем HTTP запросы
        if let Some(http_request) = fields.get("http_request") {
            if let Some(req_obj) = http_request.as_object() {
                if let Some(host) = req_obj.get("host") {
                    log_record.host = host.as_str().map(|s| s.to_string());
                }
                if let Some(method) = req_obj.get("method") {
                    log_record.method = method.as_str().map(|s| s.to_string());
                }
                if let Some(path) = req_obj.get("path") {
                    log_record.path = path.as_str().map(|s| s.to_string());
                }
                if let Some(version) = req_obj.get("version") {
                    log_record.version = version.as_str().map(|s| s.to_string());
                }
            }
        }

        // Обрабатываем HTTP ответы
        if let Some(http_response) = fields.get("http_response") {
            if let Some(res_obj) = http_response.as_object() {
                if let Some(status) = res_obj.get("status") {
                    if let Some(status_num) = status.as_u64() {
                        log_record.status = Some(status_num as u16);
                    } else if let Some(status_str) = status.as_str() {
                        // Пробуем распарсить строку в число
                        if let Ok(status_num) = status_str.parse::<u16>() {
                            log_record.status = Some(status_num);
                        }
                    }
                }
                if let Some(duration) = res_obj.get("duration") {
                    // Извлекаем числовое значение из строки типа "83.845µs"
                    if let Some(duration_str) = duration.as_str() {
                        if duration_str.ends_with("µs") {
                            if let Ok(micro_secs) = duration_str
                                .trim_end_matches("µs")
                                .parse::<f64>() {
                                log_record.duration_ms = Some(micro_secs / 1000.0);
                            }
                        } else if duration_str.ends_with("ms") {
                            if let Ok(milli_secs) = duration_str
                                .trim_end_matches("ms")
                                .parse::<f64>() {
                                log_record.duration_ms = Some(milli_secs);
                            }
                        } else if duration_str.ends_with("s") {
                            if let Ok(secs) = duration_str
                                .trim_end_matches("s")
                                .parse::<f64>() {
                                log_record.duration_ms = Some(secs * 1000.0);
                            }
                        }
                    }
                }
            }
        }

        // Собираем информацию из spans
        if let Some(scope) = ctx.event_scope() {

            let spans: Vec<_> = scope.from_root().collect();

            // Используем последний span для span_id
            if let Some(span) = spans.last() {
                log_record.span_id = Some(format!("{:x}", span.id().into_u64()));
            }

            // Собираем дополнительные поля из span - исправляем ошибку "Value used after being moved"
            for span in spans {
                // Используем ссылку чтобы избежать перемещения
                let extensions = span.extensions();

                // Нужно скопировать поля вместо их перемещения
                if let Some(fields) = extensions.get::<HashMap<String, String>>() {
                    for (key, value) in fields.iter() {
                        log_record.additional_fields.insert(key.clone(), json!(value.clone()));
                    }
                }
            }
        }

        // Добавляем все оставшиеся поля
        for (key, value) in fields {
            // Пропускаем уже обработанные поля
            if !["message", "http_request", "http_response"].contains(&key.as_str()) {
                log_record.additional_fields.insert(key, value);
            }
        }

        // Определяем формат вывода на основе структуры существующих логов
        let formatted_log = match (log_record.level.as_str(), &log_record.message) {
            // Для простых сообщений используем формат "LEVEL service: message"
            ("info", _) if !log_record.message.contains("http_request") && !log_record.message.contains("http_response") => {
                format!("{}  {} {}: {}",
                        log_record.timestamp,
                        log_record.level.to_uppercase(),
                        self.service_name,
                        log_record.message)
            },
            // Для HTTP запросов используем формат с JSON частью
            (_, _) if log_record.host.is_some() || log_record.method.is_some() => {
                let http_req_part = json!({
                    "host": log_record.host,
                    "method": log_record.method,
                    "path": log_record.path,
                    "version": log_record.version
                });

                // Если есть данные ответа
                if log_record.status.is_some() {
                    let http_resp_part = json!({
                        "duration": format!("{:.3}µs",
                            log_record.duration_ms.unwrap_or(0.0) * 1000.0),
                        "message": "OK", // Можно дополнить логикой для разных статусов
                        "status": log_record.status
                    });

                    format!("{}  {} \"http_request\":{}: http_response: \"http_response\":{}",
                            log_record.timestamp,
                            log_record.level.to_uppercase(),
                            http_req_part.to_string(),
                            http_resp_part.to_string())
                } else {
                    format!("{}  {} \"http_request\":{}: {}: {}",
                            log_record.timestamp,
                            log_record.level.to_uppercase(),
                            http_req_part.to_string(),
                            metadata.target(),
                            log_record.message)
                }
            },
            // Для остальных случаев - полный JSON
            _ => serde_json::to_string(&log_record).unwrap_or_else(|_|
                format!("{{\"level\":\"error\",\"message\":\"Failed to serialize log\",\"timestamp\":\"{}\"}}",
                        log_record.timestamp)
            )
        };

        // Записываем в вывод
        writeln!(writer, "{}", formatted_log)
    }
}

/// Обертка для stdout без буферизации
struct StdoutUnbuffered {
    stdout: io::Stdout,
}

impl StdoutUnbuffered {
    fn new() -> Self {
        Self { stdout: io::stdout() }
    }
}

impl Write for StdoutUnbuffered {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let result = self.stdout.write(buf);
        self.stdout.flush()?;
        result
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }
}

/// Инициализация логгера
/// Пример использования в main.rs
///
/// ```rust
/// fn main() {
///     // Инициализация логгера с именем текущего пакета
///     logging::init_logger(env!("CARGO_PKG_NAME"));
///
///     // Остальной код инициализации сервера
///     // ...
///
///     // Пример логирования
///     tracing::info!("Starting server on http://0.0.0.0:8023");
///
///     // Пример логирования HTTP-запроса
///     tracing::info!(
///         http_request = json!({
///             "host": "127.0.0.1",
///             "method": "GET",
///             "path": "/api/v1/upload-ui/upload-ui-multiple/123/123",
///             "version": "HTTP/1.1"
///         }),
///         "Hello from tracing!"
///     );
///
///     // Пример логирования HTTP-ответа
///     tracing::info!(
///         http_request = json!({
///             "host": "127.0.0.1",
///             "method": "GET",
///             "path": "/api/v1/upload-ui/upload-ui-multiple/123/123",
///             "version": "HTTP/1.1"
///         }),
///         http_response = json!({
///             "duration": "83.845µs",
///             "message": "OK",
///             "status": 200
///         }),
///         "HTTP Response sent"
///     );
/// }
/// ```
pub fn init_logger(service_name: &str) {
    if std::env::var_os("RUST_LOG").is_none() {
        // Устанавливаем значения по умолчанию, если не заданы
        std::env::set_var(
            "RUST_LOG",
            format!(
                "{}=debug,\
                tower_http=debug,\
                axum::rejection=trace,\
                api=info,\
                http_response=info,\
                http_failure=info",
                service_name
            ),
        );
    }

    let env_filter = EnvFilter::from_default_env();
    let formatting_layer = tracing_subscriber::fmt::layer()
        .event_format(JsonFormatter::new(service_name))
        .with_writer(StdoutUnbuffered::new);

    let subscriber = Registry::default()
        .with(env_filter)
        .with(formatting_layer);

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set subscriber");

    tracing::info!("Logger initialized for service: {}", service_name);
}




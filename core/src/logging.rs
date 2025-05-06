// use chrono::{DateTime, Utc};
// use lazy_regex::regex::Regex;
// use serde::Serialize;
// use serde_json::{json, Value};
// use std::collections::HashMap;
// use std::fmt;
// use std::io::{self, Write};
// use tracing::{Event, Subscriber, field::{Field, Visit}};
// use tracing::span::{Attributes, Id, Record};
// use tracing_subscriber::{
//     fmt::{format::Writer, FmtContext, FormatEvent, FormatFields},
//     EnvFilter, Registry,
// };
// use tracing_subscriber::layer::{Context, SubscriberExt};
// use tracing_subscriber::registry::LookupSpan;
// use ulid::Ulid;
//
// /// Основная структура для хранения данных лога
// #[derive(Serialize, Debug, Default)]
// pub struct LogRecord {
//     timestamp: String,
//     level: String,
//     message: String,
//     service: String,
//     target: String,
//     trace_id: String,
//
//     // HTTP данные
//     #[serde(skip_serializing_if = "Option::is_none")]
//     host: Option<String>,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     method: Option<String>,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     path: Option<String>,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     version: Option<String>,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     status: Option<u16>,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     duration: Option<String>,
//
//     // Дополнительные поля
//     #[serde(default, skip_serializing_if = "HashMap::is_empty")]
//     additional_fields: HashMap<String, Value>,
// }
//
// /// Хранилище данных для спанов
// #[derive(Debug)]
// struct SpanData(HashMap<String, Value>);
//
// /// Единый визитор для сбора полей
// struct FieldVisitor<'a>(&'a mut HashMap<String, Value>);
//
// impl<'a> Visit for FieldVisitor<'a> {
//     fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
//         self.0.insert(field.name().to_string(), json!(format!("{:?}", value)));
//     }
//
//     fn record_str(&mut self, field: &Field, value: &str) {
//         self.0.insert(field.name().to_string(), json!(value));
//     }
//
//     fn record_i64(&mut self, field: &Field, value: i64) {
//         self.0.insert(field.name().to_string(), json!(value));
//     }
//
//     fn record_u64(&mut self, field: &Field, value: u64) {
//         self.0.insert(field.name().to_string(), json!(value));
//     }
//
//     fn record_bool(&mut self, field: &Field, value: bool) {
//         self.0.insert(field.name().to_string(), json!(value));
//     }
//
//     fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
//         self.0.insert(field.name().to_string(), json!({
//             "error": value.to_string(),
//             "type": std::any::type_name::<dyn std::error::Error>(),
//         }));
//     }
// }
//
// /// Слой для сохранения данных спанов
// pub struct SpanDataLayer;
//
// impl<S> tracing_subscriber::Layer<S> for SpanDataLayer
// where
//     S: Subscriber,
//     S: for<'lookup> LookupSpan<'lookup>,
// {
//     fn on_new_span(
//         &self,
//         attrs: &Attributes<'_>,
//         id: &Id,
//         ctx: Context<'_, S>,
//     ) {
//         let span = ctx.span(id).expect("Span not found");
//         let mut fields = HashMap::new();
//         let mut visitor = FieldVisitor(&mut fields);
//         attrs.record(&mut visitor);
//
//         let storage = SpanData(fields);
//         let mut extensions = span.extensions_mut();
//         extensions.insert(storage);
//     }
//
//     fn on_record(
//         &self,
//         id: &Id,
//         values: &Record<'_>,
//         ctx: Context<'_, S>,
//     ) {
//         let span = ctx.span(id).expect("Span not found");
//         let mut extensions = span.extensions_mut();
//
//         if let Some(storage) = extensions.get_mut::<SpanData>() {
//             let mut visitor = FieldVisitor(&mut storage.0);
//             values.record(&mut visitor);
//         }
//     }
// }
//
// /// Форматтер для логов JSON
// struct JsonFormatter {
//     service_name: String,
// }
//
// impl JsonFormatter {
//     fn new(service_name: impl Into<String>) -> Self {
//         Self { service_name: service_name.into() }
//     }
//
//     /// Извлекает данные из JSON-подстроки
//     fn extract_json_from_message(&self, message: &str, json_key: &str) -> Option<Value> {
//         // Паттерн для поиска "key":{...}
//         let pattern = format!(r#""{}":\s*(\{{.*?\}})"#, json_key);
//         let re = Regex::new(&pattern).ok()?;
//
//         if let Some(captures) = re.captures(message) {
//             if let Some(json_str) = captures.get(1) {
//                 return serde_json::from_str::<Value>(json_str.as_str()).ok();
//             }
//         }
//
//         // Альтернативный формат: "key":"value1":"value2",...
//         let json_prefix = format!(r#""{}":"#, json_key);
//         if message.contains(&json_prefix) {
//             if let Some(start_pos) = message.find(&json_prefix) {
//                 let start_content = start_pos + json_prefix.len();
//                 let content_str = message[start_content..].trim();
//
//                 // Если контент не начинается с {, добавим скобки
//                 if !content_str.starts_with('{') {
//                     let json_str = format!("{{{}}}", content_str);
//                     return serde_json::from_str::<Value>(&json_str).ok();
//                 } else {
//                     return serde_json::from_str::<Value>(content_str).ok();
//                 }
//             }
//         }
//
//         None
//     }
//
//     /// Парсит строку длительности в миллисекунды
//     fn parse_duration(&self, duration_str: &str) -> Option<f64> {
//         if duration_str.ends_with("µs") {
//             duration_str.trim_end_matches("µs").parse::<f64>().ok().map(|v| v / 1000.0)
//         } else if duration_str.ends_with("ms") {
//             duration_str.trim_end_matches("ms").parse::<f64>().ok()
//         } else if duration_str.ends_with("s") {
//             duration_str.trim_end_matches("s").parse::<f64>().ok().map(|v| v * 1000.0)
//         } else {
//             None
//         }
//     }
// }
//
// impl<S, N> FormatEvent<S, N> for JsonFormatter
// where
//     S: Subscriber + for<'a> LookupSpan<'a>,
//     N: for<'a> FormatFields<'a> + 'static,
// {
//     fn format_event(
//         &self,
//         ctx: &FmtContext<'_, S, N>,
//         mut writer: Writer<'_>,
//         event: &Event<'_>,
//     ) -> fmt::Result {
//         let metadata = event.metadata();
//         let now: DateTime<Utc> = Utc::now();
//
//         // Создаем базовую структуру лога
//         let mut log_record = LogRecord {
//             timestamp: now.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string(),
//             level: metadata.level().to_string().to_lowercase(),
//             message: String::new(),
//             service: self.service_name.clone(),
//             target: metadata.target().to_string(),
//             trace_id: Ulid::new().to_string(),
//             ..Default::default()
//         };
//
//         // Собираем поля события
//         let mut fields = HashMap::new();
//         let mut visitor = FieldVisitor(&mut fields);
//         event.record(&mut visitor);
//
//         // Извлекаем сообщение
//         let mut has_http_response_in_message = false;
//         if let Some(msg) = fields.get("message") {
//             if let Some(msg_str) = msg.as_str() {
//                 log_record.message = msg_str.to_string();
//
//                 // Проверяем, содержит ли сообщение "http_response":{...}
//                 if msg_str.contains(r#""http_response":"#) || msg_str.contains(r#""http_response":{"#) {
//                     has_http_response_in_message = true;
//
//                     // Извлекаем JSON из строки сообщения
//                     if let Some(http_response) = self.extract_json_from_message(msg_str, "http_response") {
//                         if let Some(obj) = http_response.as_object() {
//                             // Если есть message в http_response, используем его
//                             if let Some(response_msg) = obj.get("message").and_then(|v| v.as_str()) {
//                                 log_record.message = response_msg.to_string();
//                             }
//
//                             // Извлекаем остальные поля
//                             if let Some(status) = obj.get("status") {
//                                 if let Some(status_num) = status.as_u64() {
//                                     log_record.status = Some(status_num as u16);
//                                 } else if let Some(status_str) = status.as_str() {
//                                     if let Ok(status_num) = status_str.parse::<u16>() {
//                                         log_record.status = Some(status_num);
//                                     }
//                                 }
//                             }
//
//                             if let Some(duration) = obj.get("duration").and_then(|v| v.as_str()) {
//                                 log_record.duration = Some(duration.to_string());
//                             }
//
//                             // Добавляем остальные поля из http_response
//                             for (key, value) in obj {
//                                 if !["message", "status", "duration"].contains(&key.as_str()) {
//                                     log_record.additional_fields.insert(key.clone(), value.clone());
//                                 }
//                             }
//                         }
//                     }
//                 }
//
//                 // Также проверяем наличие "http_request":{...}
//                 if msg_str.contains(r#""http_request":"#) || msg_str.contains(r#""http_request":{"#) {
//                     if let Some(http_request) = self.extract_json_from_message(msg_str, "http_request") {
//                         if let Some(obj) = http_request.as_object() {
//                             if let Some(host) = obj.get("host").and_then(|v| v.as_str()) {
//                                 log_record.host = Some(host.to_string());
//                             }
//                             if let Some(method) = obj.get("method").and_then(|v| v.as_str()) {
//                                 log_record.method = Some(method.to_string());
//                             }
//                             if let Some(path) = obj.get("path").and_then(|v| v.as_str()) {
//                                 log_record.path = Some(path.to_string());
//                             }
//                             if let Some(version) = obj.get("version").and_then(|v| v.as_str()) {
//                                 log_record.version = Some(version.to_string());
//                             }
//
//                             // Добавляем остальные поля из http_request
//                             for (key, value) in obj {
//                                 if !["host", "method", "path", "version"].contains(&key.as_str()) {
//                                     log_record.additional_fields.insert(key.clone(), value.clone());
//                                 }
//                             }
//                         }
//                     }
//                 }
//             } else {
//                 log_record.message = msg.to_string();
//             }
//         }
//
//         // Если сообщение все еще пустое, используем стандартное сообщение
//         if log_record.message.is_empty() {
//             log_record.message = format!("Event in {}", metadata.target());
//         }
//
//         // Получаем данные из спанов
//         if let Some(scope) = ctx.event_scope() {
//             // Собираем сначала все данные из спанов
//             let mut http_request_data = HashMap::new();
//             let mut all_spans_data = Vec::new();
//
//             // Собираем данные из всех спанов для последующей обработки
//             for span in scope.from_root() {
//                 let span_name = span.name().to_string();
//                 if let Some(span_data) = span.extensions().get::<SpanData>() {
//                     // Если это HTTP запрос, сохраняем его данные отдельно
//                     if span_name == "http_request" {
//                         for (key, value) in &span_data.0 {
//                             http_request_data.insert(key.clone(), value.clone());
//                         }
//                     }
//
//                     // Сохраняем все данные спана для последующей обработки
//                     all_spans_data.push((span_name, span_data.0.clone()));
//                 }
//             }
//
//             // Обрабатываем собранные данные спанов
//             for (span_name, span_data) in all_spans_data {
//                 // Обрабатываем данные HTTP запроса
//                 if span_name == "http_request" {
//                     if let Some(host) = span_data.get("host").and_then(|v| v.as_str()) {
//                         log_record.host = Some(host.to_string());
//                     }
//                     if let Some(method) = span_data.get("method").and_then(|v| v.as_str()) {
//                         log_record.method = Some(method.to_string());
//                     }
//                     if let Some(path) = span_data.get("path").and_then(|v| v.as_str()) {
//                         log_record.path = Some(path.to_string());
//                     }
//                     if let Some(version) = span_data.get("version").and_then(|v| v.as_str()) {
//                         log_record.version = Some(version.to_string());
//                     }
//                 }
//
//                 // Обрабатываем данные HTTP ответа
//                 if span_name == "http_response" {
//                     // Применяем данные HTTP запроса к ответу, если их нет в самом ответе
//                     if log_record.host.is_none() && http_request_data.contains_key("host") {
//                         if let Some(host) = http_request_data.get("host").and_then(|v| v.as_str()) {
//                             log_record.host = Some(host.to_string());
//                         }
//                     }
//                     if log_record.method.is_none() && http_request_data.contains_key("method") {
//                         if let Some(method) = http_request_data.get("method").and_then(|v| v.as_str()) {
//                             log_record.method = Some(method.to_string());
//                         }
//                     }
//                     if log_record.path.is_none() && http_request_data.contains_key("path") {
//                         if let Some(path) = http_request_data.get("path").and_then(|v| v.as_str()) {
//                             log_record.path = Some(path.to_string());
//                         }
//                     }
//                     if log_record.version.is_none() && http_request_data.contains_key("version") {
//                         if let Some(version) = http_request_data.get("version").and_then(|v| v.as_str()) {
//                             log_record.version = Some(version.to_string());
//                         }
//                     }
//
//                     // Получаем статус
//                     if let Some(status) = span_data.get("status") {
//                         if let Some(status_num) = status.as_u64() {
//                             log_record.status = Some(status_num as u16);
//                         } else if let Some(status_str) = status.as_str() {
//                             if let Ok(status_num) = status_str.parse::<u16>() {
//                                 log_record.status = Some(status_num);
//                             }
//                         }
//                     }
//
//                     // Получаем длительность
//                     if let Some(duration) = span_data.get("duration").and_then(|v| v.as_str()) {
//                         log_record.duration = Some(duration.to_string());
//                     }
//
//                     // Если в span.http_response есть поле message, используем его
//                     if let Some(msg) = span_data.get("message").and_then(|v| v.as_str()) {
//                         log_record.message = msg.to_string();
//                     }
//                 }
//
//                 // Копируем дополнительные поля из спанов
//                 for (key, value) in &span_data {
//                     if !["host", "method", "path", "version", "status", "duration", "message"].contains(&key.as_str()) {
//                         log_record.additional_fields.insert(key.clone(), value.clone());
//                     }
//                 }
//             }
//
//             // Если у нас есть HTTP ответ (есть status или duration), но нет данных HTTP запроса,
//             // попробуем достать их из дополнительных полей
//             if (log_record.status.is_some() || log_record.duration.is_some()) &&
//                 (log_record.host.is_none() || log_record.method.is_none() || log_record.path.is_none()) {
//                 if let Some(fields) = log_record.additional_fields.get("fields").and_then(|v| v.as_object()) {
//                     if log_record.host.is_none() {
//                         if let Some(host) = fields.get("host").and_then(|v| v.as_str()) {
//                             log_record.host = Some(host.to_string());
//                         }
//                     }
//                     if log_record.method.is_none() {
//                         if let Some(method) = fields.get("method").and_then(|v| v.as_str()) {
//                             log_record.method = Some(method.to_string());
//                         }
//                     }
//                     if log_record.path.is_none() {
//                         if let Some(path) = fields.get("path").and_then(|v| v.as_str()) {
//                             log_record.path = Some(path.to_string());
//                         }
//                     }
//                     if log_record.version.is_none() {
//                         if let Some(version) = fields.get("version").and_then(|v| v.as_str()) {
//                             log_record.version = Some(version.to_string());
//                         }
//                     }
//                 }
//             }
//         }
//
//         // Добавляем оставшиеся поля из события, если они не были обработаны ранее
//         for (key, value) in fields {
//             if !["message", "http_request", "http_response"].contains(&key.as_str()) {
//                 log_record.additional_fields.insert(key, value);
//             }
//         }
//
//         // Сериализуем и записываем
//         serde_json::to_string(&log_record)
//             .map_err(|_| fmt::Error)
//             .and_then(|json_str| writeln!(writer, "{}", json_str))
//     }
// }
//
// /// Обертка для stdout без буферизации
// struct StdoutUnbuffered {
//     stdout: io::Stdout,
// }
//
// impl StdoutUnbuffered {
//     fn new() -> Self {
//         Self { stdout: io::stdout() }
//     }
// }
//
// impl Write for StdoutUnbuffered {
//     fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
//         let result = self.stdout.write(buf);
//         self.stdout.flush()?;
//         result
//     }
//
//     fn flush(&mut self) -> io::Result<()> {
//         self.stdout.flush()
//     }
// }
//
// /// Инициализация логгера с указанием имени сервиса
// pub fn init_logger(service_name: &str) {
//     if std::env::var_os("RUST_LOG").is_none() {
//         // Устанавливаем значения по умолчанию, если не заданы
//         std::env::set_var(
//             "RUST_LOG",
//             format!(
//                 "{}=debug,\
//                 tower_http=debug,\
//                 api=info,\
//                 http_response=info,\
//                 core=info",
//                 service_name
//             ),
//         );
//     }
//
//     let env_filter = EnvFilter::from_default_env();
//     let formatting_layer = tracing_subscriber::fmt::layer()
//         .event_format(JsonFormatter::new(service_name))
//         .with_writer(StdoutUnbuffered::new);
//
//     let subscriber = Registry::default()
//         .with(SpanDataLayer)  // Сначала добавляем наш слой для сбора данных спанов
//         .with(env_filter)
//         .with(formatting_layer);
//
//     tracing::subscriber::set_global_default(subscriber)
//         .expect("Failed to set subscriber");
//
//     tracing::info!("Logger initialized for service: {}", service_name);
// }


use chrono::{DateTime, Utc};
use lazy_regex::regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fmt;
use std::io::{self, Write};
use tracing::{Event, Subscriber, field::{Field, Visit}};
use tracing::span::{Attributes, Id, Record};
use tracing_subscriber::{
    fmt::{format::Writer, FmtContext, FormatEvent, FormatFields},
    EnvFilter, Registry,
};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use ulid::Ulid;

/// Основная структура для хранения данных лога
#[derive(Serialize, Debug, Default)]
pub struct LogRecord {
    timestamp: String,
    level: String,
    message: String,
    service: String,
    target: String,
    trace_id: String,

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
    duration: Option<String>,

    // Дополнительные поля
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    additional_fields: HashMap<String, Value>,
}

impl LogRecord {
    /// Создает новую запись лога с базовыми полями
    fn new(service_name: &str, metadata: &tracing::Metadata<'_>) -> Self {
        let now: DateTime<Utc> = Utc::now();

        Self {
            timestamp: now.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string(),
            level: metadata.level().to_string().to_lowercase(),
            message: String::new(),
            service: service_name.to_string(),
            target: metadata.target().to_string(),
            trace_id: Ulid::new().to_string(),
            ..Default::default()
        }
    }

    /// Устанавливает сообщение лога
    fn set_message(&mut self, message: String) {
        self.message = message;
    }

    /// Добавляет HTTP запрос в запись лога
    fn add_http_request(&mut self, request: &HashMap<String, Value>) {
        if let Some(host) = request.get("host").and_then(|v| v.as_str()) {
            self.host = Some(host.to_string());
        }
        if let Some(method) = request.get("method").and_then(|v| v.as_str()) {
            self.method = Some(method.to_string());
        }
        if let Some(path) = request.get("path").and_then(|v| v.as_str()) {
            self.path = Some(path.to_string());
        }
        if let Some(version) = request.get("version").and_then(|v| v.as_str()) {
            self.version = Some(version.to_string());
        }

        // Добавляем остальные поля
        for (key, value) in request {
            if !["host", "method", "path", "version"].contains(&key.as_str()) {
                self.additional_fields.insert(key.clone(), value.clone());
            }
        }
    }

    /// Добавляет HTTP ответ в запись лога
    fn add_http_response(&mut self, response: &HashMap<String, Value>) {
        // Обрабатываем специальные поля ответа
        if let Some(msg) = response.get("message").and_then(|v| v.as_str()) {
            self.message = msg.to_string();
        }

        // Обрабатываем статус
        self.set_status_from_value(response.get("status"));

        // Обрабатываем длительность
        if let Some(duration) = response.get("duration").and_then(|v| v.as_str()) {
            self.duration = Some(duration.to_string());
        }

        // Добавляем остальные поля
        for (key, value) in response {
            if !["message", "status", "duration"].contains(&key.as_str()) {
                self.additional_fields.insert(key.clone(), value.clone());
            }
        }
    }

    /// Устанавливает статус из различных типов значений
    fn set_status_from_value(&mut self, status_value: Option<&Value>) {
        if let Some(status) = status_value {
            if let Some(status_num) = status.as_u64() {
                self.status = Some(status_num as u16);
            } else if let Some(status_str) = status.as_str() {
                if let Ok(status_num) = status_str.parse::<u16>() {
                    self.status = Some(status_num);
                }
            }
        }
    }

    /// Добавляет дополнительные поля в запись лога
    fn add_additional_fields(&mut self, fields: &HashMap<String, Value>, exclude_keys: &[&str]) {
        for (key, value) in fields {
            if !exclude_keys.contains(&key.as_str()) {
                self.additional_fields.insert(key.clone(), value.clone());
            }
        }
    }

    /// Примененяет данные HTTP запроса к ответу, если они отсутствуют
    fn apply_http_request_data_if_missing(&mut self, http_request_data: &HashMap<String, Value>) {
        if self.host.is_none() {
            if let Some(host) = http_request_data.get("host").and_then(|v| v.as_str()) {
                self.host = Some(host.to_string());
            }
        }
        if self.method.is_none() {
            if let Some(method) = http_request_data.get("method").and_then(|v| v.as_str()) {
                self.method = Some(method.to_string());
            }
        }
        if self.path.is_none() {
            if let Some(path) = http_request_data.get("path").and_then(|v| v.as_str()) {
                self.path = Some(path.to_string());
            }
        }
        if self.version.is_none() {
            if let Some(version) = http_request_data.get("version").and_then(|v| v.as_str()) {
                self.version = Some(version.to_string());
            }
        }
    }

    /// Извлекает HTTP данные из дополнительных полей
    fn extract_http_data_from_fields(&mut self) {
        // Если у нас есть HTTP ответ (есть status или duration), но нет данных HTTP запроса,
        // попробуем достать их из дополнительных полей
        if (self.status.is_some() || self.duration.is_some()) &&
            (self.host.is_none() || self.method.is_none() || self.path.is_none()) {
            if let Some(fields) = self.additional_fields.get("fields").and_then(|v| v.as_object()) {
                let fields_map: HashMap<String, Value> = fields.clone().into_iter().collect();

                if self.host.is_none() {
                    if let Some(host) = fields_map.get("host").and_then(|v| v.as_str()) {
                        self.host = Some(host.to_string());
                    }
                }
                if self.method.is_none() {
                    if let Some(method) = fields_map.get("method").and_then(|v| v.as_str()) {
                        self.method = Some(method.to_string());
                    }
                }
                if self.path.is_none() {
                    if let Some(path) = fields_map.get("path").and_then(|v| v.as_str()) {
                        self.path = Some(path.to_string());
                    }
                }
                if self.version.is_none() {
                    if let Some(version) = fields_map.get("version").and_then(|v| v.as_str()) {
                        self.version = Some(version.to_string());
                    }
                }
            }
        }
    }
}

/// Хранилище данных для спанов
#[derive(Debug)]
struct SpanData(HashMap<String, Value>);

/// Единый визитор для сбора полей
struct FieldVisitor<'a>(&'a mut HashMap<String, Value>);

impl<'a> Visit for FieldVisitor<'a> {
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

/// Слой для сохранения данных спанов
pub struct SpanDataLayer;

impl<S> tracing_subscriber::Layer<S> for SpanDataLayer
where
    S: Subscriber,
    S: for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(
        &self,
        attrs: &Attributes<'_>,
        id: &Id,
        ctx: Context<'_, S>,
    ) {
        let span = ctx.span(id).expect("Span not found");
        let mut fields = HashMap::new();
        let mut visitor = FieldVisitor(&mut fields);
        attrs.record(&mut visitor);

        let storage = SpanData(fields);
        let mut extensions = span.extensions_mut();
        extensions.insert(storage);
    }

    fn on_record(
        &self,
        id: &Id,
        values: &Record<'_>,
        ctx: Context<'_, S>,
    ) {
        let span = ctx.span(id).expect("Span not found");
        let mut extensions = span.extensions_mut();

        if let Some(storage) = extensions.get_mut::<SpanData>() {
            let mut visitor = FieldVisitor(&mut storage.0);
            values.record(&mut visitor);
        }
    }
}

/// Форматтер для логов JSON
struct JsonFormatter {
    service_name: String,
}

impl JsonFormatter {
    fn new(service_name: impl Into<String>) -> Self {
        Self { service_name: service_name.into() }
    }

    /// Извлекает данные из JSON-подстроки
    fn extract_json_from_message(&self, message: &str, json_key: &str) -> Option<Value> {
        // Паттерн для поиска "key":{...}
        let pattern = format!(r#""{}":\s*(\{{.*?\}})"#, json_key);
        let re = Regex::new(&pattern).ok()?;

        if let Some(captures) = re.captures(message) {
            if let Some(json_str) = captures.get(1) {
                return serde_json::from_str::<Value>(json_str.as_str()).ok();
            }
        }

        // Альтернативный формат: "key":"value1":"value2",...
        let json_prefix = format!(r#""{}":"#, json_key);
        if message.contains(&json_prefix) {
            if let Some(start_pos) = message.find(&json_prefix) {
                let start_content = start_pos + json_prefix.len();
                let content_str = message[start_content..].trim();

                // Если контент не начинается с {, добавим скобки
                if !content_str.starts_with('{') {
                    let json_str = format!("{{{}}}", content_str);
                    return serde_json::from_str::<Value>(&json_str).ok();
                } else {
                    return serde_json::from_str::<Value>(content_str).ok();
                }
            }
        }

        None
    }

    /// Обрабатывает сообщение события и извлекает HTTP данные
    fn process_event_message(&self, message: &str, log_record: &mut LogRecord) -> bool {
        let mut has_http_response_in_message = false;

        // Проверяем наличие http_response
        if message.contains(r#""http_response":"#) || message.contains(r#""http_response":{"#) {
            has_http_response_in_message = true;

            // Извлекаем JSON из строки сообщения
            if let Some(http_response) = self.extract_json_from_message(message, "http_response") {
                if let Some(obj) = http_response.as_object() {
                    let response_map: HashMap<String, Value> = obj.clone().into_iter().collect();
                    log_record.add_http_response(&response_map);
                }
            }
        }

        // Проверяем наличие http_request
        if message.contains(r#""http_request":"#) || message.contains(r#""http_request":{"#) {
            if let Some(http_request) = self.extract_json_from_message(message, "http_request") {
                if let Some(obj) = http_request.as_object() {
                    let request_map: HashMap<String, Value> = obj.clone().into_iter().collect();
                    log_record.add_http_request(&request_map);
                }
            }
        }

        // Проверяем наличие http_request
        if message.contains(r#""http_failure":"#) || message.contains(r#""http_failure":{"#) {
            if let Some(http_failure) = self.extract_json_from_message(message, "http_failure") {
                if let Some(obj) = http_failure.as_object() {
                    let request_map: HashMap<String, Value> = obj.clone().into_iter().collect();
                    log_record.add_http_request(&request_map);
                }
            }
        }

        has_http_response_in_message
    }

    /// Процессинг данных из спанов
    fn process_spans<'a, S, I>(&self,
                               log_record: &mut LogRecord,
                               scope_iter: I)
    where
        S: Subscriber + for<'lookup> LookupSpan<'lookup>,
        I: Iterator<Item = tracing_subscriber::registry::SpanRef<'a, S>>,
    {
        // Собираем данные HTTP запроса
        let mut http_request_data = HashMap::new();

        // Сначала соберем все ссылки в вектор для многократного использования
        let span_refs: Vec<_> = scope_iter.collect();

        // Первый проход: собираем данные HTTP запроса
        for span in &span_refs {
            if span.name() == "http_request" {
                if let Some(span_data) = span.extensions().get::<SpanData>() {
                    for (key, value) in &span_data.0 {
                        http_request_data.insert(key.clone(), value.clone());
                    }
                }
            }
        }

        // Второй проход: обрабатываем все спаны
        for span in &span_refs {
            let span_name = span.name().to_string();
            if let Some(span_data) = span.extensions().get::<SpanData>() {
                match span_name.as_str() {
                    "http_request" => {
                        log_record.add_http_request(&span_data.0);
                    },
                    "http_response" => {
                        // Применяем данные HTTP запроса к ответу, если их нет в самом ответе
                        log_record.apply_http_request_data_if_missing(&http_request_data);

                        // Добавляем данные HTTP ответа
                        log_record.add_http_response(&span_data.0);
                    },
                    _ => {
                        // Копируем дополнительные поля из других спанов
                        log_record.add_additional_fields(&span_data.0, &["host", "method", "path", "version", "status", "duration", "message"]);
                    }
                }
            }
        }
    }

    // Метод process_span_data больше не нужен, так как его функциональность
    // теперь включена в process_spans
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
        let metadata = event.metadata();

        // Создаем базовую структуру лога
        let mut log_record = LogRecord::new(&self.service_name, metadata);

        // Собираем поля события
        let mut fields = HashMap::new();
        let mut visitor = FieldVisitor(&mut fields);
        event.record(&mut visitor);

        // Обрабатываем сообщение
        let mut has_http_response_in_message = false;
        if let Some(msg) = fields.get("message") {
            if let Some(msg_str) = msg.as_str() {
                log_record.set_message(msg_str.to_string());
                has_http_response_in_message = self.process_event_message(msg_str, &mut log_record);
            } else {
                log_record.set_message(msg.to_string());
            }
        }

        // Если сообщение все еще пустое, используем стандартное сообщение
        if log_record.message.is_empty() {
            log_record.set_message(format!("Event in {}", metadata.target()));
        }

        // Получаем данные из спанов
        if let Some(scope) = ctx.event_scope() {
            self.process_spans(&mut log_record, scope.from_root());
        }

        // Извлекаем HTTP данные из дополнительных полей, если нужно
        log_record.extract_http_data_from_fields();

        // Добавляем оставшиеся поля из события, если они не были обработаны ранее
        log_record.add_additional_fields(&fields, &["message", "http_request", "http_response"]);

        // Сериализуем и записываем
        serde_json::to_string(&log_record)
            .map_err(|_| fmt::Error)
            .and_then(|json_str| writeln!(writer, "{}", json_str))
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

/// Инициализация логгера с указанием имени сервиса
pub fn init_logger(service_name: &str) {
    if std::env::var_os("RUST_LOG").is_none() {
        // Устанавливаем значения по умолчанию, если не заданы
        std::env::set_var(
            "RUST_LOG",
            format!(
                "{}=debug,\
                tower_http=debug,\
                api=trace,\
                response_trace=info,\
                http_response=info,\
                core=info",
                service_name
            ),
        );
    }

    let env_filter = EnvFilter::from_default_env();
    let formatting_layer = tracing_subscriber::fmt::layer()
        .event_format(JsonFormatter::new(service_name))
        .with_writer(StdoutUnbuffered::new);

    let subscriber = Registry::default()
        .with(SpanDataLayer)  // Сначала добавляем наш слой для сбора данных спанов
        .with(env_filter)
        .with(formatting_layer);

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set subscriber");

    tracing::info!("Logger initialized for service: {}", service_name);
}
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{Event, Subscriber, field::{Field, Visit}};
use tracing_subscriber::{
    fmt::{format::Writer, time::FormatTime, FmtContext, FormatEvent, FormatFields},
    EnvFilter, Registry,
};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use std::io::{self, Write};
use std::any::TypeId;
use lazy_regex::regex;

struct CustomFormatter {}
//
impl CustomFormatter {
    fn new() -> Self { Self {} }
}

struct MessageVisitor(HashMap<String, Value>);

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
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
}

fn process_log_message(message: &str) -> Value {
    let mut result = json!({});

    // Регулярное выражение для извлечения JSON-объекта "request"
    let re = regex!(r#""request":\s*(\{[^}]+\})"#);

    if let Some(captures) = re.captures(message) {
        if let Some(request_json_str) = captures.get(1) {
            // Парсим извлеченный JSON
            if let Ok(mut request_json) = serde_json::from_str::<Value>(request_json_str.as_str()) {
                // Извлекаем поле "message" из request_json, если оно есть
                if let Some(request_message) = request_json.as_object_mut().and_then(|obj| obj.remove("message")) {
                    result["message"] = request_message;
                }

                // Переносим все оставшиеся поля из request_json в result
                if let Some(obj) = request_json.as_object() {
                    for (key, value) in obj {
                        result[key] = value.clone();
                    }
                }
            }
        }

        // Если поле "message" не было найдено в request_json, используем остаток исходного сообщения
        if !result.as_object().map_or(false, |obj| obj.contains_key("message")) {
            let new_message = re.replace(message, "").trim().to_string();
            result["message"] = json!(new_message);
        }
    } else {
        // Если "request":{} не найден, сохраняем всю строку как message
        result["message"] = json!(message);
    }

    result
}

impl<S, N> FormatEvent<S, N> for CustomFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        let metadata = event.metadata();
        let now: DateTime<Utc> = Utc::now();

        let mut visitor = MessageVisitor(HashMap::new());
        event.record(&mut visitor);

        let mut log_data = visitor.0;

        // Extract information from spans (for request data)
        if let Some(scope) = ctx.event_scope() {
            for span in scope.from_root() {
                let extensions = span.extensions();
                if let Some(fields) = extensions.get::<HashMap<String, String>>() {
                    for (key, value) in fields {
                        log_data.insert(key.clone(), json!(value));
                    }
                }
            }
        }

        // log_data.insert("timestamp".to_string(), json!(now.to_rfc3339_opts(chrono::SecondsFormat::Micros, true)));
        // log_data.insert("level".to_string(), json!(metadata.level().to_string().to_lowercase()));
        // log_data.insert("service".to_string(), json!("uploader"));

        // Format timestamp for json
        let json_timestamp = now.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string();

        // Format level
        let level = metadata.level().to_string().to_lowercase();

        // Extract message
        let message = if let Some(msg) = log_data.get("message") {
            msg.as_str().unwrap_or("").to_string()
        } else {
            format!("{:?}", event)
        };

        // // Extract message
        // let message = if let Some(msg) = log_data.get("message") {
        //     msg.as_str().unwrap_or("").to_string()
        // } else {
        //     format!("{:?}", event)
        // };
        // log_data.insert("message".to_string(), json!(message));
        //
        // // Map poem-specific fields
        // if let Some(remote_addr) = log_data.remove("remote_addr") {
        //     log_data.insert("host".to_string(), remote_addr);
        // }
        // if let Some(method) = log_data.remove("method") {
        //     log_data.insert("method".to_string(), method);
        // }
        // if let Some(uri) = log_data.remove("uri") {
        //     log_data.insert("path".to_string(), uri);
        // }
        // if let Some(status) = log_data.remove("status") {
        //     log_data.insert("status".to_string(), status);
        // }
        //
        // let json_string = serde_json::to_string(&log_data).unwrap();
        //
        // writeln!(
        //     writer,
        //     "host_key={} json={} source_type=stdin timestamp={}",
        //     self.host_key,
        //     json_string,
        //     now.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
        // )
        // Extract message
        // Prepare JSON data



        let mut json_data = json!({
            "level": level,
            "message": message,
            "service": "uploader",
            "timestamp": json_timestamp,
        });

        let mut process_data = process_log_message(&message);
        for (key, value) in process_data.as_object().unwrap() {
            json_data[key] = value.clone();
        }

        // // Add additional fields if present
        // if let Some(host) = log_data.get("remote_addr") {
        //     json_data["host"] = json!(host);
        // }
        // if let Some(method) = log_data.get("method") {
        //     json_data["method"] = json!(method);
        // }
        // if let Some(uri) = log_data.get("uri") {
        //     json_data["path"] = json!(uri);
        // }
        // if let Some(status) = log_data.get("status") {
        //     json_data["status"] = json!(status);
        // }

        // Format timestamp for the end of the log line
        let end_timestamp = now.format("%Y-%m-%dT%H:%M:%S%.9fZ").to_string();

        // writeln!(
        //     writer,
        //     "host_key={} json={} source_type=stdin timestamp={}",
        //     self.host_key,
        //     json_data.to_string(),
        //     end_timestamp
        // )
        writeln!(
            writer,
            "{}",
            json_data.to_string(),
        )
    }
}


pub fn init_logger() {
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var(
            "RUST_LOG",
            "api=debug,\
            core=debug,\
            tower_http=debug,\
            axum::rejection=trace,\
            services=debug,\
            svaha_mini_uploader_axum=debug",
        );
    }

    let env_filter = EnvFilter::from_default_env();
    let formatting_layer = tracing_subscriber::fmt::layer()
        .event_format(CustomFormatter::new())
        .with_writer(StdoutUnbuffered::new);

    let subscriber = Registry::default()
        .with(env_filter)
        .with(formatting_layer);

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set subscriber");
}

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
// src/logging.rs
use std::net::IpAddr;
use std::time::Duration;

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{HeaderMap, Request, StatusCode},
    response::Response,
    middleware::Next,
};
use axum::body::Bytes;
use rfc7239::{NodeIdentifier, NodeName};
use serde_json::{json, Value};
use tower_http::classify::{ServerErrorsAsFailures, ServerErrorsFailureClass, SharedClassifier};
use tower_http::trace::TraceLayer;
use tracing::{Span};
use std::net::SocketAddr;
use std::sync::Arc;

use std::fmt::Debug;
use std::ops::Deref;
use tracing::field::Field;

// use hyper::body::to_bytes;
// use hyper::{body::Body as HyperBody, Response as HyperResponse};


/// Структура для хранения реального IP-адреса клиента
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct RealIp(pub Option<IpAddr>);

impl From<&Request<Body>> for RealIp {
    fn from(req: &Request<Body>) -> Self {
        // Порядок проверки:
        // 1. X-Real-IP заголовок
        if let Some(real_ip) = extract_ip_from_header(req, "x-real-ip") {
            return RealIp(Some(real_ip));
        }

        // 2. Forwarded заголовок (RFC 7239)
        if let Some(real_ip) = extract_ip_from_forwarded(req) {
            return RealIp(Some(real_ip));
        }

        // 3. X-Forwarded-For заголовок
        if let Some(real_ip) = extract_ip_from_x_forwarded_for(req) {
            return RealIp(Some(real_ip));
        }

        // 4. Оригинальный IP из ConnectInfo
        if let Some(ip) = extract_ip_from_connect_info(req) {
            return RealIp(Some(ip));
        }

        RealIp(None)
    }
}

/// Извлекает IP из обычного заголовка
fn extract_ip_from_header(req: &Request<Body>, header_name: &str) -> Option<IpAddr> {
    req.headers()
        .get(header_name)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<IpAddr>().ok())
}

/// Извлекает IP из заголовка Forwarded (RFC 7239)
fn extract_ip_from_forwarded(req: &Request<Body>) -> Option<IpAddr> {
    req.headers()
        .get("forwarded")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| rfc7239::parse(value).collect::<Result<Vec<_>, _>>().ok())
        .and_then(|forwarded| {
            forwarded
                .into_iter()
                .find_map(|item| match item.forwarded_for {
                    Some(NodeIdentifier {
                             name: NodeName::Ip(ip_addr),
                             ..
                         }) => Some(ip_addr),
                    _ => None,
                })
        })
}

/// Извлекает IP из заголовка X-Forwarded-For
fn extract_ip_from_x_forwarded_for(req: &Request<Body>) -> Option<IpAddr> {
    req.headers()
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            value
                .split(',')
                .map(|value| value.trim())
                .find_map(|value| value.parse::<IpAddr>().ok())
        })
}

/// Извлекает IP из ConnectInfo
fn extract_ip_from_connect_info(req: &Request<Body>) -> Option<IpAddr> {
    if let Some(ConnectInfo(SocketAddr::V4(addr))) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
        return Some(IpAddr::V4(*addr.ip()));
    }
    if let Some(ConnectInfo(SocketAddr::V6(addr))) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
        return Some(IpAddr::V6(*addr.ip()));
    }
    None
}

/// Возвращает реальный IP клиента
pub fn get_real_ip(req: &Request<Body>) -> Option<IpAddr> {
    RealIp::from(req).0
}

/// Мидлвар для добавления данных запроса в extensions ответа
pub async fn request_data_middleware(
    mut request: Request<Body>,
    next: Next,
) -> Response<Body> {
    // Пропускаем favicon.ico
    if request.uri().path() == "/favicon.ico" {
        return next.run(request).await;
    }

    // Собираем данные о запросе
    let remote_addr = get_real_ip(&request)
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let method = request.method().to_string();
    let path = request.uri().to_string();
    let version = format!("{:?}", request.version());

    // Создаем структуру с данными запроса
    let request_data = RequestData {
        remote_addr,
        method,
        path,
        version,
    };

    // Добавляем данные в extensions запроса
    // Здесь у нас есть мутабельный доступ к request
    request.extensions_mut().insert(Arc::new(request_data));

    // Продолжаем обработку запроса
    next.run(request).await
}

/// Безопасно превращает Value в строку JSON
fn safe_json_to_string(value: Value) -> String {
    serde_json::to_string(&value).unwrap_or_else(|_| "{\"error\":\"JSON serialization failed\"}".to_string())
}

/// Создаёт слой трейсинга для HTTP-запросов
pub fn create_tracing_layer() -> TraceLayer<
    SharedClassifier<ServerErrorsAsFailures>,
    impl Fn(&Request<Body>) -> Span + Clone,
    impl Fn(&Request<Body>, &Span) + Clone,
    impl Fn(&Response<Body>, Duration, &Span) + Clone,
    impl Fn(&Bytes, Duration, &Span) + Clone,
    impl Fn(Option<&HeaderMap>, Duration, &Span) + Clone,
    impl Fn(ServerErrorsFailureClass, Duration, &Span) + Clone
> {
    TraceLayer::new_for_http()
        .make_span_with(make_request_span)
        .on_request(on_request)
        .on_response(on_response)
        .on_body_chunk(on_body_chunk)
        .on_eos(on_eos)
        .on_failure(on_failure)
}



/// Создаёт span для входящего запроса
fn make_request_span(request: &Request<Body>) -> Span {
    // Игнорируем favicon.ico запросы
    if request.uri().path() == "/favicon.ico" {
        return tracing::Span::none();
    }

    let remote_addr = get_real_ip(request)
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    // Извлекаем метод и путь запроса
    let method = request.method().to_string();
    let path = request.uri().to_string();
    let version = format!("{:?}", request.version());

    // Создаем span с отдельными полями
    tracing::info_span!(
        "http_request",
        host = %remote_addr,
        method = %method,
        path = %path,
        version = %version,
    )
}

#[derive(Clone)]
struct RequestData {
    method: String,
    path: String,
    remote_addr: String,
    version: String,
}

/// Обрабатывает начало запроса
fn on_request(request: &Request<Body>, _span: &Span) {
    // Логируем начало запроса
    tracing::trace!(
        target: "http_quest",
        version = ?request.version(),
        uri = %request.uri(),
        "Request received"
    );
}

/// Обрабатывает ответ
fn on_response(response: &Response<Body>, latency: Duration, span: &Span) {
    if span.is_none() {
        return;
    }
    let status = response.status().as_u16();
    let msg = response.status().canonical_reason().unwrap_or("Unknown");

    let log = json!({
        "status": status,
        "message": msg,
        "duration": format!("{:?}", latency),
    });

    tracing::info!(target: "http_response", "\"http_response\":{}", safe_json_to_string(log));
}

/// Обрабатывает чанки тела ответа
fn on_body_chunk(_chunk: &Bytes, _latency: Duration, _span: &Span) {
    tracing::trace!("Body chunk processed");
}

/// Обрабатывает окончание стрима
fn on_eos(_trailers: Option<&HeaderMap>, _stream_duration: Duration, _span: &Span) {
    tracing::trace!("End of stream reached");
}

/// Обрабатывает ошибки
fn on_failure(error: ServerErrorsFailureClass, latency: Duration, span: &Span) {
    if span.is_none() {
        return;
    }

    let (status, msg) = match &error {
        ServerErrorsFailureClass::StatusCode(status) => (
            status.as_u16(),
            status.canonical_reason().unwrap_or("Unknown"),
        ),
        ServerErrorsFailureClass::Error(_) => (
            StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            "Internal Server Error",
        ),
    };

    let log = json!({
        "status": status,
        "message": msg,
        "error": format!("{:?}", error),
        "duration": format!("{:?}", latency)
    });

    tracing::warn!(target: "http_failure", "\"http_failure\":{}", safe_json_to_string(log));
}

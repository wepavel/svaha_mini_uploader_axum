// // src/tracing.rs
// use std::net::IpAddr;
// use std::str::FromStr;
//
// use axum::{
//     extract::MatchedPath,
//     http::{Request, StatusCode},
//     response::Response,
//     body::Body,
// };
// use serde_json::json;
// use std::time::Duration;
// use axum::body::Bytes;
// use axum::http::HeaderMap;
// use tower_http::trace::{TraceLayer, DefaultOnBodyChunk, DefaultOnEos, MakeSpan, OnFailure, OnRequest, OnResponse};
// use tracing::{Span, Level};
// use tower_http::classify::{ServerErrorsFailureClass, SharedClassifier};
// use tower_http::classify::ServerErrorsAsFailures;
// use axum::extract::ConnectInfo;
// use std::net::SocketAddr;
// use rfc7239::{NodeIdentifier, NodeName};
//
// #[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
// pub struct RealIp(pub Option<IpAddr>);
//
// impl From<&Request<Body>> for RealIp {
//     fn from(req: &Request<Body>) -> Self {
//         if let Some(real_ip) = req
//             .headers()
//             .get("x-real-ip")
//             .and_then(|value| value.to_str().ok())
//             .and_then(|value| value.parse::<IpAddr>().ok())
//         {
//             return RealIp(Some(real_ip));
//         }
//
//         if let Some(forwarded) = req
//             .headers()
//             .get("forwarded")
//             .and_then(|value| value.to_str().ok())
//             .and_then(|value| rfc7239::parse(value).collect::<Result<Vec<_>, _>>().ok())
//         {
//             if let Some(real_ip) = forwarded
//                 .into_iter()
//                 .find_map(|item| match item.forwarded_for {
//                     Some(NodeIdentifier {
//                              name: NodeName::Ip(ip_addr),
//                              ..
//                          }) => Some(ip_addr),
//                     _ => None,
//                 })
//             {
//                 return RealIp(Some(real_ip));
//             }
//         }
//
//         if let Some(real_ip) = req
//             .headers()
//             .get("x-forwarded-for")
//             .and_then(|value| value.to_str().ok())
//             .and_then(|value| {
//                 value
//                     .split(',')
//                     .map(|value| value.trim())
//                     .find_map(|value| value.parse::<IpAddr>().ok())
//             })
//         {
//             return RealIp(Some(real_ip));
//         }
//
//         // Try to get from ConnectInfo
//         if let Some(ConnectInfo(SocketAddr::V4(addr))) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
//             return RealIp(Some(IpAddr::V4(*addr.ip())));
//         }
//         if let Some(ConnectInfo(SocketAddr::V6(addr))) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
//             return RealIp(Some(IpAddr::V6(*addr.ip())));
//         }
//
//         RealIp(None)
//     }
// }
//
// pub fn get_real_ip(req: &Request<Body>) -> Option<IpAddr> {
//     RealIp::from(req).0
// }
//
// pub fn create_tracing_layer() -> TraceLayer<
//     SharedClassifier<ServerErrorsAsFailures>,
//     impl Fn(&Request<Body>) -> Span + Clone,
//     impl Fn(&Request<Body>, &Span) + Clone,
//     impl Fn(&Response<Body>, Duration, &Span) + Clone,
//     impl Fn(&Bytes, Duration, &Span) + Clone,
//     impl Fn(Option<&HeaderMap>, Duration, &Span) + Clone,
//     impl Fn(ServerErrorsFailureClass, Duration, &Span) + Clone
// > {
//     TraceLayer::new_for_http()
//         .make_span_with(|request: &Request<Body>| {
//
//             let remote_addr: String = get_real_ip(request)
//                 .map(|ip| ip.to_string())
//                 .unwrap_or_else(|| "unknown".to_string());
//
//             // let header_map: HeaderMap = request.headers().clone().into();
//
//             // let request_info = json!({
//             //     "method": request.method().to_string(),
//             //     "path": request.uri().to_string(),
//             //     "version": format!("{:?}", request.version()),
//             //     "host": remote_addr,
//             // });
//             //
//             // tracing::info_span!(
//             //     "\"http_request\":",
//             //     request = %serde_json::to_string(&request_info).unwrap()
//             // )
//             let request_info = json!({
//                 "host": remote_addr,
//                 "method": request.method().to_string(),
//                 "path": request.uri().to_string(),
//                 "version": format!("{:?}", request.version()),
//             });
//
//             let json_string = serde_json::to_string(&request_info).unwrap();
//             let trimmed_json = json_string.trim_start_matches('{').trim_end_matches('}');
//
//             // tracing::info_span!(
//             //     "\"http_request\":",
//             //     "{}", trimmed_json
//             // )
//             if request.uri().path() == "/favicon.ico" {
//                 // Создаем пустой span для favicon.ico
//                 tracing::Span::none()
//             } else {
//                 tracing::info_span!(
//                     "\"http_request\":",
//                     "{}", trimmed_json
//                 )
//             }
//         })
//         .on_request(|request: &Request<Body>, _span: &Span| {
//
//             tracing::trace!(
//                 version = ?request.version(),
//                 uri = %request.uri(),
//                 "Request received"
//             );
//         })
//         .on_response(|response: &Response<Body>, latency: Duration, span: &Span| {
//             if !span.is_none() {
//                 let status = response.status().as_u16();
//                 let msg = response.status().canonical_reason().unwrap_or("Unknown");
//
//                 let log = json!({
//                     "status": status,
//                     "message": msg,
//                     "duration": format!("{:?}", latency)
//                 });
//
//                 tracing::info!(target: "http_response", "\"http_response\":{}", serde_json::to_string(&log).unwrap());
//             }
//
//         })
//         .on_body_chunk(|_chunk: &Bytes, _latency: Duration, _span: &Span| {
//             // You can add custom logic for body chunks if needed
//             tracing::trace!("Проблема в chunk");
//         })
//         .on_eos(|_trailers: Option<&HeaderMap>, _stream_duration: Duration, _span: &Span| {
//             tracing::trace!("Проблема в eos");
//             // You can add custom logic for end of stream if needed
//         })
//         .on_failure(|error: ServerErrorsFailureClass, latency: Duration, span: &Span| {
//             if !span.is_none() {
//                 let (status, msg) = match &error {
//                     ServerErrorsFailureClass::StatusCode(status) => (
//                         status.as_u16(),
//                         status.canonical_reason().unwrap_or("Unknown"),
//                     ),
//                     ServerErrorsFailureClass::Error(error_msg) => (
//                         StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
//                         "Internal Server Error",
//                     ),
//                 };
//
//                 let log = json!({
//                 "status": status,
//                 "message": msg,
//                 "error": format!("{:?}", error),
//                 "duration": format!("{:?}", latency)
//             });
//
//                 tracing::warn!(target: "http_failure", "\"http_failure\":{}", serde_json::to_string(&log).unwrap());
//             }
//
//
//             // tracing::info!("Проблема в on_faulure");
//             // let (status, msg) = match &error {
//             //     ServerErrorsFailureClass::StatusCode(status) => (
//             //         status.as_u16(),
//             //         status.canonical_reason().unwrap_or("Unknown"),
//             //     ),
//             //     ServerErrorsFailureClass::Error(error_msg) => (
//             //         StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
//             //         "Internal Server Error",
//             //     ),
//             // };
//             //
//             // let log = json!({
//             //     "status": status,
//             //     "message": msg,
//             //     "error": format!("{:?}", error),
//             //     "duration": format!("{:?}", latency)
//             // });
//             //
//             // tracing::error!(target: "http_error", "\"error\":{}", serde_json::to_string(&log).unwrap());
//         })
// }
//

// src/tracing.rs
use std::net::IpAddr;
use std::str::FromStr;
use std::time::Duration;

use axum::{
    body::Body,
    extract::{ConnectInfo, MatchedPath},
    http::{HeaderMap, Request, StatusCode},
    response::Response,
};
use axum::body::Bytes;
use rfc7239::{NodeIdentifier, NodeName};
use serde_json::{json, Value};
use tower_http::classify::{ServerErrorsAsFailures, ServerErrorsFailureClass, SharedClassifier};
use tower_http::trace::{MakeSpan, OnFailure, OnRequest, OnResponse, TraceLayer};
use tracing::{Level, Span};

use std::net::SocketAddr;

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

    let request_info = json!({
        "host": remote_addr,
        "method": request.method().to_string(),
        "path": request.uri().to_string(),
        "version": format!("{:?}", request.version()),
    });

    let json_string = safe_json_to_string(request_info);
    let trimmed_json = json_string.trim_start_matches('{').trim_end_matches('}');

    tracing::info_span!(
        "\"http_request\":",
        "{}", trimmed_json
    )
}

/// Обрабатывает начало запроса
fn on_request(request: &Request<Body>, _span: &Span) {
    tracing::trace!(
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
        "duration": format!("{:?}", latency)
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

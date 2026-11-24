use serde::{Deserialize, Serialize};

use crate::cli::{extract_metadata_from_bytes, extract_metadata_from_path, ErrorResponse};

/// JSON request payload for `POST /v1/metadata/extract` when referencing a file path in the Spin sandbox.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetadataExtractRequest {
    pub file: String,
}

/// Generic HTTP Response representation for Fermyon Spin Wagi / WASI HTTP providers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl HttpResponse {
    pub fn ok_json(body: String) -> Self {
        Self {
            status: 200,
            headers: vec![("Content-Type".into(), "application/json".into())],
            body,
        }
    }

    pub fn error_json(status: u16, message: impl Into<String>) -> Self {
        let err = ErrorResponse {
            success: false,
            error: message.into(),
        };
        let body = serde_json::to_string_pretty(&err)
            .unwrap_or_else(|_| "{\"success\":false,\"error\":\"Serialization failure\"}".to_string());
        Self {
            status,
            headers: vec![("Content-Type".into(), "application/json".into())],
            body,
        }
    }
}

/// Handles incoming Fermyon Spin / WASI HTTP requests for `/v1/metadata/...`.
///
/// Supported routes:
/// - `GET /v1/metadata/health` -> health check JSON
/// - `POST /v1/metadata/extract` -> accepts either:
///   1. Raw binary image bytes (`JPEG`, `TIFF`, `PNG`, `WebP`) in the request body (optional `X-File-Name` header or `?file=` query parameter).
///   2. A JSON object `{"file": "<image_path>"}` referencing a mounted image file.
pub fn handle_http_request(
    method: &str,
    uri: &str,
    headers: &[(String, String)],
    body: &[u8],
) -> HttpResponse {
    let (path, query) = uri.split_once('?').unwrap_or((uri, ""));

    if path == "/v1/metadata/health" {
        if !method.eq_ignore_ascii_case("GET") {
            return HttpResponse::error_json(405, "Method Not Allowed (expected GET)");
        }
        return HttpResponse::ok_json(
            r#"{"status":"ok","service":"wasm-img-metadata-spin"}"#.to_string(),
        );
    }

    if path != "/v1/metadata/extract" {
        return HttpResponse::error_json(
            404,
            format!("Endpoint not found: '{path}' (expected /v1/metadata/extract or /v1/metadata/health)"),
        );
    }

    if !method.eq_ignore_ascii_case("POST") {
        return HttpResponse::error_json(405, "Method Not Allowed (expected POST)");
    }

    if body.is_empty() {
        return HttpResponse::error_json(400, "Empty request body (expected raw image bytes or JSON {\"file\": \"...\"})");
    }

    let content_type = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("Content-Type"))
        .map(|(_, v)| v.as_str())
        .unwrap_or("");

    let is_json_request = content_type.contains("application/json")
        || (body.first().copied() == Some(b'{') && !looks_like_binary_image(body));

    if is_json_request {
        let req: MetadataExtractRequest = match serde_json::from_slice(body) {
            Ok(r) => r,
            Err(e) => {
                return HttpResponse::error_json(400, format!("Invalid JSON request payload: {e}"));
            }
        };
        return match extract_metadata_from_path(&req.file) {
            Ok(resp) => {
                let json = serde_json::to_string_pretty(&resp)
                    .unwrap_or_else(|_| "{\"success\":false,\"error\":\"Serialization failure\"}".to_string());
                HttpResponse::ok_json(json)
            }
            Err(err) => {
                let json = serde_json::to_string_pretty(&err)
                    .unwrap_or_else(|_| "{\"success\":false,\"error\":\"Serialization failure\"}".to_string());
                HttpResponse {
                    status: 422,
                    headers: vec![("Content-Type".into(), "application/json".into())],
                    body: json,
                }
            }
        };
    }

    // Raw binary image upload mode
    let file_label = extract_file_label(headers, query);
    match extract_metadata_from_bytes(&file_label, body) {
        Ok(resp) => {
            let json = serde_json::to_string_pretty(&resp)
                .unwrap_or_else(|_| "{\"success\":false,\"error\":\"Serialization failure\"}".to_string());
            HttpResponse::ok_json(json)
        }
        Err(err) => {
            let json = serde_json::to_string_pretty(&err)
                .unwrap_or_else(|_| "{\"success\":false,\"error\":\"Serialization failure\"}".to_string());
            HttpResponse {
                status: 422,
                headers: vec![("Content-Type".into(), "application/json".into())],
                body: json,
            }
        }
    }
}

fn looks_like_binary_image(body: &[u8]) -> bool {
    body.starts_with(&[0xFF, 0xD8])
        || body.starts_with(b"II*\0")
        || body.starts_with(b"MM\0*")
        || body.starts_with(b"\x89PNG\r\n\x1a\n")
        || (body.len() >= 12 && &body[0..4] == b"RIFF" && &body[8..12] == b"WEBP")
}

fn extract_file_label(headers: &[(String, String)], query: &str) -> String {
    if let Some((_, val)) = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("X-File-Name"))
    {
        let trimmed = val.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if (k == "file" || k == "filename") && !v.trim().is_empty() {
                return v.trim().to_string();
            }
        }
    }

    "upload".to_string()
}

pub mod http;

use crate::cli::{extract_metadata_from_bytes, extract_metadata_from_path};

pub struct MetadataProvider;

impl MetadataProvider {
    /// Component-level metadata extraction from a file path, returning the JSON document string.
    pub fn extract_from_path(image_path: &str) -> Result<String, String> {
        match extract_metadata_from_path(image_path) {
            Ok(resp) => serde_json::to_string_pretty(&resp)
                .map_err(|e| format!("{{\"success\":false,\"error\":\"Serialization error: {e}\"}}")),
            Err(err) => {
                let json = serde_json::to_string_pretty(&err)
                    .unwrap_or_else(|_| "{\"success\":false,\"error\":\"Serialization failure\"}".to_string());
                Err(json)
            }
        }
    }

    /// Component-level metadata extraction from raw image bytes, returning the JSON document string.
    pub fn extract_from_bytes(file_label: &str, image_bytes: &[u8]) -> Result<String, String> {
        match extract_metadata_from_bytes(file_label, image_bytes) {
            Ok(resp) => serde_json::to_string_pretty(&resp)
                .map_err(|e| format!("{{\"success\":false,\"error\":\"Serialization error: {e}\"}}")),
            Err(err) => {
                let json = serde_json::to_string_pretty(&err)
                    .unwrap_or_else(|_| "{\"success\":false,\"error\":\"Serialization failure\"}".to_string());
                Err(json)
            }
        }
    }
}

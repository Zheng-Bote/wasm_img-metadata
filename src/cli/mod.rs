use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::container::demux_container;
use crate::exif::parse_tiff_exif;
use crate::iptc::parse_iptc_iim;
use crate::xmp::parse_xmp_packet;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MetadataSections {
    pub exif: Map<String, Value>,
    pub iptc: Map<String, Value>,
    pub xmp: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SuccessResponse {
    pub success: bool,
    pub file: String,
    pub metadata: MetadataSections,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
}

pub fn extract_metadata_from_bytes(file_label: &str, bytes: &[u8]) -> Result<SuccessResponse, ErrorResponse> {
    let payloads = demux_container(bytes).map_err(|e| ErrorResponse {
        success: false,
        error: e,
    })?;

    let mut sections = MetadataSections {
        exif: payloads.extra_exif,
        iptc: payloads.extra_iptc,
        xmp: payloads.extra_xmp,
    };

    let mut iptc_chunks = payloads.iptc_chunks;
    let mut xmp_chunks = payloads.xmp_chunks;

    for exif_chunk in &payloads.exif_chunks {
        if let Ok(outcome) = parse_tiff_exif(exif_chunk) {
            for (k, v) in outcome.exif {
                sections.exif.insert(k, v);
            }
            iptc_chunks.extend(outcome.embedded_iptc);
            xmp_chunks.extend(outcome.embedded_xmp);
        }
    }

    for iptc_chunk in &iptc_chunks {
        parse_iptc_iim(iptc_chunk, &mut sections.iptc);
    }

    for xmp_chunk in &xmp_chunks {
        parse_xmp_packet(xmp_chunk, &mut sections.xmp);
    }

    Ok(SuccessResponse {
        success: true,
        file: file_label.to_string(),
        metadata: sections,
    })
}

pub fn extract_metadata_from_path(image_path: &str) -> Result<SuccessResponse, ErrorResponse> {
    if image_path.trim().is_empty() {
        return Err(ErrorResponse {
            success: false,
            error: "Missing or empty <image_path> argument".to_string(),
        });
    }

    let path = Path::new(image_path);
    let bytes = fs::read(path).map_err(|e| ErrorResponse {
        success: false,
        error: format!("Failed to read file '{}': {}", image_path, e),
    })?;

    extract_metadata_from_bytes(image_path, &bytes)
}

pub fn run_cli_with_args<I, S>(args: I) -> (String, i32)
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let collected: Vec<String> = args.into_iter().map(Into::into).collect();
    // Expect binary name + single argument <image_path>
    if collected.len() != 2 {
        let err = ErrorResponse {
            success: false,
            error: format!(
                "Usage: {} <image_path> (expected exactly 1 argument, got {})",
                collected.first().map(String::as_str).unwrap_or("wasm_img-metadata"),
                collected.len().saturating_sub(1)
            ),
        };
        let json = serde_json::to_string_pretty(&err)
            .unwrap_or_else(|_| "{\"success\":false,\"error\":\"Serialization failure\"}".to_string());
        return (json, 1);
    }

    let image_path = &collected[1];
    match extract_metadata_from_path(image_path) {
        Ok(resp) => {
            let json = serde_json::to_string_pretty(&resp)
                .unwrap_or_else(|_| "{\"success\":false,\"error\":\"Serialization failure\"}".to_string());
            (json, 0)
        }
        Err(err) => {
            let json = serde_json::to_string_pretty(&err)
                .unwrap_or_else(|_| "{\"success\":false,\"error\":\"Serialization failure\"}".to_string());
            (json, 1)
        }
    }
}

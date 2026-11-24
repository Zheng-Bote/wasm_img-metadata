use std::io::Read;

use flate2::read::ZlibDecoder;
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Jpeg,
    Tiff,
    Png,
    WebP,
}

#[derive(Debug, Default, Clone)]
pub struct ExtractedPayloads {
    pub format: Option<ImageFormat>,
    pub exif_chunks: Vec<Vec<u8>>,
    pub iptc_chunks: Vec<Vec<u8>>,
    pub xmp_chunks: Vec<String>,
    /// Container-native metadata entries (e.g. PNG tEXt/zTXt/iTXt/tIME/pHYs or JPEG COM)
    /// routed to the appropriate section so no metadata is ever silently discarded.
    pub extra_exif: Map<String, Value>,
    pub extra_iptc: Map<String, Value>,
    pub extra_xmp: Map<String, Value>,
}

const MAX_DECOMPRESSED_SIZE: u64 = 16 * 1024 * 1024; // 16 MiB cap against decompression bombs

pub fn demux_container(data: &[u8]) -> Result<ExtractedPayloads, String> {
    if data.len() < 4 {
        return Err("File is too small to be a valid image container".to_string());
    }

    if data.starts_with(&[0xFF, 0xD8]) {
        demux_jpeg(data)
    } else if data.starts_with(b"II*\0") || data.starts_with(b"MM\0*") {
        demux_tiff(data)
    } else if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        demux_png(data)
    } else if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        demux_webp(data)
    } else {
        Err("Unsupported or unrecognized image format (expected JPEG, TIFF, PNG, or WebP)".to_string())
    }
}

fn demux_jpeg(data: &[u8]) -> Result<ExtractedPayloads, String> {
    let mut payloads = ExtractedPayloads {
        format: Some(ImageFormat::Jpeg),
        ..Default::default()
    };

    let mut pos = 2usize;
    let len = data.len();

    while pos + 1 < len {
        if data[pos] != 0xFF {
            // Find next marker if padding/garbage precedes it
            if let Some(next_ff) = data[pos..].iter().position(|&b| b == 0xFF) {
                pos += next_ff;
            } else {
                break;
            }
        }

        // Skip fill 0xFF bytes
        while pos < len && data[pos] == 0xFF {
            pos += 1;
        }
        if pos >= len {
            break;
        }

        let marker = data[pos];
        pos += 1;

        // Standalone markers without length
        if marker == 0xD8 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            continue;
        }
        // EOI or SOS (Start of Scan - image bitstream follows)
        if marker == 0xD9 || marker == 0xDA {
            break;
        }

        if pos + 2 > len {
            break;
        }
        let seg_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
        if seg_len < 2 || pos + seg_len > len {
            break;
        }

        let seg_data = &data[pos + 2..pos + seg_len];
        pos += seg_len;

        match marker {
            // APP1: EXIF or XMP
            0xE1 => {
                if seg_data.starts_with(b"Exif\0\0") {
                    payloads.exif_chunks.push(seg_data[6..].to_vec());
                } else if seg_data.starts_with(b"Exif\0") {
                    payloads.exif_chunks.push(seg_data[5..].to_vec());
                } else if seg_data.starts_with(b"http://ns.adobe.com/xap/1.0/\0") {
                    let xmp_bytes = &seg_data[29..];
                    if let Ok(s) = std::str::from_utf8(xmp_bytes) {
                        payloads.xmp_chunks.push(s.trim_matches('\0').to_string());
                    } else {
                        payloads
                            .xmp_chunks
                            .push(String::from_utf8_lossy(xmp_bytes).trim_matches('\0').to_string());
                    }
                } else if seg_data.starts_with(b"http://ns.adobe.com/xmp/extension/\0") {
                    // Extended XMP: header + 32-byte GUID + 4-byte full length + 4-byte offset
                    let header_end = 35 + 32 + 4 + 4;
                    if seg_data.len() > header_end {
                        let ext_bytes = &seg_data[header_end..];
                        payloads
                            .xmp_chunks
                            .push(String::from_utf8_lossy(ext_bytes).trim_matches('\0').to_string());
                    }
                }
            }
            // APP13: Photoshop IRB / IPTC
            0xED => {
                if seg_data.starts_with(b"Photoshop 3.0\0") {
                    extract_photoshop_irb(&seg_data[14..], &mut payloads);
                } else if seg_data.starts_with(b"Adobe_Photoshop2.5:") {
                    extract_photoshop_irb(&seg_data[19..], &mut payloads);
                } else if seg_data.first() == Some(&0x1C) {
                    payloads.iptc_chunks.push(seg_data.to_vec());
                }
            }
            // COM: JPEG Comment
            0xFE => {
                let comment = String::from_utf8_lossy(seg_data).trim_matches('\0').to_string();
                if !comment.is_empty() {
                    insert_or_append(&mut payloads.extra_exif, "Comment", Value::String(comment));
                }
            }
            _ => {}
        }
    }

    Ok(payloads)
}

pub fn extract_photoshop_irb(irb: &[u8], payloads: &mut ExtractedPayloads) {
    let mut pos = 0usize;
    let len = irb.len();

    while pos + 12 <= len {
        if &irb[pos..pos + 4] != b"8BIM" {
            break;
        }
        let resource_id = u16::from_be_bytes([irb[pos + 4], irb[pos + 5]]);
        let name_len = irb[pos + 6] as usize;
        // Pascal string padded to even number of bytes (including the length byte)
        let padded_name_len = if (name_len + 1) % 2 == 0 {
            name_len + 1
        } else {
            name_len + 2
        };

        let size_pos = pos + 6 + padded_name_len;
        if size_pos + 4 > len {
            break;
        }
        let data_size = u32::from_be_bytes([
            irb[size_pos],
            irb[size_pos + 1],
            irb[size_pos + 2],
            irb[size_pos + 3],
        ]) as usize;

        let data_start = size_pos + 4;
        if data_start + data_size > len {
            break;
        }
        let res_data = &irb[data_start..data_start + data_size];

        match resource_id {
            0x0404 => {
                // IPTC-NAA record
                payloads.iptc_chunks.push(res_data.to_vec());
            }
            0x0422 => {
                // Exif data inside 8BIM
                payloads.exif_chunks.push(res_data.to_vec());
            }
            0x0424 => {
                // XMP packet inside 8BIM
                payloads
                    .xmp_chunks
                    .push(String::from_utf8_lossy(res_data).trim_matches('\0').to_string());
            }
            _ => {}
        }

        let padded_data_size = if data_size % 2 == 0 {
            data_size
        } else {
            data_size + 1
        };
        pos = data_start + padded_data_size;
    }
}

fn demux_tiff(data: &[u8]) -> Result<ExtractedPayloads, String> {
    let mut payloads = ExtractedPayloads {
        format: Some(ImageFormat::Tiff),
        ..Default::default()
    };
    payloads.exif_chunks.push(data.to_vec());
    Ok(payloads)
}

fn demux_png(data: &[u8]) -> Result<ExtractedPayloads, String> {
    let mut payloads = ExtractedPayloads {
        format: Some(ImageFormat::Png),
        ..Default::default()
    };

    let mut pos = 8usize;
    let len = data.len();

    while pos + 12 <= len {
        let chunk_len = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let chunk_type = &data[pos + 4..pos + 8];
        let data_start = pos + 8;
        let data_end = match data_start.checked_add(chunk_len) {
            Some(end) if end + 4 <= len => end,
            _ => break,
        };
        let chunk_data = &data[data_start..data_end];
        pos = data_end + 4; // skip 4-byte CRC

        match chunk_type {
            b"IHDR" if chunk_data.len() >= 13 => {
                let width = u32::from_be_bytes([chunk_data[0], chunk_data[1], chunk_data[2], chunk_data[3]]);
                let height = u32::from_be_bytes([chunk_data[4], chunk_data[5], chunk_data[6], chunk_data[7]]);
                let bit_depth = chunk_data[8];
                let color_type = chunk_data[9];
                payloads.extra_exif.insert("ImageWidth".to_string(), Value::from(width));
                payloads.extra_exif.insert("ImageLength".to_string(), Value::from(height));
                payloads.extra_exif.insert("BitDepth".to_string(), Value::from(bit_depth));
                payloads.extra_exif.insert("ColorType".to_string(), Value::from(color_type));
            }
            b"eXIf" => {
                let exif_slice = if chunk_data.starts_with(b"Exif\0\0") {
                    &chunk_data[6..]
                } else {
                    chunk_data
                };
                payloads.exif_chunks.push(exif_slice.to_vec());
            }
            b"iTXt" => {
                parse_png_itxt(chunk_data, &mut payloads);
            }
            b"tEXt" => {
                parse_png_text(chunk_data, &mut payloads);
            }
            b"zTXt" => {
                parse_png_ztxt(chunk_data, &mut payloads);
            }
            b"tIME" if chunk_data.len() >= 7 => {
                let year = u16::from_be_bytes([chunk_data[0], chunk_data[1]]);
                let month = chunk_data[2];
                let day = chunk_data[3];
                let hour = chunk_data[4];
                let minute = chunk_data[5];
                let second = chunk_data[6];
                let iso = format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z");
                payloads.extra_exif.insert("ModifyDate".to_string(), Value::String(iso));
            }
            b"pHYs" if chunk_data.len() >= 9 => {
                let ppu_x = u32::from_be_bytes([chunk_data[0], chunk_data[1], chunk_data[2], chunk_data[3]]);
                let ppu_y = u32::from_be_bytes([chunk_data[4], chunk_data[5], chunk_data[6], chunk_data[7]]);
                let unit = chunk_data[8];
                payloads.extra_exif.insert("PixelsPerUnitX".to_string(), Value::from(ppu_x));
                payloads.extra_exif.insert("PixelsPerUnitY".to_string(), Value::from(ppu_y));
                payloads.extra_exif.insert("PixelUnits".to_string(), Value::from(unit));
            }
            b"IEND" => break,
            _ => {}
        }
    }

    Ok(payloads)
}

fn parse_png_itxt(chunk_data: &[u8], payloads: &mut ExtractedPayloads) {
    let Some(null_pos) = chunk_data.iter().position(|&b| b == 0) else {
        return;
    };
    let keyword = String::from_utf8_lossy(&chunk_data[..null_pos]).to_string();
    let rest = &chunk_data[null_pos + 1..];
    if rest.len() < 2 {
        return;
    }
    let compression_flag = rest[0];
    let _compression_method = rest[1];
    let rest = &rest[2..];

    // Language tag (null-terminated)
    let Some(lang_null) = rest.iter().position(|&b| b == 0) else {
        return;
    };
    let rest = &rest[lang_null + 1..];

    // Translated keyword (null-terminated)
    let Some(trans_null) = rest.iter().position(|&b| b == 0) else {
        return;
    };
    let text_bytes = &rest[trans_null + 1..];

    let decoded_text = if compression_flag == 1 {
        let Some(decompressed) = decompress_zlib(text_bytes) else {
            return;
        };
        String::from_utf8_lossy(&decompressed).to_string()
    } else {
        String::from_utf8_lossy(text_bytes).to_string()
    };

    route_png_text_entry(&keyword, &decoded_text, payloads);
}

fn parse_png_text(chunk_data: &[u8], payloads: &mut ExtractedPayloads) {
    let Some(null_pos) = chunk_data.iter().position(|&b| b == 0) else {
        return;
    };
    let keyword = String::from_utf8_lossy(&chunk_data[..null_pos]).to_string();
    let text_bytes = &chunk_data[null_pos + 1..];
    // Latin-1 / UTF-8 decode
    let text = match std::str::from_utf8(text_bytes) {
        Ok(s) => s.to_string(),
        Err(_) => text_bytes.iter().map(|&b| b as char).collect(),
    };
    route_png_text_entry(&keyword, &text, payloads);
}

fn parse_png_ztxt(chunk_data: &[u8], payloads: &mut ExtractedPayloads) {
    let Some(null_pos) = chunk_data.iter().position(|&b| b == 0) else {
        return;
    };
    let keyword = String::from_utf8_lossy(&chunk_data[..null_pos]).to_string();
    let rest = &chunk_data[null_pos + 1..];
    if rest.is_empty() {
        return;
    }
    let _compression_method = rest[0];
    let compressed = &rest[1..];
    if let Some(decompressed) = decompress_zlib(compressed) {
        let text = match std::str::from_utf8(&decompressed) {
            Ok(s) => s.to_string(),
            Err(_) => decompressed.iter().map(|&b| b as char).collect(),
        };
        route_png_text_entry(&keyword, &text, payloads);
    }
}

fn route_png_text_entry(keyword: &str, text: &str, payloads: &mut ExtractedPayloads) {
    let trimmed = text.trim_matches('\0');
    if keyword == "XML:com.adobe.xmp" || keyword.eq_ignore_ascii_case("xmp") || trimmed.contains("<x:xmpmeta") || trimmed.contains("<rdf:RDF") {
        payloads.xmp_chunks.push(trimmed.to_string());
        return;
    }

    if keyword.eq_ignore_ascii_case("Raw profile type exif") || keyword.eq_ignore_ascii_case("exif") {
        if let Some(bytes) = decode_imagemagick_raw_profile(trimmed) {
            let slice = if bytes.starts_with(b"Exif\0\0") {
                &bytes[6..]
            } else {
                &bytes[..]
            };
            payloads.exif_chunks.push(slice.to_vec());
            return;
        }
    }

    if keyword.eq_ignore_ascii_case("Raw profile type iptc")
        || keyword.eq_ignore_ascii_case("Raw profile type 8bim")
        || keyword.eq_ignore_ascii_case("iptc")
    {
        if let Some(bytes) = decode_imagemagick_raw_profile(trimmed) {
            if bytes.starts_with(b"8BIM") {
                extract_photoshop_irb(&bytes, payloads);
            } else {
                payloads.iptc_chunks.push(bytes);
            }
            return;
        }
    }

    if keyword.eq_ignore_ascii_case("Raw profile type xmp") {
        if let Some(bytes) = decode_imagemagick_raw_profile(trimmed) {
            payloads
                .xmp_chunks
                .push(String::from_utf8_lossy(&bytes).trim_matches('\0').to_string());
            return;
        }
    }

    // Preserve any other PNG text metadata key/value pairs without discarding them
    let normalized_val = crate::exif::normalize_date_or_value(trimmed);
    if keyword.contains(':') {
        insert_or_append(&mut payloads.extra_xmp, keyword, normalized_val);
    } else {
        insert_or_append(&mut payloads.extra_exif, keyword, normalized_val);
    }
}

fn decode_imagemagick_raw_profile(text: &str) -> Option<Vec<u8>> {
    // Format:
    // \n<name>\n<length>\n<hex_lines...>\n
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    let _profile_type = lines.next()?;
    let _len_str = lines.next()?;
    let hex_str: String = lines.collect();
    if hex_str.is_empty() || hex_str.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(hex_str.len() / 2);
    let bytes = hex_str.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        let hi = (bytes[i] as char).to_digit(16)?;
        let lo = (bytes[i + 1] as char).to_digit(16)?;
        out.push(((hi << 4) | lo) as u8);
        i += 2;
    }
    Some(out)
}

fn decompress_zlib(compressed: &[u8]) -> Option<Vec<u8>> {
    let decoder = ZlibDecoder::new(compressed);
    let mut limited = decoder.take(MAX_DECOMPRESSED_SIZE);
    let mut buf = Vec::new();
    limited.read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn demux_webp(data: &[u8]) -> Result<ExtractedPayloads, String> {
    let mut payloads = ExtractedPayloads {
        format: Some(ImageFormat::WebP),
        ..Default::default()
    };

    let mut pos = 12usize;
    let len = data.len();

    while pos + 8 <= len {
        let fourcc = &data[pos..pos + 4];
        let chunk_size = u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]]) as usize;
        let data_start = pos + 8;
        let data_end = match data_start.checked_add(chunk_size) {
            Some(end) if end <= len => end,
            _ => break,
        };
        let chunk_data = &data[data_start..data_end];

        match fourcc {
            b"EXIF" => {
                let exif_slice = if chunk_data.starts_with(b"Exif\0\0") {
                    &chunk_data[6..]
                } else {
                    chunk_data
                };
                payloads.exif_chunks.push(exif_slice.to_vec());
            }
            b"XMP " | b"XMP\0" => {
                payloads
                    .xmp_chunks
                    .push(String::from_utf8_lossy(chunk_data).trim_matches('\0').to_string());
            }
            b"IPTC" | b"PHOT" => {
                if chunk_data.starts_with(b"8BIM") {
                    extract_photoshop_irb(chunk_data, &mut payloads);
                } else {
                    payloads.iptc_chunks.push(chunk_data.to_vec());
                }
            }
            b"VP8X" if chunk_data.len() >= 10 => {
                let width = 1 + u32::from_le_bytes([chunk_data[4], chunk_data[5], chunk_data[6], 0]);
                let height = 1 + u32::from_le_bytes([chunk_data[7], chunk_data[8], chunk_data[9], 0]);
                payloads.extra_exif.insert("ImageWidth".to_string(), Value::from(width));
                payloads.extra_exif.insert("ImageLength".to_string(), Value::from(height));
            }
            _ => {}
        }

        let padded_size = if chunk_size % 2 == 0 {
            chunk_size
        } else {
            chunk_size + 1
        };
        pos = match data_start.checked_add(padded_size) {
            Some(next) => next,
            None => break,
        };
    }

    Ok(payloads)
}

pub fn insert_or_append(map: &mut Map<String, Value>, key: &str, new_val: Value) {
    match map.get_mut(key) {
        Some(Value::Array(arr)) => {
            if let Value::Array(mut new_items) = new_val {
                arr.append(&mut new_items);
            } else {
                arr.push(new_val);
            }
        }
        Some(existing) => {
            let prev = existing.clone();
            if prev != new_val {
                *existing = Value::Array(vec![prev, new_val]);
            }
        }
        None => {
            map.insert(key.to_string(), new_val);
        }
    }
}

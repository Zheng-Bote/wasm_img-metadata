use serde_json::{Map, Value};

pub fn parse_iptc_iim(data: &[u8], out: &mut Map<String, Value>) {
    let mut pos = 0usize;
    let len = data.len();

    while pos + 5 <= len {
        // IPTC IIM dataset tag marker is 0x1C
        if data[pos] != 0x1C {
            if let Some(next) = data[pos..].iter().position(|&b| b == 0x1C) {
                pos += next;
                if pos + 5 > len {
                    break;
                }
            } else {
                break;
            }
        }

        let record = data[pos + 1];
        let dataset = data[pos + 2];
        let raw_len = u16::from_be_bytes([data[pos + 3], data[pos + 4]]);
        pos += 5;

        let data_len: usize = if (raw_len & 0x8000) != 0 {
            // Extended dataset length
            let len_bytes_count = (raw_len & 0x7FFF) as usize;
            if len_bytes_count == 0 || len_bytes_count > 4 || pos + len_bytes_count > len {
                break;
            }
            let mut acc = 0usize;
            for &b in &data[pos..pos + len_bytes_count] {
                acc = (acc << 8) | (b as usize);
            }
            pos += len_bytes_count;
            acc
        } else {
            raw_len as usize
        };

        if pos + data_len > len {
            break;
        }

        let payload = &data[pos..pos + data_len];
        pos += data_len;

        let (field_name, is_repeatable) = lookup_iptc_dataset(record, dataset);
        let val = decode_iptc_value(record, dataset, payload);

        if is_repeatable {
            match out.get_mut(&field_name) {
                Some(Value::Array(arr)) => arr.push(val),
                Some(existing) => {
                    let prev = existing.clone();
                    *existing = Value::Array(vec![prev, val]);
                }
                None => {
                    out.insert(field_name, Value::Array(vec![val]));
                }
            }
        } else {
            match out.get_mut(&field_name) {
                Some(Value::Array(arr)) => arr.push(val),
                Some(existing) => {
                    let prev = existing.clone();
                    *existing = Value::Array(vec![prev, val]);
                }
                None => {
                    out.insert(field_name, val);
                }
            }
        }
    }
}

fn decode_iptc_value(record: u8, dataset: u8, payload: &[u8]) -> Value {
    // Numeric binary datasets in Record 1 or Record 2
    if (record == 1 && (dataset == 0 || dataset == 20 || dataset == 30))
        || (record == 2 && (dataset == 0 || dataset == 70))
    {
        if payload.len() == 2 {
            return Value::from(u16::from_be_bytes([payload[0], payload[1]]));
        } else if payload.len() == 1 {
            return Value::from(payload[0]);
        }
    }

    let text = match std::str::from_utf8(payload) {
        Ok(s) => s.trim_matches('\0').to_string(),
        Err(_) => payload.iter().map(|&b| b as char).collect::<String>(),
    };

    // Normalize IPTC dates (YYYYMMDD) to ISO-8601 (YYYY-MM-DD)
    if (record == 2 && matches!(dataset, 30 | 37 | 47 | 55 | 62))
        || (record == 1 && dataset == 70)
    {
        if let Some(iso) = try_iptc_date_to_iso8601(&text) {
            return Value::String(iso);
        }
    }

    Value::String(text)
}

fn try_iptc_date_to_iso8601(s: &str) -> Option<String> {
    let trimmed = s.trim();
    let b = trimmed.as_bytes();
    if b.len() == 8 && b.iter().all(|c| c.is_ascii_digit()) {
        return Some(format!(
            "{}-{}-{}",
            &trimmed[0..4],
            &trimmed[4..6],
            &trimmed[6..8]
        ));
    }
    crate::exif::try_exif_date_to_iso8601(trimmed)
}

fn lookup_iptc_dataset(record: u8, dataset: u8) -> (String, bool) {
    match (record, dataset) {
        (1, 0) => ("ModelVersion".to_string(), false),
        (1, 5) => ("Destination".to_string(), true),
        (1, 20) => ("FileFormat".to_string(), false),
        (1, 22) => ("FileVersion".to_string(), false),
        (1, 30) => ("ServiceIdentifier".to_string(), false),
        (1, 40) => ("EnvelopeNumber".to_string(), false),
        (1, 50) => ("ProductID".to_string(), true),
        (1, 60) => ("EnvelopePriority".to_string(), false),
        (1, 70) => ("DateSent".to_string(), false),
        (1, 80) => ("TimeSent".to_string(), false),
        (1, 90) => ("CodedCharacterSet".to_string(), false),
        (2, 0) => ("RecordVersion".to_string(), false),
        (2, 3) => ("ObjectTypeReference".to_string(), false),
        (2, 4) => ("ObjectAttributeReference".to_string(), true),
        (2, 5) => ("ObjectName".to_string(), false),
        (2, 7) => ("EditStatus".to_string(), false),
        (2, 8) => ("EditorialUpdate".to_string(), false),
        (2, 10) => ("Urgency".to_string(), false),
        (2, 12) => ("SubjectReference".to_string(), true),
        (2, 15) => ("Category".to_string(), false),
        (2, 20) => ("SupplementalCategories".to_string(), true),
        (2, 22) => ("FixtureIdentifier".to_string(), false),
        (2, 25) => ("Keywords".to_string(), true),
        (2, 26) => ("ContentLocationCode".to_string(), true),
        (2, 27) => ("ContentLocationName".to_string(), true),
        (2, 30) => ("ReleaseDate".to_string(), false),
        (2, 35) => ("ReleaseTime".to_string(), false),
        (2, 37) => ("ExpirationDate".to_string(), false),
        (2, 38) => ("ExpirationTime".to_string(), false),
        (2, 40) => ("SpecialInstructions".to_string(), false),
        (2, 42) => ("ActionAdvised".to_string(), false),
        (2, 45) => ("ReferenceService".to_string(), true),
        (2, 47) => ("ReferenceDate".to_string(), true),
        (2, 50) => ("ReferenceNumber".to_string(), true),
        (2, 55) => ("DateCreated".to_string(), false),
        (2, 60) => ("TimeCreated".to_string(), false),
        (2, 62) => ("DigitalCreationDate".to_string(), false),
        (2, 63) => ("DigitalCreationTime".to_string(), false),
        (2, 65) => ("OriginatingProgram".to_string(), false),
        (2, 70) => ("ProgramVersion".to_string(), false),
        (2, 75) => ("ObjectCycle".to_string(), false),
        (2, 80) => ("By-line".to_string(), true),
        (2, 85) => ("By-lineTitle".to_string(), true),
        (2, 90) => ("City".to_string(), false),
        (2, 92) => ("Sub-location".to_string(), false),
        (2, 95) => ("Province-State".to_string(), false),
        (2, 100) => ("Country-PrimaryLocationCode".to_string(), false),
        (2, 101) => ("Country-PrimaryLocationName".to_string(), false),
        (2, 103) => ("OriginalTransmissionReference".to_string(), false),
        (2, 105) => ("Headline".to_string(), false),
        (2, 110) => ("Credit".to_string(), false),
        (2, 115) => ("Source".to_string(), false),
        (2, 116) => ("CopyrightNotice".to_string(), false),
        (2, 118) => ("Contact".to_string(), true),
        (2, 120) => ("Caption-Abstract".to_string(), false),
        (2, 121) => ("LocalCaption".to_string(), false),
        (2, 122) => ("Writer-Editor".to_string(), true),
        (2, 130) => ("ImageType".to_string(), false),
        (2, 131) => ("ImageOrientation".to_string(), false),
        (2, 135) => ("LanguageIdentifier".to_string(), false),
        _ => (format!("IPTC_{record}_{dataset}"), false),
    }
}

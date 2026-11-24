use serde_json::{Map, Number, Value};

#[derive(Debug, Clone)]
struct XmlNode {
    name: String,
    attributes: Vec<(String, String)>,
    children: Vec<XmlNode>,
    text: String,
}

const STRUCTURAL_WRAPPERS: &[&str] = &[
    "x:xmpmeta",
    "xmpmeta",
    "rdf:RDF",
    "RDF",
    "rdf:Description",
    "Description",
];

pub fn parse_xmp_packet(xml: &str, out: &mut Map<String, Value>) {
    let nodes = parse_xml_forest(xml);
    for node in &nodes {
        extract_xmp_from_node(node, None, out);
    }
}

fn extract_xmp_from_node(node: &XmlNode, parent_prop: Option<&str>, out: &mut Map<String, Value>) {
    let tag = node.name.as_str();

    if STRUCTURAL_WRAPPERS.contains(&tag) {
        // Attributes on rdf:Description (or wrappers) may carry XMP properties directly
        for (attr_name, attr_val) in &node.attributes {
            if is_xmp_property_attribute(attr_name) {
                let key = match parent_prop {
                    Some(prefix) => format!("{prefix}/{attr_name}"),
                    None => attr_name.clone(),
                };
                crate::container::insert_or_append(out, &key, coerce_xmp_scalar(attr_val));
            }
        }
        for child in &node.children {
            extract_xmp_from_node(child, parent_prop, out);
        }
        return;
    }

    // Ignore processing instructions or structural container tags if encountered directly
    if tag == "rdf:Bag" || tag == "rdf:Seq" || tag == "rdf:Alt" || tag == "Bag" || tag == "Seq" || tag == "Alt" {
        if let Some(prop_key) = parent_prop {
            let val = decode_rdf_collection(node);
            out.insert(prop_key.to_string(), val);
        }
        return;
    }

    let prop_key = match parent_prop {
        Some(prefix) => format!("{prefix}/{tag}"),
        None => tag.to_string(),
    };

    // Check if this property contains an rdf:Bag, rdf:Seq, or rdf:Alt child
    if let Some(coll_child) = node.children.iter().find(|c| {
        matches!(
            c.name.as_str(),
            "rdf:Bag" | "rdf:Seq" | "rdf:Alt" | "Bag" | "Seq" | "Alt"
        )
    }) {
        let val = decode_rdf_collection(coll_child);
        out.insert(prop_key, val);
        return;
    }

    // Check if this property contains a nested rdf:Description or structured sub-fields
    let prop_attrs: Vec<&(String, String)> = node
        .attributes
        .iter()
        .filter(|(k, _)| is_xmp_property_attribute(k))
        .collect();

    if !node.children.is_empty() {
        for (k, v) in prop_attrs {
            let attr_key = format!("{prop_key}/{k}");
            crate::container::insert_or_append(out, &attr_key, coerce_xmp_scalar(v));
        }
        for child in &node.children {
            extract_xmp_from_node(child, Some(&prop_key), out);
        }
        return;
    }

    // If element has rdf:resource or property attributes and empty text
    let trimmed_text = node.text.trim();
    if trimmed_text.is_empty() && !prop_attrs.is_empty() {
        if prop_attrs.len() == 1 && (prop_attrs[0].0 == "rdf:resource" || prop_attrs[0].0 == "rdf:value") {
            crate::container::insert_or_append(out, &prop_key, coerce_xmp_scalar(&prop_attrs[0].1));
        } else {
            for (k, v) in prop_attrs {
                let attr_key = format!("{prop_key}/{k}");
                crate::container::insert_or_append(out, &attr_key, coerce_xmp_scalar(v));
            }
        }
        return;
    }

    let val = coerce_xmp_scalar(trimmed_text);
    crate::container::insert_or_append(out, &prop_key, val);
}

fn decode_rdf_collection(coll_node: &XmlNode) -> Value {
    let is_alt = matches!(coll_node.name.as_str(), "rdf:Alt" | "Alt");
    let li_nodes: Vec<&XmlNode> = coll_node
        .children
        .iter()
        .filter(|c| c.name == "rdf:li" || c.name == "li")
        .collect();

    if is_alt {
        // rdf:Alt often represents a localized string (e.g. dc:title with xml:lang="x-default")
        // If there is a single item, emit its scalar value directly (so `"dc:title": "..."` works out of the box);
        // if multiple language alternatives exist, emit an array of all items so nothing is lost.
        if li_nodes.len() == 1 {
            return decode_li_node(li_nodes[0]);
        }
    }

    let items: Vec<Value> = li_nodes.into_iter().map(decode_li_node).collect();
    Value::Array(items)
}

fn decode_li_node(li: &XmlNode) -> Value {
    if !li.children.is_empty() {
        let mut sub_map = Map::new();
        for child in &li.children {
            extract_xmp_from_node(child, None, &mut sub_map);
        }
        for (k, v) in &li.attributes {
            if is_xmp_property_attribute(k) {
                sub_map.insert(k.clone(), coerce_xmp_scalar(v));
            }
        }
        return Value::Object(sub_map);
    }

    let prop_attrs: Vec<&(String, String)> = li
        .attributes
        .iter()
        .filter(|(k, _)| is_xmp_property_attribute(k))
        .collect();

    let trimmed = li.text.trim();
    if trimmed.is_empty() && !prop_attrs.is_empty() {
        let mut sub_map = Map::new();
        for (k, v) in prop_attrs {
            sub_map.insert(k.clone(), coerce_xmp_scalar(v));
        }
        return Value::Object(sub_map);
    }

    coerce_xmp_scalar(trimmed)
}

fn is_xmp_property_attribute(attr_name: &str) -> bool {
    if attr_name == "xmlns" || attr_name.starts_with("xmlns:") || attr_name.starts_with("xml:") {
        return false;
    }
    if matches!(
        attr_name,
        "rdf:about"
            | "about"
            | "rdf:parseType"
            | "parseType"
            | "rdf:nodeID"
            | "rdf:ID"
            | "rdf:datatype"
            | "x:xmptk"
            | "xmptk"
    ) {
        return false;
    }
    true
}

pub fn coerce_xmp_scalar(raw: &str) -> Value {
    let s = unescape_xml(raw.trim());
    if s.eq_ignore_ascii_case("true") {
        return Value::Bool(true);
    }
    if s.eq_ignore_ascii_case("false") {
        return Value::Bool(false);
    }

    // Preserve leading-zero strings (like serial numbers "00123") as strings
    let has_leading_zero = s.len() > 1
        && (s.starts_with('0') || s.starts_with("-0"))
        && !s.starts_with("0.")
        && !s.starts_with("-0.");

    if !has_leading_zero {
        if let Ok(i) = s.parse::<i64>() {
            return Value::from(i);
        }
        if let Ok(u) = s.parse::<u64>() {
            return Value::from(u);
        }
        if s.contains('.') {
            if let Ok(f) = s.parse::<f64>() {
                if let Some(n) = Number::from_f64(f) {
                    return Value::Number(n);
                }
            }
        }
    }

    // Check EXIF rational strings like "1/125" or "28/10" only if not a date
    if let Some(iso) = crate::exif::try_exif_date_to_iso8601(&s) {
        return Value::String(iso);
    }

    Value::String(s)
}

fn unescape_xml(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn parse_xml_forest(input: &str) -> Vec<XmlNode> {
    let mut pos = 0usize;
    let bytes = input.as_bytes();
    parse_xml_children(input, bytes, &mut pos, None)
}

fn parse_xml_children(
    input: &str,
    bytes: &[u8],
    pos: &mut usize,
    closing_tag: Option<&str>,
) -> Vec<XmlNode> {
    let mut nodes = Vec::new();
    let len = bytes.len();

    while *pos < len {
        let Some(lt_rel) = bytes[*pos..].iter().position(|&b| b == b'<') else {
            break;
        };
        *pos += lt_rel;

        if *pos + 1 >= len {
            break;
        }

        // Comment <!-- ... -->
        if bytes[*pos..].starts_with(b"<!--") {
            if let Some(end) = find_subslice(&bytes[*pos + 4..], b"-->") {
                *pos += 4 + end + 3;
                continue;
            } else {
                break;
            }
        }

        // CDATA <![CDATA[ ... ]]>
        if bytes[*pos..].starts_with(b"<![CDATA[") {
            if let Some(end) = find_subslice(&bytes[*pos + 9..], b"]]>") {
                *pos += 9 + end + 3;
                continue;
            } else {
                break;
            }
        }

        // Processing instruction <? ... ?> or declaration <! ... >
        if bytes[*pos + 1] == b'?' {
            if let Some(end) = find_subslice(&bytes[*pos + 2..], b"?>") {
                *pos += 2 + end + 2;
                continue;
            } else {
                break;
            }
        }
        if bytes[*pos + 1] == b'!' {
            if let Some(end) = bytes[*pos + 2..].iter().position(|&b| b == b'>') {
                *pos += 2 + end + 1;
                continue;
            } else {
                break;
            }
        }

        // Closing tag </name>
        if bytes[*pos + 1] == b'/' {
            let Some(gt_rel) = bytes[*pos + 2..].iter().position(|&b| b == b'>') else {
                break;
            };
            let close_name = input[*pos + 2..*pos + 2 + gt_rel].trim();
            *pos += 2 + gt_rel + 1;
            if let Some(expected) = closing_tag {
                if close_name == expected {
                    return nodes;
                }
            }
            return nodes;
        }

        // Opening tag <name attr="val"...> or <name ... />
        let Some((tag_name, attributes, self_closing, next_pos)) =
            parse_opening_tag(input, bytes, *pos)
        else {
            *pos += 1;
            continue;
        };
        *pos = next_pos;

        if self_closing {
            nodes.push(XmlNode {
                name: tag_name,
                attributes,
                children: Vec::new(),
                text: String::new(),
            });
            continue;
        }

        let content_start = *pos;
        let mut text_acc = String::new();
        let mut children = Vec::new();

        // Parse mixed text/child nodes until matching </tag_name>
        loop {
            if *pos >= len {
                break;
            }
            let Some(next_lt_rel) = bytes[*pos..].iter().position(|&b| b == b'<') else {
                text_acc.push_str(&input[*pos..]);
                *pos = len;
                break;
            };
            if next_lt_rel > 0 {
                text_acc.push_str(&input[*pos..*pos + next_lt_rel]);
                *pos += next_lt_rel;
            }
            if *pos + 1 >= len {
                break;
            }
            if bytes[*pos..].starts_with(b"<![CDATA[") {
                if let Some(end) = find_subslice(&bytes[*pos + 9..], b"]]>") {
                    text_acc.push_str(&input[*pos + 9..*pos + 9 + end]);
                    *pos += 9 + end + 3;
                    continue;
                } else {
                    break;
                }
            }
            if bytes[*pos..].starts_with(b"<!--") {
                if let Some(end) = find_subslice(&bytes[*pos + 4..], b"-->") {
                    *pos += 4 + end + 3;
                    continue;
                } else {
                    break;
                }
            }
            if bytes[*pos + 1] == b'/' {
                let Some(gt_rel) = bytes[*pos + 2..].iter().position(|&b| b == b'>') else {
                    *pos = len;
                    break;
                };
                let _close_name = input[*pos + 2..*pos + 2 + gt_rel].trim();
                *pos += 2 + gt_rel + 1;
                break;
            }

            // Recurse for child element
            let child_list = parse_single_element(input, bytes, pos);
            if let Some(child) = child_list {
                children.push(child);
            } else if *pos == content_start {
                *pos += 1;
            }
        }

        nodes.push(XmlNode {
            name: tag_name,
            attributes,
            children,
            text: text_acc,
        });
    }

    nodes
}

fn parse_single_element(input: &str, bytes: &[u8], pos: &mut usize) -> Option<XmlNode> {
    let len = bytes.len();
    if *pos >= len || bytes[*pos] != b'<' {
        return None;
    }
    let (tag_name, attributes, self_closing, next_pos) = parse_opening_tag(input, bytes, *pos)?;
    *pos = next_pos;
    if self_closing {
        return Some(XmlNode {
            name: tag_name,
            attributes,
            children: Vec::new(),
            text: String::new(),
        });
    }

    let mut text_acc = String::new();
    let mut children = Vec::new();

    while *pos < len {
        let Some(next_lt_rel) = bytes[*pos..].iter().position(|&b| b == b'<') else {
            text_acc.push_str(&input[*pos..]);
            *pos = len;
            break;
        };
        if next_lt_rel > 0 {
            text_acc.push_str(&input[*pos..*pos + next_lt_rel]);
            *pos += next_lt_rel;
        }
        if *pos + 1 >= len {
            break;
        }
        if bytes[*pos..].starts_with(b"<![CDATA[") {
            if let Some(end) = find_subslice(&bytes[*pos + 9..], b"]]>") {
                text_acc.push_str(&input[*pos + 9..*pos + 9 + end]);
                *pos += 9 + end + 3;
                continue;
            } else {
                break;
            }
        }
        if bytes[*pos..].starts_with(b"<!--") {
            if let Some(end) = find_subslice(&bytes[*pos + 4..], b"-->") {
                *pos += 4 + end + 3;
                continue;
            } else {
                break;
            }
        }
        if bytes[*pos + 1] == b'/' {
            let Some(gt_rel) = bytes[*pos + 2..].iter().position(|&b| b == b'>') else {
                *pos = len;
                break;
            };
            *pos += 2 + gt_rel + 1;
            break;
        }
        if let Some(child) = parse_single_element(input, bytes, pos) {
            children.push(child);
        } else {
            *pos += 1;
        }
    }

    Some(XmlNode {
        name: tag_name,
        attributes,
        children,
        text: text_acc,
    })
}

fn parse_opening_tag(
    input: &str,
    bytes: &[u8],
    start: usize,
) -> Option<(String, Vec<(String, String)>, bool, usize)> {
    let len = bytes.len();
    let mut i = start + 1;
    while i < len && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let name_start = i;
    while i < len && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' && bytes[i] != b'/' {
        i += 1;
    }
    if i == name_start {
        return None;
    }
    let tag_name = input[name_start..i].to_string();
    let mut attrs = Vec::new();
    let mut self_closing = false;

    while i < len {
        while i < len && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= len {
            return None;
        }
        if bytes[i] == b'>' {
            i += 1;
            break;
        }
        if bytes[i] == b'/' && i + 1 < len && bytes[i + 1] == b'>' {
            self_closing = true;
            i += 2;
            break;
        }

        let attr_start = i;
        while i < len
            && !bytes[i].is_ascii_whitespace()
            && bytes[i] != b'='
            && bytes[i] != b'>'
            && bytes[i] != b'/'
        {
            i += 1;
        }
        if i == attr_start {
            i += 1;
            continue;
        }
        let attr_name = input[attr_start..i].to_string();
        while i < len && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < len && bytes[i] == b'=' {
            i += 1;
            while i < len && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < len && (bytes[i] == b'"' || bytes[i] == b'\'') {
                let quote = bytes[i];
                i += 1;
                let val_start = i;
                while i < len && bytes[i] != quote {
                    i += 1;
                }
                let val = unescape_xml(&input[val_start..i]);
                if i < len {
                    i += 1;
                }
                attrs.push((attr_name, val));
            }
        }
    }

    Some((tag_name, attrs, self_closing, i))
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

use std::io::{Read, Write};

use wasm_img_metadata::provider::http::handle_http_request;

fn main() {
    let method = std::env::var("REQUEST_METHOD").unwrap_or_else(|_| "POST".into());
    let uri = std::env::var("PATH_INFO").unwrap_or_else(|_| "/v1/metadata/extract".into());
    let content_type = std::env::var("CONTENT_TYPE")
        .or_else(|_| std::env::var("HTTP_CONTENT_TYPE"))
        .unwrap_or_default();
    let file_name = std::env::var("HTTP_X_FILE_NAME").unwrap_or_default();

    let mut body = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut body);

    let mut headers = Vec::new();
    if !content_type.is_empty() {
        headers.push(("Content-Type".to_string(), content_type));
    }
    if !file_name.is_empty() {
        headers.push(("X-File-Name".to_string(), file_name));
    }

    let response = handle_http_request(&method, &uri, &headers, &body);

    print!("Status: {}\r\n", response.status);
    for (k, v) in &response.headers {
        print!("{k}: {v}\r\n");
    }
    print!("\r\n");
    print!("{}", response.body);
    let _ = std::io::stdout().flush();
}

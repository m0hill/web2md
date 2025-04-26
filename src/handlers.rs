use worker::*;
use crate::config::ConvertRequest;
use crate::fetch::fetch_url_with_timeout;
use crate::markdown::html_to_markdown;
use crate::utils::add_cors_headers;

pub async fn handle_conversion_request(mut req: Request) -> worker::Result<Response> {
    let request: ConvertRequest = match req.json().await {
        Ok(req_data) => req_data,
        Err(e) => {
            console_error!("JSON parsing error: {:?}", e);
            let resp = Response::error(format!("Invalid request format: {}", e), 400)?;
            return add_cors_headers(resp);
        }
    };
    handle_conversion(request).await
}

pub async fn handle_conversion(request: ConvertRequest) -> worker::Result<Response> {
    let url_for_logging = request.url.clone();
    console_log!("Processing URL: {}", url_for_logging);

    match fetch_and_convert(request).await {
        Ok(markdown) => {
             let headers = Headers::from_iter([
                ("Cache-Control", "no-store"),
                ("Content-Type", "text/markdown; charset=utf-8"),
            ]);
            let resp = Response::ok(markdown)?.with_headers(headers);
            add_cors_headers(resp)
        },
        Err(e) => {
            console_error!("Error during conversion for {}: {}", url_for_logging, e);
             let status = if e.to_string().contains("HTTP error 404") { 404 }
                         else if e.to_string().contains("HTTP error 403") || e.to_string().contains("access denied") { 403 }
                         else if e.to_string().contains("HTTP error 503") || e.to_string().contains("Service unavailable") { 503 }
                         else { 500 };
             let error_message = format!("Failed to fetch or convert URL '{}': {}", url_for_logging, e);
             let resp = Response::error(error_message, status)?;
             add_cors_headers(resp)
        }
    }
}

async fn fetch_and_convert(req: ConvertRequest) -> worker::Result<String> {
    let html = fetch_url_with_timeout(&req.url, 10000).await?;
    Ok(html_to_markdown(&html, req.config).markdown)
}
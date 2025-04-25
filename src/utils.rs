// Removed unused Headers import
use worker::{Response, Result};

pub fn add_cors_headers(mut resp: Response) -> Result<Response> {
    let headers = resp.headers_mut(); // headers_mut() returns &mut Headers
    headers.set("Access-Control-Allow-Origin", "*")?;
    headers.set("Access-Control-Allow-Methods", "GET, POST, OPTIONS")?;
    headers.set("Access-Control-Allow-Headers", "Content-Type")?;
    Ok(resp)
}

pub fn handle_options_request() -> Result<Response> {
    let mut resp = Response::ok("")?;
    let headers = resp.headers_mut();
    headers.set("Access-Control-Allow-Origin", "*")?;
    headers.set("Access-Control-Allow-Methods", "GET, POST, OPTIONS")?;
    headers.set("Access-Control-Allow-Headers", "Content-Type")?;
    headers.set("Access-Control-Max-Age", "86400")?; // Cache preflight response for 1 day
    Ok(resp)
}
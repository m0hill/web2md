#![recursion_limit = "512"]

mod config;
mod crawl;
mod fetch;
mod fingerprint;
mod handlers;
mod markdown;
mod metadata;
mod utils; // Keep module import

use worker::*;
use worker_macros::event;
use console_error_panic_hook;

// Use module paths directly or import specific items if preferred style
use crate::config::CrawlRequest; // ConvertRequest removed
use crate::crawl::handle_crawl;
use crate::handlers::{handle_conversion_request, handle_get_conversion_request};
// Removed add_cors_headers, handle_options_request from specific use line


#[event(fetch)]
pub async fn main(mut req: Request, _env: Env, _ctx: Context) -> Result<Response> {
    // It's crucial to set the panic hook, otherwise panics might silently fail
    console_error_panic_hook::set_once();

    let url = req.url()?;
    let path = url.path();

    // Handle OPTIONS request for CORS preflight
    if req.method() == Method::Options {
        // Use qualified path
        return utils::handle_options_request();
    }

    // Handle favicon requests early
    if path == "/favicon.ico" {
        let mut resp = Response::empty()?.with_status(204); // No Content
        let headers = resp.headers_mut();
        headers.set("Cache-Control", "public, max-age=604800")?; // Cache for a week
         // Use qualified path
        return utils::add_cors_headers(resp); // Apply CORS headers
    }

    // Route requests based on method and path
    match (req.method(), path) {
        (Method::Post, "/crawl") => {
            match req.json::<CrawlRequest>().await {
                Ok(crawl_req) => {
                    match handle_crawl(crawl_req).await {
                        Ok(results) => {
                            let resp = Response::from_json(&results)?;
                            utils::add_cors_headers(resp)
                        }
                        Err(e) => {
                            console_error!("Crawl handler error: {}", e);
                            let resp = Response::error(format!("Crawl failed: {}", e), 500)?;
                            utils::add_cors_headers(resp)
                        }
                    }
                },
                Err(e) => {
                    console_error!("Crawl request parsing error: {}", e);
                    let resp = Response::error(format!("Invalid crawl request: {}", e), 400)?;
                    utils::add_cors_headers(resp)
                }
            }
        },
        (Method::Get, "/") => {
             handle_get_conversion_request(url).await // Already adds CORS via utils::
        },
        (Method::Post, "/") => {
            handle_conversion_request(req).await // Already adds CORS via utils::
        },
        _ => {
            let resp = Response::error("Not Found", 404)?;
            utils::add_cors_headers(resp) // Add CORS to 404 response
        }
    }
}
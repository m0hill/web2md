#![recursion_limit = "512"]

mod config;
mod crawl;
mod fetch;
mod fingerprint;
mod handlers;
mod markdown;
mod metadata;
mod utils;

use worker::*;
use worker_macros::event;
use console_error_panic_hook;

use crate::config::CrawlRequest;
use crate::crawl::handle_crawl;
use crate::handlers::{handle_conversion_request, handle_get_conversion_request};


#[event(fetch)]
pub async fn main(mut req: Request, _env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    let url = req.url()?;
    let path = url.path();

    if req.method() == Method::Options {
        return utils::handle_options_request();
    }

    if path == "/favicon.ico" {
        let mut resp = Response::empty()?.with_status(204);
        let headers = resp.headers_mut();
        headers.set("Cache-Control", "public, max-age=604800")?;
        return utils::add_cors_headers(resp);
    }

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
             handle_get_conversion_request(url).await
        },
        (Method::Post, "/") => {
            handle_conversion_request(req).await
        },
        _ => {
            let resp = Response::error("Not Found", 404)?;
            utils::add_cors_headers(resp)
        }
    }
}
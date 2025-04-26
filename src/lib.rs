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
use console_error_panic_hook;

use crate::config::CrawlRequest;
use crate::crawl::handle_crawl;
use crate::handlers::{handle_conversion_request, handle_conversion};

#[worker::event(fetch)]
pub async fn main(mut req: Request, _env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    let url = req.url()?;
    let path = url.path();

    if req.method() == Method::Options {
        return utils::handle_options_request();
    }

    if path == "/favicon.ico" {
        let resp = Response::empty()?.with_status(204);
        return utils::add_cors_headers(resp);
    }

    match (req.method(), path) {
        (Method::Post, "/crawl") => {
            match req.json::<CrawlRequest>().await {
                Ok(crawl_req) => {
                    match handle_crawl(crawl_req).await {
                        Ok(results) => {
                            if results.is_empty() {
                                let mut resp = Response::ok("Crawl completed, but no results were generated.")?;
                                resp.headers_mut().set("Content-Type", "text/plain; charset=utf-8")?;
                                resp.headers_mut().set("Cache-Control", "no-cache")?;
                                utils::add_cors_headers(resp)
                            } else {
                                let separator = "\n\n---\n\n";
                                let combined_markdown = results.join(separator);
                                let mut resp = Response::ok(combined_markdown)?;
                                resp.headers_mut().set("Content-Type", "text/markdown; charset=utf-8")?;
                                resp.headers_mut().set("Cache-Control", "no-cache")?;
                                utils::add_cors_headers(resp)
                            }
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

        (Method::Get, path) if path.starts_with("/http://") || path.starts_with("/https://") => {
            let target_url = match path.strip_prefix('/') {
                Some(url_str) => url_str.to_string(),
                None => path.to_string(),
            };

            console_log!("GET request for URL in path: {}", target_url);

            let request = config::ConvertRequest {
                url: target_url,
                config: config::ConvertConfig {
                    include_links: true,
                    clean_whitespace: true,
                    cleaning_rules: config::CleaningRules {
                        remove_scripts: true,
                        remove_styles: true,
                        remove_comments: true,
                        preserve_line_breaks: true,
                    },
                    preserve_headings: true,
                    include_metadata: true,
                    max_heading_level: 6,
                },
            };
            handle_conversion(request).await
        },

        (Method::Get, "/") => {
             let mut resp = Response::ok("Usage: \nGET /{URL} (e.g., /https://example.com)\nPOST / { \"url\": \"https://example.com\", \"config\": {...} }\nPOST /crawl { \"url\": \"...\", \"limit\": N, ... }")?;
             resp.headers_mut().set("Content-Type", "text/plain; charset=utf-8")?;
             resp.headers_mut().set("Cache-Control", "no-store")?;
             utils::add_cors_headers(resp)
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
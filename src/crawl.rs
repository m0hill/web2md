use worker::*;
use std::collections::{HashSet, VecDeque};
use url::Url;
use regex::Regex;
use lazy_static::lazy_static;

use crate::config::CrawlRequest;
use crate::fetch::fetch_url_with_timeout;
use crate::markdown::html_to_markdown;
use crate::config::HtmlConversionResult;

lazy_static! {
    static ref URL_REGEX: Regex = Regex::new(r"^https?://").unwrap();
}

pub async fn handle_crawl(request: CrawlRequest) -> worker::Result<Vec<String>> {
    let start_url = request.url;
    let limit = request.limit;
    let max_depth = request.max_depth;
    let config = request.config;
    let follow_relative = request.follow_relative;

    let mut results: Vec<String> = Vec::with_capacity(limit as usize);
    let mut queue: VecDeque<(String, u32)> = VecDeque::new();
    let mut visited: HashSet<String> = HashSet::new();

    let base_url_parsed = match Url::parse(&start_url) {
        Ok(url) => url,
        Err(_) => {
            return Err(Error::RustError(format!("Invalid starting URL: {}", start_url)));
        }
    };
    let base_domain = base_url_parsed.domain().map(|s| s.to_string());

    queue.push_back((start_url.clone(), 0));
    visited.insert(start_url);

    console_log!(
        "Starting sequential crawl. Limit: {}, Max Depth: {}",
        limit,
        max_depth
    );

    while let Some((current_url, current_depth)) = queue.pop_front() {
        if results.len() >= limit as usize {
            console_log!("Reached limit ({}), stopping crawl.", limit);
            break;
        }

        console_log!(
            "Processing: {} (Depth {}, Queue size: {}, Results: {})",
            current_url.chars().take(80).collect::<String>(),
            current_depth,
            queue.len(),
            results.len()
        );

        let html_content = match fetch_url_with_timeout(&current_url, 10000).await {
            Ok(html) => html,
            Err(e) => {
                console_error!(
                    "Error fetching {}: {}. Skipping.",
                    current_url.chars().take(80).collect::<String>(),
                    e
                );
                continue;
            }
        };

        let conversion_result: HtmlConversionResult = html_to_markdown(&html_content, config.clone());
        let markdown_content = conversion_result.markdown;
        let links = conversion_result.links;

        console_log!(
            "Converted {}. Markdown snippet: \"{}\"",
            current_url.chars().take(60).collect::<String>(),
            markdown_content.chars().take(100).collect::<String>().replace('\n', " ")
        );

        results.push(markdown_content);

        if current_depth < max_depth {
            let Ok(current_base_url) = Url::parse(&current_url) else {
                 console_warn!("Could not parse current URL '{}' as base for relative links. Skipping link discovery for this page.", current_url);
                 continue;
            };

            console_log!("Found {} links on page {}.", links.len(), current_url.chars().take(60).collect::<String>());

            for link in links {
                let absolute_url_str = match resolve_link(&current_base_url, &link, follow_relative) {
                    Some(url_str) => url_str,
                    None => {
                        continue
                    }
                };


                if absolute_url_str.len() > 512 { continue; }

                match Url::parse(&absolute_url_str) {
                    Ok(abs_parsed) => {
                        if abs_parsed.scheme() != "http" && abs_parsed.scheme() != "https" {
                            continue;
                        }

                        if let Some(ref required_domain) = base_domain {
                            if abs_parsed.domain() != Some(required_domain.as_str()) {
                                continue;
                            }
                        }

                        if visited.insert(absolute_url_str.clone()) {
                            console_log!(
                                "Queuing NEW URL: {} (Depth {})",
                                absolute_url_str.chars().take(60).collect::<String>(),
                                current_depth + 1
                            );
                            queue.push_back((absolute_url_str, current_depth + 1));
                        } else {
                        }
                    }
                    Err(_) => {
                        console_warn!("Could not parse discovered URL: {}", absolute_url_str.chars().take(80).collect::<String>());
                        continue;
                    }
                }
            }
             console_log!("Finished link check for {}", current_url.chars().take(60).collect::<String>());
        }
    }

    console_log!("Crawl finished. Collected {} results.", results.len());
    Ok(results)
}

fn resolve_link(base_url: &Url, link: &str, follow_relative: bool) -> Option<String> {
    if URL_REGEX.is_match(link) {
        Some(link.to_string())
    } else if follow_relative {
        base_url.join(link).ok().map(|u| u.to_string())
    } else {
        None
    }
}
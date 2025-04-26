use worker::*;
use futures::channel::mpsc;
use std::sync::Arc;
use futures::lock::Mutex;
use futures::{StreamExt, SinkExt};
use std::collections::HashSet;
use url::Url;
use regex::Regex;
use lazy_static::lazy_static;

use crate::config::CrawlRequest;
use crate::fetch::fetch_url_with_timeout;
use crate::markdown::html_to_markdown;

lazy_static! {
    static ref URL_REGEX: Regex = Regex::new(r"^https?://").unwrap();
}

pub async fn handle_crawl(request: CrawlRequest) -> worker::Result<Vec<String>> {
    let (mut url_tx, mut url_rx) = mpsc::unbounded::<(String, u32)>();
    let (result_tx, mut result_rx) = mpsc::unbounded::<String>();

    let visited = Arc::new(Mutex::new(HashSet::new()));
    let results_counter = Arc::new(Mutex::new(0u32));
    let limit = request.limit;
    let concurrency_limit = 6;

    let base_url_res = Url::parse(&request.url);
    let base_domain = match base_url_res {
        Ok(ref url) => url.domain().map(|s| s.to_string()),
        Err(_) => {
            return Err(Error::RustError(format!("Invalid starting URL: {}", request.url)));
        }
    };

    {
        let mut visited_set = visited.lock().await;
        if !visited_set.insert(request.url.clone()) {
            console_warn!("Initial URL {} already visited?", request.url);
            return Ok(Vec::new());
        }
        url_tx.send((request.url.clone(), 0)).await
            .map_err(|e| Error::RustError(format!("Failed to send initial URL: {}", e)))?;
    }

    console_log!("Initial URL sent to channel.");

    console_log!(
        "Starting {} workers, limit: {}, depth: {}",
        concurrency_limit,
        request.limit,
        request.max_depth
    );

    let mut worker_txs = Vec::with_capacity(concurrency_limit as usize);
    for worker_id in 0..concurrency_limit {
        let (worker_tx, mut worker_rx) = mpsc::unbounded::<(String, u32)>();
        worker_txs.push(worker_tx);

        let result_tx = result_tx.clone();
        let visited = Arc::clone(&visited);
        let results_counter = Arc::clone(&results_counter);
        let config = request.config.clone();
        let max_depth = request.max_depth;
        let follow_relative = request.follow_relative;
        let base_domain = base_domain.clone();
        let mut url_tx = url_tx.clone();
        let limit = limit;

        wasm_bindgen_futures::spawn_local(async move {
            while let Some((url, depth)) = worker_rx.next().await {
                console_log!(
                    "W{}: Processing: {} (Depth {})",
                    worker_id,
                    url.chars().take(60).collect::<String>(),
                    depth
                );

                let markdown = match fetch_url_with_timeout(&url, 10000).await {
                    Ok(html) => html_to_markdown(&html, config.clone()),
                    Err(e) => {
                        console_error!(
                            "W{}: Error fetching/processing {}: {}",
                            worker_id,
                            url.chars().take(60).collect::<String>(),
                            e
                        );
                        continue;
                    }
                };

                let should_send = {
                    let mut count = results_counter.lock().await;
                    if *count < limit {
                        *count += 1;
                        console_log!(
                            "W{}: Incremented result count to {}/{} for {}",
                            worker_id,
                            *count,
                            limit,
                            url.chars().take(60).collect::<String>()
                        );
                        true
                    } else {
                        false
                    }
                };

                if should_send {
                    if let Err(e) = result_tx.unbounded_send(markdown.markdown.clone()) {
                        console_error!("W{}: Error sending result for {}: {}", worker_id, url, e);
                    }
                }

                if depth < max_depth {
                    let count = *results_counter.lock().await;
                    if count < limit {
                        let base_url_for_join = Url::parse(&url).ok();
                        for link in markdown.links {
                            let is_absolute = URL_REGEX.is_match(&link);
                            let absolute_url = if is_absolute {
                                Some(link)
                            } else if follow_relative {
                                base_url_for_join
                                    .as_ref()
                                    .and_then(|base| base.join(&link).ok())
                                    .map(|u| u.to_string())
                            } else {
                                None
                            };

                            if let Some(absolute_url) = absolute_url {
                                if absolute_url.len() > 512 {
                                    continue;
                                }
                                if let Ok(abs_parsed) = Url::parse(&absolute_url) {
                                    if let Some(ref domain_filter) = base_domain {
                                        if abs_parsed.domain() != Some(domain_filter.as_str()) {
                                            continue;
                                        }
                                    }
                                    if abs_parsed.scheme() != "http" && abs_parsed.scheme() != "https" {
                                        continue;
                                    }

                                    let mut visited_set = visited.lock().await;
                                    if visited_set.insert(absolute_url.clone()) {
                                        console_log!(
                                            "W{}: Queuing: {} (Depth {})",
                                            worker_id,
                                            absolute_url.chars().take(60).collect::<String>(),
                                            depth + 1
                                        );
                                        if let Err(e) = url_tx.send((absolute_url.clone(), depth + 1)).await {
                                            console_error!(
                                                "W{}: Error sending URL {}: {}",
                                                worker_id,
                                                absolute_url.chars().take(60).collect::<String>(),
                                                e
                                            );
                                            visited_set.remove(&absolute_url);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            console_log!("W{}: Finished processing.", worker_id);
        });
    }

    wasm_bindgen_futures::spawn_local(async move {
        let mut next_worker = 0;

        while let Some((url, depth)) = url_rx.next().await {
            if let Some(worker_tx) = worker_txs.get(next_worker) {
                if let Err(e) = worker_tx.unbounded_send((url.clone(), depth)) {
                    console_error!("Error sending URL to worker {}: {}", next_worker, e);
                }
            }

            next_worker = (next_worker + 1) % worker_txs.len();
        }
    });

    drop(result_tx);
    console_log!("Original result sender dropped.");

    let mut results = Vec::with_capacity(request.limit as usize);
    while results.len() < request.limit as usize {
        match result_rx.next().await {
            Some(markdown) => {
                results.push(markdown);
                console_log!("Collected result {}/{}", results.len(), request.limit);
            }
            None => {
                console_log!("Result channel closed.");
                break;
            }
        }
    }

    console_log!("Collected {} results (limit was {}).", results.len(), request.limit);
    Ok(results)
}
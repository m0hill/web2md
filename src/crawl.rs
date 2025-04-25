use worker::*;
use futures::channel::mpsc;
use std::sync::Arc;
use tokio::sync::Semaphore;
use futures::lock::Mutex;
use futures::{future, StreamExt, SinkExt};
use std::collections::HashSet;
use url::Url;
use regex::Regex;
use lazy_static::lazy_static;

use crate::config::CrawlRequest; // Keep CrawlRequest
use crate::fetch::fetch_url_with_timeout;
use crate::markdown::html_to_markdown;

lazy_static! {
    static ref URL_REGEX: Regex = Regex::new(r"^https?://").unwrap();
}

pub async fn handle_crawl(request: CrawlRequest) -> worker::Result<Vec<String>> {
    let (result_tx, mut result_rx) = mpsc::unbounded::<String>();
    let (url_tx, url_rx) = mpsc::unbounded::<(String, u32)>();
    let visited = Arc::new(Mutex::new(HashSet::new()));
    let semaphore = Arc::new(Semaphore::new(6));

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
             return Ok(Vec::new());
        }
    }

    let url_tx_clone = Arc::new(Mutex::new(url_tx));
     {
        let mut url_tx_locked = url_tx_clone.lock().await;
        url_tx_locked.send((request.url.clone(), 0)).await.map_err(|e| Error::RustError(format!("Failed to send initial URL: {}", e)))?;
     }


    let worker_count = 6;
    let url_rx_arc = Arc::new(Mutex::new(url_rx));
    let results_counter = Arc::new(Mutex::new(0u32));

    let workers = (0..worker_count).map(|_| {
        let url_rx_worker = Arc::clone(&url_rx_arc);
        let mut result_tx_worker = result_tx.clone();
        let visited_worker = Arc::clone(&visited);
        let semaphore_worker = Arc::clone(&semaphore);
        let config_worker = request.config.clone();
        let url_tx_worker = Arc::clone(&url_tx_clone);
        let max_depth_worker = request.max_depth;
        let limit_worker = request.limit;
        // *** Use the follow_relative flag ***
        let follow_relative_worker = request.follow_relative;
        let base_domain_worker = base_domain.clone();
        let results_counter_worker = Arc::clone(&results_counter);


        async move {
            loop {
                let url_opt = {
                    let mut url_rx_locked = url_rx_worker.lock().await;
                    let current_count = *results_counter_worker.lock().await;
                    if current_count >= limit_worker {
                         break;
                    }
                    url_rx_locked.next().await
                };

                let (current_url, depth) = match url_opt {
                    Some(pair) => pair,
                    None => break,
                };

                {
                    let current_count = *results_counter_worker.lock().await;
                    if current_count >= limit_worker {
                        break;
                    }
                }

                console_log!("Worker processing: {} (Depth: {})", current_url, depth);

                let permit = match semaphore_worker.acquire().await {
                    Ok(p) => p,
                    Err(_) => break,
                };

                match fetch_url_with_timeout(&current_url, 10000).await {
                    Ok(html) => {
                        let conversion_result = html_to_markdown(&html, config_worker.clone());

                        let send_result = {
                             let mut count = results_counter_worker.lock().await;
                             if *count < limit_worker {
                                 *count += 1;
                                 true
                             } else {
                                 false
                             }
                        };

                        if send_result {
                             if let Err(e) = result_tx_worker.send(conversion_result.markdown).await {
                                console_error!("Error sending result for {}: {}", current_url, e);
                                let mut count = results_counter_worker.lock().await;
                                *count -= 1;
                            }
                        } else {
                             console_log!("Limit reached, skipping result send for {}", current_url);
                        }

                        if depth < max_depth_worker {
                            let base_url_for_join = Url::parse(&current_url).ok();

                            for link in conversion_result.links {
                                // *** Check if the link is absolute ***
                                let is_absolute = URL_REGEX.is_match(&link);

                                // *** Decide whether to process the link based on follow_relative ***
                                let absolute_url_str = if is_absolute {
                                    // Already absolute, use as is (after domain/scheme check)
                                    Some(link.clone())
                                } else if follow_relative_worker {
                                    // It's relative AND we should follow relatives
                                    match base_url_for_join {
                                        Some(ref base) => base.join(&link).ok().map(|u| u.to_string()),
                                        None => {
                                             console_log!("Could not parse base URL '{}' to resolve relative link '{}'", current_url, link);
                                             None // Skip if base URL is invalid
                                        }
                                    }
                                } else {
                                    // It's relative and we should NOT follow relatives
                                    console_log!("Skipping relative link (follow_relative=false): {}", link);
                                    None // Skip
                                };

                                // *** Process the absolute URL string if we have one ***
                                if let Some(absolute_url) = absolute_url_str {
                                    // Re-parse to easily check domain and scheme
                                    if let Ok(abs_parsed) = Url::parse(&absolute_url) {
                                        // Check domain filter
                                        if let Some(ref domain_filter) = base_domain_worker {
                                            if abs_parsed.domain() != Some(domain_filter.as_str()) {
                                                console_log!("Skipping external link: {}", absolute_url);
                                                continue;
                                            }
                                        }
                                        // Check scheme filter
                                        if abs_parsed.scheme() != "http" && abs_parsed.scheme() != "https" {
                                            console_log!("Skipping non-HTTP(S) link: {}", absolute_url);
                                            continue;
                                        }

                                        // If all checks pass, proceed with visited check and queueing
                                        let mut visited_set = visited_worker.lock().await;
                                        if visited_set.insert(absolute_url.clone()) {
                                            let current_count = *results_counter_worker.lock().await;
                                            if current_count < limit_worker {
                                                console_log!("Queueing: {} (Depth: {})", absolute_url, depth + 1);
                                                let mut url_tx_locked = url_tx_worker.lock().await;
                                                if let Err(e) = url_tx_locked.send((absolute_url, depth + 1)).await {
                                                    console_error!("Error sending URL to queue: {}", e);
                                                }
                                            } else {
                                                console_log!("Limit reached, stopping further URL queueing from {}", current_url);
                                                // break; // Optionally break inner loop
                                            }
                                        }
                                    } else {
                                         console_log!("Could not re-parse potential absolute URL: {}", absolute_url);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => console_error!("Error crawling {}: {}", current_url, e),
                }
                drop(permit);
            }
            drop(result_tx_worker);
            console_log!("Worker finished.");
        }
    }).collect::<Vec<_>>();

    drop(result_tx);
    drop(url_tx_clone);

    future::join_all(workers).await;
    console_log!("All workers finished.");

    let mut results = Vec::new();
    while let Some(markdown) = result_rx.next().await {
        if results.len() >= request.limit as usize {
            break;
        }
        results.push(markdown);
    }
    console_log!("Collected {} results.", results.len());

    results.truncate(request.limit as usize);

    Ok(results)
}
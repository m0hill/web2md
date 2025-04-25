use worker::*;
use std::time::Duration;
use crate::fingerprint::FingerprintCache;

pub async fn fetch_url_with_timeout(url: &str, _timeout_ms: u32) -> worker::Result<String> {
    // Note: Cloudflare Workers don't have explicit request timeouts like standard async runtimes.
    // The platform imposes its own limits. The timeout_ms parameter is kept for potential future use
    // or adaptation but isn't directly used in the Fetch API here.

    let mut opts = RequestInit::new();
    opts.method = Method::Get;

    let cache = FingerprintCache::new(); // Or get from a static instance
    let fingerprint = cache.get_random();
    let mut headers = Headers::new(); // Create new headers
    fingerprint.apply_to_headers(&mut headers)?; // Apply the fingerprint headers
    opts.headers = headers; // Assign the generated headers to the request options

    console_log!("Fetching URL: {}", url);

    let mut retry_count = 0;
    let max_retries = 3;

    loop {
        let request = Request::new_with_init(url, &opts)?;
        let mut response = Fetch::Request(request).send().await?;

        if response.status_code() >= 400 {
            if response.status_code() == 429 || response.status_code() == 403 {
                if retry_count >= max_retries {
                    return Err(worker::Error::RustError(format!(
                        "Rate limit or access denied after {} retries for URL {}",
                        max_retries, url
                    )));
                }
                console_error!("Rate limit or access denied for {}, retrying...", url);
                worker::Delay::from(Duration::from_secs(2u64.pow(retry_count))).await; // Exponential backoff
                retry_count += 1;
                continue;
            }

            if response.status_code() == 503 {
                 // Often indicates service unavailable or sometimes CAPTCHA-like blocks
                if retry_count >= max_retries {
                    return Err(worker::Error::RustError(format!(
                        "Service unavailable (503) after {} retries for URL {}",
                         max_retries, url
                    )));
                }
                console_error!("Service unavailable (503) for {}, retrying...", url);
                worker::Delay::from(Duration::from_secs(3u64.pow(retry_count))).await; // Longer backoff for 503
                retry_count += 1;
                continue;
            }

            console_error!("Fetch error on attempt {} for {}: Status {}", retry_count + 1, url, response.status_code());
            return Err(worker::Error::RustError(format!(
                "HTTP error {} for URL {}",
                response.status_code(), url
            )));
        }

        match response.text().await {
            Ok(text) => return Ok(text),
            Err(e) => {
                console_error!("Text extraction error for {}: {:?}", url, e);
                // Don't retry on text extraction error, likely a non-HTML response or corrupted data
                return Err(worker::Error::RustError(format!("Text extraction failed for {}: {}", url, e)));
            }
        }
    }
}
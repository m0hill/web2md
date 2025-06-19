-----

# web2md - Web Scraper & Markdown Converter

[](https://www.rust-lang.org/)
[](https://workers.cloudflare.com/)

`getmd` is a high-performance, resilient web scraper and HTML-to-Markdown converter deployed as a Cloudflare Worker. It leverages Rust and WebAssembly to deliver exceptional speed and reliability for converting any webpage into clean, readable Markdown.

The service is live and can be accessed at: `https://scrape.mohil.dev/`

## Core Features

  * **High-Quality Markdown Conversion**: Translates complex HTML structures—including tables, lists, code blocks, and blockquotes—into well-formatted Markdown.
  * **Website Crawler**: Recursively crawls a website from a starting URL, converting multiple pages in a single run, with configurable depth and page limits.
  * **Advanced Anti-Scraping Evasion**: Utilizes sophisticated, randomized browser fingerprinting to mimic real user requests, successfully bypassing many anti-bot measures.
  * **Intelligent Metadata Extraction**: Automatically extracts titles, descriptions, authors, and keywords from `<meta>` tags and formats them into a clean YAML front-matter block.
  * **Robust & Resilient**: Implements automatic retries with exponential backoff for common HTTP errors (`429`, `403`, `503`), ensuring a higher success rate.
  * **Highly Performant**: Built with Rust and compiled to WebAssembly, `getmd` runs on Cloudflare's edge network for minimal latency.

-----

## API Usage

You can interact with the `getmd` service through three primary endpoints.

### 1\. Simple GET Request

The quickest way to convert a single page. Simply append the full URL of the target page to the service endpoint.

**Method**: `GET`  
**Endpoint**: `https://scrape.mohil.dev/{URL_TO_SCRAPE}`

**Example:**

```bash
# Get the Markdown for the Rust language homepage
curl "https://scrape.mohil.dev/https://www.rust-lang.org"
```

### 2\. Advanced POST Request (Single URL)

This endpoint provides fine-grained control over the conversion process for a single URL.

**Method**: `POST`  
**Endpoint**: `https://scrape.mohil.dev/`  
**Body**: JSON payload with `url` and an optional `config` object.

**Example:**

```bash
curl --request POST 'https://scrape.mohil.dev/' \
--header 'Content-Type: application/json' \
--data-raw '{
    "url": "https://example.com",
    "config": {
        "include_links": false,
        "clean_whitespace": true,
        "preserve_headings": true,
        "include_metadata": true
    }
}'
```

#### `ConvertConfig` Options:

All configuration options are `false` by default unless specified.

| Parameter             | Type      | Description                                                                 |
| --------------------- | --------- | --------------------------------------------------------------------------- |
| `include_links`       | `boolean` | If `true`, includes hyperlinks (`<a>` tags) in the output.                   |
| `clean_whitespace`    | `boolean` | If `true`, collapses multiple whitespace characters into a single space.      |
| `preserve_headings`   | `boolean` | If `true`, converts `<h1>`-`<h6>` tags to Markdown headings.                 |
| `include_metadata`    | `boolean` | If `true`, extracts page metadata and adds it as YAML front-matter.         |
| `max_heading_level`   | `number`  | The maximum heading level to include (1-6). Default is `6`.                 |
| `cleaning_rules`      | `object`  | A nested object for fine-grained cleaning rules.                            |
| ↳ `remove_scripts`    | `boolean` | Strips all `<script>` tags and their content.                               |
| ↳ `remove_styles`     | `boolean` | Strips all `<style>` tags and their content.                                |
| ↳ `remove_comments`   | `boolean` | Strips all HTML comments.                                                   |
| ↳ `preserve_line_breaks`| `boolean`| If `true`, attempts to preserve line breaks within text blocks.           |

### 3\. Crawl Request

This endpoint crawls a website starting from a given URL and returns the Markdown for all successfully scraped pages, concatenated by a separator.

**Method**: `POST`  
**Endpoint**: `https://scrape.mohil.dev/crawl`  
**Body**: JSON payload specifying the crawl parameters.

**Example:**

```bash
curl --request POST 'https://scrape.mohil.dev/crawl' \
--header 'Content-Type: application/json' \
--data-raw '{
    "url": "https://docs.rs/worker/latest/worker/",
    "limit": 5,
    "max_depth": 2,
    "follow_relative": true,
    "config": {
        "include_links": true,
        "include_metadata": false
    }
}'
```

#### `CrawlRequest` Parameters:

| Parameter         | Type      | Description                                                                    |
| ----------------- | --------- | ------------------------------------------------------------------------------ |
| `url`             | `string`  | **Required.** The starting URL for the crawl.                                  |
| `limit`           | `number`  | **Required.** The maximum number of pages to crawl.                            |
| `max_depth`       | `number`  | **Required.** The maximum link depth to follow from the starting URL. `0` means only the starting page. |
| `follow_relative` | `boolean` | If `true`, the crawler will follow relative links (e.g., `/page/about`). Default `false`. |
| `config`          | `object`  | An optional `ConvertConfig` object (see above) to apply to every crawled page. |

#### Crawl Output

The crawled Markdown documents are concatenated and separated by `\n\n---\n\n`.

-----

## Local Development & Deployment

You can run and deploy your own instance of `getmd`.

### Prerequisites

  * [Rust](https://www.rust-lang.org/tools/install)
  * [Wrangler CLI](https://developers.cloudflare.com/workers/wrangler/install-and-update/)
  * [Node.js and npm](https://nodejs.org/en/)

### Setup

1.  **Clone the repository:**

    ```bash
    git clone <repository-url>
    cd getmd
    ```

2.  **Install dependencies:**

    ```bash
    npm install
    ```

3.  **Log in to Wrangler:**

    ```bash
    npx wrangler login
    ```

### Running Locally

To start a local development server that live-reloads on changes:

```bash
npx wrangler dev
```

### Building & Deploying

1.  **Build for production:**
    The build command from `wrangler.toml` will be executed automatically on deploy. To run it manually:

    ```bash
    cargo install -q worker-build && worker-build --release
    ```

2.  **Deploy to Cloudflare:**
    Make sure to edit the `wrangler.toml` file to use your own Cloudflare account details and desired route.

    ```bash
    npx wrangler deploy
    ```

    This will deploy the worker to the route specified in `wrangler.toml`.

-----

## Author

  * **Mohil Garg** - [mohil.garg13@gmail.com](mailto:mohil.garg13@gmail.com)

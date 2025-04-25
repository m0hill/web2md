use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ConvertRequest {
    pub url: String,
    #[serde(default)]
    pub config: ConvertConfig,
}

#[derive(Debug, Deserialize)]
pub struct CrawlRequest {
    pub url: String,
    pub limit: u32,
    pub max_depth: u32,
    #[serde(default)]
    pub config: ConvertConfig,
    #[serde(default)]
    pub follow_relative: bool,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct ConvertConfig {
    pub include_links: bool,
    pub clean_whitespace: bool,
    #[serde(default)]
    pub cleaning_rules: CleaningRules,
    #[serde(default)]
    pub preserve_headings: bool,
    #[serde(default)]
    pub include_metadata: bool,
    #[serde(default)]
    pub max_heading_level: u8,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct CleaningRules {
    pub remove_scripts: bool,
    pub remove_styles: bool,
    pub remove_comments: bool,
    pub preserve_line_breaks: bool,
}

#[derive(Debug, Serialize)]
pub struct CrawlResult {
    pub url: String,
    pub markdown: String,
    pub depth: u32,
}

#[derive(Debug)]
pub struct HtmlConversionResult {
    pub markdown: String,
    pub links: Vec<String>,
}